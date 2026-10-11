// muguet - android.rs
// Constantin Clerc (c22dev) - 0.1

// The Android app: a Java launcher (android/) imports the game and writes config.json, then opens
// GameActivity, a NativeActivity that enters android_main below. Java keeps the input queue and
// forwards touches, keys and controllers through the JNI calls at the bottom of this file
// (winit would turn controller sticks into touches).

use crate::pad::{btn, Gamepad};
use jni_sys::{jboolean, jclass, jfloat, jint, jlong, jstring, JNIEnv, JNI_FALSE, JNI_TRUE};
use std::ffi::{c_char, c_void, CStr, CString};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, Once, OnceLock};
use std::time::{Duration, Instant};
use winit::event::TouchPhase;
use winit::event_loop::EventLoopProxy;
use winit::platform::android::activity::AndroidApp;

static APP: OnceLock<AndroidApp> = OnceLock::new();

pub fn app() -> AndroidApp {
    APP.get().expect("android_main").clone()
}

#[no_mangle]
fn android_main(app: AndroidApp) {
    let support = app.external_data_path().or_else(|| app.internal_data_path());
    let internal = app.internal_data_path();
    set_paths(support, internal);
    APP.set(app).ok();
    crate::raise_open_file_limit();

    // the launcher may have changed config.json since this process last read it
    crate::config::clear_settings();
    let startup = match crate::config::setup() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[config] {e:#}");
            alert("Muguet", &format!("{e:#}"));
            quit(1);
        }
    };
    let args = crate::engine_args(&startup);
    let fps_30 = args.iter().any(|a| a == "-frameLimit=2");
    FRAME_TARGET_NS.store(if fps_30 { 33_333_333 } else { 16_666_667 }, Ordering::Relaxed);
    if let Err(e) = crate::app::run(None, startup, args) {
        eprintln!("[app] {e:#}");
    }
    // the game's state is global: never run it twice in one process
    quit(0);
}

fn set_paths(support: Option<PathBuf>, internal: Option<PathBuf>) {
    if let Some(dir) = support {
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("MUGUET_SUPPORT", dir);
    }
    if let Some(dir) = internal {
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("MUGUET_CACHE", dir);
    }
    log_to_logcat();
}

fn support_dir() -> PathBuf {
    PathBuf::from(std::env::var("MUGUET_SUPPORT").unwrap_or_default())
}

/// Ends the process without exit(): that runs the system libraries' C++ teardown while their
/// threads (UI rendering, audio, the GPU driver) still run, and Android aborts on their destroyed
/// mutexes. The log gets a moment to reach muguet.log first.
pub fn quit(code: i32) -> ! {
    extern "C" {
        fn _exit(code: i32) -> !;
    }
    eprintln!("[app] quitting ({code})");
    std::thread::sleep(Duration::from_millis(300));
    unsafe { _exit(code) }
}

/// The launcher shows this file on its next start.
pub fn alert(title: &str, msg: &str) {
    eprintln!("[{title}] {msg}");
    let _ = std::fs::write(support_dir().join("last_error.txt"), msg);
}

#[link(name = "log")]
extern "C" {
    fn __android_log_write(prio: i32, tag: *const c_char, text: *const c_char) -> i32;
}

