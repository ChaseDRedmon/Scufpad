# HidXInputBridge

A Linux application that bridges a Scuf Envision Pro V2 controller to a virtual Xbox Elite 2 controller via the uinput
subsystem.

## Overview

The Scuf Envision Pro V2 controller (VID: `0x1b1c`, PID: `0x3a05`) uses highly non-standard evdev mappings that cause
incorrect button/axis assignments in most Linux games. This bridge:

1. **Reads** input from the physical Scuf controller via evdev
2. **Translates** the non-standard mappings to standard Xbox Elite 2 format
3. **Outputs** to a virtual gamepad that games recognize correctly

The virtual controller appears to games as a standard Xbox Elite 2 controller (Microsoft VID: `0x045e`, PID: `0x0b12`).

## Requirements

- Linux with uinput support
- .NET 10.0 or later
- Scuf Envision Pro V2 controller (VID: `0x1b1c`, PID: `0x3a05`)

## Quick Start

```bash
# 1. Set up udev rules (one-time)
sudo tee /etc/udev/rules.d/99-scuf.rules << 'EOF'
SUBSYSTEM=="input", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", MODE="0666"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", MODE="0666"
KERNEL=="uinput", MODE="0666"
EOF

# 2. Reload udev rules
sudo udevadm control --reload-rules && sudo udevadm trigger

# 3. Load uinput module
sudo modprobe uinput

# 4. Run the bridge
dotnet run --project src/HidXInputBridge
```

## Game-Specific Configuration

### Steam Games (Important!)

For the best experience with Steam games, you need to **disable Steam Input** and tell SDL to ignore the physical
controller:

#### 1. Disable Steam Input for the Game

1. Right-click the game in Steam → **Properties**
2. Go to **Controller** tab
3. Set **Override for [Game]** to **Disable Steam Input**

#### 2. Add Launch Options (SDL Games)

For games using SDL (like Stardew Valley), add this to the Steam launch options:

```
SDL_GAMECONTROLLER_IGNORE_DEVICES=0x1b1c/0x3a05 %command%
```

This tells SDL to ignore the physical Scuf controller so only the virtual Xbox controller is used.

**Example for Stardew Valley:**

1. Right-click Stardew Valley → **Properties**
2. In **Launch Options**, enter:
   ```
   SDL_GAMECONTROLLER_IGNORE_DEVICES=0x1b1c/0x3a05 %command%
   ```
3. In the **Controller** tab, disable Steam Input

### Non-Steam Games

The bridge grabs the physical controller exclusively, so most games will only see the virtual Xbox controller. However,
some games may need the `SDL_GAMECONTROLLER_IGNORE_DEVICES` environment variable set before launching.

## Setup Details

### 1. Create udev rules

Create `/etc/udev/rules.d/99-scuf.rules`:

```bash
sudo tee /etc/udev/rules.d/99-scuf.rules << 'EOF'
SUBSYSTEM=="input", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", MODE="0666"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", MODE="0666"
KERNEL=="uinput", MODE="0666"
EOF
```

### 2. Reload udev rules

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Then unplug and replug your controller.

### 3. Load the uinput module

```bash
sudo modprobe uinput
```

To load uinput automatically on boot:

```bash
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf
```

### 4. Verify setup

```bash
# Check controller is detected
lsusb | grep 1b1c:3a05

# Check device permissions
ls -la /dev/input/by-id/*Envision*
ls -la /dev/uinput
```

## Usage

```bash
# Run from source
dotnet run --project src/HidXInputBridge

# Or build and run
dotnet build
./src/HidXInputBridge/bin/Debug/net10.0/HidXInputBridge

# AOT publish (single binary)
dotnet publish -c Release -r linux-x64
```

## Architecture

### Data Flow Pipeline

```
Physical Scuf Controller
        ↓
EvdevReader (reads evdev events)
        ↓
InputPoller (multiplexes with poll(2), 4ms timeout ≈ 250Hz)
        ↓
EnvisionMapping (translates Scuf's non-standard mappings)
        ↓
InputFilter (applies deadzone and jitter filtering)
        ↓
VirtualGamepad (emits to virtual Xbox Elite 2 via uinput)
        ↓
Games see standard Xbox controller
```

### Project Structure

```
src/HidXInputBridge/
├── Discovery/           # Device discovery via /sys/class
│   └── DeviceDiscovery.cs
├── Input/               # Input device readers
│   ├── EvdevReader.cs   # Reads evdev input events
│   ├── HidrawReader.cs  # Reads raw HID reports (optional)
│   └── InputPoller.cs   # Multiplexes input with poll(2)
├── Interop/             # P/Invoke bindings
│   ├── Hidraw.cs        # hidraw ioctl constants
│   ├── Libc.cs          # libc functions (open, read, ioctl, poll)
│   ├── LinuxInput.cs    # input_event struct, button/axis codes
│   └── Uinput.cs        # uinput ioctl constants
├── Mapping/             # Input translation and filtering
│   ├── EnvisionMapping.cs  # Scuf V2 → Xbox mapping
│   ├── InputFilter.cs      # Deadzone and jitter filtering
│   └── InputState.cs       # Controller state representation
├── Output/              # Virtual device creation
│   └── VirtualGamepad.cs   # Creates Xbox Elite 2 via uinput
├── Services/            # Main service orchestration
│   └── BridgeService.cs    # Main loop coordination
└── Program.cs           # Entry point
```

## Hardware Mapping Reference

### Scuf Envision Pro V2 Axis Mappings

The V2 uses completely non-standard axis positions:

