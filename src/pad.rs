// muguet - pad.rs
// Constantin Clerc (c22dev) - 0.1

use crate::input;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub static ACTIVE: AtomicBool = AtomicBool::new(false);

pub static IN_VEHICLE: AtomicBool = AtomicBool::new(false);

pub static HORN_PAD: AtomicBool = AtomicBool::new(false);
pub static HORN_TOUCH: AtomicBool = AtomicBool::new(false);
pub fn horn_held() -> bool {
    HORN_PAD.load(Ordering::Relaxed) || HORN_TOUCH.load(Ordering::Relaxed)
}

struct State {
    held: HashSet<&'static str>,
    buttons: u8,
    last: Instant,
    down_since: Option<Instant>,
}
static STATE: Mutex<Option<State>> = Mutex::new(None);

const SWITCH_HOLD: Duration = Duration::from_millis(300);
const STICK_DEADZONE: f32 = 0.3;
const LOOK_DEADZONE: f32 = 0.12;

const LOOK_SPEED: f64 = 700.0;

/// One reading of the controller, sticks up and right positive.
#[derive(Clone, Copy, Default)]
pub struct Gamepad {
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    pub lt: f32,
    pub rt: f32,
    pub buttons: u32,
}

pub mod btn {
    pub const A: u32 = 1 << 0;
    pub const B: u32 = 1 << 1;
    pub const X: u32 = 1 << 2;
    pub const Y: u32 = 1 << 3;
    pub const LB: u32 = 1 << 4;
    pub const RB: u32 = 1 << 5;
    pub const L3: u32 = 1 << 6;
    pub const R3: u32 = 1 << 7;
    pub const UP: u32 = 1 << 8;
    pub const DOWN: u32 = 1 << 9;
    pub const LEFT: u32 = 1 << 10;
    pub const RIGHT: u32 = 1 << 11;
    pub const MENU: u32 = 1 << 12;
    pub const OPTIONS: u32 = 1 << 13;
}

#[cfg(target_vendor = "apple")]
fn read() -> Option<Gamepad> {
    use objc2_game_controller::{GCController, GCControllerButtonInput};
    let gp = unsafe { GCController::controllers().iter().find_map(|c| c.extendedGamepad()) }?;
    fn down(b: &GCControllerButtonInput) -> bool {
        unsafe { b.isPressed() }
    }
    unsafe {
        let (ls, rs, dp) = (gp.leftThumbstick(), gp.rightThumbstick(), gp.dpad());
        let mut buttons = 0;
        for (on, bit) in [
            (down(&gp.buttonA()), btn::A), (down(&gp.buttonB()), btn::B), (down(&gp.buttonX()), btn::X), (down(&gp.buttonY()), btn::Y),
            (down(&gp.leftShoulder()), btn::LB), (down(&gp.rightShoulder()), btn::RB),
            (gp.leftThumbstickButton().is_some_and(|b| down(&b)), btn::L3), (gp.rightThumbstickButton().is_some_and(|b| down(&b)), btn::R3),
            (down(&dp.up()), btn::UP), (down(&dp.down()), btn::DOWN), (down(&dp.left()), btn::LEFT), (down(&dp.right()), btn::RIGHT),
            (down(&gp.buttonMenu()), btn::MENU), (gp.buttonOptions().is_some_and(|b| down(&b)), btn::OPTIONS),
        ] {
            if on { buttons |= bit; }
        }
        Some(Gamepad {
            lx: ls.xAxis().value(), ly: ls.yAxis().value(), rx: rs.xAxis().value(), ry: rs.yAxis().value(),
            lt: gp.leftTrigger().value(), rt: gp.rightTrigger().value(), buttons,
        })
    }
}

#[cfg(target_os = "android")]
fn read() -> Option<Gamepad> {
    crate::android::gamepad()
}

#[cfg(not(any(target_vendor = "apple", target_os = "android")))]
fn read() -> Option<Gamepad> {
    None
}

