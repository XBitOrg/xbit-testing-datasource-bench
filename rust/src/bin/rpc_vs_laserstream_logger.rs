use anyhow::Result;
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
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;

#[derive(Parser)]
#[command(name = "rpc-vs-laserstream-logger")]
#[command(about = "Log block information from both RPC and Laserstream for comparison")]
struct Args {
    #[arg(long, help = "Helius API key")]
    api_key: Option<String>,

    #[arg(
        long,
        default_value = "https://laserstream-mainnet-tyo.helius-rpc.com",
        help = "Helius Laserstream endpoint"
    )]
    endpoint: String,

    #[arg(
        long,
        default_value = "../shared/config.json",
        help = "Config file path"
    )]
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
struct BlockInfo {
    slot: u64,
    block_time: Option<i64>,
    received_time: i64,
    source: String, // "Laserstream" or "RPC"
    parent_slot: Option<u64>,
    block_height: Option<u64>,
    transaction_count: Option<usize>,
    // Laserstream-specific fields
    laserstream_created_time: Option<i64>, // When Laserstream processed the block
    network_latency_ms: Option<i64>,       // Laserstream delivery speed
    propagation_latency_ms: Option<i64>,   // Block creation to receipt
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let api_key = args
        .api_key
        .clone()
        .or_else(|| std::env::var("HELIUS_API_KEY").ok())
        .unwrap_or_else(|| "9de07723-0030-4ee0-b175-6722231d5d97".to_string());

    println!("🔍 RPC vs Laserstream Block Information Logger");
    println!("Comparing block data from both sources");
    println!("Duration: {} minutes", args.duration);
    println!("Laserstream endpoint: {}", args.endpoint);
    println!();

    let config = load_config(&args.config)?;

    // Get premium RPC (Helius) or fallback to first active RPC
    let rpc = config
        .rpcs
        .values()
        .find(|r| r.provider == "Helius" && r.status == "active")
        .or_else(|| config.rpcs.values().find(|r| r.status == "active"))
        .ok_or_else(|| anyhow::anyhow!("No active RPCs found"))?;

    println!("RPC Provider: {} ({})", rpc.name, rpc.provider);
    println!("RPC URL: {}", rpc.url);
    println!();

    let mut all_blocks = Vec::new();

    // Start both monitoring tasks
    let laserstream_handle = tokio::spawn(monitor_laserstream(
        api_key.clone(),
        args.endpoint.clone(),
        args.duration,
        args.verbose,
    ));

    let rpc_handle = tokio::spawn(monitor_rpc(rpc.clone(), args.duration, args.verbose));

    println!("🚀 Starting dual monitoring...");
    println!("📡 Laserstream: Real-time gRPC stream");
    println!("🌐 RPC: HTTP polling every 400ms");
    println!();
    println!("Block Format:");
    println!("SOURCE     | Slot     | Block Time | Received | Network Lat. | Propagation | Parent | Height | TXs");
    println!("{}", "-".repeat(95));

    // Wait for both to complete
    let (laserstream_result, rpc_result) = tokio::join!(laserstream_handle, rpc_handle);

    let laserstream_blocks = laserstream_result??;
    let rpc_blocks = rpc_result??;

    all_blocks.extend(laserstream_blocks);
    all_blocks.extend(rpc_blocks);

    // Sort by slot for comparison
    all_blocks.sort_by_key(|b| b.slot);

    println!();
    println!("📊 Final Summary");
    print_block_comparison(&all_blocks);

    Ok(())
}

