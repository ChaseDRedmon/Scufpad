# Rust workspace progress

## Hardware-free seeded fuzzing and stress — 2026-10-29

- Added deterministic mapping/ownership, USB mutation, editor/reference/draft and
  headless composer campaigns without new dependencies. The extended engine run
  passed 4,718,592 randomized frames plus deadline witnesses; successful tick paths
  made zero counted Rust heap allocations. This is not coverage-guided fuzzing.
- Reproduced/fixed two stale-index editor panics, structurally valid but oversized
  workspace acceptance, near-limit GetProfile telemetry overflow, and hidden load
  rejection/applied-state errors. Existing 1 MiB framing and hardware safeguards
  remain intact. Independent review found the envelope and oracle gaps; fixes and
  positive witnesses were reconciled with no remaining material review finding.
- Local final gate: **121 Rust tests passed in debug and Release, four ignored**;
  all three non-live
  opt-in stress checks separately passed. **64 Python checks**, strict Clippy,
  formatting, optimized workspace build and desktop self-test passed.
- CPU-2-pinned Release diagnostics: deliverable 32-layer/128-macro edits about
  1.9 ms per rename/edit pair; repeated 4,096-frame headless composer runs had p50
  about 0.20 ms and p95 0.21–0.30 ms. Warm pure engine workloads remained allocation-free. These are
  not physical latency, GPU, full desktop idle CPU/RSS, or long-term leak results.
- Report and runnable checks: [rust-fuzz.md](rust-fuzz.md). Evidence remains in
  `/tmp/opencode/scufpad-fuzz-20261029-1/`. No commit, push, GitHub mutation, native
  window, real profile/device access, installed update or live output occurred.

## History migration and issue triage — 2026-10-29

- Initialized Git in this workspace, retaining original `release` commit `2ca0975`
  and its full C# ancestry. Imported the existing Rust application on new branch
  `migration/rust-rewrite`; the requested PR targets `release`. The dirty C# checkout
  and installed application remain untouched. No remote branch is rewritten.
- Reproduced and fixed #7's X/Y kernel-code reversal. #8's unread-hidraw mechanism
  is absent in the Rust actor; a real poll/read helper regression confirms reports
  are consumed and readiness clears. #9 now has early, seat-scoped udev guidance,
  including the hidraw access Rust requires. Physical hardware confirmation is open.
- Rust validation now builds/smokes native bundles for CI artifacts. Publication on
  `release` waits for reusable validation, uses a narrowly scoped write job, and
  resumes the same commit's release on retry. Retired legacy .NET Qodana and unsafe
  or check-suppressing housekeeping automation; no new secrets/dependencies added.
- Local gate: **110 Rust tests passed, one live test ignored; 64 Python tests passed**,
  strict lint/formatting, optimized workspace build and desktop self-test passed.
  Actual native archive, packaged-binary tests and isolated install/uninstall passed.
- Details and limits: [rust-migration.md](rust-migration.md). Hosted PR checks and
  publication are not claimed by local checks. No merge, release, cutover or real
  device access is authorized by this checkpoint.

## Rust 1.99.1 update and application review — 2026-10-28

- Installed and pinned Rust 1.99.1 for local builds and CI; retained and verified the
  workspace's Rust 1.88 minimum across all targets. Locked dependencies remain unchanged.
- Corrected new float fallback and Clippy warnings. CI now runs the private-bus tray
  regression. Official release highlights and the full application review are in
  [rust-1.99-review.md](rust-1.99-review.md).
- The review fixed a trigger hysteresis telemetry boundary and misleading profile
  status; focused actor regressions pass.
- Final gate: 107 Rust tests, 50 Python checks, desktop self-test, strict Clippy,
  formatting and optimized workspace build pass. Code graph: 944 nodes, 2225 edges,
  33 communities. Live hardware, actual tray visibility and performance remain untested.
