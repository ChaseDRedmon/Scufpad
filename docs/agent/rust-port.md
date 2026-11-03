# Full Rust application port

This is now the independent Rust workspace at `/home/chase/Coding/Rust/Scufpad`.
The .NET comparison source remains in its original C# directory and is not required
to build, test or package this workspace.

## Workspace relocation

Requested destination: `/home/chase/Coding/Rust/Scufpad`. Preserve the .NET repository,
installed application and profiles; move the Cargo workspace and Rust-only checks,
and include independent copies of shared fixtures, installer assets and documentation.
Do not initialize or publish a new Git repository without approval.

| Relocation stage | Checkable output | Status |
| --- | --- | --- |
| Dependency inventory | Destination/instruction checks and standalone file set | completed |
| Move and references | Rust workspace moved; shared support present; old references corrected | completed |
| Verification | Independent tests/lints/build/package and preservation checks | completed |

Relocation result: Cargo manifests/lockfile, toolchain pin, all three crates, Rust-only
runtime checks and build artifacts moved. Shared license, installer/tests, USB fixtures,
icon and records were copied so both projects remain intact. All 31 moved source/config
files retain their hashes; all 67 .NET source/project/XAML files remain unchanged.
From the new directory, 86 Rust tests, 43 installer checks, two native application
checks, strict Clippy, formatting, Release builds and native bundle packaging pass.
The source reference's four workflow and 43 installer checks also pass. The OpenCode
session now uses the Rust directory. No Git repository, remote or installation was created.

Approved outcome: move the desktop editor, CLI/background service and shared remapping
logic to Rust. Preserve the verified USB behavior, existing profile format, finite
macros/layers, output safety, independent desktop/backend lifetimes and user data.
The installed .NET application remains intact until the Rust replacement is verified.
Wireless remains a separate feature; this port does not change firmware or permissions.

| Stage | Checkable output | Status |
| --- | --- | --- |
| Parity contract | Existing behavior inventory, Rust architecture and runnable check contract | completed |
| Rust implementation | Shared core, Linux CLI/service and native desktop editor | completed |
| Integration and review | Packaging/launch wiring, regression cases and reconciled review | completed |
| Final verification | Cargo checks, Release binaries, isolated runtime/UI smoke and delivery record | completed |

Do not enable virtual outputs, grab physical devices, stop the user's running bridge,
replace the installed app, delete the C# reference or publish remotely without the
corresponding verification/approval. No performance improvement is assumed from a port.

## Architecture and parity contract

- Cargo workspace: `scufpad-core`, `scufpad` (CLI/daemon) and `scufpad-desktop`
  (`scufpad-editor`). Native egui/eframe desktop; no .NET host or web wrapper.
- Preserve version-1 camelCase profiles and PascalCase enum values, all binding kinds,
  trigger hysteresis, press-time ownership, held/toggled layers, 16-key chords,
  256-step finite macros and the 60-second activation bound.
- Preserve strict interface-03 report-06 USB decoding, all six alias-free checks,
  same-controller evdev ownership, neutral shutdown, disconnect handling and Profile
  disarming. No outputs until explicit Arm plus verified hardware readiness.
- Preserve bounded little-endian length-prefixed JSON over same-user Unix IPC and
  the existing environment overrides/profile paths. Single daemon owner; validate
  peer identity and refuse an active endpoint rather than unlinking it.
- Desktop: live front/rear schematic, one-step setup, profile library/import/export,
  binding/layer/macro editing, draft preservation, telemetry, automatic bridge launch,
  actual running status, manual Stop/retry and independent editor/backend lifetime.
- Validate with captured USB fixtures, core regression cases, isolated IPC/profile
  tests and no-hardware desktop/service smoke. Live output parity requires separate
  approval with games closed and owned interception; no such activation is implied.
- Keep old source and installation untouched during the port. Add Rust bundle support
  and clear cutover instructions rather than silently replacing a running service.

Implementation has three non-overlapping owners: shared core/contracts, Linux service,
and desktop. The coordinator owns Cargo workspace integration, packaging, documentation
and final verification. Native Rust/egui and platform dependencies are available;
the initial Rust sources are not evidence of behavioral parity until checked below.

## Implementation/review checkpoint

- All three native Rust crates compile. Authentication failures interrupted three
  agents after they wrote their source files; coordinator integration continued from
  those files, and bounded review retries succeeded. No model substitution was used.
- Independent core review found no material decoder/engine parity issue. Profile writes
  now perform all fallible preparation before atomic replacement, require a private
  same-UID parent, and use no-replace creation for new files. Existing-file replacement
  assumes no concurrent same-UID writer; parent-directory crash durability is not claimed.
- Core Unix connect is nonblocking/deadline-bounded, including full-backlog failure.
  Independent follow-up accepts the repaired storage/connect behavior under that boundary.
- Independent native review found a missing uinput FD identity gate. It now checks
  character-device type and kernel sysfs major/minor before configuration, then requires
  the supported setup API. Pure tests reject regular files and unrelated character devices.
  The reviewer confirms the repair. No real uinput node was opened for these tests.
