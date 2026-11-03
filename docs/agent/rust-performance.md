# Full Rust application performance review

## Approved optimization follow-up

Scope: fix the measured IPC bottleneck, reduce unchanged desktop work, and retain
initial profile delivery. Keep engine semantics, validation, durability, device ownership
and independent service lifetime unchanged. Rebenchmark on private NoHardware endpoints;
live hardware/output profiling remains outside scope.

| Stage | Checkable result | Status |
| --- | --- | --- |
| Implementation | Bounded responsive IPC, change-driven desktop work, deferred profile delivery | completed |
| Review and regression coverage | Safety review and runnable concurrency/startup/scheduling checks | completed |
| Rebenchmark | Comparable Release process/control results with verified workspace delivery | completed |
| Final gate | Passing tests/builds, preservation/cleanup audit and before/after report | completed |

Implemented changes:

- `scufpad/src/ipc.rs`: four scoped connection workers and four queued connections.
  Excess connections close; listener readiness replaces four-millisecond accept polling.
  Peer checks, frame bounds, per-stage deadlines and queued-command cancellation remain.
- `scufpad/src/actor.rs`: an accepted Shutdown blocks subsequent command execution
  while its acknowledgement is written. Concurrent queued Enable cannot rearm shutdown.
- `scufpad-desktop/src/worker.rs` and `app.rs`: publish/repaint only changed status,
  remove unconditional timed repaint, and park until telemetry is due. Actions, Stop All
  and editor closure explicitly wake the worker; telemetry retains its 50 ms cadence.
- `scufpad-desktop/src/edits.rs` and `app.rs`: retain the initial profile while busy,
  then reconsider delivery without overriding actual user choices or draft fields.

Independent source review found no high/medium issue in these changes. Regression
coverage now checks bounded overflow, slow-client StopAll/Shutdown, queued Enable after
Shutdown, unchanged/full-queue telemetry, worker wakeups and deferred-profile guards.
The initial gate passed 91 Rust tests and 49 Python checks, strict Clippy, formatting
and a Release build. Existing engine and persistence algorithms remain unchanged.

Fresh comparisons use the same process driver, three eight-second windows per case,
and copied pre-change executables whose hashes match the original review. Before-run
source fields describe the current checkout, not those copied binaries; original
measurement metadata supplies their pre-change source provenance. No compiler/governor
settings or production framework defaults were changed. Do not compare rejected
large-workspace baseline windows as if both editors rendered the same workspace.

### Confirmed before/after results

Fresh before/after runs completed 30 process windows. Both used the same host, X11
backend, tmpfs configuration, default Release settings and three eight-second repeats.
Before executables were copied before compilation and match the original review hashes.
The driver ran alone during sampling; no builds/tests ran concurrently with either run.

| Matching workload / metric | Before | After |
| --- | ---: | ---: |
| Default GUI visible, median CPU / one core | 4.25% | 0.00% sampled |
| Default GUI visible, window CPU range | 4.00–4.375% | 0.00–0.25% |
| Default GUI minimized, median CPU / one core | 3.50% | 0.00% sampled |
| Default GUI minimized, window CPU range | 3.375–3.50% | 0.00–0.25% |
| Headless daemon idle, median CPU / one core | 0.25% | 0.125% |
| GetState at 20/s, p99 (480 samples) | 4.041 ms | 0.132 ms |
| Serial GetState burst, p99 (1000 samples) | 4.131 ms | 0.073 ms |
| StopAll behind a partial client, median of three confirmations | 2937.202 ms | 4.142 ms |
| Default GUI, median RSS | 84.391 MiB | 80.879 MiB |
| Daemon with default GUI open, median RSS | 4.461 MiB | 6.473 MiB |

The partial-client confirmation trials were 2937.202/2969.834/2905.049 ms before and
4.142/3.947/4.190 ms after. This is approximately a **709-fold** reduction in median
acknowledgement latency for that controlled case. The complete process run independently
measured 2948.317 ms before and 1.218 ms after. These are control-delivery times, not
physical controller latency. A fully occupied worker pool still rejects excess clients;
this does not guarantee emergency service during deliberate connection saturation.

