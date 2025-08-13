import { subscribe, CommitmentLevel } from 'helius-laserstream';
import { program } from 'commander';

// Command line argument parsing
program
  .name('laserstream-benchmark')
  .description('Benchmark Helius Laserstream block propagation latency')
  .option('--api-key <key>', 'Helius API key (or use HELIUS_API_KEY env var)')
  .option('--endpoint <url>', 'Helius Laserstream endpoint', 'https://laserstream-mainnet-tyo.helius-rpc.com')
  .option('--duration <minutes>', 'Test duration in minutes', '5')
  .option('--json', 'Output results as JSON', false)
  .parse();

const options = program.opts();

async function main() {
  const apiKey = options.apiKey || process.env.HELIUS_API_KEY || '9de07723-0030-4ee0-b175-6722231d5d97';

  console.log('🚀 Helius Laserstream Block Propagation Benchmark');
  console.log('Testing claim: "Fastest block propagation"');
  console.log(`Duration: ${options.duration} minutes`);
  console.log(`Endpoint: ${options.endpoint}`);
  console.log();

  const config = {
    apiKey: apiKey,
    endpoint: options.endpoint,
  };

  const request = {
    // Subscribe to all blocks for comprehensive latency testing
    blocks: {
      client: {
        accountInclude: [], // All blocks
        accountExclude: [],
        accountRequired: [],
        includeTransactions: false, // Don't need tx data for latency test
        includeAccounts: false,
        includeEntries: false,
      }
    },
    commitment: CommitmentLevel.PROCESSED, // Fastest commitment level
    // Empty objects for unused subscription types
    transactions: {},
    accounts: {},
    slots: {},
    transactionsStatus: {},
    blocksMeta: {},
    entry: {},
    accountsDataSlice: [],
  };

  const latencies = [];
  const startTime = Date.now();
  const durationMs = parseInt(options.duration) * 60 * 1000;

  let blockCount = 0;
  let totalLatency = 0;
  let minLatency = Infinity;
  let maxLatency = 0;

  console.log('📡 Connecting to Helius Laserstream...');
  console.log('⏱️  Starting latency measurement...');
  console.log();

  try {
    await subscribe(
      config,
      request,
      async (data) => {
        const receivedTime = Date.now();
        
        if (data.block) {
          const slot = data.block.slot;
          const blockTime = data.block.blockTime ? data.block.blockTime * 1000 : receivedTime;
          const propagationLatencyMs = receivedTime - blockTime;

          // Filter out unrealistic latencies
          if (propagationLatencyMs >= 0 && propagationLatencyMs < 60000) {
            blockCount++;
            totalLatency += propagationLatencyMs;
            minLatency = Math.min(minLatency, propagationLatencyMs);
            maxLatency = Math.max(maxLatency, propagationLatencyMs);

            const latencyData = {
              slot,
              blockTime: Math.floor(blockTime / 1000),
              receivedTime: Math.floor(receivedTime / 1000),
              propagationLatencyMs
            };

            latencies.push(latencyData);

            // Real-time feedback
            process.stdout.write(`⚡ Slot ${slot}: ${propagationLatencyMs}ms`);
            
            if (propagationLatencyMs < 100) {
              console.log(' 🟢 EXCELLENT');
            } else if (propagationLatencyMs < 300) {
              console.log(' 🟡 GOOD');
            } else if (propagationLatencyMs < 1000) {
              console.log(' 🟠 FAIR');
            } else {
              console.log(' 🔴 SLOW');
            }

            // Show running average every 10 blocks
            if (blockCount % 10 === 0) {
              const avg = Math.round(totalLatency / blockCount);
              console.log(`📊 Running Average: ${avg}ms (after ${blockCount} blocks)`);
              console.log();
            }
          }
        }

        // Stop after duration
        if (Date.now() - startTime >= durationMs) {
          console.log('⏰ Test duration completed');
          printBenchmarkResults(latencies, options);
          process.exit(0);
        }
      },
      async (error) => {
        console.error('❌ Laserstream error:', error);
      }
    );
  } catch (error) {
    console.error('❌ Connection error:', error);
    process.exit(1);
  }
}

