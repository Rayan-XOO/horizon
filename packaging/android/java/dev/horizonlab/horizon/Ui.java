package dev.horizonlab.horizon;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.ColorFilter;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.File;
import java.util.Locale;
import java.util.Random;

/**
 * Shared look of the setup and profile screens, in the style of the game's own menus: square
 * stone buttons with a light top edge and a dark bottom edge, dark see-through panels, a pixel
 * font (Monocraft, SIL OFL, bundled in assets/fonts) with a drop shadow, and the title
 * panorama behind everything.
 *
 * Nothing of Mojang's ships in the APK: the panorama is read at run time from the files the
 * player's own game download put in the game folder, and without them a background drawn here
 * (a dirt-like pixel pattern) stands in.
 */
final class Ui {
    static final int BG = Color.rgb(18, 18, 22);
    static final int CARD = Color.argb(200, 16, 16, 20);
    static final int EDGE = Color.rgb(0, 0, 0);
    static final int GREEN = Color.rgb(85, 255, 85);
    static final int GREEN_DARK = Color.rgb(44, 110, 40);
    static final int TEXT = Color.rgb(255, 255, 255);
    static final int MUTED = Color.rgb(170, 170, 170);
    static final int WARN = Color.rgb(255, 255, 85);
    static final int SHADOW = Color.rgb(40, 40, 40);

    private static Typeface font;

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

    /** The pixel font; the system's monospace when the asset can't be read. */
    static Typeface font(Context c) {
        if (font == null) {
            try {
                font = Typeface.createFromAsset(c.getAssets(), "fonts/Monocraft.ttf");
            } catch (Exception e) {
                font = Typeface.MONOSPACE;
            }
        }
        return font;
    }

    /** One "game pixel" on screen. */
    static int px(Context c) {
        return Math.max(2, dp(c, 2));
    }

    /**
     * A panel or field: `fill` with a one-pixel `stroke` rim (none for 0) and, for a field, the
     * sunken look of the game's text boxes. The radius is ignored: everything is square here.
     */
    static Drawable rounded(Context c, int fill, int stroke, float radius) {
        return new Panel(px(c) / 2 + 1, fill, stroke);
    }

    /** A full-screen panorama with a centered panel; returns the panel to fill. */
    static LinearLayout card(android.app.Activity a) {
        FrameLayout root = new FrameLayout(a);
        root.addView(new Panorama(a), new FrameLayout.LayoutParams(-1, -1));
        android.widget.ScrollView scroll = new android.widget.ScrollView(a);
        scroll.setFillViewport(true);
        FrameLayout center = new FrameLayout(a);
        LinearLayout card = new LinearLayout(a);
        card.setOrientation(LinearLayout.VERTICAL);
        int p = dp(a, 20);
        card.setPadding(p, p, p, p);
        card.setBackground(rounded(a, CARD, Color.rgb(90, 90, 90), 0));
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(
                Math.min(dp(a, 540), a.getResources().getDisplayMetrics().widthPixels - dp(a, 32)),
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
        // The pixel font reads larger than the system one at the same size.
        t.setTextSize(size * 0.92f);
        t.setTextColor(color);
        t.setTypeface(font(c));
        t.getPaint().setFakeBoldText(bold && size >= 18);
        t.setShadowLayer(0.01f, px(c) / 2f + 0.5f, px(c) / 2f + 0.5f, shade(color));
        t.setLineSpacing(0, 1.15f);
        t.setPadding(0, dp(c, 4), 0, dp(c, 4));
        return t;
    }

    /** The text's drop shadow: a quarter of its colour, as the game draws it. */
    static int shade(int color) {
        return Color.rgb(Color.red(color) / 4, Color.green(color) / 4, Color.blue(color) / 4);
    }

    static Button button(Context c, String s, boolean primary) {
        Button b = new Button(c);
        b.setText(s);
        b.setAllCaps(false);
        b.setTextSize(primary ? 16 : 14);
        b.setTextColor(new android.content.res.ColorStateList(
                new int[][] {new int[] {-android.R.attr.state_enabled}, new int[] {}},
                new int[] {Color.rgb(160, 160, 160), primary ? Color.WHITE : Color.rgb(224, 224, 224)}));
        b.setTypeface(font(c));
        b.setShadowLayer(0.01f, px(c) / 2f + 0.5f, px(c) / 2f + 0.5f, Color.rgb(56, 56, 56));
        b.setBackground(new StoneButton(px(c), primary));
        b.setMinHeight(dp(c, primary ? 52 : 42));
        b.setStateListAnimator(null);
        b.setSoundEffectsEnabled(true);
        return b;
    }

    static LinearLayout.LayoutParams gap(Context c, int top) {
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        lp.topMargin = dp(c, top);
        return lp;
    }

    /** A section header: a pixel divider and a caption (empty title: divider only). */
    static void section(LinearLayout card, String title) {
        Context c = card.getContext();
        View line = new View(c);
        line.setBackgroundColor(Color.argb(160, 120, 120, 120));
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, px(c) / 2 + 1);
        lp.topMargin = dp(c, 16);
        lp.bottomMargin = dp(c, 6);
        card.addView(line, lp);
        if (title.isEmpty()) return;
        TextView t = text(c, title, 13, WARN, false);
        card.addView(t);
    }

