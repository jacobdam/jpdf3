#!/usr/bin/env bash
# One-line progress (not protected).
cd "$(dirname "$0")/.." || exit 1
ms=$(grep -m1 -i '^milestone:' ops/QUEUE.md 2>/dev/null | cut -d: -f2- | xargs)
last=$(tail -1 ops/logs/index.tsv 2>/dev/null | cut -f1,2)
counts=$(gh api "repos/jacobdam/jpdf3/milestones?state=open" --jq ".[] | select(.title==\"${ms:-M0}\") | \"\(.closed_issues)/\(.closed_issues+.open_issues) issues closed\"" 2>/dev/null)
halt=$([[ -f ops/HALT ]] && echo "HALTED" || echo "running")
echo "jpdf: ${ms:-?} | ${counts:-no milestone data} | last session: ${last:-none} | $halt"
