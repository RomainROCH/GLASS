//! Passive anti-cheat detector — Self-Check system.
//!
//! Scans for known anti-cheat systems using **passive, read-only APIs only**.
//! Called before any window/GPU initialization to gate whether GLASS starts.
//!
//! # API Safety Contract
//!
//! **Allowed APIs**: `CreateToolhelp32Snapshot`, `Process32First/Next`,
//! `OpenSCManager` + `OpenService(SERVICE_QUERY_STATUS)`, `Path::exists`.
//!
//! **Forbidden**: Any API that opens a handle to an AC process, queries
//! NT system info, or reads another process's memory.
//!
//! # Detection Policy
//! - **Block**: Kernel-level AC (Vanguard, Ricochet) → refuse to start
//! - **Warn**: User-mode AC (EAC, BattlEye) → start with warning
//! - **Info**: Informational (VAC) → log only

// # Unsafe usage in this module
//
// - `win32_service_exists`: Win32 FFI — `OpenSCManagerW` and `OpenServiceW` are
//   read-only service-control calls; handles are closed with `CloseServiceHandle`
//   before the function returns. Wide-string pointers are derived from `Vec<u16>`
//   locals that outlive the calls.
// - `win32_process_running`: Win32 FFI — `CreateToolhelp32Snapshot` takes a
//   process snapshot (passive; does not open any target process). `Process32FirstW`/
//   `Process32NextW` read into a stack-allocated `PROCESSENTRY32W` with `dwSize`
//   pre-set to the struct's actual size. The snapshot handle is always closed before
//   returning, on both success and failure paths.

use std::path::Path;
use tracing::{info, warn, error};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Known anti-cheat systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiCheatSystem {
    /// Riot Vanguard — kernel-level anti-cheat used by Valorant and other Riot titles.
    Vanguard,
    /// RICOCHET — kernel-level anti-cheat used by Activision's Call of Duty titles.
    Ricochet,
    /// Easy Anti-Cheat — user-mode anti-cheat used by Epic Games and many other titles.
    EasyAntiCheat,
    /// BattlEye — user-mode anti-cheat used by a wide range of multiplayer games.
    BattlEye,
    /// Valve Anti-Cheat — heuristic detection based on a running Steam process.
    VAC,
}

impl std::fmt::Display for AntiCheatSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AntiCheatSystem::Vanguard => write!(f, "Riot Vanguard"),
            AntiCheatSystem::Ricochet => write!(f, "RICOCHET"),
            AntiCheatSystem::EasyAntiCheat => write!(f, "EasyAntiCheat"),
            AntiCheatSystem::BattlEye => write!(f, "BattlEye"),
            AntiCheatSystem::VAC => write!(f, "Valve Anti-Cheat"),
        }
    }
}

/// Policy decision for a detected anti-cheat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionPolicy {
    /// Kernel-level AC — refuse to start.
    Block,
    /// User-mode AC — start with warning.
    Warn,
    /// Informational — log only, no action.
    Info,
}

/// A single detection result.
#[derive(Debug, Clone)]
pub struct Detection {
    /// The anti-cheat system that was detected.
    pub system: AntiCheatSystem,
    /// Policy decision for this detection (Block, Warn, or Info).
    pub policy: DetectionPolicy,
    /// Detection method used: `"service"`, `"driver"`, or `"process"`.
    pub method: &'static str,
}

/// Complete scan result.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// All anti-cheat systems detected during the scan.
    pub detections: Vec<Detection>,
}

impl ScanResult {
    /// Whether any detection requires blocking startup.
    pub fn should_block(&self) -> bool {
        self.detections
            .iter()
            .any(|d| d.policy == DetectionPolicy::Block)
    }

    /// Get names of systems that require blocking.
    pub fn blocked_names(&self) -> Vec<String> {
        self.detections
            .iter()
            .filter(|d| d.policy == DetectionPolicy::Block)
            .map(|d| d.system.to_string())
            .collect()
    }

    /// Whether any detection is a warning.
    pub fn has_warnings(&self) -> bool {
        self.detections
            .iter()
            .any(|d| d.policy == DetectionPolicy::Warn)
    }

    /// Get warning messages for user display.
    pub fn warning_names(&self) -> Vec<String> {
        self.detections
            .iter()
            .filter(|d| d.policy == DetectionPolicy::Warn)
            .map(|d| d.system.to_string())
            .collect()
    }
}

// ─── Detection signatures ───────────────────────────────────────────────────

/// Detection signature for a single anti-cheat system.
struct AcSignature {
    system: AntiCheatSystem,
    policy: DetectionPolicy,
    /// Windows service names to check (read-only).
    services: &'static [&'static str],
    /// Driver files under `C:\Windows\System32\drivers\` to check.
    drivers: &'static [&'static str],
    /// Process names to check (case-insensitive).
    processes: &'static [&'static str],
}

