#!/bin/sh
# Uninstall OxiDNS files installed by scripts/install.sh on Linux and macOS.
#
# Common overrides:
#   OXIDNS_INSTALL_DIR=/opt/oxidns
#   OXIDNS_BIN_DIR=/usr/local/bin
#   OXIDNS_UNINSTALL_SERVICE=1
#   OXIDNS_PURGE=1

set -eu

INSTALL_DIR="${OXIDNS_INSTALL_DIR:-}"
BIN_DIR="${OXIDNS_BIN_DIR:-}"
NO_PATH="${OXIDNS_NO_PATH:-0}"
UNINSTALL_SERVICE="${OXIDNS_UNINSTALL_SERVICE:-auto}"
PURGE="${OXIDNS_PURGE:-0}"
HOME_DIR="${HOME:-}"

log() {
    printf '%s\n' "$*"
}

warn() {
    printf 'warning: %s\n' "$*" >&2
}

err() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

is_truthy() {
    case "$1" in
        1|true|TRUE|yes|YES|on|ON) return 0 ;;
        *) return 1 ;;
    esac
}

is_root() {
    [ "$(id -u 2>/dev/null || printf '1')" = "0" ]
}

should_uninstall_service() {
    case "$UNINSTALL_SERVICE" in
        auto|"")
            is_root
            ;;
        *)
            is_truthy "$UNINSTALL_SERVICE"
            ;;
    esac
}

same_file() {
    a="$1"
    b="$2"

    if [ ! -e "$a" ] || [ ! -e "$b" ]; then
        return 1
    fi

    if command -v cmp >/dev/null 2>&1; then
        cmp -s "$a" "$b"
    else
        return 1
    fi
}

safe_purge_dir() {
    dir="$1"

    [ -n "$dir" ] || return 1

    if [ -d "$dir" ]; then
        dir_check="$(cd "$dir" && pwd -P)"
    else
        dir_check="$dir"
    fi

    if [ -n "$HOME_DIR" ] && [ "$dir_check" = "$HOME_DIR" ]; then
        return 1
    fi

    case "$dir_check" in
        /|/bin|/sbin|/usr|/usr/bin|/usr/local|/usr/local/bin|/opt|/etc|/var|/tmp)
            return 1
            ;;
        *)
            return 0
            ;;
    esac
}

remove_command_shim() {
    link_path="$BIN_DIR/oxidns"

    if [ "$BIN_DIR" = "$INSTALL_DIR" ]; then
        return 0
    fi

    if [ -L "$link_path" ]; then
        target="$(readlink "$link_path" 2>/dev/null || printf '')"
        if [ "$target" = "$INSTALL_DIR/oxidns" ]; then
            rm -f "$link_path"
            log "Removed command shim: $link_path"
        else
            warn "$link_path points to $target; leaving it unchanged"
        fi
        return 0
    fi

    if [ -f "$link_path" ]; then
        if same_file "$link_path" "$INSTALL_DIR/oxidns"; then
            rm -f "$link_path"
            log "Removed copied command: $link_path"
        else
            warn "$link_path is not managed by this installer; leaving it unchanged"
        fi
    fi
}

uninstall_service() {
    bin="$INSTALL_DIR/oxidns"

    if [ ! -x "$bin" ]; then
        warn "cannot uninstall service because $bin was not found"
        return 0
    fi

    "$bin" service stop >/dev/null 2>&1 || true
    if "$bin" service uninstall >/dev/null 2>&1; then
        log "Removed OxiDNS service"
    else
        warn "service uninstall failed or service was not installed"
    fi
}

if [ -z "$INSTALL_DIR" ]; then
    if is_root; then
        INSTALL_DIR="/opt/oxidns"
    else
        [ -n "${HOME:-}" ] || err "HOME is not set; set OXIDNS_INSTALL_DIR explicitly"
        INSTALL_DIR="$HOME/.oxidns"
    fi
fi

if [ -z "$BIN_DIR" ]; then
    if is_root; then
        BIN_DIR="/usr/local/bin"
    else
        [ -n "${HOME:-}" ] || err "HOME is not set; set OXIDNS_BIN_DIR explicitly"
        BIN_DIR="$HOME/.local/bin"
    fi
fi

if should_uninstall_service; then
    uninstall_service
fi

if ! is_truthy "$NO_PATH"; then
    remove_command_shim
fi

if is_truthy "$PURGE"; then
    if safe_purge_dir "$INSTALL_DIR"; then
        rm -rf "$INSTALL_DIR"
        log "Purged OxiDNS install directory: $INSTALL_DIR"
    else
        err "refusing to purge unsafe install directory: $INSTALL_DIR"
    fi
else
    rm -f "$INSTALL_DIR/oxidns" "$INSTALL_DIR/oxidns.tmp" "$INSTALL_DIR/LICENSE"
    rm -rf "$INSTALL_DIR/webui"
    log "Removed OxiDNS binary and WebUI from $INSTALL_DIR"
    if [ -f "$INSTALL_DIR/config.yaml" ]; then
        log "Kept config: $INSTALL_DIR/config.yaml"
        log "Use OXIDNS_PURGE=1 to remove the install directory and config."
    fi
fi

log "OxiDNS uninstall complete"
