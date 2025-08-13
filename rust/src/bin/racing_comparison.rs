use clap::Parser;
use futures::StreamExt;
use helius_laserstream::{
    grpc::{SubscribeRequest, SubscribeRequestFilterBlocks},
    subscribe, LaserstreamConfig,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "racing-comparison")]
#[command(about = "Real-time racing comparison between RPC and LaserStream")]
struct Args {
    #[arg(long, help = "Helius API key")]
    api_key: Option<String>,

    #[arg(long, default_value = "https://laserstream-mainnet-tyo.helius-rpc.com", help = "Helius Laserstream endpoint")]
    endpoint: String,

    #[arg(long, default_value = "../shared/config.json", help = "Config file path")]
    config: String,

    #[arg(long, default_value = "3", help = "Test duration in minutes")]
    duration: u64,

    #[arg(long, help = "Verbose logging")]
    verbose: bool,
}

#[derive(serde::Deserialize)]
struct Config {
    rpcs: HashMap<String, RPCConfig>,
}

#[derive(serde::Deserialize, Clone)]
struct RPCConfig {
    name: String,
    url: String,
    provider: String,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Clone)]
struct BlockEvent {
    slot: u64,
    block_time: Option<i64>,
    received_time: i64,
    source: String,
    latency_ms: Option<i64>,
}

type SharedBlocks = Arc<Mutex<HashMap<u64, (Option<BlockEvent>, Option<BlockEvent>)>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let api_key = args.api_key.clone()
        .or_else(|| std::env::var("HELIUS_API_KEY").ok())
        .unwrap_or_else(|| "9de07723-0030-4ee0-b175-6722231d5d97".to_string());

    println!("🏁 Real-Time Block Detection Race");
    println!("LaserStream vs RPC - Who gets the block first?");
    println!("Duration: {} minutes", args.duration);
    println!("LaserStream endpoint: {}", args.endpoint);
    println!();

    let config = load_config(&args.config)?;
    
    // Get premium RPC (Helius) or fallback to first active RPC
    let rpc = config.rpcs.values()
        .find(|r| r.provider == "Helius" && r.status == "active")
        .or_else(|| config.rpcs.values().find(|r| r.status == "active"))
        .ok_or_else(|| anyhow::anyhow!("No active RPCs found"))?;

    println!("RPC Provider: {} ({})", rpc.name, rpc.provider);
    println!("RPC URL: {}", rpc.url);
    println!();

    // Shared state for tracking blocks from both sources
    let shared_blocks: SharedBlocks = Arc::new(Mutex::new(HashMap::new()));

    // Start LaserStream monitoring
    let laserstream_handle = tokio::spawn(monitor_laserstream(
        api_key.clone(),
        args.endpoint.clone(),
        args.duration,
        shared_blocks.clone(),
        args.verbose
    ));

    // Start RPC monitoring
    let rpc_handle = tokio::spawn(monitor_rpc(
        rpc.clone(),
        args.duration,
        shared_blocks.clone(),
        args.verbose
    ));

    println!("🚀 Starting the race...");
    println!("🏆 First to detect each new block wins!");
    println!();
    println!("Slot       | Winner           | LaserStream     | RPC            | Advantage   | Status");
    println!("{}", "-".repeat(75));

    // Wait for both to complete
    let _ = tokio::join!(laserstream_handle, rpc_handle);

    Ok(())
}

