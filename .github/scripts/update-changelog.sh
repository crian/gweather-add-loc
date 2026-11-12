#!/usr/bin/env bash
# Insert the generated changelog entry for a release tag into CHANGELOG.md.
#
# Usage: update-changelog.sh vX.Y.Z [CHANGELOG.md]
#
# The entry is inserted below the "## Unreleased" heading, replacing any
# manually written unreleased content. Running the script twice is safe.
set -euo pipefail

tag="${1:?usage: update-changelog.sh vX.Y.Z [CHANGELOG.md]}"
file="${2:-CHANGELOG.md}"
[[ "$tag" == v* ]] || tag="v$tag"
ver="${tag#v}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

if [ ! -f "$file" ]; then
    echo "Error: $file not found" >&2
    exit 1
fi

if ! grep -qE '^## Unreleased[[:space:]]*$' "$file"; then
    echo "Error: $file has no '## Unreleased' section" >&2
    exit 1
fi

if grep -qF "## $ver " "$file"; then
    echo "$file already contains version $ver; nothing to do." >&2
    exit 0
fi

entry="$(mktemp)"
trap 'rm -f "$entry"' EXIT
"$repo_root/.github/scripts/changelog-entry.sh" "$tag" > "$entry"

if ! grep -q '^- ' "$entry"; then
    echo "Error: no changelog entries generated for $tag" >&2
    exit 1
fi

# Print everything up to "## Unreleased", drop the old unreleased content,
# then insert the new entry before the next release section.
awk -v entry="$entry" '
    BEGIN {
        while ((getline line < entry) > 0) block = block line "\n"
        close(entry)
    }
    { lines[NR] = $0 }
    END {
        unreleased = 0
        for (i = 1; i <= NR; i++) {
            if (lines[i] ~ /^## Unreleased[[:space:]]*$/) {
                unreleased = i
                break
            }
        }
        for (i = 1; i <= unreleased; i++) print lines[i]

        next_section = unreleased + 1
        while (next_section <= NR && lines[next_section] !~ /^## /) next_section++

        print ""
        printf "%s", block
        if (next_section <= NR) print ""
        for (; next_section <= NR; next_section++) print lines[next_section]
    }
' "$file" > "$file.tmp"
mv "$file.tmp" "$file"
