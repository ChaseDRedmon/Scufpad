using System.Runtime.InteropServices;

namespace HidXInputBridge.Interop;

/// <summary>
///     Linux input event structure representing a single input event from evdev.
///     Each event contains a timestamp, type, code, and value.
/// </summary>
/// <remarks>
///     This maps to the kernel's <c>struct input_event</c> defined in <c>linux/input.h</c>.
///     The structure is 24 bytes on x64 Linux due to 64-bit time_t.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
internal struct InputEvent
{
    /// <summary>Seconds component of the event timestamp.</summary>
    public long TvSec;

    /// <summary>Microseconds component of the event timestamp.</summary>
    public long TvUsec;

    /// <summary>Event type (EV_KEY, EV_ABS, EV_SYN, etc.).</summary>
    public ushort Type;

    /// <summary>Event code (specific to the event type).</summary>
    public ushort Code;

    /// <summary>Event value (key state, axis position, etc.).</summary>
    public int Value;

    /// <summary>Size of this structure in bytes (24 on x64 Linux).</summary>
    public const int Size = 24;

    /// <summary>
    ///     Validates that the managed struct size matches the expected kernel struct size.
    ///     Call this at startup to detect potential marshalling issues.
    /// </summary>
    /// <exception cref="InvalidOperationException">Thrown if size mismatch is detected.</exception>
    public static void ValidateSize()
    {
        var actualSize = Marshal.SizeOf<InputEvent>();
        if (actualSize != Size)
        {
            throw new InvalidOperationException(
                $"InputEvent structure size mismatch: expected {Size} bytes, got {actualSize} bytes. " +
                "This indicates a marshalling issue with the kernel's struct input_event.");
        }
    }
}

/// <summary>
///     Input device identification structure.
///     Contains bus type and USB vendor/product/version IDs.
/// </summary>
/// <remarks>
///     This maps to the kernel's <c>struct input_id</c> defined in <c>linux/input.h</c>.
///     This is a readonly struct because it is only set during device initialization.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
internal readonly struct InputId
{
    /// <summary>Bus type (BUS_USB, BUS_VIRTUAL, etc.).</summary>
    public readonly ushort BusType;

    /// <summary>USB Vendor ID (VID).</summary>
    public readonly ushort Vendor;

    /// <summary>USB Product ID (PID).</summary>
    public readonly ushort Product;

    /// <summary>Device version number.</summary>
    public readonly ushort Version;

    /// <summary>
    ///     Creates a new input device identification.
    /// </summary>
    public InputId(ushort busType, ushort vendor, ushort product, ushort version)
    {
        BusType = busType;
        Vendor = vendor;
        Product = product;
        Version = version;
    }
}

/// <summary>
///     Absolute axis information structure.
///     Describes the range and characteristics of an absolute axis (joystick, trigger, etc.).
/// </summary>
/// <remarks>
///     This maps to the kernel's <c>struct input_absinfo</c> defined in <c>linux/input.h</c>.
///     This is a readonly struct because it is only set during axis configuration.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
internal readonly struct InputAbsInfo
{
    /// <summary>Current value of the axis.</summary>
    public readonly int Value;

    /// <summary>Minimum reportable value.</summary>
    public readonly int Minimum;

    /// <summary>Maximum reportable value.</summary>
    public readonly int Maximum;

    /// <summary>
    ///     Fuzz value - values within this range of the previous value are ignored.
    ///     Used for hardware noise filtering.
    /// </summary>
    public readonly int Fuzz;

    /// <summary>
    ///     Flat zone - values within this range of center are reported as center.
    ///     Used for hardware deadzone.
    /// </summary>
    public readonly int Flat;

    /// <summary>Resolution in units per millimeter (optional).</summary>
    public readonly int Resolution;

    /// <summary>
    ///     Creates a new absolute axis information structure.
    /// </summary>
    public InputAbsInfo(int minimum, int maximum, int fuzz = 0, int flat = 0, int value = 0, int resolution = 0)
    {
        Value = value;
        Minimum = minimum;
        Maximum = maximum;
        Fuzz = fuzz;
        Flat = flat;
        Resolution = resolution;
    }
}

/// <summary>
///     Event type constants for Linux input events.
/// </summary>
internal static class EventTypes
{
    /// <summary>Synchronization event - marks the end of a set of related events.</summary>
    public const ushort EV_SYN = 0x00;

    /// <summary>Key/button event - press, release, or repeat.</summary>
    public const ushort EV_KEY = 0x01;

    /// <summary>Absolute axis event - joystick position, trigger value, etc.</summary>
    public const ushort EV_ABS = 0x03;

    /// <summary>Force feedback event (rumble, vibration).</summary>
    public const ushort EV_FF = 0x15;
}

/// <summary>
///     Synchronization event codes.
/// </summary>
internal static class SynCodes
{
    /// <summary>
    ///     Marks the end of a related set of events.
    ///     All events between SYN_REPORT events should be processed together.
    /// </summary>
    public const ushort SYN_REPORT = 0x00;
}

/// <summary>
///     Button/key codes for gamepad buttons.
///     These are the standard Linux kernel button codes used by evdev.
/// </summary>
/// <remarks>
///     Note: The Scuf Envision Pro V2 uses non-standard button codes.
///     See <see cref="Mapping.EnvisionMapping" /> for the actual mappings.
/// </remarks>
internal static class ButtonCodes
{
    /// <summary>Base code for miscellaneous buttons.</summary>
    public const ushort BTN_MISC = 0x100;

