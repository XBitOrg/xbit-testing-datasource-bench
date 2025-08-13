use clap::Parser;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "rpc-latency-test")]
#[command(about = "Measure RPC latency using processed slot detection")]
struct Args {
    #[arg(long, default_value = "../shared/config.json", help = "Config file path")]
    config: String,

    #[arg(long, default_value = "2", help = "Test duration in minutes")]
    duration: u64,

    #[arg(long, help = "Verbose logging")]
    verbose: bool,

    #[arg(long, help = "RPC provider to test (helius, solana, etc)")]
    provider: Option<String>,
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
struct SlotLatency {
    slot: u64,
    block_time: i64,
    detected_time: i64,
    latency_ms: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("⚡ RPC Slot Latency Test (Processed Commitment)");
    println!("Duration: {} minutes", args.duration);
    println!();

    let config = load_config(&args.config)?;
    
    // Select RPC based on provider preference
    let rpc = if let Some(provider) = &args.provider {
        config.rpcs.values()
            .find(|r| r.provider.to_lowercase().contains(&provider.to_lowercase()) && r.status == "active")
            .ok_or_else(|| anyhow::anyhow!("No active RPC found for provider: {}", provider))?
    } else {
        config.rpcs.values()
            .find(|r| r.provider == "Helius" && r.status == "active")
            .or_else(|| config.rpcs.values().find(|r| r.status == "active"))
            .ok_or_else(|| anyhow::anyhow!("No active RPCs found"))?
    };

    println!("🌐 RPC Provider: {} ({})", rpc.name, rpc.provider);
    println!("🔗 RPC URL: {}", rpc.url);
    println!();

    let latencies = monitor_slot_latency(rpc.clone(), args.duration, args.verbose).await?;

    print_latency_results(&latencies);

    Ok(())
}

async fn monitor_slot_latency(
    rpc: RPCConfig,
    duration_minutes: u64,
    verbose: bool
) -> Result<Vec<SlotLatency>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let mut latencies = Vec::new();
    let start_time = SystemTime::now();
    let duration = Duration::from_secs(duration_minutes * 60);
    
    println!("🚀 Starting slot latency monitoring...");
    println!("📊 Checking new slots every 400ms");
    println!();
    
    if !verbose {
        println!("Slot      | Block Time   | Detected    | Latency | Status");
        println!("{}", "-".repeat(55));
    }

    let mut last_slot = get_latest_slot(&client, &rpc.url).await?;

    while start_time.elapsed()? < duration {
        match get_latest_slot(&client, &rpc.url).await {
            Ok(current_slot) => {
                if current_slot > last_slot {
                    // New slot detected! Now check if we can get its block time
                    let detected_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)?
                        .as_millis() as i64;

                    match get_block_time(&client, &rpc.url, current_slot).await {
                        Ok(Some(block_time)) => {
                            let latency_ms = detected_time - (block_time * 1000);
                            
                            let slot_latency = SlotLatency {
                                slot: current_slot,
                                block_time,
                                detected_time,
                                latency_ms,
                            };
                            
                            log_slot_latency(&slot_latency, verbose);
                            latencies.push(slot_latency);
                        }
                        Ok(None) => {
                            if verbose {
                                println!("Slot {} | Block time not available yet", current_slot);
                            }
                        }
                        Err(e) => {
                            if verbose {
                                println!("Slot {} | Error getting block time: {}", current_slot, e);
                            }
                        }
                    }
                    last_slot = current_slot;
                }
            }
            Err(e) => {
                if verbose {
                    eprintln!("❌ Error getting latest slot: {}", e);
                }
            }
        }
        
        time::sleep(Duration::from_millis(400)).await;
    }

    Ok(latencies)
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
        // getBlockTime might fail for very recent slots, that's normal
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

fn log_slot_latency(latency: &SlotLatency, verbose: bool) {
    let status = if latency.latency_ms < 300 {
        "🟢 FAST"
    } else if latency.latency_ms < 1000 {
        "🟡 GOOD"
    } else if latency.latency_ms < 3000 {
        "🟠 SLOW"
    } else {
        "🔴 VERY SLOW"
    };

    if verbose {
        println!("🎯 Slot {} Latency Analysis:", latency.slot);
        println!("   Block Created: {} (Unix timestamp)", latency.block_time);
        println!("   RPC Detected:  {} (Unix timestamp ms)", latency.detected_time);
        println!("   Latency:       {}ms {}", latency.latency_ms, status);
        println!();
    } else {
        println!("{:<9} | {:<12} | {:<11} | {:<7}ms | {}", 
            latency.slot,
            latency.block_time,
            latency.detected_time / 1000,
            latency.latency_ms,
            status
        );
    }
}

fn print_latency_results(latencies: &[SlotLatency]) {
    if latencies.is_empty() {
        println!("❌ No slot latency measurements collected");
        return;
    }

    let latency_values: Vec<i64> = latencies.iter().map(|l| l.latency_ms).collect();
    let count = latency_values.len();
    let avg = latency_values.iter().sum::<i64>() as f64 / count as f64;
    
    let mut sorted = latency_values.clone();
    sorted.sort();
    
    let min = sorted[0];
    let max = sorted[count - 1];
    let median = sorted[count / 2];
    let p90 = sorted[(count as f64 * 0.9) as usize];
    let p95 = sorted[(count as f64 * 0.95) as usize];

    println!();
    println!("📊 RPC Slot Detection Latency Results");
    println!("{}", "=".repeat(50));
    println!("Slots measured:     {}", count);
    println!("Average latency:    {:.1}ms", avg);
    println!("Min latency:        {}ms", min);
    println!("Max latency:        {}ms", max);
    println!("Median latency:     {}ms", median);
    println!("90th percentile:    {}ms", p90);
    println!("95th percentile:    {}ms", p95);

    // Performance categories
    let fast_count = latency_values.iter().filter(|&&l| l < 300).count();
    let good_count = latency_values.iter().filter(|&&l| l < 1000).count();
    let slow_count = latency_values.iter().filter(|&&l| l < 3000).count();

    println!();
    println!("⚡ Performance Distribution:");
    println!("🟢 Fast (<300ms):     {}/{} ({:.1}%)", 
        fast_count, count, (fast_count as f64 / count as f64) * 100.0);
    println!("🟡 Good (<1000ms):    {}/{} ({:.1}%)", 
        good_count, count, (good_count as f64 / count as f64) * 100.0);
    println!("🟠 Slow (<3000ms):    {}/{} ({:.1}%)", 
        slow_count, count, (slow_count as f64 / count as f64) * 100.0);

    println!();
    println!("🎯 Overall Performance:");
    if avg < 300.0 {
        println!("✅ EXCELLENT - Sub-300ms average latency!");
        println!("💡 Perfect for real-time trading and indexing");
    } else if avg < 1000.0 {
        println!("🟡 GOOD - Sub-1000ms average latency");
        println!("💡 Suitable for most real-time applications");
    } else if avg < 3000.0 {
        println!("🟠 FAIR - Sub-3000ms average latency");
        println!("💡 Acceptable for general applications");
    } else {
        println!("🔴 SLOW - >3000ms average latency");
        println!("💡 Consider faster RPC providers");
    }

    println!();
    println!("📋 Methodology:");
    println!("• Uses getSlot() with processed commitment for slot detection");
    println!("• Uses getBlockTime() to get block creation timestamp");  
    println!("• Latency = slot_detection_time - block_creation_time");
    println!("• Polling interval: 400ms for real-time detection");
}

fn load_config(config_path: &str) -> Result<Config> {
    let content = fs::read_to_string(config_path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}