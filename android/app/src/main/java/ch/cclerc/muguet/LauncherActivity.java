// muguet - LauncherActivity.java
// Constantin Clerc (c22dev) - 0.1

package ch.cclerc.muguet;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.graphics.Insets;
import android.graphics.Typeface;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelFileDescriptor;
import android.os.StatFs;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.widget.ArrayAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.RadioButton;
import android.widget.RadioGroup;
import android.widget.ScrollView;
import android.widget.SeekBar;
import android.widget.Spinner;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.Toast;
import java.io.File;
import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.NetworkInterface;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** Imports the game, compiles its code once, edits config.json, then opens GameActivity (like ios/Launcher.swift). */
public class LauncherActivity extends Activity {
    private static final int PICK_ZIP = 1;
    private enum Phase { IDLE, IMPORTING, PREPARING, COMPILING }

    private final Handler ui = new Handler(Looper.getMainLooper());
    private final Runnable tick = this::refresh;
    private Phase phase = Phase.IDLE;
    private String error;
    private JSONObject config = new JSONObject();

    private TextView status;
    private ProgressBar progress;
    private Button primary, secondary;
    private Spinner mode, scale, textures, fps;
    private SeekBar touch;
    private TextView touchValue, hostInfo;
    private Switch mute, multiplayer;
    private LinearLayout multiplayerBox;
    private RadioGroup session;
    private EditText name, server;

