# Scufpad (Rust)

A Linux controller bridge and native desktop editor for the SCUF Envision Pro V2.
The workspace contains `scufpad-core`, the `scufpad` CLI/background service and the
`scufpad-editor` egui desktop. These applications do not require .NET or the C# checkout.

## Build and run

Requirements: Linux x86-64, the pinned Rust 1.99.1 toolchain, and native X11/Wayland
and OpenGL libraries. Python 3.11+ is needed only for installation and runtime checks.

```sh
cargo build --workspace --release --locked
./target/release/scufpad-editor
```

The editor starts or attaches to the bridge and shows its actual running status.
Closing the editor leaves the bridge running. Normal gamepad forwarding starts
automatically; no paddle/SAX setup or Enable click is needed for ordinary gamepad use.
The saved custom profile, including keyboard/mouse actions and macros, activates after
all six paddle/SAX inputs verify independently. Until then, only normal gamepad inputs
forward. Loss of verification returns to normal forwarding without removing the virtual
gamepad. USB is supported; wireless is not implemented. Do not run competing bridges.

Stop All removes every virtual output and remains stopped across reports, reconnects
and profile application. Use **Resume outputs** or `scufpad --arm` to resume. Applying a
profile releases the old custom actions but preserves the current running/stopped intent.

On desktops with a StatusNotifier tray, the running bridge shows its connection and
output state. The icon stays when the editor closes. Its menu opens the editor, resumes
outputs, stops all outputs, or stops the bridge. A missing tray host does not stop the
bridge; use the editor or CLI instead. Hardware-free previews do not show a tray icon.
An editor opened from the tray attaches without automatically starting a stopped bridge.

For a hardware-free preview isolated from normal profiles and the installed service:

```sh
SCUFPAD_NO_HARDWARE=1 SCUFPAD_PIPE_NAME=scufpad-rust-preview \
SCUFPAD_CONFIG_HOME="$HOME/.cache/scufpad/rust-preview/config" \
XDG_DATA_HOME="$HOME/.cache/scufpad/rust-preview/data" \
./target/release/scufpad-editor

# Stop that preview's independent background service afterward.
SCUFPAD_PIPE_NAME=scufpad-rust-preview ./target/release/scufpad --stop
```

## Controller permissions (Linux desktop)

Run Scufpad as your normal user, never as root. On a systemd-logind desktop, an
administrator can put these rules in `/etc/udev/rules.d/70-scufpad.rules`:

```udev
SUBSYSTEM=="input", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", TAG+="uaccess"
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
```

The filename must sort **before `73-seat-late.rules`**, not `99-...`, so logind
can apply ACLs to all controller interfaces. These rules grant access to the active
local seat user rather than every local process. `/dev/uinput` can synthesize keyboard
input; do not make it world-writable. Replace any older permissive Scufpad rules rather
than leaving both installed. The installer does not change device permissions.

After saving the rules, load uinput and reload/reapply the rules with games closed
and the bridge stopped:

```sh
sudo modprobe uinput
sudo udevadm control --reload-rules
sudo udevadm trigger --action=add --subsystem-match=misc --sysname-match=uinput
```

Unplug and reconnect the controller, then restart Scufpad as your normal user.
The Rust bridge **requires hidraw** for its verified USB decoder and consumes those
reports; the old C# workaround of removing hidraw access does not apply. It also needs
read access to all six controller evdev interfaces and write access to `/dev/uinput`.
`uaccess` is for an active local login session, not a headless/system service. On
systems without systemd-logind, use your distribution's scoped ACL/group mechanism;
do not grant global input access. No live permission changes are performed by tests.

## CLI

```sh
./target/release/scufpad --help
./target/release/scufpad --daemon
./target/release/scufpad --status
./target/release/scufpad --stop
```

Profiles, keyboard/mouse bindings, finite macros, held/toggled layers, setup and output
telemetry are shared between the CLI/service and desktop. Version-1 camelCase JSON and
PascalCase enum names remain compatible with the reference implementation.

## Verification

```sh
mkdir -p /tmp/opencode
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --workspace --release --locked
./target/release/scufpad-editor --self-test
python3 tests/installation_tools_test.py
python3 tests/workflows_test.py
python3 tests/rust_application_test.py
python3 tests/performance_review_test.py
```

For the isolated tray integration check, install `dbus-run-session` and the system
Python D-Bus and GLib bindings, then run `/usr/bin/python3 tests/rust_tray_test.py`.
The check uses a private session bus and does not access controller hardware.

### Hardware-free randomized checks

The normal Rust test suite includes fixed-seed mapping/ownership, USB-report mutation,
editor reference/draft, and headless macro-composer checks. Longer campaigns are opt-in:

```sh
cargo test -p scufpad-core --test engine --release --locked extended_seeded_campaign -- --ignored
cargo test -p scufpad-desktop --release --locked large_profile_edit_stress -- --ignored --nocapture
cargo test -p scufpad-desktop --release --locked extended_headless_macro_composer -- --ignored --nocapture
cargo run -p scufpad-core --example performance_probe --release --locked -- --preflight
cargo run -p scufpad-core --example performance_probe --release --locked
```

