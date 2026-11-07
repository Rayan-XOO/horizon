//! The part of preparation that needs no Java: everything read straight out of the pinned client
//! JAR. Used on a device (Android's first-run download), where there is no JVM; the files are the
//! same bytes `prepare` writes for them.
use crate::*;

/// The files and folders [`from_client_jar`] writes, relative to the resources folder. Everything
/// else a prepared folder holds comes from the vanilla exporters (Java).
pub const JAR_OUTPUTS: &[&str] = &[
    "assets",
    "data",
    "pack.png",
    "block-tags.json",
    "damage-types.json",
    "damage-tags.json",
    "worldgen/structure",
    "worldgen/noises.json",
    "worldgen/densities.json",
    "worldgen/rules.json",
    "worldgen/conditions.json",
    "worldgen/settings.json",
    "worldgen/carvers.json",
    "worldgen/biomes.json",
    "worldgen/world_presets.json",
    "worldgen/features.json",
    "worldgen/placed_features.json",
    "worldgen/state_providers.json",
    "worldgen/biome_tags.json",
    "worldgen/feature_tags.json",
    "worldgen/block_tags.json",
    "worldgen/fluid_tags.json",
    "worldgen/processor_lists.json",
    "worldgen/template_pools.json",
    "worldgen/structures.json",
    "worldgen/structure_sets.json",
];

/// Writes the client JAR's assets, data and `pack.png`, the block and damage tag bundles and the
/// world generator's JSON bundles and structure templates into `root`. Returns the number of
/// resource files (the manifest's `resource_files`). Indexed assets already in `root` with the
/// asset index's SHA-1 (`indexed`: name -> sha1) are kept, as `prepare` keeps them.
pub fn from_client_jar(
    client: &Path,
    root: &Path,
    indexed: &dyn Fn(&str) -> Option<String>,
    progress: &dyn Fn(usize, usize),
) -> Result<usize, String> {
    let mut jar = open_zip(client)?;
    let names = zip_names(&jar);
    let total = names.len();
    let mut count = 0;
    for (i, name) in names.iter().enumerate() {
        if i % 256 == 0 {
            progress(i, total);
        }
        if !(name.starts_with("assets/") || name.starts_with("data/") || name == "pack.png")
            || name.ends_with('/')
        {
            continue;
        }
        let relative = member_path(name, "Invalid resource path")?;
        count += 1;
        let target = root.join(&relative);
        if let Some(asset) = name.strip_prefix("assets/") {
            if let Some(hash) = indexed(asset) {
                if target.is_file() && digest(&target, "sha1")? == hash {
                    continue;
                }
            }
        }
        let member = jar.by_name(name).map_err(|e| e.to_string())?;
        let size = member.size();
        drop(member);
        if target.is_file() && fs::metadata(&target).map_err(|e| e.to_string())?.len() == size {
            // Same size: compare the CRC as `prepare` does.
            let crc = jar.by_name(name).map_err(|e| e.to_string())?.crc32();
            let data = read(&target)?;
            if crc32fast::hash(&data) == crc {
                continue;
            }
        }
        mkdir(target.parent().unwrap())?;
        write(&target, &zip_bytes(&mut jar, name)?)?;
    }
    progress(total, total);
    write_json(
        &root.join("block-tags.json"),
        &json_resources(&mut jar, "data/minecraft/tags/block/")?,
    )?;
    write_json(
        &root.join("damage-types.json"),
        &json_resources(&mut jar, "data/minecraft/damage_type/")?,
    )?;
    write_json(
        &root.join("damage-tags.json"),
        &json_resources(&mut jar, "data/minecraft/tags/damage_type/")?,
    )?;
    let out = root.join("worldgen");
    mkdir(&out)?;
    let prefix = "data/minecraft/structure/";
    for name in &names {
        if name.starts_with(prefix) && name.ends_with(".nbt") {
            let relative = member_path(&name[prefix.len()..], "Invalid structure path")?;
            let data = zip_bytes(&mut jar, name)?;
            write_if_changed(&out.join("structure/minecraft").join(relative), &data)?;
        }
    }
    for (name, path) in WORLDGEN_BUNDLES {
        let resources = json_resources(&mut jar, &format!("data/minecraft/{path}/"))?;
        if resources.as_object().is_none_or(<[_]>::is_empty) {
            return Err(format!("The client JAR has no {path}"));
        }
        write_text(
            &out.join(format!("{name}.json")),
            &pyjson::dumps(&resources, &pyjson::COMPACT),
        )?;
    }
    Ok(count)
}
