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
        card.addView(Ui.text(this, "Vulkan", 14, Ui.MUTED, false), Ui.gap(this, 16));
        driverStatus = Ui.text(this, Turnip.status(this), 13, Ui.TEXT, false);
        card.addView(driverStatus);
        card.addView(Ui.text(this, Ui.t(
                "Zip с Turnip (libvulkan_freedreno.so). На Snapdragon часто быстрее системного драйвера. Нужен перезапуск.",
                "A Turnip zip (libvulkan_freedreno.so). Often faster than the system driver on Snapdragon. Restart required."),
                12, Ui.MUTED, false), Ui.gap(this, 4));
        driver = Ui.button(this, Ui.t("Установить Turnip", "Install Turnip"), false);
        driver.setOnClickListener(this);
        card.addView(driver, Ui.gap(this, 8));
        systemDriver = Ui.button(this, Ui.t("Системный драйвер", "System driver"), false);
        systemDriver.setOnClickListener(this);
        card.addView(systemDriver, Ui.gap(this, 8));
        card.addView(Ui.text(this, Ui.t("Вырез камеры", "Camera cutout"), 14, Ui.MUTED, false), Ui.gap(this, 16));
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
        card.addView(Ui.text(this, "Microsoft", 14, Ui.MUTED, false), Ui.gap(this, 16));
        accountStatus = Ui.text(this, accountLine(), 13, Ui.TEXT, false);
        card.addView(accountStatus);
        card.addView(Ui.text(this, Ui.t(
                "Свой Azure client ID (не чужой). Без одобрения Mojang вход на лицензионные сервера не пройдёт. aka.ms/mce-reviewappid",
                "Your own Azure client ID (not someone else's). Without Mojang's approval, licensed servers will refuse the sign-in. aka.ms/mce-reviewappid"),
                12, Ui.MUTED, false), Ui.gap(this, 4));
        clientId = new EditText(this);
        clientId.setSingleLine(true);
        clientId.setTextColor(Ui.TEXT);
        clientId.setHint("00000000-0000-0000-0000-000000000000");
        clientId.setText(Msa.clientId(this));
        clientId.setPadding(p, p, p, p);
        clientId.setBackground(Ui.rounded(this, android.graphics.Color.rgb(24, 24, 30), Ui.EDGE, 8));
        card.addView(clientId, Ui.gap(this, 8));
        microsoft = Ui.button(this, Ui.t("Войти в Microsoft", "Sign in with Microsoft"), true);
        microsoft.setOnClickListener(this);
        card.addView(microsoft, Ui.gap(this, 8));
        signOut = Ui.button(this, Ui.t("Выйти из аккаунта", "Sign out"), false);
        signOut.setOnClickListener(this);
        card.addView(signOut, Ui.gap(this, 8));
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
        } else if (v == microsoft) {
            String id = clientId.getText().toString().trim();
            if (id.length() < 32) {
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
            Msa.start(this, id, new Msa.Listener() {
                public void onCode(final String userCode, final String uri) {
                    runOnUiThread(new Runnable() {
                        public void run() {
                            message.setText(Ui.t("Откройте ", "Open ") + uri + Ui.t(" и введите ", " and enter ") + userCode);
                            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(uri)));
                        }
                    });
                }
                public void onDone(final String name) {
                    runOnUiThread(new Runnable() {
                        public void run() {
                            accountStatus.setText(accountLine());
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
        if (name.isEmpty()) return Ui.t("Аккаунт не подключён. Сервера с лицензией не пустят.", "No account. Licensed servers will refuse you.");
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
