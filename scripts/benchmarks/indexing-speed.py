#!/usr/bin/env python3
"""Run a prebuilt indexing benchmark without including compilation in wall/RSS measurements."""
import argparse
import hashlib
import json
import os
import platform
import resource
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--files", type=int, default=100)
parser.add_argument("--repeats", type=int, default=3)
parser.add_argument("--analyzers", action="store_true", help="Enable installed language servers and build analyzers")
args = parser.parse_args()
environment = dict(os.environ, CODEGRAPH_SURREALDB_URL="mem://", CODEGRAPH_NO_PROGRESS="1", CODEGRAPH_EMBEDDING_POLICY="off", CODEGRAPH_SEMANTIC_RESOLUTION="off", CODEGRAPH_VECTOR_INDEX_MODE="off", CODEGRAPH_ANALYZERS="1" if args.analyzers else "0", CODEGRAPH_BENCH_FILES=str(args.files), CODEGRAPH_BENCH_REPEATS=str(args.repeats))
start = time.perf_counter()
process = subprocess.run([str(args.binary.resolve())], env=environment, capture_output=True, text=True, check=True)
report = json.loads(process.stdout)
report["process_wall_seconds"] = time.perf_counter() - start
rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
report["largest_process_peak_rss_bytes"] = rss if platform.system() == "Darwin" else rss * 1024
report["rss_scope"] = "Largest process peak in this benchmark process tree; not simultaneous aggregate RSS"
report["platform"] = platform.platform()
binary_hash = hashlib.sha256()
with args.binary.open("rb") as binary:
    for chunk in iter(lambda: binary.read(1024 * 1024), b""):
        binary_hash.update(chunk)
report["binary_sha256"] = binary_hash.hexdigest()
args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
