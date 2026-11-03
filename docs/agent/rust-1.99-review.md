# Rust 1.99.1 update and application review

| Stage | Result | Status |
| --- | --- | --- |
| Release and application review | Official release evidence and full Rust application sweep | completed |
| Upgrade | Pinned toolchain, CI and current usage updated; justified feature changes | completed |
| Verification | Formatting, strict Clippy, tests and optimized workspace rebuild | completed |
| Delivery | Findings, risks, release notes and validation evidence | completed |

## Official release notes

Rust 1.99.1 was released on 2026-10-15. It fixes an x86-64 FFI `bool` return
miscompilation, a missing `wcslen` symbol on some UEFI/embedded targets, and a macOS
`rust-lld` library-search problem. These are compiler/toolchain fixes; this review does
not claim that Scufpad exercised the affected FFI return pattern.

Rust 1.99.0 adds C-variadic function definitions, `Vec::into_parts`/`from_parts`,
`Box::into_non_null`/`from_non_null`, owned lossy UTF-8 conversion, and raw-value layout
APIs. It improves some inclusive-range iteration; the observed endpoints of an
exhausted range were never guaranteed. Rust 1.94–1.98 also introduced, among others,
`array_windows`, conditional match guards, atomic updates, `assert_matches!`, bit-width
APIs and `strip_circumfix`.

Sources: [Rust 1.99.1 announcement](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/),
[Rust 1.99.0 announcement](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/), and
[official release notes](https://doc.rust-lang.org/stable/releases.html).

| Version since the prior 1.93.1 pin | Selected additions or fixes | Sweep result |
| --- | --- | --- |
| [1.94](https://doc.rust-lang.org/stable/releases.html#version-1940-2026-03-05) | Slice `array_windows`, `LazyLock` accessors and `Peekable::next_if_map` | HID parsing is variable-length; no compelling fixed-window replacement found. |
| [1.95](https://doc.rust-lang.org/stable/releases.html#version-1950-2026-04-16) | `if let` match guards, atomic update APIs, `cfg_select!`, `cold_path` | Existing actor and IPC atomics express specific transitions. Do not alter ordering or add cold hints without evidence. |
| [1.96](https://doc.rust-lang.org/stable/releases.html#version-1960-2026-05-28) | `assert_matches!` and range improvements | Could improve test failure messages, but not worth raising MSRV solely for diagnostics. |
| [1.97](https://doc.rust-lang.org/stable/releases.html#version-1970-2026-07-09) | Integer bit operations and Cargo warning configuration | Mapping masks could use newer bit helpers; measure before changing the input loop. |
| [1.98](https://doc.rust-lang.org/stable/releases.html#version-1980-2026-08-20) | Atomic slice conversions, UTF-16 constructors, `strip_circumfix` | No direct replacement found for existing application code. |
| [1.99](https://doc.rust-lang.org/stable/releases.html#version-1990-2026-10-01) | C variadics and ownership/layout APIs; range-iteration optimization | No code in this workspace requires the new ownership or C-variadic APIs. No speedup is claimed without measurements. |
| [1.99.1](https://doc.rust-lang.org/stable/releases.html#version-1991-2026-10-15) | x86-64 FFI `bool`, UEFI `wcslen`, macOS linker fixes | Patch release selected; application-specific effects are unproven. |

## Application sweep

Reviewed the core mapping/profile library, bridge discovery, native I/O, actor and IPC,
tray, desktop editor, tests and build/CI configuration. The automated inventory covered
26 Rust files, about 9,900 lines. No `Box::leak`, raw `Vec` reconstruction, `VecDeque`,
or exhausted `RangeInclusive` endpoint use was found by scoped text searches. These
searches do not prove that dependencies cannot use those operations.

The newer `assert_matches!` could improve failure diagnostics; `File::try_lock` may
remove one native `flock` call after careful verification of lock compatibility; and
`lowest_one` could simplify set-bit iteration if profiling supports changing the loop.
None justifies raising the minimum supported Rust version by itself. The fixed-width
test decoding now uses `slice::as_chunks`, which was already stable in Rust 1.88; it
addresses a new Clippy lint and preserves the existing handling of trailing bytes.
Three GUI stroke widths are explicitly `f32` to resolve new future-compatibility
warnings. Profile sorting uses equivalent `sort_by_key`, as requested by Clippy.

The review also found a pre-existing telemetry mismatch: at the exact release threshold,
the actor kept a trigger pressed while the mapping engine released it. With a release
threshold of zero, telemetry could stay pressed and miss subsequent rising edges. The
actor now matches the engine, with boundary, zero-threshold and re-press regressions.
Applying a replacement profile also recalculates trigger telemetry from the new press
threshold, matching the new engine's reset hysteresis state.
Profile-apply status also now distinguishes disconnected, unverified, verified and
stopped conditions without claiming an unavailable gamepad is forwarding.

Static review identified a separate latent unsafe abstraction in
`crates/scufpad/src/hardware.rs`: safe generic `ioctl_ptr` accepts any request and
`T`, although the kernel's request determines how much it reads or writes and the
bytes may not be valid for arbitrary `T`. Inspected callers pass matching layouts;
there is no evidence of an existing triggered overflow. A future hardening change
should restrict requests to audited types or mark the helper unsafe and document each
caller's requirements. This is not a Rust 1.99 regression and is not silently changed
as part of the toolchain update.

Two additional pre-existing limits merit follow-up if requirements call for them:
`client_request` uses a bounded connect and per-operation socket timeouts, not one total
deadline across all partial reads and writes; a slowly progressing same-user peer can
extend the overall call. Profile writes sync the temporary file before atomic rename,
but do not sync the containing directory afterward, so power-loss durability of the
new directory entry is not established. Neither limitation is specific to Rust 1.99.

The pinned compiler and CI now use 1.99.1. The declared Rust 1.88 minimum remains:
`cargo +1.88.0 check --workspace --all-targets --locked` passed after the changes.
The CI workflow now also runs the existing private-bus NoHardware tray regression.

## Verification

- `rustc 1.99.1 (58cf8f9c3 2026-10-13)` and `cargo 1.99.1` compiled all three
  workspace crates in Release mode with the lockfile unchanged.
- Formatting and strict all-target Clippy pass. The initial Clippy runs exposed the
  now-fixed float fallback, sorting and constant-sized chunk warnings.
- **107 Rust tests and 50 Python checks pass**, including isolated NoHardware IPC and
  private session-bus tray checks. The desktop self-test also passes.
- `cargo +1.88.0 check --workspace --all-targets --locked` passes, confirming the
  retained minimum compiler can check the whole workspace, including tests.
- The final optimized workspace build ran after the checks. Evidence is in
  `/tmp/opencode/scufpad-rust-1.99/`; the code graph was refreshed.
- No controller was opened, grabbed or profiled. Live game input, actual tray
  presentation, and changes in runtime performance remain unmeasured.