/// All known AC signatures. Maintained as a data-driven list.
const AC_SIGNATURES: &[AcSignature] = &[
    AcSignature {
        system: AntiCheatSystem::Vanguard,
        policy: DetectionPolicy::Block,
        services: &["vgk", "vgc"],
        drivers: &["vgk.sys"],
        processes: &["vgc.exe", "vgtray.exe"],
    },
    AcSignature {
        system: AntiCheatSystem::Ricochet,
        policy: DetectionPolicy::Block,
        services: &["ricochet"],
        drivers: &["ricochet.sys"],
        processes: &[],
    },
    AcSignature {
        system: AntiCheatSystem::EasyAntiCheat,
        policy: DetectionPolicy::Warn,
        services: &["EasyAntiCheat"],
        drivers: &[],
        processes: &["EasyAntiCheat.exe", "EasyAntiCheat_EOS.exe"],
    },
    AcSignature {
        system: AntiCheatSystem::BattlEye,
        policy: DetectionPolicy::Warn,
        services: &["BEService"],
        drivers: &["BEDaisy.sys"],
        processes: &["BEService.exe"],
    },
    AcSignature {
        system: AntiCheatSystem::VAC,
        policy: DetectionPolicy::Info,
        services: &[],
        drivers: &[],
        processes: &["steam.exe"], // heuristic: Steam running implies VAC may be active
    },
];

// ─── Platform detection methods (passive only) ──────────────────────────────

/// Trait abstracting system queries for testability.
pub trait SystemProbe: Send {
    /// Check if a Windows service exists (read-only query).
    fn service_exists(&self, name: &str) -> bool;
    /// Check if a driver file exists.
    fn driver_exists(&self, name: &str) -> bool;
    /// Check if a process with the given name is running.
    fn process_running(&self, name: &str) -> bool;
}

/// Real Windows implementation using passive APIs only.
pub struct WindowsProbe;

impl SystemProbe for WindowsProbe {
    fn service_exists(&self, name: &str) -> bool {
        win32_service_exists(name)
    }

    fn driver_exists(&self, name: &str) -> bool {
        let path = format!("C:\\Windows\\System32\\drivers\\{name}");
        Path::new(&path).exists()
    }

    fn process_running(&self, name: &str) -> bool {
        win32_process_running(name)
    }
}

/// Check if a Windows service exists using read-only OpenService.
///
/// Uses `OpenSCManager(SC_MANAGER_ENUMERATE_SERVICE)` + `OpenService(SERVICE_QUERY_STATUS)`.
/// These are passive, read-only operations.
fn win32_service_exists(name: &str) -> bool {
    use windows::Win32::System::Services::*;
    use windows::core::PCWSTR;

    // SAFETY: `OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), ...)` opens the local
    // machine's active SCM database; passing null for machine and database names is
    // explicitly documented to mean "local machine / active database" — no pointer is
    // dereferenced from these nulls. `wide` is a null-terminated `Vec<u16>` kept alive
    // for the entire block; `PCWSTR(wide.as_ptr())` is valid while `wide` is in scope.
    // Both handles obtained via `Ok(...)` are closed via `CloseServiceHandle` before
    // the function returns, preventing handle leaks on all code paths.
    unsafe {
        let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ENUMERATE_SERVICE);
        let scm = match scm {
            Ok(h) => h,
            Err(_) => return false,
        };

        // Convert service name to wide string
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let result = OpenServiceW(scm, PCWSTR(wide.as_ptr()), SERVICE_QUERY_STATUS);

        let exists = match result {
            Ok(svc) => {
                let _ = CloseServiceHandle(svc);
                true
            }
            Err(_) => false,
        };

        let _ = CloseServiceHandle(scm);
        exists
    }
}

/// Enumerate running processes using CreateToolhelp32Snapshot (passive, snapshot-based).
///
/// Only reads the process entry structure — never opens a handle to any process.
fn win32_process_running(name: &str) -> bool {
    use windows::Win32::System::Diagnostics::ToolHelp::*;

    // SAFETY: `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)` takes a snapshot of
    // all running processes — it does not open a handle to any individual process, so
    // it cannot interfere with AC kernel drivers. `PROCESSENTRY32W` is initialized with
    // `dwSize` set to its exact `sizeof` (required by the API) and all other fields
    // zeroed via `Default::default()`. `Process32FirstW`/`Process32NextW` read entries
    // from the immutable snapshot — no process memory is accessed. `CloseHandle` is
    // called on the snapshot on all exit paths (early return and normal loop exit),
    // preventing handle leaks.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        let snapshot = match snapshot {
            Ok(h) => h,
            Err(_) => return false,
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let name_lower = name.to_lowercase();

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                // Read the exe name from the entry (no OpenProcess!)
                let exe_name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len())],
                );

                if exe_name.to_lowercase() == name_lower {
                    let _ = windows::Win32::Foundation::CloseHandle(snapshot);
                    return true;
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
        false
    }
}

