#!/usr/bin/env bash
set -euo pipefail

report="${1:-mutants.out/outcomes.json}"
test -f "$report"

caught=$(jq '.caught | length' "$report")
missed=$(jq '.missed | length' "$report")
timeout=$(jq '.timeout | length' "$report")
tested=$((caught + missed + timeout))
test "$tested" -gt 0

score=$(awk -v caught="$caught" -v tested="$tested" 'BEGIN { printf "%.2f", (caught / tested) * 100 }')
echo "mutation score: ${score}% (${caught}/${tested}; unviable excluded)"
awk -v score="$score" 'BEGIN { exit !(score >= 95.0) }'
