using HidXInputBridge.Interop;

namespace HidXInputBridge.Input;

/// <summary>
///     Reads raw HID reports from a Linux hidraw device.
///     Hidraw provides direct access to HID (Human Interface Device) reports,
///     which can contain data not exposed through the standard evdev interface.
///     Note: On Scuf Envision Pro V2 hardware, both triggers are available via evdev,
///     so hidraw reading is optional and currently disabled to avoid latency issues.
///     This class is kept for potential V1 hardware support or future use.
/// </summary>
internal sealed class HidrawReader : IDisposable
{
    private bool _disposed;

    private HidrawReader(int fd)
    {
        FileDescriptor = fd;
    }

    /// <summary>
    ///     Gets the file descriptor for the hidraw device.
    ///     Used by <see cref="InputPoller" /> to multiplex input from multiple devices.
    /// </summary>
    public int FileDescriptor { get; }

    /// <summary>
    ///     Closes the hidraw file descriptor.
    /// </summary>
    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        Libc.Close(FileDescriptor);
    }

    /// <summary>
    ///     Opens a hidraw device for reading.
    /// </summary>
    /// <param name="devicePath">
    ///     Path to the hidraw device (e.g., "/dev/hidraw0").
    ///     If empty or null, returns null immediately.
    /// </param>
    /// <returns>
    ///     A <see cref="HidrawReader" /> instance, or null if the device could not be opened
    ///     or the path was empty.
    /// </returns>
    public static HidrawReader? Open(string devicePath)
    {
        if (string.IsNullOrEmpty(devicePath))
        {
            return null;
        }

        var fd = Libc.Open(devicePath, Libc.O_RDONLY | Libc.O_NONBLOCK);
        if (fd < 0)
        {
            var errno = Libc.GetLastError();
            Console.Error.WriteLine($"Failed to open {devicePath}: {Libc.StrError(errno)} (errno={errno})");
            return null;
        }

        var reader = new HidrawReader(fd);

        if (!reader.VerifyDevice())
        {
            Console.Error.WriteLine("Warning: Could not verify hidraw device identity");
        }

        // Continue anyway, the device might still work
        return reader;
    }

    /// <summary>
    ///     Verifies that the opened hidraw device is the expected Scuf controller
    ///     by checking its vendor and product IDs using the HIDIOCGRAWINFO ioctl.
    /// </summary>
    /// <returns>True if the device matches the expected Scuf controller IDs.</returns>
    private unsafe bool VerifyDevice()
    {
        HidrawDevInfo devInfo;
        var result = Libc.Ioctl(FileDescriptor, HidrawIoctl.HIDIOCGRAWINFO, &devInfo);

        if (result < 0)
        {
            return false;
        }

        // Verify it's the Scuf controller
        const ushort ScufVendorId = 0x1b1c;
        const ushort ScufProductId = 0x3a05;

        return (ushort)devInfo.Vendor == ScufVendorId && (ushort)devInfo.Product == ScufProductId;
    }

    /// <summary>
    ///     Reads a raw HID report from the device.
    ///     This method is non-blocking - if no report is available, it returns 0 immediately.
    /// </summary>
    /// <param name="buffer">
    ///     Buffer to receive the HID report. Should be at least 64 bytes for most controllers.
    /// </param>
    /// <returns>
    ///     The number of bytes read, 0 if no report was available, or -1 on error.
    /// </returns>
    public unsafe int ReadReport(Span<byte> buffer)
    {
        if (_disposed)
        {
            return -1;
        }

        fixed (byte* ptr = buffer)
        {
            var bytesRead = Libc.Read(FileDescriptor, ptr, (nuint)buffer.Length);

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

            return (int)bytesRead;
        }
    }
}