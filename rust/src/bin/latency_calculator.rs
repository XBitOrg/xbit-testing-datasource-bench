use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use helius_laserstream::{
    grpc::{SubscribeRequest, SubscribeRequestFilterBlocks},
    subscribe, LaserstreamConfig,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use helius_laserstream::grpc::SubscribeRequestFilterBlocksMeta;

#[derive(Parser)]
#[command(name = "latency-calculator")]
#[command(about = "Calculate average latency for RPC or gRPC over specified number of blocks")]
struct Args {
    #[arg(long, value_enum, help = "Method to test: rpc, grpc, or websocket")]
    method: Method,

    #[arg(long, help = "Endpoint URL")]
    endpoint: String,

    #[arg(long, help = "API key (for gRPC)")]
    api_key: Option<String>,

    #[arg(long, help = "Number of blocks to calculate average latency")]
    blocks: u64,

    #[arg(long, help = "Verbose logging")]
    verbose: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Method {
    Rpc,
    Grpc,
    Websocket,
}

#[derive(Debug, Clone)]
struct LatencyMeasurement {
    slot: u64,
    block_time: i64,
    received_time: i64,
    latency_ms: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("🚀 Latency Calculator");
    println!("Method: {:?}", args.method);
    println!("Endpoint: {}", args.endpoint);
    println!("Target blocks: {}", args.blocks);
    println!();

    let measurements = match args.method {
        Method::Rpc => measure_rpc_latency(&args).await?,
        Method::Grpc => measure_grpc_latency(&args).await?,
        Method::Websocket => measure_websocket_latency(&args).await?,
    };

    print_results(&measurements, &args);

    Ok(())
}

async fn measure_rpc_latency(args: &Args) -> Result<Vec<LatencyMeasurement>> {
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
    let mut measurements = Vec::new();
    let mut processed_blocks = 0u64;

    println!("📡 Starting RPC latency measurement...");
    println!("Slot       | Block Time    | Received Time | Latency   | Status");
    println!("{}", "-".repeat(70));

    let mut last_slot = get_latest_slot(&client, &args.endpoint).await?;

    while processed_blocks < args.blocks {
        match get_latest_slot(&client, &args.endpoint).await {
            Ok(current_slot) => {
                if current_slot > last_slot {
                    // Process the new slot
                    match get_block_time(&client, &args.endpoint, current_slot).await {
                        Ok(Some(block_time)) => {
                            let received_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)?
                                .as_millis() as i64;

                            let latency_ms = received_time - (block_time * 1000);

                            // Filter out unrealistic latencies
                            if latency_ms > 0 && latency_ms < 10000 {
                                let measurement = LatencyMeasurement {
                                    slot: current_slot,
                                    block_time,
                                    received_time,
                                    latency_ms,
                                };

                                let status = get_latency_status(latency_ms);
                                
                                println!(
                                    "{:<10} | {:<12} | {:<12} | {:<9}ms | {}",
                                    current_slot,
                                    block_time,
                                    received_time / 1000,
                                    latency_ms,
                                    status
                                );

                                measurements.push(measurement);
                                processed_blocks += 1;

                                if args.verbose {
                                    println!("Progress: {}/{} blocks processed", processed_blocks, args.blocks);
                                }
                            }
                        }
                        Ok(None) => {
                            if args.verbose {
                                println!("Block time not available for slot {}", current_slot);
                            }
                        }
                        Err(e) => {
                            if args.verbose {
                                println!("Error getting block time for slot {}: {}", current_slot, e);
                            }
                        }
                    }
                    last_slot = current_slot;
                }
            }
            Err(e) => {
                if args.verbose {
                    eprintln!("Error getting latest slot: {}", e);
                }
            }
        }

        time::sleep(Duration::from_millis(500)).await;
    }

    Ok(measurements)
}

async fn measure_grpc_latency(args: &Args) -> Result<Vec<LatencyMeasurement>> {
    let api_key = args
        .api_key
        .clone()
        .or_else(|| std::env::var("HELIUS_API_KEY").ok())
        .ok_or_else(|| anyhow::anyhow!("API key required for gRPC method"))?;

    let config = LaserstreamConfig {
        api_key,
        endpoint: args.endpoint.parse()?,
        ..Default::default()
    };

    let mut request = SubscribeRequest::default();
    
    request.blocks_meta.insert(
        "all".to_string(),
        SubscribeRequestFilterBlocksMeta::default(),
    );

    let (stream, _handle) = subscribe(config, request);
    futures::pin_mut!(stream);

    let mut measurements = Vec::new();
    let mut processed_blocks = 0u64;

    println!("📡 Starting gRPC latency measurement...");
    println!("Slot       | Block Time    | Received Time | Latency   | Status");
    println!("{}", "-".repeat(70));

    while processed_blocks < args.blocks {
        if let Some(result) = stream.next().await {
            match result {
                Ok(update) => {
                    let received_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)?
                        .as_millis() as i64;

                    if let Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::BlockMeta(block)) = update.update_oneof {
                        let slot = block.slot;
                        
                        if let Some(bt) = block.block_time {
                            let block_time = bt.timestamp;
                            let latency_ms = received_time - (block_time * 1000);

                            // Filter out unrealistic latencies
                            if latency_ms > 0 && latency_ms < 10000 {
                                let measurement = LatencyMeasurement {
                                    slot,
                                    block_time,
                                    received_time,
                                    latency_ms,
                                };

                                let status = get_latency_status(latency_ms);
                                
                                println!(
                                    "{:<10} | {:<12} | {:<12} | {:<9}ms | {}",
                                    slot,
                                    block_time,
                                    received_time / 1000,
                                    latency_ms,
                                    status
                                );

                                measurements.push(measurement);
                                processed_blocks += 1;

                                if args.verbose {
                                    println!("Progress: {}/{} blocks processed", processed_blocks, args.blocks);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    if args.verbose {
                        eprintln!("gRPC stream error: {}", e);
                    }
                }
            }
        }
    }

    Ok(measurements)
}

async fn measure_websocket_latency(args: &Args) -> Result<Vec<LatencyMeasurement>> {
    let mut measurements = Vec::new();
    let mut processed_blocks = 0u64;

    println!("📡 Starting WebSocket latency measurement...");
    println!("Slot       | Block Time    | Received Time | Latency   | Status");
    println!("{}", "-".repeat(70));

    // Convert HTTP(S) URL to WebSocket URL
    let ws_url = if args.endpoint.starts_with("https://") {
        args.endpoint.replace("https://", "wss://")
    } else if args.endpoint.starts_with("http://") {
        args.endpoint.replace("http://", "ws://")
    } else if !args.endpoint.starts_with("ws://") && !args.endpoint.starts_with("wss://") {
        format!("wss://{}", args.endpoint)
    } else {
        args.endpoint.clone()
    };

    let (ws_stream, _) = connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Subscribe to block notifications
    let subscription = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "blockSubscribe",
        "params": [
            "all",
            {
                "commitment": "processed",
                "encoding": "json",
                "transactionDetails": "none",
                "rewards": false
            }
        ]
    });

    write.send(Message::Text(subscription.to_string())).await?;

    // Handle subscription confirmation and block notifications
    let mut subscription_confirmed = false;
    while processed_blocks < args.blocks {
        let timeout = tokio::time::sleep(Duration::from_secs(30));
        tokio::pin!(timeout);
        
        tokio::select! {
            msg_result = read.next() => {
                if let Some(msg) = msg_result {
                    match msg? {
                Message::Text(text) => {
                    let received_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)?
                        .as_millis() as i64;

                    if let Ok(json_msg) = serde_json::from_str::<Value>(&text) {
                        if args.verbose {
                            println!("Received WebSocket message: {}", serde_json::to_string_pretty(&json_msg).unwrap_or_else(|_| "Invalid JSON".to_string()));
                        }
                        // Check if this is a block notification
                        if let Some(params) = json_msg.get("params") {
                            if let Some(result) = params.get("result") {
                                if let Some(value) = result.get("value") {
                                    if let Some(block) = value.get("block") {
                                        let slot = value.get("slot")
                                            .and_then(|s| s.as_u64())
                                            .unwrap_or_else(|| {
                                                block.get("parentSlot")
                                                    .and_then(|s| s.as_u64())
                                                    .unwrap_or_default() + 1
                                            });
                                        
                                        if let Some(block_time) = block.get("blockTime").and_then(|bt| bt.as_i64()) {
                                            let latency_ms = received_time - (block_time * 1000);

                                            // Filter out unrealistic latencies
                                            if latency_ms > 0 && latency_ms < 10000 {
                                                let measurement = LatencyMeasurement {
                                                    slot,
                                                    block_time,
                                                    received_time,
                                                    latency_ms,
                                                };

                                                let status = get_latency_status(latency_ms);
                                                
                                                println!(
                                                    "{:<10} | {:<12} | {:<12} | {:<9}ms | {}",
                                                    slot,
                                                    block_time,
                                                    received_time / 1000,
                                                    latency_ms,
                                                    status
                                                );

                                                measurements.push(measurement);
                                                processed_blocks += 1;

                                                if args.verbose {
                                                    println!("Progress: {}/{} blocks processed", processed_blocks, args.blocks);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else if json_msg.get("result").is_some() {
                            // Subscription confirmation
                            subscription_confirmed = true;
                            if args.verbose {
                                println!("WebSocket subscription confirmed");
                            }
                        }
                    }
                }
                Message::Close(_) => {
                    println!("WebSocket connection closed");
                    break;
                }
                _ => {}
                    }
                } else {
                    println!("WebSocket connection ended");
                    break;
                }
            }
            _ = &mut timeout => {
                if !subscription_confirmed {
                    return Err(anyhow::anyhow!("WebSocket subscription timeout"));
                } else {
                    println!("No new blocks received in 30 seconds, continuing...");
                }
            }
        }
    }

    Ok(measurements)
}

fn print_results(measurements: &[LatencyMeasurement], args: &Args) {
    if measurements.is_empty() {
        println!("❌ No measurements collected");
        return;
    }

    let latencies: Vec<i64> = measurements.iter().map(|m| m.latency_ms).collect();
    let total: i64 = latencies.iter().sum();
    let avg = total as f64 / latencies.len() as f64;
    
    let mut sorted_latencies = latencies.clone();
    sorted_latencies.sort();
    
    let min = *sorted_latencies.first().unwrap();
    let max = *sorted_latencies.last().unwrap();
    let median = sorted_latencies[sorted_latencies.len() / 2];
    let p95_idx = (sorted_latencies.len() as f64 * 0.95) as usize;
    let p95 = sorted_latencies[p95_idx.min(sorted_latencies.len() - 1)];
    let p99_idx = (sorted_latencies.len() as f64 * 0.99) as usize;
    let p99 = sorted_latencies[p99_idx.min(sorted_latencies.len() - 1)];

    // Count performance categories
    let excellent = latencies.iter().filter(|&&l| l < 500).count();
    let good = latencies.iter().filter(|&&l| l >= 500 && l < 1000).count();
    let fair = latencies.iter().filter(|&&l| l >= 1000 && l < 2000).count();
    let slow = latencies.iter().filter(|&&l| l >= 2000).count();

    println!();
    println!("📊 Latency Results Summary");
    println!("{}", "=".repeat(50));
    println!("Method:             {:?}", args.method);
    println!("Endpoint:           {}", args.endpoint);
    println!("Blocks processed:   {}", measurements.len());
    println!("Average latency:    {:.1}ms", avg);
    println!("Min latency:        {}ms", min);
    println!("Max latency:        {}ms", max);
    println!("Median latency:     {}ms", median);
    println!("95th percentile:    {}ms", p95);
    println!("99th percentile:    {}ms", p99);
    println!();

    println!("⚡ Performance Distribution:");
    println!("🟢 Excellent (<500ms):   {}/{} ({:.1}%)", 
        excellent, measurements.len(), (excellent as f64 / measurements.len() as f64) * 100.0);
    println!("🟡 Good (500-1000ms):    {}/{} ({:.1}%)", 
        good, measurements.len(), (good as f64 / measurements.len() as f64) * 100.0);
    println!("🟠 Fair (1000-2000ms):   {}/{} ({:.1}%)", 
        fair, measurements.len(), (fair as f64 / measurements.len() as f64) * 100.0);
    println!("🔴 Slow (>2000ms):       {}/{} ({:.1}%)", 
        slow, measurements.len(), (slow as f64 / measurements.len() as f64) * 100.0);
    println!();

    println!("🎯 Overall Assessment:");
    if avg < 500.0 {
        println!("✅ EXCELLENT - Very fast latency!");
    } else if avg < 1000.0 {
        println!("🟡 GOOD - Acceptable latency for most use cases");
    } else if avg < 2000.0 {
        println!("🟠 FAIR - Moderate latency, consider optimization");
    } else {
        println!("🔴 SLOW - High latency, investigate network/provider issues");
    }
}

fn get_latency_status(latency_ms: i64) -> &'static str {
    if latency_ms < 500 {
        "🟢 EXCELLENT"
    } else if latency_ms < 1000 {
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
        return Ok(None);
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