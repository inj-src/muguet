# Muguet
A native iOS/macOS/Android port of GTA V, with multiplayer

## How does this work ?
this whole port relies on the WebAssembly compilation of the game. in a browser, that `game.wasm` runs inside the JS engine, behind WebGPU, Web Audio, web workers...
Muguet throws that layer away and runs the same `game.wasm` as a regular native app, with [wasmtime](https://wasmtime.dev) for the code and [wgpu](https://wgpu.rs) on Metal (Vulkan on Android) for the graphics

On macOS and Android, the game code is compiled to native arm64 the first time you launch it, then cached. On iOS, we can't properly do that (i dont rlly want you to deal with the hassle of enabling JIT), so the game code is compiled when you build the app and baked into it

On top of that, Muguet adds a small UDP multiplayer (for other people who have the same port to play with you)

## Game files
Because I care about me not getting sued, you will need your own copy as a `.zip` : the folder holding `data/` and `b/<build>/` (with `game.wasm` and `shaders/`)

It's basically the same source as the web port.

Muguet itself will then asks for that zip

## Requirements
- Apple Silicon Mac, macOS 14 or later
- Rust toolchain
- Xcode 26 or later

For Android, instead :
- Rust toolchain
- Android Studio (or the Android SDK command-line tools) with an NDK installed, and `ANDROID_HOME` set
- a 64-bit ARM device on Android 10 or later with Vulkan 1.1, ideally 8 GB of RAM or more

For iOS, also :
- `rustup` with `rustup toolchain install 1.98.1` and `rustup target add aarch64-apple-ios --toolchain 1.98.1`
- `xcodegen` (`brew install xcodegen`)
- an Apple developer account
- a device on iOS 15 or later, ideally 8 GB of RAM

## Building
macOS :
```
./make-app.sh
```
gives `build/Muguet.app`

iOS :
```
TEAM=<your team id> ./make-ios.sh game.zip --install
```
`--ipa` makes `build/Muguet.ipa` instead. With a free account, add `FREE=1` (+ a `BUNDLE_ID` of your own)

Android :
```
./make-android.sh
```
gives `build/Muguet.apk`; `--install` also installs it with `adb`. Then pick your game `.zip` in the app: it is imported, and the game's code is compiled for your phone once (a few minutes).

Please install the iOS app with AltStore. This probably won't work well with Sideloadly (or even SideStore, as per [this issue])(https://github.com/SideStore/SideStore/issues/1616)

The app also works with a paid developer account.

## Multiplayer
One player hosts from the launcher (or runs `muguet-server` on any machine), the others join with that address. UDP port 47520

## Android
- Settings, saves and the imported game live in `Android/data/ch.cclerc.muguet/files/` (with `config.json` and `muguet.log`), the compiled game code in the app's internal storage
- Touch controls are drawn by the game's renderer; a controller hides them, a keyboard works too. Back opens the pause menu
- GPUs whose Vulkan driver reads BC textures (most recent Snapdragon ones) use the game's textures as they are. Others (e.g. Mali) get them decoded on the CPU, on all cores: more memory and slower streaming there. The log says which (`decoding them on the CPU`)
- Android 13+ is told the frame time target (performance hints), so it keeps the CPU clocks up for the game's threads

## Limitations
- iOS needs the increased memory limit entitlement
- Minimap doesn't show
- Story missions are single player only


Contributions are welcome !
