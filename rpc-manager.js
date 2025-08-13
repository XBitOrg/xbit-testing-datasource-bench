#!/usr/bin/env node

const { Command } = require('commander');
const chalk = require('chalk');
const Table = require('cli-table3');
const fs = require('fs-extra');
const { RPCManager } = require('./shared/rpc-manager');

const program = new Command();
const rpcManager = new RPCManager();

// List RPCs command
program
  .command('list')
  .alias('ls')
  .description('List all RPC endpoints')
  .option('--chain <chain>', 'Filter by chain')
  .option('--provider <provider>', 'Filter by provider')
  .option('--tier <tier>', 'Filter by tier')
  .option('--network <network>', 'Filter by network')
  .option('--status <status>', 'Filter by status')
  .option('--format <format>', 'Output format (table|json|csv)', 'table')
  .action((options) => {
    const filter = {};
    if (options.chain) filter.chain = options.chain;
    if (options.provider) filter.provider = options.provider;
    if (options.tier) filter.tier = options.tier;
    if (options.network) filter.network = options.network;
    if (options.status) filter.status = options.status;

    const rpcs = rpcManager.getRPCs(filter);
    
    if (Object.keys(rpcs).length === 0) {
      console.log(chalk.yellow('No RPCs found matching the criteria.'));
      return;
    }

    switch (options.format) {
      case 'json':
        console.log(JSON.stringify(rpcs, null, 2));
        break;
      case 'csv':
        console.log(rpcManager.exportRPCs('csv', filter));
        break;
      default:
        displayRPCTable(rpcs);
    }
  });

// Show detailed RPC info
program
  .command('show <id>')
  .description('Show detailed information about an RPC endpoint')
  .action((id) => {
    const rpc = rpcManager.getRPC(id);
    
    if (!rpc) {
      console.error(chalk.red(`RPC with ID '${id}' not found.`));
      process.exit(1);
    }

    console.log(chalk.blue.bold(`\n📡 RPC Details: ${rpc.name}`));
    console.log(chalk.gray('─'.repeat(50)));
    
    const details = [
      ['ID', rpc.id],
      ['Name', rpc.name],
      ['Provider', rpc.provider],
      ['URL', rpc.url],
      ['Chain', rpc.chain],
      ['Network', rpc.network],
      ['Tier', rpc.tier],
      ['Region', rpc.region],
      ['Rate Limit', rpc.rateLimit],
      ['Status', rpc.status],
      ['Features', (rpc.features || []).join(', ')],
      ['Description', rpc.description || 'N/A']
    ];

    details.forEach(([key, value]) => {
      console.log(`${chalk.cyan(key.padEnd(12))}: ${value}`);
    });

    if (rpc.metadata && Object.keys(rpc.metadata).length > 0) {
      console.log(chalk.blue.bold('\n🔧 Metadata:'));
      Object.entries(rpc.metadata).forEach(([key, value]) => {
        console.log(`${chalk.cyan(key.padEnd(12))}: ${JSON.stringify(value)}`);
      });
    }

    if (rpc.methods && rpc.methods.length > 0) {
      console.log(chalk.blue.bold('\n⚙️  Supported Methods:'));
      rpc.methods.forEach(method => {
        console.log(`  • ${method}`);
      });
    }

    if (rpc.documentation) {
      console.log(chalk.blue.bold('\n📚 Documentation:'));
      console.log(`  ${rpc.documentation}`);
    }
  });

