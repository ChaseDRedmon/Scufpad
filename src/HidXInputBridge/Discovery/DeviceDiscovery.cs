using System.Globalization;

namespace HidXInputBridge.Discovery;

/// <summary>
///     Represents the discovered Scuf Envision Pro controller devices.
///     Contains paths to all input devices associated with the controller.
/// </summary>
internal sealed class DiscoveredDevices
{
    /// <summary>
    ///     Path to the primary evdev device (e.g., "/dev/input/event5").
    ///     This is the main joystick interface that provides axis and button events.
    /// </summary>
    public required string EvdevPath { get; init; }

    /// <summary>
    ///     Path to the hidraw device (e.g., "/dev/hidraw0").
    ///     Used for raw HID reports. On V2 hardware this is optional since both
    ///     triggers are available via evdev.
    /// </summary>
    public required string HidrawPath { get; init; }

    /// <summary>
    ///     Additional evdev devices that should be grabbed exclusively to hide them from games.
    ///     The Scuf controller exposes multiple input devices (e.g., mouse emulation, keyboard),
    ///     and ungrabbed ones can leak raw axis data to games, causing phantom button presses
    ///     or duplicate input.
    /// </summary>
    public required IReadOnlyList<string> SecondaryEvdevPaths { get; init; }
}

/// <summary>
///     Discovers Scuf Envision Pro V2 controller devices by scanning /sys/class.
///     Finds both evdev (input events) and hidraw (raw HID reports) devices
///     by matching the controller's USB vendor/product IDs.
/// </summary>
internal static class DeviceDiscovery
{
    /// <summary>
    ///     Corsair/Scuf USB Vendor ID.
    /// </summary>
    private const ushort ScufVendorId = 0x1b1c;

    /// <summary>
    ///     Scuf Envision Pro V2 USB Product ID.
    /// </summary>
    private const ushort ScufProductId = 0x3a05;

    /// <summary>
    ///     Searches for connected Scuf Envision Pro V2 controller devices.
    ///     Scans /sys/class/input for evdev devices and /sys/class/hidraw for hidraw devices
    ///     matching the Scuf vendor/product IDs.
    /// </summary>
    /// <returns>
    ///     A <see cref="DiscoveredDevices" /> object containing paths to all controller devices,
    ///     or null if the controller is not found.
    /// </returns>
    public static DiscoveredDevices? FindController()
    {
        var (evdevPath, secondaryPaths) = FindAllEvdevDevices();
        var hidrawPath = FindHidrawDevice();

        if (evdevPath is null)
        {
            Console.Error.WriteLine($"""
                                     Error: Could not find Scuf Envision Pro controller evdev device.
                                     Looking for VID={ScufVendorId:x4} PID={ScufProductId:x4}
                                     """);
            return null;
        }

        if (hidrawPath is null)
        {
            Console.Error.WriteLine("Warning: Could not find hidraw device. R2 trigger may not work correctly.");
            // We can continue without hidraw, just won't have R2 from hidraw
            hidrawPath = string.Empty;
        }

        return new DiscoveredDevices
        {
            EvdevPath = evdevPath,
            HidrawPath = hidrawPath,
            SecondaryEvdevPaths = secondaryPaths
        };
    }

    /// <summary>
    ///     Finds all evdev devices matching the Scuf controller.
    ///     The controller exposes multiple event devices - we identify the primary joystick
    ///     device (which has a js* handler) and collect secondary devices for grabbing.
    /// </summary>
    /// <returns>
    ///     A tuple containing the primary evdev path (or null if not found) and a list of
    ///     secondary evdev paths that should be grabbed to prevent input leakage.
    /// </returns>
    private static (string? primary, List<string> secondary) FindAllEvdevDevices()
    {
        const string inputClassPath = "/sys/class/input";
        var allDevices = new List<(string path, bool hasJoystick)>();

        if (!Directory.Exists(inputClassPath))
        {
            return (null, []);
        }

        foreach (var eventDir in Directory.GetDirectories(inputClassPath, "event*"))
        {
            var deviceIdPath = Path.Combine(eventDir, "device", "id");
            if (!Directory.Exists(deviceIdPath))
            {
                continue;
            }

            var vendorPath = Path.Combine(deviceIdPath, "vendor");
            var productPath = Path.Combine(deviceIdPath, "product");

            if (!File.Exists(vendorPath) || !File.Exists(productPath))
            {
                continue;
            }

            try
            {
                var vendorStr = File.ReadAllText(vendorPath).Trim();
                var productStr = File.ReadAllText(productPath).Trim();

                if (ushort.TryParse(vendorStr, NumberStyles.HexNumber, null, out var vendor) &&
                    ushort.TryParse(productStr, NumberStyles.HexNumber, null, out var product))
                {
                    if (vendor == ScufVendorId && product == ScufProductId)
                    {
                        var eventName = Path.GetFileName(eventDir);
                        var devPath = $"/dev/input/{eventName}";

                        // Check if this device has a js* handler (indicates main joystick interface)
                        // The main joystick device has EV_KEY capability for buttons
                        var hasJoystick = HasJoystickHandler(eventDir);

                        allDevices.Add((devPath, hasJoystick));
                    }
                }
            }
            catch (IOException)
            {
                // Skip devices we can't read
            }
        }

        // Sort: primary joystick device first, then secondary devices
        // The primary device is the one with the js* handler
        string? primary = null;
        var secondary = new List<string>();

        foreach (var (path, hasJoystick) in allDevices)
        {
            if (hasJoystick && primary is null)
            {
                primary = path;
            }
            else
            {
                secondary.Add(path);
            }
        }

        // If no joystick device found, use the first one as primary
        if (primary is null && allDevices.Count > 0)
        {
            primary = allDevices[0].path;
            secondary = allDevices.Skip(1).Select(d => d.path).ToList();
        }

        return (primary, secondary);
    }

