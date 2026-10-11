// muguet - GameActivity.java
// Constantin Clerc (c22dev) - 0.1

package ch.cclerc.muguet;

import android.app.NativeActivity;
import android.content.Context;
import android.hardware.input.InputManager;
import android.os.Build;
import android.os.Bundle;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;

/**
 * The game. NativeActivity runs android_main (src/android.rs) for the window and the engine, but
 * input stays here and goes to it through Native: native input would turn controller sticks
 * into touches.
 */
public class GameActivity extends NativeActivity implements InputManager.InputDeviceListener {
    // src/pad.rs, mod btn
    static final int BTN_A = 1, BTN_B = 1 << 1, BTN_X = 1 << 2, BTN_Y = 1 << 3, BTN_LB = 1 << 4, BTN_RB = 1 << 5,
            BTN_L3 = 1 << 6, BTN_R3 = 1 << 7, BTN_UP = 1 << 8, BTN_DOWN = 1 << 9, BTN_LEFT = 1 << 10,
            BTN_RIGHT = 1 << 11, BTN_MENU = 1 << 12, BTN_OPTIONS = 1 << 13;
    private static final int DPAD = BTN_UP | BTN_DOWN | BTN_LEFT | BTN_RIGHT;

    private int padKeys, padHat;
    private boolean l2, r2;
    private float lx, ly, rx, ry, lt, rt;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        Native.setup(this);
        super.onCreate(savedInstanceState);
        // before the window is attached: input goes through the dispatch* methods below
        getWindow().takeInputQueue(null);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        ((InputManager) getSystemService(Context.INPUT_SERVICE)).registerInputDeviceListener(this, null);
        immersive();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) immersive();
    }

    @Override
    protected void onDestroy() {
        // the engine can't stop or start over in this process, and NativeActivity would wait for it
        android.os.Process.killProcess(android.os.Process.myPid());
        super.onDestroy();
    }

    @SuppressWarnings("deprecation")
    private void immersive() {
        if (Build.VERSION.SDK_INT >= 30) {
            getWindow().setDecorFitsSystemWindows(false);
            WindowInsetsController c = getWindow().getInsetsController();
            if (c != null) {
                c.hide(WindowInsets.Type.systemBars());
                c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN);
        }
    }

    @Override
    public boolean dispatchTouchEvent(MotionEvent e) {
        switch (e.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
            case MotionEvent.ACTION_POINTER_DOWN: {
                int i = e.getActionIndex();
                Native.touch(e.getPointerId(i), 0, e.getX(i), e.getY(i));
                break;
            }
            case MotionEvent.ACTION_MOVE:
                for (int i = 0; i < e.getPointerCount(); i++) Native.touch(e.getPointerId(i), 1, e.getX(i), e.getY(i));
                break;
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_POINTER_UP: {
                int i = e.getActionIndex();
                Native.touch(e.getPointerId(i), 2, e.getX(i), e.getY(i));
                break;
            }
            case MotionEvent.ACTION_CANCEL:
                for (int i = 0; i < e.getPointerCount(); i++) Native.touch(e.getPointerId(i), 3, e.getX(i), e.getY(i));
                break;
            default:
                break;
        }
        return true;
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent e) {
        int code = e.getKeyCode();
        switch (code) {
            case KeyEvent.KEYCODE_VOLUME_UP:
            case KeyEvent.KEYCODE_VOLUME_DOWN:
            case KeyEvent.KEYCODE_VOLUME_MUTE:
                return super.dispatchKeyEvent(e);
            default:
                break;
        }
        if (e.getAction() != KeyEvent.ACTION_DOWN && e.getAction() != KeyEvent.ACTION_UP) return true;
        boolean down = e.getAction() == KeyEvent.ACTION_DOWN;
        if (fromController(e) && padKey(code, down)) return true;
        // back (gesture, button or a controller's) opens the pause menu
        if (code == KeyEvent.KEYCODE_BACK) code = KeyEvent.KEYCODE_ESCAPE;
        Native.key(code, down, e.getUnicodeChar(), e.getRepeatCount() > 0);
        return true;
    }

    @Override
    public boolean dispatchGenericMotionEvent(MotionEvent e) {
        if ((e.getSource() & InputDevice.SOURCE_JOYSTICK) != InputDevice.SOURCE_JOYSTICK || e.getAction() != MotionEvent.ACTION_MOVE) {
            return super.dispatchGenericMotionEvent(e);
        }
        lx = e.getAxisValue(MotionEvent.AXIS_X);
        ly = e.getAxisValue(MotionEvent.AXIS_Y);
        rx = e.getAxisValue(MotionEvent.AXIS_Z);
        ry = e.getAxisValue(MotionEvent.AXIS_RZ);
        lt = Math.max(e.getAxisValue(MotionEvent.AXIS_LTRIGGER), e.getAxisValue(MotionEvent.AXIS_BRAKE));
        rt = Math.max(e.getAxisValue(MotionEvent.AXIS_RTRIGGER), e.getAxisValue(MotionEvent.AXIS_GAS));
        float hx = e.getAxisValue(MotionEvent.AXIS_HAT_X), hy = e.getAxisValue(MotionEvent.AXIS_HAT_Y);
        padHat = (hx < -0.5f ? BTN_LEFT : 0) | (hx > 0.5f ? BTN_RIGHT : 0) | (hy < -0.5f ? BTN_UP : 0) | (hy > 0.5f ? BTN_DOWN : 0);
        sendPad();
        return true;
    }

    private static boolean fromController(KeyEvent e) {
        int s = e.getSource();
        return KeyEvent.isGamepadButton(e.getKeyCode())
                || (s & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                || (s & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK;
    }

    private boolean padKey(int code, boolean down) {
        int bit;
        switch (code) {
            case KeyEvent.KEYCODE_BUTTON_A: bit = BTN_A; break;
            case KeyEvent.KEYCODE_BUTTON_B: bit = BTN_B; break;
            case KeyEvent.KEYCODE_BUTTON_X: bit = BTN_X; break;
            case KeyEvent.KEYCODE_BUTTON_Y: bit = BTN_Y; break;
            case KeyEvent.KEYCODE_BUTTON_L1: bit = BTN_LB; break;
            case KeyEvent.KEYCODE_BUTTON_R1: bit = BTN_RB; break;
            case KeyEvent.KEYCODE_BUTTON_THUMBL: bit = BTN_L3; break;
            case KeyEvent.KEYCODE_BUTTON_THUMBR: bit = BTN_R3; break;
            case KeyEvent.KEYCODE_BUTTON_START: bit = BTN_MENU; break;
            case KeyEvent.KEYCODE_BUTTON_SELECT: bit = BTN_OPTIONS; break;
            case KeyEvent.KEYCODE_DPAD_UP: bit = BTN_UP; break;
            case KeyEvent.KEYCODE_DPAD_DOWN: bit = BTN_DOWN; break;
            case KeyEvent.KEYCODE_DPAD_LEFT: bit = BTN_LEFT; break;
            case KeyEvent.KEYCODE_DPAD_RIGHT: bit = BTN_RIGHT; break;
            // controllers without analog triggers
            case KeyEvent.KEYCODE_BUTTON_L2: l2 = down; sendPad(); return true;
            case KeyEvent.KEYCODE_BUTTON_R2: r2 = down; sendPad(); return true;
            default: return false;
        }
        padKeys = down ? padKeys | bit : padKeys & ~bit;
        sendPad();
        return true;
    }

    private void sendPad() {
        int buttons = padKeys | (padHat & DPAD);
        Native.pad(buttons, lx, ly, rx, ry, l2 ? 1f : lt, r2 ? 1f : rt);
    }

    private static boolean anyController() {
        for (int id : InputDevice.getDeviceIds()) {
            InputDevice d = InputDevice.getDevice(id);
            if (d == null || d.isVirtual()) continue;
            int s = d.getSources();
            if ((s & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                    || (s & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK) return true;
        }
        return false;
    }

    @Override
    public void onInputDeviceAdded(int deviceId) {}

    @Override
    public void onInputDeviceChanged(int deviceId) {}

    @Override
    public void onInputDeviceRemoved(int deviceId) {
        if (anyController()) return;
        padKeys = padHat = 0;
        l2 = r2 = false;
        lx = ly = rx = ry = lt = rt = 0;
        Native.padGone();
    }
}
