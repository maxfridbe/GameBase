# GameBase

A minimal Bevy (Rust) starter game that already ships to **Linux**, **Windows**, **Android (APK)**, and the **browser (WASM)** from a single Linux dev machine. The build methodology was extracted from the Julian2 train game project.

**[▶ Play the demo in your browser](https://maxfridbe.github.io/GameBase/)** — deployed automatically to GitHub Pages by the release workflow.

The base game is a sphere you drive around a 3D plane:

- **WASD / arrow keys** — move on the ground (relative to camera)
- **Space / Ctrl** — fly up / down
- **Mouse drag** — orbit the camera (one-finger drag on Android)
- **Scroll wheel** — zoom

The pillars and arena walls are solid: bumping into them nudges the sphere out and plays a knock sound (`assets/bounce.wav`, pitch-varied per hit) — a tiny demo of collision response plus the audio pipeline working on every platform.

---

## How the multi-platform methodology works

One Rust crate produces every platform:

```
src/lib.rs   -> ALL game code lives here (run_game()).
                crate-type = ["staticlib", "cdylib", "rlib"]
                #[bevy_main] fn main() is the Android entry point.
src/main.rs  -> tiny desktop wrapper: fn main() { game_base::run_game() }
```

| Platform | Mechanism | Entry artifact |
|----------|-----------|----------------|
| Linux | native `cargo run` (x11 feature) | `target/release/game_base` |
| Windows | cross-compile from Linux with MinGW (`x86_64-pc-windows-gnu`) | `target/windows_dist/` (exe + assets, zip and ship) |
| Android A (**primary**) | `cargo apk` builds the **cdylib** into an APK per ABI; NativeActivity, no Java code at all | `target/{debug,release}/apk/*.apk` |
| Android B (alternative) | `cargo ndk` drops the cdylib into `app/src/main/jniLibs/`, then Gradle wraps it with a Java `GameActivity` into one universal APK | `app/build/outputs/apk/debug/app-debug.apk` |
| Browser (WASM) | compile the bin to `wasm32-unknown-unknown` (webgl2 feature), `wasm-bindgen` generates the JS glue, `web/index.html` hosts the canvas | `target/web_dist/` (static site — serve anywhere) |

Key load-bearing details (easy to lose, hard to rediscover):

- **`bevy_embedded_assets`** embeds `assets/` into the binary. On Android there is no loose filesystem for Bevy's `AssetServer`, so this is what makes assets work in the APK. It must be added **before** `DefaultPlugins`.
- **`cpal` with `oboe-shared-stdcxx`** makes audio work on Android; it links against `libc++_shared.so`, which the Gradle path copies out of the NDK explicitly (Step 2 of `buildanddeploy.sh`).
- Bevy is built with `default-features = false`; the `android-native-activity` feature is what Path A needs, `x11` is what native Linux needs. If you use **Path B (Gradle/GameActivity)**, switch the bevy feature `android-native-activity` → `android-game-activity` so it matches the Java `GameActivity` wrapper (Path A was the proven/primary path in the source project).
- Two APKs are built on purpose in Path A: **x86_64** for the desktop emulator, **arm64-v8a** for real phones. Path B builds one fat APK containing both.
- Emulator GPU emulation matters for wgpu/Vulkan: `swangle_indirect` (run_emulator.sh) is the most stable; `start_new_emulator.sh` is the SwiftShader/CPU fallback for hosts whose GPU driver crashes the emulator.
- `game.env` centralizes the game identity + Android SDK paths; every script sources it.
- Browser build details: `bevy_embedded_assets` means the `.wasm` is fully self-contained (no asset fetch issues on static hosts); `wasm-bindgen-cli` must exactly match the `wasm-bindgen` crate version in `Cargo.lock` (`build_web.sh` auto-installs the right one); `getrandom` 0.3 (pulled via ahash/bevy) needs the `wasm_js` feature **and** `RUSTFLAGS=--cfg getrandom_backend="wasm_js"` — both are wired in already; the window is bound to the `#game-canvas` element in `web/index.html`.
- Browser build size: the web build uses the dedicated `[profile.wasm-release]` (opt-level `z`, fat LTO — native platforms keep the fast default release profile) and then `wasm-opt -Oz` from binaryen if installed. Expect roughly half the size of a plain release wasm.

## Scripts

| Script | What it does |
|--------|--------------|
| `setup_env.sh --check` | Verify every requirement of every build script (no sudo, no installs) — prints `[ OK ]`/`[MISS]` per item |
| `setup_env.sh` | Check, then install only what's missing: system packages (apt or dnf detected automatically), Rust + cross targets, MinGW-w64, cargo-apk/cargo-ndk, Android SDK/NDK 26, debug keystore |
| `run_linux.sh [debug]` | Build + run natively on Linux |
| `build_windows.sh` | Cross-compile Windows release, package exe + assets into `target/windows_dist/` |
| `build_web.sh` | Build the browser version into `target/web_dist/` (compile to wasm, run wasm-bindgen, add `web/index.html`) |
| `build_cargo_apk.sh` / `_debug.sh` | Path A: build emulator (x86_64) + phone (ARM64) APKs |
| `deploy_cargo_apk.sh` | Install Path A APK to running emulator, launch, follow logcat |
| `deploy_phone.sh` | Install Path A ARM64 APK to USB phone, launch, follow logcat |
| `run_emulator.sh` | Start a clean emulator (Swangle GPU — most stable for Bevy) |
| `start_new_emulator.sh` | Fallback emulator (SwiftShader CPU rendering) |
| `buildanddeploy.sh` | Path B: cargo-ndk → jniLibs → Gradle universal APK → install + launch + logs |
| `deploy_apk_to_emulator.sh` | Re-install Path B APK without rebuilding |
| `read_logs.sh` / `read_logs_phone.sh` | Dump last 50 relevant log lines (emulator / phone) |
| `kill_phone_app.sh` | Force-stop the game on the phone |
| `increment_version.sh [major\|minor\|patch]` | Bump `version.txt` (default: patch) and sync the version into `Cargo.toml` and `app/build.gradle` |

## Versioning + CI releases

`version.txt` at the repo root is the single source of truth for the game version.

1. `./increment_version.sh` (or `minor` / `major`) bumps it and syncs `Cargo.toml` + Gradle `versionName`/`versionCode`.
2. Commit and push to `main`.
3. `.github/workflows/release.yml` builds all three platforms on GitHub Actions and publishes a **GitHub Release tagged `v<version>`** with:
   - `<game>-linux-x86_64-v<version>.tar.gz` (binary + assets)
   - `<game>-windows-x86_64-v<version>.zip` (exe + assets, MinGW cross-compiled)
   - `<game>-android-arm64-v<version>.apk` (phones) and `<game>-android-x86_64-v<version>.apk` (emulator)
   - `<game>-web-v<version>.zip` (static site) — the same build is also deployed to **GitHub Pages** as the live demo (first run: if the `pages` job fails, enable Pages once under repo Settings → Pages → Source: GitHub Actions)

Pushing again without bumping the version updates the existing release for that tag rather than creating a new one. CI signs APKs with a freshly generated debug keystore — replace that step with a real keystore (repo secret) before shipping to a store.

## Quick start

```bash
./setup_env.sh --check  # see what your machine is missing
./setup_env.sh          # once per machine; installs only the missing pieces (apt or dnf)
./run_linux.sh          # play on Linux
./build_windows.sh      # produce target/windows_dist/ for Windows
./build_web.sh          # produce target/web_dist/ for the browser
python3 -m http.server -d target/web_dist 8080   # ...then play at localhost:8080
./run_emulator.sh       # boot the Android emulator...
./build_cargo_apk_debug.sh && ./deploy_cargo_apk.sh   # ...and play in it
./build_cargo_apk.sh && ./deploy_phone.sh             # play on a USB phone
```

---

## Making it yours (renaming checklist)

The base identity is `game_base` / "Game Base". To turn this into *your* game, change these — they must stay consistent with each other:

**1. `Cargo.toml`** — the source of truth:

```toml
[package]  name = "my_game"            # crate name
[lib]      name = "my_game"            # native library name -> libmy_game.so
[[bin]]    name = "my_game"            # exe name -> my_game / my_game.exe

[package.metadata.android]
package = "com.yourstudio.mygame"      # Android application id (Path A)
label = "My Game"                      # app name shown under the icon
```

Also update `src/main.rs` (`my_game::run_game();`) and the window title in `src/lib.rs`.

> The `[package.metadata.android.signing.release]` block points at a **debug keystore** — fine for sideloading; generate a real keystore (`keytool -genkey ...`) before any store release.

**2. `game.env`** — the scripts read the same names from here:

```bash
GAME_NAME="my_game"                    # = Cargo.toml lib/bin name
ANDROID_PACKAGE="com.yourstudio.mygame"   # = [package.metadata.android] package
GRADLE_PACKAGE="org.yourstudio.my_game"   # = app/build.gradle applicationId
```

**3. App icon** — replace `assets/android-res/mipmap-mdpi/ic_launcher.png` (used by both Android paths).

**3b. Demo link** — the browser demo deploys to `https://<your-user>.github.io/<your-repo>/`; update the link at the top of this README. (`web/index.html` needs no changes — `build_web.sh` fills in the game name.)

**4. Only if you use Path B (Gradle):**

- `app/build.gradle` — `namespace` and `applicationId`
- `app/src/main/AndroidManifest.xml` — `android:label` (app name) and the `android.app.lib_name` meta-data value (= `GAME_NAME`)
- `app/src/main/java/.../MainActivity.java` — the `package` line and `System.loadLibrary("my_game")`; move the file to a directory matching the new package (`app/src/main/java/com/yourstudio/mygame/`)
- `settings.gradle` — `rootProject.name`
- `Cargo.toml` — swap bevy feature `android-native-activity` → `android-game-activity`

**One-shot rename** (Path A pieces, from the repo root — review the diff after):

```bash
NEW=my_game; NEWPKG=com.yourstudio.mygame; NEWLABEL="My Game"
sed -i "s/game_base/$NEW/g" Cargo.toml src/main.rs game.env
sed -i "s/com\.gamebase\.game/$NEWPKG/g" Cargo.toml game.env
sed -i "s/Game Base/$NEWLABEL/g" Cargo.toml src/lib.rs
```

**5. Grow the game** — everything lives in `src/lib.rs`. The pattern that scales (used by the parent project): split features into modules, each exposing a Bevy `Plugin`, and `add_plugins(...)` them in `run_game()`. Put new assets in `assets/` — they are embedded automatically on every platform.

## Version pins that matter

| Thing | Version | Why pinned |
|-------|---------|-----------|
| bevy | 0.15.x | input/render APIs used here |
| bevy_embedded_assets | 0.12 | matches bevy 0.15 |
| Android NDK | 26.1.10909125 | referenced by every script + setup |
| AGP / Gradle | 8.4.0 / 8.6 | Path B only |
| games-activity | 4.4.0 | must stay compatible with bevy's `android-activity` crate |
| wasm-bindgen-cli | = `wasm-bindgen` in Cargo.lock | hard requirement; `build_web.sh`/CI resolve it automatically |
| minSdk / target/compileSdk | 30 / 33 / 34 | Path B only; cargo-apk defaults handle Path A |
