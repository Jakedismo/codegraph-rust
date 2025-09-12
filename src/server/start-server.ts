#!/usr/bin/env node

import { EnhancedMCPServer } from './enhanced-mcp-server.js';
import { Command } from 'commander';
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Parse command line arguments
const program = new Command();

program
  .name('codegraph-mcp-server')
  .description('Enhanced MCP server with CodeGraph integration')
  .version('1.0.0')
  .option('-t, --transport <type>', 'Transport type: stdio, http, or both', 'both')
  .option('-p, --port <number>', 'HTTP port', '3000')
  .option('--api-url <url>', 'CodeGraph API URL', 'http://localhost:3030')
  .option('--no-cache', 'Disable caching')
  .option('--no-metrics', 'Disable metrics')
  .option('--cors <origins>', 'CORS origins (comma-separated)', '*')
  .option('--allowed-hosts <hosts>', 'Allowed hosts for DNS rebinding protection', '127.0.0.1,localhost')
  .option('--no-dns-protection', 'Disable DNS rebinding protection')
  .parse();

const options = program.opts();

// Parse CORS origins
const corsOrigins = options.cors === '*' 
  ? ['*'] 
  : options.cors.split(',').map((s: string) => s.trim());

// Parse allowed hosts
const allowedHosts = options.allowedHosts
  .split(',')
  .map((s: string) => s.trim());

// Create server configuration
const config = {
  name: 'codegraph-mcp-server',
  version: '1.0.0',
  transport: options.transport as 'stdio' | 'http' | 'both',
  httpPort: parseInt(options.port),
  codeGraphApiUrl: options.apiUrl,
  enableCache: options.cache,
  enableMetrics: options.metrics,
  corsOrigins,
  allowedHosts,
  enableDnsRebindingProtection: options.dnsProtection
};

// Create and start server
const server = new EnhancedMCPServer(config);

// Setup event listeners
server.on('server:started', (info) => {
  console.error('✅ Server started successfully');
  console.error(`   Transport: ${info.transport}`);
  if (info.httpPort) {
    console.error(`   HTTP Port: ${info.httpPort}`);
    console.error(`   Health Check: http://localhost:${info.httpPort}/health`);
    if (options.metrics) {
      console.error(`   Metrics: http://localhost:${info.httpPort}/metrics`);
    }
  }
});

server.on('server:error', (error) => {
  console.error('❌ Server error:', error);
});

server.on('transport:connected', (info) => {
  console.error(`📡 Transport connected: ${info.type}`);
  if (info.port) {
    console.error(`   Port: ${info.port}`);
  }
});

server.on('transport:error', (info) => {
  console.error(`❌ Transport error (${info.type}):`, info.error);
});

server.on('session:created', (info) => {
  console.error(`🔗 Session created: ${info.sessionId}`);
});

server.on('session:closed', (info) => {
  console.error(`🔚 Session closed: ${info.sessionId}`);
});

server.on('codegraph:error', (error) => {
  console.error('⚠️  CodeGraph error:', error);
});

// Handle process signals
process.on('SIGINT', async () => {
  console.error('\n🛑 Shutting down server...');
  try {
    await server.stop();
    console.error('✅ Server stopped');
    process.exit(0);
  } catch (error) {
    console.error('❌ Error stopping server:', error);
    process.exit(1);
  }
});

process.on('SIGTERM', async () => {
  console.error('\n🛑 Shutting down server...');
  try {
    await server.stop();
    console.error('✅ Server stopped');
    process.exit(0);
  } catch (error) {
    console.error('❌ Error stopping server:', error);
    process.exit(1);
  }
});

// Start the server
server.start().catch((error) => {
  console.error('❌ Failed to start server:', error);
  process.exit(1);
});

// Log startup configuration
console.error('🚀 Starting CodeGraph MCP Server');
console.error('📋 Configuration:');
console.error(`   Transport: ${options.transport}`);
console.error(`   CodeGraph API: ${options.apiUrl}`);
console.error(`   Cache: ${options.cache ? 'enabled' : 'disabled'}`);
console.error(`   Metrics: ${options.metrics ? 'enabled' : 'disabled'}`);
console.error(`   CORS Origins: ${corsOrigins.join(', ')}`);
console.error(`   DNS Protection: ${options.dnsProtection ? 'enabled' : 'disabled'}`);
if (options.dnsProtection) {
  console.error(`   Allowed Hosts: ${allowedHosts.join(', ')}`);
}
console.error('');