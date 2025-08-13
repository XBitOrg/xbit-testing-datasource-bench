use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;
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

#[derive(Debug, Serialize, Clone)]
pub struct BlockFeedLatency {
    pub slot: u64,
    pub block_time: i64,           // When block was produced by validator
    pub rpc_received_time: i64,    // When our RPC provider received it
    pub feed_latency_ms: i64,      // Difference = how late the feed is
    pub rpc_provider: String,
}

#[derive(Debug, Serialize)]
pub struct FeedLatencyStats {
    pub provider: String,
    pub samples: usize,
    pub avg_latency_ms: f64,
    pub min_latency_ms: i64,
    pub max_latency_ms: i64,
    pub p50_latency_ms: i64,
    pub p95_latency_ms: i64,
    pub recent_blocks: Vec<BlockFeedLatency>,
}

pub struct BlockFeedMonitor {
    client: Client,
    rpc_url: String,
    rpc_name: String,
}

impl BlockFeedMonitor {
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

    // Get the latest slot and its block time
    async fn get_latest_slot(&self) -> Result<u64> {
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getSlot".to_string(),
            params: Some(serde_json::json!([{"commitment": "confirmed"}])),
        };

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let response_text = response.text().await?;
        let json_value: serde_json::Value = serde_json::from_str(&response_text)?;

        // Debug: print the response to understand the structure
        println!("Debug - getSlot response: {}", response_text);

        if let Some(slot) = json_value.get("result").and_then(|v| v.as_u64()) {
            Ok(slot)
        } else {
            Err(anyhow::anyhow!("Failed to get slot from response: {}", response_text))
        }
    }

    // Get block info including block time
    pub async fn get_block_info(&self, slot: u64) -> Result<Option<BlockFeedLatency>> {
        let request_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getBlock".to_string(),
            params: Some(serde_json::json!([
                slot,
                {
                    "encoding": "json",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0,
                    "rewards": false
                }
            ])),
        };

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let response_text = response.text().await?;
        let json_value: serde_json::Value = serde_json::from_str(&response_text)?;

        if let Some(result) = json_value.get("result") {
            if result.is_null() {
                return Ok(None); // Block not available yet
            }

            if let Some(block_time) = result.get("blockTime").and_then(|v| v.as_i64()) {
                let feed_latency_ms = (request_time - block_time) * 1000;

                Ok(Some(BlockFeedLatency {
                    slot,
                    block_time,
                    rpc_received_time: request_time,
                    feed_latency_ms,
                    rpc_provider: self.rpc_name.clone(),
                }))
            } else {
                Err(anyhow::anyhow!("Block time not available for slot {}", slot))
            }
        } else {
            Ok(None)
        }
    }

    // Monitor feed latency continuously
    pub async fn monitor_feed_latency(&self, duration_minutes: u64) -> Result<Vec<BlockFeedLatency>> {
        let mut latencies = Vec::new();
        
        // Get current slot to start from
        let current_slot = self.get_latest_slot().await?;
        let mut last_slot = current_slot;
        
        println!("Monitoring block feed latency for {} minutes...", duration_minutes);
        println!("Provider: {}", self.rpc_name);
        println!("Starting from slot: {}", current_slot);
        println!("Checking how late blocks arrive compared to validator production time\n");

        let end_time = SystemTime::now() + Duration::from_secs(duration_minutes * 60);

        while SystemTime::now() < end_time {
            match self.get_latest_slot().await {
                Ok(current_slot) => {
                    if current_slot > last_slot {
                        // New block(s) available, check the latest few
                        for slot in (last_slot + 1)..=current_slot {
                            match self.get_block_info(slot).await {
                                Ok(Some(latency)) => {
                                    println!("Slot {}: Block produced at {}, received {}ms later", 
                                        latency.slot, 
                                        format_timestamp(latency.block_time),
                                        latency.feed_latency_ms);
                                    
                                    latencies.push(latency);
                                }
                                Ok(None) => {
                                    println!("Slot {}: Block not available yet", slot);
                                }
                                Err(e) => {
                                    println!("Error getting block {}: {}", slot, e);
                                }
                            }
                        }
                        last_slot = current_slot;
                    }
                }
                Err(e) => {
                    println!("Error getting latest slot: {}", e);
                }
            }

            // Check every few seconds for new blocks
            time::sleep(Duration::from_secs(2)).await;
        }

        Ok(latencies)
    }

    // Compare multiple RPC providers for feed latency
    pub async fn compare_feed_latencies(
        providers: Vec<(String, String)>, // (name, url) pairs
        test_slots: Vec<u64>
    ) -> Result<Vec<FeedLatencyStats>> {
        let mut provider_stats = Vec::new();

        for (name, url) in providers {
            let monitor = BlockFeedMonitor::new(url, name.clone());
            let mut latencies = Vec::new();

            println!("Testing {} slots for provider: {}", test_slots.len(), name);

            for slot in &test_slots {
                if let Ok(Some(latency)) = monitor.get_block_info(*slot).await {
                    latencies.push(latency);
                }
                // Small delay to avoid rate limiting
                time::sleep(Duration::from_millis(100)).await;
            }

            if !latencies.is_empty() {
                let stats = calculate_feed_stats(&latencies, name);
                provider_stats.push(stats);
            }
        }

        provider_stats.sort_by(|a, b| a.avg_latency_ms.partial_cmp(&b.avg_latency_ms).unwrap());
        Ok(provider_stats)
    }

    // Get recent slots for testing
    pub async fn get_recent_slots(&self, count: u32) -> Result<Vec<u64>> {
        let latest_slot = self.get_latest_slot().await?;
        let slots: Vec<u64> = ((latest_slot - count as u64 + 1)..=latest_slot).collect();
        Ok(slots)
    }
}

