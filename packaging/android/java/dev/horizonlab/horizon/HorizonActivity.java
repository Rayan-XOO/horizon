package dev.horizonlab.horizon;

import android.app.NativeActivity;
import android.content.Context;
import android.os.Bundle;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;

/**
 * NativeActivity plus a text bridge: an invisible view takes the soft
 * keyboard's text (any script, not only what maps to key codes) and hands it
 * to the game (libhorizon's nativeText/nativeKey). The game calls
 * showKeyboard while a text field has focus. The window draws under the
 * display cutout.
 */
public class HorizonActivity extends NativeActivity {
    static {
        System.loadLibrary("horizon");
    }

    static native void nativeText(String text);
    /** 1 = backspace, 2 = enter. */
    static native void nativeKey(int key);
    /** The saved Microsoft account was renewed (Msa.refresh). */
    static native void nativeAccountRefreshed();

    private TextSink sink;
    private volatile boolean quitting;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        applyCutout(this);
        sink = new TextSink(this);
        addContentView(sink, new ViewGroup.LayoutParams(1, 1));
        renewAccount();
    }

    /** Keeps the Microsoft sign-in alive: renews it in the background when it is about to expire. */
    private static volatile boolean renewing;

    private void renewAccount() {
        if (renewing || !Msa.needsRefresh(this)) return;
        renewing = true;
        final android.content.Context app = getApplicationContext();
        new Thread(new Runnable() {
            public void run() {
                if (Msa.refresh(app) == null) {
                    try {
                        nativeAccountRefreshed();
                    } catch (Throwable ignored) {
                    }
                }
                renewing = false;
            }
        }, "horizon-msa-refresh").start();
    }

    @Override
    protected void onResume() {
        super.onResume();
        renewAccount();
    }

    /** 0 short edges, 1 never (letterbox past the camera), 2 always. */
    static void applyCutout(android.app.Activity a) {
        if (android.os.Build.VERSION.SDK_INT < 28) return;
        int mode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        try {
            String t = new String(java.nio.file.Files.readAllBytes(new java.io.File(a.getFilesDir(), "cutout.txt").toPath()), java.nio.charset.StandardCharsets.UTF_8).trim();
            if (t.equals("never")) mode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_NEVER;
            else if (t.equals("always")) mode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS;
        } catch (Exception ignored) {
        }
        WindowManager.LayoutParams lp = a.getWindow().getAttributes();
        lp.layoutInDisplayCutoutMode = mode;
        a.getWindow().setAttributes(lp);
    }

    /** Called from the game thread: the profile screen (name, skin). */
    public void openProfile() {
        startActivity(new android.content.Intent(this, ProfileActivity.class));
    }

    /**
     * Called from the game thread once the native loop has exited (Quit Game, or the game could
     * not go on). NativeActivity can't run the game twice in one process, so the task is removed
     * and the process ends; the next launch starts clean instead of on a dead black window.
     * Only this explicit path ends the process: opening Profile or Files, or the system
     * recreating the activity, never does.
     */
    public void quit() {
        quitting = true;
        runOnUiThread(new Runnable() {
            public void run() {
                finishAndRemoveTask();
            }
        });
        new Thread(new Runnable() {
            public void run() {
                try {
                    Thread.sleep(300);
                } catch (InterruptedException ignored) {
                }
                System.exit(0);
            }
        }, "horizon-quit").start();
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        // The window went away for good while the game still ran in this process (swiped away,
        // or finished by the system). The native side can't start again here: end the process
        // so the next launch is a fresh one. A configuration change is not an end.
        if (isFinishing() && !isChangingConfigurations() && !quitting) {
            System.exit(0);
        }
    }

    /**
     * Called from the game thread: the Files app on {@code rel}, a folder inside the game folder
     * ("" for the game folder itself). Falls back to the game folder's root, then to Profile.
     */
    public void openGameFolder(final String rel) {
        runOnUiThread(new Runnable() {
            public void run() {
                String authority = "dev.horizonlab.horizon.documents";
                android.net.Uri root = android.provider.DocumentsContract.buildRootUri(authority, "horizon");
                java.util.List<android.content.Intent> tries = new java.util.ArrayList<>();
                if (rel != null && !rel.isEmpty() && !rel.contains("..")) {
                    android.content.Intent dir = new android.content.Intent(android.content.Intent.ACTION_VIEW);
                    dir.setDataAndType(android.provider.DocumentsContract.buildDocumentUri(authority, "root:" + rel),
                            android.provider.DocumentsContract.Document.MIME_TYPE_DIR);
                    tries.add(dir);
                }
                android.content.Intent top = new android.content.Intent(android.content.Intent.ACTION_VIEW);
                top.setData(root);
                tries.add(top);
                for (android.content.Intent i : tries) {
                    i.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK
                            | android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    try {
                        startActivity(i);
                        return;
                    } catch (Exception ignored) {
                    }
                }
                startActivity(new android.content.Intent(HorizonActivity.this, ProfileActivity.class));
            }
        });
    }

    /** Called from the game thread while a text field has the focus. */
    public void showKeyboard(final boolean show) {
        runOnUiThread(new Toggle(this, show));
    }

    static class Toggle implements Runnable {
        private final HorizonActivity a;
        private final boolean show;

        Toggle(HorizonActivity a, boolean show) {
            this.a = a;
            this.show = show;
        }

        public void run() {
            TextSink sink = a.sink;
            {
                InputMethodManager imm = (InputMethodManager) a.getSystemService(Context.INPUT_METHOD_SERVICE);
                if (show) {
                    sink.setFocusable(true);
                    sink.setFocusableInTouchMode(true);
                    sink.requestFocus();
                    imm.restartInput(sink);
                    imm.showSoftInput(sink, InputMethodManager.SHOW_FORCED);
                } else {
                    imm.hideSoftInputFromWindow(sink.getWindowToken(), 0);
                    sink.clearFocus();
                }
            }
        }
    }

    static class TextSink extends View {
        TextSink(Context c) {
            super(c);
        }

        @Override
        public boolean onCheckIsTextEditor() {
            return true;
        }

        @Override
        public InputConnection onCreateInputConnection(EditorInfo info) {
            // Plain text: a password variation would lock some keyboards to Latin letters.
            info.inputType = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
            info.imeOptions = EditorInfo.IME_ACTION_DONE | EditorInfo.IME_FLAG_NO_FULLSCREEN
                    | EditorInfo.IME_FLAG_NO_EXTRACT_UI;
            return new Connection(this);
        }
    }

    static class Connection extends BaseInputConnection {
                private String composing = "";

                Connection(View view) {
                    super(view, false);
                }

                @Override
                public boolean commitText(CharSequence text, int pos) {
                    composing = "";
                    if (text.length() > 0) nativeText(text.toString());
                    return true;
                }

                @Override
                public boolean setComposingText(CharSequence text, int pos) {
                    String t = text.toString();
                    // Compose by sending the difference: deletes then the new tail.
                    int common = 0;
                    while (common < t.length() && common < composing.length()
                            && t.charAt(common) == composing.charAt(common)) common++;
                    for (int i = common; i < composing.length(); i++) nativeKey(1);
                    if (common < t.length()) nativeText(t.substring(common));
                    composing = t;
                    return true;
                }

                @Override
                public boolean finishComposingText() {
                    composing = "";
                    return true;
                }

                @Override
                public boolean deleteSurroundingText(int before, int after) {
                    for (int i = 0; i < before; i++) nativeKey(1);
                    return true;
                }

                @Override
                public boolean performEditorAction(int action) {
                    composing = "";
                    nativeKey(2);
                    return true;
                }

                @Override
                public boolean sendKeyEvent(KeyEvent e) {
                    if (e.getAction() == KeyEvent.ACTION_DOWN) {
                        int code = e.getKeyCode();
                        if (code == KeyEvent.KEYCODE_DEL) {
                            nativeKey(1);
                        } else if (code == KeyEvent.KEYCODE_ENTER) {
                            nativeKey(2);
                        } else {
                            int ch = e.getUnicodeChar();
                            if (ch != 0) nativeText(new String(Character.toChars(ch)));
                        }
                    }
                    return true;
                }
    }
}