// Add new RPC
program
  .command('add')
  .description('Add a new RPC endpoint')
  .option('--id <id>', 'Unique RPC ID (required)')
  .option('--name <name>', 'Display name (required)')
  .option('--url <url>', 'RPC endpoint URL (required)')
  .option('--chain <chain>', 'Blockchain chain (required)')
  .option('--network <network>', 'Network type (required)')
  .option('--provider <provider>', 'Service provider (required)')
  .option('--tier <tier>', 'Service tier (free|premium)', 'free')
  .option('--region <region>', 'Geographic region', 'Unknown')
  .option('--rate-limit <limit>', 'Rate limit description', 'Unknown')
  .option('--features <features>', 'Comma-separated features list', 'http')
  .option('--status <status>', 'Status (active|inactive)', 'active')
  .option('--description <desc>', 'Description')
  .option('--docs <url>', 'Documentation URL')
  .action((options) => {
    const required = ['id', 'name', 'url', 'chain', 'network', 'provider'];
    const missing = required.filter(field => !options[field]);
    
    if (missing.length > 0) {
      console.error(chalk.red(`Missing required fields: ${missing.join(', ')}`));
      console.log(chalk.yellow('\nExample:'));
      console.log('node rpc-manager.js add --id my-rpc --name "My RPC" --url https://my-rpc.com --chain solana --network mainnet --provider "My Provider"');
      process.exit(1);
    }

    try {
      const rpcData = {
        id: options.id,
        name: options.name,
        url: options.url,
        chain: options.chain,
        network: options.network,
        provider: options.provider,
        tier: options.tier,
        region: options.region,
        rateLimit: options.rateLimit,
        features: options.features.split(',').map(f => f.trim()),
        status: options.status,
        description: options.description || '',
        documentation: options.docs || ''
      };

      const validation = rpcManager.validateRPC(rpcData);
      if (!validation.valid) {
        console.error(chalk.red('Validation failed:'));
        validation.errors.forEach(error => {
          console.error(`  • ${error}`);
        });
        process.exit(1);
      }

      const newRPC = rpcManager.addRPC(rpcData);
      console.log(chalk.green(`✅ Successfully added RPC: ${newRPC.name} (${newRPC.id})`));
      
    } catch (error) {
      console.error(chalk.red(`Failed to add RPC: ${error.message}`));
      process.exit(1);
    }
  });

// Update existing RPC
program
  .command('update <id>')
  .description('Update an existing RPC endpoint')
  .option('--name <name>', 'Display name')
  .option('--url <url>', 'RPC endpoint URL')
  .option('--provider <provider>', 'Service provider')
  .option('--tier <tier>', 'Service tier')
  .option('--region <region>', 'Geographic region')
  .option('--rate-limit <limit>', 'Rate limit description')
  .option('--features <features>', 'Comma-separated features list')
  .option('--status <status>', 'Status (active|inactive)')
  .option('--description <desc>', 'Description')
  .option('--docs <url>', 'Documentation URL')
  .action((id, options) => {
    try {
      const updates = {};
      
      Object.keys(options).forEach(key => {
        if (options[key] !== undefined) {
          if (key === 'features') {
            updates[key] = options[key].split(',').map(f => f.trim());
          } else if (key === 'rateLimit') {
            updates.rateLimit = options[key];
          } else if (key === 'docs') {
            updates.documentation = options[key];
          } else {
            updates[key] = options[key];
          }
        }
      });

      if (Object.keys(updates).length === 0) {
        console.error(chalk.red('No updates provided.'));
        process.exit(1);
      }

      const updatedRPC = rpcManager.updateRPC(id, updates);
      console.log(chalk.green(`✅ Successfully updated RPC: ${updatedRPC.name} (${updatedRPC.id})`));
      
    } catch (error) {
      console.error(chalk.red(`Failed to update RPC: ${error.message}`));
      process.exit(1);
    }
  });

// Delete RPC
program
  .command('delete <id>')
  .alias('rm')
  .description('Delete an RPC endpoint')
  .option('--force', 'Skip confirmation prompt')
  .action(async (id, options) => {
    try {
      const rpc = rpcManager.getRPC(id);
      if (!rpc) {
        console.error(chalk.red(`RPC with ID '${id}' not found.`));
        process.exit(1);
      }

      if (!options.force) {
        const readline = require('readline');
        const rl = readline.createInterface({
          input: process.stdin,
          output: process.stdout
        });

        const answer = await new Promise(resolve => {
          rl.question(chalk.yellow(`Are you sure you want to delete '${rpc.name}' (${id})? [y/N]: `), resolve);
        });

        rl.close();

        if (answer.toLowerCase() !== 'y' && answer.toLowerCase() !== 'yes') {
          console.log('Cancelled.');
          return;
        }
      }

      rpcManager.deleteRPC(id);
      console.log(chalk.green(`✅ Successfully deleted RPC: ${rpc.name} (${id})`));
      
    } catch (error) {
      console.error(chalk.red(`Failed to delete RPC: ${error.message}`));
      process.exit(1);
    }
  });

