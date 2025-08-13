# Block Propagation Latency Testing

This tool measures the **block propagation latency** - the time difference between when a block was finalized on the Solana network versus when you receive it through RPC calls.

## 🎯 What is Block Propagation Latency?

**Block Propagation Latency** = `Time when you fetch block via RPC` - `Block's on-chain timestamp`

This metric tells you:
- **Data freshness**: How recent is the data you're getting?
- **RPC performance**: How quickly does the RPC provider serve new blocks?
- **Network propagation**: How fast do blocks propagate through the network?

## 🚀 Usage

### Basic Block Latency Testing

```bash
# Test specific RPC from config
cargo run --bin block_latency_test -- --rpc-id helius-mainnet --measurements 20

# Test custom RPC endpoint
cargo run --bin block_latency_test -- --url "https://api.mainnet-beta.solana.com" --name "Solana Official"

# Quick test with fewer measurements
cargo run --bin block_latency_test -- --rpc-id solana-mainnet-official -m 10 -i 2
```

### Compare Multiple Providers

```bash
# Compare all active RPCs from config
cargo run --bin block_latency_test -- --compare

# With JSON output
cargo run --bin block_latency_test -- --compare --json

# Longer test for statistical significance
cargo run --bin block_latency_test -- --compare -m 30 -i 5
```

### Integration with Main Benchmark

```bash
# Add block latency to regular benchmark
cargo run -- --block-latency --block-measurements 15

# Test specific provider with block latency
cargo run -- --provider "Helius" --block-latency --measurement-interval 3
```

## 📊 Sample Output

### Single RPC Test
```
🕐 Block Propagation Latency Test
RPC: Helius Mainnet
URL: https://chorions-inveighs-joaigjuvuf-dedicated.helius-rpc.com/...

  Measurement 1: Slot 283451234 - Propagation: 1250ms, RPC: 120ms
  Measurement 2: Slot 283451235 - Propagation: 980ms, RPC: 115ms
  Measurement 3: Slot 283451237 - Propagation: 1100ms, RPC: 125ms

📊 Results for Helius Mainnet:
──────────────────────────────────────────────────
Success Rate: 100.0%
Measurements: 15/15

📦 Block Propagation Latency:
  Average: 1.1s
  Minimum: 850ms
  Maximum: 1.8s
  P50 (Median): 1.0s
  P95: 1.5s
  P99: 1.7s
  🟡 Good propagation speed

🌐 RPC Call Latency:
  Average: 118.5ms
  P95: 145ms
  Range: 98-167ms
```

### Provider Comparison
```
🏆 Block Propagation Latency Comparison
──────────────────────────────────────────────────────────────────────────────
Provider              Tier            Success  Avg Prop.       P95 Prop.       RPC P95
──────────────────────────────────────────────────────────────────────────────
🥇 Helius (Helius Main) premium         100.0%  1.1s            1.5s            145ms
🥈 Solana Labs (Solana  free            95.0%   2.3s            3.2s            285ms
🥉 QuickNode (QuickNode premium         100.0%  2.8s            4.1s            190ms

🏆 Best Propagation Latency: Helius with avg 1.1s
```

## 🔍 Understanding the Results

### Block Propagation Latency
- **< 1 second**: 🟢 Excellent - Very fresh data
- **1-3 seconds**: 🟡 Good - Acceptable for most applications
- **3-10 seconds**: 🟠 Moderate - May impact time-sensitive applications
- **> 10 seconds**: 🔴 High - Significant delay, investigate issues

### What Affects Propagation Latency?

1. **RPC Provider Infrastructure**:
   - Node location and connectivity
   - Caching strategies
   - Load balancing efficiency

2. **Network Conditions**:
   - Internet connectivity
   - Geographic distance
   - Network congestion

3. **Solana Network**:
   - Block production timing
   - Network partitions
   - Validator performance

## 🛠️ Technical Details

### How It Works

1. **Get Current Slot**: Fetch the latest confirmed slot
2. **Get Block Info**: Retrieve block data for recent slot (usually slot-1)
3. **Extract Timestamps**:
   - `blockTime`: When the block was finalized on-chain (Unix timestamp)
   - `fetchTime`: When we received the block via RPC
4. **Calculate Latency**: `fetchTime - blockTime`

### Key Metrics

- **Block Propagation Latency**: Time from block finalization to RPC retrieval
- **RPC Call Latency**: Time for the `getBlock` API call to complete  
- **Success Rate**: Percentage of successful measurements
- **Statistical Distribution**: P50, P95, P99 percentiles

### RPC Methods Used

- `getSlot`: Get current confirmed slot
- `getBlock`: Retrieve block information including timestamp

## 🎯 Use Cases

### 1. RPC Provider Evaluation
```bash
# Compare multiple providers
cargo run --bin block_latency_test -- --compare -m 50
```

### 2. Application Performance Requirements
```bash
# Test specific requirements (e.g., sub-second latency)
cargo run --bin block_latency_test -- --rpc-id helius-mainnet -m 100 -i 1
```

### 3. Geographic Performance Testing
```bash
# Test from different regions/times
cargo run --bin block_latency_test -- --compare --json > latency_$(date +%Y%m%d_%H%M).json
```

### 4. Continuous Monitoring
```bash
# Long-running test for monitoring
cargo run --bin block_latency_test -- --rpc-id production-rpc -m 1440 -i 60  # 24 hours, 1 minute intervals
```

## 🔧 Command Line Options

```
--rpc-id <ID>           Use RPC from config file
--url <URL>             Custom RPC endpoint URL  
--name <NAME>           Display name for custom RPC
--measurements <N>      Number of measurements (default: 20)
--interval <SECONDS>    Interval between measurements (default: 3)
--compare               Test all active RPCs
--json                  Output results as JSON
--config <PATH>         Path to config file
```

## 📈 Integration Options

### JSON Output
```bash
cargo run --bin block_latency_test -- --compare --json | jq '.[] | {provider: .rpc.provider, avg_latency: .stats.propagation_latency.avg}'
```

### Monitoring Integration
The tool outputs structured JSON that can be consumed by monitoring systems like:
- Prometheus + Grafana
- DataDog
- Custom monitoring dashboards

## ⚠️ Important Notes

1. **Block Timestamps**: Not all blocks have timestamps immediately. Some measurements may fail.

2. **Statistical Significance**: Use at least 20-30 measurements for reliable statistics.

3. **Network Variability**: Results can vary significantly based on network conditions and time of day.

4. **Rate Limiting**: Be mindful of RPC rate limits when running frequent measurements.

This tool provides deep insights into RPC data freshness and helps you choose the best provider for time-sensitive Solana applications! 🚀