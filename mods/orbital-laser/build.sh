#!/bin/bash
# Builds orbital.hnmod: Orbital Laser 0.5.10 (Stink-O/orbital-laser, MIT) from its original source, as a prebuilt
# Horizon SDK mod for Android (arm64). Needs git, cargo-ndk and an Android NDK (ANDROID_NDK_HOME).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
TAG=v0.5.10
COMMIT=51468c5f69c8f61cb8982a44700853390eb23ed9
OUT=$HERE/target
SRC=$OUT/upstream
mkdir -p "$OUT"
if [ ! -d "$SRC/.git" ]; then
    git clone --quiet https://github.com/Stink-O/orbital-laser "$SRC"
fi
git -C "$SRC" fetch --quiet --tags origin || true
git -C "$SRC" checkout --quiet "$COMMIT"
[ "$(git -C "$SRC" rev-parse HEAD)" = "$COMMIT" ] || { echo "upstream $TAG is not $COMMIT" >&2; exit 1; }
# The mod crate as it is, with its SDK dependency on this repo's horizon-mod-sdk.
B=$OUT/build
rm -rf "$B" && mkdir -p "$B"
cp -r "$SRC/mod" "$B/mod"
sed -i "s|^horizon-mod-sdk = .*|horizon-mod-sdk = { path = \"$ROOT/crates/horizon-mod-sdk\" }|" "$B/mod/Cargo.toml"
printf '\n[workspace]\n' >> "$B/mod/Cargo.toml"
rm -f "$B/mod/Cargo.lock"
(cd "$B/mod" && cargo ndk -t arm64-v8a -P 26 build --release)
LIB=$B/mod/target/aarch64-linux-android/release/liborbital_laser.so
"$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*/bin/llvm-strip "$LIB"
# The bundle: the library, the content and the sounds, and the licence.
P=$OUT/pack
rm -rf "$P" && mkdir -p "$P/bin/aarch64-linux-android" "$P/mixins/orbital"
cp "$LIB" "$P/bin/aarch64-linux-android/liborbital.so"
cp "$SRC/orbital-laser-mod/horizon-mod.toml" "$P/"
cp -r "$SRC/orbital-laser-mod/assets" "$SRC/orbital-laser-mod/data" "$P/"
cp -r "$SRC/orbital-sounds/assets/." "$P/assets/"
cp "$HERE/LICENSE" "$P/mixins/orbital/LICENSE.txt"
cp "$HERE/NOTICE" "$P/mixins/orbital/NOTICE.txt"
HORIZON=$(git -C "$ROOT" rev-parse HEAD)
cat > "$P/horizon-native.json" <<JSON
{
  "schema": 1,
  "id": "orbital",
  "version": "0.5.10-android.2",
  "game": "26.3",
  "horizon": "$HORIZON",
  "mixins": "0000000000000000000000000000000000000000",
  "binaries": { "aarch64-linux-android": "bin/aarch64-linux-android/liborbital.so" }
}
JSON
rm -f "$OUT/orbital.hnmod"
python3 - "$P" "$OUT/orbital.hnmod" <<'PYZ'
import os, sys, zipfile
src, out = sys.argv[1], sys.argv[2]
names = sorted(os.path.relpath(os.path.join(d, f), src) for d, _, fs in os.walk(src) for f in fs)
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for n in names:
        info = zipfile.ZipInfo(n, (2026, 1, 1, 0, 0, 0))
        info.external_attr = 0o100644 << 16
        info.compress_type = zipfile.ZIP_STORED if n.endswith(".ogg") else zipfile.ZIP_DEFLATED
        with open(os.path.join(src, n), "rb") as f:
            z.writestr(info, f.read())
PYZ
ls -l "$OUT/orbital.hnmod"