function printBenchmarkResults(latencies, options) {
  if (latencies.length === 0) {
    console.log('❌ No blocks received during test period');
    return;
  }

  const times = latencies.map(l => l.propagationLatencyMs).sort((a, b) => a - b);
  const count = times.length;
  const avg = times.reduce((sum, t) => sum + t, 0) / count;
  const min = times[0];
  const max = times[count - 1];
  const p50 = times[Math.floor(count / 2)];
  const p90 = times[Math.floor(count * 0.9)];
  const p95 = times[Math.floor(count * 0.95)];
  const p99 = times[Math.floor(count * 0.99)];

  // Speed categories
  const sub100ms = times.filter(t => t < 100).length;
  const sub300ms = times.filter(t => t < 300).length;
  const sub1s = times.filter(t => t < 1000).length;

  if (options.json) {
    const results = {
      provider: 'Helius Laserstream',
      test_duration_minutes: parseInt(options.duration),
      blocks_received: count,
      latency_stats: {
        avg_ms: Math.round(avg * 10) / 10,
        min_ms: min,
        max_ms: max,
        p50_ms: p50,
        p90_ms: p90,
        p95_ms: p95,
        p99_ms: p99
      },
      speed_distribution: {
        sub_100ms: sub100ms,
        sub_300ms: sub300ms,
        sub_1000ms: sub1s,
        sub_100ms_percent: Math.round((sub100ms / count) * 1000) / 10,
        sub_300ms_percent: Math.round((sub300ms / count) * 1000) / 10,
        sub_1000ms_percent: Math.round((sub1s / count) * 1000) / 10
      },
      verdict: getPerformanceVerdict(avg)
    };
    console.log(JSON.stringify(results, null, 2));
  } else {
    console.log();
    console.log('🏁 Helius Laserstream Benchmark Results');
    console.log('='.repeat(50));
    console.log(`Blocks tested: ${count}`);
    console.log(`Average latency: ${Math.round(avg * 10) / 10}ms`);
    console.log(`Min latency: ${min}ms`);
    console.log(`Max latency: ${max}ms`);
    console.log(`Median (P50): ${p50}ms`);
    console.log(`P90: ${p90}ms`);
    console.log(`P95: ${p95}ms`);
    console.log(`P99: ${p99}ms`);
    console.log();
    
    console.log('⚡ Speed Distribution:');
    console.log(`Sub-100ms: ${sub100ms}/${count} (${Math.round((sub100ms / count) * 1000) / 10}%)`);
    console.log(`Sub-300ms: ${sub300ms}/${count} (${Math.round((sub300ms / count) * 1000) / 10}%)`);
    console.log(`Sub-1000ms: ${sub1s}/${count} (${Math.round((sub1s / count) * 1000) / 10}%)`);
    console.log();
    
    console.log('🎯 Performance Verdict:');
    const verdict = getPerformanceVerdict(avg);
    switch (verdict) {
      case 'excellent':
        console.log('✅ EXCELLENT - Laserstream delivers sub-200ms latency!');
        break;
      case 'very_good':
        console.log('✅ VERY GOOD - Sub-500ms latency, great for real-time apps');
        break;
      case 'good':
        console.log('🟡 GOOD - Sub-1000ms, suitable for most applications');
        break;
      case 'fair':
        console.log('🟠 FAIR - 1-3s latency, consider optimizing');
        break;
      default:
        console.log('🔴 SLOW - >3s latency, may need different provider');
    }
    
    console.log();
    console.log('📈 Compared to typical RPC providers:');
    console.log('• Regular HTTP RPC: 3-5 seconds');
    console.log('• Premium WebSocket: 500-2000ms');
    console.log(`• Laserstream: ${Math.round(avg)}ms average`);
    
    if (avg < 200) {
      console.log('🏆 CLAIM VERIFIED: Laserstream IS significantly faster!');
    } else if (avg < 500) {
      console.log('✅ CLAIM SUPPORTED: Much faster than regular RPCs');
    } else {
      console.log('⚠️  CLAIM QUESTIONABLE: Similar to other premium providers');
    }
  }
}

function getPerformanceVerdict(avgLatency) {
  if (avgLatency < 200) return 'excellent';
  if (avgLatency < 500) return 'very_good';
  if (avgLatency < 1000) return 'good';
  if (avgLatency < 3000) return 'fair';
  return 'poor';
}

main().catch(console.error);