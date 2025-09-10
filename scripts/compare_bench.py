#!/usr/bin/env python3
import argparse
import json
import sys
from pathlib import Path


def find_estimate_files(root: Path):
    return list(root.glob('**/new/estimates.json'))


def load_json(p: Path):
    try:
        with p.open('r') as f:
            return json.load(f)
    except Exception:
        return None


def locate_baseline_for(new_est_path: Path, baseline_name: str):
    # Typical Criterion layout: <target>/criterion/<group>/<bench>/{new|base/<baseline_name>}/estimates.json
    parent = new_est_path.parent  # .../new
    base_dir_candidates = [parent.parent / 'base' / baseline_name, parent.parent / 'baseline' / baseline_name]
    for cand in base_dir_candidates:
        cand_est = cand / 'estimates.json'
        if cand_est.exists():
            return cand_est
    return None


def main():
    parser = argparse.ArgumentParser(description='Compare Criterion benchmark results to a saved baseline.')
    parser.add_argument('--baseline', default='baseline', help='Baseline name saved with --save-baseline')
    parser.add_argument('--threshold', type=float, default=0.10, help='Regression threshold as fraction (e.g., 0.10 = 10%)')
    parser.add_argument('--root', default='target/criterion', help='Criterion output root directory')
    args = parser.parse_args()

    root = Path(args.root)
    if not root.exists():
        print(f"No criterion output at {root}, nothing to compare yet.")
        return 0

    new_estimates = find_estimate_files(root)
    if not new_estimates:
        print("No new estimates found; ensure you ran cargo bench with --save-baseline.")
        return 1

    regressions = []
    improvements = []
    comparisons = []

    for new_path in new_estimates:
        base_path = locate_baseline_for(new_path, args.baseline)
        if not base_path:
            # First run, no baseline recorded for this bench yet
            continue
        new = load_json(new_path)
        base = load_json(base_path)
        if not new or not base:
            continue

        def mean_ns(est):
            # Criterion estimates are in seconds by default; convert to ns for readability
            return float(est.get('mean', {}).get('point_estimate', 0.0)) * 1e9

        new_mean = mean_ns(new)
        base_mean = mean_ns(base)
        if new_mean == 0 or base_mean == 0:
            continue

        ratio = (new_mean / base_mean) - 1.0
        bench_id = str(new_path.parent.parent.relative_to(root))  # group/bench
        comparisons.append({'bench': bench_id, 'baseline_ns': base_mean, 'current_ns': new_mean, 'delta_pct': ratio * 100.0})

        if ratio > args.threshold:
            regressions.append((bench_id, base_mean, new_mean, ratio))
        elif ratio < -args.threshold:
            improvements.append((bench_id, base_mean, new_mean, ratio))

    # Print summary
    print("Benchmark comparison vs baseline='{}':".format(args.baseline))
    for cmpo in sorted(comparisons, key=lambda x: x['delta_pct'], reverse=True):
        sign = '+' if cmpo['delta_pct'] >= 0 else ''
        print(f"- {cmpo['bench']}: {cmpo['current_ns']:.0f}ns (baseline {cmpo['baseline_ns']:.0f}ns) [{sign}{cmpo['delta_pct']:.1f}%]")

    # Persist summary JSON
    reports_dir = Path('benchmarks/reports')
    reports_dir.mkdir(parents=True, exist_ok=True)
    with (reports_dir / 'last_compare_summary.json').open('w') as f:
        json.dump({'baseline': args.baseline, 'threshold': args.threshold, 'comparisons': comparisons}, f, indent=2)

    if regressions:
        print("\nDetected regressions exceeding threshold {:.1f}%:".format(args.threshold * 100.0))
        for bench_id, base_mean, new_mean, ratio in regressions:
            print(f"  REGRESSION: {bench_id} -> {new_mean:.0f}ns (was {base_mean:.0f}ns) [+{ratio*100.0:.1f}%]")
        # Exit non-zero for CI
        return 2

    if improvements:
        print("\nDetected improvements exceeding threshold {:.1f}%:".format(args.threshold * 100.0))
        for bench_id, base_mean, new_mean, ratio in improvements:
            print(f"  IMPROVEMENT: {bench_id} -> {new_mean:.0f}ns (was {base_mean:.0f}ns) [{ratio*100.0:.1f}%]")

    return 0


if __name__ == '__main__':
    sys.exit(main())

