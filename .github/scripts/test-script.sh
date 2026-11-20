#!/usr/bin/env bash
# Smoke test for gweather-add-loc.sh.
#
# Runs the script against a stubbed `gsettings` and checks that the value it
# writes is a valid libgweather-4 "av" entry, for all code paths (Python/GLib,
# awk fallback, empty and non-empty location lists).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="$root/gweather-add-loc.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

export SMOKE_STATE="$work"

mkdir -p "$work/bin"
cat > "$work/bin/gsettings" <<'STUB'
#!/usr/bin/env bash
case "$1" in
    get) cat "$SMOKE_STATE/current" ;;
    set) printf '%s' "$4" > "$SMOKE_STATE/written" ;;
    *) echo "unexpected gsettings call: $*" >&2; exit 1 ;;
esac
STUB
cat > "$work/bin/flatpak" <<'STUB'
#!/usr/bin/env bash
exit 1
STUB
chmod +x "$work/bin/gsettings" "$work/bin/flatpak"

run_add() {
    PATH="$work/bin:$PATH" "$script" --no-restart "$@"
}

# check_written NAMES_JSON LATITUDE LONGITUDE
# NAMES_JSON is the expected list of location names, newest first.
check_written() {
    python3 - "$work/written" "$1" "$2" "$3" <<'PY'
import json
import math
import sys

from gi.repository import GLib

path, names_raw, lat_deg, lon_deg = sys.argv[1:5]
names = json.loads(names_raw)
root = GLib.Variant.parse(GLib.VariantType("av"), open(path, encoding="utf-8").read(), None, None)
if not root.is_normal_form():
    sys.exit("written value is not in normal form")

entries = [root.get_child_value(i).get_variant() for i in range(root.n_children())]
if len(entries) != len(names):
    sys.exit(f"expected {len(names)} entries, got {len(entries)}")

found = []
for entry in entries:
    if entry.get_child_value(0).get_uint32() != 2:
        sys.exit("unexpected entry version")
    payload = entry.get_child_value(1).get_variant()
    found.append(payload.get_child_value(0).get_string())
    if payload.get_child_value(3).n_children() != 1:
        sys.exit("expected exactly one coordinate pair")

if found != names:
    sys.exit(f"names differ: {found!r} != {names!r}")

pair = entries[0].get_child_value(1).get_variant().get_child_value(3).get_child_value(0)
for got, deg, label in (
    (pair.get_child_value(0).get_double(), lat_deg, "latitude"),
    (pair.get_child_value(1).get_double(), lon_deg, "longitude"),
):
    if abs(got - math.radians(float(deg))) > 1e-9:
        sys.exit(f"{label} mismatch")
PY
}

# 1. Empty location list.
printf '[]\n' > "$work/current"
run_add "Berlin" 52.52 13.405 > /dev/null
check_written '["Berlin"]' 52.52 13.405

# 2. Append to an existing list; names must round-trip unchanged.
cp "$work/written" "$work/current"
name='Zürich "O'"'"'Brien" & <x>'
run_add "$name" 47.37 8.54 > /dev/null
check_written "$(python3 -c 'import json, sys; print(json.dumps(sys.argv[1:]))' "$name" "Berlin")" 47.37 8.54

# 3. Fall back to awk when the Python/GLib encoder is unavailable.
mkdir -p "$work/no-python"
printf '#!/bin/sh\nexit 1\n' > "$work/no-python/python3"
chmod +x "$work/no-python/python3"
printf '[]\n' > "$work/current"
PATH="$work/no-python:$work/bin:$PATH" "$script" --no-restart "Fallback" -33.8688 151.2093 \
    > /dev/null 2>&1
check_written '["Fallback"]' -33.8688 151.2093

echo "gweather-add-loc.sh: all smoke tests passed"
