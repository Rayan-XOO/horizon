package dev.horizonlab.horizon;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.util.Locale;

/** Shared look of the setup and profile screens: dark, blocky, one centered card. */
final class Ui {
    static final int BG = Color.rgb(18, 18, 22);
    static final int CARD = Color.rgb(36, 36, 44);
    static final int EDGE = Color.rgb(60, 60, 72);
    static final int GREEN = Color.rgb(60, 160, 70);
    static final int GREEN_DARK = Color.rgb(40, 110, 48);
    static final int TEXT = Color.rgb(236, 236, 240);
    static final int MUTED = Color.rgb(160, 160, 172);
    static final int WARN = Color.rgb(240, 190, 90);

    private Ui() {}

    static boolean ru() {
        return "ru".equals(Locale.getDefault().getLanguage());
    }

    static String t(String ru, String en) {
        return ru() ? ru : en;
    }

    static int dp(Context c, float v) {
        return (int) (v * c.getResources().getDisplayMetrics().density + 0.5f);
    }

    static GradientDrawable rounded(Context c, int fill, int stroke, float radius) {
        GradientDrawable d = new GradientDrawable();
        d.setColor(fill);
        d.setCornerRadius(dp(c, radius));
        if (stroke != 0) d.setStroke(dp(c, 1), stroke);
        return d;
    }

    /** A full-screen background with a centered card; returns the card to fill. */
    static LinearLayout card(android.app.Activity a) {
        FrameLayout root = new FrameLayout(a);
        root.setBackground(new GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM,
                new int[] {Color.rgb(28, 34, 40), BG, Color.rgb(10, 10, 13)}));
        android.widget.ScrollView scroll = new android.widget.ScrollView(a);
        scroll.setFillViewport(true);
        FrameLayout center = new FrameLayout(a);
        LinearLayout card = new LinearLayout(a);
        card.setOrientation(LinearLayout.VERTICAL);
        int p = dp(a, 22);
        card.setPadding(p, p, p, p);
        card.setBackground(rounded(a, CARD, EDGE, 18));
        card.setElevation(dp(a, 6));
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(
                Math.min(dp(a, 520), a.getResources().getDisplayMetrics().widthPixels - dp(a, 32)),
                FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.CENTER);
        lp.setMargins(0, dp(a, 16), 0, dp(a, 16));
        center.addView(card, lp);
        scroll.addView(center);
        root.addView(scroll);
        a.setContentView(root);
        return card;
    }

    static TextView text(Context c, String s, float size, int color, boolean bold) {
        TextView t = new TextView(c);
        t.setText(s);
        t.setTextSize(size);
        t.setTextColor(color);
        t.setTypeface(bold ? Typeface.create("sans-serif-medium", Typeface.BOLD) : Typeface.create("sans-serif", Typeface.NORMAL));
        t.setLineSpacing(0, 1.15f);
        t.setPadding(0, dp(c, 4), 0, dp(c, 4));
        return t;
    }

    static Button button(Context c, String s, boolean primary) {
        Button b = new Button(c);
        b.setText(s);
        b.setAllCaps(false);
        b.setTextSize(primary ? 17 : 15);
        b.setTextColor(primary ? Color.WHITE : TEXT);
        b.setTypeface(Typeface.create("sans-serif-medium", Typeface.BOLD));
        b.setBackground(primary ? rounded(c, GREEN, GREEN_DARK, 10) : rounded(c, Color.rgb(52, 52, 62), EDGE, 10));
        b.setMinHeight(dp(c, primary ? 54 : 44));
        b.setStateListAnimator(null);
        return b;
    }

    static LinearLayout.LayoutParams gap(Context c, int top) {
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        lp.topMargin = dp(c, top);
        return lp;
    }

    /** A section header: a thin divider and a small green caption (empty title: divider only). */
    static void section(LinearLayout card, String title) {
        Context c = card.getContext();
        View line = new View(c);
        line.setBackgroundColor(EDGE);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, Math.max(1, dp(c, 1)));
        lp.topMargin = dp(c, 18);
        lp.bottomMargin = dp(c, 8);
        card.addView(line, lp);
        if (title.isEmpty()) return;
        TextView t = text(c, title.toUpperCase(Locale.getDefault()), 12, GREEN, true);
        t.setLetterSpacing(0.08f);
        card.addView(t);
    }

    /** A rounded progress bar, 0..1000. */
    static final class Bar extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF r = new RectF();
        private int value;

        Bar(Context c) {
            super(c);
            setMinimumHeight(dp(c, 14));
        }

        void set(int permille) {
            value = Math.max(0, Math.min(1000, permille));
            invalidate();
        }

        @Override
        protected void onMeasure(int w, int h) {
            setMeasuredDimension(MeasureSpec.getSize(w), dp(getContext(), 14));
        }

        @Override
        protected void onDraw(Canvas canvas) {
            float rad = getHeight() / 2f;
            r.set(0, 0, getWidth(), getHeight());
            paint.setColor(Color.rgb(24, 24, 30));
            canvas.drawRoundRect(r, rad, rad, paint);
            if (value > 0) {
                r.set(0, 0, Math.max(getHeight(), getWidth() * value / 1000f), getHeight());
                paint.setColor(GREEN);
                canvas.drawRoundRect(r, rad, rad, paint);
            }
        }
    }
}
