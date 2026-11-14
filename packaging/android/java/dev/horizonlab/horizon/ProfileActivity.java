package dev.horizonlab.horizon;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.text.InputFilter;
import android.view.Gravity;
import android.view.View;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/**
 * The launcher, landscape and on one screen: the skin with its picker and arm model, the
 * name, the account, a big Play button and every setting on the right.
 *
 * The name goes to horizon-player.txt (offline_name.rs: the name used on servers and LAN);
 * the skin, a PNG picked with the system file picker, to horizon-skin.png as a plain 64x64
 * or 64x32 RGBA PNG, and its arm model to horizon-skin.txt (game/local_skin.rs).
 */
public class ProfileActivity extends Activity implements View.OnClickListener {
    private static final int PICK = 7;
    private static final int PICK_DRIVER = 8;
    private static final int MS_LOGIN = 9;
    private File dir;
    private EditText name;
    private TextView message;
    private SkinView preview;
    private Button pick, model, play, driver, systemDriver, microsoft, deviceCode, signOut;
    private TextView driverStatus, accountStatus, accountChip;
    private EditText clientId;
    private LinearLayout codeBox, advancedBox;
    private TextView code, codeHint;
    private Button copy, browser, advanced, mods;
    private Button settingsTab, betaTab, folder, via;
    private View settingsPage, betaPage;
    private TextView folderStatus;
    private boolean waitingAccess;
    private String verifyUri = "https://www.microsoft.com/link";
    private boolean slim;

