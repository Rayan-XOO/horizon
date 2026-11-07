#!/usr/bin/env bash
# Builds target/android/horizon.apk (arm64-v8a). Signed with $HORIZON_KEYSTORE if set, else the debug key. See ANDROID.md.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}"
NDK="${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -n1)}"
export ANDROID_NDK_HOME="$NDK"
BT="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -n1)"
PLATFORM="$ANDROID_HOME/platforms/android-35/android.jar"
PROFILE="${PROFILE:-fast}"
OUT=target/android
rm -rf "$OUT/stage" && mkdir -p "$OUT/stage/lib"
cargo ndk -t arm64-v8a -P 26 -o "$OUT/jni" build -p horizon-android --profile "$PROFILE" ${CARGO_FLAGS:-}
mkdir -p "$OUT/stage/lib/arm64-v8a"
"$NDK"/toolchains/llvm/prebuilt/*/bin/llvm-strip -o "$OUT/stage/lib/arm64-v8a/libhorizon.so" "$OUT/jni/arm64-v8a/libhorizon.so"
# HorizonActivity (the soft keyboard bridge) -> classes.dex.
rm -rf "$OUT/java" && mkdir -p "$OUT/java/classes"
"${JAVAC:-javac}" --release 8 -nowarn -classpath "$PLATFORM" -d "$OUT/java/classes" \
  $(find packaging/android/java -name '*.java') 2>&1 | grep -v "^warning\|^Note\|^1 warning" || true
"$BT/d8" --min-api 26 --lib "$PLATFORM" --output "$OUT/java" $(find "$OUT/java/classes" -name '*.class')
rm -rf "$OUT/res" && mkdir -p "$OUT/res"
"$BT/aapt2" compile --dir packaging/android/res -o "$OUT/res/res.zip"
"$BT/aapt2" link -o "$OUT/unaligned.apk" --manifest packaging/android/AndroidManifest.xml -I "$PLATFORM" \
  --min-sdk-version 26 --target-sdk-version 35 "$OUT/res/res.zip"
python3 - "$OUT" <<'PY'
import sys, zipfile
out = sys.argv[1]
with zipfile.ZipFile(f"{out}/unaligned.apk", "a", zipfile.ZIP_DEFLATED) as z:
    z.write(f"{out}/java/classes.dex", "classes.dex")
    z.write(f"{out}/stage/lib/arm64-v8a/libhorizon.so", "lib/arm64-v8a/libhorizon.so")
PY
"$BT/zipalign" -f -p 4 "$OUT/unaligned.apk" "$OUT/aligned.apk"
if [ -n "${HORIZON_KEYSTORE:-}" ] && [ -f "$HORIZON_KEYSTORE" ]; then
  : "${HORIZON_KEYSTORE_PASS:?set HORIZON_KEYSTORE_PASS for $HORIZON_KEYSTORE}"
  SIGN=(--ks "$HORIZON_KEYSTORE" --ks-pass env:HORIZON_KEYSTORE_PASS --key-pass env:HORIZON_KEYSTORE_PASS)
  [ -n "${HORIZON_KEY_ALIAS:-}" ] && SIGN+=(--ks-key-alias "$HORIZON_KEY_ALIAS")
else
  echo "HORIZON_KEYSTORE not set: signing with the debug key" >&2
  KS="$HOME/.android/debug.keystore"
  if [ ! -f "$KS" ]; then
    mkdir -p "$(dirname "$KS")"
    keytool -genkeypair -keystore "$KS" -storepass android -keypass android -alias androiddebugkey \
      -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
  fi
  SIGN=(--ks "$KS" --ks-pass pass:android --key-pass pass:android)
fi
"$BT/apksigner" sign "${SIGN[@]}" --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true \
  --out "$OUT/horizon.apk" "$OUT/aligned.apk"
rm -f "$OUT/unaligned.apk" "$OUT/aligned.apk" "$OUT/horizon.apk.idsig"
ls -l "$OUT/horizon.apk"