    private static final double[] SCALES = {0.35, 0.5, 0.75, 1.0};

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Native.setup(this);
        loadConfig();
        setContentView(build());
        File last = new File(Native.supportDir(this), "last_error.txt");
        if (last.exists()) {
            try {
                error = new String(Files.readAllBytes(last.toPath()), StandardCharsets.UTF_8).trim();
            } catch (Exception ignored) {
            }
            last.delete();
        }
        if (error == null) error = LogReport.lastProblem(this);
        refresh();
    }

    // ---- layout ----

    private int dp(float v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }

    private View build() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.HORIZONTAL);
        int pad = dp(24);
        root.setPadding(pad, pad, pad, pad);
        if (Build.VERSION.SDK_INT >= 30) {
            root.setOnApplyWindowInsetsListener((v, insets) -> {
                Insets i = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                v.setPadding(pad + i.left, pad + i.top, pad + i.right, pad + i.bottom);
                return WindowInsets.CONSUMED;
            });
        } else {
            root.setFitsSystemWindows(true);
        }

        LinearLayout left = new LinearLayout(this);
        left.setOrientation(LinearLayout.VERTICAL);
        left.setGravity(Gravity.CENTER);
        TextView title = new TextView(this);
        title.setText("Muguet");
        title.setTextSize(34);
        title.setTypeface(Typeface.DEFAULT_BOLD);
        left.addView(title);
        status = new TextView(this);
        status.setGravity(Gravity.CENTER);
        status.setPadding(0, dp(16), 0, dp(8));
        left.addView(status, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        progress = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        progress.setMax(1000);
        left.addView(progress, new LinearLayout.LayoutParams(dp(260), ViewGroup.LayoutParams.WRAP_CONTENT));
        primary = new Button(this);
        primary.setOnClickListener(v -> {
            if (Native.gameReady()) play();
            else pickZip();
        });
        left.addView(primary, new LinearLayout.LayoutParams(dp(220), ViewGroup.LayoutParams.WRAP_CONTENT));
        secondary = new Button(this, null, android.R.attr.borderlessButtonStyle);
        secondary.setText("Import other game data…");
        secondary.setAllCaps(false);
        secondary.setOnClickListener(v -> pickZip());
        left.addView(secondary);
        Button share = new Button(this, null, android.R.attr.borderlessButtonStyle);
        share.setText("Share log");
        share.setAllCaps(false);
        share.setOnClickListener(v -> shareLog());
        left.addView(share);
        TextView credit = new TextView(this);
        credit.setText("proudly made by c22dev");
        credit.setAlpha(0.6f);
        credit.setPadding(0, dp(12), 0, 0);
        left.addView(credit);
        root.addView(left, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1));

        ScrollView scroll = new ScrollView(this);
        LinearLayout form = new LinearLayout(this);
        form.setOrientation(LinearLayout.VERTICAL);
        form.setPadding(dp(16), 0, 0, 0);
        scroll.addView(form);
        root.addView(scroll, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1));

        mode = spinner(form, "Mode", "Story Mode", "Sandbox");
        mode.setSelection(config.optInt("start_mode", 2) == 1 ? 0 : 1);
        scale = spinner(form, "Resolution", "35 % (fastest)", "50 %", "75 %", "100 % (sharpest)");
        scale.setSelection(closest(SCALES, config.optDouble("scale", 0.5)));
        textures = spinner(form, "Textures", "Normal (less memory)", "High", "Very High");
        textures.setSelection(Math.max(0, Math.min(2, config.optInt("texture_quality", 0))));
        fps = spinner(form, "Frame limit", "30 fps (cooler)", "60 fps");
        fps.setSelection(config.optInt("fps_limit", 60) == 30 ? 0 : 1);

        LinearLayout sens = row(form, "Look sensitivity");
        touch = new SeekBar(this);
        touch.setMax(11); // 0.25 ... 3 by 0.25
        touch.setProgress((int) Math.round(Math.max(0.25, Math.min(3, config.optDouble("touch_sensitivity", 1.0))) / 0.25) - 1);
        touchValue = new TextView(this);
        touchValue.setMinWidth(dp(52));
        touchValue.setGravity(Gravity.END);
        touch.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar s, int p, boolean user) { touchValue.setText(String.format("%.2f×", (p + 1) * 0.25)); }
            @Override public void onStartTrackingTouch(SeekBar s) {}
            @Override public void onStopTrackingTouch(SeekBar s) {}
        });
        touchValue.setText(String.format("%.2f×", (touch.getProgress() + 1) * 0.25));
        sens.addView(touch, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        sens.addView(touchValue);

        mute = toggle(form, "Mute audio", config.optBoolean("mute", false));

        TextView section = new TextView(this);
        section.setText("MULTIPLAYER");
        section.setAlpha(0.6f);
        section.setPadding(0, dp(18), 0, dp(4));
        form.addView(section);
        multiplayer = toggle(form, "Play with friends", config.optBoolean("multiplayer", false));
        multiplayerBox = new LinearLayout(this);
        multiplayerBox.setOrientation(LinearLayout.VERTICAL);
        form.addView(multiplayerBox);
        name = new EditText(this);
        name.setHint("Your name");
        name.setSingleLine(true);
        String player = config.optString("player_name", "");
        name.setText(player.isEmpty() ? Build.MODEL : player);
        multiplayerBox.addView(name);
        session = new RadioGroup(this);
        session.setOrientation(RadioGroup.HORIZONTAL);
        RadioButton join = new RadioButton(this);
        join.setText("Join");
        join.setId(View.generateViewId());
        RadioButton host = new RadioButton(this);
        host.setText("Host");
        host.setId(View.generateViewId());
        session.addView(join);
        session.addView(host);
        session.check(config.optBoolean("host", false) ? host.getId() : join.getId());
        multiplayerBox.addView(session);
        hostInfo = new TextView(this);
        hostInfo.setTextIsSelectable(true);
        hostInfo.setText("Friends join with: " + String.join(" or ", localIps()));
        multiplayerBox.addView(hostInfo);
        server = new EditText(this);
        server.setHint("Host address, e.g. 192.168.1.20");
        server.setSingleLine(true);
        server.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        server.setText(config.optString("server", ""));
        multiplayerBox.addView(server);
        multiplayer.setOnCheckedChangeListener((b, on) -> refresh());
        session.setOnCheckedChangeListener((g, id) -> refresh());
        return root;
    }

    private LinearLayout row(LinearLayout form, String label) {
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setMinimumHeight(dp(48));
        TextView l = new TextView(this);
        l.setText(label);
        l.setMinWidth(dp(130));
        row.addView(l);
        form.addView(row);
        return row;
    }

    private Spinner spinner(LinearLayout form, String label, String... items) {
        Spinner s = new Spinner(this);
        ArrayAdapter<String> a = new ArrayAdapter<>(this, android.R.layout.simple_spinner_item, items);
        a.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item);
        s.setAdapter(a);
        row(form, label).addView(s, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        return s;
    }

    private Switch toggle(LinearLayout form, String label, boolean on) {
        Switch s = new Switch(this);
        s.setText(label);
        s.setChecked(on);
        s.setMinimumHeight(dp(48));
        form.addView(s, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return s;
    }

    private static int closest(double[] values, double v) {
        int best = 0;
        for (int i = 1; i < values.length; i++) if (Math.abs(values[i] - v) < Math.abs(values[best] - v)) best = i;
        return best;
    }

    private boolean hosting() {
        return session.getCheckedRadioButtonId() == session.getChildAt(1).getId();
    }

    private void refresh() {
        boolean busy = phase != Phase.IDLE;
        boolean ready = Native.gameReady();
        multiplayerBox.setVisibility(multiplayer.isChecked() ? View.VISIBLE : View.GONE);
        hostInfo.setVisibility(hosting() ? View.VISIBLE : View.GONE);
        server.setVisibility(hosting() ? View.GONE : View.VISIBLE);
        primary.setText(ready ? "Play" : "Import game data .zip");
        primary.setEnabled(!busy);
        secondary.setVisibility(ready && !busy ? View.VISIBLE : View.GONE);
        progress.setVisibility(busy ? View.VISIBLE : View.GONE);
        progress.setIndeterminate(phase == Phase.PREPARING || phase == Phase.COMPILING);
        String text;
        switch (phase) {
            case IMPORTING: {
                long total = Native.importTotal(), done = Native.importDone();
                int p = total > 0 ? (int) (done * 1000 / total) : 0;
                progress.setProgress(p);
                text = "Importing… " + p / 10 + " %";
                break;
            }
            case PREPARING: text = "Loading…"; break;
            case COMPILING: text = "Compiling the game for this device (one time, a few minutes)…"; break;
            default:
                text = ready ? "" : "Import your game data: a .zip of the folder holding data/ and b/<build>/ (with game.wasm and shaders/).";
        }
        if (error != null && !busy) text = text.isEmpty() ? error : error + "\n\n" + text;
        status.setText(text);
        status.setTextColor(error != null && !busy ? 0xFFFF6B6B : 0xFFCCCCCC);
        ui.removeCallbacks(tick);
        if (phase == Phase.IMPORTING) ui.postDelayed(tick, 250);
    }

    /** Copies the log report and offers to send it. */
    private void shareLog() {
        String report = LogReport.build(this);
        ((ClipboardManager) getSystemService(CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText("Muguet log", report));
        Toast.makeText(this, "Log copied", Toast.LENGTH_SHORT).show();
        Intent send = new Intent(Intent.ACTION_SEND);
        send.setType("text/plain");
        send.putExtra(Intent.EXTRA_SUBJECT, "Muguet log");
        send.putExtra(Intent.EXTRA_TEXT, report);
        startActivity(Intent.createChooser(send, "Share log"));
    }

    // ---- import, compile, play ----

    private void pickZip() {
        Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        i.addCategory(Intent.CATEGORY_OPENABLE);
        i.setType("*/*");
        i.putExtra(Intent.EXTRA_MIME_TYPES, new String[] {"application/zip", "application/x-zip-compressed", "application/octet-stream"});
        startActivityForResult(i, PICK_ZIP);
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == PICK_ZIP && result == RESULT_OK && data != null && data.getData() != null) importZip(data.getData());
    }

    private void importZip(Uri uri) {
        error = null;
        phase = Phase.IMPORTING;
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        refresh();
        new Thread(() -> {
            String err;
            try (ParcelFileDescriptor pfd = getContentResolver().openFileDescriptor(uri, "r")) {
                if (pfd == null) err = "could not open the file";
                else if ((err = spaceFor(pfd.getStatSize())) == null) err = Native.importZip(pfd.getFd());
            } catch (Exception e) {
                err = String.valueOf(e.getMessage());
            }
            String failed = err == null ? null : "Import failed: " + err;
            ui.post(() -> {
                if (failed != null) done(failed);
                else prepare(false);
            });
        }, "import").start();
    }

    /** The game unpacks to about the zip's size (its files are compressed already). Null if it fits. */
    private String spaceFor(long zipSize) {
        if (zipSize <= 0) return null;
        long free = new StatFs(Native.supportDir(this).getPath()).getAvailableBytes();
        long need = zipSize + (1L << 30); // and room for the compiled game code
        if (free >= need) return null;
        return String.format("not enough free space: the game needs about %.1f GB, the device has %.1f GB free", need / 1e9, free / 1e9);
    }

    /** Compiles the game's code if this device doesn't have it yet, then plays if asked. */
    private void prepare(boolean thenPlay) {
        error = null;
        phase = Phase.PREPARING;
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        refresh();
        new Thread(() -> {
            String err = null;
            if (!Native.codeReady()) {
                ui.post(() -> { phase = Phase.COMPILING; refresh(); });
                err = Native.precompile();
            }
            String failed = err == null ? null : "Could not compile the game: " + err;
            ui.post(() -> {
                done(failed);
                if (failed == null && thenPlay) {
                    startActivity(new Intent(this, GameActivity.class));
                    finish();
                }
            });
        }, "precompile").start();
    }

    private void done(String failed) {
        error = failed;
        phase = Phase.IDLE;
        getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        refresh();
    }

    private void play() {
        if (multiplayer.isChecked() && !hosting() && server.getText().toString().trim().isEmpty()) {
            error = "Enter the host's address to join.";
            refresh();
            return;
        }
        try {
            saveConfig();
        } catch (Exception e) {
            error = "Could not save settings: " + e.getMessage();
            refresh();
            return;
        }
        prepare(true);
    }

    // ---- config.json (src/config.rs) ----

    private File configFile() {
        return new File(Native.supportDir(this), "config.json");
    }

    private void loadConfig() {
        try {
            config = new JSONObject(new String(Files.readAllBytes(configFile().toPath()), StandardCharsets.UTF_8));
        } catch (Exception e) {
            config = new JSONObject();
        }
    }

    private void saveConfig() throws Exception {
        config.put("start_mode", mode.getSelectedItemPosition() == 0 ? 1 : 2);
        config.put("scale", SCALES[scale.getSelectedItemPosition()]);
        config.put("texture_quality", textures.getSelectedItemPosition());
        config.put("fps_limit", fps.getSelectedItemPosition() == 0 ? 30 : 60);
        config.put("touch_sensitivity", (touch.getProgress() + 1) * 0.25);
        config.put("mute", mute.isChecked());
        config.put("multiplayer", multiplayer.isChecked());
        config.put("host", hosting());
        config.put("server", server.getText().toString().trim());
        config.put("player_name", name.getText().toString().trim());
        if (!config.has("extra_args")) config.put("extra_args", new JSONArray());
        Files.write(configFile().toPath(), config.toString(2).getBytes(StandardCharsets.UTF_8));
    }

    private static List<String> localIps() {
        List<String> out = new ArrayList<>();
        try {
            for (NetworkInterface n : Collections.list(NetworkInterface.getNetworkInterfaces())) {
                if (!n.isUp() || n.isLoopback()) continue;
                for (InetAddress a : Collections.list(n.getInetAddresses())) {
                    if (a instanceof Inet4Address && !out.contains(a.getHostAddress())) out.add(a.getHostAddress());
                }
            }
        } catch (Exception ignored) {
        }
        if (out.isEmpty()) out.add("?");
        return out;
    }
}
