# Rust history migration and reported-issue triage

## History and preservation

- Migration branch: `migration/rust-rewrite`; requested PR base: `release`.
- Original remote base: `2ca0975e9c388baa368c6a919dd9f90004609a8a`.
- The standalone Rust directory originally had no `.git` directory and no Rust commits
  to replay. Its existing application was committed as an imported Rust snapshot,
  directly on the original `release` tip. Original commit IDs and merge ancestry are
  unchanged; no orphan history, force-push or replacement of the remote branch is used.
- C# source is removed from the new tree, but remains in Git history. The separate
  C# checkout at `/home/chase/Coding/CSharp/Personal/Console/Scufpad` retains its
  uncommitted remapping/editor, reliability, installation and performance work. None
  of that dirty checkout was committed, reset, copied over or otherwise changed here.
- No installed application, saved user profile, device permission, physical grab or
  real virtual-controller output was changed during this migration.

## Issue #7 — X/Y swapped in SDL/Steam

**Reproduced in the Rust output mapping and fixed.** `output.rs` advertised Microsoft
`045e:0b12` but emitted logical X on `0x134` and Y on `0x133`. The Linux kernel aliases
are confusing: `BTN_X`/`BTN_NORTH` is `0x133`, while `BTN_Y`/`BTN_WEST` is `0x134`.
The report's xpad-compatible mapping is correct, regardless of the compass names.

`xbox_face_buttons_use_kernel_btn_x_and_btn_y_codes` failed before the fix with X
emitting decimal 308 rather than 307. It now passes for X and Y independently. The
shared capability/event table is corrected, so both normal forwarding and remapped
outputs use the same fix. The pre-existing incorrect X-code assertion is corrected too.
The existing virtual product ID/name are retained; renaming the controller is not needed
for this fix. Live Steam/SDL gamepad mapping and physical-button acceptance were not run.

## Issue #8 — 100% CPU with readable hidraw

**The reported unread-fd mechanism is absent in Rust; physical CPU reproduction is
not verified.** Rust needs hidraw for interface-03 USB report decoding. `Session::poll`
polls only that raw fd. On readiness, `Actor::run` reads up to 64 reports before polling
again. Even ignored `0x58` reports are read before being skipped. A nonblocking read
returns `None` on `WouldBlock`; errors disconnect and release the session. Grabbed evdev
fds are not added to the poll set. A positive poll timeout alone would not prevent the
reported defect if the fd were left readable; consuming reports is the relevant change.

`readable_raw_reports_are_consumed_and_poll_returns_to_idle` uses a private Unix
datagram pair with the real `Session::poll` and `Session::read` implementations. Both
`0x06` and ignored `0x58`-shaped reports make the fd readable, are consumed, reach
`WouldBlock`, and leave poll idle. It passes without any controller/uinput access.
This is helper-level coverage plus actor-path inspection, not an injected actor-loop
integration test or a whole-process hardware CPU measurement. No polling sleep or
speculative performance change was added. Leave live CPU confirmation open until
approved testing with the owned USB controller and games closed is available.

Do not apply the old workaround of removing hidraw access to the Rust application.

## Issue #9 — world-writable device rules

**Confirmed in original `release` documentation; corrected for Rust.** The old README
repeated world-writable input/hidraw/uinput rules under a late `99-scuf.rules` filename.
The relocated Rust README did not contain those examples, but also did not supply
replacement permission instructions.

The current README documents `70-scufpad.rules`, ordered before `73-seat-late.rules`,
with active-seat `uaccess` ACLs and a static uinput node. Controller access is restricted
by VID/PID. Hidraw remains necessary for Rust's active decoder. The guide warns about
uinput's keyboard-injection capability, removal of old permissive rules, normal-user
execution, and the local-login/systemd-logind requirement. No installer changes udev.
`test_udev_instructions_use_seat_acl_for_input_hidraw_and_uinput` checks these examples.
No real ACLs or kernel rules were changed or tested in this session.

## Rust CI and release publication

- Read-only validation covers formatting, strict Clippy, tests, optimized applications,
  desktop self-test, private no-hardware IPC/profile and tray checks, installation tools,
  release policy/version checks, and the round-trip harness's **non-live** self-test.
- CI builds a native Rust bundle with both applications and the existing checked
  per-user installer, smoke-tests its binaries, and uploads the tarball plus SHA-256.
- Only pushes to `release` invoke publication, after the same reusable CI workflow.
  Only that publication job receives `contents: write`; it downloads artifacts from
  its own run, never from an untrusted PR run. Checkout credentials are not persisted.
- Versions continue the semantic patch sequence, including reserved draft versions.
  An existing legacy `v1.0.0` draft has no Git tag; it is left untouched. Retries reuse
  the current commit's tag/draft/release instead of allocating duplicate releases.
  Mocked cases cover both partial-publication and completed-publication retry selection.
- Publication is serialized, not guaranteed FIFO: GitHub can coalesce pending runs.
  The archive is dynamically linked and built on Ubuntu 24.04, not universally portable.
- The migration source exception is restricted to the same repository and exact branch
  name. Remove it after merge; move `dev` forward to the merged Rust ancestry before
  resuming ordinary `dev` → `release` PRs. No remote ruleset or protected branch changed.
- Retired .NET Qodana, the old automatic branch-deletion job (post-merge ref-loss race),
  and the legacy PR bot (`GITHUB_TOKEN` creation suppresses normal PR checks). Use
  manual release PRs and branch deletion; automation can return with an approved App
  token and a safe ownership policy. No new secrets or dependencies are required.
- Repository rulesets currently require review, verified signatures, CodeQL and code
  quality. Cargo/Clippy passing does not satisfy those remote rules automatically.
  This task opens a PR; it does not bypass rules, merge, publish a release, or install
  the new application. Hosted Actions and actual publication remain separate evidence.

## Local verification

Toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, Cargo 1.99.1; Linux x86-64.

Passed:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --workspace --release --locked
python3 tests/installation_tools_test.py
python3 tests/workflows_test.py
python3 tests/performance_review_test.py
python3 tests/rust_application_test.py
/usr/bin/python3 tests/rust_tray_test.py
python3 tests/rust_roundtrip_test.py --self-test
./target/release/scufpad-editor --self-test
```

Results: **110 Rust tests passed**, one live round-trip test intentionally ignored;
**64 Python tests passed** (43 installer, 7 workflow, 4 performance helper, 2 application,
1 private-bus tray, 7 non-live round-trip harness). GLib emitted Python deprecation
warnings during the passing tray check. Expected installer failure-path diagnostics
are not failed tests. Workflow YAML parsing and `git diff --check` also passed;
`actionlint` was unavailable, so its validation is not claimed.

An actual `--runtime rust` package also passed: six regular archive members, native
x86-64 ELF bridge/editor, Rust manifest and all payload hashes checked. The two native
application tests passed again against packaged binaries. A real isolated per-user
install, installed editor self-test, and uninstall passed without touching the normal
installation. Bundle and archive evidence: `/tmp/opencode/scufpad-rust-migration-20261029/`.
Archive SHA-256: `fb61cb2dbfd4323469d89ec302a9cf47cf3be3888608bb4c6822390e6101aec3`.

Hardware-free tests do not establish physical input latency, whole-process CPU with
hidraw, game/SDL mapping, or real logind ACL behavior. Live controller/output checks
remain gated on explicit games-closed approval and owned devices.
