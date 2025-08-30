#!/usr/bin/env node
import { program } from 'commander';
import fetch from 'node-fetch';
import WebSocket from 'ws';

const METHODS = ['rpc', 'grpc', 'websocket'];

class LatencyMeasurement {
    constructor(slot, blockTime, receivedTime, latencyMs) {
        this.slot = slot;
        this.blockTime = blockTime;
        this.receivedTime = receivedTime;
        this.latencyMs = latencyMs;
    }
}

program
    .name('latency-calculator')
    .description('Calculate average latency for RPC or gRPC over specified number of blocks')
    .requiredOption('--method <method>', `Method to test: ${METHODS.join(', ')}`)
    .requiredOption('--endpoint <url>', 'Endpoint URL')
    .option('--api-key <key>', 'API key (for gRPC)')
    .requiredOption('--blocks <number>', 'Number of blocks to calculate average latency', parseInt)
    .option('--verbose', 'Verbose logging', false);

program.parse();
const args = program.opts();

if (!METHODS.includes(args.method)) {
    console.error(`Error: Method must be one of: ${METHODS.join(', ')}`);
    process.exit(1);
}

console.log('🚀 Latency Calculator');
console.log(`Method: ${args.method}`);
console.log(`Endpoint: ${args.endpoint}`);
console.log(`Target blocks: ${args.blocks}`);
console.log();

async function main() {
    try {
        let measurements;
        
        switch (args.method) {
            case 'rpc':
                measurements = await measureRpcLatency(args);
                break;
            case 'grpc':
                measurements = await measureGrpcLatency(args);
                break;
            case 'websocket':
                measurements = await measureWebsocketLatency(args);
                break;
            default:
                throw new Error(`Unsupported method: ${args.method}`);
        }

        printResults(measurements, args);
    } catch (error) {
        console.error('Error:', error.message);
        process.exit(1);
    }
}

async function measureRpcLatency(args) {
    const measurements = [];
    let processedBlocks = 0;

    console.log('📡 Starting RPC latency measurement...');
    console.log('Slot       | Block Time    | Received Time | Latency   | Status');
    console.log('-'.repeat(70));

    let lastSlot = await getLatestSlot(args.endpoint);

    while (processedBlocks < args.blocks) {
        try {
            const currentSlot = await getLatestSlot(args.endpoint);
            
            if (currentSlot > lastSlot) {
                const blockTime = await getBlockTime(args.endpoint, currentSlot);
                
                if (blockTime !== null) {
                    const receivedTime = Date.now();
                    const latencyMs = receivedTime - (blockTime * 1000);

                    if (latencyMs > 0 && latencyMs < 10000) {
                        const measurement = new LatencyMeasurement(
                            currentSlot,
                            blockTime,
                            receivedTime,
                            latencyMs
                        );

                        const status = getLatencyStatus(latencyMs);
                        
                        console.log(
                            `${currentSlot.toString().padEnd(10)} | ${blockTime.toString().padEnd(12)} | ${Math.floor(receivedTime / 1000).toString().padEnd(12)} | ${latencyMs.toString().padEnd(9)}ms | ${status}`
                        );

                        measurements.push(measurement);
                        processedBlocks++;

                        if (args.verbose) {
                            console.log(`Progress: ${processedBlocks}/${args.blocks} blocks processed`);
                        }
                    }
                } else if (args.verbose) {
                    console.log(`Block time not available for slot ${currentSlot}`);
                }
                
                lastSlot = currentSlot;
            }
        } catch (error) {
            if (args.verbose) {
                console.error('Error getting latest slot:', error.message);
            }
        }

        await sleep(500);
    }

    return measurements;
}