// ─── AntiCheatDetector ──────────────────────────────────────────────────────

/// Passive anti-cheat detector. Uses only read-only system APIs.
pub struct AntiCheatDetector<P: SystemProbe = WindowsProbe> {
    probe: P,
}

impl AntiCheatDetector<WindowsProbe> {
    /// Create a detector using real Windows APIs.
    pub fn new() -> Self {
        Self {
            probe: WindowsProbe,
        }
    }
}

impl<P: SystemProbe> AntiCheatDetector<P> {
    /// Create a detector with a custom probe (for testing).
    pub fn with_probe(probe: P) -> Self {
        Self { probe }
    }

    /// Scan for all known anti-cheat systems.
    ///
    /// Returns a `ScanResult` with all detections. Does not take any action —
    /// the caller decides whether to block, warn, or continue.
    pub fn scan(&self) -> ScanResult {
        info!("Anti-cheat self-check starting (passive scan)");
        let mut detections = Vec::new();

        for sig in AC_SIGNATURES {
            let mut detected = false;
            let mut method = "";

            // Check services
            for svc in sig.services {
                if self.probe.service_exists(svc) {
                    detected = true;
                    method = "service";
                    break;
                }
            }

            // Check drivers (only if not already detected)
            if !detected {
                for drv in sig.drivers {
                    if self.probe.driver_exists(drv) {
                        detected = true;
                        method = "driver";
                        break;
                    }
                }
            }

            // Check processes (only if not already detected)
            if !detected {
                for proc in sig.processes {
                    if self.probe.process_running(proc) {
                        detected = true;
                        method = "process";
                        break;
                    }
                }
            }

            if detected {
                info!(
                    "Detected: {} (method: {}, policy: {:?})",
                    sig.system, method, sig.policy
                );
                detections.push(Detection {
                    system: sig.system,
                    policy: sig.policy,
                    method,
                });
            }
        }

        if detections.is_empty() {
            info!("Anti-cheat self-check complete: no AC systems detected");
        } else {
            info!(
                "Anti-cheat self-check complete: {} system(s) detected",
                detections.len()
            );
        }

        ScanResult { detections }
    }
}

impl Default for AntiCheatDetector<WindowsProbe> {
    fn default() -> Self {
        Self::new()
    }
}

