#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Writes the flat report of each pass of a benchmark session, named by its hash, and the run's entry
# for reports.json (doc/formats.md), from the session's results folder. Uploads nothing.
# Usage: publish-reports.sh <results dir> <run>-<attempt> <out dir>
# Run from the repository's root, after `cargo build --release -p brep-conformance`.
set -euo pipefail

results=$1
run=$2
out=$3
tool=target/release/brep-conformance-report

if ! [[ "$run" =~ ^[0-9]+-[0-9]+$ ]]; then
    echo "publish-reports: $run is not <run>-<attempt>" >&2
    exit 1
fi
sha() {
    if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi
}

mkdir -p "$out"
# A session from before the plan recorded its scope ran every kernel.
scope=$(sed -n 's/^scope=//p' "$results/session.txt")
entry=$(jq -n --arg run "$run" --arg scope "${scope:-all}" \
    '{run: $run, suite: null, scope: $scope, reports: {}}')
# A pass is a `<kernel>: exit=` line of session.txt, which every pass writes when it ends.
while read -r pass seconds; do
    "$tool" "$results/$pass.json" --flat "$out/$pass.json" --verified \
        --bench "$results" --run "$run" > /dev/null
    hash=$(sha "$out/$pass.json" | cut -c1-16)
    file="$pass-$hash.json"
    mv "$out/$pass.json" "$out/$file"
    entry=$(jq --arg k "$pass" --arg file "$file" --argjson seconds "${seconds:-null}" \
        --slurpfile r "$out/$file" '
        .suite = $r[0].suite.version
        | .reports[$k] = {file: $file, complete: ($r[0].run.complete == true),
                          seconds: $seconds}' <<<"$entry")
    echo "$pass: $file"
done < <(sed -n -E 's/^([A-Za-z0-9][A-Za-z0-9._-]*): exit=[^ ]*( seconds=([0-9]+))?.*/\1 \3/p' \
    "$results/session.txt")
echo "$entry" > "$out/entry.json"
if [ "$(jq '.reports | length' "$out/entry.json")" -eq 0 ]; then
    echo "publish-reports: session.txt names no pass" >&2
    exit 1
fi
