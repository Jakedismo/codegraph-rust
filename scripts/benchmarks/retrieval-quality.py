#!/usr/bin/env python3
"""Compare aligned same-model vector exports before accepting runtime/precision changes."""
import argparse
import json
import math
from pathlib import Path


def cosine(a, b):
    if len(a) != len(b) or not a or not all(math.isfinite(x) for x in a + b):
        raise ValueError("Invalid vector dimensions or non-finite values")
    denominator = math.sqrt(sum(x*x for x in a) * sum(x*x for x in b))
    if not denominator:
        raise ValueError("Zero vector")
    return sum(x*y for x, y in zip(a, b)) / denominator


def rows(data, name):
    result = {row["id"]: row for row in data[name]}
    if len(result) != len(data[name]) or not result:
        raise ValueError(f"Empty or duplicate {name} identities")
    return result


def compare(baseline, candidate, k):
    for key in ("model", "revision", "task", "tokenizer", "corpus"):
        if not baseline["identity"].get(key) or baseline["identity"][key] != candidate["identity"].get(key):
            raise ValueError(f"Missing or mismatched identity: {key}")
    bd, cd = rows(baseline, "documents"), rows(candidate, "documents")
    bq, cq = rows(baseline, "queries"), rows(candidate, "queries")
    if bd.keys() != cd.keys() or bq.keys() != cq.keys():
        raise ValueError("Document/query identity sets differ")
    delta = [1-cosine(bd[key]["vector"], cd[key]["vector"]) for key in bd]
    probes = []
    for key in sorted(bq):
        expected = set(bq[key].get("expected", []))
        if not expected or expected != set(cq[key].get("expected", [])) or not expected <= bd.keys():
            raise ValueError(f"Missing or mismatched relevance labels: {key}")
        top = lambda docs, query: [identifier for identifier, _ in sorted(((identifier, cosine(row["vector"], query["vector"])) for identifier, row in docs.items()), key=lambda pair: (-pair[1], pair[0]))[:k]]
        bt, ct = top(bd, bq[key]), top(cd, cq[key])
        probes.append({"id": key, "baseline_recall": len(expected.intersection(bt))/len(expected), "candidate_recall": len(expected.intersection(ct))/len(expected), "top_k_overlap": len(set(bt).intersection(ct))/min(k, len(bd)), "baseline_top": bt, "candidate_top": ct})
    mean = lambda field: sum(probe[field] for probe in probes)/len(probes)
    return {"queries": probes, "baseline_recall": mean("baseline_recall"), "candidate_recall": mean("candidate_recall"), "top_k_overlap": mean("top_k_overlap"), "mean_document_cosine_delta": sum(delta)/len(delta), "max_document_cosine_delta": max(delta)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--k", type=int, default=10)
    parser.add_argument("--max-recall-drop", type=float, default=0.01)
    parser.add_argument("--min-overlap", type=float, default=0.95)
    args = parser.parse_args()
    if args.k < 1:
        parser.error("k must be positive")
    report = compare(json.loads(args.baseline.read_text()), json.loads(args.candidate.read_text()), args.k)
    report["pass"] = report["candidate_recall"] >= report["baseline_recall"] - args.max_recall_drop and report["top_k_overlap"] >= args.min_overlap
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if report["pass"] else 1)

if __name__ == "__main__":
    main()
