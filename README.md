# Video Saver (Fetchr)

Video Saver is a clean, modern media downloading and processing wrapper around `yt-dlp` and `ffmpeg` designed for creator workflows.

## Features

- **Multi-Platform Support**: Works on Windows, macOS, Linux, and Android.
- **Auto-Updates**: Automatically downloads and updates dependencies (`yt-dlp`, `ffmpeg`, and `ffprobe`) in the background if they are missing at startup, keeping the application ready out of the box.
- **Hardware Acceleration**: Dynamically detects the best H.264 hardware encoder on your system (e.g. `h264_videotoolbox`, `h264_nvenc`, `h264_amf`, or `h264_qsv`) for high-performance GPU transcoding.

---

## Desktop Setup & Running

1. **Install Frontend Dependencies**:
   ```bash
   npm install
   ```

2. **Run in Development Mode**:
   ```bash
   npm run tauri dev
   ```

3. **Build Desktop App**:
   ```bash
   npm run tauri build
   ```

---

## Android Cross-Compilation

To compile the Android app, ensure you have the Android SDK and NDK installed, and run:

1. **Install Android Target in Rust**:
   ```bash
   rustup target add aarch64-linux-android
   ```

2. **Build Android Package (APK)**:
   ```bash
   ANDROID_HOME=/opt/homebrew/share/android-commandlinetools \
   NDK_HOME=/opt/homebrew/share/android-commandlinetools/ndk/26.3.11579264 \
   npm run tauri android build -- --debug --target aarch64 --ci --apk
   ```

The compiled APK will be available in:
`src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`
