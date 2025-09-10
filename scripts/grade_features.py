#!/usr/bin/env python3
import re
import json
import subprocess
from pathlib import Path
from datetime import datetime

ROOT = Path(__file__).resolve().parents[1]
FEATURE_MD = ROOT / 'docs/specifications/FEATURE_INVENTORY.md'
REPORT_DIR = ROOT / 'docs/reports'
REPORT_DIR.mkdir(parents=True, exist_ok=True)

def rg(pattern: str) -> int:
    try:
        res = subprocess.run(['rg', '-nS', pattern], cwd=str(ROOT), capture_output=True, text=True)
        if res.returncode == 0:
            return len([l for l in res.stdout.splitlines() if l.strip()])
        return 0
    except FileNotFoundError:
        return 0

def has_path(p: str) -> bool:
    return (ROOT / p).exists()

def score_feature(name: str, section: str) -> tuple[int, list[str]]:
    n = name.lower()
    evidence = []
    score = 0

    def bump(val: int, ev: str):
        nonlocal score
        score = max(score, val)
        evidence.append(ev)

    # Phase 0 heuristics
    if 'cargo workspace' in n:
        if rg(r'^\[workspace\]'):
            bump(5, 'Cargo.toml [workspace] present')
    if 'ci/cd' in n or 'pipeline' in n and 'ci' in section.lower():
        if has_path('.github/workflows/ci.yml'):
            bump(5, 'GitHub Actions CI present')
    if 'development tooling' in n or 'rustfmt' in n or 'clippy' in n:
        if has_path('.clippy.toml') or has_path('clippy.toml'):
            bump(4, 'Clippy config present')
        if rg('rustfmt'):
            bump(4, 'Rustfmt usage referenced')
    if 'build optimization' in n or 'release profile' in n:
        if rg(r'\[profile\.') and rg('lto'):
            bump(4, 'Profiles and LTO configured')
    if 'documentation structure' in n or 'readme' in n:
        if has_path('docs/index.md') and has_path('README.md'):
            bump(4, 'Docs hub and README present')

    # Core traits & abstractions
    if 'graphstore trait' in n:
        if rg('trait GraphStore'):
            bump(5, 'GraphStore trait defined')
    if 'vectorindex trait' in n or 'vector store' in n:
        if rg('trait VectorStore'):
            bump(5, 'VectorStore trait defined')
    if 'codeanalyzer' in n or 'code parser' in n:
        if rg('trait CodeParser'):
            bump(5, 'CodeParser trait defined')
    if 'embeddingprovider' in n:
        if rg('trait EmbeddingProvider'):
            bump(5, 'EmbeddingProvider trait defined')
    if 'error handling' in n:
        if rg('thiserror::Error') or rg('pub enum .*Error'):
            bump(4, 'Custom error type present')

    # Data models
    if 'node/edge' in n or 'node/edge structures' in n:
        if rg('struct CodeNode') or rg('struct CodeEdge'):
            bump(4, 'Node/Edge types exist')
    if 'entity types' in n or 'ast nodes' in n:
        if rg('CodeEntity'):
            bump(3, 'Entity representation exists')
    if 'embedding metadata' in n:
        if rg('Embedding') or rg('embedding'):
            bump(3, 'Embedding types present')
    if 'serialization' in n or 'rkyv' in n or 'zero-copy' in n:
        if rg('rkyv') or has_path('crates/codegraph-zerocopy'):
            bump(4, 'Zero-copy/serialization present')
    if 'configuration structures' in n:
        if has_path('crates/codegraph-core/src/config.rs'):
            bump(4, 'Config models implemented')

    # RocksDB & graph
    if 'rocksdb' in n or 'database' in n:
        if rg('rocksdb'):
            bump(4, 'RocksDB integration present')
    if 'transaction' in n:
        if has_path('crates/codegraph-graph/src/transactional_graph.rs'):
            bump(3, 'Transactional graph skeleton')
    if 'batch operation' in n:
        if has_path('crates/codegraph-graph/src/io_batcher.rs'):
            bump(4, 'I/O batcher implemented')
    if 'backup' in n or 'restore' in n:
        if has_path('crates/codegraph-graph/src/recovery.rs'):
            bump(3, 'Recovery module present')
    if 'compaction' in n:
        if rg('compaction'):
            bump(3, 'Compaction options configured')

    if 'node crud' in n or 'edge crud' in n:
        if has_path('crates/codegraph-graph/src/graph.rs'):
            bump(4, 'Graph CRUD implemented')
    if 'traversal' in n or 'bfs' in n or 'dfs' in n:
        if has_path('crates/codegraph-graph/src/traversal.rs'):
            bump(4, 'Traversal algorithms present')

    # Parser & entity extraction
    if 'tree-sitter' in n or 'parser engine' in section.lower() or 'ast parsing' in n:
        if has_path('crates/codegraph-parser/src/parser.rs'):
            bump(4, 'Parser implemented with tree-sitter')
    if 'python extractor' in n or 'javascript extractor' in n or 'typescript extractor' in n or 'rust extractor' in n:
        if has_path('crates/codegraph-parser/src/text_processor.rs'):
            bump(3, 'Multi-language entity extraction present')
    if 'dependency analysis' in section.lower() or 'import resolution' in n or 'call graph' in n or 'type inference' in n or 'data flow' in n:
        if rg('call graph') or rg('dependency'):
            bump(2, 'Partial analysis hooks')

    if 'incremental parsing' in section.lower() or 'delta parsing' in n or 'invalidation' in n:
        if has_path('crates/codegraph-parser/src/diff.rs') or has_path('crates/codegraph-graph/src/incremental.rs'):
            bump(3, 'Incremental foundations implemented')

    # Vector / FAISS
    if 'faiss' in section.lower() or 'index' in n and 'faiss' in section.lower():
        if has_path('crates/codegraph-vector/src/faiss_manager.rs'):
            bump(4, 'FAISS manager implemented (feature-gated)')
    if 'gpu' in n:
        if has_path('crates/codegraph-vector/src/gpu.rs'):
            bump(3, 'GPU hooks present')
    if 'memory mapping' in n or 'memory-mapped' in n:
        if rg('mmap'):
            bump(3, 'MMAP support present')
    if 'persistence' in n or 'save/load' in n:
        if has_path('crates/codegraph-vector/src/persistent.rs'):
            bump(3, 'Index persistence module present')
    if 'knn' in n or 'range quer' in n:
        if has_path('crates/codegraph-vector/src/knn.rs') or has_path('crates/codegraph-vector/src/search.rs'):
            bump(4, 'KNN search implemented')
    if 'query optimization' in n or 'search caching' in n or 'parallel search' in n or 'simd' in n:
        if has_path('crates/codegraph-vector/src/optimized_search.rs') or has_path('crates/codegraph-vector/src/simd_ops.rs'):
            bump(4, 'Optimized search / SIMD present')

    # AI Integration
    if 'embedding' in n and 'local' in n:
        if has_path('crates/codegraph-vector/src/local_provider.rs'):
            bump(3, 'Local embeddings via Candle (feature-gated)')
    if 'openai' in n:
        if has_path('crates/codegraph-vector/src/openai_provider.rs'):
            bump(4, 'OpenAI embeddings implemented')
    if 'text tokenization' in n or 'chunking' in n or 'normalization' in n or 'deduplication' in n:
        if has_path('crates/codegraph-parser/src/text_processor.rs'):
            bump(4, 'Text processing implemented')
    if 'rag' in section.lower() or 'response generation' in n or 'context retrieval' in n or 'result ranking' in n:
        if has_path('crates/codegraph-vector/src/rag'):
            bump(4, 'RAG pipeline modules present')

    # API Layer
    if 'graphql' in section.lower() or 'schema' in n or 'resolver' in n:
        if has_path('crates/codegraph-api/src/graphql'):
            bump(4, 'GraphQL schema/resolvers implemented')
    if 'real-time' in section.lower() or 'subscription' in n or 'websocket' in n or 'event broadcasting' in n:
        if has_path('crates/codegraph-api/src/subscriptions.rs') or has_path('crates/codegraph-api/src/streaming_handlers.rs'):
            bump(4, 'Real-time features implemented')
    if 'rate limiting' in n:
        if rg('RateLimitManager'):
            bump(5, 'Rate limiting implemented')
    if 'connection pooling' in n:
        if has_path('crates/codegraph-api/src/connection_pool.rs'):
            bump(4, 'Connection pooling implemented')
    if 'compression' in n:
        if rg('compression') or rg('gzip'):
            bump(2, 'Compression references present')
    if 'pagination' in n:
        if rg('pagination'):
            bump(2, 'Pagination references present')

    # MCP Protocol
    if 'mcp' in section.lower() or 'protocol' in n and 'mcp' in section.lower():
        if has_path('crates/codegraph-mcp/src'):
            bump(4, 'MCP protocol crate implemented')
    if 'multi-agent' in n or 'coordination' in section.lower():
        if rg('coordination'):
            bump(3, 'Coordination primitives present')
    if 'sdk' in section.lower():
        if has_path('crates/core-rag-mcp-server'):
            bump(3, 'Core RAG MCP server as SDK-like component')

    # Incremental updates
    if 'file system' in section.lower() or 'monitoring' in n or 'watch' in n:
        if has_path('crates/codegraph-graph/src/file_watcher.rs') or has_path('crates/codegraph-git/src/watcher.rs'):
            bump(4, 'File system watcher implemented')
    if 'git integration' in section.lower() or 'branch' in n or 'merge' in n or 'conflict' in n or 'history' in n:
        if has_path('crates/codegraph-git/src'):
            bump(4, 'Git integration implemented')
    if 'update pipeline' in section.lower() or 'change queue' in n or 'priority' in n or 'rollback' in n:
        if has_path('crates/codegraph-graph/src/update_scheduler.rs') and has_path('crates/codegraph-graph/src/delta_processor.rs'):
            bump(3, 'Update pipeline components present')

    # Optimization & Performance
    if 'memory optimization' in section.lower() or 'arena' in n or 'zero-copy' in n or 'memory pool' in n or 'leak' in n:
        if rg('Arena|buffer_pool|leak|memory'):
            bump(4, 'Memory optimization features present')
    if 'cpu optimization' in section.lower() or 'simd' in n or 'lock-free' in n or 'branch prediction' in n or 'hot path' in n:
        if has_path('crates/codegraph-vector/src/simd_ops.rs') or has_path('crates/codegraph-concurrent/src'):
            bump(4, 'CPU optimization features present')
    if 'i/o optimization' in section.lower() or 'async i/o' in n or 'read-ahead' in n or 'write coalesc' in n:
        if has_path('crates/codegraph-graph/src/io_batcher.rs'):
            bump(4, 'I/O optimization present')
    if 'network optimization' in section.lower() or 'http/2' in n or 'streaming' in n or 'load balancing' in n:
        if has_path('crates/codegraph-api/src/http2_optimizer.rs') or has_path('crates/codegraph-lb'):
            bump(4, 'Network optimization features present')

    # Deployment & Packaging
    if 'binary optimization' in section.lower() or 'lto' in n or 'strip' in n or 'dead code' in n:
        if rg('lto'):
            bump(4, 'Binary optimization configured')
    if 'configuration management' in section.lower() or 'environment' in n or 'command-line' in n or 'validation' in n or 'reloading' in n:
        if has_path('crates/codegraph-core/src/config.rs'):
            bump(4, 'Config mgmt implemented (incl. watcher)')
    if 'deployment tooling' in section.lower() or 'docker' in n or 'kubernetes' in n or 'health checks' in n or 'graceful shutdown' in n:
        if has_path('Dockerfile') or has_path('k8s'):
            bump(4, 'Docker/K8s tooling present')

    # Cross-cutting: Observability, Security, Testing
    if 'structured logging' in n or 'tracing' in n:
        if rg('tracing::'):
            bump(4, 'Structured logging with tracing used')
    if 'metrics' in n or 'prometheus' in n:
        if rg('prometheus') or has_path('monitoring'):
            bump(4, 'Prometheus metrics integrated')
    if 'distributed tracing' in n or 'opentelemetry' in n:
        if rg('opentelemetry'):
            bump(3, 'OpenTelemetry referenced')
    if 'performance profiling' in n:
        if rg('flamegraph') or has_path('benches'):
            bump(3, 'Profiling artifacts present')
    if 'error tracking' in n:
        if rg('SecurityEvent|error'):
            bump(3, 'Error tracking/logging present')

    if 'authentication' in n or 'authorization' in n or 'rate limiting' in n or 'input validation' in n or 'tls' in n:
        if has_path('crates/codegraph-api/src/middleware/security.rs'):
            bump(5, 'Security middleware implemented')
        if 'tls' in n and not rg('tls'):
            score = min(score, 2)

    if 'unit tests' in n:
        if rg('#[test]') or rg('#[tokio::test]'):
            bump(4, 'Extensive unit tests')
    if 'integration tests' in n:
        if rg('/tests/'):
            bump(3, 'Integration tests present')
    if 'performance tests' in n or 'benchmark' in n:
        if has_path('benches') or has_path('benchmarks'):
            bump(3, 'Benchmarks present')
    if 'property-based' in n or 'fuzz' in n:
        if rg('proptest') or rg('arbitrary'):
            bump(2, 'Some property/fuzz testing references')

    # Default: if nothing matched but feature resembles a plausible implemented area in repo, give minimal score
    if score == 0:
        if any(x in n for x in ['query', 'api', 'graph', 'vector', 'parser', 'git', 'index', 'mcp', 'cache', 'config']):
            score = 2
            evidence.append('Heuristic default for related module present')

    # Cap score to 5
    score = min(score, 5)
    return score, evidence

