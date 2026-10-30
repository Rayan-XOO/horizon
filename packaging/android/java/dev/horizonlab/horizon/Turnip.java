package dev.horizonlab.horizon;

import android.content.Context;
import android.net.Uri;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/** A user-supplied Vulkan driver (Turnip / freedreno) in internal storage. */
final class Turnip {
    private static final long MAX = 256L * 1024 * 1024;

    private Turnip() {}

    static File root(Context c) {
        File dir = new File(c.getFilesDir(), "turnip");
        dir.mkdirs();
        return dir;
    }

    static File choice(Context c) {
        return new File(c.getFilesDir(), "vulkan-driver.txt");
    }

    /** Absolute path of the selected .so, or empty for the system driver. */
    static String selected(Context c) {
        try {
            byte[] b = java.nio.file.Files.readAllBytes(choice(c).toPath());
            String p = new String(b, StandardCharsets.UTF_8).trim();
            return new File(p).isFile() ? p : "";
        } catch (Exception e) {
            return "";
        }
    }

    static String status(Context c) {
        String p = selected(c);
        if (p.isEmpty()) return Ui.t("Сейчас: системный драйвер Qualcomm", "Now: the system Qualcomm driver");
        return Ui.t("Сейчас: ", "Now: ") + new File(p).getName();
    }

    static void useSystem(Context c) {
        choice(c).delete();
    }

    /**
     * Unpacks a Turnip zip into internal storage and selects its Vulkan .so.
     * Internal storage is required: Android refuses to dlopen a library on shared storage.
     */
    static String install(Context c, Uri uri) throws Exception {
        File dest = new File(root(c), "driver-" + Integer.toHexString(uri.toString().hashCode()));
        if (dest.exists()) deleteRec(dest);
        dest.mkdirs();
        long total = 0;
        File vulkan = null;
        try (InputStream in = c.getContentResolver().openInputStream(uri);
             ZipInputStream zip = new ZipInputStream(in)) {
            if (in == null) throw new Exception("open");
            ZipEntry e;
            byte[] buf = new byte[65536];
            while ((e = zip.getNextEntry()) != null) {
                if (e.isDirectory()) continue;
                String name = e.getName().replace('\\', '/');
                int slash = name.lastIndexOf('/');
                String base = slash >= 0 ? name.substring(slash + 1) : name;
                if (base.isEmpty() || base.contains("..") || name.contains("..")) continue;
                if (!base.endsWith(".so")) continue;
                File out = new File(dest, base);
                try (FileOutputStream fos = new FileOutputStream(out)) {
                    int n;
                    while ((n = zip.read(buf)) > 0) {
                        total += n;
                        if (total > MAX) throw new Exception("too big");
                        fos.write(buf, 0, n);
                    }
                }
                if (vulkan == null || rank(base) > rank(vulkan.getName())) vulkan = out;
            }
        }
        if (vulkan == null) {
            deleteRec(dest);
            throw new Exception("no so");
        }
        byte[] text = (vulkan.getAbsolutePath() + "\n").getBytes(StandardCharsets.UTF_8);
        File tmp = new File(choice(c).getPath() + ".tmp");
        try (FileOutputStream fos = new FileOutputStream(tmp)) {
            fos.write(text);
        }
        if (!tmp.renameTo(choice(c))) throw new Exception("rename");
        return vulkan.getName();
    }

    private static int rank(String name) {
        String n = name.toLowerCase();
        if (n.contains("freedreno") || n.contains("turnip")) return 3;
        if (n.equals("libvulkan.so")) return 2;
        if (n.contains("vulkan")) return 1;
        return 0;
    }

    private static void deleteRec(File f) {
        File[] kids = f.listFiles();
        if (kids != null) for (File k : kids) deleteRec(k);
        f.delete();
    }
}
