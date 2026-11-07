#!/usr/bin/env python3
"""Host-JVM regression tests for the real Msa.java, using fake Android and HTTPS.

Set JAVA_HOME and JSON_JAR (org.json 20240303); no device or account is used.
The JSON dependency can be fetched from Maven Central; its hash is pinned below.
"""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
jar = Path(os.environ["JSON_JAR"]).resolve()
assert hashlib.sha256(jar.read_bytes()).hexdigest() == "3cf6cd6892e32e2b4c1c39e0f52f5248a2f5b37646fdfbb79a66b46b618414ed", "Expected org.json 20240303"
java = Path(os.environ["JAVA_HOME"]) / "bin"
stubs = {
    "android/content/Context.java": """package android.content;
public class Context {
    private final java.io.File dir;
    public Context(java.io.File dir) { this.dir = dir; }
    public java.io.File getFilesDir() { return dir; }
}""",
    "android/os/Looper.java": """package android.os;
public class Looper { public static Looper getMainLooper() { return new Looper(); } }""",
    "android/os/Handler.java": """package android.os;
public class Handler {
    public static final java.util.concurrent.BlockingQueue<Runnable> tasks = new java.util.concurrent.LinkedBlockingQueue<>();
    public Handler(Looper looper) {}
    public boolean post(Runnable task) { tasks.add(task); return true; }
    public static void drain() { Runnable task; while ((task = tasks.poll()) != null) task.run(); }
}""",
    "dev/horizonlab/horizon/Ui.java": """package dev.horizonlab.horizon;
final class Ui { static String t(String ru, String en) { return en; } }""",
    "dev/horizonlab/horizon/HorizonActivity.java": """package dev.horizonlab.horizon;
final class HorizonActivity {
    static volatile int refreshed;
    static void nativeAccountRefreshed() {
        if (!Thread.holdsLock(Msa.class)) throw new AssertionError("Publication outside session lock");
        refreshed++;
    }
}""",
}
with tempfile.TemporaryDirectory(prefix="horizon-msa-tests-") as directory:
    root = Path(directory)
    sources = []
    for name, text in stubs.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        sources.append(str(path))
    sources += [str(HERE.parent / "java/dev/horizonlab/horizon/Msa.java"), str(HERE / "MsaRaceTest.java")]
    subprocess.run([str(java / "javac"), "--release", "8", "-classpath", str(jar), "-d", str(root), *sources], check=True)
    subprocess.run([str(java / "java"), "-ea", "-Djava.io.tmpdir=" + str(root), "-classpath", str(root) + os.pathsep + str(jar), "dev.horizonlab.horizon.MsaRaceTest"], check=True, timeout=60)