CPU accounting remains 100 Hz: one tick in eight seconds is approximately 0.125
percentage point. A sampled zero is **not zero CPU use**. Every matching GUI window
improved substantially, but the data do not support an exact percentage reduction at
that resolution. Minimized state was confirmed. Frame count, GPU time and power remain
unmeasured. Headless daemon changes are small relative to tick quantization.

Memory is not uniformly improved. The GUI used about 3.5 MiB less RSS in this fresh
comparison, while the warmed daemon retained about **2.0 MiB more** with the GUI open.
Record that concurrent-service tradeoff rather than claiming a general memory reduction
or a leak. No cold-start improvement is claimed from the single window-start markers.

### Workspace delivery and engine preservation

The fresh before run reproduced the large-backend/default-workspace mismatch. Its three
large-workspace windows remain rejected. The after screenshot shows the correct profile
name, all 16 layer labels, macro entries, the CTRL+C binding and **Loaded / applied to
bridge**, not Pending Apply. The generated private profile contains 96 macros. Therefore
the after large-workspace case is accepted as delivered; its median GUI CPU was 0.125%
and RSS 81.184 MiB. Do not compare it with the rejected before workload as a scaling test.

Manual acceptance sidecars retain this distinction. The raw driver's conservative
`large_gui_workspace_verified: false` is unchanged: it does not inspect the workspace.
No automatic UI-inspection success is claimed. User-draft protection and busy-state
deferral are also covered by the display-free self-test and Rust regressions.

The repeated twelve-case engine probe used the identical probe binary and CPU 23.
Frame workloads still reported zero allocation requests; neutral tick median was
119.6 ns versus the original 120.2 ns, and the 256-step macro case was 592.6 ns versus
594.6 ns. Engine sources were not changed, so these small timing differences are not
claimed as engine optimizations. Synchronous durable persistence remains unchanged.

### Final optimization verification

After implementation and independent source review, **91 Rust tests and 49 Python
checks pass**. Formatting, strict all-target Clippy, final optimized workspace/example
builds, native NoHardware application checks and repeated probe preflight pass.
New regressions exercise connection overflow, slow-client control delivery, shutdown
command ordering, status retry/suppression, explicit worker wakeups and profile guards.

Evidence root: `/tmp/opencode/scufpad-performance-improvements/`. It retains before/after
process data and screenshots, copied baseline binaries, the exact driver, manual workload
acceptance, `hol-comparison.json`, `comparison.json`, `after-micro-results.json`, final
check logs and preservation/cleanup evidence. Final binary hashes match measured after
binaries. Core engine/decoder, Cargo settings/lockfile, saved user profiles and the
67-file .NET reference remain unchanged. No installation, release or live output occurred.

Code graph refreshed to 824 nodes, 1971 edges and 25 communities. Root Cargo manifest
extraction remains degraded; source/compiler/runtime evidence remains authoritative.
The conditional persistence, native hardware, large-library and macro-composer findings
from the original review remain follow-ups, not silently applied optimizations.

## Original review baseline — before optimization

Scope: the shared decoder/remapping engine, Linux CLI/background service, native
I/O, desktop editor, IPC and persistence. Review and measure; do not apply production
optimizations or change framework/deployment defaults without measured justification.
No live controller grabs, virtual outputs, firmware or permission changes are authorized.

| Stage | Checkable result | Status |
| --- | --- | --- |
| Baseline and measurement contract | Current source/tool provenance and safe workload definitions | completed |
| Source review | Cited findings across core, daemon/I/O and desktop/IPC | completed |
| Measurements | Reproducible hardware-free engine, process and GUI evidence | completed |
| Final report | Ranked findings, uncertainty, checks and follow-up boundaries | completed |

Use optimized binaries. Treat warmed synthetic engine timing separately from startup,
whole-process CPU/RSS, IPC and GUI measurements. Hardware-free daemon measurements do
not establish armed USB CPU, native syscall costs or physical input latency. Preserve
existing application/profile data and clean up only owned test processes/files.

