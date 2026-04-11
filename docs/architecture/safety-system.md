# Safety System (Anti-Cheat Detection)

← [Architecture Overview](ARCHITECTURE.md)

---

## Design

The safety system is a passive, read-only anti-cheat detector. It scans for
known anti-cheat systems **before any window or GPU initialization** and gates
whether GLASS starts. The system is feature-gated behind the `gaming` Cargo
feature — default builds include no anti-cheat code.

**Source:** `glass-overlay/src/safety.rs`  
**Integration:** `glass-starter/src/main.rs`

---

## Feature Gate

The entire `safety.rs` module is only compiled when the `gaming` feature is
enabled. This is a deliberate architectural decision: a developer building a
productivity overlay (screen annotation, notes, status display) should never
see anti-cheat warnings or incur the cost of process scanning.

### Feature chain

```
glass-starter/Cargo.toml:
    gaming = ["glass-core/gaming", "glass-overlay/gaming"]

glass-overlay/Cargo.toml:
    gaming = []

glass-core/Cargo.toml:
    gaming = []
```

The feature propagates through the workspace. `glass-starter` enables it via
`cargo run --features gaming`. Default builds (`cargo run`) have no safety
system compiled in.

### Conditional compilation in consumer code

```rust
// glass-starter/src/main.rs
#[cfg(feature = "gaming")]
{
    let detector = AntiCheatDetector::new();
    let result = detector.scan();
    safety::log_scan_result(&result);

    if result.should_block() {
        // Show error dialog and exit
    }
    if result.has_warnings() {
        // Log warning and continue
    }
}
```

When `gaming` is not enabled, this entire block is elided at compile time.

---

## API Safety Contract

The safety system uses exclusively **passive, read-only** Win32 APIs. This
contract is load-bearing — violating it could cause GLASS itself to be
flagged by anti-cheat systems.

### Allowed APIs

| API | Purpose | Why safe |
|---|---|---|
| `CreateToolhelp32Snapshot` | Take snapshot of running processes | Read-only snapshot; does not open any process handle |
| `Process32First` / `Process32Next` | Iterate process snapshot entries | Reads from an immutable snapshot; never accesses process memory |
| `OpenSCManager(SC_MANAGER_ENUMERATE_SERVICE)` | Open service control manager | Read-only enumeration access |
| `OpenService(SERVICE_QUERY_STATUS)` | Query if a service exists | Read-only query; does not start, stop, or modify any service |
| `Path::exists` | Check if a driver file is installed | Filesystem metadata check only |

### Forbidden APIs

The following are explicitly forbidden in this module:

- Any API that opens a handle **to an anti-cheat process** (e.g. `OpenProcess`)
- `NtQuerySystemInformation` or similar NT-level introspection
- Any API that reads another process's memory (`ReadProcessMemory`)
- Any API that modifies service state (`StartService`, `ControlService`)

This contract is documented in the module's `// # Unsafe usage` header comment
and in the `SAFETY:` annotations on each `unsafe` block.

---

## Known Anti-Cheat Systems

```rust
pub enum AntiCheatSystem {
    Vanguard,        // Riot Vanguard — kernel-level (Valorant, League of Legends)
    Ricochet,        // RICOCHET — kernel-level (Call of Duty)
    EasyAntiCheat,   // EAC — user-mode (Epic Games titles, many others)
    BattlEye,        // BattlEye — user-mode (PUBG, Fortnite, many others)
    VAC,             // Valve Anti-Cheat — heuristic (Steam)
}
```

Each variant implements `Display` for user-facing messages (e.g. `"Riot
Vanguard"`, `"RICOCHET"`, `"EasyAntiCheat"`).

---

## Detection Policy

Each detected anti-cheat maps to a policy that determines GLASS's response:

```rust
pub enum DetectionPolicy {
    Block,  // Kernel-level AC → refuse to start
    Warn,   // User-mode AC → start with warning
    Info,   // Informational → log only
}
```

### Policy Mapping