    private static final int PANEL = Color.argb(176, 0, 0, 0);
    private static final int FIELD = Color.rgb(0, 0, 0);
    private static final int RIM = Color.rgb(90, 90, 90);

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        dir = new File(SetupActivity.home(this));
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_PAN
                | WindowManager.LayoutParams.SOFT_INPUT_STATE_HIDDEN);
        int p = Ui.dp(this, 12);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.HORIZONTAL);
        int pad = Ui.dp(this, 16);
        root.setPadding(pad, pad, pad, pad);

        // Left: who plays.
        LinearLayout left = new LinearLayout(this);
        left.setOrientation(LinearLayout.VERTICAL);
        left.setPadding(p, p, p, p);
        left.setBackground(Ui.rounded(this, PANEL, RIM, 0));
        LinearLayout head = new LinearLayout(this);
        head.setOrientation(LinearLayout.HORIZONTAL);
        head.setGravity(Gravity.CENTER_VERTICAL);
        TextView title = Ui.text(this, "HORIZON", 24, Ui.TEXT, true);
        title.setLetterSpacing(0.08f);
        head.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
        TextView ver = Ui.text(this, version(), 12, Ui.MUTED, false);
        head.addView(ver);
        left.addView(head);

        LinearLayout who = new LinearLayout(this);
        who.setOrientation(LinearLayout.HORIZONTAL);
        preview = new SkinView(this);
        preview.setBackground(Ui.rounded(this, Color.argb(120, 0, 0, 0), 0, 0));
        int pv = Ui.dp(this, 6);
        preview.setPadding(pv, pv, pv, pv);
        who.addView(preview, new LinearLayout.LayoutParams(0, -1, 1));
        LinearLayout side = new LinearLayout(this);
        side.setOrientation(LinearLayout.VERTICAL);
        side.setGravity(Gravity.CENTER_VERTICAL);
        LinearLayout.LayoutParams sideLp = new LinearLayout.LayoutParams(0, -1, 1.3f);
        sideLp.leftMargin = p;
        side.addView(Ui.text(this, Ui.t("Ник", "Name"), 12, Ui.MUTED, false));
        name = new EditText(this);
        name.setSingleLine(true);
        name.setTextColor(Ui.TEXT);
        name.setHintTextColor(Ui.MUTED);
        name.setHint("Steve_123");
        name.setTextSize(16);
        name.setFilters(new InputFilter[] {new InputFilter.LengthFilter(16)});
        name.setPadding(p, Ui.dp(this, 8), p, Ui.dp(this, 8));
        name.setBackground(Ui.rounded(this, FIELD, Color.rgb(160, 160, 160), 0));
        name.setTypeface(Ui.font(this));
        name.setImeOptions(android.view.inputmethod.EditorInfo.IME_ACTION_DONE
                | android.view.inputmethod.EditorInfo.IME_FLAG_NO_EXTRACT_UI);
        name.setText(readName());
        side.addView(name);
        pick = Ui.button(this, Ui.t("Скин…", "Skin…"), false);
        pick.setOnClickListener(this);
        side.addView(pick, Ui.gap(this, 8));
        slim = readModel();
        model = Ui.button(this, "", false);
        model.setOnClickListener(this);
        side.addView(model, Ui.gap(this, 6));
        accountChip = Ui.text(this, "", 12, Ui.MUTED, false);
        side.addView(accountChip, Ui.gap(this, 6));
        who.addView(side, sideLp);
        left.addView(who, new LinearLayout.LayoutParams(-1, 0, 1));

        message = Ui.text(this, "", 12, Ui.WARN, false);
        message.setMaxLines(3);
        left.addView(message, Ui.gap(this, 4));
        play = Ui.button(this, Ui.t("ИГРАТЬ", "PLAY"), true);
        play.setTextSize(22);
        play.setLetterSpacing(0.1f);
        play.setMinHeight(Ui.dp(this, 60));
        play.setOnClickListener(this);
        left.addView(play, Ui.gap(this, 6));
        root.addView(left, new LinearLayout.LayoutParams(0, -1, 1));

        // Right: settings, and the beta tab.
        LinearLayout right = new LinearLayout(this);
        right.setOrientation(LinearLayout.VERTICAL);
        LinearLayout tabs = new LinearLayout(this);
        tabs.setOrientation(LinearLayout.HORIZONTAL);
        settingsTab = Ui.button(this, Ui.t("Настройки", "Settings"), false);
        betaTab = Ui.button(this, Ui.t("Бета", "Beta"), false);
        tabs.addView(settingsTab, new LinearLayout.LayoutParams(0, -2, 1));
        LinearLayout.LayoutParams tabLp = new LinearLayout.LayoutParams(0, -2, 1);
        tabLp.leftMargin = Ui.dp(this, 6);
        tabs.addView(betaTab, tabLp);
        right.addView(tabs);
        FrameLayout pages = new FrameLayout(this);
        LinearLayout.LayoutParams pagesLp = new LinearLayout.LayoutParams(-1, 0, 1);
        pagesLp.topMargin = Ui.dp(this, 6);
        right.addView(pages, pagesLp);
        ScrollView scroll = new ScrollView(this);
        scroll.setBackground(Ui.rounded(this, PANEL, RIM, 0));
        scroll.setVerticalScrollBarEnabled(false);
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setPadding(Ui.dp(this, 16), Ui.dp(this, 4), Ui.dp(this, 16), Ui.dp(this, 16));
        scroll.addView(card);
        pages.addView(scroll, new FrameLayout.LayoutParams(-1, -1));
        settingsPage = scroll;
        ScrollView betaScroll = new ScrollView(this);
        betaScroll.setBackground(Ui.rounded(this, PANEL, RIM, 0));
        betaScroll.setVerticalScrollBarEnabled(false);
        LinearLayout beta = new LinearLayout(this);
        beta.setOrientation(LinearLayout.VERTICAL);
        beta.setPadding(Ui.dp(this, 16), Ui.dp(this, 4), Ui.dp(this, 16), Ui.dp(this, 16));
        betaScroll.addView(beta);
        pages.addView(betaScroll, new FrameLayout.LayoutParams(-1, -1));
        betaPage = betaScroll;
        settingsTab.setOnClickListener(v -> showTab(false));
        betaTab.setOnClickListener(v -> showTab(true));
        buildBeta(beta);
        showTab(false);
        LinearLayout.LayoutParams rightLp = new LinearLayout.LayoutParams(0, -1, 1.15f);
        rightLp.leftMargin = Ui.dp(this, 14);
        root.addView(right, rightLp);

        Ui.section(card, Ui.t("Аккаунт Microsoft", "Microsoft account"));
        accountStatus = Ui.text(this, accountLine(), 15, Ui.TEXT, true);
        card.addView(accountStatus);
        card.addView(Ui.text(this, Ui.t(
                "Для лицензионных серверов. Войдите почтой и паролем один раз: дальше вход обновляется сам.",
                "For licensed servers. Sign in with your email and password once: it renews by itself."),
                12, Ui.MUTED, false));
        microsoft = Ui.button(this, Ui.t("Войти через Microsoft", "Sign in with Microsoft"), true);
        microsoft.setOnClickListener(this);
        card.addView(microsoft, Ui.gap(this, 8));
        codeBox = new LinearLayout(this);
        codeBox.setOrientation(LinearLayout.VERTICAL);
        codeBox.setPadding(p, p, p, p);
        codeBox.setBackground(Ui.rounded(this, FIELD, Ui.GREEN_DARK, 0));
        codeBox.setVisibility(View.GONE);
        codeHint = Ui.text(this, "", 13, Ui.MUTED, false);
        codeBox.addView(codeHint);
        code = Ui.text(this, "", 28, Ui.TEXT, true);
        code.setGravity(Gravity.CENTER);
        code.setTypeface(android.graphics.Typeface.MONOSPACE, android.graphics.Typeface.BOLD);
        code.setLetterSpacing(0.12f);
        code.setTextIsSelectable(true);
        codeBox.addView(code, Ui.gap(this, 4));
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        copy = Ui.button(this, Ui.t("Скопировать код", "Copy code"), false);
        copy.setOnClickListener(this);
        browser = Ui.button(this, Ui.t("Открыть браузер", "Open browser"), true);
        browser.setOnClickListener(this);
        row.addView(copy, new LinearLayout.LayoutParams(0, -2, 1));
        LinearLayout.LayoutParams half2 = new LinearLayout.LayoutParams(0, -2, 1);
        half2.leftMargin = Ui.dp(this, 8);
        row.addView(browser, half2);
        codeBox.addView(row, Ui.gap(this, 8));
        card.addView(codeBox, Ui.gap(this, 8));
        signOut = Ui.button(this, Ui.t("Выйти из аккаунта", "Sign out"), false);
        signOut.setOnClickListener(this);
        card.addView(signOut, Ui.gap(this, 8));
        advanced = Ui.button(this, Ui.t("Другой способ входа ▾", "Other sign-in ▾"), false);
        advanced.setOnClickListener(this);
        card.addView(advanced, Ui.gap(this, 8));
        advancedBox = new LinearLayout(this);
        advancedBox.setOrientation(LinearLayout.VERTICAL);
        advancedBox.addView(Ui.text(this, Ui.t(
                "Вход по коду через свой Azure client ID, одобренный Mojang (aka.ms/mce-reviewappid). Нужен, только если вход выше не работает.",
                "Code sign-in with your own Azure client ID approved by Mojang (aka.ms/mce-reviewappid). Only needed if the sign-in above fails."),
                12, Ui.MUTED, false));
        clientId = new EditText(this);
        clientId.setSingleLine(true);
        clientId.setTextColor(Ui.TEXT);
        clientId.setHintTextColor(Ui.MUTED);
        clientId.setHint("00000000-0000-0000-0000-000000000000");
        clientId.setText(Msa.clientId(this));
        clientId.setPadding(p, p, p, p);
        clientId.setBackground(Ui.rounded(this, FIELD, Color.rgb(160, 160, 160), 0));
        clientId.setTypeface(Ui.font(this));
        clientId.setImeOptions(android.view.inputmethod.EditorInfo.IME_FLAG_NO_EXTRACT_UI);
        advancedBox.addView(clientId, Ui.gap(this, 6));
        deviceCode = Ui.button(this, Ui.t("Войти по коду", "Sign in with a code"), false);
        deviceCode.setOnClickListener(this);
        advancedBox.addView(deviceCode, Ui.gap(this, 6));
        advancedBox.setVisibility(View.GONE);
        card.addView(advancedBox, Ui.gap(this, 6));

        Ui.section(card, Ui.t("Драйвер Vulkan · экспериментально", "Vulkan driver · experimental"));
        driverStatus = Ui.text(this, Turnip.status(this), 13, Ui.TEXT, false);
        card.addView(driverStatus);
        if (new File(getFilesDir(), "vulkan-driver.failed").isFile()) {
            card.addView(Ui.text(this, Ui.t(
                    "Свой драйвер не запустился в прошлый раз, игра вернулась на системный. Попробуйте другую сборку Turnip.",
                    "The custom driver did not start last time, so the game went back to the system one. Try another Turnip build."),
                    12, Ui.WARN, false));
        }
        card.addView(Ui.text(this, Ui.t(
                "Zip с Turnip (libvulkan_freedreno.so). На части Snapdragon быстрее системного. Если игра с ним не стартует, следующий запуск будет на системном.",
                "A Turnip zip (libvulkan_freedreno.so). Faster than the system driver on some Snapdragons. If the game fails to start with it, the next launch uses the system one."),
                12, Ui.MUTED, false));
        LinearLayout drivers = new LinearLayout(this);
        drivers.setOrientation(LinearLayout.HORIZONTAL);
        driver = Ui.button(this, Ui.t("Установить Turnip", "Install Turnip"), false);
        driver.setOnClickListener(this);
        systemDriver = Ui.button(this, Ui.t("Системный", "System"), false);
        systemDriver.setOnClickListener(this);
        drivers.addView(driver, new LinearLayout.LayoutParams(0, -2, 1));
        LinearLayout.LayoutParams dl = new LinearLayout.LayoutParams(0, -2, 1);
        dl.leftMargin = Ui.dp(this, 8);
        drivers.addView(systemDriver, dl);
        card.addView(drivers, Ui.gap(this, 8));
        final File safeMode = new File(getFilesDir(), "safe-mode.txt");
        if (safeMode.isFile()) {
            card.addView(Ui.text(this, Ui.t(
                    "Безопасный запуск: Vulkan не стартовал, игра работает на OpenGL ES и может быть медленнее.",
                    "Safe start: Vulkan did not start, so the game runs on OpenGL ES and can be slower."),
                    12, Ui.WARN, false), Ui.gap(this, 8));
            Button vulkan = Ui.button(this, Ui.t("Попробовать Vulkan снова", "Try Vulkan again"), false);
            vulkan.setOnClickListener(new View.OnClickListener() {
                public void onClick(View v) {
                    safeMode.delete();
                    new File(getFilesDir(), "launch.pending").delete();
                    v.setEnabled(false);
                    ((Button) v).setText(Ui.t("Перезапустите игру", "Restart the game"));
                }
            });
            card.addView(vulkan, Ui.gap(this, 8));
        }

        Ui.section(card, Ui.t("Экран", "Screen"));
        Button cutout = Ui.button(this, cutoutLabel(), false);
        cutout.setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                String next = "short";
                String cur = cutoutMode();
                if (cur.equals("short")) next = "never";
                else if (cur.equals("never")) next = "always";
                try {
                    Files.write(new File(getFilesDir(), "cutout.txt").toPath(), (next + "\n").getBytes(StandardCharsets.UTF_8));
                } catch (Exception ignored) {
                }
                HorizonActivity.applyCutout(ProfileActivity.this);
                ((Button) v).setText(cutoutLabel());
            }
        });
        card.addView(cutout, Ui.gap(this, 4));
        Button glow = Ui.button(this, glowLabel(), false);
        glow.setOnClickListener(v -> {
            File f = new File(getFilesDir(), "no-glow.txt");
            try {
                if (f.isFile()) f.delete();
                else write(f, "1\n".getBytes(StandardCharsets.UTF_8));
            } catch (Exception ignored) {
            }
            glow.setText(glowLabel());
            message.setTextColor(Ui.WARN);
            message.setText(Ui.t("Сработает при следующем запуске игры.", "Takes effect the next time the game starts."));
        });
        card.addView(glow, Ui.gap(this, 6));

        Ui.section(card, Ui.t("Моды", "Mods"));
        card.addView(Ui.text(this, Ui.t(
                ".hnmod со сборкой для Android или .hmod. Или положите файл в папку mods/ в папке игры.",
                "A .hnmod with an Android build, or a .hmod. Or drop the file into mods/ in the game folder."),
                12, Ui.MUTED, false));
        mods = Ui.button(this, Ui.t("Установить мод", "Install a mod"), false);
        mods.setOnClickListener(this);
        card.addView(mods, Ui.gap(this, 8));

        Ui.section(card, Ui.t("Папка игры", "Game folder"));
        folderStatus = Ui.text(this, folderLine(), 12, Ui.MUTED, false);
        card.addView(folderStatus);
        folder = Ui.button(this, folderButton(), false);
        folder.setOnClickListener(v -> askSharedFolder());
        card.addView(folder, Ui.gap(this, 6));

        Ui.section(card, Ui.t("Логи", "Logs"));
        card.addView(Ui.text(this, Ui.t(
                "Для отчёта об ошибке: логи, отчёты о вылетах и модель телефона в одном zip.",
                "For a bug report: the logs, crash reports and the phone model in one zip."),
                12, Ui.MUTED, false));
        LinearLayout logRow = new LinearLayout(this);
        logRow.setOrientation(LinearLayout.HORIZONTAL);
        Button shareLogs = Ui.button(this, Ui.t("Поделиться логами", "Share logs"), false);
        shareLogs.setOnClickListener(v -> {
            try {
                LogShare.share(this);
            } catch (Exception e) {
                message.setTextColor(Ui.WARN);
                message.setText(Ui.t("Не удалось собрать логи: ", "Could not collect the logs: ") + e.getMessage());
            }
        });
        Button saveLogs = Ui.button(this, Ui.t("В Загрузки", "To Downloads"), false);
        saveLogs.setOnClickListener(v -> {
            try {
                String where = LogShare.saveToDownloads(this);
                message.setTextColor(Ui.GREEN);
                message.setText(Ui.t("Сохранено: ", "Saved: ") + where);
            } catch (Exception e) {
                message.setTextColor(Ui.WARN);
                message.setText(Ui.t("Не удалось сохранить: ", "Could not save: ") + e.getMessage());
            }
        });
        logRow.addView(shareLogs, new LinearLayout.LayoutParams(0, -2, 1.3f));
        LinearLayout.LayoutParams logLp = new LinearLayout.LayoutParams(0, -2, 1);
        logLp.leftMargin = Ui.dp(this, 8);
        logRow.addView(saveLogs, logLp);
        card.addView(logRow, Ui.gap(this, 6));

        FrameLayout screen = new FrameLayout(this);
        screen.addView(new Ui.Panorama(this), new FrameLayout.LayoutParams(-1, -1));
        screen.addView(root, new FrameLayout.LayoutParams(-1, -1));
        setContentView(screen);
        hideBars();
        refreshAccount();
        updateModel();
        showPreview();
        if (!Storage.asked(this) && !Storage.inShared(this)) {
            screen.post(this::askSharedFolder);
        }
    }

    @Override
    public void onWindowFocusChanged(boolean focus) {
        super.onWindowFocusChanged(focus);
        if (focus) hideBars();
    }

    private void hideBars() {
        if (android.os.Build.VERSION.SDK_INT >= 30) {
            android.view.WindowInsetsController c = getWindow().getInsetsController();
            if (c != null) {
                c.hide(android.view.WindowInsets.Type.systemBars());
                c.setSystemBarsBehavior(android.view.WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        }
    }

    private String version() {
        try {
            return "v" + getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Exception e) {
            return "";
        }
    }

    private String readName() {
        try {
            for (String l : new String(Files.readAllBytes(new File(dir, "horizon-player.txt").toPath()), StandardCharsets.UTF_8).split("\n")) {
                if (l.startsWith("name:")) return l.substring(5).trim();
            }
        } catch (Exception e) {
            // No name yet: the game makes one.
        }
        return "";
    }

    private boolean readModel() {
        try {
            return new String(Files.readAllBytes(new File(dir, "horizon-skin.txt").toPath()), StandardCharsets.UTF_8).contains("model:slim");
        } catch (Exception e) {
            return false;
        }
    }

    private void updateModel() {
        model.setText(Ui.t("Руки: ", "Arms: ") + (slim ? Ui.t("тонкие (Alex)", "slim (Alex)") : Ui.t("обычные (Steve)", "classic (Steve)")));
    }

    static boolean validName(String n) {
        return n.matches("[A-Za-z0-9_]{3,16}");
    }

    private void refreshAccount() {
        String n = Msa.signedIn(this);
        accountStatus.setText(accountLine());
        signOut.setVisibility(n.isEmpty() ? View.GONE : View.VISIBLE);
        microsoft.setVisibility(n.isEmpty() ? View.VISIBLE : View.GONE);
        accountChip.setText(n.isEmpty() ? Ui.t("● Без лицензии (офлайн)", "● Offline (no license)") : Ui.t("● Лицензия: ", "● Licensed: ") + n);
        accountChip.setTextColor(n.isEmpty() ? Ui.MUTED : Ui.GREEN);
    }

    /** Saves the name and arm model; false (with a message) for a bad name. */
    private boolean saveProfile() {
        String n = name.getText().toString().trim();
        if (!n.isEmpty() && !validName(n)) {
            message.setTextColor(Ui.WARN);
            message.setText(Ui.t("Ник: 3–16 символов, только латиница, цифры и _.", "Name: 3–16 characters, letters, digits and _ only."));
            return false;
        }
        try {
            if (!n.isEmpty() && !n.equals(readName())) {
                write(new File(dir, "horizon-player.txt"), ("name:" + n + "\n").getBytes(StandardCharsets.UTF_8));
            }
            saveModel();
            return true;
        } catch (Exception e) {
            message.setText(Ui.t("Не удалось сохранить.", "Could not save."));
            return false;
        }
    }

    private void saveModel() throws Exception {
        if (readModel() == slim && new File(dir, "horizon-skin.txt").isFile()) return;
        write(new File(dir, "horizon-skin.txt"), ("model:" + (slim ? "slim" : "classic") + "\n").getBytes(StandardCharsets.UTF_8));
    }

    @Override
    protected void onPause() {
        super.onPause();
        // Leaving for the game or the file picker keeps what was typed.
        String n = name.getText().toString().trim();
        if (n.isEmpty() || validName(n)) saveProfile();
    }

    public void onClick(View v) {
        if (v == pick) {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("image/*");
            startActivityForResult(i, PICK);
        } else if (v == model) {
            slim = !slim;
            updateModel();
            showPreview();
            try {
                saveModel();
            } catch (Exception ignored) {
            }
        } else if (v == play) {
            if (!saveProfile()) return;
            Intent i = new Intent(this, HorizonActivity.class);
            i.addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT);
            startActivity(i);
            finish();
        } else if (v == driver) {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("*/*");
            startActivityForResult(i, PICK_DRIVER);
        } else if (v == systemDriver) {
            Turnip.useSystem(this);
            driverStatus.setText(Turnip.status(this));
            message.setTextColor(Ui.GREEN);
            message.setText(Ui.t("Системный драйвер. Перезапустите игру.", "System driver. Restart the game."));
        } else if (v == copy) {
            android.content.ClipboardManager cm = (android.content.ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
            if (cm != null) cm.setPrimaryClip(android.content.ClipData.newPlainText("code", code.getText()));
            copy.setText(Ui.t("Скопировано ✓", "Copied ✓"));
        } else if (v == browser) {
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(verifyUri)));
            } catch (Exception e) {
                message.setText(Ui.t("Нет браузера. Откройте ", "No browser. Open ") + verifyUri);
            }
        } else if (v == advanced) {
            advancedBox.setVisibility(advancedBox.getVisibility() == View.VISIBLE ? View.GONE : View.VISIBLE);
        } else if (v == microsoft) {
            try {
                startActivityForResult(new Intent(this, MsaLoginActivity.class), MS_LOGIN);
            } catch (Exception e) {
                advancedBox.setVisibility(View.VISIBLE);
                message.setTextColor(Ui.WARN);
                message.setText(Ui.t("Нет WebView на телефоне: войдите по коду ниже.", "No WebView on this phone: sign in with a code below."));
            }
        } else if (v == deviceCode) {
            startDeviceCode();
        } else if (v == signOut) {
            Msa.cancel();
            Msa.signOut(this);
            MsaLoginActivity.forgetCookies();
            codeBox.setVisibility(View.GONE);
            microsoft.setEnabled(true);
            deviceCode.setEnabled(true);
            refreshAccount();
            message.setTextColor(Ui.GREEN);
            message.setText(Ui.t("Вышли из аккаунта.", "Signed out."));
        } else if (v == mods) {
            startActivity(new Intent(this, ModImportActivity.class));
        }
    }

    private final Msa.Listener listener = new Msa.Listener() {
        public void onCode(String userCode, String uri) {
            verifyUri = uri;
            code.setText(userCode);
            codeHint.setText(Ui.t("Откройте ", "Open ") + uri.replace("https://", "")
                    + Ui.t(", введите код и войдите. Этот экран сам поймёт, что вы вошли.",
                    ", enter the code and sign in. This screen notices by itself."));
            copy.setText(Ui.t("Скопировать код", "Copy code"));
            codeBox.setVisibility(View.VISIBLE);
            deviceCode.setText(Ui.t("Ждём вход…", "Waiting for sign-in…"));
            message.setText("");
        }

        public void onDone(String n) {
            codeBox.setVisibility(View.GONE);
            resetButtons();
            refreshAccount();
            message.setTextColor(Ui.GREEN);
            message.setText(Ui.t("Вход выполнен: ", "Signed in: ") + n);
        }

        public void onError(String error) {
            codeBox.setVisibility(View.GONE);
            resetButtons();
            message.setTextColor(Ui.WARN);
            message.setText(error);
        }
    };

    private void resetButtons() {
        microsoft.setText(Ui.t("Войти через Microsoft", "Sign in with Microsoft"));
        microsoft.setEnabled(true);
        deviceCode.setText(Ui.t("Войти по коду", "Sign in with a code"));
        deviceCode.setEnabled(true);
    }

    private void startDeviceCode() {
        String id = clientId.getText().toString().trim();
        if (!id.matches("[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")) {
            message.setTextColor(Ui.WARN);
            message.setText(Ui.t("Вставьте свой Azure client ID.", "Paste your own Azure client ID."));
            return;
        }
        try {
            Msa.saveClientId(this, id);
        } catch (Exception e) {
            message.setText(Ui.t("Не удалось сохранить client ID.", "Could not save the client ID."));
            return;
        }
        message.setTextColor(Ui.WARN);
        message.setText(Ui.t("Запрашиваем код…", "Requesting a code…"));
        deviceCode.setEnabled(false);
        microsoft.setEnabled(false);
        Msa.start(this, id, listener);
    }

    private void showTab(boolean beta) {
        settingsPage.setVisibility(beta ? View.GONE : View.VISIBLE);
        betaPage.setVisibility(beta ? View.VISIBLE : View.GONE);
        settingsTab.setAlpha(beta ? 0.55f : 1f);
        betaTab.setAlpha(beta ? 1f : 0.55f);
    }

    /** The beta tab: which Minecraft the game is, and the experimental older-server support. */
    private void buildBeta(LinearLayout beta) {
        Ui.section(beta, Ui.t("Версия Minecraft", "Minecraft version"));
        beta.addView(Ui.text(this, Ui.t(
                "Horizon сейчас — это Minecraft 26.3. Другие версии готовятся.",
                "Horizon is Minecraft 26.3 for now. Other versions are in the works."),
                12, Ui.MUTED, false));
        String[] versions = {"26.3", "26.2", "1.21.x", "1.20.1", "1.16.5", "1.12.2", "1.7.10"};
        for (int i = 0; i < versions.length; i++) {
            boolean current = i == 0;
            Button b = Ui.button(this, current
                    ? "> " + versions[i] + "  " + Ui.t("(выбрана)", "(selected)")
                    : versions[i] + "  " + Ui.t("— Скоро", "— Soon"), false);
            b.setEnabled(current);
            if (current) b.setTextColor(Ui.GREEN);
            beta.addView(b, Ui.gap(this, i == 0 ? 8 : 4));
        }

        Ui.section(beta, "ViaHorizon");
        beta.addView(Ui.text(this, Ui.t(
                "Вход на серверы других версий: от 1.20.2 до 26.2. Версия сервера определяется сама. Экспериментально: что-то может работать не так. Серверы старше 1.20.2 (1.7–1.20.1) пока не поддерживаются.",
                "Join servers of other versions: 1.20.2 to 26.2. The server's version is detected by itself. Experimental: some things may not work right. Servers older than 1.20.2 (1.7–1.20.1) aren't supported yet."),
                12, Ui.MUTED, false));
        via = Ui.button(this, viaLabel(), false);
        via.setOnClickListener(v -> {
            File f = new File(getFilesDir(), "viahorizon.txt");
            try {
                if (viaOn()) f.delete();
                else write(f, "on\n".getBytes(StandardCharsets.UTF_8));
            } catch (Exception ignored) {
            }
            via.setText(viaLabel());
        });
        beta.addView(via, Ui.gap(this, 6));
    }

    private String glowLabel() {
        return new File(getFilesDir(), "no-glow.txt").isFile()
                ? Ui.t("Обводка «Свечения»: выкл (быстрее)", "Glowing outline: off (faster)")
                : Ui.t("Обводка «Свечения»: вкл", "Glowing outline: on");
    }

    private boolean viaOn() {
        return new File(getFilesDir(), "viahorizon.txt").isFile();
    }

    private String viaLabel() {
        return viaOn() ? Ui.t("ViaHorizon: включён", "ViaHorizon: on") : Ui.t("ViaHorizon: выключен", "ViaHorizon: off");
    }

    private String folderLine() {
        return Ui.t("Сейчас: ", "Now: ") + Storage.home(this).getAbsolutePath();
    }

    private String folderButton() {
        return Storage.inShared(this)
                ? Ui.t("Вернуть в папку приложения", "Move back to the app folder")
                : Ui.t("Перенести в папку Horizon", "Move to the Horizon folder");
    }

    /**
     * Offers the "Horizon" folder in the root of the storage (needs all-files access), or, when
     * the game is already there, moving it back. Asked once on the first start too.
     */
    private void askSharedFolder() {
        Storage.markAsked(this);
        if (Storage.inShared(this)) {
            new android.app.AlertDialog.Builder(this)
                    .setTitle(Ui.t("Папка приложения", "App folder"))
                    .setMessage(Ui.t(
                            "Перенести миры, моды, логи и файлы игры обратно в Android/data? Папка Horizon станет пустой.",
                            "Move the worlds, mods, logs and game files back to Android/data? The Horizon folder will be empty."))
                    .setPositiveButton(Ui.t("Перенести", "Move"), (d, w) -> migrate(Storage.home(this), Storage.appDir(this), null))
                    .setNegativeButton(Ui.t("Отмена", "Cancel"), null)
                    .show();
            return;
        }
        new android.app.AlertDialog.Builder(this)
                .setTitle(Ui.t("Папка Horizon", "The Horizon folder"))
                .setMessage(Ui.t(
                        "Игра может хранить свои файлы (saves, mods, resourcepacks, logs…) в папке Horizon в корне памяти — рядом с Download и Android. Её видно в любом файловом менеджере, и она не удаляется вместе с приложением.\n\nДля этого Android попросит «Доступ ко всем файлам». Без него всё остаётся в Android/data/dev.horizonlab.horizon/files, как раньше. Уже скачанное перенесётся, ничего не потеряется.",
                        "The game can keep its files (saves, mods, resourcepacks, logs…) in a Horizon folder in the root of the storage, next to Download and Android. Any file manager sees it, and it stays when the app is uninstalled.\n\nAndroid will ask for \"All files access\" for that. Without it everything stays in Android/data/dev.horizonlab.horizon/files, as before. What's already downloaded moves over, nothing is lost."))
                .setPositiveButton(Ui.t("Дать доступ", "Grant access"), (d, w) -> {
                    if (Storage.granted(this)) {
                        moveToShared();
                        return;
                    }
                    waitingAccess = true;
                    Intent i = Storage.permissionIntent(this);
                    try {
                        if (i != null) startActivity(i);
                        else requestPermissions(new String[] {android.Manifest.permission.WRITE_EXTERNAL_STORAGE}, 2);
                    } catch (Exception e) {
                        waitingAccess = false;
                    }
                })
                .setNegativeButton(Ui.t("Оставить как есть", "Keep it as is"), null)
                .show();
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (waitingAccess) {
            waitingAccess = false;
            if (Storage.granted(this)) moveToShared();
            else {
                message.setTextColor(Ui.WARN);
                message.setText(Ui.t("Доступа нет — файлы остаются в Android/data.", "No access: the files stay in Android/data."));
            }
        }
    }

    @Override
    public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        if (request != 2) return;
        waitingAccess = false;
        if (Storage.granted(this)) moveToShared();
    }

    private void moveToShared() {
        File to = Storage.sharedDir();
        if (!Storage.usable(to)) {
            message.setTextColor(Ui.WARN);
            message.setText(Ui.t("Не удалось создать ", "Could not create ") + to);
            return;
        }
        migrate(Storage.appDir(this), to, to);
    }

    /** Moves the game folder on a background thread, with progress; `chosen` is the new home (null: the app folder). */
    private void migrate(File from, File to, File chosen) {
        android.app.ProgressDialog wait = new android.app.ProgressDialog(this);
        wait.setCancelable(false);
        wait.setMessage(Ui.t("Переносим файлы…", "Moving the files…"));
        wait.show();
        new Thread(() -> {
            String error = null;
            try {
                // Chosen first: if the move stops halfway the game still finds what moved,
                // and the rest is moved on the next try (nothing is ever overwritten).
                Storage.choose(this, chosen);
                Storage.move(from, to, (n, name) -> runOnUiThread(() ->
                        wait.setMessage(Ui.t("Переносим файлы… ", "Moving the files… ") + n + "\n" + name)));
            } catch (Exception e) {
                error = e.getMessage();
            }
            String err = error;
            runOnUiThread(() -> {
                wait.dismiss();
                dir = Storage.home(this);
                folderStatus.setText(folderLine());
                folder.setText(folderButton());
                message.setTextColor(err == null ? Ui.GREEN : Ui.WARN);
                message.setText(err == null
                        ? Ui.t("Готово. Папка игры: ", "Done. The game folder: ") + Storage.home(this)
                        : Ui.t("Перенос прерван (", "The move stopped (") + err
                                + Ui.t("). Нажмите ещё раз — уже перенесённое не пострадает.", "). Press it again; what moved already is safe."));
            });
        }, "horizon-move").start();
    }

    private String cutoutMode() {
        try {
            return new String(Files.readAllBytes(new File(getFilesDir(), "cutout.txt").toPath()), StandardCharsets.UTF_8).trim();
        } catch (Exception e) {
            return "short";
        }
    }

    private String cutoutLabel() {
        String m = cutoutMode();
        if (m.equals("never")) return Ui.t("Вырез: не заходить под камеру", "Cutout: stay clear of the camera");
        if (m.equals("always")) return Ui.t("Вырез: всегда под камеру", "Cutout: draw under the camera");
        return Ui.t("Вырез: по краям экрана", "Cutout: short edges");
    }

    private String accountLine() {
        String n = Msa.signedIn(this);
        if (n.isEmpty()) return Ui.t("Не подключён", "Not connected");
        return Ui.t("Аккаунт: ", "Account: ") + n;
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == MS_LOGIN) {
            String c = data == null ? null : data.getStringExtra("code");
            if (result != RESULT_OK || c == null) {
                String err = data == null ? null : data.getStringExtra("error");
                if (err != null && !err.isEmpty()) {
                    message.setTextColor(Ui.WARN);
                    message.setText(Ui.t("Вход отменён: ", "Sign-in cancelled: ") + err);
                }
                return;
            }
            message.setTextColor(Ui.WARN);
            message.setText(Ui.t("Входим в Xbox и Minecraft…", "Signing in to Xbox and Minecraft…"));
            microsoft.setEnabled(false);
            microsoft.setText(Ui.t("Входим…", "Signing in…"));
            Msa.startWithCode(this, c, listener);
            return;
        }
        if (result != RESULT_OK || data == null || data.getData() == null) return;
        if (request == PICK_DRIVER) {
            message.setTextColor(Ui.WARN);
            try {
                String n = Turnip.install(this, data.getData());
                driverStatus.setText(Turnip.status(this));
                message.setTextColor(Ui.GREEN);
                message.setText(Ui.t("Драйвер установлен: ", "Driver installed: ") + n
                        + Ui.t(". Полностью закройте игру и откройте снова.", ". Fully close the game and open it again."));
            } catch (Exception e) {
                message.setText(Ui.t("В архиве нет .so драйвера (нужен zip с libvulkan_freedreno.so).",
                        "The archive has no driver .so (a zip with libvulkan_freedreno.so is needed)."));
            }
            return;
        }
        if (request != PICK) return;
        message.setTextColor(Ui.WARN);
        try (InputStream in = getContentResolver().openInputStream(data.getData())) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
                if (out.size() > 4 << 20) throw new Exception("too big");
            }
            byte[] png = normalizeSkin(out.toByteArray());
            if (png == null) {
                message.setText(Ui.t("Нужен скин: PNG 64×64 или 64×32 (HD 128/256… тоже подойдёт).",
                        "A skin is needed: a 64×64 or 64×32 PNG (HD 128/256… works too)."));
                return;
            }
            write(new File(dir, "horizon-skin.png"), png);
            saveModel();
            message.setTextColor(Ui.GREEN);
            message.setText(Ui.t("Скин применён.", "Skin applied."));
            showPreview();
        } catch (Exception e) {
            message.setText(Ui.t("Не удалось прочитать файл.", "Could not read the file."));
        }
    }

    /**
     * The picked image as the plain PNG the game reads: 8-bit RGBA, 64x64 or 64x32 (an HD skin
     * is scaled down pixel for pixel), and a legacy skin's fully opaque hat made see-through
     * (vanilla's SkinTextureDownloader.doNotchTransparencyHack). Null when it is no skin.
     */
    static byte[] normalizeSkin(byte[] bytes) {
        BitmapFactory.Options o = new BitmapFactory.Options();
        o.inScaled = false;
        o.inPremultiplied = false;
        o.inPreferredConfig = Bitmap.Config.ARGB_8888;
        Bitmap b = BitmapFactory.decodeByteArray(bytes, 0, bytes.length, o);
        if (b == null) return null;
        int w = b.getWidth(), h = b.getHeight();
        if (w < 64 || w > 1024 || w % 64 != 0 || (h != w && h * 2 != w)) return null;
        if (b.getConfig() != Bitmap.Config.ARGB_8888) b = b.copy(Bitmap.Config.ARGB_8888, true);
        if (w != 64) b = Bitmap.createScaledBitmap(b, 64, h * 64 / w, false);
        int[] px = new int[64 * b.getHeight()];
        b.getPixels(px, 0, 64, 0, 0, 64, b.getHeight());
        if (b.getHeight() == 32) {
            boolean opaque = true;
            for (int y = 0; y < 16 && opaque; y++) {
                for (int x = 32; x < 64; x++) {
                    if ((px[y * 64 + x] >>> 24) < 128) {
                        opaque = false;
                        break;
                    }
                }
            }
            if (opaque) for (int y = 0; y < 16; y++) for (int x = 32; x < 64; x++) px[y * 64 + x] = 0;
        }
        Bitmap clean = Bitmap.createBitmap(64, b.getHeight(), Bitmap.Config.ARGB_8888);
        clean.setPremultiplied(false);
        clean.setPixels(px, 0, 64, 0, 0, 64, b.getHeight());
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        if (!clean.compress(Bitmap.CompressFormat.PNG, 100, out)) return null;
        return out.toByteArray();
    }

    private static void write(File f, byte[] bytes) throws Exception {
        File tmp = new File(f.getPath() + ".tmp");
        try (FileOutputStream out = new FileOutputStream(tmp)) {
            out.write(bytes);
        }
        if (!tmp.renameTo(f)) {
            f.delete();
            if (!tmp.renameTo(f)) throw new Exception("rename");
        }
    }

    private void showPreview() {
        File f = new File(dir, "horizon-skin.png");
        Bitmap skin = null;
        if (f.isFile()) {
            BitmapFactory.Options o = new BitmapFactory.Options();
            o.inScaled = false;
            o.inPreferredConfig = Bitmap.Config.ARGB_8888;
            skin = BitmapFactory.decodeFile(f.getPath(), o);
            if (skin != null && (skin.getWidth() != 64 || (skin.getHeight() != 64 && skin.getHeight() != 32))) {
                // An older file the game cannot use either: clean it like a new pick.
                try {
                    byte[] png = normalizeSkin(Files.readAllBytes(f.toPath()));
                    if (png != null) {
                        write(f, png);
                        skin = BitmapFactory.decodeByteArray(png, 0, png.length, o);
                    } else {
                        skin = null;
                    }
                } catch (Exception e) {
                    skin = null;
                }
            }
        }
        preview.set(skin, slim);
    }
}
