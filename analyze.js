#!/usr/bin/env node

const fs = require('fs-extra');
const path = require('path');
const chalk = require('chalk');
const Table = require('cli-table3');
const moment = require('moment');

class ResultsAnalyzer {
  constructor() {
    this.reportsDir = 'reports';
  }

  async analyzeResults(reportFile = null) {
    try {
      let data;
      
      if (reportFile) {
        // Analyze specific report file
        const filePath = path.join(this.reportsDir, reportFile);
        data = await fs.readJSON(filePath);
      } else {
        // Find and analyze the most recent report
        data = await this.loadLatestReport();
      }

      if (!data) {
        console.log(chalk.red('No report data found'));
        return;
      }

      console.log(chalk.blue.bold('\n📊 Performance Analysis Report\n'));
      
      this.displayOverallStats(data);
      this.displayLanguageRankings(data);
      this.displayEndpointComparison(data);
      this.displayRecommendations(data);
      
    } catch (error) {
      console.error(chalk.red('Analysis failed:'), error.message);
    }
  }

  async loadLatestReport() {
    try {
      const files = await fs.readdir(this.reportsDir);
      const jsonFiles = files.filter(f => f.endsWith('.json'));
      
      if (jsonFiles.length === 0) {
        console.log(chalk.yellow('No JSON reports found in reports/ directory'));
        return null;
      }

      // Sort by modification time, most recent first
      const fileStats = await Promise.all(
        jsonFiles.map(async (file) => {
          const filePath = path.join(this.reportsDir, file);
          const stats = await fs.stat(filePath);
          return { file, mtime: stats.mtime };
        })
      );

      fileStats.sort((a, b) => b.mtime - a.mtime);
      const latestFile = fileStats[0].file;
      
      console.log(chalk.gray(`📄 Analyzing latest report: ${latestFile}`));
      
      const filePath = path.join(this.reportsDir, latestFile);
      return await fs.readJSON(filePath);
      
    } catch (error) {
      console.error('Error loading reports:', error.message);
      return null;
    }
  }

  displayOverallStats(data) {
    const { meta, results } = data;
    
    console.log(chalk.green('📈 Overall Statistics'));
    console.log(`Report Generated: ${moment(meta.timestamp).format('YYYY-MM-DD HH:mm:ss')}`);
    console.log(`Total Duration: ${(meta.duration / 1000).toFixed(2)} seconds`);
    console.log(`Endpoints Tested: ${Object.keys(results).length}`);
    console.log(`Languages Tested: ${this.getUniqueLanguages(results).length}`);
    console.log(`Iterations per Language: ${meta.options.iterations}\n`);
  }

  displayLanguageRankings(data) {
    console.log(chalk.green('🏆 Language Performance Rankings'));
    
    const languageStats = this.aggregateLanguageStats(data.results);
    
    // Create ranking table
    const table = new Table({
      head: ['Rank', 'Language', 'Avg Latency', 'Success Rate', 'Throughput', 'Score'],
      style: { head: ['cyan'] }
    });

    languageStats.forEach((lang, index) => {
      const rank = index + 1;
      const rankIcon = rank === 1 ? '🥇' : rank === 2 ? '🥈' : rank === 3 ? '🥉' : rank.toString();
      
      table.push([
        rank === 1 ? chalk.yellow.bold(rankIcon) : rankIcon,
        rank === 1 ? chalk.green.bold(lang.name) : lang.name,
        `${lang.avgLatency.toFixed(2)}ms`,
        `${lang.successRate.toFixed(1)}%`,
        `${lang.throughput.toFixed(2)} req/s`,
        chalk.cyan(lang.score.toFixed(2))
      ]);
    });

    console.log(table.toString());
    console.log();
  }

  displayEndpointComparison(data) {
    const endpoints = Object.keys(data.results);
    
    if (endpoints.length <= 1) {
      return;
    }

    console.log(chalk.green('🌐 Endpoint Performance Comparison'));
    
    const table = new Table({
      head: ['Endpoint', 'Best Language', 'Avg Latency', 'Success Rate'],
      style: { head: ['cyan'] }
    });

    endpoints.forEach(endpoint => {
      const endpointResults = data.results[endpoint];
      const bestResult = this.findBestLanguageForEndpoint(endpointResults);
      
      if (bestResult) {
        table.push([
          this.truncateUrl(endpoint),
          bestResult.language,
          `${bestResult.avgLatency.toFixed(2)}ms`,
          `${bestResult.successRate.toFixed(1)}%`
        ]);
      }
    });

    console.log(table.toString());
    console.log();
  }