- Independent desktop review found no confirmed medium/high defect. It covers draft
  guards, reference updates, asynchronous file operations, pending-Arm cancellation,
  startup, explicit Stop and editor/backend lifetime. Runtime coverage is recorded below.
- Native Rust bundle support reuses the checked installer and preserves legacy .NET
  bundle support. All 43 installer cases pass, including native round trips and failed
  Cargo-build retention. The current user installation has not been changed.

## Final software verification

- Rust **1.93.1**: `cargo check --workspace --all-targets --locked`, formatting,
  strict Clippy (`--workspace --all-targets -- -D warnings`), all **86 Rust tests**
  and optimized workspace builds pass. Both normal Release paths and explicit Linux
  x86-64 target paths were built. No warnings were suppressed to pass the lint gate.
- The Rust tests include 38 captured USB reports, engine/ownership/layer/macro cases,
  profile/IPC limits, private storage, native descriptor and write checks, readiness,
  disconnect/Profile behavior, singleton/socket safety and desktop draft/edit helpers.
- **128 Python checks pass:** 43 installation, 4 workflow, 7 performance tools,
  23 capture, 49 daemon measurement and 2 native application checks.
- The native application checks run real Rust processes with private endpoints and
  `SCUFPAD_NO_HARDWARE=1`. They verify GetState/GetProfile, guarded Arm, ApplyProfile,
  private persistence, invalid-profile preservation, malformed/oversized frames,
  duplicate-daemon refusal, StopAll, Shutdown, SIGTERM and display-free editor self-test.
  They assert that no input, hidraw or uinput descriptors are open and outputs stay zero.
- A real egui window opened from an isolated installed wrapper with the Rust bridge.
  Its maps contain no libcoreclr. Normal editor close leaves the daemon alive; a second
  editor reuses the same daemon PID. Both owned sessions shut down cleanly. The final
  front-view screenshot verifies the status, setup, workspace, inspector and diagram;
  an observed footer/legend collision was fixed and recaptured.
- Native file-picker interaction is not separately claimed. Synthetic X11 input did
  not confirm the rear-tab transition, so its attempted capture is labeled accordingly.
  The rear layout and edit/lifecycle paths are compiled, source-reviewed and covered
  by the relevant pure helper checks, not claimed as complete interactive acceptance.
- Final native archive: **six checked regular members**, about **6.38 MiB**, at
  `/tmp/opencode/scufpad-rust-port/scufpad-rust-linux-x64-final.tar.gz`.
  Both executables are native x86-64 ELF; the five payload files and archive hashes agree.
  The bundle is `/tmp/opencode/scufpad-rust-port/bundle-final/`. This artifact size is
  not a runtime-performance or universal Linux compatibility claim.
- Real isolated install/uninstall passes, including a preserved profile sentinel.
  The installed Rust editor self-test passes with no usable system .NET runtime.
  Removed the isolated prefix, sentinel and named private runtime locks/directories.
  Retained binaries, package, logs, screenshots and the user-data hash baseline.
- The actual existing .NET installation, command wrappers, menu entry and saved JSON
  match their original hashes. No user installation, firmware, permissions, login
  startup or remote repository state was changed. All owned Rust test runtimes are stopped.
- Code-only graph refresh: **2468 nodes, 5631 edges, 123 communities**. A real query
  finds the Rust decoder. Cargo manifest extraction, AXAML skips and historical graph
  identity collisions limit completeness; compiled source and runtime checks are authoritative.

The software-port stages are complete. This does **not** certify live controller/output
parity or a production cutover. No physical device grabs or virtual output activation
were performed during this port. Existing .NET sources remain a comparison/recovery
reference; no Rust product binary requires them or launches a .NET runtime.

## Run and cutover

From this checkout:

```sh
cargo build --workspace --release --locked
./target/release/scufpad-editor
./target/release/scufpad --status
```

For a hardware-free preview, use a unique endpoint and private configuration directory
for both programs, plus `SCUFPAD_NO_HARDWARE=1`. Never accidentally attach a validation
run to the normal user endpoint. The automated native checks provide a ready example:

```sh
python3 tests/rust_application_test.py
```

Before replacing the current installation, obtain approval for games-closed, owned,
intercepted live USB/output validation. Check all six extras, gamepad/keyboard/mouse,
finite and held macros, layers, StopAll, firmware Profile, disconnect/reconnect and
shutdown neutralization. Native file dialogs and full interactive editor acceptance
remain separate manual checks. No Rust speed or memory improvement is claimed.

Once validated and explicitly approved for cutover, close the old editor and stop its
bridge, uninstall its managed application files (profiles remain), then install the new
native bundle. Do not run competing hardware bridges. The old `--legacy` evdev-only
mode remains in the .NET reference; the Rust service implements the verified raw-USB
remapping path rather than recreating that obsolete mapper. Wireless remains unimplemented.
