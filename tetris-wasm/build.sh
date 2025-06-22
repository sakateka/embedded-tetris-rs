#!/bin/bash

# Build script for tetris-wasm

set -e

echo "🦀 Building Tetris WASM..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "❌ wasm-pack is not installed. Please install it with:"
    echo "   curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
    exit 1
fi

# Build the WASM package
echo "📦 Building WASM package..."
wasm-pack build --target web --no-typescript --out-dir pkg

echo "🔧 Optimizing WASM file..."
# Optional: Use wasm-opt for size optimization if available
if command -v wasm-opt &> /dev/null; then
    wasm-opt -Oz -o pkg/tetris_wasm_bg.wasm pkg/tetris_wasm_bg.wasm
    echo "✅ WASM file optimized"
else
    echo "ℹ️  wasm-opt not found, skipping optimization (install binaryen for smaller files)"
fi

echo "🎮 Build complete! Files generated in pkg/"
echo ""
echo "To serve the demo:"
echo "   cd tetris-wasm"
echo "   python3 -m http.server 8000"
echo "   # or use any other static file server"
echo ""
echo "Then open http://localhost:8000 in your browser" 