async fn monitor_laserstream(
    api_key: String,
    endpoint: String,
    duration_minutes: u64,
    verbose: bool,
) -> Result<Vec<BlockInfo>> {
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
            include_transactions: Some(true), // Get transaction data
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

    let mut blocks = Vec::new();
    let start_time = SystemTime::now();
    let duration = Duration::from_secs(duration_minutes * 60);

    while start_time.elapsed()? < duration {
        if let Some(result) = stream.next().await {
            match result {
                Ok(update) => {
                    let received_time =
                        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

                    // Print raw Laserstream update as JSON
                    println!("🔥 LASERSTREAM RAW UPDATE:");
                    println!("{{");
                    println!("  \"received_at\": {},", received_time);
                    println!("  \"filters\": {:?},", update.filters);
                    println!("  \"created_at\": {:?},", update.created_at);
                    println!("  \"update_type\": \"{}\"", match &update.update_oneof {
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Block(_)) => "Block",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Transaction(_)) => "Transaction",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Account(_)) => "Account",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Slot(_)) => "Slot",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::TransactionStatus(_)) => "TransactionStatus",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::BlockMeta(_)) => "BlockMeta",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Entry(_)) => "Entry",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Ping(_)) => "Ping",
                        Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Pong(_)) => "Pong",
                        None => "None"
                    });
                    println!("}}");

                    if let Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Block(
                        block,
                    )) = update.update_oneof
                    {
                        // Calculate both types of latency
                        let laserstream_created_time = update
                            .created_at
                            .map(|ts| (ts.seconds * 1000) + (ts.nanos as i64 / 1_000_000));

                        let network_latency =
                            laserstream_created_time.map(|created| received_time - created);

                        let block_time = block.block_time.map(|bt| bt.timestamp);
                        let propagation_latency = block_time.map(|bt| received_time - (bt * 1000));

                        println!("📦 LASERSTREAM BLOCK DETAILS:");
                        println!("{{");
                        println!("  \"slot\": {},", block.slot);
                        println!("  \"parent_slot\": {},", block.parent_slot);
                        println!(
                            "  \"block_height\": {:?},",
                            block.block_height.as_ref().map(|bh| bh.block_height)
                        );
                        println!("  \"block_time\": {:?},", block_time);
                        println!(
                            "  \"laserstream_created_time\": {:?},",
                            laserstream_created_time.map(|t| t / 1000)
                        );
                        println!("  \"network_latency_ms\": {:?},", network_latency);
                        println!("  \"propagation_latency_ms\": {:?},", propagation_latency);
                        println!("  \"transaction_count\": {},", block.transactions.len());
                        println!("  \"blockhash\": \"{}\",", block.blockhash);
                        println!("  \"parent_blockhash\": \"{}\",", block.parent_blockhash);
                        println!(
                            "  \"rewards_count\": {}",
                            block
                                .rewards
                                .map(|rewards| rewards.rewards.len())
                                .unwrap_or(0)
                        );
                        println!("}}");
                        println!();
                        println!();

                        let slot = block.slot;
                        let parent_slot = block.parent_slot;
                        let block_height =
                            block.block_height.map(|bh| bh.block_height).unwrap_or(0);
                        let tx_count = block.transactions.len();

                        let block_info = BlockInfo {
                            slot,
                            block_time,
                            received_time,
                            source: "LASERSTREAM".to_string(),
                            parent_slot: Some(parent_slot),
                            block_height: Some(block_height),
                            transaction_count: Some(tx_count),
                            laserstream_created_time,
                            network_latency_ms: network_latency,
                            propagation_latency_ms: propagation_latency,
                        };

                        // Log block information
                        log_block_info(&block_info, verbose);
                        blocks.push(block_info);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Laserstream error: {}", e);
                }
            }
        }
    }

    Ok(blocks)
}

