#!/usr/bin/env node
import { program } from 'commander';
import Client, { CommitmentLevel, SubscribeRequest } from "@triton-one/yellowstone-grpc";
import { performance } from "perf_hooks";

program
    .name('grpc-comparison')
    .description('Compare latency between multiple gRPC endpoints')
    .requiredOption('--endpoints <urls>', 'Comma-separated gRPC endpoint URLs')
    .option('--tokens <tokens>', 'Comma-separated API tokens (optional, matches endpoint order)')
    .option('--names <names>', 'Comma-separated endpoint names (optional, matches endpoint order)')
    .option('--duration <seconds>', 'Test duration in seconds', parseInt, 30)
    .option('--verbose', 'Verbose logging', false);

program.parse();
const args = program.opts();

const endpoints = args.endpoints.split(',').map((url, index) => {
    const tokens = args.tokens ? args.tokens.split(',') : [];
    const names = args.names ? args.names.split(',') : [];
    
    return {
        name: names[index] || `Endpoint-${index + 1}`,
        url: url.trim(),
        token: tokens[index]?.trim() || ""
    };
});

console.log('🚀 gRPC Endpoint Comparison');
console.log(`Endpoints: ${endpoints.map(e => e.name).join(', ')}`);
console.log(`Test duration: ${args.duration} seconds`);
console.log();

const blockDataBySlot = new Map();
const endpointStats = {};
const firstSlotReceived = {};
const activeEndpoints = new Set();
let startedFormalStats = false;
const pendingBlockData = new Map();

const maxNameLength = Math.max(...endpoints.map(e => e.name.length));

endpoints.forEach(endpoint => {
    firstSlotReceived[endpoint.name] = false;
    activeEndpoints.add(endpoint.name);
    endpointStats[endpoint.name] = {
        totalLatency: 0,
        latencies: [],
        firstReceived: 0,
        totalReceived: 0,
        isAvailable: true,
        hasReceivedData: false
    };
});

function processCollectedData() {
    for (const [slot, blockDataList] of pendingBlockData.entries()) {
        const activeEndpointData = blockDataList.filter(
            data => activeEndpoints.has(data.endpoint) && endpointStats[data.endpoint].hasReceivedData
        );

        if (activeEndpointData.length >= 2) {
            activeEndpointData.forEach(bd => {
                endpointStats[bd.endpoint].totalReceived++;
            });

            const earliestTimestamp = Math.min(...activeEndpointData.map(bd => bd.timestamp));

            activeEndpointData.forEach(bd => {
                const latency = bd.timestamp - earliestTimestamp;
                if (latency > 0) {
                    endpointStats[bd.endpoint].latencies.push(latency);
                    endpointStats[bd.endpoint].totalLatency += latency;
                    if (args.verbose) {
                        console.log(`${bd.endpoint.padEnd(maxNameLength)} slot ${bd.slot}: +${latency.toFixed(2)}ms`);
                    }
                } else {
                    endpointStats[bd.endpoint].firstReceived++;
                    if (args.verbose) {
                        console.log(`${bd.endpoint.padEnd(maxNameLength)} slot ${bd.slot}: FIRST`);
                    }
                }
            });
        }
    }
    pendingBlockData.clear();
}

function endTest() {
    console.log('\n📊 Performance Results');
    console.log('='.repeat(50));

    const sortedEndpoints = endpoints
        .map(e => ({ name: e.name, stats: endpointStats[e.name] }))
        .filter(e => e.stats.totalReceived > 0)
        .sort((a, b) => 
            (b.stats.firstReceived / b.stats.totalReceived) - 
            (a.stats.firstReceived / a.stats.totalReceived)
        );

    if (sortedEndpoints.length >= 2) {
        for (const endpoint of sortedEndpoints) {
            const firstPercent = (endpoint.stats.firstReceived / endpoint.stats.totalReceived) * 100;
            const avgLatencyWhenSlower = endpoint.stats.latencies.length > 0
                ? endpoint.stats.totalLatency / endpoint.stats.latencies.length
                : 0;

            console.log(`${endpoint.name.padEnd(maxNameLength)}: First ${firstPercent.toFixed(1)}%, Avg latency ${avgLatencyWhenSlower.toFixed(1)}ms`);
        }
    } else {
        console.log('❌ Insufficient data for comparison');
    }

    process.exit(0);
}

