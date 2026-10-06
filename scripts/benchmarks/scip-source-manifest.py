#!/usr/bin/env python3
"""Capture SHA-256 source identity immediately alongside a freshly generated SCIP index.
Run before editing sources. This records identity; it cannot certify an older artifact.
"""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("root", type=Path)
parser.add_argument("index", type=Path)
parser.add_argument("sources", nargs="+", help="Paths relative to root, as emitted by the compiler index")
args = parser.parse_args()
root = args.root.resolve()
manifest = {}
for relative in args.sources:
    source = (root / relative).resolve()
    if not source.is_relative_to(root):
        parser.error(f"Source escapes root: {relative}")
    manifest[relative] = hashlib.sha256(source.read_bytes()).hexdigest()
args.index.with_suffix(".sources.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
