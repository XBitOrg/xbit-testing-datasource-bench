import WebSocket from 'ws';
import { program } from 'commander';

// Command line argument parsing
program
  .name('websocket-block-benchmark')
  .description('Benchmark WebSocket block propagation latency (fallback for Laserstream)')
  .option('--url <url>', 'WebSocket RPC URL', 'wss://api.mainnet-beta.solana.com')
  .option('--duration <minutes>', 'Test duration in minutes', '3')
  .option('--json', 'Output results as JSON', false)
  .parse();

const options = program.opts();

async function main() {
  console.log('🚀 WebSocket Block Propagation Benchmark');
  console.log('Alternative to Helius Laserstream for comparison');
  console.log(`WebSocket URL: ${options.url}`);
  console.log(`Duration: ${options.duration} minutes`);
  console.log();

  const latencies = [];
  const startTime = Date.now();
  const durationMs = parseInt(options.duration) * 60 * 1000;

  let blockCount = 0;
  let totalLatency = 0;
  let minLatency = Infinity;
  let maxLatency = 0;

  try {
    const ws = new WebSocket(options.url);

    ws.on('open', () => {
      console.log('📡 Connected to WebSocket');
      console.log('⏱️  Starting latency measurement...');
      console.log();

      // Subscribe to block updates
      const subscribeMessage = JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'blockSubscribe',
        params: [
          'all',
          {
            commitment: 'processed', // Fastest commitment level
            encoding: 'json',
            showRewards: false,
            maxSupportedTransactionVersion: 0
          }
        ]
      });

      ws.send(subscribeMessage);
    });

    ws.on('message', (data) => {
      const receivedTime = Date.now();
      
      try {
        const message = JSON.parse(data.toString());
        
        // Handle block notifications
        if (message.method === 'blockNotification' && message.params && message.params.result) {
          const block = message.params.result.value;
          
          if (block && block.blockTime) {
            const slot = block.slot;
            const blockTime = block.blockTime * 1000; // Convert to milliseconds
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
              
              if (propagationLatencyMs < 500) {
                console.log(' 🟢 EXCELLENT');
              } else if (propagationLatencyMs < 1000) {
                console.log(' 🟡 GOOD');
              } else if (propagationLatencyMs < 3000) {
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
        }
      } catch (err) {
        // Ignore JSON parsing errors
      }

      // Stop after duration
      if (Date.now() - startTime >= durationMs) {
        console.log('⏰ Test duration completed');
        ws.close();
        printBenchmarkResults(latencies, options);
      }
    });

    ws.on('error', (error) => {
      console.error('❌ WebSocket error:', error.message);
      process.exit(1);
    });

    ws.on('close', () => {
      console.log('🔌 WebSocket connection closed');
      if (latencies.length > 0) {
        printBenchmarkResults(latencies, options);
      } else {
        console.log('❌ No blocks received during test period');
      }
      process.exit(0);
    });

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

  // Speed categories
  const sub500ms = times.filter(t => t < 500).length;
  const sub1s = times.filter(t => t < 1000).length;
  const sub3s = times.filter(t => t < 3000).length;

  if (options.json) {
    const results = {
      provider: 'WebSocket RPC (Fallback)',
      websocket_url: options.url,
      test_duration_minutes: parseInt(options.duration),
      blocks_received: count,
      latency_stats: {
        avg_ms: Math.round(avg * 10) / 10,
        min_ms: min,
        max_ms: max,
        p50_ms: p50,
        p90_ms: p90,
        p95_ms: p95
      },
      speed_distribution: {
        sub_500ms: sub500ms,
        sub_1000ms: sub1s,
        sub_3000ms: sub3s,
        sub_500ms_percent: Math.round((sub500ms / count) * 1000) / 10,
        sub_1000ms_percent: Math.round((sub1s / count) * 1000) / 10,
        sub_3000ms_percent: Math.round((sub3s / count) * 1000) / 10
      }
    };
    console.log(JSON.stringify(results, null, 2));
  } else {
    console.log();
    console.log('🏁 WebSocket Block Propagation Results');
    console.log('='.repeat(50));
    console.log(`Provider: WebSocket RPC (${options.url})`);
    console.log(`Blocks tested: ${count}`);
    console.log(`Average latency: ${Math.round(avg * 10) / 10}ms`);
    console.log(`Min latency: ${min}ms`);
    console.log(`Max latency: ${max}ms`);
    console.log(`Median (P50): ${p50}ms`);
    console.log(`P90: ${p90}ms`);
    console.log(`P95: ${p95}ms`);
    console.log();
    
    console.log('⚡ Speed Distribution:');
    console.log(`Sub-500ms: ${sub500ms}/${count} (${Math.round((sub500ms / count) * 1000) / 10}%)`);
    console.log(`Sub-1000ms: ${sub1s}/${count} (${Math.round((sub1s / count) * 1000) / 10}%)`);
    console.log(`Sub-3000ms: ${sub3s}/${count} (${Math.round((sub3s / count) * 1000) / 10}%)`);
    console.log();
    
    console.log('📈 Comparison Baseline:');
    console.log('• This WebSocket approach: ' + Math.round(avg) + 'ms average');
    console.log('• Regular HTTP RPC: 3000-5000ms');
    console.log('• Helius Laserstream (target): <200ms');
    console.log();
    
    if (avg < 300) {
      console.log('✅ EXCELLENT: WebSocket delivers sub-300ms latency!');
    } else if (avg < 1000) {
      console.log('✅ GOOD: Sub-1000ms is suitable for most applications');
    } else {
      console.log('⚠️ Consider premium RPC providers for better performance');
    }
  }
}

main().catch(console.error);