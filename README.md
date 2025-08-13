# Solana RPC Performance Testing

A comprehensive performance testing suite for Solana RPC/gRPC endpoints implemented in Node.js, Go, and Rust. This project provides detailed performance metrics, automated comparisons, and professional reporting capabilities.

## 🚀 Features

- **Multi-language implementations**: Node.js, Go, and Rust
- **Comprehensive metrics**: Latency (avg, min, max, p50, p95, p99), success rate, throughput
- **Professional reporting**: JSON, CSV, HTML reports with charts
- **Automated comparisons**: Side-by-side performance analysis
- **Configurable endpoints**: Test against any Solana RPC endpoint
- **Extensible**: Easy to add support for Ethereum and other chains

## 📁 Project Structure

```
├── nodejs/           # Node.js implementation
├── golang/           # Go implementation  
├── rust/             # Rust implementation
├── shared/           # Shared configuration and utilities
├── reports/          # Generated performance reports
├── benchmark.js      # Advanced benchmark runner
├── run-all.sh        # Quick comparison script
└── README.md         # This file
```

## 🏃 Quick Start

### Option 1: Run All Languages (Recommended)
```bash
./run-all.sh https://api.mainnet-beta.solana.com 100
```

### Option 2: Advanced Benchmark with Reports
```bash
npm install
node benchmark.js --endpoint https://api.mainnet-beta.solana.com --iterations 100 --format html
```

### Option 3: Individual Languages

**Node.js:**
```bash
cd nodejs && npm install && node index.js [endpoint] [iterations]
```

**Go:**
```bash
cd golang && go mod tidy && go run main.go [endpoint] [iterations]
```

**Rust:**
```bash
cd rust && cargo run -- --endpoint [endpoint] --iterations [iterations]
```

## 📊 Reporting & Analysis

### Automated Benchmark Runner

The `benchmark.js` script provides advanced reporting capabilities:

```bash
# Basic benchmark with HTML report
node benchmark.js --format html

# Custom configuration
node benchmark.js \
  --endpoint https://api.devnet.solana.com \
  --iterations 200 \
  --format csv \
  --output my-benchmark

# Multiple endpoints comparison
node benchmark.js \
  --endpoints mainnet,devnet,testnet \
  --iterations 50 \
  --format html
```

### Report Formats

1. **JSON** (`--format json`): Machine-readable results
2. **CSV** (`--format csv`): Spreadsheet-compatible data
3. **HTML** (`--format html`): Interactive web report with charts
4. **Console** (`--format console`): Terminal-friendly output

### Performance Metrics

Each test provides:
- **Latency Statistics**: avg, min, max, p50, p95, p99
- **Success Rate**: Percentage of successful requests
- **Throughput**: Requests per second
- **Error Analysis**: Breakdown of failure types
- **Language Comparison**: Side-by-side performance

## 🔧 Configuration

### Default Settings
- **Endpoint**: `https://api.mainnet-beta.solana.com`
- **Iterations**: 100 requests per language
- **Timeout**: 30 seconds
- **Methods**: getVersion, getSlot
- **Report Format**: JSON

### Custom Configuration

Edit `shared/config.json`:
```json
{
  "endpoints": {
    "solana": {
      "mainnet": "https://api.mainnet-beta.solana.com",
      "devnet": "https://api.devnet.solana.com",
      "testnet": "https://api.testnet.solana.com"
    }
  },
  "benchmark": {
    "defaultIterations": 100,
    "timeout": 30000,
    "methods": ["getVersion", "getSlot"]
  }
}
```

## 📈 Sample Output

### Console Output
```
=== RPC Performance Comparison ===
Endpoint: https://api.mainnet-beta.solana.com
Iterations: 100 per language

🟢 Node.js Results:
  Success Rate: 100.0%
  Avg Latency: 245.67ms
  P95 Latency: 380ms
  Throughput: 4.08 req/s

🔵 Go Results:
  Success Rate: 100.0%
  Avg Latency: 198.23ms
  P95 Latency: 312ms
  Throughput: 5.04 req/s

🟠 Rust Results:
  Success Rate: 100.0%
  Avg Latency: 187.45ms
  P95 Latency: 298ms
  Throughput: 5.33 req/s

🏆 Winner: Rust (fastest avg latency)
```

### HTML Report Features
- Interactive latency distribution charts
- Success rate comparison graphs
- Detailed error analysis
- Performance rankings
- Exportable results

## 🎯 Use Cases

### RPC Provider Evaluation
```bash
# Test multiple providers
node benchmark.js --endpoints \
  "https://api.mainnet-beta.solana.com,https://solana-api.projectserum.com" \
  --iterations 200 --format html
```

### Load Testing
```bash
# High-volume testing
node benchmark.js --iterations 1000 --concurrent 10
```

### CI/CD Integration
```bash
# Generate machine-readable reports
node benchmark.js --format json --output ci-results
```

## 🔄 Extending for Other Chains

### Adding Ethereum Support

1. **Update configuration** (`shared/config.json`):
```json
{
  "endpoints": {
    "ethereum": {
      "mainnet": "https://mainnet.infura.io/v3/YOUR_PROJECT_ID"
    }
  },
  "benchmark": {
    "methods": {
      "ethereum": ["eth_blockNumber", "eth_gasPrice"]
    }
  }
}
```

2. **Modify implementations** to support Ethereum JSON-RPC methods

3. **Update benchmark runner** to handle different chain types

## 🛠️ Advanced Usage

### Custom Test Scenarios

Create custom test files:
```javascript
// custom-test.js
const benchmark = require('./benchmark');

benchmark.run({
  scenarios: [
    {
      name: 'High Load Test',
      iterations: 1000,
      concurrent: 20
    },
    {
      name: 'Latency Test',
      iterations: 100,
      concurrent: 1
    }
  ]
});
```

### Monitoring Integration

Export metrics to monitoring systems:
```bash
# Prometheus format
node benchmark.js --format prometheus --output metrics.prom

# StatsD format
node benchmark.js --format statsd --statsd-host localhost:8125
```

## 🐛 Troubleshooting

### Common Issues

1. **Connection Timeouts**
   ```bash
   # Increase timeout
   node benchmark.js --timeout 60000
   ```

2. **Rate Limiting**
   ```bash
   # Add delays between requests
   node benchmark.js --delay 100
   ```

3. **Memory Issues (High Iterations)**
   ```bash
   # Run with more memory
   node --max-old-space-size=4096 benchmark.js --iterations 10000
   ```

### Debug Mode
```bash
# Enable verbose logging
DEBUG=* node benchmark.js
```

## 📋 Requirements

- **Node.js**: v16+ (for benchmark runner)
- **Go**: v1.19+ (for Go implementation)
- **Rust**: v1.70+ (for Rust implementation)
- **Internet**: For RPC endpoint access

## 🤝 Contributing

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Follow existing code patterns
4. Add tests for new features
5. Update documentation
6. Submit pull request

## 📄 License

MIT License - see LICENSE file for details

---

**Need help?** Open an issue or check the [troubleshooting guide](#-troubleshooting).