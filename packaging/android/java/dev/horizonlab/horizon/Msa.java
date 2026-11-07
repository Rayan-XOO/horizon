package dev.horizonlab.horizon;

import android.content.Context;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;

/**
 * Microsoft device-code sign-in for online-mode servers. Uses this build's own
 * Azure client ID (msa-client-id.txt). Never another app's ID. Tokens stay in
 * the app's private files and are not logged.
 */
final class Msa {
    interface Listener {
        void onCode(String userCode, String uri);
        void onDone(String name);
        void onError(String message);
    }

    private static volatile boolean cancel;

    private Msa() {}

    static File idFile(Context c) {
        return new File(c.getFilesDir(), "msa-client-id.txt");
    }

    static File accountFile(Context c) {
        return new File(c.getFilesDir(), "msa-account.json");
    }

    static String clientId(Context c) {
        try {
            return new String(java.nio.file.Files.readAllBytes(idFile(c).toPath()), StandardCharsets.UTF_8).trim();
        } catch (Exception e) {
            return "";
        }
    }

    static void saveClientId(Context c, String id) throws Exception {
        write(idFile(c), id.trim().getBytes(StandardCharsets.UTF_8));
    }

    /** The signed-in Minecraft name, or empty. */
    static String signedIn(Context c) {
        try {
            JSONObject o = new JSONObject(new String(java.nio.file.Files.readAllBytes(accountFile(c).toPath()), StandardCharsets.UTF_8));
            return o.optString("name", "");
        } catch (Exception e) {
            return "";
        }
    }

    /** The saved account's Minecraft token runs out within the hour and it can be renewed. */
    static boolean needsRefresh(Context c) {
        try {
            JSONObject o = new JSONObject(new String(java.nio.file.Files.readAllBytes(accountFile(c).toPath()), StandardCharsets.UTF_8));
            long exp = o.optLong("minecraft_expires_at", 0);
            return !o.optString("refresh_token", "").isEmpty() && !clientId(c).isEmpty()
                    && exp < System.currentTimeMillis() / 1000L + 3600;
        } catch (Exception e) {
            return false;
        }
    }

    /**
     * Renews the saved account with its refresh token, so a player signs in once and stays
     * signed in. Blocking; call it off the UI thread. Returns the error, or null when renewed.
     * A refused refresh token signs the account out (the player has to sign in again).
     */
    static synchronized String refresh(Context c) {
        try {
            JSONObject o = new JSONObject(new String(java.nio.file.Files.readAllBytes(accountFile(c).toPath()), StandardCharsets.UTF_8));
            String refresh = o.optString("refresh_token", "");
            String id = clientId(c);
            if (refresh.isEmpty() || id.isEmpty()) return Ui.t("Нет данных для обновления входа.", "Nothing to renew the sign-in with.");
            JSONObject token = postForm("https://login.microsoftonline.com/consumers/oauth2/v2.0/token",
                    "grant_type=refresh_token&client_id=" + enc(id) + "&refresh_token=" + enc(refresh)
                            + "&scope=" + enc("XboxLive.signin offline_access"));
            String access = token.optString("access_token", "");
            if (access.isEmpty()) {
                String err = token.optString("error", "");
                if ("invalid_grant".equals(err)) {
                    signOut(c);
                    return Ui.t("Вход устарел, войдите в Microsoft ещё раз.", "The sign-in has expired; sign in with Microsoft again.");
                }
                return oauth(token);
            }
            String next = token.optString("refresh_token", "");
            JSONObject account = finish(access, next.isEmpty() ? refresh : next);
            write(accountFile(c), account.toString().getBytes(StandardCharsets.UTF_8));
            return null;
        } catch (java.io.IOException e) {
            return Ui.t("Нет сети, вход обновится позже.", "No network; the sign-in renews later.");
        } catch (Exception e) {
            return e.getMessage() == null ? Ui.t("Не удалось обновить вход.", "Could not renew the sign-in.") : e.getMessage();
        }
    }

    static void signOut(Context c) {
        accountFile(c).delete();
    }

    static void cancel() {
        cancel = true;
    }

    static void start(final Context c, final String clientId, final Listener listener) {
        cancel = false;
        new Thread(new Runnable() {
            public void run() {
                try {
                    listener.onDone(signIn(c, clientId, listener));
                } catch (Exception e) {
                    if (!cancel) listener.onError(e.getMessage() == null ? "Sign-in failed." : e.getMessage());
                }
            }
        }, "horizon-msa").start();
    }

