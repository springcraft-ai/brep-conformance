#!/bin/bash
# SPDX-License-Identifier: MIT
# Installs the system libraries an adapter's build needs on GitHub's Ubuntu runner: OpenCascade's
# libraries, as its Python package ships them, load a graphics toolkit that needs libgl1.
set -euo pipefail

sudo DEBIAN_FRONTEND=noninteractive apt-get update -qq
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libgl1
