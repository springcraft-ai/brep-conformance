#!/bin/bash
# SPDX-License-Identifier: MIT
# Prints an installation token of the GitHub App that registers the benchmark runner, limited to
# this repository's self-hosted runners. Reads APP_CLIENT_ID, APP_PRIVATE_KEY and
# GITHUB_REPOSITORY.
set -euo pipefail

api=https://api.github.com
accept="Accept: application/vnd.github+json"

base64url() {
    openssl base64 -A | tr '+/' '-_' | tr -d '='
}

now=$(date +%s)
header=$(printf '{"alg":"RS256","typ":"JWT"}' | base64url)
claims=$(printf '{"iat":%d,"exp":%d,"iss":"%s"}' "$((now - 60))" "$((now + 300))" "$APP_CLIENT_ID" \
    | base64url)
signature=$(printf '%s.%s' "$header" "$claims" \
    | openssl dgst -sha256 -sign <(printf '%s\n' "$APP_PRIVATE_KEY") -binary | base64url)
jwt="$header.$claims.$signature"

installation=$(curl -sSf -H "Authorization: Bearer $jwt" -H "$accept" \
    "$api/repos/$GITHUB_REPOSITORY/installation" | jq -r .id)
scope=$(jq -n --arg repo "${GITHUB_REPOSITORY#*/}" \
    '{repositories: [$repo], permissions: {administration: "write"}}')
curl -sSf -X POST -H "Authorization: Bearer $jwt" -H "$accept" \
    "$api/app/installations/$installation/access_tokens" -d "$scope" | jq -r .token
