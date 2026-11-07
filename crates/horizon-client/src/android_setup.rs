//! First run on Android: downloads and prepares the game files on the phone, without a PC or Java.
//!
//! The Mojang files come straight from Mojang (piston-meta / piston-data and
//! resources.download.minecraft.net), each checked by SHA-1, the client JAR against
//! research-lock.json's pin: the JAR's assets and data, the asset index and its sounds, fonts,
//! languages, panorama and built-in packs. What only the vanilla exporters (Java running Mojang's
//! code) can make comes from a "derived pack" zip the player points at (a URL or
//! `files/horizon-derived.zip`), made once by packaging/android/make_derived_pack.sh.
//! `SetupActivity.java` is the screen; it calls the `native*` functions below.
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use sha1::{Digest, Sha1};

const LOCK: &str = include_str!("../../../research-lock.json");
const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
/// Marks a resources folder whose setup has not finished; removed last.
const INCOMPLETE: &str = ".horizon-setup-incomplete";
const THREADS: usize = 8;

#[derive(Default, Clone)]
struct Status {
    stage: String,
    running: bool,
    done: bool,
    error: String,
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);
static FILES_DONE: AtomicUsize = AtomicUsize::new(0);
static FILES_TOTAL: AtomicUsize = AtomicUsize::new(0);
static BYTES_DONE: AtomicU64 = AtomicU64::new(0);
static BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);

fn set(f: impl FnOnce(&mut Status)) {
    let mut s = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(Status::default));
}
fn stage(text: &str) {
    log::info!("setup: {text}");
    set(|s| s.stage = text.into());
}
fn counters(files: usize, bytes: u64) {
    FILES_DONE.store(0, Ordering::SeqCst);
    BYTES_DONE.store(0, Ordering::SeqCst);
    FILES_TOTAL.store(files, Ordering::SeqCst);
    BYTES_TOTAL.store(bytes, Ordering::SeqCst);
}

/// Whether `resources` holds a finished preparation (made here or pushed from a PC).
pub fn ready(resources: &Path) -> bool {
    resources.join("horizon-resources.json").is_file()
        && resources.join("facts.json").is_file()
        && !resources.join(INCOMPLETE).exists()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn sha1_file(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hash = Sha1::new();
    let mut buf = vec![0; 1 << 16];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Some(hex(&hash.finalize()))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(20))
        .timeout_read(std::time::Duration::from_secs(60))
        .user_agent("Horizon-Android-Setup")
        .build()
}

fn get_bytes(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    agent
        .get(url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_reader()
        .read_to_end(&mut data)
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(data)
}

/// Downloads `url` to `target`, resuming a `.part` file, counting bytes; checks `sha1` if given.
fn download(agent: &ureq::Agent, url: &str, target: &Path, sha1: Option<&str>) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let part = target.with_extension("part");
    let mut last = String::new();
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(attempt * 2));
        }
        let have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let mut request = agent.get(url);
        if have > 0 {
            request = request.set("Range", &format!("bytes={have}-"));
        }
        let response = match request.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(416, _)) => {
                let _ = fs::remove_file(&part);
                last = format!("{url}: bad range");
                continue;
            }
            Err(e) => {
                last = format!("{url}: {e}");
                continue;
            }
        };
        let append = have > 0 && response.status() == 206;
        let mut file = match fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&part)
        {
            Ok(f) => f,
            Err(e) => return Err(format!("{}: {e}", part.display())),
        };
        if append {
            BYTES_DONE.fetch_add(have, Ordering::Relaxed);
        }
        let mut reader = response.into_reader();
        let mut buf = vec![0; 1 << 16];
        let mut got = 0u64;
        let result: Result<(), String> = loop {
            match reader.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(n) => {
                    if let Err(e) = file.write_all(&buf[..n]) {
                        break Err(format!("{}: {e}", part.display()));
                    }
                    got += n as u64;
                    BYTES_DONE.fetch_add(n as u64, Ordering::Relaxed);
                }
                Err(e) => break Err(format!("{url}: {e}")),
            }
        };
        drop(file);
        if let Err(e) = result {
            // Keep the partial file for the next try; take its bytes back off the counter.
            BYTES_DONE.fetch_sub(got + if append { have } else { 0 }, Ordering::Relaxed);
            last = e;
            continue;
        }
        if let Some(expected) = sha1 {
            if sha1_file(&part).as_deref() != Some(expected) {
                let _ = fs::remove_file(&part);
                BYTES_DONE.fetch_sub(got + if append { have } else { 0 }, Ordering::Relaxed);
                last = format!("{url}: SHA-1 mismatch");
                continue;
            }
        }
        fs::rename(&part, target).map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err(last)
}

