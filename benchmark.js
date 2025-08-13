#!/usr/bin/env node

const { Command } = require('commander');
const chalk = require('chalk');
const Table = require('cli-table3');
const fs = require('fs-extra');
const path = require('path');
const moment = require('moment');
const { execSync, spawn } = require('child_process');
const { RPCManager } = require('./shared/rpc-manager');

const program = new Command();

class BenchmarkRunner {
  constructor(options = {}) {
    this.rpcManager = new RPCManager();
    this.options = {
      endpoint: options.endpoint || null,
      iterations: options.iterations || 100,
      format: options.format || 'console',
      output: options.output || null,
      timeout: options.timeout || 30000,
      delay: options.delay || 0,
      endpoints: options.endpoints || [],
      rpcs: options.rpcs || [],
      concurrent: options.concurrent || 1,
      verbose: options.verbose || false,
      chain: options.chain || 'solana',
      provider: options.provider || null,
      tier: options.tier || null,
      network: options.network || null
    };
    
    this.results = {};
    this.startTime = new Date();
    this.resolveRPCs();
  }

  resolveRPCs() {
    let targetRPCs = {};

    // If specific RPC IDs provided, use those
    if (this.options.rpcs && this.options.rpcs.length > 0) {
      this.options.rpcs.forEach(rpcId => {
        const rpc = this.rpcManager.getRPC(rpcId);
        if (rpc) {
          targetRPCs[rpcId] = rpc;
        } else {
          console.warn(chalk.yellow(`Warning: RPC '${rpcId}' not found in config`));
        }
      });
    }
    // If legacy endpoints provided, create temporary RPC objects
    else if (this.options.endpoints && this.options.endpoints.length > 0) {
      this.options.endpoints.forEach((url, index) => {
        const tempId = `custom-${index}`;
        targetRPCs[tempId] = {
          id: tempId,
          name: `Custom Endpoint ${index + 1}`,
          url: url,
          chain: this.options.chain,
          network: 'unknown',
          provider: 'Custom',
          tier: 'unknown',
          region: 'Unknown',
          rateLimit: 'Unknown',
          features: ['http'],
          status: 'active',
          description: 'Custom endpoint provided via command line',
          methods: this.rpcManager.getTestMethods(this.options.chain),
          metadata: {}
        };
      });
    }
    // If single endpoint provided, create temporary RPC object
    else if (this.options.endpoint) {
      const tempId = 'custom-single';
      targetRPCs[tempId] = {
        id: tempId,
        name: 'Custom Endpoint',
        url: this.options.endpoint,
        chain: this.options.chain,
        network: 'unknown',
        provider: 'Custom',
        tier: 'unknown',
        region: 'Unknown',
        rateLimit: 'Unknown',
        features: ['http'],
        status: 'active',
        description: 'Custom endpoint provided via command line',
        methods: this.rpcManager.getTestMethods(this.options.chain),
        metadata: {}
      };
    }
    // Otherwise, use filtered RPCs from config
    else {
      const filter = {};
      if (this.options.chain) filter.chain = this.options.chain;
      if (this.options.provider) filter.provider = this.options.provider;
      if (this.options.tier) filter.tier = this.options.tier;
      if (this.options.network) filter.network = this.options.network;
      
      const filteredRPCs = this.rpcManager.getRPCs(filter);
      
      if (Object.keys(filteredRPCs).length === 0) {
        // Fallback to default RPCs
        targetRPCs = this.rpcManager.getDefaultRPCs();
      } else {
        targetRPCs = filteredRPCs;
      }
    }

    this.targetRPCs = targetRPCs;
  }

  async runLanguageTest(language, rpcId, rpcConfig, iterations) {
    this.log(`🚀 Running ${language} benchmark for ${rpcConfig.name}...`);
    
    try {
      let result;
      const startTime = Date.now();
      
      switch (language) {
        case 'nodejs':
          result = await this.runNodeJS(rpcConfig.url, iterations);
          break;
        case 'golang':
          result = await this.runGolang(rpcConfig.url, iterations);
          break;
        case 'rust':
          result = await this.runRust(rpcConfig.url, iterations);
          break;
        default:
          throw new Error(`Unsupported language: ${language}`);
      }
      
      const duration = Date.now() - startTime;
      result.executionTime = duration;
      result.language = language;
      result.rpcId = rpcId;
      result.rpcConfig = rpcConfig;
      result.endpoint = rpcConfig.url;
      result.timestamp = new Date().toISOString();
      
      return result;
    } catch (error) {
      this.log(`❌ ${language} benchmark failed: ${error.message}`);
      return {
        language,
        rpcId,
        rpcConfig,
        endpoint: rpcConfig.url,
        error: error.message,
        success: false,
        timestamp: new Date().toISOString()
      };
    }
  }

