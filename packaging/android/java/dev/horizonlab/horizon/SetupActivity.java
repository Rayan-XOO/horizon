package dev.horizonlab.horizon;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.content.DialogInterface;
import android.content.Intent;
import android.net.ConnectivityManager;
import android.os.Bundle;
import android.os.Handler;
import android.os.StatFs;
import android.os.SystemClock;
import android.view.View;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.File;

/**
 * The launcher activity. Starts the game when the resources are ready; otherwise shows the
 * first-run screen that downloads and prepares the game files on the phone (android_setup.rs).
 * The derived pack comes from the build's HORIZON_DERIVED_URL or files/horizon-derived.zip.
 */
public class SetupActivity extends Activity
        implements View.OnClickListener, DialogInterface.OnClickListener, Runnable {
    static {
        System.loadLibrary("horizon");
    }

    static native int nativeReady(String home);
    static native String nativeDefaultUrl();
    static native void nativeStart(String home, String derivedUrl);
    static native String nativeStatus();

    private static final long NEED_BYTES = 1500L * 1024 * 1024;

    private String home;
    private TextView stage, detail, note;
    private Ui.Bar bar;
    private Button button, profile;
    private final Handler handler = new Handler();
    private long lastBytes = -1, lastTime;
    private double speed;
    private int failures;
    private String lastError = "";
    private Button copyError;

    static String home(Context c) {
        File dir = c.getExternalFilesDir(null);
        if (dir == null) dir = c.getFilesDir();
        dir.mkdirs();
        return dir.getAbsolutePath();
    }

    /** Whether the derived pack can be found (a build URL or the local zip). */
    private boolean haveDerived() {
        return !nativeDefaultUrl().isEmpty() || new File(home, "horizon-derived.zip").isFile();
    }

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        home = home(this);
        if (nativeReady(home) == 1) {
            launch();
            return;
        }
        LinearLayout card = Ui.card(this);
        card.addView(Ui.text(this, "Horizon", 32, Ui.TEXT, true));
        card.addView(Ui.text(this, Ui.t("Первый запуск", "First launch"), 15, Ui.MUTED, false));
        card.addView(Ui.text(this, Ui.t(
                "Игре нужны файлы Minecraft 26.3. Они скачаются автоматически, около 700 МБ. Лучше подключиться к Wi-Fi.",
                "The game needs the Minecraft 26.3 files. They will be downloaded automatically, about 700 MB. Wi-Fi is best."),
                15, Ui.TEXT, false), Ui.gap(this, 12));
        note = Ui.text(this, "", 14, Ui.WARN, false);
        note.setVisibility(View.GONE);
        card.addView(note, Ui.gap(this, 8));
        button = Ui.button(this, Ui.t("Скачать файлы игры", "Download game files"), true);
        button.setOnClickListener(this);
        card.addView(button, Ui.gap(this, 18));
        bar = new Ui.Bar(this);
        bar.setVisibility(View.GONE);
        card.addView(bar, Ui.gap(this, 18));
        copyError = Ui.button(this, Ui.t("Скопировать текст ошибки", "Copy the error text"), false);
        copyError.setVisibility(View.GONE);
        copyError.setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                android.content.ClipboardManager cm = (android.content.ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
                if (cm != null) cm.setPrimaryClip(android.content.ClipData.newPlainText("error",
                        "Horizon " + version() + " setup: " + lastError));
                copyError.setText(Ui.t("Скопировано ✓", "Copied ✓"));
            }
        });
        card.addView(copyError, Ui.gap(this, 8));
        stage = Ui.text(this, "", 15, Ui.TEXT, true);
        card.addView(stage, Ui.gap(this, 6));
        detail = Ui.text(this, "", 13, Ui.MUTED, false);
        card.addView(detail);
        Ui.section(card, "");
        Button turnip = Ui.button(this, Ui.t("Драйвер Vulkan (экспериментально)", "Vulkan driver (experimental)"), false);
        turnip.setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                startActivity(new Intent(SetupActivity.this, ProfileActivity.class));
            }
        });
        card.addView(turnip, Ui.gap(this, 8));
        profile = Ui.button(this, Ui.t("Профиль: ник и скин", "Profile: name and skin"), false);
        profile.setOnClickListener(this);
        card.addView(profile, Ui.gap(this, 14));
        if (!haveDerived()) {
            showNote(Ui.t(
                    "В этой сборке нет адреса пакета данных Horizon. Положите horizon-derived.zip в папку игры (Android/data/dev.horizonlab.horizon/files) или установите сборку, где он указан.",
                    "This build has no Horizon data pack address. Put horizon-derived.zip in the game folder (Android/data/dev.horizonlab.horizon/files) or install a build that has one."));
        }
        if (android.os.Build.VERSION.SDK_INT >= 33) {
            requestPermissions(new String[] {"android.permission.POST_NOTIFICATIONS"}, 1);
        }
        if (nativeStatus().contains("\trun\t")) {
            running();
        }
    }

    private void showNote(String s) {
        note.setText(s);
        note.setVisibility(View.VISIBLE);
    }

    public void onClick(View v) {
        if (v == profile) {
            startActivity(new Intent(this, ProfileActivity.class));
        } else {
            confirm();
        }
    }

    public void onClick(DialogInterface d, int w) {
        begin();
    }

    private void confirm() {
        StatFs fs = new StatFs(home);
        if (fs.getAvailableBytes() < NEED_BYTES) {
            showNote(Ui.t("Мало места. Нужно около 1,5 ГБ, свободно ", "Not enough space. About 1.5 GB is needed, free: ")
                    + (fs.getAvailableBytes() >> 20) + Ui.t(" МБ.", " MB."));
            return;
        }
        ConnectivityManager cm = (ConnectivityManager) getSystemService(Context.CONNECTIVITY_SERVICE);
        if (cm != null && cm.isActiveNetworkMetered()) {
            new AlertDialog.Builder(this)
                    .setTitle(Ui.t("Мобильный интернет", "Mobile data"))
                    .setMessage(Ui.t("Вы не подключены к Wi-Fi. Скачать около 700 МБ через мобильную сеть?",
                            "You are not on Wi-Fi. Download about 700 MB over mobile data?"))
                    .setPositiveButton(Ui.t("Скачать", "Download"), this)
                    .setNegativeButton(Ui.t("Отмена", "Cancel"), null)
                    .show();
        } else {
            begin();
        }
    }

    private void begin() {
        Intent service = new Intent(this, SetupService.class);
        service.putExtra("home", home);
        service.putExtra("url", nativeDefaultUrl());
        if (android.os.Build.VERSION.SDK_INT >= 26) {
            startForegroundService(service);
        } else {
            startService(service);
        }
        running();
    }

    private void running() {
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        button.setEnabled(false);
        button.setAlpha(0.5f);
        bar.setVisibility(View.VISIBLE);
        if (haveDerived()) note.setVisibility(View.GONE);
        lastBytes = -1;
        handler.removeCallbacks(this);
        handler.post(this);
    }

    static String stageName(String s) {
        if (s.equals("version") || s.equals("index")) return Ui.t("Подготовка", "Preparing");
        if (s.equals("client")) return Ui.t("Скачивание игры", "Downloading the game");
        if (s.equals("extract")) return Ui.t("Распаковка", "Unpacking");
        if (s.equals("assets")) return Ui.t("Звуки, шрифты и языки", "Sounds, fonts and languages");
        if (s.equals("derived")) return Ui.t("Данные Horizon", "Horizon data");
        if (s.equals("done")) return Ui.t("Готово", "Ready");
        return Ui.t("Подготовка", "Preparing");
    }

    /** A message a player can act on, from the native error text. */
    static String friendlyError(String e) {
        if (e.startsWith("No derived pack")) {
            return Ui.t("Не найден пакет данных Horizon. Положите horizon-derived.zip в папку игры и нажмите «Повторить».",
                    "The Horizon data pack was not found. Put horizon-derived.zip in the game folder and tap Retry.");
        }
        if (e.contains("status code 404") || e.contains("status code 410")) {
            return Ui.t("Файл на сервере не найден (404). Ссылка в этой сборке устарела: обновите Horizon.",
                    "A file is missing on the server (404). This build's link is out of date: update Horizon.");
        }
        if (e.contains("status code 403") || e.contains("status code 429")) {
            return Ui.t("Сервер не отдал файл (доступ ограничен). Подождите пару минут или смените сеть (VPN/мобильный интернет).",
                    "The server refused the file. Wait a few minutes or switch networks (VPN or mobile data).");
        }
        if (e.contains("status code 5")) {
            return Ui.t("Сервер загрузки временно недоступен. Попробуйте позже.", "The download server is down for now. Try again later.");
        }
        if (e.contains("certificate") || e.contains("TLS") || e.contains("tls")) {
            return Ui.t("Защищённое соединение не установилось. Проверьте дату и время на телефоне или смените сеть.",
                    "A secure connection failed. Check the phone's date and time, or switch networks.");
        }
        if (e.contains("zip") || e.contains("Zip") || e.contains("archive")) {
            return Ui.t("Пакет данных повреждён. Удалите horizon-derived.zip из папки игры и нажмите «Повторить».",
                    "The data pack is damaged. Delete horizon-derived.zip from the game folder and tap Retry.");
        }
        if (e.contains("SHA-1")) {
            return Ui.t("Файл скачался с ошибкой. Нажмите «Повторить».", "A file arrived damaged. Tap Retry.");
        }
        if (e.contains("Dns") || e.contains("onnect") || e.contains("timed out") || e.contains("Network")) {
            return Ui.t("Нет связи с сервером. Проверьте интернет и нажмите «Повторить».",
                    "Could not reach the server. Check the connection and tap Retry.");
        }
        if (e.contains("No space") || e.contains("ENOSPC")) {
            return Ui.t("Закончилось место на телефоне.", "The phone ran out of space.");
        }
        return Ui.t("Что-то пошло не так. Нажмите «Повторить», загрузка продолжится с того же места.",
                "Something went wrong. Tap Retry, the download continues where it stopped.");
    }

    public void run() {
        String[] p = nativeStatus().split("\t", -1);
        if (p.length < 7) {
            handler.postDelayed(this, 300);
            return;
        }
        long files = Long.parseLong(p[1]), filesTotal = Long.parseLong(p[2]);
        long bytes = Long.parseLong(p[3]), bytesTotal = Long.parseLong(p[4]);
        long now = SystemClock.elapsedRealtime();
        if (lastBytes >= 0 && now > lastTime && bytes >= lastBytes) {
            double s = (bytes - lastBytes) * 1000.0 / (now - lastTime);
            speed = speed == 0 ? s : speed * 0.8 + s * 0.2;
        }
        lastBytes = bytes;
        lastTime = now;
        int permille = 0;
        String line = "";
        if (bytesTotal > 0) {
            permille = (int) (1000 * bytes / Math.max(1, bytesTotal));
            line = (bytes >> 20) + " / " + (bytesTotal >> 20) + Ui.t(" МБ", " MB");
            if (speed > 1024) line += "  ·  " + String.format(java.util.Locale.ROOT, "%.1f", speed / 1048576) + Ui.t(" МБ/с", " MB/s");
        } else if (filesTotal > 0) {
            permille = (int) (1000 * files / Math.max(1, filesTotal));
            line = files + " / " + filesTotal + Ui.t(" файлов", " files");
        }
        bar.set(permille);
        stage.setText(stageName(p[0]) + "  " + (permille / 10) + "%");
        detail.setText(line);
        if (p[5].equals("done")) {
            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            launch();
            return;
        }
        if (p[5].equals("error")) {
            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            stage.setText(Ui.t("Загрузка остановилась", "The download stopped"));
            detail.setText("");
            failures++;
            lastError = p[6];
            String raw = p[6].replaceAll("https?://\\S+", "").trim();
            if (raw.length() > 160) raw = raw.substring(0, 160) + "…";
            showNote(friendlyError(p[6]) + (failures >= 2 && !raw.isEmpty() ? "\n\n" + Ui.t("Подробно: ", "Details: ") + raw : ""));
            if (failures >= 3) copyError.setVisibility(View.VISIBLE);
            button.setText(Ui.t("Повторить", "Retry") + (failures > 1 ? " (" + failures + ")" : ""));
            button.setEnabled(true);
            button.setAlpha(1f);
            return;
        }
        handler.postDelayed(this, 500);
    }

    private String version() {
        try {
            return getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Exception e) {
            return "?";
        }
    }

    private void launch() {
        startActivity(new Intent(this, HorizonActivity.class));
        finish();
    }

    @Override
    protected void onDestroy() {
        handler.removeCallbacks(this);
        super.onDestroy();
    }
}
