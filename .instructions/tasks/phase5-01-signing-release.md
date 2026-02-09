---
schema: task/v1
id: phase5-01
title: "Obtain EV code-signing certificate and implement signed release pipeline + GitHub Releases distribution"
type: feature
status: not-started
priority: high
owner: "unassigned"
skills: ["security", "infra-settings", "docs", "planning-feature"]
depends_on: ["phase4-01-beta-testing", "phase3-03-ci-pipeline"]
next_tasks: ["phase5-03-distribution"]
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

Phase 5 of the project covers release readiness: code signing, reproducible release builds, and establishing distribution channels. See the Phase 5 entry in the project plan: `.instructions/artefacts/glass-arch-v3_2-PLAN-artefact.md` → **Phase 5 – Release** (signing, release pipeline, distribution).

The goal of this task is to obtain an **EV (Extended Validation) Authenticode** code-signing certificate (or an equivalent trustable signing approach), establish a secure signing workflow suitable for CI, produce release builds with LTO/strip/version stamping, and publish signed portable `.exe` artifacts on GitHub Releases with checksums and verification artifacts.

## Acceptance Criteria ✅

- EV code-signing certificate procured and ownership/administrative contacts recorded (or approved cloud/HSM signing provider selected).  
- A documented secure signing approach is chosen: hardware token (USB/HSM) or cloud HSM (Azure Key Vault, signing-as-a-service), with notes on secrets/storage/access controls.  
- CI release job that triggers on tags (e.g., `v*.*.*`) builds the release artifact with: LTO enabled, symbols stripped (or controlled symbol artifact), and a deterministic/version-stamped `.exe` artifact (embedded `--version` output showing tag/commit+dirty).  
- Release artifact is signed with a timestamped EV Authenticode signature and the signature verifies successfully (`signtool verify /pa /v` or `Get-AuthenticodeSignature`) including timestamp and full chain.  
- Signed `.exe`, `SHA256` checksum file, and a signature verification artifact (e.g., `.sig` or verified output) are uploaded automatically to a GitHub Release for the matching tag.  
- Validation steps (below) are documented and automated where feasible (signature verification, checksum verification, basic smoke run on a clean Windows runner/VM).  
- Reviewer can follow the Validation Notes and confirm a signed, timestamped binary that Windows trusts (signature chain present) and that the release pipeline runs on CI successfully.

## Plan / Approach 🔧

1. Procurement & policy (owner / budgeting): identify account and approver to purchase EV certificate or sign-up with a managed signing provider. Capture legal/billing steps and expected lead time (EV issuance may take several business days).  
2. Decide signing medium: local hardware token (USB/YubiKey), corporate HSM (Azure Key Vault), or a third-party signing service (e.g., Azure SignTool / vendor). Document tradeoffs (security, CI integration, cost).  
3. Implement build changes: enable LTO (Cargo/Rust release settings), produce stripped binaries (use `strip` in CI or `-C link-arg=-s`), add a `build.rs` or CI step for deterministic version stamping (git tag, commit short, build timestamp).  
4. Create CI signing job: a dedicated, auditable job that checks out a release tag, builds artifacts reproducibly, signs with EV cert via `signtool` (or an equivalent), timestamps the signature, and performs post-sign verification.  
5. Publish release: upload signed `.exe`, `SHA256SUMS` (and `.sha256` per artifact), and a short release notes file to GitHub Releases. Mark the job to run on protected branches and require manual approval if private keys are involved.  
6. Documentation: add a `RELEASE.md` or update `.instructions/release.md` with procurement steps, CI runbook, how-to-verify instructions, and a checklist for making a release.

## Validation Notes (how a reviewer verifies this work) 🔍

- Run the CI release job on a candidate tag and confirm it finishes successfully and uploads artifacts to GitHub Releases.  
- Download the signed `.exe` on a clean Windows VM and run:
  - `Get-AuthenticodeSignature .\glass-<version>.exe` in PowerShell: confirm `Status` is `Valid` and a timestamp is present.  
  - `signtool verify /pa /v .\glass-<version>.exe`: confirm the verification chain and timestamp.  
- Confirm `glass.exe --version` reports the expected tag/commit+build metadata.  
- Validate checksum file: `Get-FileHash -Algorithm SHA256 .\glass-<version>.exe` matches the published `SHA256SUMS`.  
- Perform a smoke-run of the binary on a clean test VM to ensure it launches and basic functionality works.  
- Document manual mitigation steps if SmartScreen or Windows Defender blocks: (collect SmartScreen report details, advise waiting for reputation accrual or consider notarization approaches).  

## Links & References 🔗

- Project plan: `.instructions/artefacts/glass-arch-v3_2-PLAN-artefact.md` (Phase 5 notes)  
- Microsoft Authenticode / SignTool docs: https://learn.microsoft.com/windows/win32/seccrypto/signtool  
- Windows timestamping and EV guidance: https://learn.microsoft.com/windows/win32/seccrypto/certificate-signing  
- Sigstore / modern signing alternatives (for discussion): https://sigstore.dev

## Next Steps / Checklist ✅

- [ ] Decide owner and budget to obtain an EV cert (or approve use of a managed signing service).  
- [ ] Select signing medium (USB token vs cloud HSM) and procure/test it.  
- [ ] Implement CI release job that builds (LTO/strip/version stamp), signs, verifies, and uploads to GitHub Releases.  
- [ ] Add `RELEASE.md` with verification steps and runbook.

## Questions for you ❓

1) Who should I assign as the `owner` for this task?  
2) Do you prefer a local hardware token (USB/HSM) that the team controls, or a cloud-managed signing approach (Azure Key Vault / signing service) to integrate with CI?  

---

**How to validate this task file:** Open this file and confirm the acceptance criteria and validation steps are sufficient; assign the owner and approve procurement strategy and I can proceed to implementation.