Measurement contract: twelve pure Release workloads with output-validating preflight,
separate uninstrumented timing and allocation-count passes; private NoHardware daemon
idle/fixed-rate IPC/burst/slow-client/profile-apply windows; actual native desktop
visible and minimized process windows. Per-PID CPU ticks, RSS/PSS and RPC delivery
latency are distinct from physical controller latency. Repeat samples and retain raw
counts, source/binary hashes, host/toolchain metadata and uncertainty.

RustRover symbol search resolves the actual MappingEngine; its dependency listing is
empty, so Cargo metadata/lockfile remain authoritative for dependencies. Native perf,
strace, heaptrack, valgrind and samply are not installed. Use /proc and the isolated
allocation probe rather than privileged installation or weakened perf_event settings;
stack attribution and native syscall timing remain unmeasured.

## Source review ledger

These are source-supported costs and measurement candidates, not assumed bottlenecks.
Production code remains unchanged.

| ID | Area and source | Cost / trigger | Candidate, if measurements justify it |
| --- | --- | --- | --- |
| P01 | `scufpad/src/ipc.rs:301–349`, serial `serve` | One incomplete/slow client occupies the accept path across read, command and write phases. Each phase has a separate deadline. Other status/shutdown/StopAll clients wait. | Bounded concurrent connection handling; retain UID checks, limits, queue capacity and expired-command cancellation. |
| P02 | `scufpad/src/actor.rs:317–339`; `ipc.rs:307–312` | Separate four-millisecond idle input-owner and nonblocking-accept loops; roughly 250 iterations/s each are permitted before work. | Readiness/deadline-driven waits, with prompt command, shutdown and reconnect wakeups. |
| P03 | `scufpad/src/actor.rs:168–214,336,390` | Snapshot reconstruction follows loop/input cadence: 256 key flags, recent presses and two string clones, rather than consumer demand. | Dirty/deadline-based publication with defined state age and recent-press expiry. |
| P04 | `scufpad-desktop/src/app.rs:1022–1073`; `worker.rs:402–415,500–550` | Timed repaint, polling repaint and five-millisecond action/emergency checks continue in idle code. Repaint requests coalesce; this does not prove 40 FPS or hidden rendering. | Repaint on changes; coordinate telemetry/visibility and wakeable action waits. |
| P05 | `scufpad/src/actor.rs:237–248`; `scufpad-core/src/io.rs`, `write_profile` | ApplyProfile performs validation/serialization/write/fsync on the input owner. Filesystem stalls delay subsequent reads and commands. | Measure first; consider off-owner persistence without losing atomic/private saves or explicit failure reporting. |
| P06 | `scufpad-core/src/engine.rs:156–248,299–305,379–417` | Unchanged ticks still scan sources, layers, macros and 21/256/8 digital ownership slots. | Avoid rebuilding unchanged digital state only if material; macros, analog values and hysteresis must still advance. |
| P07 | `engine.rs:185–245,260–268,400–406` | Linear layer resolution, including unheld triggers. Ordinary presses may resolve during both layer detection and ordinary dispatch. | Compare one/32 layers; use active-layer iteration before adding caches. |
| P08 | `engine.rs:202–234,278–284,308–376,459–473` | String target searches, sparse macro traversal and 285-flag instance cleanup; deadline cleanup lies outside the 64-step budget. | Measure active counts, completion/deadline tails and work delivery before indexing or active-slot tracking. |
| P09 | `engine.rs:92–139`; `validation.rs` | Construction validates twice and compiles cloned dense bindings while retaining the immutable profile. Startup/apply cost, not per-frame cost. | Remove redundant construction validation through a private validated path only if relevant. |
| P10 | `scufpad-desktop/src/app.rs:462–467,639–648,783–810,869–951,1185–1193`; `worker.rs:198–217` | Small per-frame vectors/text; up to 256 macro rows; full library parsing and comparison-time lowercase keys. | Repaint reduction first; then visible-row rendering or precomputed sort keys where measured. |
| P11 | `scufpad/src/actor.rs:110–160,264–285,344–379`; `output.rs:178–210,334–344` | Armed frames and the post-drain pass can repeat mapping/output scans; batches of 64 reports and 64 macro steps bound work, not elapsed latency. | Requires offline actor replay or approved intercepted live measurements before changing scheduling. |
| P12 | `scufpad/src/actor.rs:318–332`; `discovery.rs:115–163`; `hardware.rs:38–92` | Hardware-enabled reconnect scans/revalidates sysfs and owned native FDs once per second. NoHardware measurements exclude it. | Measure device-inventory scaling; retain strict ownership and all required grabs. |

