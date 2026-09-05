#!/usr/bin/env bash
# ci/build-target.sh  build an isolex release binary for one target triple.
# Usage: bash ci/build-target.sh <target-triple>
# All per-target knowledge (linkers, toolchains, std availability) lives
# here; bitbucket-pipelines.yml just dispatches. Modelled on
# axonibyte/ddwill's script of the same name.
set -euo pipefail

TARGET="${1:?usage: build-target.sh <target-triple>}"
export CARGO_HOME="${CARGO_HOME:-$BITBUCKET_CLONE_DIR/.cargo_cache}"

# Pinned: -Z build-std against a floating nightly breaks spontaneously and
# makes tag builds unreproducible. Bump deliberately, together with the
# image pin in bitbucket-pipelines.yml and rust-version in Cargo.toml.
NIGHTLY="nightly-2026-08-01"

apt_install() {
    apt-get update
    apt-get install -y --no-install-recommends "$@"
}

# Zig ships FreeBSD libc headers, letting cargo-zigbuild cross-link
# FreeBSD binaries from Linux with no docker and no sysroot images.
install_zigbuild() {
    apt_install python3-pip
    pip3 install --break-system-packages cargo-zigbuild
}

build() { # tries offline first, falls back to online
    cargo build --target "$TARGET" --release --locked --bin isolex --offline ||
    cargo build --target "$TARGET" --release --locked --bin isolex
}

# The one lane CI can execute: run the Windows .exe under Wine and make it
# do real work -- compile the reference fixture to IR and confirm the
# expected icon is present -- so a Windows artifact is tested, not just
# linked.
smoke_test_wine() {
    apt_install wine
    export WINEDEBUG=-all
    local wine
    for wine in wine wine-stable wine64; do
        command -v "$wine" >/dev/null 2>&1 && break
    done
    command -v "$wine" >/dev/null 2>&1 || { echo "no wine launcher found after install" >&2; exit 1; }

    local bin="$PWD/target/$TARGET/release/isolex.exe"
    "$wine" "$bin" --version

    local dir
    dir=$(mktemp -d)
    cp tests/fixtures/sample.ilx "$dir/"
    "$wine" "$bin" compile "$dir" --ir "$dir/out.json"
    grep -q '"name": "switch-l3"' "$dir/out.json" \
        || { echo "wine smoke test: expected icon missing from IR" >&2; exit 1; }
    echo "wine smoke test: compile round trip OK"
    rm -rf "$dir"
}

case "$TARGET" in
    x86_64-unknown-linux-gnu)
        build
        ;;

    aarch64-unknown-linux-gnu)
        # libc6-dev-arm64-cross is only a Recommends of the gcc package,
        # so with --no-install-recommends it must be named explicitly.
        apt_install gcc-aarch64-linux-gnu libc6-dev-arm64-cross
        rustup target add "$TARGET"
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
        build
        ;;

    x86_64-pc-windows-gnu)
        # Tier 1: prebuilt std; MinGW cross-linker from apt. Runs under Wine
        # afterwards, making this the only cross target CI executes.
        apt_install gcc-mingw-w64-x86-64
        rustup target add "$TARGET"
        export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
        build
        smoke_test_wine
        ;;

    x86_64-unknown-freebsd)
        # Tier 2: prebuilt std exists; stable toolchain + zig linker. This
        # is the pack's own platform, so it is the load-bearing artifact.
        install_zigbuild
        rustup target add "$TARGET"
        cargo zigbuild --target "$TARGET" --release --locked --bin isolex
        ;;

    aarch64-unknown-freebsd)
        # Tier 3: no prebuilt std, so compile it with nightly -Z build-std.
        install_zigbuild
        rustup toolchain install "$NIGHTLY" --profile minimal --component rust-src
        cargo "+$NIGHTLY" zigbuild --target "$TARGET" --release --locked \
            --bin isolex -Z build-std=std,panic_abort
        ;;

    *)
        echo "unknown target: $TARGET" >&2
        exit 1
        ;;
esac

# --- package ---------------------------------------------------------------
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
mkdir -p dist
if [[ "$TARGET" == *windows* ]]; then
    cp "target/$TARGET/release/isolex.exe" "dist/isolex-v${VERSION}-${TARGET}.exe"
else
    cp "target/$TARGET/release/isolex" "dist/isolex-v${VERSION}-${TARGET}"
fi
