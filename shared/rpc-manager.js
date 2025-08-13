const fs = require('fs-extra');
const path = require('path');

class RPCManager {
  constructor(configPath = './shared/config.json') {
    this.configPath = configPath;
    this.config = null;
    this.loadConfig();
  }

  loadConfig() {
    try {
      this.config = fs.readJsonSync(this.configPath);
    } catch (error) {
      console.error('Failed to load config:', error.message);
      this.config = { rpcs: {}, chains: {} };
    }
  }

  saveConfig() {
    try {
      fs.writeJsonSync(this.configPath, this.config, { spaces: 2 });
    } catch (error) {
      console.error('Failed to save config:', error.message);
    }
  }

  // Get all RPCs
  getAllRPCs() {
    return this.config.rpcs || {};
  }

  // Get RPC by ID
  getRPC(id) {
    return this.config.rpcs[id] || null;
  }

  // Get RPCs by filter
  getRPCs(filter = {}) {
    const rpcs = this.getAllRPCs();
    const filtered = {};

    Object.entries(rpcs).forEach(([id, rpc]) => {
      let matches = true;

      // Filter by chain
      if (filter.chain && rpc.chain !== filter.chain) {
        matches = false;
      }

      // Filter by network
      if (filter.network && rpc.network !== filter.network) {
        matches = false;
      }

      // Filter by provider
      if (filter.provider && rpc.provider !== filter.provider) {
        matches = false;
      }

      // Filter by tier
      if (filter.tier && rpc.tier !== filter.tier) {
        matches = false;
      }

      // Filter by status
      if (filter.status && rpc.status !== filter.status) {
        matches = false;
      }

      // Filter by features
      if (filter.features && Array.isArray(filter.features)) {
        const hasAllFeatures = filter.features.every(feature => 
          rpc.features && rpc.features.includes(feature)
        );
        if (!hasAllFeatures) {
          matches = false;
        }
      }

      if (matches) {
        filtered[id] = rpc;
      }
    });

    return filtered;
  }

  // Get default RPCs from config
  getDefaultRPCs() {
    const defaultIds = this.config.benchmark?.defaultRpcs || [];
    const rpcs = {};
    
    defaultIds.forEach(id => {
      const rpc = this.getRPC(id);
      if (rpc) {
        rpcs[id] = rpc;
      }
    });

    return rpcs;
  }

  // Add new RPC
  addRPC(rpcData) {
    if (!rpcData.id) {
      throw new Error('RPC ID is required');
    }

    if (this.config.rpcs[rpcData.id]) {
      throw new Error(`RPC with ID '${rpcData.id}' already exists`);
    }

    // Validate required fields
    const required = ['name', 'url', 'chain', 'network', 'provider'];
    for (const field of required) {
      if (!rpcData[field]) {
        throw new Error(`Field '${field}' is required`);
      }
    }

    // Set defaults
    const rpc = {
      id: rpcData.id,
      name: rpcData.name,
      url: rpcData.url,
      chain: rpcData.chain,
      network: rpcData.network,
      provider: rpcData.provider,
      region: rpcData.region || 'Unknown',
      tier: rpcData.tier || 'free',
      rateLimit: rpcData.rateLimit || 'Unknown',
      features: rpcData.features || ['http'],
      status: rpcData.status || 'active',
      description: rpcData.description || '',
      documentation: rpcData.documentation || '',
      methods: rpcData.methods || this.getDefaultMethodsForChain(rpcData.chain),
      metadata: rpcData.metadata || {}
    };

    this.config.rpcs[rpcData.id] = rpc;
    this.saveConfig();
    
    return rpc;
  }

  // Update existing RPC
  updateRPC(id, updates) {
    if (!this.config.rpcs[id]) {
      throw new Error(`RPC with ID '${id}' not found`);
    }

    this.config.rpcs[id] = {
      ...this.config.rpcs[id],
      ...updates,
      id // Ensure ID doesn't change
    };

    this.saveConfig();
    return this.config.rpcs[id];
  }

  // Delete RPC
  deleteRPC(id) {
    if (!this.config.rpcs[id]) {
      throw new Error(`RPC with ID '${id}' not found`);
    }

    const deleted = this.config.rpcs[id];
    delete this.config.rpcs[id];
    this.saveConfig();
    
    return deleted;
  }

