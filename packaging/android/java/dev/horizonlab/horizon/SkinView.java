package dev.horizonlab.horizon;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Rect;
import android.view.View;

/**
 * The skin's front, flat and pixel-sharp: head, body, arms and legs with their outer layers,
 * slim arms 3 pixels wide. Steve's silhouette in grey when there is no skin.
 */
final class SkinView extends View {
    private Bitmap skin;
    private boolean slim;
    private final Paint paint = new Paint();
    private final Rect src = new Rect(), dst = new Rect();

    SkinView(Context c) {
        super(c);
        paint.setFilterBitmap(false);
        paint.setAntiAlias(false);
        paint.setDither(false);
    }

    void set(Bitmap skin, boolean slim) {
        this.skin = skin;
        this.slim = slim;
        invalidate();
    }

    @Override
    protected void onDraw(Canvas c) {
        // The figure is 16x32 skin pixels.
        int px = Math.max(1, Math.min(getWidth() / 16, getHeight() / 32));
        int ox = (getWidth() - 16 * px) / 2, oy = (getHeight() - 32 * px) / 2;
        if (skin == null) {
            paint.setColor(Color.rgb(58, 58, 70));
            box(c, px, ox, oy, 4, 0, 8, 8);
            box(c, px, ox, oy, 4, 8, 8, 12);
            box(c, px, ox, oy, 0, 8, 4, 12);
            box(c, px, ox, oy, 12, 8, 4, 12);
            box(c, px, ox, oy, 4, 20, 4, 12);
            box(c, px, ox, oy, 8, 20, 4, 12);
            return;
        }
        boolean legacy = skin.getHeight() == 32;
        int arm = slim ? 3 : 4;
        // Base layer.
        part(c, px, ox, oy, 8, 8, 8, 8, 4, 0, false);
        part(c, px, ox, oy, 20, 20, 8, 12, 4, 8, false);
        part(c, px, ox, oy, 44, 20, arm, 12, 4 - arm, 8, false);
        if (legacy) part(c, px, ox, oy, 44, 20, arm, 12, 12, 8, true);
        else part(c, px, ox, oy, 36, 52, arm, 12, 12, 8, false);
        part(c, px, ox, oy, 4, 20, 4, 12, 4, 20, false);
        if (legacy) part(c, px, ox, oy, 4, 20, 4, 12, 8, 20, true);
        else part(c, px, ox, oy, 20, 52, 4, 12, 8, 20, false);
        // Outer layer (the hat; jacket, sleeves and trousers on 64x64 skins).
        part(c, px, ox, oy, 40, 8, 8, 8, 4, 0, false);
        if (!legacy) {
            part(c, px, ox, oy, 20, 36, 8, 12, 4, 8, false);
            part(c, px, ox, oy, 44, 36, arm, 12, 4 - arm, 8, false);
            part(c, px, ox, oy, 52, 52, arm, 12, 12, 8, false);
            part(c, px, ox, oy, 4, 36, 4, 12, 4, 20, false);
            part(c, px, ox, oy, 4, 52, 4, 12, 8, 20, false);
        }
    }

    private void box(Canvas c, int px, int ox, int oy, int x, int y, int w, int h) {
        c.drawRect(ox + x * px, oy + y * px, ox + (x + w) * px, oy + (y + h) * px, paint);
    }

    private void part(Canvas c, int px, int ox, int oy, int x, int y, int w, int h, int dx, int dy, boolean mirror) {
        dst.set(ox + dx * px, oy + dy * px, ox + (dx + w) * px, oy + (dy + h) * px);
        if (!mirror) {
            src.set(x, y, x + w, y + h);
            c.drawBitmap(skin, src, dst, paint);
            return;
        }
        // Legacy skins: the left limbs are the right ones mirrored, a column at a time.
        for (int i = 0; i < w; i++) {
            src.set(x + w - 1 - i, y, x + w - i, y + h);
            dst.set(ox + (dx + i) * px, oy + dy * px, ox + (dx + i + 1) * px, oy + (dy + h) * px);
            c.drawBitmap(skin, src, dst, paint);
        }
    }
}