  async runNodeJS(endpoint, iterations) {
    const command = `cd nodejs && npm install --silent && node index.js "${endpoint}" ${iterations}`;
    const output = execSync(command, { encoding: 'utf8', timeout: this.options.timeout });
    
    // Extract JSON from output
    const jsonMatch = output.match(/\{[\s\S]*\}/);
    if (!jsonMatch) {
      throw new Error('Failed to parse Node.js output');
    }
    
    return JSON.parse(jsonMatch[0]);
  }

  async runGolang(endpoint, iterations) {
    const command = `cd golang && go mod tidy && go run main.go "${endpoint}" ${iterations}`;
    const output = execSync(command, { encoding: 'utf8', timeout: this.options.timeout });
    
    // Extract JSON from output
    const jsonMatch = output.match(/\{[\s\S]*\}/);
    if (!jsonMatch) {
      throw new Error('Failed to parse Go output');
    }
    
    return JSON.parse(jsonMatch[0]);
  }

  async runRust(endpoint, iterations) {
    const command = `cd rust && cargo run --quiet -- --endpoint "${endpoint}" --iterations ${iterations}`;
    const output = execSync(command, { encoding: 'utf8', timeout: this.options.timeout });
    
    // Extract JSON from output
    const jsonMatch = output.match(/\{[\s\S]*\}/);
    if (!jsonMatch) {
      throw new Error('Failed to parse Rust output');
    }
    
    return JSON.parse(jsonMatch[0]);
  }

  async runFullBenchmark() {
    const languages = ['nodejs', 'golang', 'rust'];
    const rpcs = this.targetRPCs;

    if (Object.keys(rpcs).length === 0) {
      console.error(chalk.red('No RPCs found to test. Please check your configuration or provide endpoints.'));
      return;
    }

    console.log(chalk.blue.bold('\n=== RPC Performance Benchmark ==='));
    console.log(chalk.gray(`Started: ${moment().format('YYYY-MM-DD HH:mm:ss')}`));
    console.log(chalk.gray(`RPCs: ${Object.keys(rpcs).length} endpoints`));
    console.log(chalk.gray(`Iterations: ${this.options.iterations} per language`));
    console.log(chalk.gray(`Languages: ${languages.join(', ')}\n`));

    // Display RPC information
    console.log(chalk.cyan('🌐 RPC Endpoints to test:'));
    Object.entries(rpcs).forEach(([id, rpc]) => {
      console.log(chalk.gray(`  • ${rpc.name} (${rpc.provider}) - ${rpc.tier} tier`));
    });
    console.log();

    for (const [rpcId, rpcConfig] of Object.entries(rpcs)) {
      this.results[rpcId] = {
        config: rpcConfig,
        tests: {}
      };
      
      console.log(chalk.yellow(`\n📡 Testing: ${rpcConfig.name} (${rpcConfig.provider})`));
      console.log(chalk.gray(`   URL: ${rpcConfig.url}`));
      console.log(chalk.gray(`   Tier: ${rpcConfig.tier} | Network: ${rpcConfig.network} | Region: ${rpcConfig.region}`));
      
      for (const language of languages) {
        const result = await this.runLanguageTest(language, rpcId, rpcConfig, this.options.iterations);
        this.results[rpcId].tests[language] = result;
        
        if (result.success !== false) {
          this.displayLanguageResult(language, result);
        }
        
        // Add delay if specified
        if (this.options.delay > 0) {
          await this.sleep(this.options.delay);
        }
      }
    }

    await this.generateReports();
    this.displaySummary();
  }

  displayLanguageResult(language, result) {
    const icon = this.getLanguageIcon(language);
    const successRate = result.successRate || 0;
    const avgLatency = result.latency ? result.latency.avg : 0;
    const p95Latency = result.latency ? result.latency.p95 : 0;
    
    const throughput = this.calculateThroughput(result);
    
    console.log(chalk.green(`${icon} ${language.charAt(0).toUpperCase() + language.slice(1)} Results:`));
    console.log(`  Success Rate: ${successRate.toFixed(1)}%`);
    console.log(`  Avg Latency: ${avgLatency.toFixed(2)}ms`);
    console.log(`  P95 Latency: ${p95Latency}ms`);
    console.log(`  Throughput: ${throughput.toFixed(2)} req/s`);
    console.log(`  Execution Time: ${result.executionTime}ms\n`);
  }