async function measureGrpcLatency(args) {
    const apiKey = args.apiKey || process.env.HELIUS_API_KEY;
    if (!apiKey) {
        throw new Error('API key required for gRPC method');
    }

    const { subscribe, CommitmentLevel } = await import('helius-laserstream');
    
    const measurements = [];
    let processedBlocks = 0;

    console.log('📡 Starting gRPC latency measurement...');
    console.log('Slot       | Block Time    | Received Time | Latency   | Status');
    console.log('-'.repeat(70));

    const config = {
        apiKey: apiKey,
        endpoint: args.endpoint,
    };

    const request = {
        blocksMeta: {
            all: {}
        },
        commitment: CommitmentLevel.PROCESSED,
        transactions: {},
        accounts: {},
        slots: {},
        transactionsStatus: {},
        blocks: {},
        entry: {},
        accountsDataSlice: [],
    };

    return new Promise(async (resolve, reject) => {
        let completed = false;
        
        const timeoutId = setTimeout(() => {
            if (!completed) {
                completed = true;
                if (processedBlocks === 0) {
                    reject(new Error('No blocks received within timeout period'));
                } else {
                    console.log(`Timeout reached. Collected ${processedBlocks} blocks.`);
                    resolve(measurements);
                }
            }
        }, 60000);

        try {
            await subscribe(
                config,
                request,
                async (data) => {
                    if (completed) return;
                    
                    const receivedTime = Date.now();
                    
                    if (data.blockMeta && processedBlocks < args.blocks) {
                        const slot = data.blockMeta.slot;
                        const blockTime = data.blockMeta.blockTime?.timestamp;
                        
                        if (blockTime) {
                            const latencyMs = receivedTime - (blockTime * 1000);

                            if (latencyMs > 0 && latencyMs < 10000) {
                                const measurement = new LatencyMeasurement(
                                    slot,
                                    blockTime,
                                    receivedTime,
                                    latencyMs
                                );

                                const status = getLatencyStatus(latencyMs);
                                
                                console.log(
                                    `${slot.toString().padEnd(10)} | ${blockTime.toString().padEnd(12)} | ${Math.floor(receivedTime / 1000).toString().padEnd(12)} | ${latencyMs.toString().padEnd(9)}ms | ${status}`
                                );

                                measurements.push(measurement);
                                processedBlocks++;

                                if (args.verbose) {
                                    console.log(`Progress: ${processedBlocks}/${args.blocks} blocks processed`);
                                }

                                if (processedBlocks >= args.blocks && !completed) {
                                    completed = true;
                                    clearTimeout(timeoutId);
                                    resolve(measurements);
                                }
                            }
                        }
                    }
                },
                async (error) => {
                    if (!completed) {
                        completed = true;
                        clearTimeout(timeoutId);
                        reject(error);
                    }
                }
            );
        } catch (error) {
            if (!completed) {
                completed = true;
                clearTimeout(timeoutId);
                reject(error);
            }
        }
    });
}

async function measureWebsocketLatency(args) {
    const measurements = [];
    let processedBlocks = 0;

    console.log('📡 Starting WebSocket latency measurement...');
    console.log('Slot       | Block Time    | Received Time | Latency   | Status');
    console.log('-'.repeat(70));

    let wsUrl = args.endpoint;
    if (wsUrl.startsWith('https://')) {
        wsUrl = wsUrl.replace('https://', 'wss://');
    } else if (wsUrl.startsWith('http://')) {
        wsUrl = wsUrl.replace('http://', 'ws://');
    } else if (!wsUrl.startsWith('ws://') && !wsUrl.startsWith('wss://')) {
        wsUrl = `wss://${wsUrl}`;
    }

    return new Promise((resolve, reject) => {
        const ws = new WebSocket(wsUrl);
        let subscriptionConfirmed = false;
        
        const timeout = setTimeout(() => {
            if (!subscriptionConfirmed) {
                ws.close();
                reject(new Error('WebSocket subscription timeout'));
            } else {
                console.log('No new blocks received in 30 seconds, continuing...');
            }
        }, 30000);

        ws.on('open', () => {
            const subscription = {
                jsonrpc: '2.0',
                id: 1,
                method: 'blockSubscribe',
                params: [
                    'all',
                    {
                        commitment: 'processed',
                        encoding: 'json',
                        transactionDetails: 'none',
                        rewards: false
                    }
                ]
            };
            
            ws.send(JSON.stringify(subscription));
        });

        ws.on('message', (data) => {
            const receivedTime = Date.now();
            
            try {
                const jsonMsg = JSON.parse(data.toString());
                
                if (args.verbose) {
                    console.log('Received WebSocket message:', JSON.stringify(jsonMsg, null, 2));
                }

                if (jsonMsg.params && jsonMsg.params.result) {
                    const result = jsonMsg.params.result;
                    const value = result.value;
                    
                    if (value && value.block) {
                        const slot = value.slot || (value.block.parentSlot ? value.block.parentSlot + 1 : 0);
                        const blockTime = value.block.blockTime;
                        
                        if (blockTime) {
                            const latencyMs = receivedTime - (blockTime * 1000);

                            if (latencyMs > 0 && latencyMs < 10000) {
                                const measurement = new LatencyMeasurement(
                                    slot,
                                    blockTime,
                                    receivedTime,
                                    latencyMs
                                );

                                const status = getLatencyStatus(latencyMs);
                                
                                console.log(
                                    `${slot.toString().padEnd(10)} | ${blockTime.toString().padEnd(12)} | ${Math.floor(receivedTime / 1000).toString().padEnd(12)} | ${latencyMs.toString().padEnd(9)}ms | ${status}`
                                );

                                measurements.push(measurement);
                                processedBlocks++;

                                if (args.verbose) {
                                    console.log(`Progress: ${processedBlocks}/${args.blocks} blocks processed`);
                                }

                                if (processedBlocks >= args.blocks) {
                                    clearTimeout(timeout);
                                    ws.close();
                                    resolve(measurements);
                                }
                            }
                        }
                    }
                } else if (jsonMsg.result !== undefined) {
                    subscriptionConfirmed = true;
                    if (args.verbose) {
                        console.log('WebSocket subscription confirmed');
                    }
                }
            } catch (error) {
                if (args.verbose) {
                    console.error('Error parsing WebSocket message:', error.message);
                }
            }
        });

        ws.on('close', () => {
            clearTimeout(timeout);
            if (processedBlocks < args.blocks) {
                console.log('WebSocket connection closed before collecting all measurements');
            }
            resolve(measurements);
        });

        ws.on('error', (error) => {
            clearTimeout(timeout);
            reject(error);
        });
    });
}

