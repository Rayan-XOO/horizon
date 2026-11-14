package dev.horizonlab.horizon;

import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Environment;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/**
 * Where the game folder is (saves, mods, logs, resource packs, the downloaded game files...).
 *
 * By default the app's own folder, Android/data/dev.horizonlab.horizon/files. With all-files
 * access the player can have it in a "Horizon" folder in the root of the phone's storage, next
 * to Download and Android, where any file manager reaches it. The choice is the internal file
 * home-path.txt (the folder's path), which the game's native side reads too (android_main.rs);
 * without the permission both sides use the app's folder, so the game never starts on a folder
 * it can't write.
 */
final class Storage {
    private Storage() {}

    static final String CHOICE = "home-path.txt";
    /** The player answered the first-start question (either way). */
    static final String ASKED = "storage-asked";

    static File appDir(Context c) {
        File dir = c.getExternalFilesDir(null);
        if (dir == null) dir = c.getFilesDir();
        dir.mkdirs();
        return dir;
    }

    static File sharedDir() {
        return new File(Environment.getExternalStorageDirectory(), "Horizon");
    }

    /** All-files access (Android 11+), or the storage permission before it. */
    static boolean granted(Context c) {
        if (android.os.Build.VERSION.SDK_INT >= 30) return Environment.isExternalStorageManager();
        return c.checkSelfPermission(android.Manifest.permission.WRITE_EXTERNAL_STORAGE)
                == android.content.pm.PackageManager.PERMISSION_GRANTED;
    }

    /** The shared folder was chosen (it may lack the permission now). */
    static File chosen(Context c) {
        try {
            String p = new String(Files.readAllBytes(new File(c.getFilesDir(), CHOICE).toPath()), StandardCharsets.UTF_8).trim();
            return p.isEmpty() ? null : new File(p);
        } catch (Exception e) {
            return null;
        }
    }

    static boolean usable(File dir) {
        dir.mkdirs();
        if (!dir.isDirectory()) return false;
        File probe = new File(dir, ".horizon-write-test");
        try (FileOutputStream out = new FileOutputStream(probe)) {
            out.write(1);
        } catch (Exception e) {
            return false;
        }
        probe.delete();
        return true;
    }

    /** The game folder now. */
    static File home(Context c) {
        File shared = chosen(c);
        if (shared != null && granted(c) && usable(shared)) return shared;
        return appDir(c);
    }

    static boolean inShared(Context c) {
        return !home(c).equals(appDir(c));
    }

    static void choose(Context c, File dir) throws IOException {
        File f = new File(c.getFilesDir(), CHOICE);
        if (dir == null) {
            f.delete();
            return;
        }
        try (FileOutputStream out = new FileOutputStream(f)) {
            out.write((dir.getAbsolutePath() + "\n").getBytes(StandardCharsets.UTF_8));
        }
    }

    static void markAsked(Context c) {
        try {
            new File(c.getFilesDir(), ASKED).createNewFile();
        } catch (Exception ignored) {
        }
    }

    static boolean asked(Context c) {
        return new File(c.getFilesDir(), ASKED).isFile();
    }

    /** The system screen that grants all-files access to this app. */
    static Intent permissionIntent(Context c) {
        if (android.os.Build.VERSION.SDK_INT >= 30) {
            Intent i = new Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION);
            i.setData(Uri.parse("package:" + c.getPackageName()));
            return i;
        }
        return null;
    }

    interface Progress {
        void moved(long files, String name);
    }

    /**
     * Moves everything in `from` into `to`, keeping what `to` already has (a file already there
     * stays and the old copy is left where it was, so nothing is lost either way). A rename
     * when the system allows it, else a copy that is checked by size before the original goes.
     */
    static long move(File from, File to, Progress progress) throws IOException {
        long[] count = {0};
        moveInto(from, to, progress, count);
        return count[0];
    }

    private static void moveInto(File from, File to, Progress progress, long[] count) throws IOException {
        File[] kids = from.listFiles();
        if (kids == null) return;
        if (!to.isDirectory() && !to.mkdirs()) throw new IOException("can't create " + to);
        for (File k : kids) {
            File target = new File(to, k.getName());
            if (k.isDirectory()) {
                if (!target.exists() && k.renameTo(target)) {
                    count[0]++;
                    progress.moved(count[0], k.getName());
                    continue;
                }
                moveInto(k, target, progress, count);
                k.delete();
            } else if (!target.exists()) {
                if (!k.renameTo(target)) {
                    copy(k, target);
                    if (target.length() != k.length()) throw new IOException("short copy of " + k);
                    k.delete();
                }
                count[0]++;
                if (count[0] % 25 == 0) progress.moved(count[0], k.getName());
            }
        }
    }

    private static void copy(File from, File to) throws IOException {
        File tmp = new File(to.getPath() + ".part");
        try (InputStream in = new FileInputStream(from); OutputStream out = new FileOutputStream(tmp)) {
            byte[] buf = new byte[1 << 16];
            int n;
            while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
        }
        if (!tmp.renameTo(to)) throw new IOException("rename " + tmp);
    }
}