| Anti-Cheat | Type | Policy | Action |
|---|---|---|---|
| Vanguard | Kernel-level | `Block` | Refuse to start. Show error dialog listing detected AC. |
| Ricochet | Kernel-level | `Block` | Refuse to start. Show error dialog listing detected AC. |
| EasyAntiCheat | User-mode | `Warn` | Start with warning logged. No dialog, no block. |
| BattlEye | User-mode | `Warn` | Start with warning logged. No dialog, no block. |
| VAC | Heuristic | `Info` | Log only. No warning, no action. |

**Rationale for the split:**

- **Block (kernel-level):** Kernel anti-cheat drivers (Vanguard, Ricochet) run
  at ring 0 and monitor all processes on the system. Even though GLASS is
  external and safe by design, a kernel driver *could* flag any overlay process.
  Blocking is the cautious choice to protect users.
- **Warn (user-mode):** User-mode anti-cheat (EAC, BattlEye) typically monitors
  only the game process. An external overlay is unlikely to be flagged, but
  users should be informed.
- **Info (heuristic):** VAC detection is heuristic (Steam is running). VAC
  operates server-side and does not monitor external processes. Logging is
  sufficient.

---

## Detection Struct

```rust
pub struct Detection {
    pub system: AntiCheatSystem,     // Which AC was detected
    pub policy: DetectionPolicy,     // Block / Warn / Info
    pub method: &'static str,        // How it was detected: "service", "driver", or "process"
}
```

## Scan Result

```rust
pub struct ScanResult {
    pub detections: Vec<Detection>,
}

impl ScanResult {
    pub fn should_block(&self) -> bool;       // Any detection with Block policy?
    pub fn blocked_names(&self) -> Vec<String>; // Display names of blocking ACs
    pub fn has_warnings(&self) -> bool;       // Any detection with Warn policy?
    pub fn warning_names(&self) -> Vec<String>; // Display names of warning ACs
}
```

---

## Detection Methods

Detection uses a data-driven signature table. Each anti-cheat has an
`AcSignature` struct listing its known indicators:

```rust
struct AcSignature {
    system: AntiCheatSystem,
    policy: DetectionPolicy,
    services: &'static [&'static str],   // Windows service names
    drivers: &'static [&'static str],    // Driver files under System32\drivers\
    processes: &'static [&'static str],  // Running process names
}
```

### Signature Table

| Anti-Cheat | Services | Drivers | Processes |
|---|---|---|---|
| Vanguard | `vgk`, `vgc` | `vgk.sys` | `vgc.exe`, `vgtray.exe` |
| Ricochet | `ricochet` | `ricochet.sys` | — |
| EasyAntiCheat | `EasyAntiCheat` | — | `EasyAntiCheat.exe`, `EasyAntiCheat_EOS.exe` |
| BattlEye | `BEService` | `BEDaisy.sys` | `BEService.exe` |
| VAC | — | — | `steam.exe` |

### Detection Priority

For each signature, checks run in order: **service → driver → process**. The
scan short-circuits on first match per anti-cheat system (if the service is
found, driver and process checks are skipped for that AC).

### Platform Implementation

The `SystemProbe` trait abstracts platform queries for testability:

```rust
pub trait SystemProbe: Send {
    fn service_exists(&self, name: &str) -> bool;
    fn driver_exists(&self, name: &str) -> bool;
    fn process_running(&self, name: &str) -> bool;
}
```

The real implementation (`WindowsProbe`) uses Win32 FFI:

- **`service_exists`:** `OpenSCManagerW` + `OpenServiceW(SERVICE_QUERY_STATUS)`.
  Both handles are closed via `CloseServiceHandle` before returning.
- **`driver_exists`:** `Path::new("C:\\Windows\\System32\\drivers\\{name}").exists()`.
- **`process_running`:** `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)` +
  `Process32FirstW`/`Process32NextW`. Case-insensitive comparison. Snapshot
  handle closed on all exit paths.

For testing, inject a mock `SystemProbe` via `AntiCheatDetector::with_probe(mock)`.

---

## AntiCheatDetector