fn calculate_feed_stats(latencies: &[BlockFeedLatency], provider_name: String) -> FeedLatencyStats {
    let mut times: Vec<i64> = latencies.iter().map(|l| l.feed_latency_ms).collect();
    times.sort();

    let avg_latency = times.iter().sum::<i64>() as f64 / times.len() as f64;
    let min_latency = times.first().copied().unwrap_or(0);
    let max_latency = times.last().copied().unwrap_or(0);
    let p50_latency = times[times.len() / 2];
    let p95_latency = times[(times.len() as f64 * 0.95) as usize];

    FeedLatencyStats {
        provider: provider_name,
        samples: latencies.len(),
        avg_latency_ms: avg_latency,
        min_latency_ms: min_latency,
        max_latency_ms: max_latency,
        p50_latency_ms: p50_latency,
        p95_latency_ms: p95_latency,
        recent_blocks: latencies.iter().take(5).cloned().collect(),
    }
}

fn format_timestamp(timestamp: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_else(|| Utc::now());
    dt.format("%H:%M:%S").to_string()
}

// WebSocket-based real-time block feed monitoring
pub async fn monitor_realtime_block_feed(rpc_url: String, provider_name: String) -> Result<()> {
    println!("Real-time Block Feed Monitoring: {}", provider_name);
    println!("This would use WebSocket subscriptions for:");
    println!("1. blockSubscribe - Get notified immediately when new blocks arrive");
    println!("2. Compare block timestamp vs notification time");
    println!("3. Measure your indexer's data freshness in real-time");
    println!();

    // WebSocket URL conversion
    let ws_url = rpc_url.replace("https://", "wss://").replace("http://", "ws://");
    println!("WebSocket endpoint: {}", ws_url);

    // Example subscription message
    let subscribe_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "blockSubscribe",
        "params": [
            "all", // or "confirmed", "finalized"
            {
                "commitment": "confirmed",
                "encoding": "json",
                "showRewards": false,
                "maxSupportedTransactionVersion": 0
            }
        ]
    });

    println!("Subscription message:");
    println!("{}", serde_json::to_string_pretty(&subscribe_msg)?);
    println!();

    println!("This would provide real-time measurements of:");
    println!("- Block production time (from blockTime field)");
    println!("- Feed delivery time (when WebSocket notification arrives)");
    println!("- Indexer processing delay (how late your data is)");
    println!();

    println!("For your indexer optimization:");
    println!("- Target <1000ms feed latency for real-time applications");
    println!("- Monitor latency spikes that could affect user experience");
    println!("- Compare multiple RPC providers to find fastest feed");

    Ok(())
}