  displaySummary() {
    console.log(chalk.blue.bold('\n🏆 Performance Summary'));
    
    // Create comparison table with RPC metadata
    const table = new Table({
      head: ['Language', 'RPC Provider', 'Tier', 'Success Rate', 'Avg Latency (ms)', 'P95 Latency (ms)', 'Throughput (req/s)'],
      style: { head: ['cyan'] }
    });

    const languageResults = [];
    
    for (const rpcId in this.results) {
      const rpcData = this.results[rpcId];
      for (const language in rpcData.tests) {
        const result = rpcData.tests[language];
        if (result.success !== false) {
          languageResults.push({
            language,
            rpcId,
            rpcName: rpcData.config.name,
            provider: rpcData.config.provider,
            tier: rpcData.config.tier,
            network: rpcData.config.network,
            successRate: result.successRate || 0,
            avgLatency: result.latency ? result.latency.avg : 0,
            p95Latency: result.latency ? result.latency.p95 : 0,
            throughput: this.calculateThroughput(result)
          });
        }
      }
    }

    // Sort by average latency (ascending - lower is better)
    languageResults.sort((a, b) => a.avgLatency - b.avgLatency);

    languageResults.forEach((result, index) => {
      const row = [
        index === 0 ? chalk.green.bold(`🥇 ${result.language}`) : result.language,
        result.provider,
        result.tier,
        `${result.successRate.toFixed(1)}%`,
        `${result.avgLatency.toFixed(2)}`,
        `${result.p95Latency}`,
        `${result.throughput.toFixed(2)}`
      ];
      table.push(row);
    });

    console.log(table.toString());

    if (languageResults.length > 0) {
      const winner = languageResults[0];
      console.log(chalk.green.bold(`\n🏆 Winner: ${winner.language} with ${winner.provider} (${winner.avgLatency.toFixed(2)}ms)`));
      
      // Show RPC provider insights
      this.displayProviderInsights(languageResults);
    }
  }

  displayProviderInsights(results) {
    console.log(chalk.blue.bold('\n📊 Provider Analysis'));
    
    const providerStats = {};
    
    results.forEach(result => {
      if (!providerStats[result.provider]) {
        providerStats[result.provider] = {
          tests: 0,
          totalLatency: 0,
          totalSuccessRate: 0,
          bestLatency: Infinity,
          worstLatency: 0,
          tiers: new Set()
        };
      }
      
      const stats = providerStats[result.provider];
      stats.tests++;
      stats.totalLatency += result.avgLatency;
      stats.totalSuccessRate += result.successRate;
      stats.bestLatency = Math.min(stats.bestLatency, result.avgLatency);
      stats.worstLatency = Math.max(stats.worstLatency, result.avgLatency);
      stats.tiers.add(result.tier);
    });

    const providerTable = new Table({
      head: ['Provider', 'Tests', 'Avg Latency', 'Success Rate', 'Best/Worst', 'Tiers'],
      style: { head: ['cyan'] }
    });

    Object.entries(providerStats).forEach(([provider, stats]) => {
      const avgLatency = stats.totalLatency / stats.tests;
      const avgSuccessRate = stats.totalSuccessRate / stats.tests;
      
      providerTable.push([
        provider,
        stats.tests.toString(),
        `${avgLatency.toFixed(2)}ms`,
        `${avgSuccessRate.toFixed(1)}%`,
        `${stats.bestLatency.toFixed(0)}/${stats.worstLatency.toFixed(0)}ms`,
        Array.from(stats.tiers).join(', ')
      ]);
    });

    console.log(providerTable.toString());
  }

  async generateReports() {
    await fs.ensureDir('reports');
    
    const timestamp = moment().format('YYYY-MM-DD_HH-mm-ss');
    const baseFilename = this.options.output || `benchmark_${timestamp}`;

    switch (this.options.format.toLowerCase()) {
      case 'json':
        await this.generateJSONReport(baseFilename);
        break;
      case 'csv':
        await this.generateCSVReport(baseFilename);
        break;
      case 'html':
        await this.generateHTMLReport(baseFilename);
        break;
      case 'console':
        // Already displayed
        break;
      default:
        await this.generateJSONReport(baseFilename);
    }
  }