| Scuf Axis         | evdev Code     | Value Range     | Maps To       |
|-------------------|----------------|-----------------|---------------|
| Left Stick X      | ABS_X (0)      | -32768 to 32767 | Left Stick X  |
| Left Stick Y      | ABS_Y (1)      | -32768 to 32767 | Left Stick Y  |
| **Right Stick X** | **ABS_Z (2)**  | -32768 to 32767 | Right Stick X |
| **Left Trigger**  | **ABS_RX (3)** | 0 to 1023       | Left Trigger  |
| **Right Trigger** | **ABS_RY (4)** | 0 to 1023       | Right Trigger |
| **Right Stick Y** | **ABS_RZ (5)** | -32768 to 32767 | Right Stick Y |
| D-pad X           | ABS_HAT0X (16) | -1 to 1         | D-pad X       |
| D-pad Y           | ABS_HAT0Y (17) | -1 to 1         | D-pad Y       |

**Bold** = Non-standard mapping (differs from typical Xbox controllers)

### Scuf Envision Pro V2 Button Mappings

The V2 uses highly non-standard button codes:

| Scuf Button | evdev Code           | Standard Code | Maps To           |
|-------------|----------------------|---------------|-------------------|
| A           | BTN_SOUTH (0x130)    | BTN_SOUTH     | A Button          |
| B           | BTN_EAST (0x131)     | BTN_EAST      | B Button          |
| **X**       | **BTN_C (0x132)**    | BTN_WEST      | X Button          |
| Y           | BTN_NORTH (0x133)    | BTN_NORTH     | Y Button          |
| **LB**      | **BTN_WEST (0x134)** | BTN_TL        | Left Bumper       |
| **RB**      | **BTN_Z (0x135)**    | BTN_TR        | Right Bumper      |
| **L3**      | **BTN_TL2 (0x138)**  | BTN_THUMBL    | Left Stick Click  |
| **R3**      | **BTN_TR2 (0x139)**  | BTN_THUMBR    | Right Stick Click |
| Select      | BTN_TL (0x136)       | BTN_SELECT    | Back/Select       |
| Start       | BTN_TR (0x137)       | BTN_START     | Start             |
| Guide       | BTN_MODE (0x13c)     | BTN_MODE      | Guide/Home        |
| Paddle 1    | BTN_TRIGGER_HAPPY1   | -             | Paddle 1          |
| Paddle 2    | BTN_TRIGGER_HAPPY2   | -             | Paddle 2          |
| Paddle 3    | BTN_TRIGGER_HAPPY3   | -             | Paddle 3          |

**Bold** = Non-standard mapping

**Note:** The V2 only has 3 paddle buttons, not 4.

## Performance Tuning

The bridge is optimized for low-latency input:

| Setting                  | Value         | Purpose                                       |
|--------------------------|---------------|-----------------------------------------------|
| Poll timeout             | 4ms           | ~250Hz polling for sub-frame latency at 60fps |
| Stick deadzone           | 3500 (~10.7%) | Eliminates stick drift                        |
| Trigger deadzone         | 10 (~1%)      | Minimal deadzone for responsiveness           |
| Stick jitter threshold   | 300           | Filters analog noise                          |
| Trigger jitter threshold | 20 (~2%)      | Low threshold for responsive triggers         |

These defaults can be adjusted in `InputFilter.cs` if needed.

## Troubleshooting

### "Permission denied" errors

```bash
# Verify udev rules
cat /etc/udev/rules.d/99-scuf.rules

# Reload rules
sudo udevadm control --reload-rules && sudo udevadm trigger

# Unplug and replug controller
# Check permissions
ls -la /dev/hidraw* /dev/uinput /dev/input/event*
```

### "uinput device not found"

```bash
# Load the module
sudo modprobe uinput

# Verify it's loaded
lsmod | grep uinput
```

### Controller not detected

```bash
# Check USB connection
lsusb | grep 1b1c

# View kernel messages
dmesg | tail -20

# Ensure you have the V2 controller (PID 3a05)
```

### Game sees both controllers / double input

Make sure:

1. The bridge is running before starting the game
2. Steam Input is disabled for the game
3. `SDL_GAMECONTROLLER_IGNORE_DEVICES=0x1b1c/0x3a05` is set (for SDL games)

### Trigger feels laggy or unresponsive

This was fixed in recent updates. If you experience trigger latency:

1. Ensure you're running the latest version
2. The poll timeout should be 4ms (not 100ms)
3. The trigger jitter threshold should be 20 (not 75)

## Building

```bash
# Debug build
dotnet build

# Release build
dotnet build -c Release

# Run tests
dotnet test

# AOT publish (single binary, no .NET runtime required)
dotnet publish -c Release -r linux-x64
```

## Testing

```bash
# Run all tests
dotnet test

# Run specific test project
dotnet test tests/hidxinputbridge.Tests.Unit
dotnet test tests/hidxinputbridge.Tests.Integration

# Test with evtest
evtest  # Select the virtual Xbox controller to verify output
```

## Technical Notes

### Why evdev grab is necessary

The bridge grabs the physical controller exclusively using `EVIOCGRAB` to:

1. Prevent games from seeing the raw Scuf input with wrong mappings
2. Prevent double-input (one from physical, one from virtual)

### Why hidraw is optional on V2

V1 hardware reportedly didn't expose the right trigger via evdev, requiring hidraw fallback. V2 hardware provides both
triggers via evdev (`ABS_RX` for LT, `ABS_RY` for RT), so hidraw is no longer needed. Hidraw processing is disabled to
avoid latency issues from stale data.

### Multiple evdev devices

The Scuf controller exposes multiple input devices (joystick, mouse emulation, etc.). All are grabbed exclusively to
prevent input leakage.

## License

MIT
