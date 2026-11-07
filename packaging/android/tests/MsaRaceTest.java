package dev.horizonlab.horizon;

import android.content.Context;
import android.os.Handler;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;

/** Deterministic network barriers exercise the production session code, not a reimplementation. */
public final class MsaRaceTest {
    private static final String ACCOUNT = "{\"name\":\"TestPlayer\",\"refresh_token\":\"test-refresh\"}";
    private static volatile Gate gate;
    private static volatile boolean invalidGrant;
    private static final AtomicBoolean failPoll = new AtomicBoolean();
    private static int passed;

    private static final class Gate {
        final String path;
        final CountDownLatch entered = new CountDownLatch(1), release = new CountDownLatch(1);
        final AtomicBoolean used = new AtomicBoolean();
        Gate(String path) { this.path = path; gate = this; }
        void await() throws Exception { check(entered.await(8, TimeUnit.SECONDS), "Network gate timed out: " + path); }
        void open() { release.countDown(); }
    }

    private static final class Listener implements Msa.Listener {
        int codes, done, errors;
        public void onCode(String code, String uri) { codes++; }
        public void onDone(String name) { done++; }
        public void onError(String message) { errors++; }
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
    private static void ok(String message) { System.out.println("PASS " + message); passed++; }
    private static Context context() throws Exception {
        Msa.cancel(); Handler.drain(); HorizonActivity.refreshed = 0;
        invalidGrant = false; failPoll.set(false); gate = null;
        Context c = new Context(Files.createTempDirectory("msa-session-").toFile());
        Msa.saveClientId(c, "test-client");
        Files.write(Msa.accountFile(c).toPath(), ACCOUNT.getBytes(StandardCharsets.UTF_8));
        return c;
    }
    private static Thread refresh(final Context c) {
        Thread thread = new Thread(new Runnable() { public void run() { Msa.refresh(c); } });
        thread.start(); return thread;
    }
    private static void join(Thread thread) throws Exception {
        thread.join(8000); check(!thread.isAlive(), "Refresh did not finish");
    }
    private static void signInsFinished() throws Exception {
        for (Thread t : Thread.getAllStackTraces().keySet())
            if (t.getName().equals("horizon-msa")) { t.join(8000); check(!t.isAlive(), "Sign-in did not finish"); }
    }
    private static void empty(Context c) {
        check(!Msa.accountFile(c).exists(), "Stale work restored account");
        check(!new File(c.getFilesDir(), "sessions/session-android.json").exists(), "Session token survived sign-out");
        check(HorizonActivity.refreshed == 0, "Stale refresh notified native code");
    }

    private static String response(URL url, String body) throws IOException {
        Gate g = gate;
        boolean block = g != null && url.getPath().equals(g.path) && g.used.compareAndSet(false, true);
        boolean rejected = invalidGrant; // Capture the server reply before unblocking a newer operation.
        if (block) {
            g.entered.countDown();
            try { if (!g.release.await(8, TimeUnit.SECONDS)) throw new IOException("Barrier timeout"); }
            catch (InterruptedException e) { throw new IOException(e); }
        }
        String path = url.getPath();
        if (path.endsWith("/devicecode")) return "{\"user_code\":\"TEST\",\"device_code\":\"test-device\",\"interval\":1}";
        if (path.endsWith("/token")) {
            if (body.contains("device_code") && failPoll.compareAndSet(true, false)) throw new IOException("Transient test outage");
            return rejected ? "{\"error\":\"invalid_grant\"}" : "{\"access_token\":\"test-access\",\"refresh_token\":\"new-test-refresh\"}";
        }
        if (path.equals("/user/authenticate")) return "{\"Token\":\"test-xbl\"}";
        if (path.equals("/xsts/authorize")) return "{\"Token\":\"test-xsts\",\"DisplayClaims\":{\"xui\":[{\"uhs\":\"test-hash\"}]}}";
        if (path.equals("/launcher/login")) return "{\"access_token\":\"test-minecraft\"}";
        if (path.equals("/entitlements/mcstore")) return "{\"items\":[{\"name\":\"game_minecraft\"}]}";
        if (path.equals("/minecraft/profile")) return "{\"name\":\"TestPlayer\",\"id\":\"0123456789abcdef0123456789abcdef\"}";
        throw new IOException("Unexpected test endpoint: " + path);
    }

