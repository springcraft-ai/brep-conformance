#!/bin/bash
# On the benchmark machine, from the repository's root, after setup.sh: one session over the four
# kernels, with the settings bench/README.md gives.
# Every pass also writes its kernel's shape files, which the suite draws after judging each case,
# outside every timed call; they go to a RAM disk, so no pass waits on the root volume, and are
# moved to assets/<kernel>/ once the last pass ends.
# Usage: run-session.sh <threads> [suite arguments, such as --family fillets/]
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
export PATH="$HOME/.local/bin:$PATH"

threads=$1
shift
root=$PWD
suite=$root/target/release/brep-conformance
shapes=/dev/shm/brep-conformance-assets

isolated=$(tr ',' '\n' < /sys/devices/system/cpu/isolated | while IFS=- read -r a b; do
    seq "$a" "${b:-$a}"
done | tr '\n' ' ')
count=$(echo "$isolated" | wc -w)
if [ "$threads" -gt "$count" ]; then
    echo "run-session: $threads threads but only $count isolated cores" >&2
    exit 1
fi
export BENCH_CORES=$isolated

# NIST's STEP files run, as step/read-nist cases, only when the session asks for them.
nist=()
if [ "${BENCH_NIST:-false}" = true ]; then
    nist=(--nist "$root/nist")
fi

rm -rf "$shapes" assets
mkdir -p results "$shapes" assets
cp /etc/benchmark-runner/environment results/environment.txt
{
    echo "threads=$threads"
    echo "suite_args=$*"
    echo "nist=${BENCH_NIST:-false}"
    echo "exports=$shapes"
    echo "adapter_cores=$isolated"
    echo "runner_cores=$(taskset -cp $$ | sed 's/.*: //')"
    echo "started=$(date -u '+%Y-%m-%d %H:%M:%S UTC')"
} > results/session.txt

# Any pass that did not finish cleanly, or is not of the pinned release, fails the session after
# everything is moved, so its results are kept and nothing is published.
failed=""

one() {
    local name=$1
    shift
    local adapter
    adapter=$(python3 -I bench/adapters.py run "$root" "$name")
    export BENCH_PIN_DIR
    BENCH_PIN_DIR=$(mktemp -d)
    echo "=== $name: started $(date -u '+%H:%M:%S')"
    local t0=$SECONDS rc=0
    "$suite" --adapter "$root/bench/pin.sh $adapter" --threads "$threads" \
        --glb "$shapes/$name" --step "$shapes/$name" "${nist[@]}" \
        --out "results/$name.json" "$@" > "results/$name.log" 2>&1 || rc=$?
    echo "=== $name: exit $rc after $((SECONDS - t0)) s"
    # A wrapper that could not place an adapter makes the suite record crashes for cases that
    # never ran, so the pass's numbers are not valid.
    local unplaced
    unplaced=$(grep -c "pin.sh: no free core" "results/$name.log" || true)
    if [ "$unplaced" -gt 0 ]; then
        echo "=== $name: INVALID, $unplaced adapter starts found no core"
        rc="invalid($unplaced unplaced)"
    fi
    # A pass is of the release the registry pins, or it is not an official number.
    local wrong
    if ! wrong=$(python3 -I bench/adapters.py check "$root" "$name" "results/$name.json" 2>&1); then
        echo "=== $name: INVALID, $wrong"
        rc="invalid(release: $wrong)"
    fi
    echo "$name: exit=$rc seconds=$((SECONDS - t0))" >> results/session.txt
    if [ "$rc" != 0 ]; then
        failed="$failed $name"
    fi
    tail -4 "results/$name.log"
}

# One pass per kernel. Each adapter is the one kernels.json pins; OpenCascade's starts the
# environment's own Python, not `uv run`, so the adapter is the suite's child and lets go of its core
# when the suite replaces it.
kernels=(springrep arris solidcore opencascade)
for kernel in "${kernels[@]}"; do
    one "$kernel" "$@"
done

echo "finished=$(date -u '+%Y-%m-%d %H:%M:%S UTC')" >> results/session.txt

for name in "${kernels[@]}"; do
    if [ -d "$shapes/$name" ]; then
        mv "$shapes/$name" "assets/$name"
        echo "assets_$name=$(find "assets/$name" -type f | wc -l) files, $(du -sh "assets/$name" | cut -f1)" \
            | tee -a results/session.txt
    fi
done
rm -rf "$shapes"
cat results/session.txt
if [ -n "$failed" ]; then
    echo "run-session: these passes did not finish cleanly:$failed" >&2
    exit 1
fi
