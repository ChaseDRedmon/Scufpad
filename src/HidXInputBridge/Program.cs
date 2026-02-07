// HidXInputBridge - Scuf Envision Pro V2 to Xbox Controller Bridge
//
// This application bridges a Scuf Envision Pro V2 controller to a virtual Xbox Elite 2
// controller via the Linux uinput subsystem. It reads from the physical controller's
// evdev/hidraw devices, translates the non-standard input mappings, and outputs to a
// virtual gamepad that games recognize as a standard Xbox controller.
//
// The Scuf Envision Pro V2 uses highly non-standard evdev mappings that cause incorrect
// button/axis assignments in most games. This bridge fixes that by remapping everything
// to standard Xbox Elite 2 format.
//
// Usage:
//   dotnet run --project src/HidXInputBridge
//
// Requirements:
//   - Linux with uinput support (sudo modprobe uinput)
//   - Appropriate udev rules for device permissions
//   - Scuf Envision Pro V2 controller (VID: 0x1b1c, PID: 0x3a05)
//
// See README.md for full setup instructions.

using HidXInputBridge.Discovery;
using HidXInputBridge.Input;
using HidXInputBridge.Output;
using HidXInputBridge.Services;

Console.WriteLine(
    """
    Scuf Envision Pro V2 to Xbox Controller Bridge
    ==============================================
    
    """);

// Set up cancellation for Ctrl+C
using var cts = new CancellationTokenSource();
Console.CancelKeyPress += (_, e) =>
{
    e.Cancel = true;
    cts.Cancel();
    Console.WriteLine("\nShutdown requested...");
};

// Discover controller devices
Console.WriteLine("Searching for Scuf Envision Pro controller...");
var devices = DeviceDiscovery.FindController();

if (devices is null)
{
    Console.Error.WriteLine(
        """
        
        Troubleshooting:
          1. Make sure the controller is connected
          2. Check if the device appears in: ls /dev/input/event*
          3. Check permissions: ls -la /dev/input/

        To grant permissions, create /etc/udev/rules.d/99-hid-xinput-bridge.rules:
          SUBSYSTEM=="input", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", MODE="0666"
          SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="3a05", MODE="0666"
          KERNEL=="uinput", MODE="0666"

        Then reload udev: sudo udevadm control --reload && sudo udevadm trigger
        """);
    
    return 1;
}

var hidrawStatus = string.IsNullOrEmpty(devices.HidrawPath)
    ? "not found (R2 trigger may not work)"
    : devices.HidrawPath;

var secondaryInfo = devices.SecondaryEvdevPaths.Count > 0
    ? $"Secondary evdev devices to grab: {devices.SecondaryEvdevPaths.Count}"
    : "";

Console.WriteLine(
    $"""
    Found evdev:  {devices.EvdevPath}
      {secondaryInfo}
    Hidraw:       {hidrawStatus}
    
    """);

// Open input devices
Console.WriteLine("Opening input devices...");
var evdev = EvdevReader.Open(devices.EvdevPath);
if (evdev is null)
{
    Console.Error.WriteLine("Failed to open evdev device. Try running with sudo.");
    return 1;
}

// Grab all secondary evdev devices to hide them from games
// These are opened but not read from - just grabbed exclusively to prevent input leakage
var grabbedDevices = new List<EvdevReader>();
foreach (var secondaryPath in devices.SecondaryEvdevPaths)
{
    var grabbed = EvdevReader.Open(secondaryPath);
    if (grabbed is not null)
    {
        grabbedDevices.Add(grabbed);
        Console.WriteLine($"  Grabbed: {secondaryPath}");
    }
    else
    {
        Console.WriteLine($"  Warning: Failed to grab {secondaryPath}");
    }
}

HidrawReader? hidraw = null;
if (!string.IsNullOrEmpty(devices.HidrawPath))
{
    hidraw = HidrawReader.Open(devices.HidrawPath);
}

// hidraw is optional, continue even if it fails
// Create virtual gamepad
Console.WriteLine("\nCreating virtual Xbox controller...");
var virtualGamepad = VirtualGamepad.Create();
if (virtualGamepad is null)
{
    evdev.Dispose();
    hidraw?.Dispose();
    foreach (var grabbed in grabbedDevices)
    {
        grabbed.Dispose();
    }

    Console.Error.WriteLine(
        """
        Failed to create virtual gamepad.
        Make sure uinput module is loaded: sudo modprobe uinput
        """);
    return 1;
}

Console.WriteLine("Created: Xbox Elite 2 Virtual Controller\n");

// Create and run the bridge service
using var bridge = new BridgeService(evdev, hidraw, virtualGamepad);

try
{
    bridge.Run(cts.Token);
}
catch (Exception ex)
{
    Console.Error.WriteLine($"Error: {ex.Message}");
    return 1;
}
finally
{
    // Release all grabbed secondary devices
    foreach (var grabbed in grabbedDevices)
    {
        grabbed.Dispose();
    }
}

return 0;