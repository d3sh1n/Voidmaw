#!/bin/bash
# Build script for Voidmaw Rust project on Linux/WSL

set -e

CONFIG="${1:-release}"
ARCH="${2:-x86_64}"

echo "Building Voidmaw (Rust 1.80 Version)"
echo "Configuration: $CONFIG"
echo "Architecture: $ARCH"
echo ""

build_target() {
    local target=$1
    local config=$2

    echo "Building for $target..."

    # Add target if not already installed
    rustup target add "$target" 2>/dev/null || true

    if [ "$config" = "release" ]; then
        cargo build --release --target "$target"
    else
        cargo build --target "$target"
    fi

    echo "Build successful for $target"
    echo ""
}

case $ARCH in
    x86_64)
        build_target "x86_64-pc-windows-msvc" "$CONFIG"
        ;;
    i686)
        build_target "i686-pc-windows-msvc" "$CONFIG"
        ;;
    both)
        build_target "x86_64-pc-windows-msvc" "$CONFIG"
        build_target "i686-pc-windows-msvc" "$CONFIG"
        ;;
    *)
        echo "Invalid architecture: $ARCH"
        echo "Valid options: x86_64, i686, both"
        exit 1
        ;;
esac

echo "All builds completed successfully!"
echo ""
echo "Binaries location:"

if [ "$CONFIG" = "release" ]; then
    if [ "$ARCH" = "x86_64" ] || [ "$ARCH" = "both" ]; then
        echo "  x64: target/x86_64-pc-windows-msvc/release/dismantle.exe"
        echo "       target/x86_64-pc-windows-msvc/release/voidmaw.exe"
    fi
    if [ "$ARCH" = "i686" ] || [ "$ARCH" = "both" ]; then
        echo "  x86: target/i686-pc-windows-msvc/release/dismantle.exe"
        echo "       target/i686-pc-windows-msvc/release/voidmaw.exe"
    fi
else
    if [ "$ARCH" = "x86_64" ] || [ "$ARCH" = "both" ]; then
        echo "  x64: target/x86_64-pc-windows-msvc/debug/dismantle.exe"
        echo "       target/x86_64-pc-windows-msvc/debug/voidmaw.exe"
    fi
    if [ "$ARCH" = "i686" ] || [ "$ARCH" = "both" ]; then
        echo "  x86: target/i686-pc-windows-msvc/debug/dismantle.exe"
        echo "       target/i686-pc-windows-msvc/debug/voidmaw.exe"
    fi
fi
