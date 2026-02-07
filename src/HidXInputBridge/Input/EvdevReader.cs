using HidXInputBridge.Interop;

namespace HidXInputBridge.Input;

/// <summary>
///     Reads input events from a Linux evdev device.
///     Evdev (event device) is the Linux kernel's interface for input devices,
///     providing structured events for buttons, axes, and other input types.
///     This class opens the device in non-blocking mode and can optionally grab it
///     exclusively to prevent other applications from receiving its events.
/// </summary>
internal sealed class EvdevReader : IDisposable
{
    private bool _disposed;

    private bool _grabbed;

    private EvdevReader(int fd)
    {
        FileDescriptor = fd;
    }

    /// <summary>
    ///     Gets the file descriptor for the evdev device.
    ///     Used by <see cref="InputPoller" /> to multiplex input from multiple devices.
    /// </summary>
    public int FileDescriptor { get; }

    /// <summary>
    ///     Releases the exclusive grab and closes the file descriptor.
    /// </summary>
    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        Ungrab();
        Libc.Close(FileDescriptor);
    }

    /// <summary>
    ///     Opens an evdev device for reading.
    /// </summary>
    /// <param name="devicePath">
    ///     Path to the evdev device (e.g., "/dev/input/event5").
    /// </param>
    /// <param name="grabExclusive">
    ///     If true, grabs the device exclusively using EVIOCGRAB ioctl.
    ///     This prevents other applications (including games) from seeing
    ///     the device's raw input, which is essential when bridging to a
    ///     virtual controller to avoid double-input.
    /// </param>
    /// <returns>
    ///     An <see cref="EvdevReader" /> instance, or null if the device could not be opened.
    /// </returns>
    public static EvdevReader? Open(string devicePath, bool grabExclusive = true)
    {
        var fd = Libc.Open(devicePath, Libc.O_RDONLY | Libc.O_NONBLOCK);
        if (fd < 0)
        {
            var errno = Libc.GetLastError();
            Console.Error.WriteLine($"Failed to open {devicePath}: {Libc.StrError(errno)} (errno={errno})");
            return null;
        }

        var reader = new EvdevReader(fd);

        if (grabExclusive && !reader.Grab())
        {
            reader.Dispose();
            return null;
        }

        return reader;
    }

    /// <summary>
    ///     Grabs the device exclusively using the EVIOCGRAB ioctl.
    ///     While grabbed, no other process can receive events from this device.
    /// </summary>
    /// <returns>True if the grab succeeded, false otherwise.</returns>
    private bool Grab()
    {
        var result = Libc.Ioctl(FileDescriptor, EvdevIoctl.EVIOCGRAB, 1);
        if (result < 0)
        {
            var errno = Libc.GetLastError();
            Console.Error.WriteLine($"Failed to grab device exclusively: {Libc.StrError(errno)} (errno={errno})");
            return false;
        }

        _grabbed = true;
        return true;
    }

    /// <summary>
    ///     Releases the exclusive grab on the device.
    ///     Called automatically during disposal.
    /// </summary>
    private void Ungrab()
    {
        if (_grabbed)
        {
            Libc.Ioctl(FileDescriptor, EvdevIoctl.EVIOCGRAB, 0);
            _grabbed = false;
        }
    }

    /// <summary>
    ///     Reads pending input events from the device.
    ///     This method is non-blocking - if no events are available, it returns 0 immediately.
    /// </summary>
    /// <param name="buffer">
    ///     Buffer to receive the events. Should be large enough to hold multiple events
    ///     (typically 64 is sufficient for a single poll cycle).
    /// </param>
    /// <returns>
    ///     The number of events read, 0 if no events were available, or -1 on error.
    /// </returns>
    public unsafe int ReadEvents(Span<InputEvent> buffer)
    {
        if (_disposed)
        {
            return -1;
        }

        fixed (InputEvent* ptr = buffer)
        {
            var bytesRead = Libc.Read(FileDescriptor, ptr, (nuint)(buffer.Length * InputEvent.Size));

            if (bytesRead < 0)
            {
                var errno = Libc.GetLastError();
                // EAGAIN/EWOULDBLOCK means no data available (normal for non-blocking)
                if (errno == Libc.EAGAIN)
                {
                    return 0;
                }

                return -1;
            }

            return (int)(bytesRead / InputEvent.Size);
        }
    }
}