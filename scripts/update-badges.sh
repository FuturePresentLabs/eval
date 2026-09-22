#!/usr/bin/env bash
# Regenerates README.md's test-count badge from ground truth. See
# ooda/cadbench/pcbbench's identical scripts -- same convention everywhere.
set -euo pipefail
cd "$(dirname "$0")/.."

output="$(cargo test --workspace 2>&1)"
tests="$(grep -oE '^test result: ok\. [0-9]+ passed' <<<"$output" \
    | grep -oE '[0-9]+' \
    | awk '{s+=$1} END {print s+0}')"

if [ "$tests" -eq 0 ]; then
    echo "update-badges: found 0 passing tests -- refusing to write a bogus badge" >&2
    echo "$output" >&2
    exit 1
fi

sed -E "s#(badge/tests-)[0-9]+(%20passing)#\\1${tests}\\2#" README.md > README.md.tmp
mv README.md.tmp README.md

echo "tests: ${tests}"