    private static String signIn(Context c, String clientId, Listener listener) throws Exception {
        String base = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
        JSONObject code = postForm(base + "/devicecode",
                "client_id=" + enc(clientId) + "&scope=" + enc("XboxLive.signin offline_access"));
        String userCode = code.optString("user_code", "");
        String device = code.optString("device_code", "");
        if (userCode.isEmpty() || device.isEmpty()) throw new Exception(oauth(code));
        String uri = code.optString("verification_uri", "https://www.microsoft.com/link");
        listener.onCode(userCode, uri);
        int interval = Math.max(1, code.optInt("interval", 5));
        long deadline = System.currentTimeMillis() + code.optLong("expires_in", 900) * 1000L;
        JSONObject token = null;
        while (System.currentTimeMillis() < deadline) {
            sleep(interval);
            if (cancel) throw new Exception(Ui.t("Вход отменён.", "Sign-in cancelled."));
            JSONObject poll = postForm(base + "/token",
                    "grant_type=" + enc("urn:ietf:params:oauth:grant-type:device_code")
                            + "&client_id=" + enc(clientId) + "&device_code=" + enc(device));
            String err = poll.optString("error", "");
            if ("authorization_pending".equals(err)) continue;
            if ("slow_down".equals(err)) {
                interval += 5;
                continue;
            }
            if (!poll.optString("access_token", "").isEmpty()) {
                token = poll;
                break;
            }
            throw new Exception(oauth(poll));
        }
        if (token == null) throw new Exception(Ui.t("Код истёк. Начните снова.", "The code expired. Start again."));
        JSONObject account = finish(token.optString("access_token"), token.optString("refresh_token"));
        write(accountFile(c), account.toString().getBytes(StandardCharsets.UTF_8));
        return account.optString("name");
    }

    private static JSONObject finish(String msAccess, String refresh) throws Exception {
        JSONObject xbl = postJson("https://user.auth.xboxlive.com/user/authenticate", new JSONObject()
                .put("Properties", new JSONObject()
                        .put("AuthMethod", "RPS")
                        .put("SiteName", "user.auth.xboxlive.com")
                        .put("RpsTicket", "d=" + msAccess))
                .put("RelyingParty", "http://auth.xboxlive.com")
                .put("TokenType", "JWT"));
        String xblToken = xbl.optString("Token", "");
        if (xblToken.isEmpty()) throw new Exception(Ui.t("Xbox Live отклонил вход.", "Xbox Live refused the sign-in."));
        JSONObject xsts = postJson("https://xsts.auth.xboxlive.com/xsts/authorize", new JSONObject()
                .put("Properties", new JSONObject().put("SandboxId", "RETAIL").put("UserTokens", new JSONArray().put(xblToken)))
                .put("RelyingParty", "rp://api.minecraftservices.com/")
                .put("TokenType", "JWT"));
        if (xsts.has("XErr")) throw new Exception(xstsError(xsts.optLong("XErr")));
        JSONObject claims = xsts.optJSONObject("DisplayClaims");
        JSONArray xui = claims == null ? null : claims.optJSONArray("xui");
        JSONObject who = xui == null || xui.length() == 0 ? null : xui.optJSONObject(0);
        String uhs = who == null ? "" : who.optString("uhs", "");
        String xstsToken = xsts.optString("Token", "");
        if (uhs.isEmpty() || xstsToken.isEmpty()) throw new Exception(Ui.t("Xbox Live отклонил вход.", "Xbox Live refused the sign-in."));
        String xtoken = "XBL3.0 x=" + uhs + ";" + xstsToken;
        JSONObject mc = postJson("https://api.minecraftservices.com/launcher/login",
                new JSONObject().put("xtoken", xtoken).put("platform", "PC_LAUNCHER"));
        if (mc.optString("access_token", "").isEmpty()) {
            mc = postJson("https://api.minecraftservices.com/authentication/login_with_xbox",
                    new JSONObject().put("identityToken", xtoken));
        }
        String mcToken = mc.optString("access_token", "");
        if (mcToken.isEmpty()) {
            if (mc.optInt("status", 0) == 403 || mc.optString("error", "").toLowerCase().contains("invalid")) {
                throw new Exception(Ui.t(
                        "Mojang ещё не одобрил этот Azure app. Зарегистрируйте свой client ID и подайте его на aka.ms/mce-reviewappid.",
                        "Mojang has not approved this Azure app yet. Register your own client ID and submit it at aka.ms/mce-reviewappid."));
            }
            throw new Exception(Ui.t("Сервисы Minecraft отклонили вход.", "The Minecraft services refused the sign-in."));
        }
        if (!owns(mcToken)) throw new Exception(Ui.t("На этом аккаунте нет Minecraft: Java Edition.", "This account doesn't own Minecraft: Java Edition."));
        JSONObject profile = get("https://api.minecraftservices.com/minecraft/profile", mcToken);
        if (profile.optInt("status", 0) == 404 || profile.optString("name", "").isEmpty()) {
            throw new Exception(Ui.t("Сначала создайте профиль на minecraft.net.", "Create a Minecraft profile at minecraft.net first."));
        }
        long exp = System.currentTimeMillis() / 1000L + mc.optLong("expires_in", 86400);
        return new JSONObject()
                .put("name", profile.optString("name"))
                .put("uuid", profile.optString("id").replace("-", "").toLowerCase())
                .put("xuid", who.optString("xid", ""))
                .put("refresh_token", refresh)
                .put("minecraft_token", mcToken)
                .put("minecraft_expires_at", exp);
    }