/// Write scan results to a log file.
pub fn log_scan_result(result: &ScanResult) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut lines = Vec::new();
    lines.push(format!("[{timestamp}] GLASS Self-Check scan"));

    if result.detections.is_empty() {
        lines.push(format!("[{timestamp}] Result: CLEAR — no anti-cheat systems detected"));
    } else {
        for det in &result.detections {
            lines.push(format!(
                "[{timestamp}] Detected: {} (method: {}, policy: {:?})",
                det.system, det.method, det.policy
            ));
        }
        if result.should_block() {
            lines.push(format!(
                "[{timestamp}] Action: BLOCK — refusing to start (kernel AC: {})",
                result.blocked_names().join(", ")
            ));
        } else if result.has_warnings() {
            lines.push(format!(
                "[{timestamp}] Action: WARN — starting with warnings ({})",
                result.warning_names().join(", ")
            ));
        }
    }

    let log_entry = lines.join("\n") + "\n\n";

    // Append to log file
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("glass-selfcheck.log")
    {
        Ok(mut file) => {
            use std::io::Write;
            if let Err(e) = file.write_all(log_entry.as_bytes()) {
                error!("Failed to write self-check log: {e}");
            }
        }
        Err(e) => {
            warn!("Could not open self-check log file: {e}");
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Mock probe for testing — configurable service/driver/process existence.
    struct MockProbe {
        services: HashSet<String>,
        drivers: HashSet<String>,
        processes: HashSet<String>,
    }

    impl MockProbe {
        fn empty() -> Self {
            Self {
                services: HashSet::new(),
                drivers: HashSet::new(),
                processes: HashSet::new(),
            }
        }

        fn with_service(mut self, name: &str) -> Self {
            self.services.insert(name.to_string());
            self
        }

        fn with_driver(mut self, name: &str) -> Self {
            self.drivers.insert(name.to_string());
            self
        }

        fn with_process(mut self, name: &str) -> Self {
            self.processes.insert(name.to_string());
            self
        }
    }

    impl SystemProbe for MockProbe {
        fn service_exists(&self, name: &str) -> bool {
            self.services.contains(name)
        }

        fn driver_exists(&self, name: &str) -> bool {
            self.drivers.contains(name)
        }

        fn process_running(&self, name: &str) -> bool {
            self.processes.contains(name)
        }
    }

    #[test]
    fn no_ac_detected_on_clean_system() {
        let detector = AntiCheatDetector::with_probe(MockProbe::empty());
        let result = detector.scan();
        assert!(result.detections.is_empty());
        assert!(!result.should_block());
        assert!(!result.has_warnings());
    }

    #[test]
    fn vanguard_service_triggers_block() {
        let probe = MockProbe::empty().with_service("vgk");
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        assert!(result.should_block());
        assert_eq!(result.detections.len(), 1);
        assert_eq!(result.detections[0].system, AntiCheatSystem::Vanguard);
        assert_eq!(result.detections[0].policy, DetectionPolicy::Block);
    }

    #[test]
    fn vanguard_driver_triggers_block() {
        let probe = MockProbe::empty().with_driver("vgk.sys");
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        assert!(result.should_block());
        assert_eq!(result.detections[0].system, AntiCheatSystem::Vanguard);
    }

    #[test]
    fn vanguard_process_triggers_block() {
        let probe = MockProbe::empty().with_process("vgc.exe");
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        assert!(result.should_block());
    }

    #[test]
    fn eac_triggers_warn_not_block() {
        let probe = MockProbe::empty().with_process("EasyAntiCheat.exe");
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        assert!(!result.should_block());
        assert!(result.has_warnings());
        assert_eq!(result.detections[0].system, AntiCheatSystem::EasyAntiCheat);
        assert_eq!(result.detections[0].policy, DetectionPolicy::Warn);
    }

    #[test]
    fn battleye_triggers_warn() {
        let probe = MockProbe::empty().with_service("BEService");
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        assert!(!result.should_block());
        assert!(result.has_warnings());
        assert_eq!(result.detections[0].system, AntiCheatSystem::BattlEye);
    }

    #[test]
    fn vac_is_info_only() {
        let probe = MockProbe::empty().with_process("steam.exe");
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        assert!(!result.should_block());
        assert!(!result.has_warnings());
        assert_eq!(result.detections.len(), 1);
        assert_eq!(result.detections[0].policy, DetectionPolicy::Info);
    }

    #[test]
    fn multiple_detections() {
        let probe = MockProbe::empty()
            .with_service("vgk") // Vanguard → block
            .with_process("EasyAntiCheat.exe"); // EAC → warn
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        assert!(result.should_block());
        assert!(result.has_warnings());
        assert_eq!(result.detections.len(), 2);
    }

    #[test]
    fn blocked_names_lists_only_blockers() {
        let probe = MockProbe::empty()
            .with_service("vgk")
            .with_process("EasyAntiCheat.exe");
        let detector = AntiCheatDetector::with_probe(probe);
        let result = detector.scan();
        let names = result.blocked_names();
        assert_eq!(names.len(), 1);
        assert!(names[0].contains("Vanguard"));
    }

    #[test]
    fn policy_routing_correct() {
        // Verify all signatures have the expected policy
        for sig in AC_SIGNATURES {
            match sig.system {
                AntiCheatSystem::Vanguard | AntiCheatSystem::Ricochet => {
                    assert_eq!(sig.policy, DetectionPolicy::Block);
                }
                AntiCheatSystem::EasyAntiCheat | AntiCheatSystem::BattlEye => {
                    assert_eq!(sig.policy, DetectionPolicy::Warn);
                }
                AntiCheatSystem::VAC => {
                    assert_eq!(sig.policy, DetectionPolicy::Info);
                }
            }
        }
    }

    #[test]
    fn source_contains_no_forbidden_apis() {
        // Belt-and-suspenders: verify this source file doesn't USE forbidden APIs.
        // We construct the forbidden patterns at runtime so the test source itself
        // doesn't contain them as literal strings (which would cause false positives).
        let source = include_str!("safety.rs");

        let forbidden = [
            (format!("{}rocess(", "OpenP"), "Open Process call"),
            (format!("{}ystemInformation", "NtQueryS"), "NtQuery SI"),
            (format!("{}emory", "ReadProcessM"), "Read Process Mem"),
        ];

        for (pattern, label) in &forbidden {
            // Count occurrences: allow in comments/strings but not as bare API calls.
            // The pattern check is intentionally simple — if this file references
            // these APIs at all (even in comments), it's a red flag worth reviewing.
            let count = source.matches(pattern.as_str()).count();
            assert!(
                count == 0,
                "FORBIDDEN API pattern ({label}) found {count} time(s) in safety.rs"
            );
        }
    }
}
