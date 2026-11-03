# Rust seeded fuzzing and bounded stress review

## Outcome and boundaries

Local, hardware-free randomized checks and Release stress runs completed. They found
and repaired editor/IPC correctness defects; no mapping mismatch or tick allocation
was observed in the tested engine streams. This is **not** proof that the application
has no performance or mapping defects. These are deterministic seeded property tests,
not coverage-guided fuzzing, sanitizers, or an exhaustive state-space search.

No commits, pushes, GitHub modifications, tool installations, real user-profile reads,
installed-app changes, device grabs, native windows, or virtual output devices were
performed. Test IPC uses private endpoints with hardware explicitly disabled. The
headless compositor tests instantiate only egui/MacroEditor, or Studio with a test-only
inert event channel—not its real worker or daemon-startup path.

## Actionable findings and fixes

1. **Two unchecked editor-index panic paths.** `rename_layer` panicked for layer index
   `len()`; `save_macro` panicked for macro index zero in an empty workspace. Both also
   lacked protection against `usize::MAX`. Replacing their unchecked lookups with
   checked errors preserves the source workspace. Both new regressions were actually
   run against the unchecked implementations: two panics, then two passing tests after
   restoration of the guards. Normal GUI selection invariants usually provide valid
   indices; a user-click sequence triggering these stale indices was not established.
2. **Editor accepted structurally valid but undeliverable workspaces.** A profile at
   32 layers, 128 macros and 256 steps per macro can exceed the unchanged 1 MiB bridge
   limit. An explicit regression first failed because `checked` accepted it. Checked
   editor transforms/load now test the profile-response envelope through the existing
   serializer before accepting the replacement; rejection leaves the old workspace
   intact and explains the limit. Structural limits remain individual ceilings, not a
   promise that every combination fits the transport. A 128-step version is accepted
   and tested through both message directions. No message-size protection was removed.
3. **Near-limit ApplyProfile succeeded while GetProfile overflowed.** Independent
   review caught the initial incomplete envelope assumption: GetProfile normally adds
   a live state snapshot with variable-sized fields. A generated near-limit profile
   fits the apply request and profile-only reply but not reply plus telemetry. Its
   framed read/write regression failed on the real `ipc::respond` path before the fix.
   GetProfile now omits **optional telemetry only when needed**, retaining the profile;
   GetState is unchanged. In-tree callers consume GetProfile's profile and obtain state
   separately. The schema already makes state optional. This boundary check uses real
   response construction and in-memory frame round trips, not a near-limit kernel
   socket test. Existing runtime tests separately cover real private IPC processes.
4. **Successful worker completion hid a rejected load error.** Loaded/Done event order
   could report success after checked load rejected a workspace. The new receive test
   failed before the fix and now confirms that the error and old workspace survive,
   busy clears, and a subsequent action can clear the old error. Rejected initial data
   also cannot mark the retained workspace as applied; its additional assertion failed
   before making load success explicit. These are inert-worker event tests, not file-
   dialog or interactive acceptance.

Independent review also corrected two initially weak fuzz oracles: valid renames must
succeed rather than silently accepting Err, with actual reference-update witnesses;
and macro campaigns must prove active key output/pending work before cancellation
and hard-deadline neutrality. The final bounded independent gate found no remaining
material defect in the reviewed changes. This does not extend to an entire-repo audit.

## Runnable coverage

Normal `cargo test --workspace --all-targets --locked` includes:

- **64 direct-mapping seeds × 512 frames.** An independent union-of-bindings oracle
  checks gamepad/key/mouse outputs, shared chord ownership (up to 16 keys), native and
  digital triggers, hysteresis, startup/cancel/apply suppression, rejected invalid
  apply, and backward clocks. Successful tick paths must make zero counted Rust heap
  allocation/reallocation calls. Profile construction/apply, other threads, native
  allocation and error-path allocation are outside that assertion.
- **8 layer/macro seeds × 512 frames.** Four acyclic layers, randomized layer actions,
  eight 256-step macro definitions plus an explicit deadline witness, concurrent
  presses, equal timestamps, cancellation, hard deadlines and `u64::MAX`. Output
  masks, neutrality and tick allocation are checked. This is an invariant/witness
  campaign, not an independent model of arbitrary macro/layer semantics; existing
  deterministic engine tests cover those contracts separately.
- **8,192 encoded USB reports**, each with nine reserved-bit corruptions, plus 8,192
  arbitrary byte slices: **90,112 decode calls**. Valid encodings must exactly recover
  expected axes, triggers, buttons and hat; corrupt reserved bits must be rejected.
- **8,192 mixed editor operations** across 32 nonzero seeds: referenced layer/macro
  renames, additions/removals, valid/invalid bindings and indices. Accepted transforms
  must validate; all inputs remain unchanged, including on rejection. Separate checks
  exercise UTF-16 name boundaries and **2,048 canonical binding/step draft pairs**.
- **128 headless composer frames** with invalid drafts, pointer/text events, changing
  viewport sizes, CPU tessellation and finite mesh-coordinate assertions. Successful
  Save-button activation/focus is not asserted, so this is not complete interactive
  workflow coverage. Direct draft checks do assert error retention and source integrity.

Extended runs were explicitly selected, never by running every ignored test:

