#!/usr/bin/env bash
# Generate the changelog entry for a release tag with git-cliff.
#
# Usage: changelog-entry.sh vX.Y.Z
#
# Prints the entry (heading, groups and commits) for all commits between the
# previous version tag and the given tag. The tag must exist locally.
set -euo pipefail

tag="${1:?usage: changelog-entry.sh vX.Y.Z}"
[[ "$tag" == v* ]] || tag="v$tag"

if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: invalid version tag '$tag' (expected vX.Y.Z)" >&2
    exit 1
fi

if ! command -v git-cliff >/dev/null 2>&1; then
    echo "Error: git-cliff is not installed" >&2
    exit 1
fi

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

if ! git rev-parse --verify --quiet "refs/tags/$tag^{commit}" >/dev/null; then
    echo "Error: tag $tag does not exist (fetch tags first)" >&2
    exit 1
fi

previous="$(git describe --tags --abbrev=0 --match 'v[0-9]*' "${tag}^" 2>/dev/null || true)"
if [ -n "$previous" ]; then
    range="$previous..$tag"
else
    # git-cliff does not accept a bare tag as range; a full commit ID makes it
    # process all history leading up to the first release.
    range="$(git rev-parse "${tag}^{commit}")"
fi

git-cliff --config cliff.toml --tag "$tag" --strip header "$range" |
    awk 'NF { last = NR } { lines[NR] = $0 } END { for (i = 1; i <= last; i++) print lines[i] }'
