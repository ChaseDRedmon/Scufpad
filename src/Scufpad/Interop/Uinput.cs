using System.Runtime.InteropServices;

namespace Scufpad.Interop;

/// <summary>
///     Setup structure for creating a uinput virtual device.
///     Contains device identification and force feedback configuration.
/// </summary>
/// <remarks>
///     This maps to the kernel's <c>struct uinput_setup</c> defined in <c>linux/uinput.h</c>.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
internal unsafe struct UinputSetup
{
    /// <summary>Device identification (bus type, vendor/product IDs).</summary>
    public InputId Id;

    /// <summary>Device name (up to 80 characters, null-terminated).</summary>
    public fixed byte Name[80];

    /// <summary>Maximum number of force feedback effects (0 to disable).</summary>
    public uint FfEffectsMax;
}

/// <summary>
///     Absolute axis setup structure for uinput.
///     Configures the range and characteristics of an axis on the virtual device.
/// </summary>
/// <remarks>
///     <para>
///         This maps to the kernel's <c>struct uinput_abs_setup</c> defined in <c>linux/uinput.h</c>.
///     </para>
///     <para>
///         The kernel structure has a 16-bit code followed by 2 bytes padding (for alignment),
///         then struct input_absinfo (24 bytes).
///         Expected size: 2 (code) + 2 (padding) + 24 (absinfo) = 28 bytes.
///     </para>
///     <para>
///         The ioctl UI_ABS_SETUP (0x401c5504) encodes size 0x1c = 28 bytes, confirming padding.
///     </para>
///     <para>
///         This is a readonly struct because it is only set once and passed to ioctl.
///     </para>
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
internal readonly struct UinputAbsSetup
{
    /// <summary>Axis code (ABS_X, ABS_Y, ABS_Z, etc.).</summary>
    public readonly ushort Code;

    // 2 bytes implicit padding here for alignment

    /// <summary>Axis parameters (min, max, fuzz, flat).</summary>
    public readonly InputAbsInfo AbsInfo;

    /// <summary>Expected size of this structure in bytes (including padding).</summary>
    public const int Size = 28;

    /// <summary>
    ///     Creates a new absolute axis setup structure.
    /// </summary>
    /// <param name="code">Axis code (ABS_X, ABS_Y, ABS_Z, etc.).</param>
    /// <param name="absInfo">Axis parameters (min, max, fuzz, flat).</param>
    public UinputAbsSetup(ushort code, InputAbsInfo absInfo)
    {
        Code = code;
        AbsInfo = absInfo;
    }

    /// <summary>
    ///     Validates that the managed struct size matches the expected kernel struct size.
    /// </summary>
    /// <exception cref="InvalidOperationException">Thrown if size mismatch is detected.</exception>
    public static void ValidateSize()
    {
        var actualSize = Marshal.SizeOf<UinputAbsSetup>();
        if (actualSize != Size)
        {
            throw new InvalidOperationException(
                $"UinputAbsSetup structure size mismatch: expected {Size} bytes, got {actualSize} bytes. " +
                "The ioctl UI_ABS_SETUP expects a 28-byte structure.");
        }
    }
}

/// <summary>
///     ioctl request codes for the uinput virtual device interface.
///     These are used to configure and create virtual input devices.
/// </summary>
/// <remarks>
///     The ioctl codes are computed using the Linux _IOW/_IO macros with 'U' as the type.
///     The typical sequence to create a device is:
///     1. Open /dev/uinput
///     2. UI_SET_EVBIT for each event type (EV_KEY, EV_ABS, etc.)
///     3. UI_SET_KEYBIT for each button
///     4. UI_SET_ABSBIT + UI_ABS_SETUP for each axis
///     5. UI_DEV_SETUP to configure device identity
///     6. UI_DEV_CREATE to create the device
///     7. Write input_event structures to emit events
///     8. UI_DEV_DESTROY when done
/// </remarks>
internal static class UinputIoctl
{
    /// <summary>
    ///     UI_SET_EVBIT - Enable an event type on the virtual device.
    ///     <c>_IOW('U', 100, int)</c> = 0x40045564
    /// </summary>
    /// <remarks>
    ///     Pass the event type (EV_KEY, EV_ABS, EV_SYN, etc.) as the argument.
    /// </remarks>
    public const nuint UI_SET_EVBIT = 0x40045564;

    /// <summary>
    ///     UI_SET_KEYBIT - Enable a key/button code on the virtual device.
    ///     <c>_IOW('U', 101, int)</c> = 0x40045565
    /// </summary>
    /// <remarks>
    ///     Pass the button code (BTN_SOUTH, BTN_EAST, etc.) as the argument.
    ///     Requires UI_SET_EVBIT(EV_KEY) first.
    /// </remarks>
    public const nuint UI_SET_KEYBIT = 0x40045565;

    /// <summary>
    ///     UI_SET_ABSBIT - Enable an absolute axis on the virtual device.
    ///     <c>_IOW('U', 103, int)</c> = 0x40045567
    /// </summary>
    /// <remarks>
    ///     Pass the axis code (ABS_X, ABS_Y, etc.) as the argument.
    ///     Requires UI_SET_EVBIT(EV_ABS) first.
    ///     Should be followed by UI_ABS_SETUP to configure axis parameters.
    /// </remarks>
    public const nuint UI_SET_ABSBIT = 0x40045567;

    /// <summary>
    ///     UI_DEV_SETUP - Configure device identification (name, vendor/product IDs).
    ///     <c>_IOW('U', 3, struct uinput_setup)</c> = 0x405c5503
    /// </summary>
    /// <remarks>
    ///     Pass a pointer to UinputSetup structure.
    /// </remarks>
    public const nuint UI_DEV_SETUP = 0x405c5503;

    /// <summary>
    ///     UI_ABS_SETUP - Configure absolute axis parameters (min, max, fuzz, flat).
    ///     <c>_IOW('U', 4, struct uinput_abs_setup)</c> = 0x401c5504
    /// </summary>
    /// <remarks>
    ///     Pass a pointer to UinputAbsSetup structure.
    ///     Must be called after UI_SET_ABSBIT for the axis.
    /// </remarks>
    public const nuint UI_ABS_SETUP = 0x401c5504;

    /// <summary>
    ///     UI_DEV_CREATE - Create the virtual device.
    ///     <c>_IO('U', 1)</c> = 0x5501
    /// </summary>
    /// <remarks>
    ///     After this call, the device appears in /dev/input/ and applications can use it.
    /// </remarks>
    public const nuint UI_DEV_CREATE = 0x5501;

    /// <summary>
    ///     UI_DEV_DESTROY - Destroy the virtual device.
    ///     <c>_IO('U', 2)</c> = 0x5502
    /// </summary>
    /// <remarks>
    ///     Should be called before closing the uinput file descriptor.
    /// </remarks>
    public const nuint UI_DEV_DESTROY = 0x5502;
}

/// <summary>
///     Bus type constants for device identification.
/// </summary>
internal static class BusType
{
    /// <summary>USB bus - used for USB-connected devices.</summary>
    public const ushort BUS_USB = 0x03;

    /// <summary>Virtual bus - used for software-created virtual devices.</summary>
    public const ushort BUS_VIRTUAL = 0x06;
}