#!/bin/bash
# On the benchmark machine, from the repository's root: installs the toolchains, checks each
# kernel's adapter out at the commit kernels.json pins, under adapters/, and builds the suite and
# every adapter as kernels.json says. Runs on the housekeeping cores; nothing here is timed.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

# libgl1 is for the graphics toolkits OpenCascade's libraries load; curl and bzip2 are for the
# OpenCascade adapter's build, which fetches micromamba.
sudo DEBIAN_FRONTEND=noninteractive apt-get update -qq
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential pkg-config util-linux \
    libgl1 git python3 unzip curl bzip2

# The session's shape files leave the machine through the staging bucket.
if ! command -v aws >/dev/null; then
    curl -sSfo /tmp/awscliv2.zip https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip
    unzip -q -o /tmp/awscliv2.zip -d /tmp
    sudo /tmp/aws/install --update
fi

if ! command -v cargo >/dev/null && [ ! -x "$HOME/.cargo/bin/cargo" ]; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
if ! command -v uv >/dev/null && [ ! -x "$HOME/.local/bin/uv" ]; then
    curl -LsSf https://astral.sh/uv/install.sh | sh
fi
export PATH="$HOME/.local/bin:$PATH"

cargo build --release --locked -p brep-conformance
mkdir -p results
python3 -I bench/adapters.py list "$root" > results/adapters.txt
while read -r kernel repo commit; do
    echo "=== $kernel: $repo at $commit"
    rm -rf "adapters/$kernel"
    git clone -q "$repo" "adapters/$kernel" < /dev/null
    git -C "adapters/$kernel" checkout -q "$commit"
    while IFS= read -r step; do
        (cd "adapters/$kernel" && sh -c "$step" < /dev/null)
    done < <(python3 -I bench/adapters.py build "$root" "$kernel")
done < results/adapters.txt

# NIST's STEP files, unless the session runs with BENCH_NIST=false. fetch.py accepts only the
# archive whose hash it pins.
if [ "${BENCH_NIST:-true}" != false ]; then
    python3 -I nist/fetch.py
fi

{
    echo "brep_conformance=$(git rev-parse HEAD)"
    while read -r kernel _ commit; do
        echo "adapter_$kernel=$commit"
    done < results/adapters.txt
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "uv=$(uv --version)"
    echo "aws=$(aws --version | cut -d' ' -f1)"
    echo "cc=$(cc --version | head -1)"
    echo "rustflags=${RUSTFLAGS:-}"
    echo "release_profile=each repository's own, no LTO, default target-cpu"
} > results/build.txt
cat results/build.txt
