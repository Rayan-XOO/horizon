package dev.horizonlab.horizon;

import android.app.Activity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.widget.Toast;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * Installs a mod: a .hnmod (or a .hmod content mod) opened with Horizon, shared to it,
 * or picked from the Profile screen. A .hnmod goes to files/mods/, a .hmod to
 * files/horizon-mods/; the game reads both when it starts (game/prebuilt_mods.rs).
 */
public class ModImportActivity extends Activity {
    private static final int PICK = 1;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Intent intent = getIntent();
        Uri uri = null;
        if (Intent.ACTION_SEND.equals(intent.getAction())) {
            uri = intent.getParcelableExtra(Intent.EXTRA_STREAM);
        } else if (Intent.ACTION_VIEW.equals(intent.getAction())) {
            uri = intent.getData();
        }
        if (uri != null) {
            install(uri);
            finish();
            return;
        }
        Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        pick.addCategory(Intent.CATEGORY_OPENABLE);
        pick.setType("*/*");
        startActivityForResult(pick, PICK);
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (result == RESULT_OK && data != null && data.getData() != null) install(data.getData());
        finish();
    }

    private void install(Uri uri) {
        String name = displayName(uri);
        String lower = name.toLowerCase(java.util.Locale.ROOT);
        String folder = lower.endsWith(".hnmod") ? "mods" : lower.endsWith(".hmod") ? "horizon-mods" : null;
        if (folder == null || !name.matches("[A-Za-z0-9._ ()+-]{1,120}")) {
            toast(Ui.t("Это не мод Horizon (.hnmod или .hmod): ", "Not a Horizon mod (.hnmod or .hmod): ") + name);
            return;
        }
        File dir = new File(SetupActivity.home(this), folder);
        dir.mkdirs();
        File out = new File(dir, name);
        File part = new File(dir, name + ".part");
        try (InputStream in = getContentResolver().openInputStream(uri);
             OutputStream o = new FileOutputStream(part)) {
            if (in == null) throw new java.io.IOException("no data");
            byte[] buf = new byte[1 << 16];
            long total = 0;
            for (int n; (n = in.read(buf)) > 0; ) {
                total += n;
                if (total > (64L << 20)) throw new java.io.IOException("over 64 MiB");
                o.write(buf, 0, n);
            }
        } catch (Exception e) {
            part.delete();
            toast(Ui.t("Не удалось установить мод: ", "Could not install the mod: ") + e.getMessage());
            return;
        }
        if (!part.renameTo(out)) {
            out.delete();
            part.renameTo(out);
        }
        toast(Ui.t("Мод установлен: ", "Mod installed: ") + name
                + Ui.t(". Перезапустите игру; он в Настройки > Моды.", ". Restart the game; it is in Options > Mods."));
    }

    private String displayName(Uri uri) {
        String name = null;
        try (Cursor c = getContentResolver().query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst()) name = c.getString(0);
        } catch (Exception ignored) {
        }
        if (name == null) name = uri.getLastPathSegment();
        if (name == null) name = "mod";
        int slash = name.lastIndexOf('/');
        return slash >= 0 ? name.substring(slash + 1) : name;
    }

    private void toast(String text) {
        Toast.makeText(getApplicationContext(), text, Toast.LENGTH_LONG).show();
    }
}