Positive constraints to retain: allocation-free decoder structure, press-time binding
ownership, bounded macro execution, bounded/validated IPC, profile `Arc` snapshots,
unchanged uinput write suppression, uncertain-write recovery and shutdown neutralization.
Transactional profile clones and trust-boundary checks are not deletion candidates.

## Reproduction

Pure synthetic engine/serialization/compilation probe:

```sh
cargo build -p scufpad-core --example performance_probe --release --locked
./target/release/examples/performance_probe --preflight
./target/release/examples/performance_probe
```

The probe uses seven samples, 1,000,000 operations/sample by default (100 for the large
profile compilation case), a separate warmup, and output-validating preflight. Timing
runs disable allocation counting. A separate pass reports allocation/reallocation
requests and requested bytes, not live heap or allocator overhead. Set
`SCUFPAD_BENCH_ITERATIONS` to change the sample size. No IPC, GUI or hardware is started.

Whole-process review (requires the native X11 desktop session):

```sh
cargo build --workspace --release --locked
python3 tests/performance_review_test.py
python3 tools/review_performance.py --output /tmp/opencode/scufpad-perf-process --seconds 8
```

The output directory must not exist. The driver creates only private NoHardware
processes/configuration/endpoints. It records three windows per workload, 100 ms memory
samples, CPU tick endpoints, RPC latencies, startup markers and source/binary hashes.
Minimized results require confirmation of the window manager's hidden state. All
owned children are shut down/reaped, including timeout cleanup; no existing daemon is stopped.

## Synthetic Release results

Rust 1.93.1; default Release settings; Linux x86-64; AMD Ryzen 9 5900X host.
The probe was pinned to logical CPU 23, not an exclusively reserved physical core.
The governor reports `powersave`; no governor/turbo settings were changed. Seven
sample medians below are descriptive results, not confidence intervals.

| Workload | Median ns/operation | Allocation requests/operation | Requested bytes/operation |
| --- | ---: | ---: | ---: |
| Decode constructed 16-byte USB report | 12.8 | 0 | 0 |
| Neutral unchanged engine tick | 120.2 | 0 | 0 |
| Neutral tick, 32 layers / base active | 153.5 | 0 | 0 |
| Alternating passthrough A edges | 130.8 | 0 | 0 |
| Alternating two-key chord | 129.4 | 0 | 0 |
| Three-step one-shot macro | 140.3 | 0 | 0 |
| 256-step one-shot macro | 594.6 | 0 | 0 |
| 26-source synthetic macro stress | 273.2 | 0 | 0 |
| Held-layer switching with A edges | 138.8 | 0 | 0 |
| Trigger chord with hysteresis crossings | 133.0 | 0 | 0 |
| State JSON framing into a new memory buffer | 496.8 | 5 | 1322 |
| Large profile clone + validate + compile | 90901.7 | 2115 | 336188 |

The large compilation input is 501611 JSON bytes, with 16 layers and 96 macros of
128 steps. Compilation includes cloning the caller's profile and occurs outside frame
timing; its median is about **90.9 microseconds** on this workload. It is not a save,
fsync, full JSON parse, maximum-limit profile or real-time input-owner pause measurement.

All frame workloads reported zero allocation/reallocation requests during their separate
counted passes. This supports retaining the current engine design rather than applying
speculative caches or container replacements. It does not prove an allocation-free
daemon/GUI: serialization, snapshots, rendering, configuration and system libraries
are separate paths. The counting allocator's disabled branch remains in the probe
binary; timing includes workload dispatch/input construction and observable output
barriers. No hardware reads, native output writes or physical-trigger latency are included.

Raw seven-sample values, allocation counts, source/compiler/binary hashes and load
metadata: `/tmp/opencode/scufpad-performance-review/{micro-results,metadata}.json`.

## Whole-process results

