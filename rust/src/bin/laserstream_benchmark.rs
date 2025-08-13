use clap::Parser;
use futures::StreamExt;
use helius_laserstream::{
    grpc::{SubscribeRequest, SubscribeRequestFilterBlocks},
    subscribe, LaserstreamConfig,
};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "laserstream-benchmark")]
#[command(about = "Benchmark Helius Laserstream block propagation latency")]
struct Args {
    #[arg(long, help = "Helius API key")]
    api_key: Option<String>,

    #[arg(long, default_value = "https://laserstream-mainnet-tyo.helius-rpc.com", help = "Helius Laserstream endpoint")]
    endpoint: String,

    #[arg(long, default_value = "5", help = "Test duration in minutes")]
    duration: u64,

    #[arg(long, help = "Output results as JSON")]
    json: bool,
}

#[derive(Debug, Clone)]
struct BlockLatencyData {
    slot: u64,
    block_time: i64,
    received_time: i64,
    propagation_latency_ms: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let api_key = args.api_key.clone()
        .or_else(|| std::env::var("HELIUS_API_KEY").ok())
        .unwrap_or_else(|| "9de07723-0030-4ee0-b175-6722231d5d97".to_string());

    println!("🚀 Helius Laserstream Block Propagation Benchmark");
    println!("Testing claim: 'Fastest block propagation'");
    println!("Duration: {} minutes", args.duration);
    println!("Endpoint: {}", args.endpoint);
    println!();

    let config = LaserstreamConfig {
        api_key,
        endpoint: args.endpoint.parse()?,
        ..Default::default()
    };

    // Subscribe to all blocks for comprehensive latency testing
    let mut block_filters = HashMap::new();
    block_filters.insert(
        "all_blocks".to_string(),
        SubscribeRequestFilterBlocks {
            account_include: vec![], // All blocks
            include_transactions: Some(false), // Don't need tx data for latency test
            include_accounts: Some(false),
            include_entries: Some(false),
        },
    );

    let request = SubscribeRequest {
        blocks: block_filters,
        ..Default::default()
    };

    println!("📡 Connecting to Helius Laserstream...");
    let (stream, _handle) = subscribe(config, request);
    futures::pin_mut!(stream);

    let mut latencies = Vec::new();
    let start_time = SystemTime::now();
    let duration = std::time::Duration::from_secs(args.duration * 60);

    let mut block_count = 0;
    let mut total_latency = 0i64;
    let mut min_latency = i64::MAX;
    let mut max_latency = 0i64;

    println!("⏱️  Starting latency measurement...");
    println!();

