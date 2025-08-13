import { program } from 'commander';

// Command line argument parsing
program
  .name('test-laserstream-connection')
  .description('Test Helius Laserstream connection and troubleshoot issues')
  .option('--api-key <key>', 'Helius API key (or use HELIUS_API_KEY env var)')
  .option('--endpoint <url>', 'Helius endpoint', 'https://api.helius.xyz')
  .parse();

const options = program.opts();

async function main() {
  const apiKey = options.apiKey || process.env.HELIUS_API_KEY;
  
  if (!apiKey) {
    console.error('❌ Error: Helius API key required. Use --api-key or set HELIUS_API_KEY environment variable.');
    process.exit(1);
  }

  console.log('🔧 Testing Helius Laserstream Connection');
  console.log(`API Key: ${apiKey.substring(0, 8)}...`);
  console.log(`Endpoint: ${options.endpoint}`);
  console.log();

  try {
    // Test importing the module
    console.log('📦 Testing Laserstream module import...');
    const { subscribe, CommitmentLevel } = await import('helius-laserstream');
    console.log('✅ Module imported successfully');

    // Test basic connection config
    const config = {
      apiKey: apiKey,
      endpoint: options.endpoint,
    };

    const request = {
      blocks: {
        client: {
          accountInclude: [],
          accountExclude: [],
          accountRequired: [],
          includeTransactions: false,
          includeAccounts: false,
          includeEntries: false,
        }
      },
      commitment: CommitmentLevel.PROCESSED,
      transactions: {},
      accounts: {},
      slots: {},
      transactionsStatus: {},
      blocksMeta: {},
      entry: {},
      accountsDataSlice: [],
    };

    console.log('🔗 Testing connection...');
    
    // Try to connect with a timeout
    const connectionTimeout = setTimeout(() => {
      console.log('⚠️  Connection taking longer than expected...');
    }, 5000);

    try {
      await subscribe(
        config,
        request,
        async (data) => {
          clearTimeout(connectionTimeout);
          console.log('✅ Connection successful!');
          console.log('📊 Received data:', JSON.stringify(data, null, 2));
          process.exit(0);
        },
        async (error) => {
          clearTimeout(connectionTimeout);
          console.error('❌ Laserstream error:', error);
          showTroubleshootingTips();
          process.exit(1);
        }
      );
    } catch (error) {
      clearTimeout(connectionTimeout);
      console.error('❌ Connection failed:', error.message);
      showTroubleshootingTips();
      process.exit(1);
    }

    // If we get here after 10 seconds, it's likely working but no data yet
    setTimeout(() => {
      console.log('✅ Connection appears stable (no errors after 10s)');
      console.log('💡 Try running the full benchmark now');
      process.exit(0);
    }, 10000);

  } catch (error) {
    console.error('❌ Import/setup error:', error.message);
    showTroubleshootingTips();
    process.exit(1);
  }
}

function showTroubleshootingTips() {
  console.log();
  console.log('🔧 Troubleshooting Tips:');
  console.log('1. ✅ Verify API key is correct:');
  console.log('   - Login to https://dashboard.helius.xyz');
  console.log('   - Check your API key has Laserstream access');
  console.log();
  console.log('2. 🌐 Try different endpoints:');
  console.log('   - https://api.helius.xyz (default)');
  console.log('   - https://mainnet.helius-rpc.com');
  console.log();
  console.log('3. 📦 Check dependencies:');
  console.log('   - npm install helius-laserstream');
  console.log('   - Ensure you have the latest version');
  console.log();
  console.log('4. 🎫 Contact Helius Support:');
  console.log('   - Laserstream may require special access');
  console.log('   - Ask for Laserstream beta/premium access');
  console.log();
  console.log('5. 🔄 Alternative approaches:');
  console.log('   - Use WebSocket RPC with blockSubscribe');
  console.log('   - Test with regular HTTP RPC first');
}

main().catch(console.error);