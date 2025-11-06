/**
 * Example: Using CodeGraph Native Addon
 *
 * Direct function calls - no process spawning, no HTTP!
 */

import {
  initialize,
  getVersion,
  // Transactions
  beginTransaction,
  commitTransaction,
  rollbackTransaction,
  getTransactionStats,
  // Versions
  createVersion,
  listVersions,
  getVersion as getVersionDetails,
  tagVersion,
  compareVersions,
  // Branches
  createBranch,
  listBranches,
  getBranch,
  deleteBranch,
  mergeBranches,
} from './index';

async function main() {
  console.log('🚀 CodeGraph Native Addon Example\n');

  // Optional: Initialize (happens automatically on first call)
  await initialize();
  console.log(`✅ CodeGraph version: ${getVersion()}\n`);

  try {
    // ========================================
    // 1. Transaction Management
    // ========================================
    console.log('💾 Transaction Management');
    const tx = await beginTransaction('serializable');
    console.log(`   Started: ${tx.transactionId}`);
    console.log(`   Isolation: ${tx.isolationLevel}\n`);

    // Simulate some work
    await new Promise(resolve => setTimeout(resolve, 100));

    const committed = await commitTransaction(tx.transactionId);
    console.log(`   Committed: ${committed.status}\n`);

    // ========================================
    // 2. Version Management
    // ========================================
    console.log('📦 Version Management');
    const version = await createVersion({
      name: 'v1.0.0',
      description: 'Initial release with core features',
      author: 'native-addon@example.com',
      parents: undefined,  // Or: ['parent-id-1', 'parent-id-2']
    });
    console.log(`   Created: ${version.versionId}`);
    console.log(`   Name: ${version.name}`);
    console.log(`   Author: ${version.author}\n`);

    // List versions
    const versions = await listVersions(10);
    console.log(`   Found ${versions.length} version(s):`);
    versions.forEach(v => {
      console.log(`     - ${v.name} (${v.versionId.substring(0, 8)}...)`);
    });
    console.log();

    // Tag version
    await tagVersion(version.versionId, 'stable');
    console.log(`   Tagged ${version.versionId} as 'stable'\n`);

    // ========================================
    // 3. Branch Management
    // ========================================
    console.log('🌿 Branch Management');
    const branch = await createBranch({
      name: 'feature/authentication',
      from: version.versionId,
      author: 'native-addon@example.com',
      description: 'Add user authentication',
    });
    console.log(`   Created: ${branch.name}`);
    console.log(`   Head: ${branch.head}\n`);

    // List branches
    const branches = await listBranches();
    console.log(`   Found ${branches.length} branch(es):`);
    branches.forEach(b => {
      console.log(`     - ${b.name} -> ${b.head.substring(0, 8)}...`);
    });
    console.log();

    // ========================================
    // 4. Statistics
    // ========================================
    console.log('📊 Transaction Statistics');
    const stats = await getTransactionStats();
    console.log(`   Active: ${stats.activeTransactions}`);
    console.log(`   Committed: ${stats.committedTransactions}`);
    console.log(`   Aborted: ${stats.abortedTransactions}`);
    console.log(`   Avg commit time: ${stats.averageCommitTimeMs.toFixed(2)}ms\n`);

    // ========================================
    // 5. Performance Demo
    // ========================================
    console.log('⚡ Performance Test (1000 operations)');
    const startTime = Date.now();

    for (let i = 0; i < 1000; i++) {
      await getTransactionStats();
    }

    const endTime = Date.now();
    const totalTime = endTime - startTime;
    const avgTime = totalTime / 1000;

    console.log(`   Total time: ${totalTime}ms`);
    console.log(`   Average per call: ${avgTime.toFixed(2)}ms`);
    console.log(`   Calls per second: ${(1000 / (totalTime / 1000)).toFixed(0)}\n`);

    console.log('✨ All operations completed successfully!');
    console.log('\n💡 Notice: No process spawning, no HTTP - just direct function calls!');

  } catch (error) {
    console.error('❌ Error:', error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

// Run the example
main().catch(console.error);
