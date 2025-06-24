#!/bin/bash

# Build script for Android Tetris using xbuild
# Requires: rustup, xbuild, Android SDK

set -e

echo "=== Tetris Android Build Script ==="
echo "Using android-activity with NativeActivity backend"
echo ""

# Check if xbuild is installed
if ! command -v x &> /dev/null; then
    echo "Installing xbuild..."
    echo 'Please install xbuild: cargo install xbuild'
    exit 1
fi

echo "✓ xbuild available"

# Check environment
echo ""
echo "Checking xbuild doctor..."
x doctor

# Build APK for Android
echo ""
echo "Building APK with xbuild..."
x build --platform android --arch arm64 --format apk --release

echo ""
echo "✅ Build complete!"
echo ""

# Find and show the APK location
APK_PATH="target/x/release/android/tetris-android.apk"
if [ -f "$APK_PATH" ]; then
    APK_SIZE=$(du -h "$APK_PATH" | cut -f1)
    echo "📱 APK generated: $APK_PATH ($APK_SIZE)"
    echo ""
    echo "🚀 To install on connected device:"
    echo "  adb install $APK_PATH"
    echo ""
    echo "📱 To install directly with xbuild:"
    echo "  x run --release --device <device-id>"
    echo ""
    echo "📋 To list connected devices:"
    echo "  adb devices"
    echo "  # or"
    echo "  x devices"
else
    echo "⚠️  APK not found at expected location: $APK_PATH"
    echo "   Searching for APK files..."
    find target -name "*.apk" -type f 2>/dev/null || echo "   No APK files found"
fi