  displayRecommendations(data) {
    console.log(chalk.green('💡 Recommendations'));
    
    const languageStats = this.aggregateLanguageStats(data.results);
    const best = languageStats[0];
    const worst = languageStats[languageStats.length - 1];
    
    console.log(chalk.blue('Performance Insights:'));
    console.log(`• ${chalk.bold(best.name)} shows the best overall performance`);
    console.log(`• Average latency difference: ${(worst.avgLatency - best.avgLatency).toFixed(2)}ms`);
    console.log(`• Success rate range: ${worst.successRate.toFixed(1)}% - ${best.successRate.toFixed(1)}%`);
    
    // Specific recommendations based on performance
    console.log(chalk.blue('\nRecommendations:'));
    
    if (best.avgLatency < 200) {
      console.log(`• ✅ Excellent latency performance with ${best.name}`);
    } else if (best.avgLatency < 500) {
      console.log(`• ⚠️  Consider optimization - latency above 200ms`);
    } else {
      console.log(`• ❌ High latency detected - investigate network or endpoint issues`);
    }
    
    if (best.successRate < 95) {
      console.log(`• ❌ Success rate below 95% - check endpoint reliability`);
    }
    
    languageStats.forEach(lang => {
      if (lang.successRate < 90) {
        console.log(`• ⚠️  ${lang.name} has low success rate (${lang.successRate.toFixed(1)}%)`);
      }
    });
    
    console.log(`• 🚀 For production use, consider: ${best.name}`);
    console.log(`• 📊 For detailed analysis, check the HTML report\n`);
  }

  aggregateLanguageStats(results) {
    const languageData = {};
    
    // Aggregate data across all endpoints
    Object.values(results).forEach(endpointResults => {
      Object.entries(endpointResults).forEach(([language, result]) => {
        if (result.success !== false) {
          if (!languageData[language]) {
            languageData[language] = {
              name: language,
              latencies: [],
              successRates: [],
              throughputs: [],
              totalRequests: 0,
              successfulRequests: 0
            };
          }
          
          const data = languageData[language];
          data.latencies.push(result.latency ? result.latency.avg : 0);
          data.successRates.push(result.successRate || 0);
          data.throughputs.push(this.calculateThroughput(result));
          data.totalRequests += result.totalRequests || 0;
          data.successfulRequests += result.successfulRequests || 0;
        }
      });
    });
    
    // Calculate averages and scores
    const languageStats = Object.values(languageData).map(data => {
      const avgLatency = data.latencies.reduce((a, b) => a + b, 0) / data.latencies.length;
      const successRate = data.successRates.reduce((a, b) => a + b, 0) / data.successRates.length;
      const throughput = data.throughputs.reduce((a, b) => a + b, 0) / data.throughputs.length;
      
      // Calculate composite score (lower latency and higher success rate = better score)
      const score = (1000 / avgLatency) * (successRate / 100) * throughput;
      
      return {
        name: data.name,
        avgLatency,
        successRate,
        throughput,
        score,
        totalRequests: data.totalRequests,
        successfulRequests: data.successfulRequests
      };
    });
    
    // Sort by score (descending - higher is better)
    return languageStats.sort((a, b) => b.score - a.score);
  }

  findBestLanguageForEndpoint(endpointResults) {
    let bestResult = null;
    let bestScore = -1;
    
    Object.entries(endpointResults).forEach(([language, result]) => {
      if (result.success !== false) {
        const avgLatency = result.latency ? result.latency.avg : Infinity;
        const successRate = result.successRate || 0;
        const score = (1000 / avgLatency) * (successRate / 100);
        
        if (score > bestScore) {
          bestScore = score;
          bestResult = {
            language,
            avgLatency,
            successRate,
            score
          };
        }
      }
    });
    
    return bestResult;
  }

  calculateThroughput(result) {
    const totalRequests = result.totalRequests || 0;
    const executionTime = result.executionTime || 1;
    return (totalRequests / executionTime) * 1000; // req/s
  }

  getUniqueLanguages(results) {
    const languages = new Set();
    Object.values(results).forEach(endpointResults => {
      Object.keys(endpointResults).forEach(language => {
        languages.add(language);
      });
    });
    return Array.from(languages);
  }

  truncateUrl(url) {
    if (url.length > 40) {
      return url.substring(0, 37) + '...';
    }
    return url;
  }

  async listReports() {
    try {
      const files = await fs.readdir(this.reportsDir);
      const reportFiles = files.filter(f => f.endsWith('.json') || f.endsWith('.csv') || f.endsWith('.html'));
      
      if (reportFiles.length === 0) {
        console.log(chalk.yellow('No reports found in reports/ directory'));
        return;
      }

      console.log(chalk.blue.bold('📁 Available Reports:\n'));
      
      const table = new Table({
        head: ['Filename', 'Type', 'Size', 'Modified'],
        style: { head: ['cyan'] }
      });

      for (const file of reportFiles) {
        const filePath = path.join(this.reportsDir, file);
        const stats = await fs.stat(filePath);
        const ext = path.extname(file);
        const type = ext === '.json' ? '📄 JSON' : ext === '.csv' ? '📊 CSV' : '🌐 HTML';
        
        table.push([
          file,
          type,
          this.formatFileSize(stats.size),
          moment(stats.mtime).format('YYYY-MM-DD HH:mm')
        ]);
      }

      console.log(table.toString());
      console.log(chalk.gray(`\nUse: node analyze.js <filename> to analyze a specific report`));
      
    } catch (error) {
      console.error('Error listing reports:', error.message);
    }
  }

  formatFileSize(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }
}

// CLI execution
if (require.main === module) {
  const analyzer = new ResultsAnalyzer();
  const reportFile = process.argv[2];
  
  if (reportFile === '--list' || reportFile === '-l') {
    analyzer.listReports();
  } else {
    analyzer.analyzeResults(reportFile);
  }
}

module.exports = { ResultsAnalyzer };