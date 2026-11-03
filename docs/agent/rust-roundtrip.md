# Synthetic controller round-trip harness

| Stage | Checkable result | Status |
| --- | --- | --- |
| Trace and design | Safe injection and independent kernel/application readers | completed |
| Implement | Isolated harness, explicit opt-in and bounded cleanup | completed |
| Verify | Hardware-free regressions, approved live round-trip, final build | completed |
| Deliver | Usage, evidence and hardware/game limits | completed |

The user approved a temporary virtual-controller test with games closed. The harness
must not discover, open or grab any physical controller. It must use private state,
identify only its own output device, neutralize it and remove it on completion, and
avoid modifying profiles or a running bridge. An evdev observation alone does not prove
delivery to an SDL application or game. Game behavior remains outside this scope.

The test-only driver replays valid 16-byte reports through the actor's real frame and
output path, without starting discovery. It finds its uinput device by the kernel
sysname returned from its own file descriptor. An independent evdev reader verifies
press and release frames; an SDL3 consumer receives only that event node in a private
user/mount namespace. No injection endpoint is exposed by the production daemon.

## Use and observed result

Run `python3 tests/rust_roundtrip_test.py --self-test` for hardware-free checks.
With games closed and explicit permission to create a virtual controller, run
`python3 tests/rust_roundtrip_test.py --run-live --games-closed`. This requires
writable `/dev/uinput`, permission to read the newly created evdev node, SDL3,
and user and mount namespaces that permit bind mounts. The driver rejects
`SCUFPAD_NO_HARDWARE=1`; there is no host-device fallback.

On 2026-10-28, the live run observed BTN_SOUTH press and release in separate
evdev SYN_REPORT frames and button 0 press and release in SDL3. It confirmed
removal of its exact `input607` device; a subsequent run removed `input609`.
An injected SDL startup failure also
confirmed cleanup of its exact `input608` device. Device numbers vary per run.
The check covers SDL joystick state, not SDL gamepad mapping, actual games, or
physical USB input. Cleanup attempts neutralization even on failure; forced
termination cannot prove that a neutral frame was delivered.

Final checks passed: harness self-tests, workspace formatting and strict Clippy,
all workspace Rust tests, installer/application/performance/tray Python tests,
the release workspace build, and the editor self-test. The live driver remains
ignored by default. The tray check emitted GLib Python deprecation warnings.

## Input-latency scope

The test driver decodes synthetic reports and invokes the actor's frame path.
The A-button workload exercises gamepad passthrough, not custom remapping.
It excludes physical switch and USB transport, physical-device discovery, the
daemon's connected polling loop, game logic, rendering, and display latency.

To measure this path, run `python3 tests/rust_roundtrip_test.py --run-live
--games-closed --profile` with games closed. The driver uses the optimized Rust
test profile. Each run validates 20 warm-up and 200 measured press/release
pairs through both readers. The parent timestamps immediately before writing
the command. The kernel result uses the owned evdev reader's monotonic input
packet timestamp; the pre-armed SDL reader timestamps its observed state.
This includes command-pipe and scheduling overhead. Kernel timestamps have
microsecond precision; SDL polling and scheduling add measurement delay.
Percentiles use nearest rank, with only 200 samples per direction per run.

On 2026-10-28, three runs on an AMD Ryzen 9 5900X, Linux 7.2.11-1-cachyos,
Rust 1.99.1 and SDL 3.4.18 passed all 1,200 measured transitions and removed
their owned devices. The CPU governor was `powersave`. Across the three runs
and both directions, per-run median command-to-kernel times were 4.7–5.4 µs;
per-run p95 values were 7.0–10.8 µs. With a 50 µs requested SDL polling sleep,
per-run median command-to-SDL-observation times were 117–119 µs and p95 values
were 124–135 µs. A separate 500 µs polling run had SDL medians around 571 µs,
showing the strong polling bias. These are harness measurements, not game or
physical-controller latency. Full per-run logs are under
`/tmp/opencode/scufpad-roundtrip/latency-*.log` on the test host.
An injected failure immediately after a press also confirmed removal of the
owned device; it does not prove delivery of the cleanup neutralization frame.
