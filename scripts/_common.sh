# shellcheck shell=bash
# Common helpers sourced by install.sh and update.sh.

: "${BINARY_NAME:=quicssh-rs}"
: "${INSTALL_DIR:=/usr/local/bin}"
: "${UNIT_PATH:=/etc/systemd/system/quicssh-rs.service}"
UNIT_NAME=$(basename "$UNIT_PATH")

die() { echo "error: $*" >&2; exit 1; }

require_root() {
    [[ $EUID -eq 0 ]] || die "must run as root (try: sudo $0 $*)"
}

require_systemd() {
    [[ -d /run/systemd/system ]] || die "systemd not detected — this script targets modern Linux with systemd"
}

check_tools() {
    local tool
    for tool in curl tar install systemctl sha256sum; do
        command -v "$tool" >/dev/null 2>&1 || die "required tool not found: $tool"
    done
}

detect_arch() {
    local m
    m=$(uname -m)
    case "$m" in
        x86_64|amd64) echo x86_64 ;;
        aarch64|arm64) echo aarch64 ;;
        *) die "unsupported architecture: $m (only x86_64 and aarch64 are released)" ;;
    esac
}

# Pass GITHUB_TOKEN via env to bump the 60/hr unauth rate limit to 5000/hr.
gh_curl() {
    local auth=()
    [[ -n "${GITHUB_TOKEN:-}" ]] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
    curl -fsSL "${auth[@]}" "$@"
}

# Ensure version string has "v" prefix (idempotent). "0.1.6" -> "v0.1.6".
normalize_version() { [[ "$1" == v* ]] && echo "$1" || echo "v$1"; }

# Resolve version string. Empty arg → latest tag from GitHub API.
# Accepts both "0.1.6" and "v0.1.6"; always returns "vX.Y.Z".
resolve_version() {
    local arg=${1:-}
    if [[ -n "$arg" ]]; then
        normalize_version "$arg"
        return
    fi
    local tag
    tag=$(gh_curl "https://api.github.com/repos/$REPO/releases/latest" \
        | grep -m1 '"tag_name"' \
        | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/') \
        || die "failed to query latest release from $REPO"
    [[ -n "$tag" ]] || die "no releases found in $REPO"
    echo "$tag"
}

# Download and extract the release tarball. Empty `ver` uses the
# /releases/latest/download/ redirect, so no API call and no rate limit.
download_binary() {
    local ver=$1 arch=$2
    local tarball="${BINARY_NAME}-Linux-${arch}-musl.tar.gz"
    local base="https://github.com/$REPO/releases"
    local url
    if [[ -z "$ver" ]]; then
        url="$base/latest/download/$tarball"
    else
        url="$base/download/$(normalize_version "$ver")/$tarball"
    fi
    local tmp
    tmp=$(mktemp -d)
    echo "downloading $url" >&2
    curl -fL --retry 3 -o "$tmp/$tarball" "$url" \
        || die "failed to download $url"
    if curl -fL -s -o "$tmp/$tarball.sha256" "$url.sha256"; then
        (cd "$tmp" && sha256sum -c "$tarball.sha256" >/dev/null) \
            || die "sha256 mismatch for $tarball"
    fi
    tar -C "$tmp" -xzf "$tmp/$tarball" || die "failed to extract $tarball"
    [[ -x "$tmp/$BINARY_NAME" ]] || die "expected $BINARY_NAME inside tarball"
    echo "$tmp/$BINARY_NAME"
}

show_status() {
    sleep 1
    systemctl --no-pager --full status "$UNIT_NAME" | head -10
}