/// The asset index entries the prepared folder copies (sounds, fonts, languages, panorama, packs).
fn wanted_asset(name: &str) -> bool {
    name == "pack.mcmeta"
        || name == "minecraft/sounds.json"
        || name.starts_with("minecraft/sounds/")
        || name.starts_with("minecraft/font/")
        || name.starts_with("minecraft/lang/")
        || name.starts_with("minecraft/resourcepacks/")
        || name.starts_with("minecraft/textures/gui/title/background/panorama_")
}

fn version_json(agent: &ureq::Agent, lock: &Value) -> Result<Value, String> {
    let version = lock["minecraft"]["version"].as_str().ok_or("lock: no version")?;
    let mut accepted: Vec<String> = vec![lock["minecraft"]["manifest_sha1"].as_str().unwrap_or("").into()];
    if let Some(list) = lock["minecraft"]["accepted_manifest_sha1"].as_array() {
        accepted.extend(list.iter().filter_map(|v| v.as_str().map(String::from)));
    }
    // The pinned version JSON first, then whatever Mojang lists now (the client JAR's pin
    // decides either way).
    for sha in &accepted {
        let url = format!("https://piston-meta.mojang.com/v1/packages/{sha}/{version}.json");
        if let Ok(data) = get_bytes(agent, &url) {
            if hex(&Sha1::digest(&data)) == *sha {
                return serde_json::from_slice(&data).map_err(|e| e.to_string());
            }
        }
    }
    let manifest: Value = serde_json::from_slice(&get_bytes(agent, MANIFEST)?).map_err(|e| e.to_string())?;
    let entry = manifest["versions"]
        .as_array()
        .and_then(|v| v.iter().find(|v| v["id"] == version))
        .ok_or_else(|| format!("Mojang lists no {version}"))?;
    let url = entry["url"].as_str().ok_or("no url")?;
    let data = get_bytes(agent, url)?;
    if Some(hex(&Sha1::digest(&data)).as_str()) != entry["sha1"].as_str() {
        return Err(format!("{url}: SHA-1 mismatch"));
    }
    serde_json::from_slice(&data).map_err(|e| e.to_string())
}

