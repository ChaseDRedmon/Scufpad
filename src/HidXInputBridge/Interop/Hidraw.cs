using System.Runtime.InteropServices;

namespace HidXInputBridge.Interop;

/// <summary>
///     Device information structure returned by the HIDIOCGRAWINFO ioctl.
///     Contains the bus type and USB vendor/product IDs for the HID device.
/// </summary>
/// <remarks>
///     This maps to the kernel's <c>struct hidraw_devinfo</c> defined in
///     <c>linux/hidraw.h</c>.
///     This is a readonly struct because it is only read after the kernel fills it.
///     In production, the kernel fills this struct via ioctl; the constructor is for testing.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
internal readonly struct HidrawDevInfo
{
    /// <summary>
    ///     Bus type (e.g., BUS_USB = 0x03, BUS_BLUETOOTH = 0x05).
    /// </summary>
    public readonly uint BusType;

    /// <summary>
    ///     USB Vendor ID (VID) of the device.
    ///     Note: Kernel uses signed short, but USB IDs are semantically unsigned.
    /// </summary>
    public readonly short Vendor;

    /// <summary>
    ///     USB Product ID (PID) of the device.
    ///     Note: Kernel uses signed short, but USB IDs are semantically unsigned.
    /// </summary>
    public readonly short Product;

    /// <summary>
    ///     Creates a new hidraw device info structure (primarily for testing).
    /// </summary>
    public HidrawDevInfo(uint busType, short vendor, short product)
    {
        BusType = busType;
        Vendor = vendor;
        Product = product;
    }
}

/// <summary>
///     ioctl request codes for hidraw devices.
///     These are used to query device information and control hidraw behavior.
/// </summary>
/// <remarks>
///     The ioctl codes are computed using the Linux _IOR/_IOW macros with 'H' as the type.
/// </remarks>
internal static class HidrawIoctl
{
    /// <summary>
    ///     HIDIOCGRAWINFO - Get raw device info (vendor/product IDs).
    ///     <c>_IOR('H', 0x03, struct hidraw_devinfo)</c> = 0x80084803
    /// </summary>
    public const nuint HIDIOCGRAWINFO = 0x80084803;

    /// <summary>
    ///     HIDIOCGRAWNAME(256) - Get raw device name string (up to 256 bytes).
    ///     <c>_IOC(_IOC_READ, 'H', 0x04, 256)</c> = 0x81004804
    /// </summary>
    public const nuint HIDIOCGRAWNAME_256 = 0x81004804;
}