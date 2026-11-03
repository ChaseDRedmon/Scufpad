# Bridge status icon

Approved scope: stop the old Rust bridge and add a KDE/system-tray status icon with
bridge controls. The verified old bridge PID 2192957 acknowledged Shutdown, exited,
and removed `/tmp/CoreFxPipe_scufpad-chase`. No forced kill or hardware restart occurred.

| Stage | Checkable result | Status |
| --- | --- | --- |
| Tray implementation | Bridge-owned status icon, truthful state and safe menu actions | completed |
| Review and regression coverage | State/action/lifecycle checks and dependency review | completed |
| Final verification | Tests, lint/format/build and isolated NoHardware tray checks | completed |
| Delivery | Current usage, shutdown evidence and explicit live-hardware boundary | completed |

Contract:

- Use KDE's StatusNotifierItem protocol (the desktop is KDE Wayland with a live watcher).
  The bridge owns the icon, so closing the editor does not remove it. Shutdown removes
  the icon; paused outputs and disconnected hardware remain visible while the bridge runs.
- Show forwarding, waiting/disconnected and outputs-stopped states with text/tooltips,
  not color alone. Do not infer custom-profile activity from verification alone.
- Offer Open editor, Resume outputs, Stop All outputs and Stop bridge. Mutating actions
  use the existing bounded actor command path, deadlines and shutdown neutralization.
  A tray-launched editor attaches without automatically starting a missing bridge, even
  if shutdown races with its launch. Its explicit Start button remains available.
- Keep tray work off the input owner. Update only changed presentation at a bounded,
  low cadence; do not subscribe to raw input or repaint for every axis/button sample.
- A missing session bus/tray host is nonfatal. No login autostart, permissions, firmware,
  native GTK dependency or installed-application replacement is authorized.
- Stop All and Stop bridge must remain deliverable when ordinary tray actions fill the
  queue. A Stop All invalidates earlier queued Resume requests; a later explicit Resume
  remains valid. A command already being dispatched can finish before the stop request
  reaches the actor; an accepted shutdown prevents later queued actions. No tray action
  may block a D-Bus callback.
- Routine NoHardware runs must not publish icons to the user's desktop. Validate any
  real tray operations only with private NoHardware processes/endpoints, never by
  starting/arming the live controller bridge while a game may be open.

## Implementation and verification

The bridge owns a StatusNotifierItem with distinct themed icons, text and tooltip for
forwarding, stopped/inactive and disconnected states. The menu offers editor launch,
Resume, Stop All and bridge shutdown. Editor launch supports workspace and installed
layouts, passes the actual bridge path, and suppresses automatic bridge startup.
Status projection runs off the input actor every 500 ms and publishes only changes.
Stop and shutdown requests bypass the bounded ordinary-action queue; queued earlier
Resume requests are invalidated. Errors remain visible in the bridge log. Tray failure
does not stop the bridge, and actor-owned outputs are destroyed before tray shutdown.

`ksni` 0.3.7 uses its async-io backend without GTK. The lockfile pins UUID 1.18.1 for
the workspace's Rust 1.88 baseline. A Rust 1.88 bridge check passes. Independent review
found and prompted a fix for dropped stop actions under queue saturation. One action
already being dispatched can precede a later stop request; accepted shutdown ends the
tray worker before any further queued action executes.

Final gate: **104 Rust tests and 50 Python checks pass**, strict Clippy and formatting
pass, and the final optimized workspace build passes. Private D-Bus integration checks
registration, tooltip updates, menu actions, editor attach-only launch, NoHardware
isolation, missing-bus behavior and shutdown cleanup. The initial integration attempt
found a test selector matching both a status line and Resume; exact menu-label matching
fixed it, and the complete gate passed. Logs: `/tmp/opencode/scufpad-tray/`.

The old bridge remains stopped. Start the rebuilt editor to launch the new bridge:

```sh
cd /home/chase/Coding/Rust/Scufpad
./target/release/scufpad-editor
```

The icon requires a compatible tray host. Actual icon visibility and live controller or
game delivery have not been tested. No controller was grabbed or live game started.
