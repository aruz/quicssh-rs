#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Clean install of quicssh-rs server on a modern systemd Linux host.
Usage: install.sh [--version vX.Y.Z] [--port 4433] [--listen 0.0.0.0] [--proxy-to 127.0.0.1:22]
EOF
}

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

version=""
port=4433
listen="0.0.0.0"
proxy_to="127.0.0.1:22"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) version=$2; shift 2 ;;
        --port) port=$2; shift 2 ;;
        --listen) listen=$2; shift 2 ;;
        --proxy-to) proxy_to=$2; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

require_root "$@"
require_systemd
check_tools

bin_path="$INSTALL_DIR/$BINARY_NAME"
[[ ! -e "$bin_path" ]] || die "$bin_path already exists — use update.sh instead"
[[ ! -e "$UNIT_PATH" ]] || die "$UNIT_PATH already exists — use update.sh or remove it first"

target=$(detect_target)
binary=$(download_binary "$version" "$target")

install -m 755 "$binary" "$bin_path"

cat > "$UNIT_PATH" <<EOF
[Unit]
Description=quicssh-rs QUIC->SSH proxy
After=network-online.target sshd.service
Wants=network-online.target

[Service]
DynamicUser=yes
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
LockPersonality=yes
MemoryDenyWriteExecute=yes
NoNewPrivileges=yes
PrivateDevices=yes
ProtectClock=yes
ProtectControlGroups=yes
ProtectHome=yes
ProtectHostname=yes
ProtectKernelLogs=yes
ProtectKernelModules=yes
ProtectKernelTunables=yes
ProtectProc=invisible
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
RestrictNamespaces=yes
RestrictRealtime=yes
SystemCallArchitectures=native
SystemCallFilter=@system-service
SystemCallFilter=~@resources @privileged

Restart=always
RestartSec=3
ExecStart=$bin_path server -l $listen:$port --proxy-to $proxy_to

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now "$UNIT_NAME"

show_status
echo
echo "installed $("$bin_path" --version) listening on $listen:$port -> $proxy_to"
