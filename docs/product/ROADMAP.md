# Approved roadmap

## Full Rust port — approved 2026-10-27

1. Preserve the current profile/wire formats and verified USB behavior in a shared
   Rust core, with regression coverage for decoding, validation, layers and macros.
2. Port the Linux CLI/service, owned native input/output, hardware gates, persistence
   and local control. Keep background lifetime independent of the editor.
3. Port the desktop editor, including diagram, binding/layer/macro editing, native
   import/export, setup, telemetry, automatic startup and status.
4. Integrate native packaging, review the port, run software/isolated runtime checks,
   and document hardware/cutover verification. Keep the existing installation and
   C# reference until replacement is verified; do not claim unrun live-output parity.

Execution and evidence: [Rust port](../agent/rust-port.md). Wireless remains a separate
feature; no release, remote publication or automatic system-wide installation is approved.
