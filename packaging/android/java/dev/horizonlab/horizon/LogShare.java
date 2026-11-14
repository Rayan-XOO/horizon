package dev.horizonlab.horizon;

import android.app.Activity;
import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Intent;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.Build;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.Arrays;
import java.util.Comparator;
import java.util.Date;
import java.util.Locale;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/**
 * The game's logs in one zip for a bug report: logs/latest.log, the last few older logs, the
 * last crash reports, the graphics settings and a short note on the phone. "Share" hands it to
 * any app (Telegram, mail...) through {@link Provider}; "Save" puts a copy in Download/Horizon.
 * No account data goes in.
 */
final class LogShare {
    private LogShare() {}

    static final String AUTHORITY = "dev.horizonlab.horizon.logs";

    static File build(Activity a) throws Exception {
        File home = Storage.home(a);
        File dir = new File(a.getCacheDir(), "logs-share");
        dir.mkdirs();
        File[] old = dir.listFiles();
        if (old != null) for (File f : old) f.delete();
        String stamp = new SimpleDateFormat("yyyy-MM-dd_HH-mm", Locale.ROOT).format(new Date());
        File zip = new File(dir, "horizon-logs-" + stamp + ".zip");
        try (ZipOutputStream out = new ZipOutputStream(new FileOutputStream(zip))) {
            add(out, new File(home, "logs/latest.log"), "logs/latest.log");
            for (File f : newest(new File(home, "logs"), 5, "latest.log")) add(out, f, "logs/" + f.getName());
            for (File f : newest(new File(home, "crash-reports"), 5, null)) add(out, f, "crash-reports/" + f.getName());
            add(out, new File(home, "horizon-graphics.txt"), "horizon-graphics.txt");
            add(out, new File(a.getFilesDir(), "safe-mode.txt"), "safe-mode.txt");
            out.putNextEntry(new ZipEntry("device.txt"));
            out.write(device(a).getBytes(StandardCharsets.UTF_8));
            out.closeEntry();
        }
        return zip;
    }

    private static String device(Activity a) {
        String version = "?";
        try {
            version = a.getPackageManager().getPackageInfo(a.getPackageName(), 0).versionName;
        } catch (Exception ignored) {
        }
        return "Horizon " + version + "\n"
                + "Device: " + Build.MANUFACTURER + " " + Build.MODEL + " (" + Build.DEVICE + ")\n"
                + "Android: " + Build.VERSION.RELEASE + " (API " + Build.VERSION.SDK_INT + ")\n"
                + "ABIs: " + Arrays.toString(Build.SUPPORTED_ABIS) + "\n"
                + "SoC: " + (Build.VERSION.SDK_INT >= 31 ? Build.SOC_MANUFACTURER + " " + Build.SOC_MODEL : Build.HARDWARE) + "\n"
                + "Game folder in shared storage: " + Storage.inShared(a) + "\n";
    }

    private static File[] newest(File dir, int n, String skip) {
        File[] all = dir.listFiles();
        if (all == null) return new File[0];
        Arrays.sort(all, new Comparator<File>() {
            public int compare(File x, File y) {
                return Long.compare(y.lastModified(), x.lastModified());
            }
        });
        java.util.List<File> out = new java.util.ArrayList<>();
        for (File f : all) {
            if (!f.isFile() || f.getName().equals(skip)) continue;
            out.add(f);
            if (out.size() == n) break;
        }
        return out.toArray(new File[0]);
    }

    private static void add(ZipOutputStream out, File f, String name) throws Exception {
        if (!f.isFile()) return;
        out.putNextEntry(new ZipEntry(name));
        try (InputStream in = new FileInputStream(f)) {
            byte[] buf = new byte[1 << 15];
            int n;
            while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
        }
        out.closeEntry();
    }

    /** Opens the system share sheet with the zip. */
    static void share(Activity a) throws Exception {
        File zip = build(a);
        Uri uri = Uri.parse("content://" + AUTHORITY + "/" + zip.getName());
        Intent send = new Intent(Intent.ACTION_SEND);
        send.setType("application/zip");
        send.putExtra(Intent.EXTRA_STREAM, uri);
        send.putExtra(Intent.EXTRA_SUBJECT, zip.getName());
        send.setClipData(android.content.ClipData.newRawUri(zip.getName(), uri));
        send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        a.startActivity(Intent.createChooser(send, Ui.t("Поделиться логами", "Share logs")));
    }

    /** Copies the zip to Download/Horizon; returns where it went. */
    static String saveToDownloads(Activity a) throws Exception {
        File zip = build(a);
        if (Build.VERSION.SDK_INT >= 29) {
            ContentValues v = new ContentValues();
            v.put(android.provider.MediaStore.MediaColumns.DISPLAY_NAME, zip.getName());
            v.put(android.provider.MediaStore.MediaColumns.MIME_TYPE, "application/zip");
            v.put(android.provider.MediaStore.MediaColumns.RELATIVE_PATH, "Download/Horizon");
            Uri item = a.getContentResolver().insert(android.provider.MediaStore.Downloads.EXTERNAL_CONTENT_URI, v);
            if (item == null) throw new Exception("MediaStore");
            try (OutputStream out = a.getContentResolver().openOutputStream(item); InputStream in = new FileInputStream(zip)) {
                byte[] buf = new byte[1 << 15];
                int n;
                while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
            }
        } else {
            File dir = new File(android.os.Environment.getExternalStoragePublicDirectory(android.os.Environment.DIRECTORY_DOWNLOADS), "Horizon");
            dir.mkdirs();
            try (InputStream in = new FileInputStream(zip); OutputStream out = new FileOutputStream(new File(dir, zip.getName()))) {
                byte[] buf = new byte[1 << 15];
                int n;
                while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
            }
        }
        return "Download/Horizon/" + zip.getName();
    }

    /** Serves the zips in cache/logs-share to the app the player shares them with. */
    public static final class Provider extends ContentProvider {
        @Override
        public boolean onCreate() {
            return true;
        }

        private File file(Uri uri) {
            String name = uri.getLastPathSegment();
            if (name == null || name.contains("/") || name.contains("..")) return null;
            File f = new File(new File(getContext().getCacheDir(), "logs-share"), name);
            return f.isFile() ? f : null;
        }

        @Override
        public Cursor query(Uri uri, String[] projection, String selection, String[] args, String sort) {
            File f = file(uri);
            if (f == null) return null;
            MatrixCursor c = new MatrixCursor(new String[] {OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE});
            c.addRow(new Object[] {f.getName(), f.length()});
            return c;
        }

        @Override
        public String getType(Uri uri) {
            return "application/zip";
        }

        @Override
        public ParcelFileDescriptor openFile(Uri uri, String mode) throws java.io.FileNotFoundException {
            File f = file(uri);
            if (f == null || !"r".equals(mode)) throw new java.io.FileNotFoundException(uri.toString());
            return ParcelFileDescriptor.open(f, ParcelFileDescriptor.MODE_READ_ONLY);
        }

        @Override public Uri insert(Uri uri, ContentValues values) { return null; }
        @Override public int delete(Uri uri, String s, String[] a) { return 0; }
        @Override public int update(Uri uri, ContentValues v, String s, String[] a) { return 0; }
    }
}