    /** A square panel with a dark rim and a faint inner light line. */
    static final class Panel extends Drawable {
        private final Paint paint = new Paint();
        private final int unit, fill, stroke;

        Panel(int unit, int fill, int stroke) {
            this.unit = unit;
            this.fill = fill;
            this.stroke = stroke;
        }

        @Override
        public void draw(Canvas c) {
            Rect b = getBounds();
            paint.setColor(fill);
            c.drawRect(b, paint);
            if (stroke == 0) return;
            paint.setColor(Color.BLACK);
            edges(c, b.left, b.top, b.right, b.bottom, unit);
            paint.setColor(stroke);
            edges(c, b.left + unit, b.top + unit, b.right - unit, b.bottom - unit, unit);
        }

        private void edges(Canvas c, int l, int t, int r, int bo, int w) {
            c.drawRect(l, t, r, t + w, paint);
            c.drawRect(l, bo - w, r, bo, paint);
            c.drawRect(l, t, l + w, bo, paint);
            c.drawRect(r - w, t, r, bo, paint);
        }

        @Override
        public boolean getPadding(Rect padding) {
            return false;
        }

        @Override public void setAlpha(int a) {}
        @Override public void setColorFilter(ColorFilter f) {}
        @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }
    }

    /**
     * The game's button, drawn: a black outline, a stone face with a lighter top-left edge and a
     * darker bottom-right one; lighter with a white outline while pressed or focused, flat grey
     * when disabled. The primary button (Play) is green.
     */
    static final class StoneButton extends Drawable {
        private final Paint paint = new Paint();
        private final int unit;
        private final boolean primary;
        private boolean pressed, focused, enabled = true;

        StoneButton(int unit, boolean primary) {
            this.unit = unit;
            this.primary = primary;
        }

        @Override
        public boolean isStateful() {
            return true;
        }

        @Override
        protected boolean onStateChange(int[] state) {
            boolean p = false, f = false, e = false;
            for (int s : state) {
                if (s == android.R.attr.state_pressed) p = true;
                if (s == android.R.attr.state_focused || s == android.R.attr.state_hovered) f = true;
                if (s == android.R.attr.state_enabled) e = true;
            }
            boolean changed = p != pressed || f != focused || e != enabled;
            pressed = p;
            focused = f;
            enabled = e;
            if (changed) invalidateSelf();
            return changed;
        }

        @Override
        public void draw(Canvas c) {
            Rect b = getBounds();
            int u = unit;
            int face, hi, lo;
            if (!enabled) {
                face = Color.rgb(44, 44, 44);
                hi = Color.rgb(60, 60, 60);
                lo = Color.rgb(34, 34, 34);
            } else if (primary) {
                face = pressed || focused ? Color.rgb(84, 168, 60) : Color.rgb(60, 133, 39);
                hi = pressed || focused ? Color.rgb(132, 214, 102) : Color.rgb(99, 178, 72);
                lo = Color.rgb(30, 82, 22);
            } else {
                face = pressed || focused ? Color.rgb(126, 136, 191) : Color.rgb(111, 111, 111);
                hi = pressed || focused ? Color.rgb(170, 180, 230) : Color.rgb(164, 164, 164);
                lo = pressed || focused ? Color.rgb(78, 86, 140) : Color.rgb(78, 78, 78);
            }
            paint.setColor(pressed || focused ? Color.WHITE : Color.BLACK);
            c.drawRect(b, paint);
            paint.setColor(lo);
            c.drawRect(b.left + u, b.top + u, b.right - u, b.bottom - u, paint);
            paint.setColor(hi);
            c.drawRect(b.left + u, b.top + u, b.right - 2 * u, b.bottom - 2 * u, paint);
            paint.setColor(face);
            c.drawRect(b.left + 2 * u, b.top + 2 * u, b.right - 2 * u, b.bottom - 2 * u, paint);
        }

        @Override
        public boolean getPadding(Rect padding) {
            int p = unit * 4;
            padding.set(p, unit * 2, p, unit * 2);
            return true;
        }

        @Override public void setAlpha(int a) {}
        @Override public void setColorFilter(ColorFilter f) {}
        @Override public int getOpacity() { return PixelFormat.OPAQUE; }
    }

    /**
     * The title screen's background: the panorama's four side faces in a row, blurred and
     * darkened, panning slowly (as the 1.12 title menu did). The faces come from the game
     * folder (`resources/assets/minecraft/textures/gui/title/background/panorama_0..3.png`, put
     * there by the player's game download); a pixel pattern drawn here stands in without them.
     */
    static final class Panorama extends View {
        private Bitmap strip;
        private final Paint paint = new Paint(Paint.FILTER_BITMAP_FLAG);
        private final Rect src = new Rect(), dst = new Rect();
        private final long start = android.os.SystemClock.uptimeMillis();
        private boolean loaded;

        Panorama(Context c) {
            super(c);
        }

        private void load() {
            loaded = true;
            File dir = new File(SetupActivity.home(getContext()), "resources/assets/minecraft/textures/gui/title/background");
            Bitmap[] faces = new Bitmap[4];
            BitmapFactory.Options o = new BitmapFactory.Options();
            o.inPreferredConfig = Bitmap.Config.ARGB_8888;
            try {
                for (int i = 0; i < 4; i++) {
                    faces[i] = BitmapFactory.decodeFile(new File(dir, "panorama_" + i + ".png").getPath(), o);
                    if (faces[i] == null) throw new Exception("face " + i);
                }
                // Small first: scaling up a small copy with filtering is the blur.
                int s = 96;
                strip = Bitmap.createBitmap(s * 5, s, Bitmap.Config.ARGB_8888);
                Canvas c = new Canvas(strip);
                Paint p = new Paint(Paint.FILTER_BITMAP_FLAG);
                for (int i = 0; i < 5; i++) {
                    Bitmap f = faces[i % 4];
                    c.drawBitmap(f, new Rect(0, 0, f.getWidth(), f.getHeight()), new Rect(i * s, 0, (i + 1) * s, s), p);
                }
                for (Bitmap f : faces) f.recycle();
            } catch (Throwable t) {
                strip = pattern();
            }
        }

        /** A dirt-like pattern of brown pixels, drawn here: no game file needed. */
        private static Bitmap pattern() {
            int s = 16;
            Bitmap tile = Bitmap.createBitmap(s * 5, s, Bitmap.Config.ARGB_8888);
            Random r = new Random(7);
            int[] tones = {Color.rgb(89, 61, 41), Color.rgb(108, 76, 52), Color.rgb(72, 50, 34), Color.rgb(121, 85, 58), Color.rgb(96, 96, 96)};
            for (int y = 0; y < s; y++) {
                for (int x = 0; x < s; x++) {
                    int v = r.nextInt(100);
                    int tone = v < 4 ? tones[4] : v < 30 ? tones[2] : v < 70 ? tones[0] : v < 92 ? tones[1] : tones[3];
                    for (int k = 0; k < 5; k++) tile.setPixel(k * s + x, y, tone);
                }
            }
            return tile;
        }

        @Override
        protected void onAttachedToWindow() {
            super.onAttachedToWindow();
            postInvalidateOnAnimation();
        }

        @Override
        protected void onDraw(Canvas c) {
            if (!loaded) load();
            int w = getWidth(), h = getHeight();
            int sh = strip.getHeight();
            boolean pixelArt = sh <= 16;
            paint.setFilterBitmap(!pixelArt);
            if (pixelArt) {
                // Tiles of the pattern at 4 screen pixels a texel, like the old options background.
                int cell = Math.max(32, dp(getContext(), 32));
                src.set(0, 0, sh, sh);
                for (int y = 0; y < h; y += cell) {
                    for (int x = 0; x < w; x += cell) {
                        dst.set(x, y, x + cell, y + cell);
                        c.drawBitmap(strip, src, dst, paint);
                    }
                }
                c.drawColor(Color.argb(150, 0, 0, 0));
            } else {
                // The strip fills the height; it pans one face every 40 seconds and wraps.
                float scale = h / (float) sh;
                float face = sh * scale;
                float t = ((android.os.SystemClock.uptimeMillis() - start) % 160_000L) / 40_000f;
                float off = t * face;
                float viewInStrip = w / scale;
                int left = (int) (off / scale) % (sh * 4);
                int right = Math.min(strip.getWidth(), left + (int) Math.ceil(viewInStrip));
                src.set(left, 0, right, sh);
                dst.set(0, 0, (int) ((right - left) * scale), h);
                c.drawBitmap(strip, src, dst, paint);
                if (dst.right < w) {
                    src.set(0, 0, (int) Math.ceil((w - dst.right) / scale) + 1, sh);
                    dst.set(dst.right, 0, w + (int) scale, h);
                    c.drawBitmap(strip, src, dst, paint);
                }
                c.drawColor(Color.argb(110, 0, 0, 0));
                postInvalidateOnAnimation();
            }
        }
    }

    /** A square progress bar, 0..1000, in the game's green. */
    static final class Bar extends View {
        private final Paint paint = new Paint();
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
            int u = px(getContext()) / 2 + 1;
            int w = getWidth(), h = getHeight();
            paint.setColor(Color.BLACK);
            canvas.drawRect(0, 0, w, h, paint);
            paint.setColor(Color.rgb(50, 50, 50));
            canvas.drawRect(u, u, w - u, h - u, paint);
            if (value > 0) {
                int end = u + (int) ((w - 2 * u) * value / 1000f);
                paint.setColor(Color.rgb(60, 133, 39));
                canvas.drawRect(u, u, end, h - u, paint);
                paint.setColor(Color.rgb(99, 178, 72));
                canvas.drawRect(u, u, end, u + (h - 2 * u) / 3, paint);
            }
        }
    }
}