    private static boolean owns(String token) throws Exception {
        JSONObject store = get("https://api.minecraftservices.com/entitlements/mcstore", token);
        JSONArray items = store.optJSONArray("items");
        if (items == null) return false;
        for (int i = 0; i < items.length(); i++) {
            String name = items.optJSONObject(i).optString("name", "");
            if ("game_minecraft".equals(name) || "product_minecraft".equals(name)) return true;
        }
        return false;
    }

    private static String xstsError(long code) {
        if (code == 2148916233L) return Ui.t("Нет профиля Xbox. Зайдите один раз на xbox.com.", "No Xbox profile yet. Sign in once at xbox.com.");
        if (code == 2148916238L) return Ui.t("Детский аккаунт: его должен добавить взрослый.", "A child account needs an adult in a Microsoft family.");
        return Ui.t("Xbox Live отклонил вход.", "Xbox Live refused the sign-in.") + " (" + code + ")";
    }

    private static String oauth(JSONObject v) {
        String err = v.optString("error", "");
        if ("authorization_declined".equals(err)) return Ui.t("Вход отклонён в браузере.", "Sign-in was declined in the browser.");
        if ("invalid_client".equals(err) || "unauthorized_client".equals(err)) {
            return Ui.t("Microsoft не принял этот client ID. Включите public client flows.", "Microsoft refused this client ID. Enable public client flows.");
        }
        return err.isEmpty() ? Ui.t("Вход в Microsoft не удался.", "Microsoft sign-in failed.") : err;
    }

    private static void sleep(int seconds) {
        try {
            Thread.sleep(seconds * 1000L);
        } catch (InterruptedException ignored) {
        }
    }

    private static String enc(String s) throws Exception {
        return URLEncoder.encode(s, "UTF-8");
    }

    private static JSONObject postForm(String url, String body) throws Exception {
        return call("POST", url, "application/x-www-form-urlencoded", body.getBytes(StandardCharsets.UTF_8), null);
    }

    private static JSONObject postJson(String url, JSONObject body) throws Exception {
        return call("POST", url, "application/json", body.toString().getBytes(StandardCharsets.UTF_8), null);
    }

    private static JSONObject get(String url, String bearer) throws Exception {
        return call("GET", url, null, null, bearer);
    }

    private static JSONObject call(String method, String url, String type, byte[] body, String bearer) throws Exception {
        HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
        c.setConnectTimeout(20000);
        c.setReadTimeout(20000);
        c.setRequestMethod(method);
        if (bearer != null) c.setRequestProperty("Authorization", "Bearer " + bearer);
        if (body != null) {
            c.setDoOutput(true);
            c.setRequestProperty("Content-Type", type);
            try (OutputStream out = c.getOutputStream()) {
                out.write(body);
            }
        }
        int status = c.getResponseCode();
        InputStream in = status >= 400 ? c.getErrorStream() : c.getInputStream();
        String text = "";
        if (in != null) {
            byte[] buf = new byte[4096];
            int n;
            StringBuilder sb = new StringBuilder();
            while ((n = in.read(buf)) > 0) sb.append(new String(buf, 0, n, StandardCharsets.UTF_8));
            text = sb.toString();
        }
        JSONObject o = text.isEmpty() ? new JSONObject() : new JSONObject(text);
        if (status >= 400 && !o.has("error") && !o.has("XErr")) o.put("status", status);
        return o;
    }

    private static void write(File f, byte[] bytes) throws Exception {
        File tmp = new File(f.getPath() + ".tmp");
        try (FileOutputStream out = new FileOutputStream(tmp)) {
            out.write(bytes);
        }
        if (!tmp.renameTo(f)) throw new Exception("rename");
    }
}