    public static void main(String[] args) throws Exception {
        URL.setURLStreamHandlerFactory(protocol -> "https".equals(protocol) ? new URLStreamHandler() {
            protected URLConnection openConnection(URL url) {
                return new HttpURLConnection(url) {
                    final ByteArrayOutputStream body = new ByteArrayOutputStream();
                    public void connect() {} public void disconnect() {} public boolean usingProxy() { return false; }
                    public OutputStream getOutputStream() { return body; }
                    public int getResponseCode() { return 200; }
                    public InputStream getInputStream() throws IOException {
                        return new ByteArrayInputStream(response(url, new String(body.toByteArray(), StandardCharsets.UTF_8)).getBytes(StandardCharsets.UTF_8));
                    }
                };
            }
        } : null);

        Context c = context();
        new File(c.getFilesDir(), "sessions").mkdirs();
        Files.write(new File(c.getFilesDir(), "sessions/session-android.json").toPath(), new byte[]{1});
        Files.write(new File(Msa.accountFile(c).getPath() + ".tmp").toPath(), new byte[]{1});
        Gate g = new Gate("/consumers/oauth2/v2.0/token"); Thread t = refresh(c); g.await();
        Msa.signOut(c); g.open(); join(t); Handler.drain(); empty(c);
        check(!new File(Msa.accountFile(c).getPath() + ".tmp").exists(), "Temporary token survived");
        ok("sign-out during refresh removes both token files and prevents native publication");

        c = context(); g = new Gate("/consumers/oauth2/v2.0/devicecode");
        Listener l = new Listener(); Msa.start(c, "test-client", l); g.await(); Msa.signOut(c); g.open();
        signInsFinished(); Handler.drain(); empty(c); check(l.codes + l.done + l.errors == 0, "Stale callback delivered");
        ok("sign-out during device-code request drops all callbacks");

        c = context(); g = new Gate("/minecraft/profile"); l = new Listener(); Msa.start(c, "test-client", l);
        g.await(); Msa.signOut(c); g.open(); signInsFinished(); Handler.drain(); empty(c);
        check(l.codes + l.done + l.errors == 0, "Queued code or stale completion delivered");
        ok("sign-out before sign-in final write drops account, queued code and completion");

        c = context(); l = new Listener(); Msa.start(c, "test-client", l); signInsFinished();
        Msa.signOut(c); Handler.drain(); empty(c); check(l.codes + l.done + l.errors == 0, "Queued completion survived");
        ok("callbacks already queued before sign-out are invalidated at delivery");

        c = context(); g = new Gate("/minecraft/profile"); Listener old = new Listener(), current = new Listener();
        Msa.start(c, "test-client", old); g.await(); Msa.start(c, "test-client", current);
        g.open(); signInsFinished(); Handler.drain();
        check(old.codes + old.done + old.errors == 0 && current.done == 1 && current.codes == 1, "New sign-in failed to supersede old one");
        ok("new sign-in invalidates older sign-in and its queued UI callbacks");

        c = context(); invalidGrant = true; g = new Gate("/consumers/oauth2/v2.0/token"); t = refresh(c); g.await();
        invalidGrant = false; l = new Listener(); Msa.start(c, "test-client", l); signInsFinished();
        g.open(); join(t); Handler.drain();
        check(Msa.accountFile(c).exists() && l.done == 1, "Old invalid_grant deleted the new account");
        check(HorizonActivity.refreshed == 0, "Stale refresh notified native code");
        ok("stale invalid_grant cannot sign out a newer successful sign-in");

        c = context(); g = new Gate("/minecraft/profile"); t = refresh(c); g.await();
        l = new Listener(); Msa.start(c, "test-client", l); g.open(); join(t); signInsFinished(); Handler.drain();
        check(l.done == 1 && HorizonActivity.refreshed == 0, "Old successful refresh was published");
        ok("new sign-in invalidates an older successful refresh");

        c = context(); g = new Gate("/minecraft/profile"); l = new Listener(); Msa.start(c, "test-client", l);
        g.await(); check(Msa.refresh(c) != null, "Refresh read the previous account during a new sign-in");
        g.open(); signInsFinished(); Handler.drain();
        check(l.done == 1 && HorizonActivity.refreshed == 0, "Refresh competed with active sign-in");
        ok("refresh cannot capture an old account while an interactive sign-in is active");

        c = context(); failPoll.set(true); l = new Listener(); Msa.start(c, "test-client", l);
        signInsFinished(); Handler.drain(); check(l.done == 1 && l.errors == 0, "Transient poll failure was not retried");
        ok("transient polling outage preserves the device code and completes sign-in");

        c = context(); check(Msa.refresh(c) == null && HorizonActivity.refreshed == 1, "Current refresh failed");
        ok("current refresh publishes once under the session lock");
        c = context(); invalidGrant = true; Msa.refresh(c); empty(c);
        ok("current invalid_grant removes the expired account");
        System.out.println(passed + " Microsoft session regression tests passed");
    }
}