// Statistics
program
  .command('stats')
  .description('Show RPC statistics')
  .action(() => {
    const stats = rpcManager.getStatistics();
    
    console.log(chalk.blue.bold('\n📊 RPC Statistics'));
    console.log(chalk.gray('─'.repeat(30)));
    console.log(`${chalk.cyan('Total RPCs')}: ${stats.total}`);
    
    if (Object.keys(stats.byChain).length > 0) {
      console.log(chalk.blue.bold('\nBy Chain:'));
      Object.entries(stats.byChain).forEach(([chain, count]) => {
        console.log(`  ${chain}: ${count}`);
      });
    }

    if (Object.keys(stats.byProvider).length > 0) {
      console.log(chalk.blue.bold('\nBy Provider:'));
      Object.entries(stats.byProvider).forEach(([provider, count]) => {
        console.log(`  ${provider}: ${count}`);
      });
    }

    if (Object.keys(stats.byTier).length > 0) {
      console.log(chalk.blue.bold('\nBy Tier:'));
      Object.entries(stats.byTier).forEach(([tier, count]) => {
        console.log(`  ${tier}: ${count}`);
      });
    }

    if (Object.keys(stats.byNetwork).length > 0) {
      console.log(chalk.blue.bold('\nBy Network:'));
      Object.entries(stats.byNetwork).forEach(([network, count]) => {
        console.log(`  ${network}: ${count}`);
      });
    }

    if (Object.keys(stats.byStatus).length > 0) {
      console.log(chalk.blue.bold('\nBy Status:'));
      Object.entries(stats.byStatus).forEach(([status, count]) => {
        console.log(`  ${status}: ${count}`);
      });
    }
  });

// Export RPCs
program
  .command('export')
  .description('Export RPCs to various formats')
  .option('--format <format>', 'Export format (json|csv|markdown)', 'json')
  .option('--output <file>', 'Output file')
  .option('--chain <chain>', 'Filter by chain')
  .option('--provider <provider>', 'Filter by provider')
  .option('--tier <tier>', 'Filter by tier')
  .action(async (options) => {
    try {
      const filter = {};
      if (options.chain) filter.chain = options.chain;
      if (options.provider) filter.provider = options.provider;
      if (options.tier) filter.tier = options.tier;

      const exportData = rpcManager.exportRPCs(options.format, filter);
      
      if (options.output) {
        await fs.writeFile(options.output, exportData);
        console.log(chalk.green(`✅ Exported to: ${options.output}`));
      } else {
        console.log(exportData);
      }
      
    } catch (error) {
      console.error(chalk.red(`Export failed: ${error.message}`));
      process.exit(1);
    }
  });

// Helper function to display RPC table
function displayRPCTable(rpcs) {
  const table = new Table({
    head: ['ID', 'Name', 'Provider', 'Chain', 'Network', 'Tier', 'Status'],
    style: { head: ['cyan'] }
  });

  Object.entries(rpcs).forEach(([id, rpc]) => {
    const statusColor = rpc.status === 'active' ? chalk.green : chalk.red;
    const tierColor = rpc.tier === 'premium' ? chalk.yellow : chalk.white;
    
    table.push([
      id,
      rpc.name.length > 25 ? rpc.name.substring(0, 22) + '...' : rpc.name,
      rpc.provider,
      rpc.chain,
      rpc.network,
      tierColor(rpc.tier),
      statusColor(rpc.status)
    ]);
  });

  console.log(table.toString());
  console.log(chalk.gray(`\nTotal: ${Object.keys(rpcs).length} RPCs`));
}

// Main CLI setup
program
  .name('rpc-manager')
  .description('RPC endpoint management tool')
  .version('1.0.0');

// If no command provided, show help
if (process.argv.length <= 2) {
  program.help();
}

program.parse();

module.exports = { rpcManager };