  // Get available chains
  getChains() {
    return this.config.chains || {};
  }

  // Get default methods for a chain
  getDefaultMethodsForChain(chain) {
    const chainConfig = this.config.chains[chain];
    return chainConfig ? chainConfig.defaultMethods : [];
  }

  // Get test methods for benchmarking
  getTestMethods(chain) {
    return this.config.benchmark?.testMethods?.[chain] || this.getDefaultMethodsForChain(chain);
  }

  // Get RPCs grouped by criteria
  getGroupedRPCs(groupBy = 'provider') {
    const rpcs = this.getAllRPCs();
    const grouped = {};

    Object.values(rpcs).forEach(rpc => {
      const key = rpc[groupBy] || 'Unknown';
      if (!grouped[key]) {
        grouped[key] = [];
      }
      grouped[key].push(rpc);
    });

    return grouped;
  }

  // Get RPC statistics
  getStatistics() {
    const rpcs = Object.values(this.getAllRPCs());
    
    const stats = {
      total: rpcs.length,
      byChain: {},
      byProvider: {},
      byTier: {},
      byNetwork: {},
      byStatus: {}
    };

    rpcs.forEach(rpc => {
      // Count by chain
      stats.byChain[rpc.chain] = (stats.byChain[rpc.chain] || 0) + 1;
      
      // Count by provider
      stats.byProvider[rpc.provider] = (stats.byProvider[rpc.provider] || 0) + 1;
      
      // Count by tier
      stats.byTier[rpc.tier] = (stats.byTier[rpc.tier] || 0) + 1;
      
      // Count by network
      stats.byNetwork[rpc.network] = (stats.byNetwork[rpc.network] || 0) + 1;
      
      // Count by status
      stats.byStatus[rpc.status] = (stats.byStatus[rpc.status] || 0) + 1;
    });

    return stats;
  }

  // Validate RPC configuration
  validateRPC(rpcData) {
    const errors = [];
    
    // Required fields
    const required = ['id', 'name', 'url', 'chain', 'network', 'provider'];
    required.forEach(field => {
      if (!rpcData[field]) {
        errors.push(`Field '${field}' is required`);
      }
    });

    // URL validation
    if (rpcData.url && !this.isValidUrl(rpcData.url)) {
      errors.push('Invalid URL format');
    }

    // Chain validation
    if (rpcData.chain && !this.config.chains[rpcData.chain]) {
      errors.push(`Unsupported chain: ${rpcData.chain}`);
    }

    return {
      valid: errors.length === 0,
      errors
    };
  }

  // URL validation helper
  isValidUrl(string) {
    try {
      new URL(string);
      return true;
    } catch (_) {
      return false;
    }
  }

  // Export RPCs to various formats
  exportRPCs(format = 'json', filter = {}) {
    const rpcs = this.getRPCs(filter);
    
    switch (format.toLowerCase()) {
      case 'json':
        return JSON.stringify(rpcs, null, 2);
        
      case 'csv':
        const headers = ['ID', 'Name', 'URL', 'Chain', 'Network', 'Provider', 'Tier', 'Rate Limit', 'Status'];
        const rows = Object.values(rpcs).map(rpc => [
          rpc.id,
          rpc.name,
          rpc.url,
          rpc.chain,
          rpc.network,
          rpc.provider,
          rpc.tier,
          rpc.rateLimit,
          rpc.status
        ]);
        
        return [headers, ...rows].map(row => row.join(',')).join('\n');
        
      case 'markdown':
        let md = '# RPC Endpoints\n\n';
        md += '| Name | Provider | Chain | Network | Tier | Status |\n';
        md += '|------|----------|-------|---------|------|--------|\n';
        
        Object.values(rpcs).forEach(rpc => {
          md += `| ${rpc.name} | ${rpc.provider} | ${rpc.chain} | ${rpc.network} | ${rpc.tier} | ${rpc.status} |\n`;
        });
        
        return md;
        
      default:
        throw new Error(`Unsupported export format: ${format}`);
    }
  }
}

module.exports = { RPCManager };