/// stdout and stderr (everything the port prints) go to muguet.log next to config.json, and the
/// port's own lines to logcat. Whatever happens to logging, it must never stop the game: every
/// thread that prints writes into this pipe and would wait on it once it's full.
fn log_to_logcat() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        extern "C" {
            fn pipe(fds: *mut i32) -> i32;
            fn dup2(old: i32, new: i32) -> i32;
            fn fcntl(fd: i32, cmd: i32, ...) -> i32;
        }
        const F_SETPIPE_SZ: i32 = 1031;
        let mut fds = [0i32; 2];
        if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
            return;
        }
        unsafe {
            // room for bursts while the file is being written
            fcntl(fds[1], F_SETPIPE_SZ, 1i32 << 20);
            dup2(fds[1], 1);
            dup2(fds[1], 2);
        }
        let read = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(fds[0]) };
        // keep the last run's log: after a crash it's the one that matters (the launcher shares it)
        let log = support_dir().join("muguet.log");
        let _ = std::fs::rename(&log, support_dir().join("muguet-previous.log"));
        let file = std::fs::File::create(&log).ok();
        std::panic::set_hook(Box::new(|info| {
            let thread = std::thread::current().name().unwrap_or("?").to_string();
            eprintln!("[panic] thread {thread}: {info}\n{}", std::backtrace::Backtrace::force_capture());
        }));

        // logcat on its own thread, fed without waiting: if liblog stalls (it did, on a OnePlus
        // phone), lines for logcat are dropped and nothing else notices
        let (logcat, lines) = std::sync::mpsc::sync_channel::<Vec<u8>>(1024);
        std::thread::Builder::new()
            .name("logcat".into())
            .spawn(move || {
                let tag = c"muguet";
                for line in lines {
                    let text = CString::new(line.into_iter().filter(|b| *b != 0 && *b != b'\n').collect::<Vec<u8>>()).unwrap_or_default();
                    unsafe { __android_log_write(4, tag.as_ptr(), text.as_ptr()) };
                }
            })
            .ok();

        std::thread::Builder::new()
            .name("log".into())
            .spawn(move || {
                let mut file = file.map(|f| std::io::BufWriter::with_capacity(64 << 10, f));
                let mut read = std::io::BufReader::with_capacity(64 << 10, read);
                let mut line = Vec::new();
                loop {
                    line.clear();
                    if !matches!(read.read_until(b'\n', &mut line), Ok(n) if n > 0) { break; }
                    LOG_LINES.fetch_add(1, Ordering::Relaxed);
                    LOG_LAST.store(START.elapsed().as_millis() as u64, Ordering::Relaxed);
                    if let Some(f) = file.as_mut() {
                        let _ = f.write_all(&line);
                        if read.buffer().is_empty() { let _ = f.flush(); }
                    }
                    // only the port's "[...]" lines: the engine's chatter would push them out of logcat
                    if line.first() == Some(&b'[') {
                        let _ = logcat.try_send(line.clone());
                    }
                }
                // nobody reads the pipe anymore: printing must not block or fail, send it nowhere
                if let Ok(null) = std::fs::OpenOptions::new().write(true).open("/dev/null") {
                    use std::os::fd::IntoRawFd;
                    let fd = null.into_raw_fd();
                    unsafe {
                        dup2(fd, 1);
                        dup2(fd, 2);
                    }
                }
            })
            .ok();
        status_file();
    });
}

static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
static LOG_LINES: AtomicU64 = AtomicU64::new(0);
static LOG_LAST: AtomicU64 = AtomicU64::new(0);

/// muguet-status.txt, rewritten every 2 s by its own thread and not through the log: when the
/// game freezes it still tells whether the log, the renderer or the engine stopped moving.
fn status_file() {
    let _ = *START;
    std::thread::Builder::new()
        .name("status".into())
        .spawn(|| {
            let path = support_dir().join("muguet-status.txt");
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let now = START.elapsed().as_millis() as u64;
                let heap = crate::RT.get().map(|r| r.mem.data_size() >> 20).unwrap_or(0);
                let text = format!(
                    "uptime {:.0} s\nlog lines {} (last {:.1} s ago)\ngpu started {}, frames presented {}, suspended {}\ngame input ready {}, controller {}\nmemory {} MB, game heap {} MB\n",
                    now as f64 / 1e3,
                    LOG_LINES.load(Ordering::Relaxed),
                    now.saturating_sub(LOG_LAST.load(Ordering::Relaxed)) as f64 / 1e3,
                    crate::gpu::STARTED.load(Ordering::Relaxed),
                    crate::gpu::PRESENTS.load(Ordering::Relaxed),
                    crate::gpu::SUSPENDED.load(Ordering::Relaxed),
                    crate::input::ready(),
                    crate::pad::ACTIVE.load(Ordering::Relaxed),
                    crate::footprint_mb(),
                    heap,
                );
                let _ = std::fs::write(&path, text);
            }
        })
        .ok();
}

// ---- input, from GameActivity.java ----

enum Msg {
    Touch(u64, TouchPhase, f64, f64),
    Key { code: i32, down: bool, ch: u32, repeat: bool },
}

