#!/bin/sh
# muguet - make-android.sh
# Constantin Clerc (c22dev) - 0.1

set -eu
cd "$(dirname "$0")"

INSTALL=""
for a in "$@"; do
  case "$a" in
    --install) INSTALL=1 ;;
    *) echo "usage: ./make-android.sh [--install]"; exit 1 ;;
  esac
done

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$SDK" ]; then
  for d in "$HOME/Library/Android/sdk" "$HOME/Android/Sdk"; do [ -d "$d" ] && SDK="$d" && break; done
fi
[ -n "$SDK" ] || { echo "set ANDROID_HOME to your Android SDK"; exit 1; }
export ANDROID_HOME="$SDK"
NDK="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
[ -n "$NDK" ] || NDK="$(ls -d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -1)"
[ -n "$NDK" ] && [ -d "$NDK" ] || { echo "no Android NDK: install one from Android Studio's SDK Manager (or set ANDROID_NDK_HOME)"; exit 1; }
case "$(uname -s)" in
  Darwin) HOST=darwin-x86_64 ;;
  *) HOST=linux-x86_64 ;;
esac
BIN="$NDK/toolchains/llvm/prebuilt/$HOST/bin"
API=29

export CC_aarch64_linux_android="$BIN/aarch64-linux-android$API-clang"
export AR_aarch64_linux_android="$BIN/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
# 16 KB pages (Android 15+ devices)
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384 --remap-path-prefix=$HOME=/home --remap-path-prefix=$PWD=/muguet"

if command -v rustup >/dev/null; then rustup target add aarch64-linux-android >/dev/null; fi
cargo build --release --lib --target aarch64-linux-android --target-dir target/android

LIBS=target/android/jniLibs/arm64-v8a
mkdir -p "$LIBS"
"$BIN/llvm-strip" --strip-debug -o "$LIBS/libmuguet.so" target/android/aarch64-linux-android/release/libmuguet.so

(cd android && ./gradlew --quiet assembleRelease)
mkdir -p build
APK="$PWD/build/Muguet.apk"
cp android/app/build/outputs/apk/release/app-release.apk "$APK"
echo "built: $APK ($(du -h "$APK" | cut -f1))"

if [ -n "$INSTALL" ]; then
  ADB="$(command -v adb || echo "$SDK/platform-tools/adb")"
  "$ADB" install -r "$APK"
  echo "installed"
fi
