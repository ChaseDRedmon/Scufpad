using Scufpad.Interop;
using Scufpad.Mapping;

namespace Scufpad.Output;

/// <summary>
///     Creates and manages a virtual Xbox Elite 2 controller via the Linux uinput subsystem.
/// </summary>
/// <remarks>
///     <para>
///         This class creates a virtual input device that appears to games as a standard Xbox Elite 2
///         controller (Microsoft VID: 0x045e, PID: 0x0b12). Games see this virtual device instead of
///         the physical Scuf controller, which has non-standard mappings.
///     </para>
///     <para>
///         The virtual device is created using the uinput kernel module, which must be loaded and
///         accessible (/dev/uinput must exist with appropriate permissions).
///     </para>
///     <para>
///         <b>Virtual device capabilities:</b>
///     </para>
///     <list type="bullet">
///         <item>
///             <description>4 face buttons (A, B, X, Y)</description>
///         </item>
///         <item>
///             <description>2 bumpers (LB, RB)</description>
///         </item>
///         <item>
///             <description>2 analog triggers (LT, RT: 0-1023)</description>
///         </item>
///         <item>
///             <description>2 analog sticks (±32768)</description>
///         </item>
///         <item>
///             <description>2 stick clicks (L3, R3)</description>
///         </item>
///         <item>
///             <description>3 menu buttons (Start, Select, Guide)</description>
///         </item>
///         <item>
///             <description>D-pad (8-way via HAT0X/HAT0Y)</description>
///         </item>
///         <item>
///             <description>4 paddle buttons (for Elite controller compatibility)</description>
///         </item>
///     </list>
/// </remarks>
internal sealed class VirtualGamepad : IDisposable
{
    private const string UinputPath = "/dev/uinput";

    // Xbox Elite 2 Controller identifiers (Microsoft)
    private const ushort XboxVendorId = 0x045e;
    private const ushort XboxProductId = 0x0b12;

    private const int StickMin = -32768;
    private const int StickMax = 32767;

    private const int TriggerMin = 0;
    private const int TriggerMax = 1023;

    private const int DpadMin = -1;
    private const int DpadMax = 1;

    // Maximum events per frame: 8 axes + 15 buttons + 1 sync = 24 events
    private const int MaxEventsPerFrame = 24;

    /// <summary>
    ///     Pre-allocated buffer for batching input events.
    ///     All events for a single frame are written with one syscall for better performance.
    /// </summary>
    private readonly InputEvent[] _eventBuffer = new InputEvent[MaxEventsPerFrame];

    private readonly int _fd;

    /// <summary>
    ///     Tracks the previous state to only emit events when values change.
    ///     This reduces unnecessary kernel traffic and improves performance.
    /// </summary>
    private readonly InputState _previousState = new();

    /// <summary>
    ///     Tracks consecutive write errors to avoid spamming the console.
    /// </summary>
    private int _consecutiveWriteErrors;

    private bool _disposed;

    /// <summary>
    ///     Current count of events queued in the buffer.
    /// </summary>
    private int _eventCount;

    private VirtualGamepad(int fd)
    {
        _fd = fd;
    }

    /// <summary>
    ///     Destroys the virtual device and closes the uinput file descriptor.
    /// </summary>
    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;

        unsafe
        {
            Libc.Ioctl(_fd, UinputIoctl.UI_DEV_DESTROY, null);
        }