    /// <summary>Base code for gamepad buttons.</summary>
    public const ushort BTN_GAMEPAD = 0x130;

    /// <summary>South button (A on Xbox, Cross on PlayStation).</summary>
    public const ushort BTN_SOUTH = 0x130;

    /// <summary>Alias for BTN_SOUTH.</summary>
    public const ushort BTN_A = BTN_SOUTH;

    /// <summary>East button (B on Xbox, Circle on PlayStation).</summary>
    public const ushort BTN_EAST = 0x131;

    /// <summary>Alias for BTN_EAST.</summary>
    public const ushort BTN_B = BTN_EAST;

    /// <summary>C button (used by Scuf V2 for X button).</summary>
    public const ushort BTN_C = 0x132;

    /// <summary>North button (Y on Xbox, Triangle on PlayStation).</summary>
    public const ushort BTN_NORTH = 0x133;

    /// <summary>Alias for BTN_NORTH (confusingly named in kernel).</summary>
    public const ushort BTN_X = BTN_NORTH;

    /// <summary>West button (X on Xbox, Square on PlayStation). Used by Scuf V2 for LB.</summary>
    public const ushort BTN_WEST = 0x134;

    /// <summary>Alias for BTN_WEST (confusingly named in kernel).</summary>
    public const ushort BTN_Y = BTN_WEST;

    /// <summary>Z button (used by Scuf V2 for RB).</summary>
    public const ushort BTN_Z = 0x135;

    /// <summary>Left bumper / L1 (standard). Used by Scuf V2 for Select.</summary>
    public const ushort BTN_TL = 0x136;

    /// <summary>Right bumper / R1 (standard). Used by Scuf V2 for Start.</summary>
    public const ushort BTN_TR = 0x137;

    /// <summary>Left trigger button / L2 (standard). Used by Scuf V2 for L3.</summary>
    public const ushort BTN_TL2 = 0x138;

    /// <summary>Right trigger button / R2 (standard). Used by Scuf V2 for R3.</summary>
    public const ushort BTN_TR2 = 0x139;

    /// <summary>Back / Share / Select button.</summary>
    public const ushort BTN_SELECT = 0x13a;

    /// <summary>Start / Options button.</summary>
    public const ushort BTN_START = 0x13b;

    /// <summary>Guide / Home / Xbox button.</summary>
    public const ushort BTN_MODE = 0x13c;

    /// <summary>Left stick click / L3 (standard).</summary>
    public const ushort BTN_THUMBL = 0x13d;

    /// <summary>Right stick click / R3 (standard).</summary>
    public const ushort BTN_THUMBR = 0x13e;

    /// <summary>Paddle 1 (for Elite controllers).</summary>
    public const ushort BTN_TRIGGER_HAPPY1 = 0x2c0;

    /// <summary>Paddle 2 (for Elite controllers).</summary>
    public const ushort BTN_TRIGGER_HAPPY2 = 0x2c1;

    /// <summary>Paddle 3 (for Elite controllers).</summary>
    public const ushort BTN_TRIGGER_HAPPY3 = 0x2c2;

    /// <summary>Paddle 4 (for Elite controllers).</summary>
    public const ushort BTN_TRIGGER_HAPPY4 = 0x2c3;
}

/// <summary>
///     Absolute axis codes for gamepad axes.
///     These are the standard Linux kernel axis codes used by evdev.
/// </summary>
/// <remarks>
///     Note: The Scuf Envision Pro V2 uses non-standard axis assignments.
///     See <see cref="Mapping.EnvisionMapping" /> for the actual mappings.
/// </remarks>
internal static class AbsCodes
{
    /// <summary>Left stick X axis (standard).</summary>
    public const ushort ABS_X = 0x00;

    /// <summary>Left stick Y axis (standard).</summary>
    public const ushort ABS_Y = 0x01;

    /// <summary>Z axis - typically left trigger, but Scuf V2 uses for right stick X.</summary>
    public const ushort ABS_Z = 0x02;

    /// <summary>Right stick X (standard), but Scuf V2 uses for left trigger.</summary>
    public const ushort ABS_RX = 0x03;

    /// <summary>Right stick Y (standard), but Scuf V2 uses for right trigger.</summary>
    public const ushort ABS_RY = 0x04;

    /// <summary>Right trigger (standard), but Scuf V2 uses for right stick Y.</summary>
    public const ushort ABS_RZ = 0x05;

    /// <summary>D-pad X axis (-1 = left, 0 = center, 1 = right).</summary>
    public const ushort ABS_HAT0X = 0x10;

    /// <summary>D-pad Y axis (-1 = up, 0 = center, 1 = down).</summary>
    public const ushort ABS_HAT0Y = 0x11;
}

/// <summary>
///     ioctl request codes for evdev devices.
/// </summary>
internal static class EvdevIoctl
{
    /// <summary>
    ///     EVIOCGRAB - Grab or release exclusive access to the device.
    ///     When grabbed, no other process receives events from this device.
    ///     <c>_IOW('E', 0x90, int)</c> = 0x40044590
    /// </summary>
    /// <remarks>
    ///     Pass 1 to grab, 0 to release.
    /// </remarks>
    public const nuint EVIOCGRAB = 0x40044590;
}