  async generateJSONReport(filename) {
    const reportData = {
      meta: {
        timestamp: this.startTime.toISOString(),
        duration: Date.now() - this.startTime.getTime(),
        options: this.options
      },
      results: this.results
    };

    const filepath = path.join('reports', `${filename}.json`);
    await fs.writeJSON(filepath, reportData, { spaces: 2 });
    console.log(chalk.green(`📄 JSON report saved: ${filepath}`));
  }

  async generateCSVReport(filename) {
    let csv = 'Language,RPC ID,RPC Name,Provider,Tier,Network,Region,Rate Limit,Endpoint,Success Rate (%),Total Requests,Successful Requests,Failed Requests,Avg Latency (ms),Min Latency (ms),Max Latency (ms),P50 Latency (ms),P95 Latency (ms),P99 Latency (ms),Throughput (req/s),Execution Time (ms),Features,Expected Latency,Expected Uptime\n';

    for (const rpcId in this.results) {
      const rpcData = this.results[rpcId];
      for (const language in rpcData.tests) {
        const result = rpcData.tests[language];
        if (result.success !== false) {
          const throughput = this.calculateThroughput(result);
          const config = rpcData.config;
          
          csv += `${language},"${rpcId}","${config.name}","${config.provider}","${config.tier}","${config.network}","${config.region}","${config.rateLimit}","${config.url}",${result.successRate || 0},${result.totalRequests || 0},${result.successfulRequests || 0},${result.failedRequests || 0},${result.latency ? result.latency.avg : 0},${result.latency ? result.latency.min : 0},${result.latency ? result.latency.max : 0},${result.latency ? result.latency.p50 : 0},${result.latency ? result.latency.p95 : 0},${result.latency ? result.latency.p99 : 0},${throughput.toFixed(2)},${result.executionTime || 0},"${(config.features || []).join(';')}","${config.metadata?.averageLatency || 'N/A'}","${config.metadata?.uptime || 'N/A'}"\n`;
        }
      }
    }

    const filepath = path.join('reports', `${filename}.csv`);
    await fs.writeFile(filepath, csv);
    console.log(chalk.green(`📊 CSV report saved: ${filepath}`));
  }

  async generateHTMLReport(filename) {
    const html = this.generateHTMLContent();
    const filepath = path.join('reports', `${filename}.html`);
    await fs.writeFile(filepath, html);
    console.log(chalk.green(`🌐 HTML report saved: ${filepath}`));
  }

  generateHTMLContent() {
    const chartData = this.prepareChartData();
    
    return `
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RPC Performance Benchmark Report</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background: #f5f7fa; }
        .container { max-width: 1200px; margin: 0 auto; }
        .header { background: white; padding: 30px; border-radius: 8px; margin-bottom: 20px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
        .header h1 { margin: 0; color: #2d3748; }
        .header .meta { color: #718096; margin-top: 10px; }
        .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; margin-bottom: 20px; }
        .card { background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
        .card h2 { margin-top: 0; color: #2d3748; }
        .table { width: 100%; border-collapse: collapse; margin-top: 20px; }
        .table th, .table td { padding: 12px; text-align: left; border-bottom: 1px solid #e2e8f0; }
        .table th { background: #f7fafc; font-weight: 600; }
        .winner { background: #f0fff4; }
        .chart-container { position: relative; height: 400px; margin: 20px 0; }
        @media (max-width: 768px) { .grid { grid-template-columns: 1fr; } }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 RPC Performance Benchmark Report</h1>
            <div class="meta">
                Generated: ${moment().format('YYYY-MM-DD HH:mm:ss')} | 
                Iterations: ${this.options.iterations} per language |
                RPCs Tested: ${Object.keys(this.results).length} |
                Providers: ${this.getUniqueProviders().length}
            </div>
        </div>

        <div class="grid">
            <div class="card">
                <h2>📊 Latency Comparison</h2>
                <div class="chart-container">
                    <canvas id="latencyChart"></canvas>
                </div>
            </div>
            <div class="card">
                <h2>🎯 Success Rate Comparison</h2>
                <div class="chart-container">
                    <canvas id="successChart"></canvas>
                </div>
            </div>
        </div>

        <div class="card">
            <h2>🏆 Performance Summary</h2>
            ${this.generateHTMLTable()}
        </div>

        <div class="card">
            <h2>🌐 RPC Provider Information</h2>
            ${this.generateRPCInfoTable()}
        </div>

        <div class="card">
            <h2>📈 Detailed Results</h2>
            <pre style="background: #f7fafc; padding: 15px; border-radius: 4px; overflow-x: auto;">${JSON.stringify(this.results, null, 2)}</pre>
        </div>
    </div>

    <script>
        const chartData = ${JSON.stringify(chartData)};
        
        // Latency Chart
        new Chart(document.getElementById('latencyChart'), {
            type: 'bar',
            data: {
                labels: chartData.languages,
                datasets: [{
                    label: 'Average Latency (ms)',
                    data: chartData.avgLatencies,
                    backgroundColor: ['#48bb78', '#4299e1', '#ed8936'],
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: { beginAtZero: true }
                }
            }
        });

        // Success Rate Chart
        new Chart(document.getElementById('successChart'), {
            type: 'doughnut',
            data: {
                labels: chartData.languages,
                datasets: [{
                    data: chartData.successRates,
                    backgroundColor: ['#48bb78', '#4299e1', '#ed8936'],
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });
    </script>
</body>
</html>`;
  }

