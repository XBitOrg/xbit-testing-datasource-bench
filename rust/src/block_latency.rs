use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use anyhow::Result;

#[derive(Serialize)]
struct RPCRequest {
    jsonrpc: String,
    id: u32,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct RPCResponse<T> {
    jsonrpc: String,
    id: u32,
    result: Option<T>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct BlockInfo {
    #[serde(rename = "blockTime")]
    pub block_time: Option<i64>,
    #[serde(rename = "blockHeight")]
    pub block_height: Option<u64>,
    pub slot: u64,
    #[serde(rename = "parentSlot")]
    pub parent_slot: u64,
}

// Remove SlotInfo struct since getSlot returns a plain number

#[derive(Debug, Serialize, Clone)]
pub struct BlockLatencyResult {
    pub slot: u64,
    pub block_time_unix: Option<i64>,
    pub rpc_received_time_unix: i64,
    pub propagation_latency_ms: Option<i64>,
    pub rpc_call_latency_ms: u64,
    pub rpc_provider: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BlockLatencyStats {
    pub total_measurements: usize,
    pub successful_measurements: usize,
    pub failed_measurements: usize,
    pub success_rate: f64,
    pub propagation_latency: Option<LatencyStats>,
    pub rpc_call_latency: LatencyStats,
}

#[derive(Debug, Serialize)]
pub struct LatencyStats {
    pub avg: f64,
    pub min: i64,
    pub max: i64,
    pub p50: i64,
    pub p95: i64,
    pub p99: i64,
}

pub struct BlockLatencyTester {
    client: Client,
    rpc_url: String,
    rpc_name: String,
}

impl BlockLatencyTester {
    pub fn new(rpc_url: String, rpc_name: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            rpc_url,
            rpc_name,
        }
    }

    // Get current slot
    async fn get_current_slot(&self) -> Result<u64> {
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getSlot".to_string(),
            params: None, // getSlot doesn't need parameters for the latest slot
        };

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        // Parse as raw JSON since we know the structure now
        let response_text = response.text().await?;
        let json_value: serde_json::Value = serde_json::from_str(&response_text)?;
        
        // Extract the result
        if let Some(result) = json_value.get("result") {
            if let Some(slot) = result.as_u64() {
                Ok(slot)
            } else {
                Err(anyhow::anyhow!("Result is not a valid u64: {:?}", result))
            }
        } else {
            Err(anyhow::anyhow!("No result field in response: {:?}", json_value))
        }
    }

    // Get block information for a specific slot
    async fn get_block_info(&self, slot: u64) -> Result<(BlockInfo, u64)> {
        let start = Instant::now();
        
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getBlock".to_string(),
            params: Some(serde_json::json!([
                slot,
                {
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0,
                    "transactionDetails": "none",
                    "rewards": false
                }
            ])),
        };

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let rpc_call_latency = start.elapsed().as_millis() as u64;
        
        // Get raw response text to parse manually
        let response_text = response.text().await?;
        
        // Try to parse the JSON manually
        let json_value: serde_json::Value = serde_json::from_str(&response_text)?;
        
        // Extract the result
        if let Some(result) = json_value.get("result") {
            if result.is_null() {
                return Err(anyhow::anyhow!("Block not found for slot {}", slot));
            }
            
            // Try to extract the fields we need manually
            let block_time = result.get("blockTime").and_then(|v| v.as_i64());
            let block_height = result.get("blockHeight").and_then(|v| v.as_u64());
            let parent_slot = result.get("parentSlot").and_then(|v| v.as_u64()).unwrap_or(0);
            
            let block_info = BlockInfo {
                block_time,
                block_height,
                slot,
                parent_slot,
            };
            
            Ok((block_info, rpc_call_latency))
        } else {
            Err(anyhow::anyhow!("No result field in getBlock response: {:?}", 
                               json_value.get("error")))
        }
    }