These named checks do not create a native window, start the normal bridge, read user
profiles, or access controller/uinput nodes. Do not run all ignored tests indiscriminately:
the separate live driver below requires explicit permission. Seeded checks are not
coverage-guided fuzzing or proof that all mappings/performance paths are defect-free.
Findings, measurements and limits: [Rust fuzz/stress report](docs/agent/rust-fuzz.md).

Workspaces must fit the existing 1 MiB framed bridge limit as well as structural
layer/macro bounds. Oversized edits are rejected without replacing the current workspace.
Near the frame limit, `GetProfile` may omit optional telemetry; use `GetState` for it.

The optional virtual-controller round-trip checks evdev and an independent SDL3
joystick reader. With all games closed and explicit permission to create a temporary
virtual controller, run:

```sh
python3 tests/rust_roundtrip_test.py --self-test
python3 tests/rust_roundtrip_test.py --run-live --games-closed
python3 tests/rust_roundtrip_test.py --run-live --games-closed --profile
```

The live check requires writable `/dev/uinput`, permission to read its event node,
SDL3, and user and mount namespaces with bind mounts enabled. It isolates the SDL
reader from host input devices and removes its virtual device afterward. It does not
test a physical USB controller, SDL gamepad mapping, or a game. The live driver also
refuses to run when `SCUFPAD_NO_HARDWARE=1`.
The profile mode measures optimized synthetic A-button passthrough through the
kernel and an SDL joystick reader. Its timings include harness scheduling and SDL
polling; they do not measure physical USB, daemon polling, custom mappings, or games.

Runtime checks use private endpoints/configuration and explicitly disable hardware.
Live USB/output parity and complete interactive/file-picker acceptance are not claimed.
Obtain approval with games closed and owned interception before live output testing.
The installed .NET app and user profiles were not replaced during development or relocation.

## Per-user package and menu entry

```sh
python3 tools/install_linux.py package --runtime rust \
  --output "$HOME/.cache/scufpad/bundle-rust" \
  --archive "$HOME/.cache/scufpad/scufpad-rust-linux-x64.tar.gz"
python3 "$HOME/.cache/scufpad/bundle-rust/install.py" install
```

Use an unused output directory. Run as your normal user, not root. The installer refuses
existing installation targets; it never silently replaces a running or modified app.
It installs commands under `~/.local/bin`, application files under `~/.local/lib/scufpad`,
and a menu entry under `$XDG_DATA_HOME/applications`. Native Linux libraries and current
controller permissions remain necessary. No udev, login autostart or systemd changes occur.

Close the editor and stop the bridge before uninstalling:

```sh
python3 "$HOME/.local/lib/scufpad/install.py" uninstall
```

Profiles are preserved. A custom `--prefix /absolute/path` supports isolated installation.
The installer retains legacy bundle compatibility, but native packaging uses `--runtime rust`;
no .NET source or SDK is required for that path.

## CI and releases

Pull requests build and test Rust, then upload a checked Linux x86-64 archive containing
both the bridge and editor plus the per-user installer. A push to `release` runs the
same validation and packaged-binary smoke tests before publishing that archive and its
SHA-256 checksum to a GitHub Release. Publication does not run on pull requests. Release
tags continue the existing `vMAJOR.MINOR.PATCH` sequence with a patch increment,
reserving existing drafts and reusing the same commit's release on retries. Concurrent
publication is serialized; GitHub may coalesce pending runs after multiple rapid pushes.
Builds use Ubuntu 24.04 and the pinned toolchain; they are not static or universal Linux
packages. Older glibc/native-library compatibility needs separate validation.

The migration branch preserves the original committed C# history unchanged. The C#
implementation remains available in that history, not in the current source tree.
The one-time `migration/rust-rewrite` exception allows this PR to target `release`;
normal release PRs still come from `dev`. Remove the exception after migration and
bring `dev` forward to the merged Rust history before resuming normal releases.
Create normal `dev` → `release` PRs manually. The legacy automatic PR bot was retired:
PRs created with `GITHUB_TOKEN` do not trigger ordinary PR checks; restore automation
only with an explicitly configured GitHub App token. The old branch-deletion workflow
was also retired to avoid deleting branches advanced after merge. Delete merged branches
manually after checking their tip. .NET-only Qodana was replaced by Rust Clippy checks.
See [issue triage and verification limits](docs/agent/rust-migration.md).

## Project records

- [Approved scope](docs/product/VISION.md) and [roadmap](docs/product/ROADMAP.md).
- [Port verification and cutover boundary](docs/agent/rust-port.md).
- [Current relocation evidence](docs/agent/progress.md).
- [Full Rust performance review and measurement limits](docs/agent/rust-performance.md).
- [Automatic forwarding, custom-input verification and restart](docs/agent/rust-startup.md).
- [Rust 1.99.1 release notes and application review](docs/agent/rust-1.99-review.md).

For the original C# source, browse the parent history before the Rust migration.
A separate local .NET comparison/recovery checkout was preserved during development;
it is not a build or runtime dependency.