  generateHTMLTable() {
    let html = '<table class="table"><thead><tr>';
    html += '<th>Language</th><th>RPC Provider</th><th>Tier</th><th>Success Rate</th><th>Avg Latency</th><th>P95 Latency</th><th>Throughput</th>';
    html += '</tr></thead><tbody>';

    const languageResults = [];
    
    for (const rpcId in this.results) {
      const rpcData = this.results[rpcId];
      for (const language in rpcData.tests) {
        const result = rpcData.tests[language];
        if (result.success !== false) {
          languageResults.push({
            language,
            provider: rpcData.config.provider,
            tier: rpcData.config.tier,
            successRate: result.successRate || 0,
            avgLatency: result.latency ? result.latency.avg : 0,
            p95Latency: result.latency ? result.latency.p95 : 0,
            throughput: this.calculateThroughput(result)
          });
        }
      }
    }

    languageResults.sort((a, b) => a.avgLatency - b.avgLatency);

    languageResults.forEach((result, index) => {
      const rowClass = index === 0 ? 'winner' : '';
      const icon = index === 0 ? '🥇 ' : '';
      html += `<tr class="${rowClass}">`;
      html += `<td>${icon}${result.language}</td>`;
      html += `<td>${result.provider}</td>`;
      html += `<td><span class="tier-${result.tier}">${result.tier}</span></td>`;
      html += `<td>${result.successRate.toFixed(1)}%</td>`;
      html += `<td>${result.avgLatency.toFixed(2)}ms</td>`;
      html += `<td>${result.p95Latency}ms</td>`;
      html += `<td>${result.throughput.toFixed(2)} req/s</td>`;
      html += '</tr>';
    });

    html += '</tbody></table>';
    return html;
  }

  generateRPCInfoTable() {
    let html = '<table class="table"><thead><tr>';
    html += '<th>RPC Name</th><th>Provider</th><th>Tier</th><th>Network</th><th>Region</th><th>Rate Limit</th><th>Features</th><th>Expected Perf</th>';
    html += '</tr></thead><tbody>';

    Object.entries(this.results).forEach(([rpcId, rpcData]) => {
      const config = rpcData.config;
      html += '<tr>';
      html += `<td><strong>${config.name}</strong></td>`;
      html += `<td>${config.provider}</td>`;
      html += `<td><span class="tier-${config.tier}">${config.tier}</span></td>`;
      html += `<td>${config.network}</td>`;
      html += `<td>${config.region}</td>`;
      html += `<td>${config.rateLimit}</td>`;
      html += `<td>${(config.features || []).join(', ')}</td>`;
      html += `<td>${config.metadata?.averageLatency || 'N/A'} / ${config.metadata?.uptime || 'N/A'}</td>`;
      html += '</tr>';
    });

    html += '</tbody></table>';
    return html;
  }

  getUniqueProviders() {
    const providers = new Set();
    Object.values(this.results).forEach(rpcData => {
      providers.add(rpcData.config.provider);
    });
    return Array.from(providers);
  }

