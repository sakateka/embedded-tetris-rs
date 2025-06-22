# 🎮 Tetris Rust Multi-Platform

A multi-platform Tetris implementation written in Rust, featuring multiple games (Tetris, Snake, Tanks, Races, Game of Life) across different targets:

- **Console**: Terminal-based gameplay with keyboard controls
- **Embedded**: Microcontroller/embedded systems support
- **WebAssembly**: Browser-based gameplay with minimal JavaScript

## 🚀 Quick Start

### 🌐 WebAssembly (Browser)

The WASM target provides the easiest way to try the games with maximum Rust code and minimal JavaScript.

```bash
# Install wasm-pack if you haven't already
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build and run the WASM version
cd tetris-wasm
./build.sh

# Serve the demo (choose one):
python3 -m http.server 8000    # Python
# or
npx serve .                    # Node.js
# or use any static file server

# Open http://localhost:8000 in your browser
```

**Controls**: Arrow keys/WASD to navigate, Enter/Space to select, Z/X for additional controls.

### 🖥️ Console Version

Experience retro terminal-based gameplay:

```bash
# Run the console version
cargo run --bin tetris-console

# Or build and run manually
cd tetris-console
cargo run
```

**Controls**: Arrow keys to navigate, Enter to select, Ctrl+C to exit.

### 🔧 Embedded Version

For microcontrollers and embedded systems:

```bash
# Build for embedded target (requires appropriate toolchain)
cd tetris-embedded
cargo build --release
```

## 🎯 Game Features

### Available Games
1. **Tetris** 🟦 - Classic falling blocks puzzle
2. **Snake** 🐍 - Navigate and grow your snake
3. **Tanks** 🚗 - Tank battle arena
4. **Races** 🏁 - High-speed racing action
5. **Life** 🧬 - Conway's Game of Life cellular automaton

### Display Format
- **8x32 pixel LED matrix** simulation
- **Retro pixelated graphics** with authentic color palette
- **Smooth animations** and responsive controls

## 🏗️ Architecture

### Project Structure
```
tetris-rs/
├── tetris-lib/          # Core game library (no_std compatible)
│   ├── src/games/       # Individual game implementations
│   ├── src/common.rs    # Shared types and traits
│   └── src/figure.rs    # Tetris piece definitions
├── tetris-console/      # Terminal/console target
├── tetris-embedded/     # Embedded systems target
└── tetris-wasm/         # WebAssembly browser target
```

### Key Design Principles
- **Maximum Rust, Minimum JavaScript**: WASM target uses only ~50 lines of JS
- **Shared Core Logic**: All games share the same `tetris-lib` implementation
- **Platform Abstractions**: Clean trait-based interfaces for different platforms
- **No-std Compatible**: Core library works in embedded environments

### Trait Interfaces
```rust
trait LedDisplay {
    async fn write(&mut self, leds: &[RGB8; 256]);
}

trait GameController {
    async fn read_x(&mut self) -> i8;
    async fn read_y(&mut self) -> i8;
    fn joystick_was_pressed(&self) -> bool;
    // ...
}

trait Timer {
    async fn sleep_millis(&self, millis: u64);
}
```

## 🛠️ Development

### Prerequisites
- **Rust 1.70+** with `wasm32-unknown-unknown` target
- **wasm-pack** for WASM builds
- **Node.js/Python** for serving WASM demo

### Building All Targets

```bash
# Console version
cargo build --bin tetris-console

# WASM version
cd tetris-wasm && ./build.sh

# Embedded version (requires target setup)
cd tetris-embedded && cargo build --release
```

### Adding New Games
1. Implement your game in `tetris-lib/src/games/`
2. Add it to the game menu in `tetris-lib/src/games/mod.rs`
3. All targets automatically inherit the new game!

### Platform-Specific Features

#### WASM Target
- **Canvas Rendering**: Direct pixel manipulation for performance
- **Keyboard Input**: Full keyboard support with preventDefault
- **Async Runtime**: wasm-bindgen-futures for async game loops
- **Error Handling**: Panic hook for better debugging

#### Console Target
- **Raw Terminal Mode**: Direct terminal control like classic games
- **Async I/O**: Tokio-based async runtime
- **Signal Handling**: Graceful cleanup on Ctrl+C

#### Embedded Target
- **No-std Environment**: Works without standard library
- **Hardware Abstraction**: Direct hardware control interfaces
- **Memory Efficiency**: Optimized for constrained environments

---

**Happy Gaming! 🎮✨**