    while start_time.elapsed()? < duration {
        if let Some(result) = stream.next().await {
            match result {
                Ok(update) => {
                    let received_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)?
                        .as_millis() as i64;

                    if let Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Block(block)) = update.update_oneof {
                        let slot = block.slot;
                        let block_time = block.block_time.map(|bt| bt.timestamp).unwrap_or(received_time / 1000);
                        let propagation_latency_ms = received_time - (block_time * 1000);

                        // Filter out unrealistic latencies (negative or too large)
                        if propagation_latency_ms >= 0 && propagation_latency_ms < 60000 {
                            block_count += 1;
                            total_latency += propagation_latency_ms;
                            min_latency = min_latency.min(propagation_latency_ms);
                            max_latency = max_latency.max(propagation_latency_ms);

                            let latency_data = BlockLatencyData {
                                slot,
                                block_time,
                                received_time,
                                propagation_latency_ms,
                            };

                            latencies.push(latency_data.clone());

                            // Real-time feedback
                            print!("⚡ Slot {}: {}ms", slot, propagation_latency_ms);
                            
                            if propagation_latency_ms < 900 {
                                println!(" 🟢 EXCELLENT");
                            } else if propagation_latency_ms < 1200 {
                                println!(" 🟡 GOOD");
                            } else if propagation_latency_ms < 2000 {
                                println!(" 🟠 FAIR");
                            } else {
                                println!(" 🔴 SLOW");
                            }

                            // Show running average every 10 blocks
                            if block_count % 10 == 0 {
                                let avg = total_latency / block_count as i64;
                                println!("📊 Running Average: {}ms (after {} blocks)", avg, block_count);
                                println!();
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Laserstream error: {}", e);
                }
            }
        }
    }

    // Calculate final statistics
    if !latencies.is_empty() {
        print_benchmark_results(&latencies, &args);
    } else {
        println!("❌ No blocks received during test period");
    }

    Ok(())
}

fn print_benchmark_results(latencies: &[BlockLatencyData], args: &Args) {
    let mut times: Vec<i64> = latencies.iter().map(|l| l.propagation_latency_ms).collect();
    times.sort();

    let count = times.len();
    let avg = times.iter().sum::<i64>() as f64 / count as f64;
    let min = times[0];
    let max = times[count - 1];
    let p50 = times[count / 2];
    let p90 = times[(count as f64 * 0.9) as usize];
    let p95 = times[(count as f64 * 0.95) as usize];
    let p99 = times[(count as f64 * 0.99) as usize];

    // Realistic speed categories
    let sub_900ms = times.iter().filter(|&&t| t < 900).count();
    let sub_1200ms = times.iter().filter(|&&t| t < 1200).count();
    let sub_2000ms = times.iter().filter(|&&t| t < 2000).count();

    if args.json {
        let results = serde_json::json!({
            "provider": "Helius Laserstream",
            "test_duration_minutes": args.duration,
            "blocks_received": count,
            "latency_stats": {
                "avg_ms": avg,
                "min_ms": min,
                "max_ms": max,
                "p50_ms": p50,
                "p90_ms": p90,
                "p95_ms": p95,
                "p99_ms": p99
            },
            "speed_distribution": {
                "sub_900ms": sub_900ms,
                "sub_1200ms": sub_1200ms,
                "sub_2000ms": sub_2000ms,
                "sub_900ms_percent": (sub_900ms as f64 / count as f64) * 100.0,
                "sub_1200ms_percent": (sub_1200ms as f64 / count as f64) * 100.0,
                "sub_2000ms_percent": (sub_2000ms as f64 / count as f64) * 100.0
            },
            "verdict": get_performance_verdict(avg)
        });
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!();
        println!("🏁 Helius Laserstream Benchmark Results");
        println!("{}", "=".repeat(50));
        println!("Blocks tested: {}", count);
        println!("Average latency: {:.1}ms", avg);
        println!("Min latency: {}ms", min);
        println!("Max latency: {}ms", max);
        println!("Median (P50): {}ms", p50);
        println!("P90: {}ms", p90);
        println!("P95: {}ms", p95);
        println!("P99: {}ms", p99);
        println!();
        
        println!("⚡ Realistic Speed Distribution:");
        println!("Sub-900ms (Excellent): {}/{} ({:.1}%)", sub_900ms, count, (sub_900ms as f64 / count as f64) * 100.0);
        println!("Sub-1200ms (Good): {}/{} ({:.1}%)", sub_1200ms, count, (sub_1200ms as f64 / count as f64) * 100.0);
        println!("Sub-2000ms (Fair): {}/{} ({:.1}%)", sub_2000ms, count, (sub_2000ms as f64 / count as f64) * 100.0);
        println!();
        
        println!("🎯 Performance Verdict:");
        match get_performance_verdict(avg) {
            "excellent" => println!("✅ EXCELLENT - Sub-900ms latency! Outstanding real-world performance"),
            "very_good" => println!("✅ VERY GOOD - Sub-1200ms latency, great for most applications"),
            "good" => println!("🟡 GOOD - Sub-2000ms latency, acceptable for general use"),
            "fair" => println!("🟠 FAIR - 2-3s latency, consider region optimization"),
            _ => println!("🔴 SLOW - >3s latency, investigate network/provider issues"),
        }
        
        println!();
        println!("📈 Compared to typical RPC providers:");
        println!("• Regular HTTP RPC: 3-5 seconds");
        println!("• Premium WebSocket: 500-2000ms");
        println!("• Laserstream: {:.0}ms average", avg);
        
        if avg < 200.0 {
            println!("🏆 CLAIM VERIFIED: Laserstream IS significantly faster!");
        } else if avg < 500.0 {
            println!("✅ CLAIM SUPPORTED: Much faster than regular RPCs");
        } else {
            println!("⚠️  CLAIM QUESTIONABLE: Similar to other premium providers");
        }
    }
}

fn get_performance_verdict(avg_latency: f64) -> &'static str {
    if avg_latency < 900.0 {
        "excellent"
    } else if avg_latency < 1200.0 {
        "very_good"
    } else if avg_latency < 2000.0 {
        "good"
    } else if avg_latency < 3000.0 {
        "fair"
    } else {
        "poor"
    }
}