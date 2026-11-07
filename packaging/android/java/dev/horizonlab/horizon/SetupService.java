package dev.horizonlab.horizon;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.net.wifi.WifiManager;
import android.os.Handler;
import android.os.IBinder;
import android.os.PowerManager;

/**
 * Keeps the first-run download going with the app in the background or the screen off: a
 * dataSync foreground service with a progress notification. The work itself is native
 * (android_setup.rs, started by SetupActivity.nativeStart); this only holds the process up.
 */
public class SetupService extends Service implements Runnable {
    static final String CHANNEL = "setup";
    static final int ID = 1;

    private final Handler handler = new Handler();
    private PowerManager.WakeLock wake;
    private WifiManager.WifiLock wifi;
    private NotificationManager nm;

    @Override
    public void onCreate() {
        super.onCreate();
        nm = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        if (android.os.Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(new NotificationChannel(CHANNEL,
                    Ui.t("Загрузка файлов игры", "Game files download"), NotificationManager.IMPORTANCE_LOW));
        }
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        Notification n = build(Ui.t("Подготовка", "Preparing"), 0, true, true);
        if (android.os.Build.VERSION.SDK_INT >= 29) {
            startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC);
        } else {
            startForeground(ID, n);
        }
        if (wake == null) {
            wake = ((PowerManager) getSystemService(Context.POWER_SERVICE))
                    .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "horizon:setup");
            wake.acquire(3 * 60 * 60 * 1000L);
            wifi = ((WifiManager) getApplicationContext().getSystemService(Context.WIFI_SERVICE))
                    .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "horizon:setup");
            wifi.acquire();
        }
        String home = intent != null ? intent.getStringExtra("home") : null;
        String url = intent != null ? intent.getStringExtra("url") : null;
        if (home != null) {
            SetupActivity.nativeStart(home, url == null ? "" : url);
        }
        handler.removeCallbacks(this);
        handler.post(this);
        return START_NOT_STICKY;
    }

    private Notification build(String text, int permille, boolean ongoing, boolean indeterminate) {
        Intent open = new Intent(this, SetupActivity.class);
        PendingIntent pi = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE);
        Notification.Builder b = android.os.Build.VERSION.SDK_INT >= 26
                ? new Notification.Builder(this, CHANNEL) : new Notification.Builder(this);
        b.setSmallIcon(android.R.drawable.stat_sys_download)
                .setContentTitle("Horizon")
                .setContentText(text)
                .setContentIntent(pi)
                .setOngoing(ongoing)
                .setOnlyAlertOnce(true);
        if (ongoing) {
            b.setProgress(1000, permille, indeterminate);
        } else {
            b.setSmallIcon(android.R.drawable.stat_sys_download_done).setAutoCancel(true);
        }
        return b.build();
    }

    public void run() {
        String[] p = SetupActivity.nativeStatus().split("\t", -1);
        if (p.length < 7) {
            handler.postDelayed(this, 1000);
            return;
        }
        long files = Long.parseLong(p[1]), filesTotal = Long.parseLong(p[2]);
        long bytes = Long.parseLong(p[3]), bytesTotal = Long.parseLong(p[4]);
        if (p[5].equals("done") || p[5].equals("error") || p[5].equals("idle")) {
            String text = p[5].equals("done") ? Ui.t("Файлы игры готовы", "Game files are ready") : SetupActivity.friendlyError(p[6]);
            stopForeground(true);
            nm.notify(ID + 1, build(text, 0, false, false));
            stopSelf();
            return;
        }
        int permille = 0;
        String text = SetupActivity.stageName(p[0]);
        if (bytesTotal > 0) {
            permille = (int) (1000 * bytes / Math.max(1, bytesTotal));
            text += "  " + (bytes >> 20) + " / " + (bytesTotal >> 20) + " MB";
        } else if (filesTotal > 0) {
            permille = (int) (1000 * files / Math.max(1, filesTotal));
            text += "  " + files + " / " + filesTotal;
        }
        text += "  " + (permille / 10) + "%";
        nm.notify(ID, build(text, permille, true, bytesTotal == 0 && filesTotal == 0));
        handler.postDelayed(this, 1000);
    }

    @Override
    public void onDestroy() {
        handler.removeCallbacks(this);
        if (wake != null && wake.isHeld()) wake.release();
        if (wifi != null && wifi.isHeld()) wifi.release();
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
