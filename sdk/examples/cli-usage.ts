/**
 * Example: Using CodeGraph CLI from TypeScript
 *
 * This example demonstrates how to integrate the Rust CLI binary
 * into your TypeScript application.
 */

import CodeGraph from '../codegraph-cli-wrapper';

async function main() {
  // Initialize the client
  const cg = new CodeGraph({
    binaryPath: 'codegraph',  // Will search PATH
    verbose: true,
    timeout: 60000  // 60 second timeout
  });

  console.log('🚀 CodeGraph CLI Integration Example\n');

  // Check if binary is available
  const available = await cg.checkBinary();
  if (!available) {
    console.error('❌ codegraph binary not found in PATH');
    console.error('   Build it with: cargo build --release --bin codegraph');
    process.exit(1);
  }

  console.log('✅ codegraph binary found\n');

  try {
    // ========================================
    // 1. Version Management
    // ========================================
    console.log('📦 Creating a new version...');
    const version = await cg.createVersion({
      name: 'v1.0.0',
      description: 'Initial release with core features',
      author: 'cli-user@example.com',
      parents: []
    });
    console.log(`   Created: ${version.version_id}`);
    console.log(`   Name: ${version.name}`);
    console.log(`   Author: ${version.author}\n`);

    // ========================================
    // 2. Branch Management
    // ========================================
    console.log('🌿 Creating a new branch...');
    const branch = await cg.createBranch({
      name: 'feature/authentication',
      from: version.version_id,
      author: 'cli-user@example.com',
      description: 'Add user authentication'
    });
    console.log(`   Branch: ${branch.name}`);
    console.log(`   Head: ${branch.head}\n`);

    // ========================================
    // 3. List Operations
    // ========================================
    console.log('📋 Listing versions...');
    const versions = await cg.listVersions(10);
    console.log(`   Found ${versions.length} version(s):`);
    versions.forEach(v => {
      console.log(`     - ${v.name} (${v.version_id.substring(0, 8)}...)`);
    });
    console.log();

    console.log('📋 Listing branches...');
    const branches = await cg.listBranches();
    console.log(`   Found ${branches.length} branch(es):`);
    branches.forEach(b => {
      console.log(`     - ${b.name} -> ${b.head.substring(0, 8)}...`);
    });
    console.log();

    // ========================================
    // 4. Transaction Management
    // ========================================
    console.log('💾 Working with transactions...');
    const tx = await cg.beginTransaction('serializable');
    console.log(`   Transaction started: ${tx.transaction_id}`);
    console.log(`   Isolation level: ${tx.isolation_level}`);

    // Simulate some work
    console.log('   Performing operations...');
    await new Promise(resolve => setTimeout(resolve, 1000));

    const committed = await cg.commitTransaction(tx.transaction_id);
    console.log(`   Transaction committed: ${committed.status}\n`);

    // ========================================
    // 5. Transaction Statistics
    // ========================================
    console.log('📊 Transaction statistics...');
    const stats = await cg.getTransactionStats();
    console.log(`   Active: ${stats.active_transactions}`);
    console.log(`   Committed: ${stats.committed_transactions}`);
    console.log(`   Aborted: ${stats.aborted_transactions}`);
    console.log(`   Avg commit time: ${stats.average_commit_time_ms.toFixed(2)}ms\n`);

    // ========================================
    // 6. Tagging
    // ========================================
    console.log('🏷️  Tagging version...');
    const tag = await cg.tagVersion({
      versionId: version.version_id,
      tag: 'stable',
      author: 'cli-user@example.com',
      message: 'Marking as stable release'
    });
    console.log(`   Tagged ${tag.version_id} as '${tag.tag}'\n`);

    // ========================================
    // 7. System Status
    // ========================================
    console.log('ℹ️  System status...');
    const status = await cg.status();
    console.log(`   Storage: ${status.storage_path}`);
    console.log(`   Status: ${status.status}`);
    console.log(`   Message: ${status.message}\n`);

    console.log('✨ All operations completed successfully!');

  } catch (error) {
    console.error('❌ Error:', error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

// Run the example
main().catch(console.error);
