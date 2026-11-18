#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="/Volumes/SSD-512G-Mac/business/dioxus/util/oxidase"
LOCK_DIR="/tmp/oxidase-cargo.lockdir"

cd "$REPO_ROOT"

acquire_lock() {
    while ! mkdir "$LOCK_DIR" 2>/dev/null; do
        if [[ -r "$LOCK_DIR/pid" ]]; then
            old_pid="$(cat "$LOCK_DIR/pid" 2>/dev/null || true)"
            if [[ "$old_pid" =~ ^[0-9]+$ ]] && kill -0 "$old_pid" 2>/dev/null; then
                sleep 1
                continue
            fi
            if [[ ! "$old_pid" =~ ^[0-9]+$ ]]; then
                sleep 1
                continue
            fi
        else
            sleep 1
            continue
        fi
        rm -f "$LOCK_DIR/pid"
        rmdir "$LOCK_DIR" 2>/dev/null || true
    done
    printf '%s\n' "$$" > "$LOCK_DIR/pid"
}

release_lock() {
    if [[ -r "$LOCK_DIR/pid" ]] &&
        [[ "$(cat "$LOCK_DIR/pid" 2>/dev/null || true)" == "$$" ]]; then
        rm -f "$LOCK_DIR/pid"
        rmdir "$LOCK_DIR" 2>/dev/null || true
    fi
}

acquire_lock
trap release_lock EXIT INT TERM

if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER
    RUSTC_WRAPPER="$(command -v sccache)"
    export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-20G}"
fi

cargo check --workspace
cargo check -p oxidase --target wasm32-unknown-unknown
cargo nextest run --workspace --features oxidase/native "$@"
