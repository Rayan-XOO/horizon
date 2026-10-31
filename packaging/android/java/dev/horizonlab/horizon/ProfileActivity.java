package dev.horizonlab.horizon;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.os.Bundle;
import android.text.InputFilter;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/**
 * Name and skin. The name goes to horizon-player.txt (offline_name.rs: the name
 * used on servers and LAN); the skin, a 64x64 or 64x32 PNG picked with the system file
 * picker, to horizon-skin.png, and its arm model to horizon-skin.txt (game/local_skin.rs).
 */
public class ProfileActivity extends Activity implements View.OnClickListener {
    private static final int PICK = 7;
    private static final int PICK_DRIVER = 8;
    private File dir;
    private EditText name;
    private TextView message;
    private ImageView preview;
    private Button pick, model, save, back, driver, systemDriver, microsoft, signOut;
    private TextView driverStatus, accountStatus;
    private EditText clientId;
    private LinearLayout codeBox, advancedBox;
    private TextView code, codeHint;
    private Button copy, browser, advanced;
    private String verifyUri = "https://www.microsoft.com/link";
    private boolean slim;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        dir = new File(SetupActivity.home(this));
        LinearLayout card = Ui.card(this);
        card.addView(Ui.text(this, Ui.t("Профиль", "Profile"), 26, Ui.TEXT, true));
        card.addView(Ui.text(this, Ui.t("Ник", "Name"), 14, Ui.MUTED, false), Ui.gap(this, 10));
        name = new EditText(this);
        name.setSingleLine(true);
        name.setTextColor(Ui.TEXT);
        name.setHintTextColor(Ui.MUTED);
        name.setHint("Steve_123");
        name.setFilters(new InputFilter[] {new InputFilter.LengthFilter(16)});
        int p = Ui.dp(this, 12);
        name.setPadding(p, p, p, p);
        name.setBackground(Ui.rounded(this, android.graphics.Color.rgb(24, 24, 30), Ui.EDGE, 8));
        name.setText(readName());
        card.addView(name);
        card.addView(Ui.text(this, Ui.t("3–16 символов: латиница, цифры, _", "3–16 characters: letters, digits, _"),
                12, Ui.MUTED, false));
        card.addView(Ui.text(this, Ui.t("Скин", "Skin"), 14, Ui.MUTED, false), Ui.gap(this, 14));
        preview = new ImageView(this);
        preview.setScaleType(ImageView.ScaleType.FIT_CENTER);
        preview.setBackground(Ui.rounded(this, android.graphics.Color.rgb(24, 24, 30), 0, 10));
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, Ui.dp(this, 180));
        card.addView(preview, lp);
        pick = Ui.button(this, Ui.t("Выбрать PNG-скин", "Choose a PNG skin"), false);
        pick.setOnClickListener(this);
        card.addView(pick, Ui.gap(this, 10));
        slim = readModel();
        model = Ui.button(this, "", false);
        model.setOnClickListener(this);
        card.addView(model, Ui.gap(this, 8));
        message = Ui.text(this, "", 13, Ui.WARN, false);
        card.addView(message, Ui.gap(this, 6));
        Ui.section(card, Ui.t("Драйвер Vulkan · экспериментально", "Vulkan driver · experimental"));
        driverStatus = Ui.text(this, Turnip.status(this), 13, Ui.TEXT, false);
        card.addView(driverStatus);
        if (new File(getFilesDir(), "vulkan-driver.failed").isFile()) {
            card.addView(Ui.text(this, Ui.t(
                    "Свой драйвер не запустился в прошлый раз, игра вернулась на системный. Попробуйте другую сборку Turnip.",
                    "The custom driver did not start last time, so the game went back to the system one. Try another Turnip build."),
                    12, Ui.WARN, false), Ui.gap(this, 4));
        }
        card.addView(Ui.text(this, Ui.t(
                "Zip с Turnip (libvulkan_freedreno.so). На части Snapdragon быстрее системного драйвера. Если игра с ним не стартует, при следующем запуске включится системный.",
                "A Turnip zip (libvulkan_freedreno.so). Faster than the system driver on some Snapdragons. If the game fails to start with it, the next launch uses the system one."),
                12, Ui.MUTED, false), Ui.gap(this, 4));
        driver = Ui.button(this, Ui.t("Установить Turnip", "Install Turnip"), false);
        driver.setOnClickListener(this);
        card.addView(driver, Ui.gap(this, 8));
        systemDriver = Ui.button(this, Ui.t("Системный драйвер", "System driver"), false);
        systemDriver.setOnClickListener(this);
        card.addView(systemDriver, Ui.gap(this, 8));
        Ui.section(card, Ui.t("Экран", "Screen"));
        Button cutout = Ui.button(this, cutoutLabel(), false);
        cutout.setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                String next = "short";
                String cur = cutoutMode();
                if (cur.equals("short")) next = "never";
                else if (cur.equals("never")) next = "always";
                try {
                    java.nio.file.Files.write(new java.io.File(getFilesDir(), "cutout.txt").toPath(),
                            (next + "\n").getBytes(java.nio.charset.StandardCharsets.UTF_8));
                } catch (Exception ignored) {
                }
                HorizonActivity.applyCutout(ProfileActivity.this);
                ((Button) v).setText(cutoutLabel());
            }
        });
        card.addView(cutout, Ui.gap(this, 8));
        Ui.section(card, Ui.t("Аккаунт Microsoft", "Microsoft account"));
        accountStatus = Ui.text(this, accountLine(), 14, Ui.TEXT, true);
        card.addView(accountStatus);
        card.addView(Ui.text(this, Ui.t(
                "Нужен только для лицензионных серверов. Войти можно один раз: вход обновляется сам.",
                "Only needed for licensed servers. Sign in once: it renews by itself."),
                12, Ui.MUTED, false), Ui.gap(this, 2));
        codeBox = new LinearLayout(this);
        codeBox.setOrientation(LinearLayout.VERTICAL);
        codeBox.setPadding(p, p, p, p);
        codeBox.setBackground(Ui.rounded(this, android.graphics.Color.rgb(24, 24, 30), Ui.GREEN_DARK, 10));
        codeBox.setVisibility(View.GONE);
        codeHint = Ui.text(this, "", 13, Ui.MUTED, false);
        codeBox.addView(codeHint);
        code = Ui.text(this, "", 30, Ui.TEXT, true);
        code.setGravity(android.view.Gravity.CENTER);
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
        LinearLayout.LayoutParams half = new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1);
        row.addView(copy, half);
        LinearLayout.LayoutParams half2 = new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1);
        half2.leftMargin = Ui.dp(this, 8);
        row.addView(browser, half2);
        codeBox.addView(row, Ui.gap(this, 8));
        card.addView(codeBox, Ui.gap(this, 8));
        microsoft = Ui.button(this, Ui.t("Войти через Microsoft", "Sign in with Microsoft"), true);
        microsoft.setOnClickListener(this);
        card.addView(microsoft, Ui.gap(this, 8));
        signOut = Ui.button(this, Ui.t("Выйти из аккаунта", "Sign out"), false);
        signOut.setOnClickListener(this);
        signOut.setVisibility(Msa.signedIn(this).isEmpty() ? View.GONE : View.VISIBLE);
        card.addView(signOut, Ui.gap(this, 8));
        advanced = Ui.button(this, Ui.t("Дополнительно ▾", "Advanced ▾"), false);
        advanced.setOnClickListener(this);
        card.addView(advanced, Ui.gap(this, 8));
        advancedBox = new LinearLayout(this);
        advancedBox.setOrientation(LinearLayout.VERTICAL);
        advancedBox.addView(Ui.text(this, Ui.t(
                "Azure client ID. Нужен свой, одобренный Mojang для Minecraft (заявка: aka.ms/mce-reviewappid), иначе сервисы Minecraft откажут во входе. Чужие ID не вставляйте.",
                "Azure client ID. It has to be your own, approved by Mojang for Minecraft (apply at aka.ms/mce-reviewappid), or the Minecraft services refuse the sign-in. Don't paste someone else's ID."),
                12, Ui.MUTED, false));
        clientId = new EditText(this);
        clientId.setSingleLine(true);
        clientId.setTextColor(Ui.TEXT);
        clientId.setHintTextColor(Ui.MUTED);
        clientId.setHint("00000000-0000-0000-0000-000000000000");
        clientId.setText(Msa.clientId(this));
        clientId.setPadding(p, p, p, p);
        clientId.setBackground(Ui.rounded(this, android.graphics.Color.rgb(24, 24, 30), Ui.EDGE, 8));
        advancedBox.addView(clientId, Ui.gap(this, 6));
        advancedBox.setVisibility(Msa.clientId(this).isEmpty() ? View.VISIBLE : View.GONE);
        card.addView(advancedBox, Ui.gap(this, 6));
        Ui.section(card, "");
        save = Ui.button(this, Ui.t("Сохранить", "Save"), true);
        save.setOnClickListener(this);
        card.addView(save, Ui.gap(this, 12));
        back = Ui.button(this, Ui.t("Назад", "Back"), false);
        back.setOnClickListener(this);
        card.addView(back, Ui.gap(this, 8));
        updateModel();
        showPreview();
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
        model.setText(Ui.t("Модель рук: ", "Arms: ") + (slim ? Ui.t("тонкие (Alex)", "slim (Alex)") : Ui.t("классические (Steve)", "classic (Steve)")));
    }

    static boolean validName(String n) {
        return n.matches("[A-Za-z0-9_]{3,16}");
    }

    public void onClick(View v) {
        if (v == pick) {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("image/png");
            startActivityForResult(i, PICK);
        } else if (v == model) {
            slim = !slim;
            updateModel();
        } else if (v == save) {
            String n = name.getText().toString().trim();
            if (!validName(n)) {
                message.setText(Ui.t("Ник: 3–16 символов, только латиница, цифры и _.", "Name: 3–16 characters, letters, digits and _ only."));
                return;
            }
            try {
                write(new File(dir, "horizon-player.txt"), ("name:" + n + "\n").getBytes(StandardCharsets.UTF_8));
                write(new File(dir, "horizon-skin.txt"), ("model:" + (slim ? "slim" : "classic") + "\n").getBytes(StandardCharsets.UTF_8));
                message.setTextColor(Ui.GREEN);
                message.setText(Ui.t("Сохранено. Скин сменится сразу, ник — на серверах после перезапуска.",
                        "Saved. The skin changes right away, the name on servers after a restart."));
            } catch (Exception e) {
                message.setText(Ui.t("Не удалось сохранить.", "Could not save."));
            }
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
            String id = clientId.getText().toString().trim();
            if (!id.matches("[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")) {
                advancedBox.setVisibility(View.VISIBLE);
                message.setTextColor(Ui.WARN);
                message.setText(Ui.t("Сначала вставьте свой Azure client ID в «Дополнительно».",
                        "Paste your own Azure client ID under Advanced first."));
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
            microsoft.setEnabled(false);
            Msa.start(this, id, new Msa.Listener() {
                public void onCode(final String userCode, final String uri) {
                    runOnUiThread(new Runnable() {
                        public void run() {
                            verifyUri = uri;
                            code.setText(userCode);
                            codeHint.setText(Ui.t("Откройте ", "Open ") + uri.replace("https://", "")
                                    + Ui.t(", введите код и войдите. Эта страница сама поймёт, что вы вошли.",
                                    ", enter the code and sign in. This screen notices by itself."));
                            copy.setText(Ui.t("Скопировать код", "Copy code"));
                            codeBox.setVisibility(View.VISIBLE);
                            microsoft.setText(Ui.t("Ждём вход…", "Waiting for sign-in…"));
                            message.setText("");
                        }
                    });
                }
                public void onDone(final String name) {
                    runOnUiThread(new Runnable() {
                        public void run() {
                            accountStatus.setText(accountLine());
                            codeBox.setVisibility(View.GONE);
                            signOut.setVisibility(View.VISIBLE);
                            microsoft.setText(Ui.t("Войти через Microsoft", "Sign in with Microsoft"));
                            microsoft.setEnabled(true);
                            message.setTextColor(Ui.GREEN);
                            message.setText(Ui.t("Вход выполнен: ", "Signed in: ") + name
                                    + Ui.t(". Перезапустите игру, чтобы зайти на лицензионный сервер.",
                                    ". Restart the game to join a licensed server."));
                        }
                    });
                }
                public void onError(final String error) {
                    runOnUiThread(new Runnable() {
                        public void run() {
                            codeBox.setVisibility(View.GONE);
                            microsoft.setText(Ui.t("Войти через Microsoft", "Sign in with Microsoft"));
                            microsoft.setEnabled(true);
                            message.setTextColor(Ui.WARN);
                            message.setText(error);
                        }
                    });
                }
            });
        } else if (v == signOut) {
            Msa.cancel();
            Msa.signOut(this);
            accountStatus.setText(accountLine());
            signOut.setVisibility(View.GONE);
            codeBox.setVisibility(View.GONE);
            microsoft.setEnabled(true);
            message.setTextColor(Ui.GREEN);
            message.setText(Ui.t("Вышли. Перезапустите игру.", "Signed out. Restart the game."));
        } else if (v == back) {
            finish();
        }
    }

    private String cutoutMode() {
        try {
            return new String(java.nio.file.Files.readAllBytes(new java.io.File(getFilesDir(), "cutout.txt").toPath()),
                    java.nio.charset.StandardCharsets.UTF_8).trim();
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
        String name = Msa.signedIn(this);
        if (name.isEmpty()) return Ui.t("Не подключён", "Not connected");
        return Ui.t("Аккаунт: ", "Account: ") + name;
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (result != RESULT_OK || data == null || data.getData() == null) return;
        if (request == PICK_DRIVER) {
            message.setTextColor(Ui.WARN);
            try {
                String name = Turnip.install(this, data.getData());
                driverStatus.setText(Turnip.status(this));
                message.setTextColor(Ui.GREEN);
                message.setText(Ui.t("Драйвер установлен: ", "Driver installed: ") + name
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
                if (out.size() > 1 << 20) throw new Exception("too big");
            }
            byte[] png = out.toByteArray();
            BitmapFactory.Options o = new BitmapFactory.Options();
            o.inJustDecodeBounds = true;
            BitmapFactory.decodeByteArray(png, 0, png.length, o);
            boolean isPng = png.length > 8 && (png[0] & 0xFF) == 0x89 && png[1] == 'P' && png[2] == 'N' && png[3] == 'G';
            if (!isPng || o.outWidth != 64 || (o.outHeight != 64 && o.outHeight != 32)) {
                message.setText(Ui.t("Нужен PNG 64×64 или 64×32.", "A 64×64 or 64×32 PNG is needed."));
                return;
            }
            write(new File(dir, "horizon-skin.png"), png);
            message.setTextColor(Ui.GREEN);
            message.setText(Ui.t("Скин выбран.", "Skin chosen."));
            showPreview();
        } catch (Exception e) {
            message.setText(Ui.t("Не удалось прочитать файл.", "Could not read the file."));
        }
    }

    private static void write(File f, byte[] bytes) throws Exception {
        File tmp = new File(f.getPath() + ".tmp");
        try (FileOutputStream out = new FileOutputStream(tmp)) {
            out.write(bytes);
        }
        if (!tmp.renameTo(f)) throw new Exception("rename");
    }

    /** The skin's front, flat: head, body, arms and legs (a 16x32 figure). */
    private void showPreview() {
        File f = new File(dir, "horizon-skin.png");
        Bitmap skin = f.isFile() ? BitmapFactory.decodeFile(f.getPath()) : null;
        if (skin == null) {
            preview.setImageDrawable(null);
            return;
        }
        boolean legacy = skin.getHeight() == 32;
        Bitmap fig = Bitmap.createBitmap(16, 32, Bitmap.Config.ARGB_8888);
        Canvas c = new Canvas(fig);
        Paint p = new Paint();
        p.setFilterBitmap(false);
        int arm = slim ? 3 : 4;
        part(c, skin, p, 8, 8, 8, 8, 4, 0);
        part(c, skin, p, 20, 20, 8, 12, 4, 8);
        part(c, skin, p, 44, 20, arm, 12, 4 - arm, 8);
        if (legacy) mirrored(c, skin, p, 44, 20, arm, 12, 12, 8);
        else part(c, skin, p, 36, 52, arm, 12, 12, 8);
        part(c, skin, p, 4, 20, 4, 12, 4, 20);
        if (legacy) mirrored(c, skin, p, 4, 20, 4, 12, 8, 20);
        else part(c, skin, p, 20, 52, 4, 12, 8, 20);
        part(c, skin, p, 40, 8, 8, 8, 4, 0);
        Bitmap big = Bitmap.createScaledBitmap(fig, 16 * 5, 32 * 5, false);
        preview.setImageBitmap(big);
    }

    private static void part(Canvas c, Bitmap s, Paint p, int x, int y, int w, int h, int dx, int dy) {
        c.drawBitmap(s, new Rect(x, y, x + w, y + h), new Rect(dx, dy, dx + w, dy + h), p);
    }

    private static void mirrored(Canvas c, Bitmap s, Paint p, int x, int y, int w, int h, int dx, int dy) {
        c.save();
        c.scale(-1, 1, dx + w / 2f, 0);
        part(c, s, p, x, y, w, h, dx, dy);
        c.restore();
    }
}
