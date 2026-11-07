package dev.horizonlab.horizon;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
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
    private String verifyUri = "https://www.microsoft.com/link";
    private boolean slim;

    private static final int PANEL = Color.rgb(26, 27, 33);
    private static final int FIELD = Color.rgb(20, 21, 26);

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        dir = new File(SetupActivity.home(this));
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_PAN
                | WindowManager.LayoutParams.SOFT_INPUT_STATE_HIDDEN);
        int p = Ui.dp(this, 12);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.HORIZONTAL);
        root.setBackground(new GradientDrawable(GradientDrawable.Orientation.TL_BR,
                new int[] {Color.rgb(22, 30, 30), Ui.BG, Color.rgb(8, 8, 11)}));
        int pad = Ui.dp(this, 16);
        root.setPadding(pad, pad, pad, pad);

        // Left: who plays.
        LinearLayout left = new LinearLayout(this);
        left.setOrientation(LinearLayout.VERTICAL);
        left.setPadding(p, p, p, p);
        left.setBackground(Ui.rounded(this, PANEL, Ui.EDGE, 16));
        LinearLayout head = new LinearLayout(this);
        head.setOrientation(LinearLayout.HORIZONTAL);
        head.setGravity(Gravity.CENTER_VERTICAL);
        TextView title = Ui.text(this, "Horizon", 22, Ui.TEXT, true);
        head.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
        TextView ver = Ui.text(this, version(), 12, Ui.MUTED, false);
        head.addView(ver);
        left.addView(head);

        LinearLayout who = new LinearLayout(this);
        who.setOrientation(LinearLayout.HORIZONTAL);
        preview = new SkinView(this);
        preview.setBackground(Ui.rounded(this, FIELD, 0, 12));
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
        name.setBackground(Ui.rounded(this, FIELD, Ui.EDGE, 8));
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

        // Right: settings.
        ScrollView scroll = new ScrollView(this);
        scroll.setBackground(Ui.rounded(this, PANEL, Ui.EDGE, 16));
        scroll.setVerticalScrollBarEnabled(false);
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setPadding(Ui.dp(this, 16), Ui.dp(this, 4), Ui.dp(this, 16), Ui.dp(this, 16));
        scroll.addView(card);
        LinearLayout.LayoutParams rightLp = new LinearLayout.LayoutParams(0, -1, 1.15f);
        rightLp.leftMargin = Ui.dp(this, 14);
        root.addView(scroll, rightLp);

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
        codeBox.setBackground(Ui.rounded(this, FIELD, Ui.GREEN_DARK, 10));
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
        clientId.setBackground(Ui.rounded(this, FIELD, Ui.EDGE, 8));
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

        Ui.section(card, Ui.t("Моды", "Mods"));
        card.addView(Ui.text(this, Ui.t(
                ".hnmod со сборкой для Android или .hmod. Или положите файл в Android/data/dev.horizonlab.horizon/files/mods/.",
                "A .hnmod with an Android build, or a .hmod. Or drop the file into Android/data/dev.horizonlab.horizon/files/mods/."),
                12, Ui.MUTED, false));
        mods = Ui.button(this, Ui.t("Установить мод", "Install a mod"), false);
        mods.setOnClickListener(this);
        card.addView(mods, Ui.gap(this, 8));

        setContentView(root);
        hideBars();
        refreshAccount();
        updateModel();
        showPreview();
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
