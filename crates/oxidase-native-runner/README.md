# `oxidase-native-runner`

Internal, un-published (`publish = false`) test harness and dogfooding application for [`oxidase`](../oxidase).

---

## 1. Overview

`oxidase-native-runner` proves real hosted-native frame driving end-to-end:
- **Core Framework**: Dioxus 0.7.10 VirtualDOM
- **Render Engine**: Blitz 0.3.0-beta.2 (Vello GPU Metal/Vulkan backend)
- **Window Host**: Winit 0.31 via in-repo `dioxus-native`
- **Frame Driver**: `oxidase::frame::set_host_redraw_requester` + `oxidase::frame::step_hosted_frame` on `WindowEvent::RedrawRequested`

It isolates local monorepo path dependencies ([`blitz/packages/dioxus-native`](../../../../blitz/packages/dioxus-native)) from the published `crates/oxidase` crate, keeping `crates/oxidase` 100% clean for crates.io publication.

---

## 2. Execution Modes

### A. Interactive GUI Window Mode (Manual Inspection)

Opens a real desktop window with live metrics, animated VSync progress indicator, and interactive event buttons:

```bash
# Debug profile (~20 FPS)
cargo run --manifest-path util/oxidase/crates/oxidase-native-runner/Cargo.toml -- --interactive

# Release profile (60-120 FPS native VSync)
cargo run --manifest-path util/oxidase/crates/oxidase-native-runner/Cargo.toml --release -- --interactive
```

*(Alternatively, set `OXIDASE_INTERACTIVE=1`)*

### B. Auto-Close Proof Mode (CI / Worker Automation)

Opens the native window, drives 20 real frames through `step_hosted_frame()`, validates all 5 proof criteria, and exits automatically with code 0:

```bash
cargo run --manifest-path util/oxidase/crates/oxidase-native-runner/Cargo.toml
```

---

## 3. Performance Characteristics: Debug vs. Release

When testing native graphics, frame rates vary significantly depending on the Rust compilation profile:

| Profile | Typical Frame Rate | Frame Delta (`dt`) | Description |
|---|---|---|---|
| **Debug (`dev`)** | **~18–20 FPS** | **~50–55 ms** | Rust compiler optimizations (`-O0`) disabled. Servo's `Stylo` CSS engine, `Taffy` layout, and `Vello` GPU compute shader encoding take ~35-40ms per frame. Due to macOS VSync quantization, the frame presentation falls back to every 3rd VSync tick (`16.67ms × 3 ≈ 50ms`). |
| **Release (`--release`)** | **~60–120 FPS** | **~8–16 ms** | Fully optimized (`-O3`). Frame computation takes 2–5ms, enabling the engine to match full display VSync frequency smoothly. |

### Diagnostic Indicators in the Runner

- **Console Banner**: Displays active profile and recommended flags at launch.
- **Window Badges**: Shows `Debug (~20 FPS)` or `Release (60+ FPS)`.
- **Live Frame Rate**: Computes instant FPS and running average FPS in real time.
- **Profile Note**: Advises developers directly within the GUI if running under an unoptimized debug build.

---

## 4. Architectural Separation

```text
util/oxidase/
├── Cargo.toml                  # members = ["crates/oxidase", "crates/oxidase-macro"]
│                               # exclude = ["crates/oxidase-native-runner"]
├── crates/oxidase/             # [PUBLISHABLE] Pure crates.io FFI engine (no local path deps)
└── crates/oxidase-native-runner/ # [INTERNAL] publish = false test harness (links local dioxus-native)
```
