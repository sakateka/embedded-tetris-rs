# Tetris Android

Android implementation of the Tetris game using Rust with `android-activity` and `NativeActivity`.

## 🎮 Features

- Pure Rust implementation using `android-activity` (no Java/JNI required)
- 8x32 pixel display (scaled to full screen)

## 🔧 Prerequisites

### 1. Rust Toolchain
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Android SDK
- Install Android Studio or Android SDK command-line tools
- Install Android SDK (API level 21+)
- Set environment variable:
  ```bash
  export ANDROID_SDK_ROOT=$HOME/Android/Sdk/
  ```

### 3. Build Tools
```bash
# xbuild for direct APK creation
cargo install xbuild
```

## 🚀 Building

### Build APK
```bash
./build.sh # Build the APK
```

The script will:
1. Check environment and dependencies
2. Build the Rust library for ARM64 Android
3. Generate APK at `../target/x/release/android/tetris-android.apk`
4. Show installation instructions

### Alternative Build Options
```bash
# Build for different architectures
x build --platform android --arch arm64 --format apk --release   # ARM64
x build --platform android --arch arm --format apk --release     # ARM32
x build --platform android --arch x86_64 --format apk --release  # x86-64

# Build, install and run directly to connected device
x run --release --device <device-id>
```

## 📱 Installation

### Install on Device
```bash
# Find connected devices
adb devices

# Install the APK
adb install ../target/x/release/android/tetris-android.apk
```

## 🏗️ Architecture

```
┌─────────────────────────────────┐
│       Android App               │
│  ┌─────────────────────────┐    │
│  │    NativeActivity       │    │
│  │  ┌─────────────────┐    │    │
│  │  │  android_main() │    │    │
│  │  │     (Rust)      │    │    │
│  │  └─────────────────┘    │    │
│  └─────────────────────────┘    │
├─────────────────────────────────┤
│       android-activity          │
│    (Event handling, Input)      │
├─────────────────────────────────┤
│       tetris-lib                │
│    (Game Logic, Display)        │
└─────────────────────────────────┘
```

This implementation uses:
- **`xbuild`**: Modern cross-platform build tool for direct APK generation
- **`android-activity`**: Modern Rust-first Android app framework
- **`NativeActivity`**: Built-in Android activity class (no Java required)
- **`tetris-lib`**: Shared game logic across all platforms

## 🔧 Technical Details

### Key Features of Implementation
- **No Java/JNI**: Pure Rust implementation using `android-activity`
- **NativeActivity**: Uses Android's built-in `NativeActivity` class
- **Input handling**: Uses `android-activity`'s `InputIterator` API
- **Async game loop**: Tokio-based async runtime for game logic
- **Global state**: Thread-safe static variables for LED display and input

## 🐛 Troubleshooting

### Build Issues

**"ANDROID_SDK_ROOT not set"**
```bash
export ANDROID_SDK_ROOT=$HOME/Android/Sdk/
```

**"x command not found"**
```bash
cargo install xbuild
```

**"Doctor check failed"**
- Ensure Android SDK is properly installed
- Run `x doctor` to see specific issues

**"ndk-sys only supports compiling for Android"**
- This is expected when running `cargo check` on host
- Only build with Android targets using `x build`

### Runtime Issues

Check logs with:
```bash
adb logcat | grep tetris
```

### Performance

The game runs with:
- 16ms frame time (60 FPS target)
- Minimal allocations in game loop
- Efficient LED array copying
- Single ARM64 architecture for optimal performance

## 🛠️ Development and Debugging

### Environment Setup
```bash
# Set Android SDK path (required)
export ANDROID_SDK_ROOT=$HOME/Android/Sdk/

# Verify environment
x doctor

# Check connected devices
x devices
adb devices
```

### Development Workflow
```bash
# Complete development cycle
cd tetris-android
export ANDROID_SDK_ROOT=$HOME/Android/Sdk/

# 1. Build
x build --platform android --arch arm64 --format apk --release

# 2. Install
adb install -r ../target/x/release/android/tetris-android.apk

# 3. Test
adb shell am force-stop com.example.tetris_android
adb shell am start -n com.example.tetris_android/android.app.NativeActivity

# 4. Debug
adb logcat -s tetris_android

# Quick rebuild and test
x build --platform android --arch arm64 --format apk --release && \
adb install -r ../target/x/release/android/tetris-android.apk && \
adb shell am force-stop com.example.tetris_android && \
adb shell am start -n com.example.tetris_android/android.app.NativeActivity
```

### Advanced Debugging
```bash
# Attach debugger (requires debug build)
adb shell gdbserver :5039 --attach $(adb shell pidof com.example.tetris_android)

# Use strace to trace system calls (requires root)
adb shell strace -p $(adb shell pidof com.example.tetris_android)

# Check native libraries
adb shell cat /proc/$(adb shell pidof com.example.tetris_android)/maps | grep "\.so"

# Monitor file descriptor usage
adb shell ls -la /proc/$(adb shell pidof com.example.tetris_android)/fd/
```

### Building Commands
```bash
# Quick development build
x build --platform android --arch arm64 --format apk

# Release build (optimized)
x build --platform android --arch arm64 --format apk --release

# Build and install directly to device
x build --platform android --device <device-id> --format apk --release

# Alternative architectures
x build --platform android --arch arm --format apk --release      # ARM32
x build --platform android --arch x86_64 --format apk --release   # x86-64

# Generate AAB for Play Store
x build --platform android --arch arm64 --format aab --release
```

### Installation and Management
```bash
# Install APK to device
adb install -r ../target/x/release/android/tetris-android.apk

# Force stop current app instance
adb shell am force-stop com.example.tetris_android

# Start the app
adb shell am start -n com.example.tetris_android/android.app.NativeActivity

# Uninstall app
adb uninstall com.example.tetris_android

# Check app info
adb shell dumpsys package com.example.tetris_android
```

### Debugging and Logging
```bash
# View all logs from the app
adb logcat -s tetris_android

# View logs with timestamps
adb logcat -v time -s tetris_android

# Clear log buffer
adb logcat -c

# View system crash logs
adb logcat | grep -i "crash\|tombstone\|fatal"
```

### App Management
```bash
# List all packages containing "tetris"
adb shell pm list packages | grep tetris

# Get app process ID
adb shell pidof com.example.tetris_android

# Monitor app resource usage
adb shell top

# Check app permissions
adb shell dumpsys package com.example.tetris_android | grep permission
```

### File System Access
```bash
# Pull APK from device (if needed)
adb shell pm path com.example.tetris_android
adb pull /system/app/YourApp/YourApp.apk

# View app's data directory (requires root)
adb shell run-as com.example.tetris_android ls -la

# Check app size
adb shell du -h /data/app/com.example.tetris_android*
```

### Performance Profiling
```bash
# Monitor CPU usage
adb shell top -p $(adb shell pidof com.example.tetris_android)

# Memory usage
adb shell dumpsys meminfo com.example.tetris_android

# GPU/graphics info
adb shell dumpsys SurfaceFlinger

# Battery usage
adb shell dumpsys batterystats com.example.tetris_android
```

### Troubleshooting Specific Issues
```bash
# Check if app is running
adb shell ps | grep tetris

# Verify NDK libraries loaded
adb logcat | grep "dlopen\|lib.*\.so"

# Check for native crashes
adb logcat | grep "SIGSEGV\|SIGABRT\|backtrace"

# Monitor touch input
adb logcat | grep -i "touch\|motion\|input"

# Check display/graphics
adb logcat | grep -i "surface\|buffer\|renderer"
```

The Android target provides native mobile experience with the simplest build process! 