    /// <summary>
    ///     Checks if an event device has an associated js* (joystick) device.
    ///     The js* device indicates this is the main gamepad interface with button support,
    ///     as opposed to secondary interfaces like mouse or keyboard emulation.
    /// </summary>
    /// <param name="eventDir">Path to the event directory in /sys/class/input.</param>
    /// <returns>True if a js* sibling device exists, indicating this is the main joystick.</returns>
    private static bool HasJoystickHandler(string eventDir)
    {
        // Check if there's a js* device associated with this event device.
        // The js* device is a sibling of the event* device under the same input device.
        // Structure: /sys/class/input/eventX -> /devices/.../inputN/eventX
        //            /sys/class/input/jsY    -> /devices/.../inputN/jsY
        // So we check for js* siblings in the parent directory.
        try
        {
            // eventDir is like /sys/class/input/event5 (symlink to /devices/.../inputN/event5)
            // We need to resolve the symlink and check for sibling js* directories
            var linkTarget = Directory.ResolveLinkTarget(eventDir, true);
            if (linkTarget is null)
            {
                return false;
            }

            var realPath = linkTarget.FullName;
            var parentDir = Path.GetDirectoryName(realPath);

            if (parentDir is not null && Directory.Exists(parentDir))
            {
                // Look for js* entries as siblings of the event device
                var jsEntries = Directory.GetDirectories(parentDir, "js*");
                if (jsEntries.Length > 0)
                {
                    return true;
                }
            }
        }
        catch (IOException)
        {
            // Ignore errors, assume not a joystick
        }

        return false;
    }

    /// <summary>
    ///     Finds the hidraw device for the Scuf controller by scanning /sys/class/hidraw.
    ///     Parses the HID_ID field in uevent files to match the controller's vendor/product IDs.
    /// </summary>
    /// <returns>
    ///     Path to the hidraw device (e.g., "/dev/hidraw0"), or null if not found.
    /// </returns>
    private static string? FindHidrawDevice()
    {
        const string hidrawClassPath = "/sys/class/hidraw";

        if (!Directory.Exists(hidrawClassPath))
        {
            return null;
        }

        foreach (var hidrawDir in Directory.GetDirectories(hidrawClassPath, "hidraw*"))
        {
            var ueventPath = Path.Combine(hidrawDir, "device", "uevent");
            if (!File.Exists(ueventPath))
            {
                continue;
            }

            try
            {
                var ueventContent = File.ReadAllText(ueventPath);

                // Look for HID_ID line: HID_ID=0003:00001B1C:00003A05
                // Format is: bustype:vendor:product (8 hex digits each after bustype)
                foreach (var line in ueventContent.Split('\n'))
                {
                    if (!line.StartsWith("HID_ID="))
                    {
                        continue;
                    }

                    var parts = line[7..].Split(':');
                    if (parts.Length < 3)
                    {
                        continue;
                    }

                    if (uint.TryParse(parts[1], NumberStyles.HexNumber, null, out var vendor) &&
                        uint.TryParse(parts[2], NumberStyles.HexNumber, null, out var product))
                    {
                        if (vendor == ScufVendorId && product == ScufProductId)
                        {
                            var hidrawName = Path.GetFileName(hidrawDir);
                            return $"/dev/{hidrawName}";
                        }
                    }
                }
            }
            catch (IOException)
            {
                // Skip devices we can't read
            }
        }

        return null;
    }
}