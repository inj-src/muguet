// muguet - lib.rs
// Constantin Clerc (c22dev) - 0.1

// the Android app is this crate as a shared library, entered through android_main (android.rs)
#![cfg(target_os = "android")]

include!("main.rs");
