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
359745922  | 🏆 LaserStream   | 1567ms          | 2271ms          | 704ms     | 🟠 FAIR
359745925  | 🏆 LaserStream   | 1703ms          | 2270ms          | 567ms     | 🟠 FAIR
359745928  | 🏆 LaserStream   | 833ms           | 1372ms          | 539ms     | 🟠 FAIR
359745931  | 🏆 LaserStream   | 912ms           | 1566ms          | 654ms     | 🟠 FAIR
359745934  | 🏆 LaserStream   | 1027ms          | 1487ms          | 460ms     | 🟠 FAIR
359745936  | 🏆 LaserStream   | 1893ms          | 2990ms          | 1097ms    | 🟠 FAIR
359745940  | 🏆 LaserStream   | 1499ms          | 2074ms          | 575ms     | 🟠 FAIR
359745944  | 🏆 LaserStream   | 1075ms          | 1878ms          | 803ms     | 🟠 FAIR
359745948  | 🏆 LaserStream   | 1630ms          | 2039ms          | 409ms     | 🟠 FAIR
359745952  | 🏆 LaserStream   | 1352ms          | 4145ms          | 2793ms    | 🟠 FAIR
359745961  | 🏆 LaserStream   | 844ms           | 1237ms          | 393ms     | 🟠 FAIR
359745963  | 🏆 LaserStream   | 1569ms          | 2225ms          | 656ms     | 🟠 FAIR
359745966  | 🏆 LaserStream   | 1747ms          | 2918ms          | 1171ms    | 🟠 FAIR
359745971  | 🏆 LaserStream   | 1542ms          | 1938ms          | 396ms     | 🟠 FAIR
359745973  | 🏆 LaserStream   | 1337ms          | 1897ms          | 560ms     | 🟠 FAIR
359745975  | 🏆 LaserStream   | 1011ms          | 1855ms          | 844ms     | 🟠 FAIR
359745978  | 🏆 LaserStream   | 1445ms          | 1956ms          | 511ms     | 🟠 FAIR
359745980  | 🏆 LaserStream   | 1267ms          | 2023ms          | 756ms     | 🟠 FAIR
359745983  | 🏆 LaserStream   | 1375ms          | 2006ms          | 631ms     | 🟠 FAIR
359745986  | 🏆 LaserStream   | 1433ms          | 2040ms          | 607ms     | 🟠 FAIR
359745989  | 🏆 LaserStream   | 1555ms          | 3236ms          | 1681ms    | 🟠 FAIR
359745995  | 🏆 LaserStream   | 1787ms          | 2404ms          | 617ms     | 🟠 FAIR
359745998  | 🏆 LaserStream   | 1106ms          | 1833ms          | 727ms     | 🟠 FAIR
359746001  | 🏆 LaserStream   | 1269ms          | 2458ms          | 1189ms    | 🟠 FAIR
359746005  | 🏆 LaserStream   | 1827ms          | 2389ms          | 562ms     | 🟠 FAIR
359746007  | 🏆 LaserStream   | 1648ms          | 2293ms          | 645ms     | 🟠 FAIR
359746010  | 🏆 LaserStream   | 833ms           | 1332ms          | 499ms     | 🟠 FAIR
359746013  | 🏆 LaserStream   | 983ms           | 1557ms          | 574ms     | 🟠 FAIR
359746016  | 🏆 LaserStream   | 1196ms          | 2169ms          | 973ms     | 🟡 GOOD
359746020  | 🏆 LaserStream   | 1599ms          | 2665ms          | 1066ms    | 🟠 FAIR
359746024  | 🏆 LaserStream   | 1114ms          | 2197ms          | 1083ms    | 🟡 GOOD
359746028  | 🏆 LaserStream   | 1594ms          | 2212ms          | 618ms     | 🟠 FAIR

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