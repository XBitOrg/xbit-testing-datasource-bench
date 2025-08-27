# Solana RPC Performance Testing - Rust

High-performance benchmarking tools for comparing Solana RPC providers, with special focus on **Helius Laserstream** block propagation latency testing.

## 🎯 Purpose

Test and compare block propagation latency between:
- **Helius Laserstream**: Real-time gRPC streaming (claims <200ms)
- **Regular RPC providers**: HTTP polling-based approaches (typically 1-5 seconds)

### Prerequisites
- Rust 1.70+
- Helius API key with Laserstream access

### Installation
```bash
git clone <your-repo>
cd rust
cargo build --release
```

## 📊 Latency Calculator - Primary Tool

### How We Calculate Latency

**Formula**: `received_time - (block_time * 1000)`

- **block_time**: Unix timestamp when block was created (from blockchain)
- **received_time**: Local timestamp when we received the block
- **latency_ms**: Time difference showing propagation delay

This measures **real network propagation latency** - how long it takes for block data to travel from Solana validators to your application.

### Usage

```bash
# Test RPC latency for 1000 blocks
cargo run --bin latency_calculator -- --method rpc --endpoint https://api.mainnet-beta.solana.com --blocks 1000

# Test gRPC (Laserstream) latency for 500 blocks
cargo run --bin latency_calculator -- --method grpc --endpoint https://laserstream-mainnet-tyo.helius-rpc.com --api-key YOUR_KEY --blocks 500

# Use environment variable for API key
HELIUS_API_KEY=your_key cargo run --bin latency_calculator -- --method grpc --endpoint https://laserstream-mainnet-tyo.helius-rpc.com --blocks 100

# Verbose logging
cargo run --bin latency_calculator -- --method rpc --endpoint https://api.mainnet-beta.solana.com --blocks 100 --verbose
```

### Parameters

- `--method <rpc|grpc>`: Choose testing method
- `--endpoint <URL>`: Target endpoint URL
- `--api-key <KEY>`: API key for gRPC (optional, uses HELIUS_API_KEY env var)
- `--blocks <NUMBER>`: Number of blocks to test for average calculation
- `--verbose`: Enable detailed logging

### Output Example

```
🚀 Latency Calculator
Method: Rpc
Endpoint: https://api.mainnet-beta.solana.com
Target blocks: 100

📡 Starting RPC latency measurement...
Slot       | Block Time    | Received Time | Latency   | Status
----------------------------------------------------------------------
293847291  | 1735123456    | 1735123459    | 843ms     | 🟢 EXCELLENT
293847292  | 1735123458    | 1735123461    | 1205ms    | 🟡 GOOD
293847293  | 1735123460    | 1735123463    | 1687ms    | 🟠 FAIR

📊 Latency Results Summary
==================================================
Method:             Rpc
Endpoint:           https://api.mainnet-beta.solana.com
Blocks processed:   100
Average latency:    1247.3ms
Min latency:        567ms
Max latency:        3421ms
Median latency:     1189ms
95th percentile:    2103ms
99th percentile:    2847ms

⚡ Performance Distribution:
🟢 Excellent (<500ms):   12/100 (12.0%)
🟡 Good (500-1000ms):    34/100 (34.0%)
🟠 Fair (1000-2000ms):   42/100 (42.0%)
🔴 Slow (>2000ms):       12/100 (12.0%)

🎯 Overall Assessment:
🟡 GOOD - Acceptable latency for most use cases
```

### Performance Categories

- **🟢 Excellent (<500ms)**: Outstanding real-world performance
- **🟡 Good (500-1000ms)**: Solid performance for most applications
- **🟠 Fair (1000-2000ms)**: Acceptable for general use cases
- **🔴 Slow (>2000ms)**: Consider faster provider/region

## 🔍 Technical Details

### Latency Calculation Methodology

**Both RPC and gRPC use identical calculation:**
```rust
let latency_ms = received_time - (block_time * 1000);
```

**Where:**
- `received_time`: System timestamp when block data arrives (milliseconds)
- `block_time`: Blockchain timestamp when block was created (seconds, converted to ms)
- `latency_ms`: Total propagation delay from creation to reception

**RPC Method:**
- Polls `getSlot()` every 500ms to detect new blocks
- Calls `getBlockTime(slot)` to get creation timestamp
- Measures time from block creation to RPC response

**gRPC Method:**
- Real-time stream receives blocks as they're processed
- Uses `block.block_time.timestamp` from stream data
- Measures time from block creation to stream reception

### Why This Matters

This latency represents the **real-world delay** your application experiences when receiving blockchain data. Lower latency means:
- Faster arbitrage opportunities
- Better user experience for real-time apps
- More accurate market data
- Reduced risk of stale information

### Realistic Expectations

Solana mainnet block propagation has physical limits:
- **Minimum possible**: ~400ms (perfect conditions, co-located)
- **Typical excellent**: 500-800ms (premium providers)
- **Good performance**: 800-1500ms (standard setups)
- **Fair performance**: 1500-3000ms (acceptable for most apps)
- **Poor performance**: >3000ms (investigate provider/network)