async fn monitor_rpc(
    rpc: RPCConfig,
    duration_minutes: u64,
    verbose: bool,
) -> Result<Vec<BlockInfo>> {
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let mut blocks = Vec::new();
    let start_time = SystemTime::now();
    let duration = Duration::from_secs(duration_minutes * 60);

    let current_slot = get_latest_slot(&client, &rpc.url).await?;
    let mut last_slot = current_slot;

    while start_time.elapsed()? < duration {
        match get_latest_slot(&client, &rpc.url).await {
            Ok(current_slot) => {
                if current_slot > last_slot {
                    for slot in (last_slot + 1)..=current_slot {
                        match get_block_info(&client, &rpc.url, slot).await {
                            Ok(Some(block_info)) => {
                                log_block_info(&block_info, verbose);
                                blocks.push(block_info);
                            }
                            Ok(None) => {
                                if verbose {
                                    println!("RPC      | {} | Block not available", slot);
                                }
                            }
                            Err(e) => {
                                if verbose {
                                    eprintln!("RPC      | {} | Error: {}", slot, e);
                                }
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

        time::sleep(Duration::from_millis(900)).await; // Moderate polling for premium RPC
    }

    Ok(blocks)
}

async fn get_latest_slot(client: &Client, rpc_url: &str) -> Result<u64> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getSlot",
        "params": [{"commitment": "processed"}]
    });

    let response = client.post(rpc_url).json(&request).send().await?;
    let response_text = response.text().await?;
    let json_value: Value = serde_json::from_str(&response_text)?;

    // Print raw RPC getSlot response
    println!("🌐 RPC getSlot RESPONSE:");
    println!(
        "{}",
        serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| response_text.clone())
    );
    println!();

    if let Some(slot) = json_value.get("result").and_then(|v| v.as_u64()) {
        Ok(slot)
    } else {
        Err(anyhow::anyhow!("Failed to get slot"))
    }
}

async fn get_block_info(client: &Client, rpc_url: &str, slot: u64) -> Result<Option<BlockInfo>> {
    let received_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBlock",
        "params": [
            slot,
            {
                "encoding": "json",
                "commitment": "processed",
                "maxSupportedTransactionVersion": 0,
                "rewards": false,
                "transactionDetails": "signatures"
            }
        ]
    });

    let response = client.post(rpc_url).json(&request).send().await?;
    let response_text = response.text().await?;
    let json_value: Value = serde_json::from_str(&response_text)?;

    // Print raw RPC getBlock response
    println!("🌐 RPC RAW RESPONSE for slot {}:", slot);
    println!(
        "{}",
        serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| response_text.clone())
    );
    println!();

    if let Some(result) = json_value.get("result") {
        if result.is_null() {
            return Ok(None);
        }

        let block_time = result.get("blockTime").and_then(|v| v.as_i64());
        let parent_slot = result.get("parentSlot").and_then(|v| v.as_u64());
        let block_height = result.get("blockHeight").and_then(|v| v.as_u64());

        let tx_count = result
            .get("transactions")
            .and_then(|txs| txs.as_array())
            .map(|arr| arr.len());

        Ok(Some(BlockInfo {
            slot,
            block_time,
            received_time,
            source: "RPC".to_string(),
            parent_slot,
            block_height,
            transaction_count: tx_count,
            laserstream_created_time: None,
            network_latency_ms: None,
            propagation_latency_ms: block_time.map(|bt| received_time - (bt * 1000)),
        }))
    } else {
        Ok(None)
    }
}

fn log_block_info(block: &BlockInfo, verbose: bool) {
    let propagation_latency = block
        .propagation_latency_ms
        .map(|l| format!("{}ms", l))
        .unwrap_or_else(|| "N/A".to_string());

    let network_latency = block
        .network_latency_ms
        .map(|l| format!("{}ms", l))
        .unwrap_or_else(|| "N/A".to_string());

    let block_time_str = block
        .block_time
        .map(|bt| format!("{}", bt))
        .unwrap_or_else(|| "N/A".to_string());

    let parent_str = block
        .parent_slot
        .map(|p| format!("{}", p))
        .unwrap_or_else(|| "N/A".to_string());

    let height_str = block
        .block_height
        .map(|h| format!("{}", h))
        .unwrap_or_else(|| "N/A".to_string());

    let tx_str = block
        .transaction_count
        .map(|c| format!("{}", c))
        .unwrap_or_else(|| "N/A".to_string());

    if block.source == "LASERSTREAM" {
        println!(
            "{:<10} | {:<8} | {:<10} | {:<8} | Net:{:<6} | Prop:{:<6} | {:<6} | {:<6} | {:<3}",
            block.source,
            block.slot,
            block_time_str,
            block.received_time / 1000,
            network_latency,
            propagation_latency,
            parent_str,
            height_str,
            tx_str
        );
    } else {
        println!(
            "{:<10} | {:<8} | {:<10} | {:<8} | {:<13} | Prop:{:<6} | {:<6} | {:<6} | {:<3}",
            block.source,
            block.slot,
            block_time_str,
            block.received_time / 1000,
            "N/A",
            propagation_latency,
            parent_str,
            height_str,
            tx_str
        );
    }

    if verbose {
        println!("  📋 Block Details:");
        println!("     Slot: {}", block.slot);
        println!("     Block Time: {:?}", block.block_time);
        println!("     Received Time: {}", block.received_time);
        println!("     Parent Slot: {:?}", block.parent_slot);
        println!("     Block Height: {:?}", block.block_height);
        println!("     Transaction Count: {:?}", block.transaction_count);
        println!("     Source: {}", block.source);
        println!();
    }
}