async function main() {
    const startTime = Date.now();
    const endTime = startTime + args.duration * 1000;
    const clients = {};
    const streams = {};
    const pingIntervals = {};

    const availabilityCheckTimeout = setTimeout(() => {
        endpoints.forEach(endpoint => {
            if (!firstSlotReceived[endpoint.name]) {
                console.log(`⚠️  ${endpoint.name} unavailable`);
                endpointStats[endpoint.name].isAvailable = false;
                activeEndpoints.delete(endpoint.name);
            }
        });

        if (!startedFormalStats && activeEndpoints.size >= 2) {
            startedFormalStats = true;
            console.log(`✅ ${activeEndpoints.size} endpoints active, starting measurement...`);
            processCollectedData();
        }
    }, Math.min(Math.floor(args.duration * 1000 / 3), 5000));

    for (const endpoint of endpoints) {
        try {
            if (args.verbose) {
                console.log(`Connecting to ${endpoint.name}: ${endpoint.url}`);
            }

            clients[endpoint.name] = new Client(endpoint.url, endpoint.token, {
                "grpc.max_receive_message_length": 16 * 1024 * 1024
            });

            streams[endpoint.name] = await clients[endpoint.name].subscribe();

            const request = {
                accounts: {},
                slots: { slot: { filterByCommitment: true } },
                transactions: {},
                transactionsStatus: {},
                blocks: {},
                blocksMeta: {},
                entry: {},
                accountsDataSlice: [],
                commitment: CommitmentLevel.PROCESSED,
                ping: undefined,
            };

            await new Promise((resolve, reject) => {
                streams[endpoint.name].write(request, err => {
                    if (!err) resolve();
                    else reject(err);
                });
            });

            streams[endpoint.name].on("data", data => {
                if (data.pong) return;

                if (data.slot) {
                    const currentSlot = parseInt(data.slot.slot);
                    const timestamp = performance.now();

                    if (!firstSlotReceived[endpoint.name]) {
                        firstSlotReceived[endpoint.name] = true;
                        endpointStats[endpoint.name].hasReceivedData = true;
                        console.log(`✅ ${endpoint.name} connected (slot ${currentSlot})`);

                        if (!startedFormalStats && Object.values(endpointStats).filter(s => s.hasReceivedData).length === endpoints.length) {
                            startedFormalStats = true;
                            console.log('📡 All endpoints ready, starting measurement...');
                            processCollectedData();
                        }
                    }

                    if (!blockDataBySlot.has(currentSlot)) {
                        blockDataBySlot.set(currentSlot, []);
                    }

                    const blockData = { endpoint: endpoint.name, slot: currentSlot, timestamp };
                    blockDataBySlot.get(currentSlot).push(blockData);

                    if (!startedFormalStats) {
                        if (!pendingBlockData.has(currentSlot)) {
                            pendingBlockData.set(currentSlot, []);
                        }
                        pendingBlockData.get(currentSlot).push(blockData);
                        return;
                    }

                    const blockDataList = blockDataBySlot.get(currentSlot);
                    const receivedEndpoints = new Set(blockDataList.map(bd => bd.endpoint));
                    const allActiveReceived = Array.from(activeEndpoints).every(ep => receivedEndpoints.has(ep));

                    if (blockDataList.length === activeEndpoints.size && allActiveReceived) {
                        blockDataList.forEach(bd => {
                            endpointStats[bd.endpoint].totalReceived++;
                        });

                        const earliestTimestamp = Math.min(...blockDataList.map(bd => bd.timestamp));

                        blockDataList.forEach(bd => {
                            const latency = bd.timestamp - earliestTimestamp;
                            if (latency > 0) {
                                endpointStats[bd.endpoint].latencies.push(latency);
                                endpointStats[bd.endpoint].totalLatency += latency;
                            } else {
                                endpointStats[bd.endpoint].firstReceived++;
                            }
                        });

                        const oldSlots = [...blockDataBySlot.keys()].filter(slot => slot < currentSlot - 100);
                        oldSlots.forEach(slot => blockDataBySlot.delete(slot));
                    }
                }
            });

            streams[endpoint.name].on("error", error => {
                console.error(`❌ ${endpoint.name} error:`, error.message);
                endpointStats[endpoint.name].isAvailable = false;
                activeEndpoints.delete(endpoint.name);
            });

            const pingRequest = {
                accounts: {},
                slots: {},
                transactions: {},
                transactionsStatus: {},
                blocks: {},
                blocksMeta: {},
                entry: {},
                accountsDataSlice: [],
                commitment: undefined,
                ping: { id: 1 },
            };

            pingIntervals[endpoint.name] = setInterval(() => {
                if (streams[endpoint.name] && !streams[endpoint.name].destroyed) {
                    streams[endpoint.name].write(pingRequest, () => {});
                }
            }, 5000);

        } catch (error) {
            console.error(`❌ Failed to connect to ${endpoint.name}:`, error.message);
            endpointStats[endpoint.name].isAvailable = false;
            activeEndpoints.delete(endpoint.name);
        }
    }

    const checkInterval = setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTime) / 1000);
        const remaining = args.duration - elapsed;

        if (elapsed % 5 === 0 && elapsed > 0) {
            console.log(`⏱️  Progress: ${elapsed}/${args.duration}s (${remaining}s remaining)`);
        }

        if (Date.now() >= endTime) {
            clearInterval(checkInterval);
            clearTimeout(availabilityCheckTimeout);
            
            Object.values(pingIntervals).forEach(clearInterval);
            Object.values(streams).forEach(stream => {
                try { stream.end(); } catch (e) {}
            });

            endTest();
        }
    }, 1000);
}

main().catch(error => {
    console.error('Error:', error.message);
    process.exit(1);
});