# Video Saver (Fetchr)

Video Saver is a clean, modern media downloading and processing wrapper around `yt-dlp` and `ffmpeg` designed for creator workflows.

## Features

- **Multi-Platform Support**: Works on Windows, macOS, Linux, and Android (WIP).
- **Auto-Updates (Desktop Only)**: Automatically downloads and updates dependencies (`yt-dlp`, `ffmpeg`, and `ffprobe`) in the background if they are missing at startup, keeping the application ready out of the box.
- **Lightweight Android Architecture (WIP)**: Bypasses Android's native execution restrictions by substituting heavy command-line tool dependencies (`yt-dlp` and `ffmpeg`) with oEmbed scanning APIs and direct HTTP downloads via `reqwest` for a lightweight and secure mobile application.
- **Hardware Acceleration (Desktop Only)**: Dynamically detects the best H.264 hardware encoder on your system (e.g. `h264_videotoolbox`, `h264_nvenc`, `h264_amf`, or `h264_qsv`) for high-performance GPU transcoding.


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

## Android Cross-Compilation (Work-in-Progress)

> [!NOTE]
> The mobile client is a Work-In-Progress (WIP). Running command-line tools like `yt-dlp` and `ffmpeg` via subprocesses is blocked on Android due to system security and SELinux restrictions. Instead, Fetchr Android is built as a lightweight client that bypasses command-line tools entirely, falling back to direct HTTP stream scraping and oEmbed metadata retrieval.

To compile the Android app, ensure you have the Android SDK and NDK installed, and run:

1. **Install Android Target in Rust**:
   ```bash
   rustup target add aarch64-linux-android
   ```

2. **Build Android Package (APK)**:

   - **Debug Build** (creates a larger binary with unstripped symbols, around ~178MB):
     ```bash
     ANDROID_HOME=/opt/homebrew/share/android-commandlinetools \
     NDK_HOME=/opt/homebrew/share/android-commandlinetools/ndk/26.3.11579264 \
     npm run tauri android build -- --debug --target aarch64 --ci --apk
     ```
     The debug APK is output to:
     `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`

   - **Release Build** (fully optimized and stripped, resulting in a lightweight package of around ~15-20MB):
     ```bash
     ANDROID_HOME=/opt/homebrew/share/android-commandlinetools \
     NDK_HOME=/opt/homebrew/share/android-commandlinetools/ndk/26.3.11579264 \
     npm run tauri android build -- --target aarch64 --ci --apk
     ```
     The release APK is output to:
     `src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`
