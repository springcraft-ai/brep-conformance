#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Builds an adapter's checkout with the commands kernels.json gives it, as adapter_update.py plan
# printed them. Usage: adapter-build.sh <adapter> <plan.json>
set -euo pipefail

plan=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
cd "$1"
while IFS= read -r command; do
    bash -c "$command"
done < <(jq -r '.build[]' "$plan")
