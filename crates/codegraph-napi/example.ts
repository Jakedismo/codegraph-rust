/**
 * Example: Using the CodeGraph Native Addon for semantic + graph analysis.
 *
 * This script runs purely inside the current Node.js process:
 * no CLI spawning, no HTTP server, just direct native calls.
 */

import {
  getAddonVersion,
  getCloudConfig,
  initialize,
  searchSimilarFunctions,
  semanticSearch,
} from './index';

async function main() {
  console.log('🚀 CodeGraph Native Addon Example\n');

  await initialize();
  console.log(`✅ Native addon loaded (version ${getAddonVersion()})`);

  const cloudConfig = await getCloudConfig();
  console.log(
    `☁️  Cloud ready: ${cloudConfig.surrealdbEnabled ? 'SurrealDB detected' : 'local-only mode'}\n`,
  );

  // ------------------------------------------------------------
  // 1. Run a semantic search across the indexed codebase
  // ------------------------------------------------------------
  const semanticResults = await semanticSearch('LRU cache eviction', {
    limit: 5,
    minSimilarity: 0.4,
  });

  console.log(`🔍 Semantic search returned ${semanticResults.totalCount} candidates:`);
  semanticResults.localResults.slice(0, 5).forEach((result, idx) => {
    console.log(
      `   ${idx + 1}. ${result.name} (${result.similarity.toFixed(3)}) - ${result.id}`,
    );
  });
  console.log();

  // ------------------------------------------------------------
  // 2. Expand context around the top semantic hit
  // ------------------------------------------------------------
  if (semanticResults.localResults.length > 0) {
    const anchor = semanticResults.localResults[0];
    console.log(`🧠 Exploring code similar to ${anchor.name} (${anchor.id})`);

    const similar = await searchSimilarFunctions(anchor.id, 5);
    similar.forEach((result, idx) => {
      console.log(
        `   ${idx + 1}. ${result.name} (${result.similarity.toFixed(3)}) - ${result.id}`,
      );
    });
    console.log();
  } else {
    console.log('⚠️  No semantic matches found, skipping similarity expansion.\n');
  }

  console.log('✨ Done! Calls executed entirely through the native addon.');
  console.log('   Try adjusting the query string or limit to explore more results.');
}

main().catch((error) => {
  console.error('❌ Example failed:', error);
  process.exit(1);
});