Actual optimized processes, private NoHardware configuration/endpoint, forced native X11
(not a Wayland comparison). Each accepted
baseline case has three eight-second windows after warmup, with 100 ms RSS/PSS samples.
CPU is percent of **one logical core**, not percent of the 24-thread host. Linux accounts
CPU at 100 Hz: one tick over eight seconds is about 0.125 percentage point. GUI rendering,
driver work, background worker and allocator/library memory are included in its process.

| Case / process | Median CPU, one core | Window CPU range | Median RSS MiB | Median PSS MiB |
| --- | ---: | ---: | ---: | ---: |
| Headless daemon, idle | 0.25% | 0.125–0.25% | 3.074 | 1.107 |
| Headless daemon, 20 GetState/s | 0.25% | 0.25–0.375% | 3.074 | 1.115 |
| Default-workspace GUI, visible | 4.25% | 3.875–4.375% | 144.676 | 91.031 |
| Same GUI, minimized/hidden confirmed | 3.375% | 3.375–3.375% | 144.676 | 91.035 |
| Daemon while default GUI is open | 0.25% | 0.25–0.375% | 4.457 | 2.939 |

The shown cases confirm non-hidden state, not complete absence of occlusion.
Minimization left appreciable process CPU in this run. This supports investigating
hidden-window/background polling and repaint behavior, but does not establish actual
frame rate, GPU time, power consumption or which thread consumed the CPU. Source
requests can coalesce; do not call them measured 40 FPS. Likewise, short-window retained
RSS is not proof of a leak. The daemon had previously processed large profiles before
the GUI cases; its higher retained RSS is not an independent clean-start comparison.

### Control delivery and profile operations

| Private local operation | Samples | Median | p99 / max |
| --- | ---: | ---: | ---: |
| GetState at 20/s | 480 | 2.000 ms | 4.012 / 4.102 ms |
| Serial GetState burst | 1000 | 4.070 ms | 4.089 / 4.145 ms |
| Apply default profile | 10 | 8.165 ms | max 8.189 ms |
| Apply generated large profile | 10 | 6.290 ms | max 6.589 ms |

These times include client connect/I/O/response parsing plus server work; request JSON
construction occurs before the timer. They are **not physical input/output latency**.
The roughly four-millisecond burst cadence is consistent with the accept polling loop.
Apply measurements include scheduling, parse/compile and filesystem work; they do not
isolate fsync. The larger profile being faster in these small samples does not mean
larger profiles are cheaper. The generated request profile is 360051 compact JSON bytes.
The private configuration was on **tmpfs**, while the normal user's configuration
directory is on **btrfs**. These are not production-disk fsync measurements.

**P01 reproduced:** a client sent one header byte and held the connection open. A second
client's StopAll acknowledgement took **2923 ms**. Three independent confirmation
trials took **2966, 2905 and 2909 ms**. All used private NoHardware processes with
outputs zero. The confirmation baseline StopAll median was **8.103 ms** (4.037–8.121 ms).
The serial server's read deadline explains this head-of-line delay;
the input owner is separate, but control delivery waits. This is a control-latency
finding, not a cross-user access claim. Bounded concurrent handling is the first
performance/safety responsiveness candidate, without removing authentication or bounds.

Single warm-filesystem startup markers were 9.1 ms to daemon IPC readiness and 203.3 ms
to the first default GUI X11 window. The latter is not first usable paint or completed
profile loading. These are single observations with polling/host-session overhead,
not cold-start distributions.

### Rejected large-workspace GUI case / correctness follow-up

The second GUI process attached to the intended large backend profile, but the captured
workspace still showed **Default**, only Base, no macros and Pending Apply. Therefore
its three windows are **excluded from large-workspace rendering/scaling conclusions**.
Its observed 83.066 MiB RSS / 39.024 MiB PSS and 4.125% median CPU are retained only
as second-launch observations. They do not show lower memory use for larger profiles.
Twelve baseline windows are accepted; three secondary windows are not accepted as the
intended large-workspace workload. Native 256-step composer rendering is unmeasured.

