#!/usr/bin/env bash
# Publishes one kernel's shape files: those a run's results list in its cases' `exports`, from the
# directory the run wrote them to (`--glb DIR --step DIR`), under
# <prefix>/<suite>/<kernel>/<version>/ in the bucket. A file the bucket already holds is never
# rewritten, so a second run uploads nothing. With no BREP_CONFORMANCE_BUCKET it only checks the
# files and says what it would upload.
# Usage: publish-assets.sh RESULTS.json FILES_DIR
set -euo pipefail
# sort and comm compare keys byte by byte, as Python's sorted does.
export LC_ALL=C

if [[ $# -ne 2 ]]; then
    echo "usage: $0 RESULTS.json FILES_DIR" >&2
    exit 2
fi
results=$1
files=$2
bucket=${BREP_CONFORMANCE_BUCKET:-}
prefix=${BREP_CONFORMANCE_PREFIX:-brep-conformance}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The kernel's folder, and every key its results list. A name or key that is not a path of plain
# names is refused: the site refuses a report with one, and the folder must not climb out.
python3 -I - "$results" "$files" "$work" <<'EOF'
import json, os, re, sys

results, files, work = sys.argv[1:]
r = json.load(open(results))
plain = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$")
folder = [str(r.get("suite", "")), str(r["kernel"].get("name", "")), str(r["kernel"].get("version", ""))]
for part in folder:
    if not plain.match(part) or ".." in part:
        sys.exit(f"{results}: {part!r} cannot name a folder")
keys = []
for c in r["cases"]:
    ex = c.get("exports") or {}
    keys += [k for k in [ex.get("glb")] + list(ex.get("step") or []) if k]
for k in keys:
    parts = k.split("/")
    if not all(plain.match(p) and ".." not in p for p in parts) or not k.endswith((".glb", ".step")):
        sys.exit(f"{results}: key {k!r} is not a path of plain names ending .glb or .step")
missing = [k for k in keys if not os.path.isfile(os.path.join(files, k))]
if missing:
    sys.exit(f"{len(missing)} listed files are not in {files}; first: {missing[0]}")
open(os.path.join(work, "folder"), "w").write("/".join(folder))
open(os.path.join(work, "keys"), "w").write("".join(k + "\n" for k in sorted(set(keys))))
EOF

folder=$(cat "$work/folder")
dest="$prefix/$folder"
listed=$(wc -l < "$work/keys" | tr -d ' ')
if [[ -z "$bucket" ]]; then
    echo "$listed files listed for $dest; no BREP_CONFORMANCE_BUCKET, so nothing uploaded"
    exit 0
fi

held() {
    aws s3api list-objects-v2 --bucket "$bucket" --prefix "$dest/" \
        --query 'Contents[].Key' --output text |
        tr '\t' '\n' | sed -e '/^None$/d' -e "s|^$dest/||" | sort
}

# Stage only the files the bucket lacks, so one recursive copy per type uploads them in parallel.
held > "$work/held"
comm -23 "$work/keys" "$work/held" > "$work/new"
stage="$work/stage"
while IFS= read -r key; do
    mkdir -p "$stage/$(dirname "$key")"
    ln "$files/$key" "$stage/$key" 2>/dev/null || cp "$files/$key" "$stage/$key"
done < "$work/new"
new=$(wc -l < "$work/new" | tr -d ' ')
echo "$dest: $listed files listed, $((listed - new)) already published, $new to upload"

if [[ "$new" -gt 0 ]]; then
    immutable="public, max-age=31536000, immutable"
    for kind in "glb model/gltf-binary" "step model/step"; do
        read -r ext type <<<"$kind"
        aws s3 cp "$stage" "s3://$bucket/$dest/" --recursive --exclude '*' --include "*.$ext" \
            --content-type "$type" --cache-control "$immutable" --no-progress --only-show-errors
    done
fi

held > "$work/held"
absent=$(comm -23 "$work/keys" "$work/held" | wc -l | tr -d ' ')
if [[ "$absent" -gt 0 ]]; then
    echo "$absent listed files are still not in the bucket" >&2
    exit 1
fi
echo "every listed file is in s3://$bucket/$dest/"