```sh
cargo test -p scufpad-core --test engine --release --locked extended_seeded_campaign -- --ignored --nocapture
cargo test -p scufpad-desktop --release --locked large_profile_edit_stress -- --ignored --nocapture
cargo test -p scufpad-desktop --release --locked extended_headless_macro_composer -- --ignored --nocapture
```

The extended engine campaign passed **4,194,304 direct-model frames** (4,096 seeds)
and **524,288 randomized layer/macro frames** (128 seeds), plus setup/cleanup/deadline
witnesses. The extended desktop campaign passed **4,096 composer frames**, rejecting
2,760 invalid drafts without corrupting saved steps. The deliverable profile stress
passed 112 rename/edit pairs over 32 layers, 128 macros and 128 steps per macro.

## Release measurements

Host: AMD Ryzen 9 5900X, Linux x86-64 (CachyOS 7.3.1), Rust/Cargo 1.99.1,
LLVM 23.1.1, normal optimized Release settings—not JIT or AOT .NET. Timed runs were
serialized and pinned to logical CPU 2; the CPU was not reserved/exclusive, frequency
scaling/boost remained enabled, and scheduler/background-work uncertainty remains.
No framework/deployment defaults were changed.

The existing `performance_probe` first passed its twelve-workload preflight, then ran
seven warmed samples per scenario (1,000,000 operations each; compile case 100).
Allocation counts use 10,000 operations (compile case 100):

| Pure scenario | Median ns/op | Counted allocation calls/op |
| --- | ---: | ---: |
| USB decode | 12.1 | 0 |
| Idle tick | 107.0 | 0 |
| Idle, 32 layers | 144.1 | 0 |
| Passthrough edges | 115.2 | 0 |
| Two-key chord | 114.2 | 0 |
| Three-step macro | 125.5 | 0 |
| 256-step macro | 478.3 | 0 |
| 26 concurrent macros | 255.0 | 0 |
| Held layer | 123.2 | 0 |
| Trigger chord | 117.6 | 0 |
| State serialization | 487.4 | 5 |
| Large profile compilation | 85,362 | 2,115 |

Compilation and serialization allocate as expected; the input tick scenarios do not.
No before/after performance improvement is claimed. Synthetic warmed operations do
not measure physical USB, IPC, scheduling latency or whole-application CPU use.

Additional pinned Release diagnostics, repeated twice:

- Large deliverable profile: medians of seven sample means **1.915 and 1.923 ms per
  rename + binding edit + extra validation**, including new deliverability checks,
  at 16 pairs per sample (sample means 1.909–2.040 ms across repetitions). This is
  not a per-edit p99 measurement.
- Composer: CPU build/tessellation p50 **0.200–0.202 ms**, p95 **0.209–0.297 ms**,
  max **1.886–1.934 ms** across two 4,096-frame runs. It includes initial font/layout
  setup, not GPU presentation, event-to-display latency, the entire Studio window,
  or native file dialogs. The p95 variation illustrates the uncontrolled-host caveat.
- Final standalone owned test-process resource samples: edit stress **0.264 s wall,
  0.221 s CPU, 16,176 KiB peak RSS**; composer **0.915 s wall, 0.871 s CPU,
  15,976 KiB peak RSS**. These are short headless test processes, not running desktop
  idle CPU/RSS or evidence of absence of long-term leaks.

No timing thresholds were installed in CI: these measurements are diagnostics, not
portable nanosecond budgets. No observed sampled cost justified a production engine
optimization. Safe profile validation, macro budgets and deadlines were retained.

## Final verification and evidence

Coordinator verification: formatting, strict workspace/all-target Clippy, **121 Rust
checks passed in both debug and Release**, with four ignored tests (one live driver plus three opt-in stress
checks). All three non-live opt-in checks above were separately run and passed.
Optimized workspace build, desktop self-test and **64 Python checks** passed (43
installer, 7 workflows, 4 performance helpers, 2 private application, 1 private-bus
tray, 7 round-trip self-tests). The live driver was not run. GLib emitted non-failing
Python deprecation warnings during the tray check.

Expected red reproductions and initial test-authoring compile/lint failures were
resolved; no failed final check is being counted as passing. Xvfb/Weston and GNU time
were unavailable: no tool was installed. Headless egui is used instead of a native
window; process CPU/RSS uses Python's standard `resource` module in fresh subprocess
measurements instead of GNU time.

Raw logs, profile/edit timings, process-resource JSON, probe samples, compiler artifact
paths and metadata/source fingerprints are retained under:
`/tmp/opencode/scufpad-fuzz-20261029-1/`. See `performance-probe-pinned.json`,
`editor-stress-{pinned,final}.*`, `composer-stress-{pinned,final}.*`,
`extended-engine-final.log`, `tests-gate-{debug,release}.log` and `metadata-final.json`. Local evidence is not a hosted
Actions result; the branch/PR has not been updated by this task.

Open acceptance: coverage-guided/sanitized fuzzing, prolonged real desktop CPU/RSS,
GPU/Wayland/X11 rendering and file dialogs, physical controller/SDL mapping and actual
input latency. None is implied by these bounded runs. Live tests still require
explicit approval with games closed and owned device interception.
