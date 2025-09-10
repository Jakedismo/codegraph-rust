#!/usr/bin/env python3
import argparse
import json
from pathlib import Path
from datetime import datetime


def collect_estimates(root: Path):
    results = []
    for est in root.glob('**/new/estimates.json'):
        try:
            data = json.loads(est.read_text())
        except Exception:
            continue
        bench_id = str(est.parent.parent.relative_to(root))  # group/bench
        mean = data.get('mean', {}).get('point_estimate')
        median = data.get('median', {}).get('point_estimate')
        slope = data.get('slope', {}).get('point_estimate')
        results.append({
            'bench': bench_id,
            'mean_s': mean,
            'median_s': median,
            'slope_s': slope,
        })
    return results


def ns(v):
    return v * 1e9 if v is not None else None


def generate_md(results, compare_summary):
    ts = datetime.utcnow().strftime('%Y-%m-%d %H:%M:%SZ')
    lines = []
    lines.append(f"# CodeGraph Benchmark Report\n")
    lines.append(f"Generated: {ts}\n")

    if compare_summary:
        lines.append(f"Baseline: {compare_summary.get('baseline')}  ")
        lines.append(f"Regression threshold: {compare_summary.get('threshold', 0.1)*100:.1f}%\n")

    lines.append("## Summary\n")
    lines.append("- Total benches: {}".format(len(results)))
    lines.append("")

    lines.append("## Benchmarks\n")
    lines.append("| Bench | Mean (ns) | Median (ns) |")
    lines.append("|---|---:|---:|")
    for r in sorted(results, key=lambda x: (x['bench'])):
        lines.append("| {} | {} | {} |".format(
            r['bench'],
            f"{ns(r['mean_s']):.0f}" if r['mean_s'] else "-",
            f"{ns(r['median_s']):.0f}" if r['median_s'] else "-",
        ))

    if compare_summary and compare_summary.get('comparisons'):
        lines.append("\n## Changes vs Baseline\n")
        lines.append("| Bench | Current (ns) | Baseline (ns) | Delta % |")
        lines.append("|---|---:|---:|---:|")
        for c in sorted(compare_summary['comparisons'], key=lambda x: -abs(x['delta_pct'])):
            delta = c['delta_pct']
            sign = '+' if delta >= 0 else ''
            lines.append("| {} | {:.0f} | {:.0f} | {}{:.1f}% |".format(
                c['bench'], c['current_ns'], c['baseline_ns'], sign, delta
            ))

    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--criterion-root', default='target/criterion')
    parser.add_argument('--output', default='benchmarks/reports/benchmark_report.md')
    args = parser.parse_args()

    root = Path(args.criterion_root)
    results = collect_estimates(root)

    compare_summary = None
    compare_path = Path('benchmarks/reports/last_compare_summary.json')
    if compare_path.exists():
        compare_summary = json.loads(compare_path.read_text())

    output = generate_md(results, compare_summary)
    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(output)

    # Persist machine-readable summary as well
    json_out = out_path.with_suffix('.json')
    json_out.write_text(json.dumps({'results': results, 'compare': compare_summary}, indent=2))

    print(f"Wrote report: {out_path}")


if __name__ == '__main__':
    main()

