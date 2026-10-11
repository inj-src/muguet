// muguet - Native.java
// Constantin Clerc (c22dev) - 0.1

package ch.cclerc.muguet;

import android.content.Context;
import java.io.File;

/** libmuguet.so (src/android.rs). */
final class Native {
    static {
        System.loadLibrary("muguet");
    }

    private Native() {}

    /** Where config.json, saves and the game live: the app's external files dir (what android_main uses). */
    static File supportDir(Context c) {
        File external = c.getExternalFilesDir(null);
        return external != null ? external : c.getFilesDir();
    }

    static void setup(Context c) {
        init(supportDir(c).getAbsolutePath(), c.getFilesDir().getAbsolutePath());
    }

    static native void init(String supportDir, String internalDir);

    static native boolean gameReady();

    /** Imports the game zip open at fd; null, or what went wrong. Blocks: call it off the UI thread. */
    static native String importZip(int fd);

    static native long importDone();

    static native long importTotal();

    /** Whether the game's code is already compiled for this phone. */
    static native boolean codeReady();

    /** Compiles the game's code once (minutes); null, or what went wrong. Blocks. */
    static native String precompile();

    /** phase: 0 down, 1 move, 2 up, 3 cancel; position in window pixels. */
    static native void touch(int id, int phase, float x, float y);

    static native void key(int keyCode, boolean down, int unicodeChar, boolean repeat);

    /** Controller buttons (GameActivity.BTN_*) and axes as Android reports them. */
    static native void pad(int buttons, float lx, float ly, float rx, float ry, float lt, float rt);

    static native void padGone();
}
