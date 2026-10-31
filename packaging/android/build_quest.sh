#!/usr/bin/env bash
# Builds target/android/horizon-quest.apk: the Meta Quest build (package dev.horizonlab.horizon.vr,
# the `vr` feature), with the Khronos OpenXR loader from Maven Central. Same signing as build_apk.sh.
set -euo pipefail
cd "$(dirname "$0")/../.."
LOADER_VERSION="${OPENXR_LOADER_VERSION:-1.1.65}"
LIBS=target/android/openxr/lib
if [ ! -f "$LIBS/libopenxr_loader.so" ]; then
  mkdir -p target/android/openxr "$LIBS"
  AAR=target/android/openxr/openxr_loader.aar
  curl -fsSL -o "$AAR" \
    "https://repo1.maven.org/maven2/org/khronos/openxr/openxr_loader_for_android/$LOADER_VERSION/openxr_loader_for_android-$LOADER_VERSION.aar"
  unzip -o -j -q "$AAR" "jni/arm64-v8a/libopenxr_loader.so" -d "$LIBS"
fi
MANIFEST=packaging/android/vr/AndroidManifest.xml EXTRA_LIBS="$LIBS" \
  CARGO_FLAGS="--features vr ${CARGO_FLAGS:-}" bash packaging/android/build_apk.sh
mv target/android/horizon.apk target/android/horizon-quest.apk
ls -l target/android/horizon-quest.apk
