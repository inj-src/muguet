// muguet - LogReport.java
// Constantin Clerc (c22dev) - 0.1

package ch.cclerc.muguet;

import android.app.ActivityManager;
import android.app.ApplicationExitInfo;
import android.content.Context;
import android.os.Build;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.List;
import java.util.Locale;

/** What to paste in a bug report: the device, how the game last stopped, and its log. */
final class LogReport {
    private LogReport() {}

    static String build(Context c) {
        StringBuilder out = new StringBuilder("Muguet log report\n\n");
        out.append("device: ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL);
        if (Build.VERSION.SDK_INT >= 31) out.append(", ").append(Build.SOC_MANUFACTURER).append(' ').append(Build.SOC_MODEL);
        out.append("\nandroid: ").append(Build.VERSION.RELEASE).append(" (API ").append(Build.VERSION.SDK_INT).append(")\n");
        ActivityManager am = (ActivityManager) c.getSystemService(Context.ACTIVITY_SERVICE);
        ActivityManager.MemoryInfo mem = new ActivityManager.MemoryInfo();
        am.getMemoryInfo(mem);
        out.append(String.format(Locale.ROOT, "memory: %.1f GB, %.1f GB available\n", mem.totalMem / 1e9, mem.availMem / 1e9));

        if (Build.VERSION.SDK_INT >= 30) {
            out.append("\nlast exits:\n");
            SimpleDateFormat time = new SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.ROOT);
            for (ApplicationExitInfo e : am.getHistoricalProcessExitReasons(c.getPackageName(), 0, 4)) {
                out.append(String.format(Locale.ROOT, "- %s: %s (status %d, %s, rss %d MB, pss %d MB)\n",
                        time.format(new Date(e.getTimestamp())), reason(e.getReason()), e.getStatus(),
                        e.getDescription(), e.getRss() >> 10, e.getPss() >> 10));
            }
        }

        File dir = Native.supportDir(c);
        append(out, new File(dir, "muguet-previous.log"), "previous run");
        append(out, new File(dir, "muguet.log"), "this run");
        return out.toString();
    }

    /** Why the game stopped last time, if it wasn't closed normally; else null. */
    static String lastProblem(Context c) {
        if (Build.VERSION.SDK_INT < 30) return null;
        ActivityManager am = (ActivityManager) c.getSystemService(Context.ACTIVITY_SERVICE);
        List<ApplicationExitInfo> exits = am.getHistoricalProcessExitReasons(c.getPackageName(), 0, 1);
        if (exits.isEmpty()) return null;
        ApplicationExitInfo e = exits.get(0);
        // only report each exit once
        long seen = c.getSharedPreferences("muguet", 0).getLong("seen_exit", 0);
        c.getSharedPreferences("muguet", 0).edit().putLong("seen_exit", e.getTimestamp()).apply();
        if (e.getTimestamp() <= seen) return null;
        switch (e.getReason()) {
            case ApplicationExitInfo.REASON_LOW_MEMORY:
                return "The game was closed by Android: the device ran out of memory. Try Textures: Normal and a lower resolution.";
            case ApplicationExitInfo.REASON_CRASH:
            case ApplicationExitInfo.REASON_CRASH_NATIVE:
            case ApplicationExitInfo.REASON_ANR:
            case ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE:
            case ApplicationExitInfo.REASON_INITIALIZATION_FAILURE:
                return "The game stopped unexpectedly (" + reason(e.getReason()) + "). Tap \"Share log\" to report it.";
            default:
                return null;
        }
    }

    private static String reason(int r) {
        switch (r) {
            case ApplicationExitInfo.REASON_EXIT_SELF: return "exited";
            case ApplicationExitInfo.REASON_SIGNALED: return "killed by a signal";
            case ApplicationExitInfo.REASON_LOW_MEMORY: return "out of memory";
            case ApplicationExitInfo.REASON_CRASH: return "java crash";
            case ApplicationExitInfo.REASON_CRASH_NATIVE: return "native crash";
            case ApplicationExitInfo.REASON_ANR: return "not responding";
            case ApplicationExitInfo.REASON_INITIALIZATION_FAILURE: return "failed to start";
            case ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE: return "excessive resource use";
            case ApplicationExitInfo.REASON_USER_REQUESTED: return "closed by the user";
            case ApplicationExitInfo.REASON_USER_STOPPED: return "force stopped";
            default: return "reason " + r;
        }
    }

    /** The log's start (device and GPU setup) and its end (what happened last). */
    private static void append(StringBuilder out, File log, String title) {
        out.append("\n==== ").append(title).append(" (").append(log.getName()).append(") ====\n");
        List<String> lines;
        try {
            lines = Files.readAllLines(log.toPath(), StandardCharsets.UTF_8);
        } catch (Exception e) {
            out.append("(none)\n");
            return;
        }
        int head = 80, tail = 300;
        for (int i = 0; i < lines.size(); i++) {
            if (i == head && lines.size() > head + tail) {
                out.append("… ").append(lines.size() - head - tail).append(" lines …\n");
                i = lines.size() - tail;
            }
            String l = lines.get(i);
            out.append(l.length() > 600 ? l.substring(0, 600) + "…" : l).append('\n');
        }
    }
}
