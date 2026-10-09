#!/usr/bin/env python3
"""Compare Criterion results against baseline; fail on regression > threshold."""
import json
import sys
from pathlib import Path

THRESHOLD = 0.20
CRITERION_DIR = Path("target/criterion")


def main() -> int:
    regressions = []
    results = []

    for change in CRITERION_DIR.glob("*/*/change/estimates.json"):
        bench = change.parent.parent
        bench_id = f"{bench.parent.name}/{bench.name}"
        with open(change) as f:
            estimates = json.load(f)
        change_pct = estimates["mean"]["point_estimate"] * 100
        results.append((bench_id, change_pct))
        if change_pct > THRESHOLD * 100:
            regressions.append((bench_id, change_pct))

    if not results:
        print("No comparison data found (no baseline?)")
        return 0

    print(f"{'benchmark':<45} {'change':>10}")
    for bench_id, pct in sorted(results):
        marker = " REGRESSION" if (bench_id, pct) in regressions else ""
        print(f"{bench_id:<45} {pct:>+9.1f}%{marker}")

    if regressions:
        print(f"\n{len(regressions)} regression(s) above {THRESHOLD * 100:.0f}% threshold")
        return 1

    print("\nNo regressions detected")
    return 0


if __name__ == "__main__":
    sys.exit(main())
