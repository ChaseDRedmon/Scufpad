# Scufpad

Provide a Linux application for the SCUF Envision Pro V2 with a native Rust desktop
editor and an independently running Rust CLI/background service. Preserve the verified
USB mapping and existing profiles, keyboard/mouse actions, finite macros and layers.

Opening the editor starts or attaches to the bridge and shows its actual status.
Closing the editor does not stop the bridge. Normal gamepad forwarding starts
automatically. The saved custom profile activates only after the six paddle/SAX inputs
verify independently. Loss of verification falls back to normal forwarding. Stop All
still removes every owned output until explicitly resumed. No firmware writes, new
device permissions or login startup are part of the port. Validate wireless separately.