fn print_block_comparison(blocks: &[BlockInfo]) {
    let laserstream_blocks: Vec<_> = blocks
        .iter()
        .filter(|b| b.source == "LASERSTREAM")
        .collect();
    let rpc_blocks: Vec<_> = blocks.iter().filter(|b| b.source == "RPC").collect();

    println!("{}", "=".repeat(60));
    println!("📊 Block Reception Comparison");
    println!("{}", "=".repeat(60));
    println!("Laserstream blocks received: {}", laserstream_blocks.len());
    println!("RPC blocks received: {}", rpc_blocks.len());
    println!();

    if !laserstream_blocks.is_empty() {
        let network_latencies: Vec<i64> = laserstream_blocks
            .iter()
            .filter_map(|b| b.network_latency_ms)
            .collect();

        let propagation_latencies: Vec<i64> = laserstream_blocks
            .iter()
            .filter_map(|b| b.propagation_latency_ms)
            .collect();

        if !network_latencies.is_empty() {
            let avg_network =
                network_latencies.iter().sum::<i64>() as f64 / network_latencies.len() as f64;
            println!(
                "⚡ Laserstream Average Network Latency: {:.1}ms",
                avg_network
            );
        }

        if !propagation_latencies.is_empty() {
            let avg_propagation = propagation_latencies.iter().sum::<i64>() as f64
                / propagation_latencies.len() as f64;
            println!(
                "📡 Laserstream Average Propagation Latency: {:.1}ms",
                avg_propagation
            );
        }
    }

    if !rpc_blocks.is_empty() {
        let rpc_propagation_latencies: Vec<i64> = rpc_blocks
            .iter()
            .filter_map(|b| b.propagation_latency_ms)
            .collect();

        if !rpc_propagation_latencies.is_empty() {
            let avg_rpc = rpc_propagation_latencies.iter().sum::<i64>() as f64
                / rpc_propagation_latencies.len() as f64;
            println!("🌐 RPC Average Propagation Latency: {:.1}ms", avg_rpc);
        }
    }

    // Find common slots for direct comparison
    let mut common_slots = Vec::new();
    for ls_block in &laserstream_blocks {
        if let Some(rpc_block) = rpc_blocks.iter().find(|r| r.slot == ls_block.slot) {
            common_slots.push((ls_block, rpc_block));
        }
    }

    if !common_slots.is_empty() {
        println!();
        println!("🔄 Common Slots (Direct Comparison):");
        println!("Slot      | Laserstream        | RPC      | Prop Diff");
        println!("{}", "-".repeat(55));

        for (ls, rpc) in common_slots.iter().take(10) {
            let ls_network = ls.network_latency_ms.unwrap_or(0);
            let ls_propagation = ls.propagation_latency_ms.unwrap_or(0);
            let rpc_propagation = rpc.propagation_latency_ms.unwrap_or(0);
            let propagation_diff = rpc_propagation - ls_propagation;

            println!(
                "{:<8} | Net:{:<4}ms Prop:{:<4}ms | Prop:{:<4}ms | {:+}ms",
                ls.slot, ls_network, ls_propagation, rpc_propagation, propagation_diff
            );
        }
    }

    println!();
    println!("💡 Key Insights:");
    println!("• Network Latency: Time from Laserstream server to client (gRPC delivery speed)");
    println!("• Propagation Latency: Time from block creation to receipt (validator→client)");
    println!("• RPC uses HTTP polling - only propagation latency measured");
    println!("• Negative propagation diff = Laserstream receives blocks faster");
}

fn load_config(config_path: &str) -> Result<Config> {
    let content = fs::read_to_string(config_path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}
