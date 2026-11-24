#!/bin/bash
# SPDX-License-Identifier: MIT
# Prints an installation token of a GitHub App, limited to one repository and to the permissions
# given as JSON; by default the benchmark's App's self-hosted runners on this repository. Reads
# APP_CLIENT_ID, APP_PRIVATE_KEY and GITHUB_REPOSITORY.
# Usage: app-token.sh [permissions [owner/repository]]
set -euo pipefail

permissions=${1:-'{"administration": "write"}'}
repository=${2:-$GITHUB_REPOSITORY}

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
    "$api/repos/$repository/installation" | jq -r .id)
scope=$(jq -n --arg repo "${repository#*/}" --argjson permissions "$permissions" \
    '{repositories: [$repo], permissions: $permissions}')
curl -sSf -X POST -H "Authorization: Bearer $jwt" -H "$accept" \
    "$api/app/installations/$installation/access_tokens" -d "$scope" | jq -r .token