static QUEUE: Mutex<Vec<Msg>> = Mutex::new(Vec::new());
static PROXY: Mutex<Option<EventLoopProxy<()>>> = Mutex::new(None);
static PAD: Mutex<Option<Gamepad>> = Mutex::new(None);

pub fn set_proxy(proxy: EventLoopProxy<()>) {
    *PROXY.lock().unwrap() = Some(proxy);
}

fn wake() {
    if let Some(p) = PROXY.lock().unwrap().as_ref() {
        let _ = p.send_event(());
    }
}

fn push(m: Msg) {
    QUEUE.lock().unwrap().push(m);
    wake();
}

/// Delivers the input Java sent since the last call, on the event loop thread.
pub fn drain_input() {
    static FIRST: Once = Once::new();
    let msgs = std::mem::take(&mut *QUEUE.lock().unwrap());
    for m in msgs {
        match m {
            Msg::Touch(id, phase, x, y) => {
                FIRST.call_once(|| eprintln!("[input] first touch at {x:.0},{y:.0} px, game input {}", if crate::input::ready() { "ready" } else { "not ready yet" }));
                crate::touch::touch_at(id, phase, x, y)
            }
            Msg::Key { code, down, ch, repeat } => {
                let Some(name) = dom_code(code) else { continue };
                if down && !repeat {
                    crate::mp::key_pressed(name);
                }
                let text = char::from_u32(ch).filter(|c| *c != '\0' && !c.is_control()).map(String::from);
                crate::input::key(name, down, repeat, text.as_deref(), false);
            }
        }
    }
}

pub fn gamepad() -> Option<Gamepad> {
    *PAD.lock().unwrap()
}

/// android.view.KeyEvent key codes, as DOM codes.
fn dom_code(code: i32) -> Option<&'static str> {
    const LETTERS: [&str; 26] = ["KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM",
        "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ"];
    const DIGITS: [&str; 10] = ["Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9"];
    const NUMPAD: [&str; 10] = ["Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9"];
    const FKEYS: [&str; 12] = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
    Some(match code {
        29..=54 => LETTERS[(code - 29) as usize],
        7..=16 => DIGITS[(code - 7) as usize],
        144..=153 => NUMPAD[(code - 144) as usize],
        131..=142 => FKEYS[(code - 131) as usize],
        19 => "ArrowUp", 20 => "ArrowDown", 21 => "ArrowLeft", 22 => "ArrowRight",
        61 => "Tab", 62 => "Space", 66 => "Enter", 67 => "Backspace", 111 => "Escape", 112 => "Delete",
        59 => "ShiftLeft", 60 => "ShiftRight", 113 => "ControlLeft", 114 => "ControlRight",
        57 => "AltLeft", 58 => "AltRight", 117 => "MetaLeft", 118 => "MetaRight",
        55 => "Comma", 56 => "Period", 68 => "Backquote", 69 => "Minus", 70 => "Equal",
        71 => "BracketLeft", 72 => "BracketRight", 73 => "Backslash", 74 => "Semicolon", 75 => "Quote", 76 => "Slash",
        92 => "PageUp", 93 => "PageDown", 122 => "Home", 123 => "End", 124 => "Insert", 115 => "CapsLock",
        154 => "NumpadDivide", 155 => "NumpadMultiply", 156 => "NumpadSubtract", 157 => "NumpadAdd",
        158 => "NumpadDecimal", 160 => "NumpadEnter",
        _ => return None,
    })
}

// ---- performance hints ----
//
// Android lowers CPU clocks to whatever looks like enough. An ADPF hint session (Android 13+)
// tells it how long a frame may take and how long it took, so it boosts the engine's threads
// instead of letting a heavy scene run slow. Older Android: no-op.

static FRAME_TARGET_NS: AtomicI64 = AtomicI64::new(16_666_667);
static HINT_TIDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

/// A thread whose speed sets the frame rate (gpu, engine render and main threads).
pub fn perf_thread(tid: i32) {
    HINT_TIDS.lock().unwrap().push(tid);
}

struct Hints {
    session: *mut c_void,
    report: unsafe extern "C" fn(*mut c_void, i64) -> i32,
}
unsafe impl Send for Hints {}

