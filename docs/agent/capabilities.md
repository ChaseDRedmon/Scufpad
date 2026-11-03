# Rust workspace capabilities

## Rust 1.99.1 — 2026-10-28

| Capability | Status | Evidence / limit |
| --- | --- | --- |
| Toolchain | OK | Rust 1.99.1 installed and pinned for workspace and CI; all three crates rebuilt in Release. |
| Minimum compiler | OK | Rust 1.88 checks all workspace targets with locked dependencies. |
| Verification | OK | 107 Rust tests, 50 Python checks, desktop self-test, strict Clippy and formatting pass. |
| Review | OK | Official release notes and application sweep in [rust-1.99-review.md](rust-1.99-review.md); latent ioctl abstraction recorded separately. |
| Live acceptance | BLOCKED | No controller, live game, visible tray or performance measurements were used. |

## Bridge status icon — 2026-10-28

| Capability | Status | Evidence / limit |
| --- | --- | --- |
| Old bridge shutdown | OK | PID 2192957 acknowledged shutdown, exited and removed its socket. |
| Tray protocol and actions | OK | Private D-Bus watcher test covers registration, status, menu actions, editor launch and cleanup. NoHardware remains isolated; a missing bus is nonfatal. |
| Final checks | OK | 104 Rust tests, 50 Python checks, Rust 1.88 bridge check, strict Clippy/format and Release workspace build pass. |
| Live tray and controller | BLOCKED | Actual desktop presentation and live controller/game behavior need user validation. No hardware was accessed. |
| Code graph | DEGRADED | Updated graph has 931 nodes, 2196 edges and 36 communities; root manifest extraction remains degraded. |

Contract and evidence: [rust-tray.md](rust-tray.md).

## Automatic forwarding — 2026-10-28

| Capability | Status | Evidence / limit |
| --- | --- | --- |
| Startup routing | OK | Normal gamepad passthrough is automatic and allocation-free. Saved custom mappings activate only after verification; loss falls back without replacing the gamepad. Stop All remains authoritative. |
| Native output lifecycle | OK | Regular-file event-sink tests verify custom neutralization and retained gamepad descriptor. NoHardware application checks verify no native device access; live uinput/Steam delivery remains unverified. |
| Final checks | OK | 98 Rust tests and 49 Python checks pass, strict all-target Clippy/formatting and optimized native builds pass. |
| Preservation | OK | Fresh profile/reference/configuration baseline matches. No running user bridge, editor or game was stopped, armed, restarted or profiled. |
| Code graph | DEGRADED | Refreshed local code graph: 846 nodes, 2033 edges, 26 communities. Root Cargo manifest still has no extracted symbols. |
| Live game acceptance | BLOCKED | Requires games-closed restart and subsequent user validation. Software checks do not establish Helldivers/Proton delivery. |

Current contract and use: [rust-startup.md](rust-startup.md).

## Performance fixes — 2026-10-28

| Capability | Status | Evidence / limit |
| --- | --- | --- |
| Verified fixes | OK | Bounded concurrent IPC, shutdown ordering, change-driven repaint/status, wakeable worker, and deferred initial-profile delivery. Independent source review found no high/medium issue. |
| Final checks | OK | 91 Rust tests and 49 Python checks pass, strict Clippy/formatting, final Release builds and repeated output/allocation probe pass. |
| Before/after measurements | OK | Thirty completed private NoHardware windows and three partial-client confirmations per binary. GUI CPU and RPC/control latency improved; warmed daemon RSS with GUI increased about 2.0 MiB. |
| Large workspace delivery | OK | After screenshot shows correct name, all 16 layers, macro entries, CTRL+C and cleared Pending Apply. Manual acceptance only; rejected before workload is not used for scaling comparisons. |
| Refreshed code graph | DEGRADED | 824 nodes, 1971 edges, 25 communities. Root Cargo manifest still yields no extracted nodes; source/compiler/runtime evidence is authoritative. |
| Live latency / native attribution | BLOCKED | No new hardware authorization. Physical latency, armed USB CPU, native syscall/stack, GPU/power and macro-composer profiling remain unmeasured. |

Results, tradeoffs and acceptance limits: [rust-performance.md](rust-performance.md).

## Performance review — 2026-10-28

| Capability | Status | Evidence / limit |
| --- | --- | --- |
| Final checks | OK | 87 all-target Rust tests and 49 Python checks pass: 43 installer, two private NoHardware application, four mocked cleanup. Formatting, strict Clippy, Release builds and probe preflight pass. |
| Synthetic performance probe | OK | Twelve output-validating Release scenarios, seven timing samples each, separate allocation passes. Frame workloads report zero allocation requests; hardware and process overhead excluded. |
| Process / control measurements | OK | Fifteen completed private NoHardware windows; twelve accepted baseline windows. `/proc` CPU/RSS/PSS, real IPC, X11 startup/minimization, source/binary hashes and owned cleanup audited. |
| Large-workspace GUI workload | DEGRADED | Three windows rejected for rendering/scaling: backend profile delivered, workspace stayed Default/Base/no macros. Screenshots retained; P13 records a possible startup race, not an instrumented run-specific trace. |
| Native stack / syscall profiling | DEGRADED | perf, strace, valgrind, heaptrack and samply unavailable. Used allocation instrumentation and `/proc`; no privileged installs or security changes. Stack, syscall, GPU and power attribution remain unmeasured. |
| Refreshed code graph | DEGRADED | 808 nodes, 1915 edges, 22 communities; new probe/cleanup symbol queries work. Root Cargo manifest still produces no extracted nodes; Cargo metadata and source remain authoritative. |
| Live USB / output profiling | BLOCKED | Separate approval required with games closed, owned interception, disabled reconnection and neutral cleanup. NoHardware results do not establish physical latency or armed CPU use. |

Methods, measurements and limits: [rust-performance.md](rust-performance.md).

## Relocation baseline

| Capability | Status | Evidence / limit |
| --- | --- | --- |
| Independent workspace | OK | Cargo root and all local package paths resolve inside this directory; copied fixtures/assets and legacy-bundle compatibility tests need no C# source checkout. |
| Toolchain | OK | Existing pinned Rust 1.93.1 builds the relocated crates; Cargo, rustfmt and strict all-target Clippy pass. |
| Tests | OK | 86 Rust tests, 43 installer cases and two private no-hardware application cases pass in the new location. |
| Native build/package | OK | Optimized CLI/editor build locally; native bundle packages from the new root and editor self-test works without system .NET. |
| Preservation | OK | Moved source/configuration hashes match; all 67 .NET source/project/XAML files and shared reference assets are unchanged. No installed app/profile or device-permission changes. |
| Graphify | DEGRADED | Independent code-only graph: 714 nodes, 1757 edges, 18 communities. Root Cargo manifest has no extracted symbols; compiled source/metadata is authoritative. |
| Git/remote | DEGRADED | This new folder is not a Git repository; none was initialized or published without approval. Existing C# repository/remote remain intact. |
| Live hardware / full UI acceptance | BLOCKED | Prior port boundary remains: separate approval and games-closed owned interception are required before live-output testing or cutover. Relocation used no hardware. |

Evidence and current location: [progress.md](progress.md).
