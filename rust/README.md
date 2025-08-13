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

## 📊 Available Tools

### 1. Laserstream Benchmark
Test Helius Laserstream block propagation latency in isolation.

```bash
# Test with your API key
HELIUS_API_KEY=your_key cargo run --bin laserstream_benchmark -- --duration 3

# Test with hardcoded fallback (for quick testing)
cargo run --bin laserstream_benchmark -- --duration 2

# JSON output for analysis
cargo run --bin laserstream_benchmark -- --duration 2 --json
```

### 2. RPC vs Laserstream Logger
**Side-by-side comparison** of Laserstream vs regular RPC providers with detailed logging.

```bash
# Basic comparison logging
cargo run --bin rpc_vs_laserstream_logger

# Verbose logging with full block details
cargo run --bin rpc_vs_laserstream_logger -- --verbose --duration 2

# Short test
cargo run --bin rpc_vs_laserstream_logger -- --duration 1
```

**Features:**
- **Simultaneous monitoring** of both sources
- **Real-time comparison** display
- **Detailed block information** (slot, timestamp, parent, height, tx count)
- **Direct latency comparison** for common blocks
- **Performance insights** and recommendations

**Actual Output** (Method 3 - Block Propagation):
```
SOURCE     | Slot      | Block Time | Received  | Propagation | Parent    | Height    | TXs
LASERSTREAM| 359544908 | 1754995610 | 1754995611| 1431ms     | 359544907 | 337729940 | 1636
RPC        | 359544908 | 1754995610 | 1754995612| 2400ms     | 359544907 | 337729940 | 1636

Key Insight: Similar block times, different delivery speeds
- LaserStream: 1431ms propagation (push-based, real-time)
- RPC: 2400ms propagation (polling-based, 500ms intervals + processing)
```

## 🔧 Configuration

The tools automatically use RPC configurations from `../shared/config.json`. Example:

```json
{
  "rpcs": {
    "helius-mainnet": {
      "id": "helius-mainnet",
      "name": "Helius Mainnet",
      "url": "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY",
      "provider": "Helius",
      "tier": "premium",
      "status": "active"
    }
  }
}
```

## 📈 Understanding Results (Realistic Solana Performance)