fn hints() -> Option<&'static Mutex<Hints>> {
    static HINTS: OnceLock<Option<Mutex<Hints>>> = OnceLock::new();
    HINTS
        .get_or_init(|| unsafe {
            extern "C" {
                fn dlopen(name: *const c_char, flags: i32) -> *mut c_void;
                fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
            }
            let lib = dlopen(c"libandroid.so".as_ptr(), 2);
            if lib.is_null() { return None; }
            let sym = |n: &CStr| dlsym(lib, n.as_ptr());
            let (manager, create, report) = (sym(c"APerformanceHint_getManager"), sym(c"APerformanceHint_createSession"), sym(c"APerformanceHint_reportActualWorkDuration"));
            if manager.is_null() || create.is_null() || report.is_null() {
                eprintln!("[perf] no performance hint API on this Android");
                return None;
            }
            let manager: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(manager);
            let create: unsafe extern "C" fn(*mut c_void, *const i32, usize, i64) -> *mut c_void = std::mem::transmute(create);
            let m = manager();
            let tids = HINT_TIDS.lock().unwrap().clone();
            if m.is_null() || tids.is_empty() { return None; }
            let session = create(m, tids.as_ptr(), tids.len(), FRAME_TARGET_NS.load(Ordering::Relaxed));
            if session.is_null() {
                eprintln!("[perf] no performance hint session");
                return None;
            }
            eprintln!("[perf] performance hint session for {} threads, {:.1} ms frames", tids.len(), FRAME_TARGET_NS.load(Ordering::Relaxed) as f64 / 1e6);
            Some(Mutex::new(Hints { session, report: std::mem::transmute(report) }))
        })
        .as_ref()
}

/// Called by the gpu thread after each present.
pub fn frame_presented() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    static FRAMES: AtomicU64 = AtomicU64::new(0);
    let now = Instant::now();
    let Some(last) = LAST.lock().unwrap().replace(now) else { return };
    // let the engine's threads start before opening the session (it can't add threads later)
    if FRAMES.fetch_add(1, Ordering::Relaxed) < 120 { return; }
    let Some(h) = hints() else { return };
    let took = (now - last).min(Duration::from_millis(250)).as_nanos() as i64;
    let h = h.lock().unwrap();
    unsafe { (h.report)(h.session, took.max(1)) };
}

// ---- JNI, from Native.java ----

unsafe fn from_java(env: *mut JNIEnv, s: jstring) -> String {
    if s.is_null() { return String::new(); }
    let chars = ((**env).GetStringUTFChars.unwrap())(env, s, std::ptr::null_mut());
    if chars.is_null() { return String::new(); }
    let out = CStr::from_ptr(chars).to_string_lossy().into_owned();
    ((**env).ReleaseStringUTFChars.unwrap())(env, s, chars);
    out
}

unsafe fn to_java(env: *mut JNIEnv, s: &str) -> jstring {
    let c = CString::new(s.replace('\0', "")).unwrap_or_default();
    ((**env).NewStringUTF.unwrap())(env, c.as_ptr())
}

fn jbool(b: bool) -> jboolean {
    if b { JNI_TRUE } else { JNI_FALSE }
}

#[no_mangle]
pub unsafe extern "system" fn Java_ch_cclerc_muguet_Native_init(env: *mut JNIEnv, _: jclass, support: jstring, internal: jstring) {
    let (support, internal) = (from_java(env, support), from_java(env, internal));
    set_paths(Some(PathBuf::from(support)), Some(PathBuf::from(internal)));
}

#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_gameReady(_: *mut JNIEnv, _: jclass) -> jboolean {
    jbool(crate::config::game_ready())
}