```rust
pub struct AntiCheatDetector<P: SystemProbe = WindowsProbe> {
    probe: P,
}

impl AntiCheatDetector<WindowsProbe> {
    pub fn new() -> Self;                    // Real Windows APIs
}

impl<P: SystemProbe> AntiCheatDetector<P> {
    pub fn with_probe(probe: P) -> Self;     // Custom probe (testing)
    pub fn scan(&self) -> ScanResult;        // Run all checks
}
```

`scan()` iterates all `AC_SIGNATURES`, runs detection checks via the probe,
and collects results. It logs each detection and produces a final summary.

---

## Integration Flow

The safety system runs **before** any window creation, DirectComposition init,
or GPU initialization. This is the first significant operation in
`glass-starter`'s `run()` function:

```rust
fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ── Anti-cheat self-check (gaming builds only) ──────────
    #[cfg(feature = "gaming")]
    {
        let detector = AntiCheatDetector::new();
        let result = detector.scan();
        glass_overlay::safety::log_scan_result(&result);

        if result.should_block() {
            let names = result.blocked_names().join(", ");
            let msg = format!(
                "Kernel-level anti-cheat detected: {}.\n\n\
                 GLASS cannot run while kernel AC is active.\n\
                 Please close the anti-cheat software and try again.",
                names
            );
            error!("Anti-cheat gate: BLOCKING startup ({names})");
            overlay_window::show_error_dialog("GLASS — Anti-Cheat Detected", &msg);
            return Err(Box::new(GlassError::SafetyBlock(names)));
        }

        if result.has_warnings() {
            let names = result.warning_names().join(", ");
            warn!("Anti-cheat self-check: user-mode AC detected ({names}) — proceeding with caution");
        }

        for det in &result.detections {
            if det.policy == DetectionPolicy::Info {
                info!("Anti-cheat self-check: {}: info-only", det.system);
            }
        }
    }

    // ── DPI awareness (must be before any window creation) ──
    overlay_window::set_dpi_awareness();
    // ... rest of initialization
}
```

### Scan Logging

`safety::log_scan_result()` appends results to `glass-selfcheck.log` with
Unix timestamps. The log file is append-only and accumulates across runs,
providing an audit trail of what AC was detected and when.

---

## Architectural Decisions

### GLASS is safe by design

GLASS is an **external-process overlay** — it creates its own window via
DirectComposition, uses its own wgpu device and swapchain, and never injects
DLLs, hooks Present calls, or reads another process's memory. This makes it
inherently anti-cheat safe. See [ADR-002](decisions.md) for the full rationale.

The safety system is an **additional cautious layer** for gaming contexts. It
does not exist because GLASS is unsafe — it exists because kernel-level
anti-cheat drivers (Vanguard, Ricochet) operate at ring 0 and *could*
theoretically flag any overlay process, even a benign external one. The safety
system warns users before they encounter unexpected behavior.

### Why feature-gated

The `gaming` feature gate exists because:

1. **Productivity overlays should never see anti-cheat warnings.** A developer
   building a screen annotation tool or a system monitor has no reason to scan
   for game anti-cheat systems.
2. **The scan has runtime cost.** Process enumeration and service queries take
   measurable time. Non-gaming builds skip this entirely.
3. **Separation of concerns.** Gaming-specific logic should not bleed into the
   generic overlay framework.

### Extending for new anti-cheat systems

To add detection for a new anti-cheat system:

1. Add a variant to `AntiCheatSystem` with a `Display` impl.
2. Add an entry to `AC_SIGNATURES` with the appropriate services, drivers,
   and/or process names.
3. Assign a `DetectionPolicy` (Block for kernel-level, Warn for user-mode,
   Info for heuristic).
4. The rest of the pipeline (scan, result aggregation, logging, UI) handles
   the new entry automatically.

For custom probes or entirely different detection strategies, implement the
`SystemProbe` trait and inject via `AntiCheatDetector::with_probe()`.

---

## Companion Documents

| Document | Covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | High-level system architecture and layer overview |
| [decisions.md](decisions.md) | Architecture Decision Records (especially ADR-002: External Window) |
| [config-system.md](config-system.md) | Configuration loading, hot-reload, `ConfigStore` API |
| [input-system.md](input-system.md) | Passive/interactive mode switching, hotkey system |
