use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
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

#[derive(Debug, Clone, Serialize)]
pub struct TransactionUpdate {
    pub signature: String,
    pub slot: Option<u64>,
    pub status: TransactionStatus,
    pub first_seen_time: i64,
    pub confirmation_time: Option<i64>,
    pub time_to_confirm: Option<i64>,
    pub rpc_provider: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum TransactionStatus {
    Processing,    // Just submitted to network
    Confirmed,     // Confirmed in a block
    Finalized,     // Finalized (35+ confirmations)
    Failed,        // Transaction failed
}

#[derive(Debug, Serialize)]
pub struct TransactionSpeed {
    pub signature: String,
    pub submission_to_processing_ms: Option<i64>,
    pub processing_to_confirmed_ms: Option<i64>,
    pub total_confirmation_time_ms: Option<i64>,
    pub rpc_provider: String,
}

pub struct FastTransactionMonitor {
    client: Client,
    rpc_url: String,
    rpc_name: String,
    tracked_transactions: HashMap<String, TransactionUpdate>,
}

impl FastTransactionMonitor {
    pub fn new(rpc_url: String, rpc_name: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            rpc_url,
            rpc_name,
            tracked_transactions: HashMap::new(),
        }
    }

    // Get transaction status immediately (catches processing state)
    pub async fn get_transaction_status(&self, signature: &str) -> Result<Option<TransactionUpdate>> {
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getTransaction".to_string(),
            params: Some(serde_json::json!([
                signature,
                {
                    "encoding": "json",
                    "commitment": "processed", // This catches transactions immediately
                    "maxSupportedTransactionVersion": 0
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
                // Transaction not found yet
                return Ok(None);
            }

            let slot = result.get("slot").and_then(|v| v.as_u64());
            let meta = result.get("meta");
            
            let status = if let Some(meta) = meta {
                if let Some(err) = meta.get("err") {
                    if err.is_null() {
                        TransactionStatus::Confirmed
                    } else {
                        TransactionStatus::Failed
                    }
                } else {
                    TransactionStatus::Confirmed
                }
            } else {
                TransactionStatus::Processing
            };

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            let confirmation_time = if matches!(status, TransactionStatus::Confirmed) {
                Some(now)
            } else {
                None
            };

            Ok(Some(TransactionUpdate {
                signature: signature.to_string(),
                slot,
                status,
                first_seen_time: now,
                confirmation_time,
                time_to_confirm: None,
                rpc_provider: self.rpc_name.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    // Check for new transactions in recent blocks (for discovery)
    pub async fn scan_recent_transactions(&self, limit: u64) -> Result<Vec<String>> {
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getRecentBlockhash".to_string(),
            params: Some(serde_json::json!([{"commitment": "processed"}])),
        };

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let response_text = response.text().await?;
        let json_value: serde_json::Value = serde_json::from_str(&response_text)?;

        // This is a simplified approach - in practice you'd want to:
        // 1. Subscribe to new blocks via WebSocket
        // 2. Monitor specific programs/addresses
        // 3. Use getSignaturesForAddress for specific addresses

        Ok(Vec::new()) // Placeholder - would implement based on your specific needs
    }

    // Monitor a specific transaction from submission to confirmation
    pub async fn track_transaction_speed(&mut self, signature: &str) -> Result<TransactionSpeed> {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let mut first_processing_time: Option<i64> = None;
        let mut confirmation_time: Option<i64> = None;
        let mut last_status = TransactionStatus::Processing;

        println!("Tracking transaction: {}", signature);
        println!("Looking for processing state...");

        // Poll for transaction status changes
        for attempt in 0..120 { // Poll for up to 2 minutes
            match self.get_transaction_status(signature).await {
                Ok(Some(update)) => {
                    let current_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;

                    match (&last_status, &update.status) {
                        (TransactionStatus::Processing, TransactionStatus::Processing) => {
                            if first_processing_time.is_none() {
                                first_processing_time = Some(current_time);
                                println!("  Found in processing state at {}ms", current_time - start_time);
                            }
                        }
                        (_, TransactionStatus::Confirmed) => {
                            if confirmation_time.is_none() {
                                confirmation_time = Some(current_time);
                                println!("  Confirmed at {}ms", current_time - start_time);
                                break;
                            }
                        }
                        (_, TransactionStatus::Failed) => {
                            println!("  Transaction failed at {}ms", current_time - start_time);
                            break;
                        }
                        _ => {}
                    }
                    
                    last_status = update.status;
                }
                Ok(None) => {
                    if attempt == 0 {
                        println!("  Transaction not found yet, waiting...");
                    }
                }
                Err(e) => {
                    println!("  Error checking status: {}", e);
                }
            }

            // Short polling interval for responsiveness
            time::sleep(Duration::from_millis(250)).await;
        }

        let final_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        Ok(TransactionSpeed {
            signature: signature.to_string(),
            submission_to_processing_ms: first_processing_time.map(|t| t - start_time),
            processing_to_confirmed_ms: if let (Some(proc), Some(conf)) = (first_processing_time, confirmation_time) {
                Some(conf - proc)
            } else {
                None
            },
            total_confirmation_time_ms: confirmation_time.map(|t| t - start_time),
            rpc_provider: self.rpc_name.clone(),
        })
    }

    // WebSocket-based real-time monitoring (for production use)
    pub async fn start_websocket_monitoring(&self, addresses: Vec<String>) -> Result<()> {
        // This would implement WebSocket connections to:
        // 1. accountSubscribe - monitor specific addresses
        // 2. signatureSubscribe - monitor specific transactions  
        // 3. logsSubscribe - monitor program logs
        // 4. blockSubscribe - monitor new blocks
        
        println!("WebSocket monitoring for addresses: {:?}", addresses);
        println!("This would provide real-time updates as transactions hit the mempool");
        
        // Implementation would use tokio-tungstenite or similar
        // to maintain persistent WebSocket connections
        
        Ok(())
    }

    // Compare RPC providers for transaction detection speed
    pub async fn benchmark_transaction_detection(
        providers: Vec<(String, String)>, // (name, url) pairs
        sample_signatures: Vec<String>
    ) -> Result<Vec<(String, f64)>> {
        let mut results = Vec::new();
        
        for (name, url) in providers {
            let monitor = FastTransactionMonitor::new(url, name.clone());
            let mut total_time = 0i64;
            let mut successful_detections = 0;
            
            for signature in &sample_signatures {
                let start = Instant::now();
                if let Ok(Some(_)) = monitor.get_transaction_status(signature).await {
                    total_time += start.elapsed().as_millis() as i64;
                    successful_detections += 1;
                }
            }
            
            if successful_detections > 0 {
                let avg_ms = total_time as f64 / successful_detections as f64;
                results.push((name, avg_ms));
            }
        }
        
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        Ok(results)
    }
}

// Helper to parse transaction signatures from various sources
pub fn extract_signatures_from_block(block_data: &serde_json::Value) -> Vec<String> {
    let mut signatures = Vec::new();
    
    if let Some(transactions) = block_data.get("transactions") {
        if let Some(tx_array) = transactions.as_array() {
            for tx in tx_array {
                if let Some(sigs) = tx.get("transaction").and_then(|t| t.get("signatures")) {
                    if let Some(sig_array) = sigs.as_array() {
                        for sig in sig_array {
                            if let Some(signature) = sig.as_str() {
                                signatures.push(signature.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    
    signatures
}

// Calculate transaction confirmation percentiles for UX optimization
pub fn calculate_confirmation_stats(speeds: &[TransactionSpeed]) -> ConfirmationStats {
    let mut times: Vec<i64> = speeds
        .iter()
        .filter_map(|s| s.total_confirmation_time_ms)
        .collect();
    
    times.sort();
    
    if times.is_empty() {
        return ConfirmationStats::default();
    }
    
    ConfirmationStats {
        count: times.len(),
        avg_ms: times.iter().sum::<i64>() as f64 / times.len() as f64,
        p50_ms: times[times.len() / 2],
        p90_ms: times[(times.len() as f64 * 0.9) as usize],
        p95_ms: times[(times.len() as f64 * 0.95) as usize],
        p99_ms: times[(times.len() as f64 * 0.99) as usize],
        min_ms: times[0],
        max_ms: times[times.len() - 1],
    }
}

#[derive(Debug, Default, Serialize)]
pub struct ConfirmationStats {
    pub count: usize,
    pub avg_ms: f64,
    pub p50_ms: i64,
    pub p90_ms: i64,
    pub p95_ms: i64,
    pub p99_ms: i64,
    pub min_ms: i64,
    pub max_ms: i64,
}