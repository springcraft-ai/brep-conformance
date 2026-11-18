#!/bin/bash
# Starts one adapter process on an isolated core of its own. The suite starts its adapter command
# once per worker; each start claims the first free core in BENCH_CORES and holds it until the
# process exits. The kernel does not balance tasks across isolated cores, so a plain taskset onto
# the whole isolated list would leave every worker on one core.
set -euo pipefail

mkdir -p "$BENCH_PIN_DIR"

# A replacement for a lost worker can start before the old one has let go of its core: an adapter
# run through a launcher is a grandchild of the suite, which waits only for the launcher. Without
# the wait below the replacement finds no core, exits, and the suite records a crash for a case
# that never ran.
deadline=$((SECONDS + ${BENCH_PIN_WAIT_S:-60}))
while :; do
    for core in $BENCH_CORES; do
        exec 9> "$BENCH_PIN_DIR/$core"
        if flock -n 9; then
            exec taskset -c "$core" "$@"
        fi
    done
    if [ "$SECONDS" -ge "$deadline" ]; then
        echo "pin.sh: no free core in BENCH_CORES" >&2
        exit 1
    fi
    sleep 0.05
done