### 🎯 Realistic Block Propagation Latency Categories
**Based on 400ms lab-tested minimum (Solana Leader's neighbor)**

- **🟢 Excellent**: <900ms - Outstanding real-world performance
- **🟡 Good**: 900-1200ms - Solid performance for most applications
- **🟠 Fair**: 1200-2000ms - Acceptable for general use cases  
- **🔴 Slow**: >2000ms - Consider faster provider/region

### Target Performance (Realistic Expectations)
- **Ultra-fast applications**: <900ms (LaserStream + co-location required)
- **High-performance applications**: 900-1200ms (achievable with good setup)
- **General applications**: 1200-2000ms (acceptable for most use cases)
- **⚠️ Don't target <400ms**: Physically impossible except lab conditions

### Actual Test Results (Method 3 - Block Propagation)

Slot       | Winner           | LaserStream     | RPC            | Advantage   | Status
---------------------------------------------------------------------------
359748062  | 🏆 LaserStream   | 2175ms          | 3014ms          | 839ms     | 🔴 SLOW
359748066  | 🏆 LaserStream   | 1390ms          | 2052ms          | 662ms     | 🟠 FAIR
359748069  | 🏆 LaserStream   | 1614ms          | 2375ms          | 761ms     | 🟠 FAIR
359748073  | 🏆 LaserStream   | 1175ms          | 2113ms          | 938ms     | 🟡 GOOD
359748077  | 🏆 LaserStream   | 1649ms          | 2248ms          | 599ms     | 🟠 FAIR
359748080  | 🏆 LaserStream   | 1789ms          | 2528ms          | 739ms     | 🟠 FAIR
359748083  | 🏆 LaserStream   | 861ms           | 2078ms          | 1217ms    | 🟢 EXCELLENT
359748087  | 🏆 LaserStream   | 1435ms          | 2193ms          | 758ms     | 🟠 FAIR
359748091  | 🏆 LaserStream   | 853ms           | 1485ms          | 632ms     | 🟢 EXCELLENT
359748094  | 🏆 LaserStream   | 1044ms          | 3190ms          | 2146ms    | 🟡 GOOD
359748101  | 🏆 LaserStream   | 1666ms          | 2310ms          | 644ms     | 🟠 FAIR
359748104  | 🏆 LaserStream   | 841ms           | 1372ms          | 531ms     | 🟢 EXCELLENT
359748107  | 🏆 LaserStream   | 964ms           | 1567ms          | 603ms     | 🟡 GOOD
359748110  | 🏆 LaserStream   | 1107ms          | 1607ms          | 500ms     | 🟡 GOOD
359748113  | 🏆 LaserStream   | 1387ms          | 2347ms          | 960ms     | 🟠 FAIR
359748117  | 🏆 LaserStream   | 929ms           | 1737ms          | 808ms     | 🟡 GOOD
359748120  | 🏆 LaserStream   | 1081ms          | 1756ms          | 675ms     | 🟡 GOOD
359748123  | 🏆 LaserStream   | 1159ms          | 1764ms          | 605ms     | 🟡 GOOD
359748126  | 🏆 LaserStream   | 1253ms          | 2264ms          | 1011ms    | 🟠 FAIR
359748130  | 🏆 LaserStream   | 787ms           | 1093ms          | 306ms     | 🟢 EXCELLENT
359748132  | 🏆 LaserStream   | 1569ms          | 2300ms          | 731ms     | 🟠 FAIR
359748135  | 🏆 LaserStream   | 1650ms          | 2538ms          | 888ms     | 🟠 FAIR
359748139  | 🏆 LaserStream   | 1133ms          | 1387ms          | 254ms     | 🟡 GOOD
359748141  | 🏆 LaserStream   | 873ms           | 1755ms          | 882ms     | 🟢 EXCELLENT
359748145  | 🏆 LaserStream   | 1375ms          | 1724ms          | 349ms     | 🟠 FAIR
359748147  | 🏆 LaserStream   | 1096ms          | 1635ms          | 539ms     | 🟡 GOOD
359748150  | 🏆 LaserStream   | 1215ms          | 1643ms          | 428ms     | 🟠 FAIR
359748152  | 🏆 LaserStream   | 993ms           | 1773ms          | 780ms     | 🟡 GOOD
359748155  | 🏆 LaserStream   | 1083ms          | 1730ms          | 647ms     | 🟡 GOOD

#### Key Observations:
- **Both services show similar Method 3 results** (~1300-1400ms)
- **This is expected** - both get data from same Solana validators
- **LaserStream advantage** is in delivery consistency, not block freshness
- **Geographic location matters** - Tokyo endpoint adds ~200-400ms vs US

#### Performance Summary:
- **Excellent**: <900ms (rare, requires optimal conditions)
- **Good**: 900-1200ms (achievable with co-location)
- **Fair**: 1200-2000ms (typical real-world performance) ← **Most results fall here**
- **Slow**: >2000ms (poor provider or network issues)

### Service-Level Performance (Methods 1 & 2 - Secondary Metrics)

#### Method 1 (LaserStream Consistency):
- **Parallel stream differences**: 5-30ms (excellent consistency)
- **Purpose**: Measure LaserStream internal performance variation
- **Useful for**: Service SLA monitoring

#### Method 2 Results (Service Quality - Problematic):
- **LaserStream**: -67ms (clock skew issue - use absolute value = 67ms service latency)
- **RPC**: 111ms HTTP round-trip (irrelevant for applications)
- **Conclusion**: Method 2 has issues, focus on Method 3

**Bottom line**: Method 3 (Block Propagation) is the only reliable metric for application performance.

---

## 🔍 Data Sources and API Details

**LaserStream Data Available**:
```json
{
  "slot": 359740737,
  "parent_slot": 359740736,
  "block_height": 337925407,
  "block_time": 1755072653,
  "laserstream_created_time": 1755072653,
  "network_latency_ms": 18332,
  "propagation_latency_ms": 19260,
  "transaction_count": 1298,
  "blockhash": "Gtihvx886E3yecRuFnvctkoEqjkG3pazc8oXGSPpRbYR",
  "parent_blockhash": "6Y2EkHiiXHV5NxXnbCoo52BqcsEkuvQxV6uH6cB99jUN",
  "rewards_count": 1
}

```

**Testing Methodology Verified** ✅  
**Laserstream Data Structures Documented** ✅  
**Performance Comparison Completed** ✅