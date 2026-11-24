#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Checks a packed release of the C++ binding as an adapter uses it: built on its own from the
# archive, and fetched by an adapter's CMake build by URL and pinned by its hash.
# usage: check-package.sh <brep_protocol-<version>.tar.xz> <its SHA-256>
set -euo pipefail
archive=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
hash=$2
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

tar -xJf "$archive" -C "$work"
source=$(echo "$work"/brep_protocol-*)
# The box kernel must answer with the protocol's major version that the header gives.
major=$(sed -n 's/.*PROTOCOL_VERSION = "\([0-9]*\)\..*/\1/p' \
    "$source/include/brep_protocol/brep_protocol.h")
cmake -S "$source" -B "$work/alone" -DCMAKE_BUILD_TYPE=Release > "$work/alone.log" 2>&1 ||
    { cat "$work/alone.log"; exit 1; }
cmake --build "$work/alone"
echo '{"cmd": "hello"}' | "$work/alone/box_kernel" | grep -q "\"protocol\":\"$major\." ||
    { echo "check-package.sh: the box kernel built from the archive does not answer"; exit 1; }

mkdir -p "$work/adapter"
cp "$source/examples/box_kernel.cpp" "$work/adapter/"
cat > "$work/adapter/CMakeLists.txt" <<EOF
cmake_minimum_required(VERSION 3.24)
project(adapter LANGUAGES CXX)
include(FetchContent)
FetchContent_Declare(brep_protocol URL file://$archive URL_HASH SHA256=$hash
                     DOWNLOAD_EXTRACT_TIMESTAMP TRUE)
FetchContent_MakeAvailable(brep_protocol)
add_executable(adapter box_kernel.cpp)
target_link_libraries(adapter PRIVATE brep_protocol::brep_protocol)
EOF
cmake -S "$work/adapter" -B "$work/adapter/build" -DCMAKE_BUILD_TYPE=Release \
    > "$work/adapter.log" 2>&1 || { cat "$work/adapter.log"; exit 1; }
cmake --build "$work/adapter/build"
echo '{"cmd": "hello"}' | "$work/adapter/build/adapter" | grep -q "\"protocol\":\"$major\." ||
    { echo "check-package.sh: an adapter fetching the archive does not answer hello"; exit 1; }
echo "the archive builds on its own and as a dependency pinned by its hash"