function printResults(measurements, args) {
    if (measurements.length === 0) {
        console.log('❌ No measurements collected');
        return;
    }

    const latencies = measurements.map(m => m.latencyMs);
    const total = latencies.reduce((sum, l) => sum + l, 0);
    const avg = total / latencies.length;
    
    const sortedLatencies = [...latencies].sort((a, b) => a - b);
    
    const min = sortedLatencies[0];
    const max = sortedLatencies[sortedLatencies.length - 1];
    const median = sortedLatencies[Math.floor(sortedLatencies.length / 2)];
    const p95Idx = Math.floor(sortedLatencies.length * 0.95);
    const p95 = sortedLatencies[Math.min(p95Idx, sortedLatencies.length - 1)];
    const p99Idx = Math.floor(sortedLatencies.length * 0.99);
    const p99 = sortedLatencies[Math.min(p99Idx, sortedLatencies.length - 1)];

    const excellent = latencies.filter(l => l < 500).length;
    const good = latencies.filter(l => l >= 500 && l < 1000).length;
    const fair = latencies.filter(l => l >= 1000 && l < 2000).length;
    const slow = latencies.filter(l => l >= 2000).length;

    console.log();
    console.log('📊 Latency Results Summary');
    console.log('='.repeat(50));
    console.log(`Method:             ${args.method}`);
    console.log(`Endpoint:           ${args.endpoint}`);
    console.log(`Blocks processed:   ${measurements.length}`);
    console.log(`Average latency:    ${avg.toFixed(1)}ms`);
    console.log(`Min latency:        ${min}ms`);
    console.log(`Max latency:        ${max}ms`);
    console.log(`Median latency:     ${median}ms`);
    console.log(`95th percentile:    ${p95}ms`);
    console.log(`99th percentile:    ${p99}ms`);
    console.log();

    console.log('⚡ Performance Distribution:');
    console.log(`🟢 Excellent (<500ms):   ${excellent}/${measurements.length} (${((excellent / measurements.length) * 100).toFixed(1)}%)`);
    console.log(`🟡 Good (500-1000ms):    ${good}/${measurements.length} (${((good / measurements.length) * 100).toFixed(1)}%)`);
    console.log(`🟠 Fair (1000-2000ms):   ${fair}/${measurements.length} (${((fair / measurements.length) * 100).toFixed(1)}%)`);
    console.log(`🔴 Slow (>2000ms):       ${slow}/${measurements.length} (${((slow / measurements.length) * 100).toFixed(1)}%)`);
    console.log();

    console.log('🎯 Overall Assessment:');
    if (avg < 500) {
        console.log('✅ EXCELLENT - Very fast latency!');
    } else if (avg < 1000) {
        console.log('🟡 GOOD - Acceptable latency for most use cases');
    } else if (avg < 2000) {
        console.log('🟠 FAIR - Moderate latency, consider optimization');
    } else {
        console.log('🔴 SLOW - High latency, investigate network/provider issues');
    }
}

function getLatencyStatus(latencyMs) {
    if (latencyMs < 500) {
        return '🟢 EXCELLENT';
    } else if (latencyMs < 1000) {
        return '🟡 GOOD';
    } else if (latencyMs < 2000) {
        return '🟠 FAIR';
    } else {
        return '🔴 SLOW';
    }
}

async function getLatestSlot(rpcUrl) {
    const request = {
        jsonrpc: '2.0',
        id: 1,
        method: 'getSlot',
        params: [{ commitment: 'processed' }]
    };

    const response = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request)
    });

    const jsonValue = await response.json();

    if (jsonValue.result !== undefined) {
        return jsonValue.result;
    } else if (jsonValue.error) {
        throw new Error(`getSlot error: ${JSON.stringify(jsonValue.error)}`);
    } else {
        throw new Error('Failed to get slot');
    }
}

async function getBlockTime(rpcUrl, slot) {
    const request = {
        jsonrpc: '2.0',
        id: 1,
        method: 'getBlockTime',
        params: [slot]
    };

    const response = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request)
    });

    const jsonValue = await response.json();

    if (jsonValue.error) {
        return null;
    }

    return jsonValue.result;
}

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

main().catch(console.error);