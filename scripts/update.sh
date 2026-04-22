#!/usr/bin/env bash
# Update existing quicssh-rs install to latest (or specified) release.
# Usage: update.sh [vX.Y.Z]

set -euo pipefail

: "${REPO:=aruz/quicssh-rs}"

common_tmp=""
binary=""
trap 'rm -f "${common_tmp:-}"; [[ -n "${binary:-}" ]] && rm -rf "$(dirname "$binary")"' EXIT

self_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-}")" 2>/dev/null && pwd || true)
if [[ -n "$self_dir" && -f "$self_dir/_common.sh" ]]; then
    # shellcheck disable=SC1091
    source "$self_dir/_common.sh"
else
    common_tmp=$(mktemp)
    curl -fsSL "https://raw.githubusercontent.com/$REPO/master/scripts/_common.sh" -o "$common_tmp" \
        || { echo "error: failed to fetch _common.sh" >&2; exit 1; }
    # shellcheck disable=SC1090
    source "$common_tmp"
fi

require_root "$@"
check_tools

bin_path="$INSTALL_DIR/$BINARY_NAME"
[[ -x "$bin_path" ]] || die "$bin_path not found — use install.sh first"
[[ -f "$UNIT_PATH" ]] || die "$UNIT_PATH not found — use install.sh first"

current=$("$bin_path" --version 2>/dev/null | awk '{print $NF}')
target=$(resolve_version "${1:-}")

if [[ "$current" == "${target#v}" ]]; then
    echo "already at $current"
    exit 0
fi

platform=$(detect_target)
binary=$(download_binary "$target" "$platform")

install -m 755 "$binary" "$bin_path"
systemctl restart "$UNIT_NAME"

show_status
echo
echo "updated $current -> $("$bin_path" --version | awk '{print $NF}')"
