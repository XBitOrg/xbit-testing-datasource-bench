const { Connection } = require('@solana/web3.js');
const fetch = require('node-fetch');

class SolanaRPCTester {
  constructor(endpoint) {
    this.endpoint = endpoint;
    this.connection = new Connection(endpoint);
  }

  async testGetVersion() {
    const start = Date.now();
    try {
      const version = await this.connection.getVersion();
      const end = Date.now();
      return {
        method: 'getVersion',
        success: true,
        latency: end - start,
        result: version
      };
    } catch (error) {
      const end = Date.now();
      return {
        method: 'getVersion',
        success: false,
        latency: end - start,
        error: error.message
      };
    }
  }

  async testGetSlot() {
    const start = Date.now();
    try {
      const slot = await this.connection.getSlot();
      const end = Date.now();
      return {
        method: 'getSlot',
        success: true,
        latency: end - start,
        result: slot
      };
    } catch (error) {
      const end = Date.now();
      return {
        method: 'getSlot',
        success: false,
        latency: end - start,
        error: error.message
      };
    }
  }

  async testGetBalance(publicKey) {
    const start = Date.now();
    try {
      const balance = await this.connection.getBalance(publicKey);
      const end = Date.now();
      return {
        method: 'getBalance',
        success: true,
        latency: end - start,
        result: balance
      };
    } catch (error) {
      const end = Date.now();
      return {
        method: 'getBalance',
        success: false,
        latency: end - start,
        error: error.message
      };
    }
  }

  async runBenchmark(iterations = 100) {
    console.log(`Running Node.js RPC benchmark with ${iterations} iterations...`);
    const results = [];

    for (let i = 0; i < iterations; i++) {
      const versionResult = await this.testGetVersion();
      const slotResult = await this.testGetSlot();
      
      results.push(versionResult, slotResult);
      
      if ((i + 1) % 10 === 0) {
        console.log(`Completed ${i + 1}/${iterations} iterations`);
      }
    }

    return this.calculateStats(results);
  }

  calculateStats(results) {
    const successfulResults = results.filter(r => r.success);
    const latencies = successfulResults.map(r => r.latency);
    
    if (latencies.length === 0) {
      return { error: 'No successful requests' };
    }

    const avg = latencies.reduce((a, b) => a + b) / latencies.length;
    const min = Math.min(...latencies);
    const max = Math.max(...latencies);
    const sorted = latencies.sort((a, b) => a - b);
    const p50 = sorted[Math.floor(sorted.length * 0.5)];
    const p95 = sorted[Math.floor(sorted.length * 0.95)];
    const p99 = sorted[Math.floor(sorted.length * 0.99)];

    return {
      totalRequests: results.length,
      successfulRequests: successfulResults.length,
      failedRequests: results.length - successfulResults.length,
      successRate: (successfulResults.length / results.length) * 100,
      latency: {
        avg: Math.round(avg * 100) / 100,
        min,
        max,
        p50,
        p95,
        p99
      }
    };
  }
}

module.exports = SolanaRPCTester;

// CLI usage
if (require.main === module) {
  const endpoint = process.argv[2] || 'https://api.mainnet-beta.solana.com';
  const iterations = parseInt(process.argv[3]) || 100;
  
  const tester = new SolanaRPCTester(endpoint);
  
  tester.runBenchmark(iterations).then(stats => {
    console.log('\n=== Node.js RPC Performance Results ===');
    console.log(JSON.stringify(stats, null, 2));
  }).catch(console.error);
}