def complexity_weight(symbol: str) -> int:
    if 'L' in symbol: return 1
    if 'M' in symbol: return 2
    if 'H' in symbol: return 3
    if 'C' in symbol: return 4
    return 1

def parse_features(md_path: Path):
    items = []
    phase = None
    section = None
    with md_path.open() as f:
        for line in f:
            if line.startswith('## '):
                phase = line.strip().replace('## ', '')
            elif line.startswith('### '):
                # Sections like 0.1, 1.1 etc
                section = line.strip().replace('### ', '')
            elif line.startswith('#### '):
                section = line.strip().replace('#### ', '')
            elif line.startswith('|') and not line.startswith('|-') and 'Feature' not in line:
                parts = [p.strip() for p in line.strip().split('|')[1:-1]]
                if len(parts) >= 5:
                    feature, complexity, parallel, desc, hours = parts[:5]
                    items.append({
                        'phase': phase,
                        'section': section,
                        'feature': feature,
                        'complexity': complexity,
                        'parallel': parallel,
                        'description': desc,
                        'estimated_hours': hours,
                    })
    return items

def main():
    features = parse_features(FEATURE_MD)
    scored = []
    by_phase = {}
    for item in features:
        # Only include features under numbered phases 0-6
        ph = (item.get('phase') or '')
        if not re.match(r'^Phase [0-6]', ph):
            continue
        score, evidence = score_feature(item['feature'], item['section'] or '')
        weight = complexity_weight(item['complexity'])
        item_scored = {
            **item,
            'score_0_5': score,
            'weight': weight,
            'weighted_score': score * weight,
            'evidence': evidence,
        }
        scored.append(item_scored)
        ph = item['phase'] or 'Unknown'
        bp = by_phase.setdefault(ph, {'features': 0, 'score': 0, 'weighted_score': 0, 'max_score': 0, 'max_weighted': 0})
        bp['features'] += 1
        bp['score'] += score
        bp['weighted_score'] += score * weight
        bp['max_score'] += 5
        bp['max_weighted'] += 5 * weight

    # Aggregates
    total_features = len(scored)
    total_score = sum(x['score_0_5'] for x in scored)
    total_max = 5 * total_features
    total_weighted = sum(x['weighted_score'] for x in scored)
    total_weighted_max = sum(5 * x['weight'] for x in scored)

    report = {
        'timestamp': datetime.utcnow().isoformat() + 'Z',
        'subject_id': 'codegraph_repo',
        'reference_document': str(FEATURE_MD),
        'totals': {
            'features': total_features,
            'score_sum': total_score,
            'score_pct': round(100 * total_score / total_max, 2) if total_max else 0.0,
            'weighted_sum': total_weighted,
            'weighted_pct': round(100 * total_weighted / total_weighted_max, 2) if total_weighted_max else 0.0,
        },
        'by_phase': {
            ph: {
                **vals,
                'completion_pct': round(100 * vals['score'] / vals['max_score'], 2) if vals['max_score'] else 0.0,
                'completion_weighted_pct': round(100 * vals['weighted_score'] / vals['max_weighted'], 2) if vals['max_weighted'] else 0.0,
            }
            for ph, vals in by_phase.items()
        },
        'features': scored,
    }

    # Write JSON
    (REPORT_DIR / 'feature_grading.json').write_text(json.dumps(report, indent=2))

    # Write MD summary table per phase
    lines = []
    lines.append(f"# Feature Implementation Rubric Grading\n\n")
    lines.append(f"Generated: {report['timestamp']}\n\n")
    lines.append(f"Reference: {FEATURE_MD}\n\n")
    lines.append(f"- Total features: {total_features}\n")
    lines.append(f"- Raw completion: {report['totals']['score_pct']}%\n")
    lines.append(f"- Weighted completion: {report['totals']['weighted_pct']}%\n\n")

    for ph, vals in report['by_phase'].items():
        lines.append(f"## {ph}\n")
        lines.append(f"- Features: {vals['features']}\n")
        lines.append(f"- Completion: {vals['completion_pct']}%\n")
        lines.append(f"- Weighted: {vals['completion_weighted_pct']}%\n\n")

    (REPORT_DIR / 'feature_grading.md').write_text(''.join(lines))

if __name__ == '__main__':
    main()