        Libc.Close(_fd);
    }

    /// <summary>
    ///     Creates a new virtual Xbox Elite 2 controller.
    /// </summary>
    /// <returns>
    ///     A <see cref="VirtualGamepad" /> instance, or null if creation failed.
    ///     Common failures: uinput module not loaded, /dev/uinput permission denied.
    /// </returns>
    public static VirtualGamepad? Create()
    {
        var fd = Libc.Open(UinputPath, Libc.O_WRONLY | Libc.O_NONBLOCK);
        if (fd < 0)
        {
            var errno = Libc.GetLastError();
            Console.Error.WriteLine($"""
                                     Failed to open {UinputPath}: {Libc.StrError(errno)} (errno={errno})
                                     Make sure uinput module is loaded: sudo modprobe uinput
                                     Check permissions on /dev/uinput
                                     """);
            return null;
        }

        var gamepad = new VirtualGamepad(fd);

        if (!gamepad.SetupDevice())
        {
            gamepad.Dispose();
            return null;
        }

        return gamepad;
    }

    /// <summary>
    ///     Configures all capabilities and creates the virtual device.
    /// </summary>
    /// <returns>True if setup succeeded, false otherwise.</returns>
    private bool SetupDevice()
    {
        // Enable event types
        if (!EnableEventType(EventTypes.EV_KEY) ||
            !EnableEventType(EventTypes.EV_ABS) ||
            !EnableEventType(EventTypes.EV_SYN))
        {
            Console.Error.WriteLine("Failed to enable event types");
            return false;
        }

        // Enable buttons
        ushort[] buttons =
        [
            ButtonCodes.BTN_SOUTH, // A
            ButtonCodes.BTN_EAST, // B
            ButtonCodes.BTN_NORTH, // Y
            ButtonCodes.BTN_WEST, // X
            ButtonCodes.BTN_TL, // LB
            ButtonCodes.BTN_TR, // RB
            ButtonCodes.BTN_SELECT, // Back/Select
            ButtonCodes.BTN_START, // Start
            ButtonCodes.BTN_MODE, // Guide
            ButtonCodes.BTN_THUMBL, // L3
            ButtonCodes.BTN_THUMBR, // R3
            ButtonCodes.BTN_TRIGGER_HAPPY1, // Paddle 1
            ButtonCodes.BTN_TRIGGER_HAPPY2, // Paddle 2
            ButtonCodes.BTN_TRIGGER_HAPPY3, // Paddle 3
            ButtonCodes.BTN_TRIGGER_HAPPY4 // Paddle 4
        ];

        foreach (var btn in buttons)
        {
            if (!EnableKey(btn))
            {
                Console.Error.WriteLine($"Failed to enable button 0x{btn:x4}");
                return false;
            }
        }

        // Enable and configure axes
        // Sticks: ±32768 with fuzz=16, flat=128 for hardware noise filtering
        // Triggers: 0-1023 with no fuzz/flat for maximum responsiveness
        // D-pad: -1 to 1 (3-state)
        if (!EnableAndConfigureAxis(AbsCodes.ABS_X, StickMin, StickMax, 16, 128) ||
            !EnableAndConfigureAxis(AbsCodes.ABS_Y, StickMin, StickMax, 16, 128) ||
            !EnableAndConfigureAxis(AbsCodes.ABS_RX, StickMin, StickMax, 16, 128) ||
            !EnableAndConfigureAxis(AbsCodes.ABS_RY, StickMin, StickMax, 16, 128) ||
            !EnableAndConfigureAxis(AbsCodes.ABS_Z, TriggerMin, TriggerMax, 0, 0) ||
            !EnableAndConfigureAxis(AbsCodes.ABS_RZ, TriggerMin, TriggerMax, 0, 0) ||
            !EnableAndConfigureAxis(AbsCodes.ABS_HAT0X, DpadMin, DpadMax, 0, 0) ||
            !EnableAndConfigureAxis(AbsCodes.ABS_HAT0Y, DpadMin, DpadMax, 0, 0))
        {
            Console.Error.WriteLine("Failed to configure axes");
            return false;
        }

        // Setup device identity
        if (!SetupDeviceInfo())
        {
            Console.Error.WriteLine("Failed to setup device info");
            return false;
        }

        // Create the device
        if (!CreateDevice())
        {
            Console.Error.WriteLine("Failed to create uinput device");
            return false;
        }

        return true;
    }

    /// <summary>
    ///     Enables an event type (EV_KEY, EV_ABS, EV_SYN) on the virtual device.
    /// </summary>
    private bool EnableEventType(ushort type)
    {
        return Libc.Ioctl(_fd, UinputIoctl.UI_SET_EVBIT, type) >= 0;
    }

    /// <summary>
    ///     Enables a button/key code on the virtual device.
    /// </summary>
    private bool EnableKey(ushort code)
    {
        return Libc.Ioctl(_fd, UinputIoctl.UI_SET_KEYBIT, code) >= 0;
    }

    /// <summary>
    ///     Enables and configures an absolute axis on the virtual device.
    /// </summary>
    /// <param name="code">The axis code (ABS_X, ABS_Y, etc.).</param>
    /// <param name="min">Minimum axis value.</param>
    /// <param name="max">Maximum axis value.</param>
    /// <param name="fuzz">Hardware noise filtering threshold.</param>
    /// <param name="flat">Flat zone (deadzone enforced by kernel).</param>
    private unsafe bool EnableAndConfigureAxis(ushort code, int min, int max, int fuzz, int flat)
    {
        if (Libc.Ioctl(_fd, UinputIoctl.UI_SET_ABSBIT, code) < 0)
        {
            return false;
        }

        var absSetup = new UinputAbsSetup(
            code,
            new InputAbsInfo(min, max, fuzz, flat));

        return Libc.Ioctl(_fd, UinputIoctl.UI_ABS_SETUP, &absSetup) >= 0;
    }

    /// <summary>
    ///     Sets up the device identification (name, vendor/product IDs).
    /// </summary>
    private unsafe bool SetupDeviceInfo()
    {
        var setup = new UinputSetup
        {
            Id = new InputId(BusType.BUS_USB, XboxVendorId, XboxProductId, 1),
            FfEffectsMax = 0
        };

        var name = "Xbox Elite 2 Virtual Controller"u8;
        for (var i = 0; i < Math.Min(name.Length, 79); i++)
        {
            setup.Name[i] = name[i];
        }

        return Libc.Ioctl(_fd, UinputIoctl.UI_DEV_SETUP, &setup) >= 0;
    }

    /// <summary>
    ///     Creates the virtual device. After this call, the device appears in /dev/input/.
    /// </summary>
    private unsafe bool CreateDevice()
    {
        return Libc.Ioctl(_fd, UinputIoctl.UI_DEV_CREATE, null) >= 0;
    }

    /// <summary>
    ///     Emits the current controller state to the virtual device.
    ///     Only changed values are emitted to reduce kernel traffic.
    ///     All events are batched into a single write syscall for better performance.
    /// </summary>
    /// <param name="state">The filtered controller state to emit.</param>
    public void EmitState(InputState state)
    {
        _eventCount = 0;

        QueueAxisIfChanged(AbsCodes.ABS_X, state.LeftStickX, _previousState.LeftStickX);
        QueueAxisIfChanged(AbsCodes.ABS_Y, state.LeftStickY, _previousState.LeftStickY);
        QueueAxisIfChanged(AbsCodes.ABS_RX, state.RightStickX, _previousState.RightStickX);
        QueueAxisIfChanged(AbsCodes.ABS_RY, state.RightStickY, _previousState.RightStickY);
        QueueAxisIfChanged(AbsCodes.ABS_Z, state.LeftTrigger, _previousState.LeftTrigger);
        QueueAxisIfChanged(AbsCodes.ABS_RZ, state.RightTrigger, _previousState.RightTrigger);
        QueueAxisIfChanged(AbsCodes.ABS_HAT0X, state.DpadX, _previousState.DpadX);
        QueueAxisIfChanged(AbsCodes.ABS_HAT0Y, state.DpadY, _previousState.DpadY);

        QueueButtonIfChanged(ButtonCodes.BTN_SOUTH, state.ButtonA, _previousState.ButtonA);
        QueueButtonIfChanged(ButtonCodes.BTN_EAST, state.ButtonB, _previousState.ButtonB);
        QueueButtonIfChanged(ButtonCodes.BTN_WEST, state.ButtonX, _previousState.ButtonX);
        QueueButtonIfChanged(ButtonCodes.BTN_NORTH, state.ButtonY, _previousState.ButtonY);
        QueueButtonIfChanged(ButtonCodes.BTN_TL, state.BumperLeft, _previousState.BumperLeft);
        QueueButtonIfChanged(ButtonCodes.BTN_TR, state.BumperRight, _previousState.BumperRight);
        QueueButtonIfChanged(ButtonCodes.BTN_SELECT, state.ButtonSelect, _previousState.ButtonSelect);
        QueueButtonIfChanged(ButtonCodes.BTN_START, state.ButtonStart, _previousState.ButtonStart);
        QueueButtonIfChanged(ButtonCodes.BTN_MODE, state.ButtonGuide, _previousState.ButtonGuide);
        QueueButtonIfChanged(ButtonCodes.BTN_THUMBL, state.ThumbLeft, _previousState.ThumbLeft);
        QueueButtonIfChanged(ButtonCodes.BTN_THUMBR, state.ThumbRight, _previousState.ThumbRight);
        QueueButtonIfChanged(ButtonCodes.BTN_TRIGGER_HAPPY1, state.Paddle1, _previousState.Paddle1);
        QueueButtonIfChanged(ButtonCodes.BTN_TRIGGER_HAPPY2, state.Paddle2, _previousState.Paddle2);
        QueueButtonIfChanged(ButtonCodes.BTN_TRIGGER_HAPPY3, state.Paddle3, _previousState.Paddle3);
        QueueButtonIfChanged(ButtonCodes.BTN_TRIGGER_HAPPY4, state.Paddle4, _previousState.Paddle4);

        QueueEvent(EventTypes.EV_SYN, SynCodes.SYN_REPORT, 0);
        FlushEvents();
        state.CopyTo(_previousState);
    }

    /// <summary>
    ///     Queues an axis event if the value has changed.
    /// </summary>
    private void QueueAxisIfChanged(ushort code, int value, int previousValue)
    {
        if (value != previousValue)
        {
            QueueEvent(EventTypes.EV_ABS, code, value);
        }
    }

    /// <summary>
    ///     Queues a button event if the state has changed.
    /// </summary>
    private void QueueButtonIfChanged(ushort code, bool pressed, bool wasPressed)
    {
        if (pressed != wasPressed)
        {
            QueueEvent(EventTypes.EV_KEY, code, pressed ? 1 : 0);
        }
    }

    /// <summary>
    ///     Queues an input event into the batch buffer.
    /// </summary>
    private void QueueEvent(ushort type, ushort code, int value)
    {
        if (_eventCount >= MaxEventsPerFrame)
        {
            return;
        }

        _eventBuffer[_eventCount++] = new InputEvent
        {
            TvSec = 0,
            TvUsec = 0,
            Type = type,
            Code = code,
            Value = value
        };
    }

    /// <summary>
    ///     Writes all queued events to the uinput device in a single syscall.
    ///     This reduces kernel crossings from ~20 per frame to exactly 1.
    /// </summary>
    private unsafe void FlushEvents()
    {
        if (_eventCount == 0)
        {
            return;
        }

        fixed (InputEvent* ptr = _eventBuffer)
        {
            var bytesToWrite = (nuint)(_eventCount * InputEvent.Size);
            var bytesWritten = Libc.Write(_fd, ptr, bytesToWrite);

            if (bytesWritten < 0)
            {
                _consecutiveWriteErrors++;
                if (_consecutiveWriteErrors == 1 || _consecutiveWriteErrors % 1000 == 0)
                {
                    var errno = Libc.GetLastError();
                    Console.Error.WriteLine(
                        $"Warning: Failed to write to uinput device: {Libc.StrError(errno)} (errno={errno}, consecutive errors={_consecutiveWriteErrors})");
                }
            }
            else
            {
                _consecutiveWriteErrors = 0;
            }
        }
    }
}