pub fn poll() -> bool {
    let Some(gp) = read() else {
        if ACTIVE.swap(false, Ordering::Relaxed) {
            if let Some(st) = STATE.lock().unwrap().take() { release(st); }
            eprintln!("[pad] controller disconnected");
        }
        return false;
    };
    if !ACTIVE.swap(true, Ordering::Relaxed) {
        eprintln!("[pad] controller connected");
    }
    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(|| State { held: HashSet::new(), buttons: 0, last: Instant::now(), down_since: None });
    let dt = st.last.elapsed().as_secs_f64().min(0.05);
    st.last = Instant::now();

    let in_car = IN_VEHICLE.load(Ordering::Relaxed);
    let mut keys: HashSet<&'static str> = HashSet::new();
    let mut buttons = 0u8;
    let down = |bit: u32| gp.buttons & bit != 0;
    let (lx, ly) = (gp.lx, gp.ly);
    if lx < -STICK_DEADZONE { keys.insert("KeyA"); }
    if lx > STICK_DEADZONE { keys.insert("KeyD"); }
    if in_car {
        if ly > STICK_DEADZONE { keys.insert("Numpad8"); }
        if ly < -STICK_DEADZONE { keys.insert("Numpad2"); }
        if gp.rt > 0.2 { keys.insert("KeyW"); }
        if gp.lt > 0.2 { keys.insert("KeyS"); }
        if down(btn::RB) { keys.insert("Space"); }
        HORN_PAD.store(down(btn::L3), Ordering::Relaxed);
        if down(btn::X) { keys.insert("Space"); }
    } else {
        if ly > STICK_DEADZONE { keys.insert("KeyW"); }
        if ly < -STICK_DEADZONE { keys.insert("KeyS"); }
        if gp.rt > 0.2 { buttons |= 1; }
        if gp.lt > 0.2 { buttons |= 2; }
        if down(btn::A) { keys.insert("ShiftLeft"); }
        if down(btn::X) { keys.insert("Space"); }
        if down(btn::RB) { keys.insert("KeyQ"); }
        if down(btn::L3) { keys.insert("ControlLeft"); }
    }
    if down(btn::A) { keys.insert("Enter"); }
    if down(btn::B) { keys.insert("KeyR"); keys.insert("Backspace"); }
    if down(btn::Y) { keys.insert("KeyF"); }
    if down(btn::LB) { keys.insert("Tab"); }
    if down(btn::R3) { keys.insert("KeyC"); }
    if down(btn::UP) { keys.insert("ArrowUp"); }
    if down(btn::DOWN) {
        let since = *st.down_since.get_or_insert_with(Instant::now);
        if since.elapsed() >= SWITCH_HOLD { keys.insert("AltLeft"); } else { keys.insert("ArrowDown"); }
    } else {
        st.down_since = None;
    }
    if down(btn::LEFT) { keys.insert("ArrowLeft"); }
    if down(btn::RIGHT) { keys.insert("ArrowRight"); }
    if down(btn::MENU) { keys.insert("Escape"); }
    if down(btn::OPTIONS) { keys.insert("KeyV"); }

    let curve = |v: f32| -> f64 {
        let a = v.abs();
        if a < LOOK_DEADZONE { 0.0 } else { (((a - LOOK_DEADZONE) / (1.0 - LOOK_DEADZONE)) as f64).powf(1.8).copysign(v as f64) }
    };
    let (dx, dy) = (curve(gp.rx), -curve(gp.ry));
    if dx != 0.0 || dy != 0.0 {
        input::mouse_delta_f(dx * LOOK_SPEED * dt, dy * LOOK_SPEED * dt);
    }

    if crate::MOBILE { input::set_locked(true); }

    for k in st.held.difference(&keys) { input::key(k, false, false, None, false); }
    for k in keys.difference(&st.held) {
        crate::mp::key_pressed(k);
        input::key(k, true, false, None, false);
    }
    st.held = keys;
    for (bit, dom) in [(1u8, 0usize), (2, 2)] {
        if (buttons ^ st.buttons) & bit != 0 { input::button(dom, buttons & bit != 0); }
    }
    st.buttons = buttons;
    true
}

fn release(st: State) {
    for k in st.held { input::key(k, false, false, None, false); }
    if st.buttons & 1 != 0 { input::button(0, false); }
    if st.buttons & 2 != 0 { input::button(2, false); }
}