    // Measure block propagation latency for current slot
    pub async fn measure_current_block_latency(&self) -> BlockLatencyResult {
        let rpc_received_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        match self.get_current_slot().await {
            Ok(current_slot) => {
                // Get the previous slot's block (current might not have blockTime yet)
                let target_slot = if current_slot > 0 { current_slot - 1 } else { current_slot };
                
                match self.get_block_info(target_slot).await {
                    Ok((block_info, rpc_latency)) => {
                        let propagation_latency = block_info.block_time.map(|block_time| {
                            rpc_received_time - block_time
                        });

                        BlockLatencyResult {
                            slot: target_slot,
                            block_time_unix: block_info.block_time,
                            rpc_received_time_unix: rpc_received_time,
                            propagation_latency_ms: propagation_latency.map(|lat| lat * 1000), // Convert to milliseconds
                            rpc_call_latency_ms: rpc_latency,
                            rpc_provider: self.rpc_name.clone(),
                            success: true,
                            error: None,
                        }
                    }
                    Err(e) => BlockLatencyResult {
                        slot: target_slot,
                        block_time_unix: None,
                        rpc_received_time_unix: rpc_received_time,
                        propagation_latency_ms: None,
                        rpc_call_latency_ms: 0,
                        rpc_provider: self.rpc_name.clone(),
                        success: false,
                        error: Some(e.to_string()),
                    }
                }
            }
            Err(e) => BlockLatencyResult {
                slot: 0,
                block_time_unix: None,
                rpc_received_time_unix: rpc_received_time,
                propagation_latency_ms: None,
                rpc_call_latency_ms: 0,
                rpc_provider: self.rpc_name.clone(),
                success: false,
                error: Some(e.to_string()),
            }
        }
    }

    // Run multiple measurements to get statistical data
    pub async fn run_block_latency_benchmark(&self, iterations: u32, interval_seconds: u64) -> BlockLatencyStats {
        println!("Running block latency benchmark for {} with {} measurements ({}s intervals)", 
                 self.rpc_name, iterations, interval_seconds);
        
        let mut results = Vec::new();
        
        for i in 0..iterations {
            let result = self.measure_current_block_latency().await;
            
            if result.success {
                if let Some(latency) = result.propagation_latency_ms {
                    println!("  Measurement {}: Slot {} - Propagation: {}ms, RPC: {}ms", 
                             i + 1, result.slot, latency, result.rpc_call_latency_ms);
                } else {
                    println!("  Measurement {}: Slot {} - No block time available, RPC: {}ms", 
                             i + 1, result.slot, result.rpc_call_latency_ms);
                }
            } else {
                println!("  Measurement {}: Failed - {}", i + 1, result.error.as_deref().unwrap_or("Unknown error"));
            }
            
            results.push(result);
            
            if i < iterations - 1 {
                tokio::time::sleep(Duration::from_secs(interval_seconds)).await;
            }
        }
        
        self.calculate_latency_stats(results)
    }

    fn calculate_latency_stats(&self, results: Vec<BlockLatencyResult>) -> BlockLatencyStats {
        let successful_results: Vec<&BlockLatencyResult> = results.iter()
            .filter(|r| r.success)
            .collect();

        let successful_count = successful_results.len();
        let total_count = results.len();

        // Calculate RPC call latency stats
        let mut rpc_latencies: Vec<i64> = successful_results.iter()
            .map(|r| r.rpc_call_latency_ms as i64)
            .collect();
        rpc_latencies.sort_unstable();

        let rpc_stats = if !rpc_latencies.is_empty() {
            let sum: i64 = rpc_latencies.iter().sum();
            LatencyStats {
                avg: sum as f64 / rpc_latencies.len() as f64,
                min: rpc_latencies[0],
                max: rpc_latencies[rpc_latencies.len() - 1],
                p50: rpc_latencies[rpc_latencies.len() / 2],
                p95: rpc_latencies[(rpc_latencies.len() as f64 * 0.95) as usize],
                p99: rpc_latencies[(rpc_latencies.len() as f64 * 0.99) as usize],
            }
        } else {
            LatencyStats { avg: 0.0, min: 0, max: 0, p50: 0, p95: 0, p99: 0 }
        };

        // Calculate propagation latency stats (only for results with block_time)
        let mut prop_latencies: Vec<i64> = successful_results.iter()
            .filter_map(|r| r.propagation_latency_ms)
            .collect();
        prop_latencies.sort_unstable();

        let prop_stats = if !prop_latencies.is_empty() {
            let sum: i64 = prop_latencies.iter().sum();
            Some(LatencyStats {
                avg: sum as f64 / prop_latencies.len() as f64,
                min: prop_latencies[0],
                max: prop_latencies[prop_latencies.len() - 1],
                p50: prop_latencies[prop_latencies.len() / 2],
                p95: prop_latencies[(prop_latencies.len() as f64 * 0.95) as usize],
                p99: prop_latencies[(prop_latencies.len() as f64 * 0.99) as usize],
            })
        } else {
            None
        };

        BlockLatencyStats {
            total_measurements: total_count,
            successful_measurements: successful_count,
            failed_measurements: total_count - successful_count,
            success_rate: (successful_count as f64 / total_count as f64) * 100.0,
            propagation_latency: prop_stats,
            rpc_call_latency: rpc_stats,
        }
    }
}

// Helper function to format duration
pub fn format_duration_ms(ms: i64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{:.1}m", ms as f64 / 60000.0)
    }
}