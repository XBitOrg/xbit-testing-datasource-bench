use clap::Parser;
use futures::StreamExt;
use helius_laserstream::{
    grpc::{SubscribeRequest, SubscribeRequestFilterBlocks},
    subscribe, LaserstreamConfig,
};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "helius-proper-latency")]
#[command(about = "Measure Helius Laserstream latency using official methodology")]
struct Args {
    #[arg(long, help = "Helius API key")]
    api_key: Option<String>,

    #[arg(long, default_value = "tyo", help = "Helius region (tyo, ewr, pitt, slc, ams, fra, sgp)")]
    region: String,

    #[arg(long, default_value = "3", help = "Test duration in minutes")]
    duration: u64,

    #[arg(long, help = "Verbose logging")]
    verbose: bool,

    #[arg(long, default_value = "2", help = "Method: 1=parallel streams, 2=created_at comparison")]
    method: u8,
}

#[derive(Debug, Clone)]
struct LatencyMeasurement {
    slot: u64,
    method1_latency_ms: Option<i64>, // Parallel stream comparison
    method2_latency_ms: Option<i64>, // created_at vs local time
    received_time: i64,
    created_at_time: Option<i64>,
    source_stream: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let api_key = args.api_key.clone()
        .or_else(|| std::env::var("HELIUS_API_KEY").ok())
        .unwrap_or_else(|| "9de07723-0030-4ee0-b175-6722231d5d97".to_string());

    let endpoint = format!("https://laserstream-mainnet-{}.helius-rpc.com", args.region);

    println!("🚀 Helius Laserstream Proper Latency Measurement");
    println!("Following official Helius documentation methodology");
    println!("Region: {} ({})", args.region.to_uppercase(), endpoint);
    println!("Method: {}", match args.method {
        1 => "Parallel gRPC Streams Comparison (Most Reliable)",
        2 => "Local Timestamp vs created_at Comparison (Moderate Reliability)",
        _ => "Invalid method"
    });
    println!("Duration: {} minutes", args.duration);
    println!();

