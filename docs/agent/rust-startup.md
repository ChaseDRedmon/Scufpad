# Automatic gamepad forwarding

Approved behavior: start normal gamepad forwarding automatically. Do not require the
six paddle/SAX checks for ordinary controller use. Keep the checks and ongoing alias
monitoring before running the saved custom profile. Once verification completes, the
profile activates automatically. No persistent verification cache is introduced.

| Stage | Checkable result | Status |
| --- | --- | --- |
| Implementation | Automatic passthrough, gated custom routing, accurate CLI/editor status | completed |
| Review and regressions | Safety review and deterministic routing/lifecycle checks without hardware | completed |
| Final checks | Rust/Python tests, strict lint/format and optimized builds | completed |
| Delivery | Updated usage, preservation evidence and explicit restart/live-test boundary | completed |

Acceptance and safety invariants:

- Normal buttons, sticks, D-pad and analog triggers forward without verification.
  G keys, paddles/SAX and Hardware Profile do not become unverified host mappings.
- Saved custom mappings, keyboard/mouse actions, macros and layers stay inactive until
  all six controls verify. Transitioning modes cancels custom ownership/macros and
  suppresses held inputs before activation.
- Losing verification or changing onboard profile falls back to ordinary forwarding,
  releases custom outputs, and keeps the same virtual gamepad alive.
- Startup requests outputs automatically. Stop All and shutdown still remove every
  owned output; subsequent reports/reconnect/Apply must not silently undo Stop All.
  Enable resumes output. Apply preserves forwarding intent and releases old mappings.
- NoHardware still performs no input discovery, grabs, uinput creation or native output.
  Profile formats, USB identity/ownership checks and independent editor lifetime remain.
- Do not stop, restart, arm or profile the user's running bridge/game. All runtime
  checks use private NoHardware endpoints; live controller/game acceptance remains open.

## Implementation and verification

- The actor requests output at hardware-enabled startup. Unverified input routes through
  an allocation-free normal-gamepad helper, not the saved custom engine. The helper
  reuses the existing normal control mapping and excludes paddles/SAX, G keys and
  Hardware Profile. Sticks and analog triggers remain unchanged before native filtering.
- Native output starts with only the virtual gamepad. Verified custom mode adds the
  keyboard/mouse pair transactionally. Falling back drops/neutralizes that pair while
  retaining the gamepad descriptor; custom gamepad owners, macros and layers cancel.
- Mode activation and profile replacement suppress held bindings. Apply keeps existing
  output intent: running continues, while Stop All remains stopped until Resume/Arm.
  No profile or wire schema was changed. Existing conservative alias checks remain.
- CLI/editor labels now distinguish active gamepad output from custom-input verification.
  The product vision and README record the approved replacement for the old startup gate.
- Independent source review found no high/medium issue. Tests cover normal identities,
  excluded extra controls, zero allocations, automatic startup intent, automatic custom
  activation, held-input suppression, alias/profile fallback, macro/layer/key release,
  profile/Stop All/reconnect behavior, and stable gamepad identity with regular-file
  event sinks. The latter does not claim live uinput acceptance.

Final checks: **98 Rust tests and 49 Python checks pass**, formatting and strict
all-target Clippy pass, optimized native workspace builds pass. NoHardware application
tests still confirm no input/uinput access. A pre-existing overflow regression exposed
a timing race: rejection can occur before the client's write. The test now accepts only
the expected reset/broken-pipe rejection during write or read; queue-capacity assertions
are unchanged. Five focused repetitions and the complete final suite pass.

Evidence: `/tmp/opencode/scufpad-auto-forward/`, including the first failed test log,
focused repetitions, final gate logs and fresh preservation baseline. Profile JSON,
the 67-file .NET reference and Cargo configuration/lockfiles are preserved. The local
code graph now has 846 nodes, 2033 edges and 26 communities; root Cargo manifest
extraction remains degraded.

## Use the new binary

The running user bridge/editor/game were not stopped, armed, restarted or profiled.
After closing the game and editor, restart the old bridge explicitly:

```sh
cd /home/chase/Coding/Rust/Scufpad
./target/release/scufpad --stop
./target/release/scufpad-editor
```

Normal gamepad output should start without manual verification or an Enable click.
Custom mappings still need all six controls verified on each new connection/session.
Live controller/Steam/Proton/Helldivers validation remains unrun; do not infer it from
software tests. Existing native device identities, permissions and launch options were
not changed. Reopening the editor does not bypass an explicit Stop All.