async fn monitor_laserstream(
    api_key: String,
    endpoint: String,
    duration_minutes: u64,
    shared_blocks: SharedBlocks,
    verbose: bool
) -> Result<()> {
    let config = LaserstreamConfig {
        api_key,
        endpoint: endpoint.parse()?,
        ..Default::default()
    };

    let mut block_filters = HashMap::new();
    block_filters.insert(
        "all_blocks".to_string(),
        SubscribeRequestFilterBlocks {
            account_include: vec![],
            include_transactions: Some(false),
            include_accounts: Some(false),
            include_entries: Some(false),
        },
    );

    let request = SubscribeRequest {
        blocks: block_filters,
        ..Default::default()
    };

    let (stream, _handle) = subscribe(config, request);
    futures::pin_mut!(stream);

    let start_time = SystemTime::now();
    let duration = Duration::from_secs(duration_minutes * 60);

    while start_time.elapsed()? < duration {
        if let Some(result) = stream.next().await {
            match result {
                Ok(update) => {
                    let received_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)?
                        .as_millis() as i64;

                    if let Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Block(block)) = update.update_oneof {
                        let slot = block.slot;
                        let block_time = block.block_time.map(|bt| bt.timestamp);
                        let latency = block_time.map(|bt| received_time - (bt * 1000));

                        let block_event = BlockEvent {
                            slot,
                            block_time,
                            received_time,
                            source: "LaserStream".to_string(),
                            latency_ms: latency,
                        };

                        // Update shared state and check if we can announce a winner
                        let mut blocks = shared_blocks.lock().await;
                        let entry = blocks.entry(slot).or_insert((None, None));
                        entry.0 = Some(block_event.clone());

                        // Only announce winner when we have both results for this slot
                        if let (Some(ls_event), Some(rpc_event)) = (&entry.0, &entry.1) {
                            announce_winner(slot, ls_event, rpc_event);
                        }
                        // Otherwise, silently wait for the other service to catch up
                    }
                }
                Err(e) => {
                    if verbose {
                        eprintln!("❌ LaserStream error: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

async fn monitor_rpc(
    rpc: RPCConfig,
    duration_minutes: u64,
    shared_blocks: SharedBlocks,
    verbose: bool
) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let start_time = SystemTime::now();
    let duration = Duration::from_secs(duration_minutes * 60);
    
    let current_slot = get_latest_slot(&client, &rpc.url).await?;
    let mut last_slot = current_slot;

    while start_time.elapsed()? < duration {
        match get_latest_slot(&client, &rpc.url).await {
            Ok(current_slot) => {
                if current_slot > last_slot {
                    // Process only the latest slot for real-time comparison
                    match get_block_time(&client, &rpc.url, current_slot).await {
                        Ok(Some(block_time)) => {
                            let received_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)?
                                .as_millis() as i64;
                            
                            let latency = received_time - (block_time * 1000);

                            let block_event = BlockEvent {
                                slot: current_slot,
                                block_time: Some(block_time),
                                received_time,
                                source: "RPC".to_string(),
                                latency_ms: Some(latency),
                            };

                            // Update shared state and check if we can announce a winner
                            let mut blocks = shared_blocks.lock().await;
                            let entry = blocks.entry(current_slot).or_insert((None, None));
                            entry.1 = Some(block_event.clone());

                            // Only announce winner when we have both results for this slot
                            if let (Some(ls_event), Some(rpc_event)) = (&entry.0, &entry.1) {
                                announce_winner(current_slot, ls_event, rpc_event);
                            }
                            // Otherwise, silently wait for the other service to catch up
                        }
                        Ok(None) => {
                            if verbose {
                                println!("RPC    | {} | Block time not available yet", current_slot);
                            }
                        }
                        Err(e) => {
                            if verbose {
                                println!("RPC    | {} | Error: {}", current_slot, e);
                            }
                        }
                    }
                    last_slot = current_slot;
                }
            }
            Err(e) => {
                if verbose {
                    eprintln!("❌ RPC slot error: {}", e);
                }
            }
        }
        
        time::sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

fn announce_winner(slot: u64, ls_event: &BlockEvent, rpc_event: &BlockEvent) {
    let ls_latency = ls_event.latency_ms.unwrap_or(0);
    let rpc_latency = rpc_event.latency_ms.unwrap_or(0);
    
    let (winner, advantage) = if ls_event.received_time < rpc_event.received_time {
        let diff = rpc_event.received_time - ls_event.received_time;
        ("🏆 LaserStream", format!("{}ms", diff))
    } else if rpc_event.received_time < ls_event.received_time {
        let diff = ls_event.received_time - rpc_event.received_time;
        ("🏆 RPC", format!("{}ms", diff))
    } else {
        ("🤝 Tie", "Same time".to_string())
    };

    let overall_status = if ls_latency < 900 || rpc_latency < 900 {
        "🟢 EXCELLENT"
    } else if ls_latency < 1200 || rpc_latency < 1200 {
        "🟡 GOOD"
    } else if ls_latency < 2000 || rpc_latency < 2000 {
        "🟠 FAIR"
    } else if ls_latency < 900 || rpc_latency < 900 {
        "🟢 EXCELLENT"
    } else if ls_latency < 1200 || rpc_latency < 1200 {
        "🟡 GOOD"
    } else if ls_latency < 2000 || rpc_latency < 2000 {
        "🟠 FAIR"
    } else {
        "🔴 SLOW"
    };

    println!("{:<10} | {:<15} | {:<15} | {:<15} | {:<9} | {}", 
        slot, winner, 
        format!("{}ms", ls_latency),
        format!("{}ms", rpc_latency), 
        advantage, overall_status);
}

fn get_latency_status(latency_ms: i64) -> &'static str {
    if latency_ms < 900 {
        "🟢 EXCELLENT"
    } else if latency_ms < 1200 {
        "🟡 GOOD"
    } else if latency_ms < 2000 {
        "🟠 FAIR"
    } else {
        "🔴 SLOW"
    }
}

async fn get_latest_slot(client: &Client, rpc_url: &str) -> Result<u64> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getSlot",
        "params": [{"commitment": "processed"}]
    });

    let response = client.post(rpc_url).json(&request).send().await?;
    let json_value: Value = response.json().await?;

    if let Some(slot) = json_value.get("result").and_then(|v| v.as_u64()) {
        Ok(slot)
    } else if let Some(error) = json_value.get("error") {
        Err(anyhow::anyhow!("getSlot error: {}", error))
    } else {
        Err(anyhow::anyhow!("Failed to get slot"))
    }
}

async fn get_block_time(client: &Client, rpc_url: &str, slot: u64) -> Result<Option<i64>> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBlockTime",
        "params": [slot]
    });

    let response = client.post(rpc_url).json(&request).send().await?;
    let json_value: Value = response.json().await?;

    if let Some(error) = json_value.get("error") {
        return Ok(None); // getBlockTime might fail for very recent slots
    }

    if let Some(result) = json_value.get("result") {
        if result.is_null() {
            Ok(None)
        } else {
            Ok(result.as_i64())
        }
    } else {
        Ok(None)
    }
}

fn load_config(config_path: &str) -> Result<Config> {
    let content = fs::read_to_string(config_path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}