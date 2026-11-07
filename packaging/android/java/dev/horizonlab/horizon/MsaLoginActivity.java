package dev.horizonlab.horizon;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Bitmap;
import android.net.Uri;
import android.os.Bundle;
import android.view.View;
import android.webkit.CookieManager;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import android.widget.ProgressBar;

/**
 * The quick Microsoft sign-in: login.live.com in a WebView. The player types an email and a
 * password (or picks a remembered account); the oauth20_desktop.srf redirect is caught here
 * and its code goes back to the launcher (extra "code"), which runs the token chain (Msa).
 * Nothing typed here is read or stored by the app.
 */
public class MsaLoginActivity extends Activity {
    private WebView web;
    private boolean done;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        FrameLayout root = new FrameLayout(this);
        root.setBackgroundColor(Ui.BG);
        web = new WebView(this);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setSaveFormData(false);
        CookieManager.getInstance().setAcceptCookie(true);
        final ProgressBar spin = new ProgressBar(this);
        web.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest r) {
                return capture(r.getUrl().toString());
            }

            @Override
            @SuppressWarnings("deprecation")
            public boolean shouldOverrideUrlLoading(WebView v, String url) {
                return capture(url);
            }

            @Override
            public void onPageStarted(WebView v, String url, Bitmap icon) {
                if (capture(url)) v.stopLoading();
                spin.setVisibility(View.VISIBLE);
            }

            @Override
            public void onPageFinished(WebView v, String url) {
                spin.setVisibility(View.GONE);
            }
        });
        root.addView(web, new FrameLayout.LayoutParams(-1, -1));
        FrameLayout.LayoutParams sp = new FrameLayout.LayoutParams(Ui.dp(this, 40), Ui.dp(this, 40), android.view.Gravity.CENTER);
        root.addView(spin, sp);
        setContentView(root);
        web.loadUrl(Msa.liveAuthorizeUrl());
    }

    /** Whether `url` is the redirect: then the result is sent and the screen closes. */
    private boolean capture(String url) {
        if (url == null || !url.startsWith(Msa.LIVE_REDIRECT)) return false;
        if (done) return true;
        done = true;
        Uri u = Uri.parse(url);
        String code = u.getQueryParameter("code");
        Intent result = new Intent();
        if (code != null && !code.isEmpty()) {
            result.putExtra("code", code);
            setResult(RESULT_OK, result);
        } else {
            String err = u.getQueryParameter("error");
            result.putExtra("error", err == null ? "" : err);
            setResult(RESULT_CANCELED, result);
        }
        finish();
        return true;
    }

    @Override
    public void onBackPressed() {
        if (web != null && web.canGoBack()) web.goBack();
        else super.onBackPressed();
    }

    @Override
    protected void onDestroy() {
        if (web != null) {
            web.stopLoading();
            web.destroy();
        }
        super.onDestroy();
    }

    /** Forgets the remembered Microsoft session (sign-out). */
    static void forgetCookies() {
        try {
            CookieManager.getInstance().removeAllCookies(null);
            CookieManager.getInstance().flush();
        } catch (Throwable ignored) {
            // No WebView on this device.
        }
    }
}