    match args.method {
        1 => run_parallel_streams_method(&api_key, &endpoint, args.duration, args.verbose).await?,
        2 => run_created_at_method(&api_key, &endpoint, args.duration, args.verbose).await?,
        _ => {
            eprintln!("❌ Invalid method. Use 1 for parallel streams or 2 for created_at comparison");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run_parallel_streams_method(
    api_key: &str,
    endpoint: &str,
    duration_minutes: u64,
    verbose: bool
) -> Result<()> {
    println!("📡 Method 1: Parallel gRPC Streams Comparison");
    println!("🔄 Starting two identical streams to compare reception times...");
    println!();

    let config = LaserstreamConfig {
        api_key: api_key.to_string(),
        endpoint: endpoint.parse()?,
        ..Default::default()
    };

    let mut block_filters = HashMap::new();
    block_filters.insert(
        "blocks".to_string(),
        SubscribeRequestFilterBlocks {
            account_include: vec![],
            include_transactions: Some(false), // Focus on latency, not data
            include_accounts: Some(false),
            include_entries: Some(false),
        },
    );

    let request = SubscribeRequest {
        blocks: block_filters,
        ..Default::default()
    };

    // Start two parallel streams
    let (stream1, _handle1) = subscribe(config.clone(), request.clone());
    let (stream2, _handle2) = subscribe(config, request);

    futures::pin_mut!(stream1, stream2);

    let mut measurements = Vec::new();
    let mut slot_timings: HashMap<u64, (Option<i64>, Option<i64>)> = HashMap::new();

    let start_time = SystemTime::now();
    let duration = std::time::Duration::from_secs(duration_minutes * 60);

    println!("Slot      | Stream1   | Stream2   | Latency Diff | Winner");
    println!("{}", "-".repeat(65));

    while start_time.elapsed()? < duration {
        tokio::select! {
            Some(result1) = stream1.next() => {
                if let Ok(update) = result1 {
                    let received_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
                    
                    if let Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Block(block)) = update.update_oneof {
                        let slot = block.slot;
                        let entry = slot_timings.entry(slot).or_insert((None, None));
                        entry.0 = Some(received_time);
                        
                        // Check if we have both timings
                        if let (Some(time1), Some(time2)) = (entry.0, entry.1) {
                            let diff = time2 - time1; // Positive = Stream1 faster, Negative = Stream2 faster
                            let winner = if diff > 0 { "Stream1" } else if diff < 0 { "Stream2" } else { "Tie" };
                            
                            println!("{:<9} | {:<9} | {:<9} | {:<12}ms | {}", 
                                slot, time1 % 100000, time2 % 100000, diff, winner);
                                
                            measurements.push(LatencyMeasurement {
                                slot,
                                method1_latency_ms: Some(diff.abs()),
                                method2_latency_ms: None,
                                received_time: std::cmp::min(time1, time2),
                                created_at_time: None,
                                source_stream: winner.to_string(),
                            });
                        }
                    }
                }
            }
            
            Some(result2) = stream2.next() => {
                if let Ok(update) = result2 {
                    let received_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
                    
                    if let Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Block(block)) = update.update_oneof {
                        let slot = block.slot;
                        let entry = slot_timings.entry(slot).or_insert((None, None));
                        entry.1 = Some(received_time);
                        
                        // Check if we have both timings (same logic as above)
                        if let (Some(time1), Some(time2)) = (entry.0, entry.1) {
                            let diff = time2 - time1;
                            let winner = if diff > 0 { "Stream1" } else if diff < 0 { "Stream2" } else { "Tie" };
                            
                            println!("{:<9} | {:<9} | {:<9} | {:<12}ms | {}", 
                                slot, time1 % 100000, time2 % 100000, diff, winner);
                                
                            measurements.push(LatencyMeasurement {
                                slot,
                                method1_latency_ms: Some(diff.abs()),
                                method2_latency_ms: None,
                                received_time: std::cmp::min(time1, time2),
                                created_at_time: None,
                                source_stream: winner.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    print_method1_results(&measurements);
    Ok(())
}

async fn run_created_at_method(
    api_key: &str,
    endpoint: &str,
    duration_minutes: u64,
    verbose: bool
) -> Result<()> {
    println!("📡 Method 2: Local Timestamp vs created_at Comparison");
    println!("⏱️  Measuring LaserStream internal processing time...");
    println!();

    let config = LaserstreamConfig {
        api_key: api_key.to_string(),
        endpoint: endpoint.parse()?,
        ..Default::default()
    };

    let mut block_filters = HashMap::new();
    block_filters.insert(
        "blocks".to_string(),
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

    let mut measurements = Vec::new();
    let start_time = SystemTime::now();
    let duration = std::time::Duration::from_secs(duration_minutes * 60);

    println!("Slot      | Created_at   | Received     | Latency   | Status");
    println!("{}", "-".repeat(70));

    while start_time.elapsed()? < duration {
        if let Some(result) = stream.next().await {
            match result {
                Ok(update) => {
                    let received_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
                    
                    let created_at_time = update.created_at
                        .map(|ts| (ts.seconds * 1000) + (ts.nanos as i64 / 1_000_000));

                    if let Some(helius_laserstream::grpc::subscribe_update::UpdateOneof::Block(block)) = update.update_oneof {
                        let slot = block.slot;
                        
                        let latency = created_at_time
                            .map(|created| (received_time - created).abs()); // Use absolute value for now

                        let status = match latency {
                            Some(l) if l < 50 => "🟢 EXCELLENT",
                            Some(l) if l < 100 => "🟡 GOOD",
                            Some(l) if l < 200 => "🟠 FAIR",
                            Some(l) => "🔴 SLOW",
                            None => "❓ NO_TIMESTAMP",
                        };

                        // Always show debug info to understand the issue
                        println!("DEBUG - Slot {}: created_at={:?}, received={}, diff={:?}ms", 
                            slot, created_at_time, received_time, latency);
                        
                        // Show the actual timestamp components
                        if let Some(created) = created_at_time {
                            let created_seconds = created / 1000;
                            let received_seconds = received_time / 1000;
                            println!("       Seconds: created={}, received={}, diff={}s", 
                                created_seconds, received_seconds, received_seconds - created_seconds);
                        }
                        
                        println!("{:<9} | {:<12} | {:<12} | {:<9}ms | {}", 
                            slot, 
                            created_at_time.unwrap_or(0),
                            received_time,
                            latency.unwrap_or(0),
                            status
                        );

                        measurements.push(LatencyMeasurement {
                            slot,
                            method1_latency_ms: None,
                            method2_latency_ms: latency,
                            received_time,
                            created_at_time,
                            source_stream: "single".to_string(),
                        });
                    }
                }
                Err(e) => {
                    if verbose {
                        eprintln!("❌ Stream error: {}", e);
                    }
                }
            }
        }
    }

    print_method2_results(&measurements);
    Ok(())
}

fn print_method1_results(measurements: &[LatencyMeasurement]) {
    if measurements.is_empty() {
        println!("❌ No measurements collected");
        return;
    }

    let latencies: Vec<i64> = measurements
        .iter()
        .filter_map(|m| m.method1_latency_ms)
        .collect();

    if latencies.is_empty() {
        println!("❌ No valid latency measurements");
        return;
    }

    let avg = latencies.iter().sum::<i64>() as f64 / latencies.len() as f64;
    let mut sorted = latencies.clone();
    sorted.sort();

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let median = sorted[sorted.len() / 2];

    println!();
    println!("📊 Method 1 Results: Parallel Stream Comparison");
    println!("{}", "=".repeat(50));
    println!("Measurements:       {}", measurements.len());
    println!("Average difference: {:.1}ms", avg);
    println!("Min difference:     {}ms", min);
    println!("Max difference:     {}ms", max);
    println!("Median difference:  {}ms", median);
    
    println!();
    println!("💡 Interpretation:");
    println!("• Lower differences = More consistent LaserStream performance");
    println!("• This measures LaserStream's internal consistency, not absolute latency");
    println!("• Differences <50ms indicate excellent service stability");
}

fn print_method2_results(measurements: &[LatencyMeasurement]) {
    if measurements.is_empty() {
        println!("❌ No measurements collected");
        return;
    }

    let latencies: Vec<i64> = measurements
        .iter()
        .filter_map(|m| m.method2_latency_ms)
        .collect();

    if latencies.is_empty() {
        println!("❌ No valid latency measurements");
        return;
    }

    let avg = latencies.iter().sum::<i64>() as f64 / latencies.len() as f64;
    let mut sorted = latencies.clone();
    sorted.sort();

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let median = sorted[sorted.len() / 2];

    // Performance categories
    let excellent = latencies.iter().filter(|&&l| l < 50).count();
    let good = latencies.iter().filter(|&&l| l < 100).count();
    let fair = latencies.iter().filter(|&&l| l < 200).count();

    println!();
    println!("📊 Method 2 Results: created_at vs Local Time");
    println!("{}", "=".repeat(50));
    println!("Measurements:       {}", measurements.len());
    println!("Average latency:    {:.1}ms", avg);
    println!("Min latency:        {}ms", min);
    println!("Max latency:        {}ms", max);
    println!("Median latency:     {}ms", median);

    println!();
    println!("⚡ Performance Distribution:");
    println!("🟢 Excellent (<50ms):  {}/{} ({:.1}%)", 
        excellent, latencies.len(), (excellent as f64 / latencies.len() as f64) * 100.0);
    println!("🟡 Good (<100ms):      {}/{} ({:.1}%)", 
        good, latencies.len(), (good as f64 / latencies.len() as f64) * 100.0);
    println!("🟠 Fair (<200ms):      {}/{} ({:.1}%)", 
        fair, latencies.len(), (fair as f64 / latencies.len() as f64) * 100.0);

    println!();
    println!("💡 Interpretation:");
    println!("• This measures LaserStream's internal processing + network delivery");
    println!("• Does NOT include upstream delays (validator → LaserStream)");
    println!("• <100ms indicates excellent LaserStream service performance");
    
    println!();
    println!("🎯 Service Quality Assessment:");
    if avg < 50.0 {
        println!("✅ EXCELLENT - LaserStream is performing optimally");
    } else if avg < 100.0 {
        println!("🟡 GOOD - LaserStream performance is solid");
    } else if avg < 200.0 {
        println!("🟠 FAIR - LaserStream has some processing delays");
    } else {
        println!("🔴 POOR - Consider different region or troubleshoot connection");
    }
}