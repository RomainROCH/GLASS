# Input System

← [Architecture Overview](ARCHITECTURE.md)

The GLASS overlay supports two input modes: **Passive** (fully click-through)
and **Interactive** (accepts mouse input on designated regions). This document
covers mode switching, hit-testing, the hotkey system, visual indicators, and
the system tray.

**Source files:** `input.rs`, `overlay_window.rs`, `config.rs` (`InputConfig`)

---

## Table of Contents

- [Two Input Modes](#two-input-modes)
- [Mode Transition Flow](#mode-transition-flow)
- [OverlayInputState](#overlayinputstate)
- [HitTester](#hittester)
- [InputManager — High-Level API](#inputmanager--high-level-api)
- [Custom Messages and Constants](#custom-messages-and-constants)
- [InputConfig — Configuration](#inputconfig--configuration)
- [Window Styles](#window-styles)
- [System Tray](#system-tray)
- [Module Update Timer](#module-update-timer)

---

## Two Input Modes

Defined in `input.rs`:

```rust
pub enum InputMode {
    Passive,     // Fully click-through — no mouse events reach the overlay.
    Interactive, // Designated rects accept mouse input until timeout.
}
```

### Passive (Mode A) — Default

The overlay window has `WS_EX_TRANSPARENT` set, making it completely
click-through. All mouse events pass to the application below. This is the
normal operating state — the overlay is a read-only HUD.

### Interactive (Mode B) — Hotkey-Triggered

Triggered by a global hotkey (default: F12). The overlay removes
`WS_EX_TRANSPARENT` so mouse events reach the window, then uses `HitTester` to
determine which interactive rectangles the cursor is over. After a configurable
timeout (default: 4000ms), the overlay automatically reverts to passive mode.

---

## Mode Transition Flow

```
                    ┌──────────────────────────────────────────────┐
                    │              PASSIVE MODE                     │
                    │  WS_EX_TRANSPARENT set — fully click-through │
                    └──────────────────┬───────────────────────────┘
                                       │
                          User presses hotkey (F12)
                          WM_HOTKEY → enter_interactive()
                                       │
                                       ▼
                    ┌──────────────────────────────────────────────┐
                    │            INTERACTIVE MODE                   │
                    │  WS_EX_TRANSPARENT removed                   │
                    │  Visual indicator shown (border + label)     │
                    │  Timer started (default 4000ms)              │
                    └──────────────────┬───────────────────────────┘
                                       │
                      Timeout expires OR hotkey re-pressed
                      enter_passive() → restore WS_EX_TRANSPARENT
                      Indicator removed from scene
                                       │
                                       ▼
                    ┌──────────────────────────────────────────────┐
                    │              PASSIVE MODE                     │
                    └──────────────────────────────────────────────┘
```

### Step-by-Step

1. **User presses hotkey** (default F12) → Windows delivers `WM_HOTKEY` to the
   overlay's message loop.
2. **`enter_interactive()`** called on `OverlayInputState`:
   - Mode set to `Interactive`, `interactive_since` set to `Instant::now()`.
   - Returns `true` if mode actually changed (was passive), `false` if already
     interactive (timer reset only).
3. **Window style change**: `WS_EX_TRANSPARENT` removed via
   `SetWindowLongPtrW` → mouse events now reach the overlay window.
4. **Visual indicator**: `InputManager::show_indicator()` adds 4 border
   rectangles + an `"INTERACTIVE"` label to the scene graph.
5. **Timer started**: Win32 `SetTimer` with `INTERACTIVE_TIMER_ID` (42) and the
   configured timeout.
6. **On timeout** (`WM_TIMER`) **or hotkey re-press** → `enter_passive()`:
   - `WS_EX_TRANSPARENT` restored.
   - Indicator nodes removed from scene via `InputManager::hide_indicator()`.
   - Timer killed via `KillTimer`.

If the hotkey is pressed while already interactive, the timer is reset (the
timeout restarts from now) but the mode doesn't transition.

---

## OverlayInputState

Stored in the HWND's `GWLP_USERDATA`. Accessed exclusively from the `wnd_proc`
thread — single-threaded by the Win32 message-pump model.

```rust
pub struct OverlayInputState {
    /// Current input mode.
    pub mode: InputMode,
    /// Hit-tester for interactive regions.
    pub hit_tester: HitTester,
    /// Timeout duration for interactive mode.
    pub timeout: Duration,
    /// When interactive mode started (for diagnostics).
    pub interactive_since: Option<Instant>,
    /// Whether interactive mode is available (hotkey registered successfully).
    pub interactivity_available: bool,
    /// Application name used in tray UI labels.
    pub app_name: String,
}
```

### Key Methods

| Method | Description |
|---|---|
| `new(timeout_ms)` | Create in passive mode with app name `"GLASS"`. |
| `with_app_name(timeout_ms, app_name)` | Create in passive mode with custom app name. |
| `enter_interactive() → bool` | Transition to interactive. Returns `true` if mode changed. |
| `enter_passive() → bool` | Transition to passive. Returns `true` if mode changed. |
| `is_interactive() → bool` | Query current mode. |

**Ownership**: Allocated with `Box::into_raw` at window creation, stored as a
raw pointer in `GWLP_USERDATA`, and reclaimed with `Box::from_raw` in
`WM_DESTROY`. The raw pointer lifetime is bounded by the HWND lifetime.

---

## HitTester

Rectangle-based hit-testing with Z-order support. Interactive UI nodes register
their bounds here. During interactive mode, mouse coordinates are tested against
all registered rects.

```rust
pub struct HitTester {
    rects: Vec<InteractiveRect>,
    next_id: u32,
}
```

### InteractiveRect

```rust
pub struct InteractiveRect {
    pub id: u32,       // Unique identifier (auto-assigned)
    pub x: f32,        // Left edge in logical pixels
    pub y: f32,        // Top edge in logical pixels
    pub width: f32,    // Width in logical pixels
    pub height: f32,   // Height in logical pixels
    pub z_order: i32,  // Higher values are tested first
}

impl InteractiveRect {
    /// Half-open containment: left/top inclusive, right/bottom exclusive.
    pub fn contains(&self, px: f32, py: f32) -> bool;
}
```

### HitTester API

| Method | Description |
|---|---|
| `new()` | Create an empty hit-tester. |
| `add_rect(x, y, width, height, z_order) → u32` | Register a rect. Returns its ID. Re-sorts by z_order descending. |
| `remove_rect(id) → bool` | Remove a rect by ID. Returns `true` if found. |
| `hit_test(px, py) → Option<u32>` | Find the topmost rect containing the point. `None` if no hit. |
| `clear()` | Remove all rects. |
| `len()` / `is_empty()` | Count queries. |

### Hit-Testing Algorithm

1. Rects are sorted by `z_order` **descending** on insertion.
2. `hit_test` scans linearly; the first rect that `contains(px, py)` wins.
3. Higher `z_order` = tested first = "on top".
4. Same `z_order`: stable sort preserves insertion order (first added wins).

### Performance

- **Zero-allocation steady-state**: The rect list is pre-allocated. Hit-testing
  performs no heap allocations.
- **Linear scan**: O(n) where n is the number of registered rects. In practice,
  n ≤ 10, making this effectively O(1).

---

## InputManager — High-Level API

Manages the visual indicator lifecycle (border rectangles + label). Used by the
main application loop, not the `wnd_proc` directly.

```rust
pub struct InputManager {
    indicator_node_ids: Vec<NodeId>,
    indicator_visible: bool,
}
```

| Method | Description |
|---|---|
| `new()` | Create a new InputManager. |
| `show_indicator(scene, width, height) → bool` | Adds 4 border rects + `"INTERACTIVE"` label. Returns `true` if added (wasn't already visible). |
| `hide_indicator(scene) → bool` | Removes indicator nodes. Returns `true` if removed (was visible). |
| `indicator_visible() → bool` | Query visibility state. |

### Visual Indicator

When interactive mode is active, the `InputManager` renders:

- **4 border rectangles**: top, bottom, left, right edges of the overlay window.
  Color: `rgba(0.2, 0.8, 1.0, 0.6)`, thickness: 3px.
- **1 text label**: `"INTERACTIVE"` in the top-right corner. Color:
  `rgba(0.2, 0.8, 1.0, 0.9)`, font size: 14px.

All 5 nodes are tracked as `NodeId`s and removed atomically when reverting to
passive mode.

---

## Custom Messages and Constants

Defined in `input.rs` and `overlay_window.rs`:

### Input Messages

| Constant | Value | Description |
|---|---|---|
| `WM_GLASS_MODE_INTERACTIVE` | `WM_APP + 10` (`0x800A`) | Posted on transition to interactive mode |
| `WM_GLASS_MODE_PASSIVE` | `WM_APP + 11` (`0x800B`) | Posted on transition to passive mode |

### Timer IDs

| Constant | Value | Description |
|---|---|---|
| `INTERACTIVE_TIMER_ID` | `42` | Win32 timer for interactive-mode timeout |
| `MODULE_UPDATE_TIMER_ID` | `43` | Win32 timer for periodic module updates |

### Hotkey

| Constant | Value | Description |
|---|---|---|
| `HOTKEY_ID` | `1` | Win32 hotkey registration ID |

### Tray

| Constant | Value | Description |
|---|---|---|
| `WM_TRAYICON` | `WM_APP + 1` (`0x8001`) | Custom message for tray icon events |
| `IDM_EXIT` | `1001` | Context menu item ID for "Quit" |
| `TRAY_ICON_ID` | `1` | Shell notification icon ID |

---

## InputConfig — Configuration

Defined in `config.rs`. Serialized in the `input` section of the RON/TOML
config file.

```rust
pub struct InputConfig {
    /// Virtual key code for the toggle hotkey.
    /// Default: 0x7B (VK_F12).
    /// Common alternatives: 0x79 (F10), 0x7A (F11).
    pub hotkey_vk: u32,

    /// Hotkey modifier flags (Win32 MOD_* bitmask).
    /// Default: 0 (no modifier).
    /// 1 = Alt, 2 = Ctrl, 4 = Shift, 8 = Win.
    pub hotkey_modifiers: u32,

    /// Interactive mode timeout in milliseconds.
    /// After this duration, the overlay reverts to passive mode.
    /// Default: 4000.
    pub interactive_timeout_ms: u32,

    /// Whether to show the visual indicator (border + label) in interactive mode.
    /// Default: true.
    pub show_indicator: bool,
}
```

### Modifier Examples

| `hotkey_modifiers` | Meaning |
|---|---|
| `0` | No modifier — hotkey alone |
| `1` | Alt + hotkey |
| `2` | Ctrl + hotkey |
| `6` | Ctrl + Shift + hotkey (2 + 4) |
| `8` | Win + hotkey |

---

## Window Styles

The overlay window uses a specific combination of extended window styles.
These are set at window creation in `overlay_window.rs` and selectively toggled
during mode transitions.

| Style | Purpose | Toggled? |
|---|---|---|
| `WS_EX_LAYERED` | Enables layered window (required for DComp transparency) | No — always set |
| `WS_EX_TRANSPARENT` | Click-through — all mouse events pass through | **Yes** — removed in interactive mode, restored in passive |
| `WS_EX_NOREDIRECTIONBITMAP` | Suppresses GDI surface (DComp provides visuals) | No — always set |
| `WS_EX_TOPMOST` | Always on top of other windows | No — always set |
| `WS_EX_TOOLWINDOW` | No Alt-Tab entry, no taskbar button | No — always set |

### Mode Toggle Implementation

```
Enter interactive:
  GetWindowLongPtrW(hwnd, GWL_EXSTYLE)
  → Remove WS_EX_TRANSPARENT bit
  → SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style)

Enter passive:
  GetWindowLongPtrW(hwnd, GWL_EXSTYLE)
  → Add WS_EX_TRANSPARENT bit
  → SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style)
```

---

## System Tray

A system tray icon is added at window creation, providing a minimal UI for
overlay management.

### Behavior

- **Icon**: Registered via `Shell_NotifyIconW` with `NIM_ADD`.
- **Tooltip**: Shows `app_name` from `OverlayInputState` (default: `"GLASS"`).
- **Right-click**: Opens a context menu with a single "Quit" item (`IDM_EXIT = 1001`).
- **Quit handling**: Posts `WM_CLOSE` → `DestroyWindow` → `PostQuitMessage(0)`.

### Message Flow

```
User right-clicks tray icon
  → Windows sends WM_TRAYICON (WM_APP + 1) to wnd_proc
  → lparam == WM_RBUTTONUP → show_tray_menu()
  → User clicks "Quit" → WM_COMMAND with IDM_EXIT
  → PostMessageW(hwnd, WM_CLOSE, ...)
```

---

## Module Update Timer

The module system is ticked from the Win32 message loop using a periodic timer.

| Constant | Value | Source |
|---|---|---|
| `MODULE_UPDATE_TIMER_ID` | `43` | `renderer.rs` |

The timer fires periodically, causing a `WM_TIMER` message in the message loop.
The handler calls `LayoutManager::update_all(scene, dt)` (or
`ModuleRegistry::update_all` in the low-level path) and triggers a re-render
if any module reports the scene as dirty.

This timer is separate from `INTERACTIVE_TIMER_ID` (42), which handles the
interactive-mode timeout.

---

## Threading Model

All input state is single-threaded:

- `OverlayInputState` lives in `GWLP_USERDATA`, accessed only from `wnd_proc`.
- `HitTester` is not `Sync` — no locking, no interior mutability.
- `InputManager` is owned by the application loop, called from the same thread.
- Mode transitions, timer setup, and style changes all happen on the
  message-loop thread.

This is enforced by the Win32 message-pump model: all window messages for a
given HWND are dispatched on the thread that created it.

---

## Companion Documents

| Document | Covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | High-level architecture, workspace crates, layer overview |
| [module-system.md](module-system.md) | `OverlayModule` trait, registry, layout, callback injection |
| [scene-graph.md](scene-graph.md) | Retained scene graph, node types, dirty tracking |
| [config-system.md](config-system.md) | RON/TOML loading, hot-reload, ConfigStore |
| [decisions.md](decisions.md) | Full ADR log |
