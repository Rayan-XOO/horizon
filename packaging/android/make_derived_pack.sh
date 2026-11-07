#!/usr/bin/env bash
# Packs what only the Java exporters make (everything in a prepared folder that the phone cannot
# take straight from Mojang's client JAR or asset index) into horizon-derived.zip, for the
# first-run setup (crates/horizon-client/src/android_setup.rs). Not for publishing: the files
# are derived from Mojang's game. Usage: make_derived_pack.sh [PREPARED_DIR] [OUT.zip]
set -euo pipefail
cd "$(dirname "$0")/../.."
SRC="${1:-.local/horizon-client/26.3}"
OUT="$(realpath -m "${2:-target/android/horizon-derived.zip}")"
python3 - "$SRC" "$OUT" <<'PY'
import os, sys, zipfile
src, out = sys.argv[1], sys.argv[2]
# Kept in step with horizon_prepare::device::JAR_OUTPUTS, plus the asset index.
skip = {"assets", "data", "pack.png", "asset-index.json", "block-tags.json", "damage-types.json",
        "damage-tags.json", "worldgen/structure", "classes", "client-classes"}
skip |= {"worldgen/%s.json" % n for n in "noises densities rules conditions settings carvers biomes world_presets features placed_features state_providers biome_tags feature_tags block_tags fluid_tags processor_lists template_pools structures structure_sets".split()}
os.makedirs(os.path.dirname(out), exist_ok=True)
n = 0
with zipfile.ZipFile(out + ".tmp", "w", zipfile.ZIP_DEFLATED, compresslevel=6) as z:
    for dirpath, dirs, files in os.walk(src):
        rel = os.path.relpath(dirpath, src).replace(os.sep, "/")
        rel = "" if rel == "." else rel + "/"
        dirs[:] = sorted(d for d in dirs if (rel + d) not in skip)
        for f in sorted(files):
            r = rel + f
            if r in skip or f.endswith(".log"):
                continue
            z.write(os.path.join(dirpath, f), r)
            n += 1
os.replace(out + ".tmp", out)
print(f"{out}: {n} files, {os.path.getsize(out) >> 20} MB")
PY