**P13 (medium correctness/benchmark-delivery issue):** `app.rs:234–244` updates the
last-bridge name but discards initial workspace loading if `can_load_initial` sees busy.
`app.rs:297–300` can start Library after a Status event and mark busy before the worker
delivers InitialProfile. `worker.rs:522,528` marks that profile sent permanently; Library
Done clears busy without reconsidering it. This source-supported interleaving explains
the observed mismatch, but event order was not instrumented in this run. Reproduce with
event/guard tracing before calling that the proven run-specific cause. Defer temporary
busy-state delivery rather than dropping it, while retaining real user/draft guards.
Resolve delivery before attempting large-workspace performance tuning.

Raw process windows/images and the exact recorded driver:
`/tmp/opencode/scufpad-perf-process/`. Slow-client confirmations:
`/tmp/opencode/scufpad-perf-hol-confirm/results.json`. The recorded run uses the original
`gui_large_visible` label; the reproducible driver now names that case
`gui_large_backend_visible` and explicitly does not verify its workspace. Raw results
were not rewritten to imply successful delivery.
`workload-acceptance.json` beside the raw process results records the twelve accepted
baseline windows and three rejected large-workspace windows without changing raw data.

## Recommendations and limits

1. **High priority: P01, measured control head-of-line blocking.** Address the serial
   connection bottleneck before micro-optimizing engine loops. Use a bounded design and
   preserve peer identity, frame limits, queue limits and command cancellation. Rerun
   the partial-client StopAll test and normal RPC latency after any change.
2. **Medium: P13, startup profile deferral/delivery.** Correct or instrument this before
   trusting large-workspace UI stress results. Do not remove real draft/choice guards.
3. **Medium: P04/P02, idle desktop/background scheduling.** Investigate minimized work,
   change-driven repaint, and wakeable deadline waits. Measure GUI and daemon separately;
   no energy saving or exact frame count has yet been established.
4. **Conditional medium: P05, synchronous persistence on the input owner.** Source
   confirms the stall point; production-disk stall duration remains unmeasured. Keep
   transactional application, private atomic files, file flushes and honest failure ACKs.
5. **Lower priority: P06–P10.** The engine is already fast and showed no frame-path
   allocations in these workloads. Profile compilation/formatting and bounded UI lists
   should not receive complex caches or new abstractions without a delivered workload
   and a demonstrated interaction problem.

Native HID/evdev/uinput syscall costs, physical end-to-end latency, armed USB CPU/RSS,
macro interval/deadline tails under real reports, power/GPU attribution, large-composer
rendering, large library scaling and long-soak leak analysis remain **unmeasured**.
The twelve synthetic scenarios are representative, not an exhaustive proof over all
valid profiles. Live controller profiling requires separate approval, games closed,
owned interception, disabled reconnection and verified neutral cleanup.

No production optimization, unsafe compiler/deployment tuning, framework replacement,
profile mutation, permission change or default-endpoint testing was performed. This
review adds only reproducible nonproduction probes, helper tests, CI coverage and records.

## Final verification — 2026-10-28

All required review stages are complete. Final checks, in order:

- `cargo fmt --all -- --check`: pass.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: pass.
- `cargo test --workspace --all-targets --locked`: **87 pass**, including probe preflight.
- `python3 tests/installation_tools_test.py`: **43 pass**.
- `python3 tests/rust_application_test.py`: **2 pass**, private NoHardware endpoints.
- `python3 tests/performance_review_test.py`: **4 pass**, mocked cleanup regressions.
- `cargo build --workspace --release --locked`: pass after the tests.
- Release example build and `performance_probe --preflight`: pass, twelve scenarios.

Logs: `/tmp/opencode/scufpad-performance-review/final-*.log`. The final build preserves
the measured CLI/editor binary hashes. All 31 relocated production Rust/configuration
files match the relocation manifest, as do the 67 preserved .NET files. The 24 production
Rust source hashes and Cargo.lock also match the measurement metadata. The saved profile
JSON matches its pre-review hash. All fifteen measurement windows completed; every
review-owned process and private endpoint was cleaned up. No existing service was stopped.

The code-only graph refresh succeeded: 808 nodes, 1915 edges, 22 communities, with working
probe/cleanup symbol queries. Cargo.toml still produces no extracted nodes; the graph is
navigation evidence, not dependency or runtime proof. No new production abstraction or
dependency was added; the probes use existing dependencies and platform facilities.