  prepareChartData() {
    const languages = [];
    const avgLatencies = [];
    const successRates = [];

    for (const rpcId in this.results) {
      const rpcData = this.results[rpcId];
      for (const language in rpcData.tests) {
        const result = rpcData.tests[language];
        if (result.success !== false) {
          languages.push(`${language} (${rpcData.config.provider})`);
          avgLatencies.push(result.latency ? result.latency.avg : 0);
          successRates.push(result.successRate || 0);
        }
      }
    }

    return { languages, avgLatencies, successRates };
  }

  calculateThroughput(result) {
    const totalRequests = result.totalRequests || 0;
    const executionTime = result.executionTime || 1;
    return (totalRequests / executionTime) * 1000; // req/s
  }

  getLanguageIcon(language) {
    const icons = {
      nodejs: '🟢',
      golang: '🔵',
      rust: '🟠'
    };
    return icons[language] || '⚪';
  }

  sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  log(message) {
    if (this.options.verbose) {
      console.log(chalk.gray(`[${moment().format('HH:mm:ss')}] ${message}`));
    }
  }
}

// CLI Setup
program
  .name('benchmark')
  .description('Advanced RPC performance benchmark runner')
  .version('1.0.0')
  .option('-e, --endpoint <url>', 'Legacy: Single RPC endpoint to test')
  .option('-i, --iterations <number>', 'Number of iterations per language', '100')
  .option('-f, --format <format>', 'Output format (console|json|csv|html)', 'console')
  .option('-o, --output <filename>', 'Output filename (without extension)')
  .option('--endpoints <urls>', 'Legacy: Comma-separated list of endpoints to test')
  .option('--rpcs <ids>', 'Comma-separated list of RPC IDs from config to test')
  .option('--chain <chain>', 'Filter RPCs by chain (e.g., solana, ethereum)', 'solana')
  .option('--provider <provider>', 'Filter RPCs by provider (e.g., "Solana Labs", QuickNode)')
  .option('--tier <tier>', 'Filter RPCs by tier (free, premium)')
  .option('--network <network>', 'Filter RPCs by network (mainnet, devnet, testnet)')
  .option('--list-rpcs', 'List all available RPCs from config')
  .option('--timeout <ms>', 'Timeout per language test in milliseconds', '60000')
  .option('--delay <ms>', 'Delay between language tests in milliseconds', '0')
  .option('--concurrent <number>', 'Concurrent requests (future use)', '1')
  .option('-v, --verbose', 'Verbose logging')
  .action(async (options) => {
    try {
      // Handle --list-rpcs option
      if (options.listRpcs) {
        const rpcManager = new RPCManager();
        const rpcs = rpcManager.getAllRPCs();
        
        console.log(chalk.blue.bold('📋 Available RPCs:\n'));
        
        const table = new Table({
          head: ['ID', 'Name', 'Provider', 'Tier', 'Network', 'Status'],
          style: { head: ['cyan'] }
        });

        Object.entries(rpcs).forEach(([id, rpc]) => {
          table.push([
            id,
            rpc.name,
            rpc.provider,
            rpc.tier,
            rpc.network,
            rpc.status
          ]);
        });

        console.log(table.toString());
        console.log(chalk.gray('\nUsage examples:'));
        console.log(chalk.gray('  node benchmark.js --rpcs solana-mainnet-official,quicknode-mainnet'));
        console.log(chalk.gray('  node benchmark.js --provider "Solana Labs" --tier free'));
        console.log(chalk.gray('  node benchmark.js --chain solana --network mainnet'));
        return;
      }

      const endpoints = options.endpoints ? options.endpoints.split(',') : [];
      const rpcs = options.rpcs ? options.rpcs.split(',') : [];
      
      const runner = new BenchmarkRunner({
        endpoint: options.endpoint,
        iterations: parseInt(options.iterations),
        format: options.format,
        output: options.output,
        timeout: parseInt(options.timeout),
        delay: parseInt(options.delay),
        endpoints,
        rpcs,
        chain: options.chain,
        provider: options.provider,
        tier: options.tier,
        network: options.network,
        concurrent: parseInt(options.concurrent),
        verbose: options.verbose
      });

      await runner.runFullBenchmark();
    } catch (error) {
      console.error(chalk.red('❌ Benchmark failed:'), error.message);
      process.exit(1);
    }
  });

// Export for programmatic use
module.exports = { BenchmarkRunner };

// CLI execution
if (require.main === module) {
  program.parse();
}