fn run(home: &Path, derived: &str) -> Result<(), String> {
    let lock: Value = serde_json::from_str(LOCK).map_err(|e| e.to_string())?;
    let resources = std::env::var_os("HORIZON_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("resources"));
    let work = home.join(".horizon-setup");
    fs::create_dir_all(&resources).map_err(|e| e.to_string())?;
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    fs::write(resources.join(INCOMPLETE), b"").map_err(|e| e.to_string())?;
    let agent = agent();

    stage("version");
    counters(1, 0);
    let version = version_json(&agent, &lock)?;
    let client = &version["downloads"]["client"];
    let client_sha1 = lock["minecraft"]["client_sha1"].as_str().ok_or("lock: no client_sha1")?;
    if client["sha1"].as_str() != Some(client_sha1) {
        return Err("Mojang's 26.3 client is not the pinned one".into());
    }
    let jar = work.join("client.jar");
    if sha1_file(&jar).as_deref() != Some(client_sha1) {
        stage("client");
        counters(1, client["size"].as_u64().unwrap_or(0));
        download(&agent, client["url"].as_str().ok_or("no client url")?, &jar, Some(client_sha1))?;
    }

    stage("index");
    let index_info = &version["assetIndex"];
    let index_path = resources.join("asset-index.json");
    let index_sha1 = index_info["sha1"].as_str().ok_or("no asset index sha1")?;
    if sha1_file(&index_path).as_deref() != Some(index_sha1) {
        download(&agent, index_info["url"].as_str().ok_or("no index url")?, &index_path, Some(index_sha1))?;
    }
    let index: Value = serde_json::from_slice(&fs::read(&index_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let objects: Vec<(String, String, u64)> = index["objects"]
        .as_object()
        .ok_or("asset index has no objects")?
        .iter()
        .filter(|(n, _)| wanted_asset(n))
        .filter_map(|(n, v)| Some((n.clone(), v["hash"].as_str()?.to_string(), v["size"].as_u64()?)))
        .filter(|(n, h, _)| h.len() == 40 && !n.contains(".."))
        .collect();

    stage("extract");
    let indexed: std::collections::HashMap<String, String> =
        objects.iter().map(|(n, h, _)| (n.clone(), h.clone())).collect();
    horizon_prepare::device::from_client_jar(
        &jar,
        &resources,
        &|name| indexed.get(name).cloned(),
        &|done, total| {
            FILES_TOTAL.store(total, Ordering::Relaxed);
            FILES_DONE.store(done, Ordering::Relaxed);
        },
    )?;

    // Assets: a verified list makes a resumed run skip hashing files it already checked.
    stage("assets");
    let done_path = work.join("objects.done");
    let verified: HashSet<String> = fs::read_to_string(&done_path)
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect();
    let total: u64 = objects.iter().map(|o| o.2).sum();
    counters(objects.len(), total);
    let mut queue = Vec::new();
    for (name, hash, size) in objects {
        let target = resources.join("assets").join(&name);
        let key = format!("{hash} {name}");
        if verified.contains(&key) && fs::metadata(&target).is_ok_and(|m| m.len() == size) {
            FILES_DONE.fetch_add(1, Ordering::Relaxed);
            BYTES_DONE.fetch_add(size, Ordering::Relaxed);
        } else {
            queue.push((target, hash, key));
        }
    }
    let queue = Arc::new(Mutex::new(queue));
    let done_file = Arc::new(Mutex::new(
        fs::OpenOptions::new().create(true).append(true).open(&done_path).map_err(|e| e.to_string())?,
    ));
    let failure: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let workers: Vec<_> = (0..THREADS)
        .map(|_| {
            let (queue, done_file, failure, agent) =
                (queue.clone(), done_file.clone(), failure.clone(), agent.clone());
            std::thread::spawn(move || loop {
                if failure.lock().unwrap().is_some() {
                    return;
                }
                let Some((target, hash, key)) = queue.lock().unwrap().pop() else {
                    return;
                };
                let ok = sha1_file(&target).as_deref() == Some(hash.as_str()) && {
                    let size = fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                    BYTES_DONE.fetch_add(size, Ordering::Relaxed);
                    true
                };
                if !ok {
                    let url = format!("https://resources.download.minecraft.net/{}/{hash}", &hash[..2]);
                    if let Err(e) = download(&agent, &url, &target, Some(&hash)) {
                        *failure.lock().unwrap() = Some(e);
                        return;
                    }
                }
                let _ = writeln!(done_file.lock().unwrap(), "{key}");
                FILES_DONE.fetch_add(1, Ordering::Relaxed);
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    if let Some(e) = failure.lock().unwrap().take() {
        return Err(e);
    }

    stage("derived");
    let local = home.join("horizon-derived.zip");
    let pack = if local.is_file() {
        local
    } else {
        let url = derived.trim();
        if url.is_empty() {
            return Err("No derived pack: put horizon-derived.zip in the app folder".into());
        }
        let target = work.join("derived.zip");
        if !target.is_file() {
            let size = agent.head(url).call().ok().and_then(|r| r.header("Content-Length")?.parse().ok());
            counters(1, size.unwrap_or(0));
            download(&agent, url, &target, None)?;
        }
        target
    };
    let file = fs::File::open(&pack).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| {
        let _ = fs::remove_file(work.join("derived.zip"));
        format!("derived pack: {e}")
    })?;
    counters(zip.len(), 0);
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(format!("derived pack: bad path {}", entry.name()));
        };
        let target = resources.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = target.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
        FILES_DONE.store(i + 1, Ordering::Relaxed);
    }
    if !resources.join("horizon-resources.json").is_file() || !resources.join("facts.json").is_file() {
        return Err("The derived pack lacks horizon-resources.json or facts.json".into());
    }
    fs::remove_file(resources.join(INCOMPLETE)).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&work);
    stage("done");
    Ok(())
}

fn jstring(env: &mut jni::JNIEnv, s: &jni::objects::JString) -> String {
    env.get_string(s).map(Into::into).unwrap_or_default()
}

/// 1 when the resources under `home` are ready to play.
#[unsafe(no_mangle)]
extern "system" fn Java_dev_horizonlab_horizon_SetupActivity_nativeReady(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    home: jni::objects::JString,
) -> jni::sys::jint {
    let home = PathBuf::from(jstring(&mut env, &home));
    ready(&home.join("resources")) as jni::sys::jint
}

/// The derived pack URL this build was made with (HORIZON_DERIVED_URL), else "".
#[unsafe(no_mangle)]
extern "system" fn Java_dev_horizonlab_horizon_SetupActivity_nativeDefaultUrl<'a>(
    env: jni::JNIEnv<'a>,
    _class: jni::objects::JClass,
) -> jni::sys::jstring {
    env.new_string(option_env!("HORIZON_DERIVED_URL").unwrap_or(""))
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Starts (or resumes) the setup on a background thread; does nothing while one runs.
#[unsafe(no_mangle)]
extern "system" fn Java_dev_horizonlab_horizon_SetupActivity_nativeStart(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    home: jni::objects::JString,
    url: jni::objects::JString,
) {
    let home = PathBuf::from(jstring(&mut env, &home));
    let url = jstring(&mut env, &url);
    {
        let mut s = STATUS.lock().unwrap_or_else(|e| e.into_inner());
        if s.as_ref().is_some_and(|s| s.running) {
            return;
        }
        *s = Some(Status { running: true, ..Status::default() });
    }
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| run(&home, &url))
            .unwrap_or_else(|_| Err("internal error".into()));
        if let Err(e) = &result {
            log::error!("setup: {e}");
        }
        set(|s| {
            s.running = false;
            s.done = result.is_ok();
            s.error = result.err().unwrap_or_default();
        });
    });
}

/// "stage\tfilesDone\tfilesTotal\tbytesDone\tbytesTotal\tstate\terror", state = run/done/error/idle.
#[unsafe(no_mangle)]
extern "system" fn Java_dev_horizonlab_horizon_SetupActivity_nativeStatus<'a>(
    env: jni::JNIEnv<'a>,
    _class: jni::objects::JClass,
) -> jni::sys::jstring {
    let s = STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
    let state = if s.running {
        "run"
    } else if s.done {
        "done"
    } else if !s.error.is_empty() {
        "error"
    } else {
        "idle"
    };
    let text = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        s.stage,
        FILES_DONE.load(Ordering::Relaxed),
        FILES_TOTAL.load(Ordering::Relaxed),
        BYTES_DONE.load(Ordering::Relaxed),
        BYTES_TOTAL.load(Ordering::Relaxed),
        state,
        s.error.replace('\t', " ")
    );
    env.new_string(text).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}
