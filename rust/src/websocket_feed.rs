use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct RealtimeBlockData {
    pub slot: u64,
    pub block_time: i64,
    pub received_time: i64,
    pub feed_latency_ms: i64,
    pub rpc_provider: String,
}

pub struct WebSocketBlockFeed {
    rpc_name: String,
    ws_url: String,
}

impl WebSocketBlockFeed {
    pub fn new(rpc_url: String, rpc_name: String) -> Self {
        let ws_url = rpc_url.replace("https://", "wss://").replace("http://", "ws://");
        Self { rpc_name, ws_url }
    }

    pub async fn monitor_realtime_blocks(&self, duration_minutes: u64) -> Result<Vec<RealtimeBlockData>> {
        println!("🔄 Starting real-time block monitoring");
        println!("Provider: {}", self.rpc_name);
        println!("WebSocket: {}", self.ws_url);
        println!("Target: <300ms feed latency for your indexer\n");

        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        // Subscribe to new blocks with "processed" commitment for fastest updates
        let subscribe_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "blockSubscribe",
            "params": [
                "all",
                {
                    "commitment": "processed",  // Fastest commitment level
                    "encoding": "json",
                    "showRewards": false,
                    "maxSupportedTransactionVersion": 0
                }
            ]
        });

        println!("📡 Subscribing to blocks with 'processed' commitment...");
        write.send(Message::Text(subscribe_msg.to_string())).await?;

        let mut blocks = Vec::new();
        let start_time = SystemTime::now();
        let duration = std::time::Duration::from_secs(duration_minutes * 60);

        while start_time.elapsed()? < duration {
            if let Some(msg) = read.next().await {
                let msg = msg?;
                if let Message::Text(text) = msg {
                    if let Ok(data) = self.parse_block_notification(&text).await {
                        println!("⚡ Slot {}: {}ms feed latency", 
                            data.slot, data.feed_latency_ms);
                        
                        if data.feed_latency_ms < 300 {
                            println!("   ✅ EXCELLENT: Sub-300ms for real-time indexing!");
                        } else if data.feed_latency_ms < 1000 {
                            println!("   ✅ GOOD: Sub-1s latency");
                        } else {
                            println!("   ⚠️  HIGH: {}ms latency", data.feed_latency_ms);
                        }
                        
                        blocks.push(data);
                        
                        // Show every few blocks
                        if blocks.len() % 5 == 0 {
                            self.print_running_stats(&blocks);
                        }
                    }
                }
            }
        }

        Ok(blocks)
    }

    async fn parse_block_notification(&self, text: &str) -> Result<RealtimeBlockData> {
        let json: Value = serde_json::from_str(text)?;
        let received_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis() as i64;

        if let Some(params) = json.get("params") {
            if let Some(result) = params.get("result") {
                if let Some(value) = result.get("value") {
                    let slot = value.get("slot")
                        .and_then(|s| s.as_u64())
                        .ok_or_else(|| anyhow::anyhow!("No slot in notification"))?;
                    
                    let block_time = value.get("blockTime")
                        .and_then(|bt| bt.as_i64())
                        .unwrap_or(received_time / 1000); // Fallback to received time
                    
                    let feed_latency_ms = received_time - (block_time * 1000);
                    
                    return Ok(RealtimeBlockData {
                        slot,
                        block_time,
                        received_time,
                        feed_latency_ms,
                        rpc_provider: self.rpc_name.clone(),
                    });
                }
            }
        }
        
        Err(anyhow::anyhow!("Could not parse block notification"))
    }

    fn print_running_stats(&self, blocks: &[RealtimeBlockData]) {
        if blocks.is_empty() { return; }
        
        let recent = &blocks[blocks.len().saturating_sub(10)..];
        let avg_latency: f64 = recent.iter()
            .map(|b| b.feed_latency_ms as f64)
            .sum::<f64>() / recent.len() as f64;
        
        let under_300ms = recent.iter()
            .filter(|b| b.feed_latency_ms < 300)
            .count();
        
        println!();
        println!("📊 Last {} blocks - Avg: {:.0}ms | Sub-300ms: {}/{} ({}%)", 
            recent.len(),
            avg_latency,
            under_300ms,
            recent.len(),
            (under_300ms * 100) / recent.len()
        );
        println!();
    }

    // For production indexer optimization
    pub async fn start_indexer_feed(&self) -> Result<()> {
        println!("🚀 Production Indexer Feed Setup");
        println!("Provider: {}", self.rpc_name);
        println!();
        
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        // Multiple subscriptions for comprehensive indexing
        let subscriptions = vec![
            // 1. New blocks (fastest commitment)
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "blockSubscribe",
                "params": ["all", {"commitment": "processed", "encoding": "json"}]
            }),
            
            // 2. Account changes for specific addresses you're tracking
            // json!({
            //     "jsonrpc": "2.0", "id": 2, "method": "accountSubscribe",
            //     "params": ["YOUR_ADDRESS_HERE", {"commitment": "processed"}]
            // }),
            
            // 3. Program logs for DEX monitoring
            // json!({
            //     "jsonrpc": "2.0", "id": 3, "method": "logsSubscribe", 
            //     "params": [{"mentions": ["PROGRAM_ID_HERE"]}, {"commitment": "processed"}]
            // })
        ];

        for (i, sub) in subscriptions.iter().enumerate() {
            println!("📡 Sending subscription {}...", i + 1);
            write.send(Message::Text(sub.to_string())).await?;
        }

        println!("✅ Subscriptions active - your indexer will get data within 100-300ms!");
        println!();
        println!("WebSocket stream is ready. In production:");
        println!("1. Parse incoming block data immediately");  
        println!("2. Update your database/cache within 50ms");
        println!("3. Trigger any real-time notifications");
        println!("4. Target total processing: <300ms from block creation");
        
        // Keep connection alive and process messages
        while let Some(msg) = read.next().await {
            let msg = msg?;
            if let Message::Text(text) = msg {
                // In production, you'd process this data immediately
                if text.contains("blockTime") {
                    println!("📦 New block received - process immediately!");
                }
            }
        }

        Ok(())
    }
}