- Review identified a latent safe-generic ioctl boundary for separate hardening; no
  currently triggered overflow was established. Evidence: `/tmp/opencode/scufpad-rust-1.99/`.

## Bridge status icon complete — 2026-10-28

- The old bridge PID 2192957 acknowledged shutdown, exited and removed its endpoint.
  No bridge or controller was restarted during tray development.
- Added a bridge-owned StatusNotifierItem with state-specific icons and menu actions.
  The editor can close independently; opening it from the tray cannot auto-restart a
  stopped bridge. Stop actions bypass queue saturation and retain shutdown safety.
- Final gate: 104 Rust tests and 50 Python checks pass, including private NoHardware
  D-Bus integration. Rust 1.88 bridge check, strict Clippy, formatting and optimized
  workspace build pass. Graph: 931 nodes, 2196 edges, 36 communities.
- Live icon visibility and controller/game delivery remain unverified. Usage and
  evidence: [rust-tray.md](rust-tray.md), `/tmp/opencode/scufpad-tray/`.

## Automatic forwarding complete — 2026-10-28

- Replaced the all-or-nothing startup gate. Normal gamepad forwarding starts by default;
  the saved custom profile activates automatically after six-input verification.
- Unverified input cannot run custom keyboard/mouse actions, macros or layers. Alias
  detection/profile changes fall back to normal forwarding without replacing the gamepad.
- Stop All/shutdown still neutralize and remove every output. Reports, reconnects and
  profile application do not undo Stop All. Resume outputs/Arm is explicit after stopping.
- Native keyboard/mouse creation is gated and transactional. UI/help/README now separate
  gamepad availability from custom verification. Profile/wire schemas are unchanged.
- Final gate: **98 Rust tests and 49 Python checks pass**, strict Clippy, formatting,
  optimized builds and private NoHardware application checks. A bounded overflow-test
  race was repaired; five focused repetitions and the full suite pass.
- Fresh preservation checks retain profile JSON, all 67 reference .NET files and Cargo
  configuration. Graph refreshed: 846 nodes, 2033 edges, 26 communities; root manifest
  extraction still degraded. No live user process or hardware was restarted/tested.

Contract, evidence and restart commands: [rust-startup.md](rust-startup.md).
Artifacts: `/tmp/opencode/scufpad-auto-forward/`. The user must close the game/editor
and restart the existing bridge to load the new binary. Live game acceptance is open.

## Performance fixes and rebenchmark complete — 2026-10-28

- Fixed P01 with four bounded IPC workers, a four-connection queue and readiness-based
  accept. Added a shutdown barrier so later queued Enable cannot rearm during shutdown.
- Fixed P04 with change-only status/repaint and wakeable worker waits. Kept 50 ms
  telemetry, immediate action/Stop All wakeups and independent bridge lifetime.
- Fixed P13 with deferred initial-profile delivery and retained draft/choice guards.
  The large workspace now visibly loads its name, 16 layers, macros and CTRL+C binding.
- Repeated 30 process windows using copied original executables and new Release builds.
  Visible GUI CPU: 4.25% median before, 0–0.25% across new windows, per logical core.
  GetState p99 at 20/s: 4.041 → 0.132 ms. Partial-client StopAll confirmation median:
  2937.202 → 4.142 ms. Warm daemon RSS with GUI increased about 2.0 MiB; this tradeoff
  is retained in the report. Sampled zero CPU does not mean no CPU use.
- Final gate: **91 Rust tests and 49 Python checks pass**, strict Clippy/formatting,
  final Release builds and repeated allocation/output probe pass. All synthetic frame
  workloads remain allocation-free; engine and durable persistence algorithms unchanged.
- Changes are limited to IPC/actor coordination, desktop worker/delivery, regressions,
  benchmark binary selection and records. Cargo settings, saved profiles and all 67
  .NET reference files remain unchanged. No owned review processes/endpoints remain.
- Code graph: 824 nodes, 1971 edges, 25 communities; manifest extraction remains degraded.