static IMPORT_DONE: AtomicU64 = AtomicU64::new(0);
static IMPORT_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Imports the game zip open at `fd` (from the system file picker). Null, or what went wrong.
#[no_mangle]
pub unsafe extern "system" fn Java_ch_cclerc_muguet_Native_importZip(env: *mut JNIEnv, _: jclass, fd: jint) -> jstring {
    IMPORT_DONE.store(0, Ordering::Relaxed);
    IMPORT_TOTAL.store(0, Ordering::Relaxed);
    let keep_code = !crate::code_embedded();
    let result = zip_file(fd).and_then(|zip| {
        crate::import::import_file(zip, &crate::config::game_dir(), keep_code, |done, total| {
            IMPORT_DONE.store(done, Ordering::Relaxed);
            IMPORT_TOTAL.store(total, Ordering::Relaxed);
        })
    });
    match result {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => {
            eprintln!("[import] {e:#}");
            to_java(env, &format!("{e:#}"))
        }
    }
}

/// Our own handle on the picked file. Not reopened through /proc/self/fd: Android refuses that
/// for files from the system picker. Java closes its descriptor when the import returns.
fn zip_file(fd: jint) -> anyhow::Result<std::fs::File> {
    use std::io::{Seek, SeekFrom};
    use std::os::fd::FromRawFd;
    extern "C" {
        fn dup(fd: i32) -> i32;
    }
    let own = unsafe { dup(fd) };
    if own < 0 {
        anyhow::bail!("could not read the picked file: {}", std::io::Error::last_os_error());
    }
    let mut file = unsafe { std::fs::File::from_raw_fd(own) };
    // a zip is read from its end: a file streamed by a cloud app can't be
    if file.seek(SeekFrom::End(0)).and_then(|_| file.seek(SeekFrom::Start(0))).is_err() {
        anyhow::bail!("this file can only be streamed, not read in place: copy it to the device's storage and pick that copy");
    }
    Ok(file)
}

#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_importDone(_: *mut JNIEnv, _: jclass) -> jlong {
    IMPORT_DONE.load(Ordering::Relaxed) as jlong
}

#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_importTotal(_: *mut JNIEnv, _: jclass) -> jlong {
    IMPORT_TOTAL.load(Ordering::Relaxed) as jlong
}

#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_codeReady(_: *mut JNIEnv, _: jclass) -> jboolean {
    let ready = crate::config::setup().ok().and_then(|s| crate::code_cache(&s).ok()).is_some_and(|p| p.exists());
    crate::config::clear_settings();
    jbool(ready)
}

/// Compiles the game's code to native code once, so launches don't. Null, or what went wrong.
#[no_mangle]
pub unsafe extern "system" fn Java_ch_cclerc_muguet_Native_precompile(env: *mut JNIEnv, _: jclass) -> jstring {
    let result = crate::config::setup().and_then(|s| crate::precompile(&s));
    crate::config::clear_settings();
    match result {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => {
            eprintln!("[host] {e:#}");
            to_java(env, &format!("{e:#}"))
        }
    }
}

/// phase: 0 down, 1 move, 2 up, 3 cancel; x, y in window pixels.
#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_touch(_: *mut JNIEnv, _: jclass, id: jint, phase: jint, x: jfloat, y: jfloat) {
    let phase = match phase {
        0 => TouchPhase::Started,
        1 => TouchPhase::Moved,
        2 => TouchPhase::Ended,
        _ => TouchPhase::Cancelled,
    };
    push(Msg::Touch(id as u64, phase, x as f64, y as f64));
}

#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_key(_: *mut JNIEnv, _: jclass, code: jint, down: jboolean, ch: jint, repeat: jboolean) {
    push(Msg::Key { code, down: down != JNI_FALSE, ch: ch as u32, repeat: repeat != JNI_FALSE });
}

/// Controller state: buttons as pad::btn bits, axes as Android reports them (y down).
#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_pad(_: *mut JNIEnv, _: jclass, buttons: jint, lx: jfloat, ly: jfloat, rx: jfloat, ry: jfloat, lt: jfloat, rt: jfloat) {
    let gp = Gamepad { lx, ly: -ly, rx, ry: -ry, lt, rt, buttons: buttons as u32 & (btn::OPTIONS * 2 - 1) };
    let was = PAD.lock().unwrap().replace(gp);
    if was.is_none() { wake(); }
}

#[no_mangle]
pub extern "system" fn Java_ch_cclerc_muguet_Native_padGone(_: *mut JNIEnv, _: jclass) {
    *PAD.lock().unwrap() = None;
    wake();
}