Report: [rust-performance.md](rust-performance.md), approved optimization follow-up.
Evidence: `/tmp/opencode/scufpad-performance-improvements/`. Baseline large-workspace
windows remain rejected; the new large-workspace case has separate manual acceptance.
No install, publication, cutover or live hardware/output benchmark was performed.

## Performance review complete — 2026-10-28

- Reviewed core, service/native I/O, IPC, persistence and desktop paths. Ranked findings
  and measured evidence: [rust-performance.md](rust-performance.md), P01–P13.
- Measured twelve pure Release scenarios and fifteen real process windows. Twelve
  baseline windows passed workload acceptance; three second-launch windows were rejected
  as large-workspace rendering tests. Raw results and screenshots remain unchanged.
- Highest priority: a partial private IPC client delayed StopAll acknowledgement by
  2.90–2.97 seconds. The minimized editor retained about 3.38% of one logical core.
  The engine showed zero allocation requests in synthetic frame workloads.
- Added only the example probe, private NoHardware measurement driver, four mocked
  cleanup checks, CI coverage and review records. No production optimization was applied.
- Final gate: **87 Rust tests and 49 Python checks pass**; formatting, strict all-target
  Clippy, optimized workspace/probe builds and twelve-scenario Release preflight pass.
- All 31 relocated production/source/configuration hashes and all 67 preserved .NET
  hashes match the relocation manifest. Measured binary hashes still match after the
  final build; saved profile JSON matches the pre-review hash. No owned review binaries
  or private endpoints remain. No install, commit, publication or cutover occurred.
- Refreshed code graph: 808 nodes, 1915 edges, 22 communities. Manifest-extraction
  degradation remains explicit; new probe/cleanup symbol queries work.

Evidence: `/tmp/opencode/scufpad-performance-review/` (micro data, provenance and final
check logs), `/tmp/opencode/scufpad-perf-process/` (process data/images and workload
acceptance), `/tmp/opencode/scufpad-perf-hol-confirm/` (independent slow-client trials).
Live USB/output latency, native syscall/stack attribution and large-workspace rendering
remain unmeasured. Follow-up implementation requires separate scope; the review did
not change IPC handling or startup profile delivery.

## Relocation complete

Current project: `/home/chase/Coding/Rust/Scufpad`.
Original reference: `/home/chase/Coding/CSharp/Personal/Console/Scufpad`.

- Moved the Cargo workspace, lockfile, toolchain pin, three crates, native runtime
  test and build artifacts. All 31 source/configuration hashes are unchanged.
- Copied shared installer support, its tests, captured USB fixtures, license, icon,
  project instructions and product/port records. No build depends on the C# checkout.
- Added a standalone README, ignore rules and Rust-only CI configuration. Corrected
  old checkout commands and removed its Rust-only CI job. Preserved all 67 .NET files.
- New-location gate: **86 Rust tests and 45 Python checks pass**, strict Clippy and
  formatting pass, optimized native builds and bundle packaging pass. Display-free
  desktop self-test runs without a usable system .NET runtime.
- Existing C# checkout: four workflow checks, 43 installer checks and diff validation
  pass. Installed applications and profiles were not changed or restarted.
- Native code graph: 714 nodes, 1757 edges, 18 communities. The old reference graph
  was refreshed after relocation: 1868 nodes, 4202 edges, 108 communities. Manifest
  extraction and previous identity/AXAML limits remain; source/builds are authoritative.

At that relocation checkpoint this directory was deliberately not initialized as a
Git repository. No remote, commit, push, release or application cutover had been
performed. Git initialization is now recorded in the migration entry above. Verification artifacts: `/tmp/opencode/scufpad-rust-relocation/`;
preservation manifest: `/tmp/opencode/scufpad-rust-relocation.json`.

The prior hardware/interactive acceptance boundary remains unchanged; see
[rust-port.md](rust-port.md). No physical input/output activity was used for relocation.
