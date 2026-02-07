using System.Runtime.InteropServices;

namespace HidXInputBridge.Interop;

/// <summary>
///     P/Invoke bindings for essential libc functions.
///     Uses LibraryImport for AOT compatibility and better performance.
/// </summary>
/// <remarks>
///     These are the core POSIX system calls needed for:
///     - Opening and closing file descriptors (open, close)
///     - Reading and writing data (read, write)
///     - Device control (ioctl)
///     - I/O multiplexing (poll)
///     - Error handling (strerror)
/// </remarks>
internal static partial class Libc
{
    /// <summary>
    ///     Opens a file or device.
    /// </summary>
    /// <param name="path">Path to the file or device.</param>
    /// <param name="flags">Open flags (O_RDONLY, O_WRONLY, O_RDWR, O_NONBLOCK, etc.).</param>
    /// <returns>File descriptor on success, -1 on error (check GetLastError).</returns>
    [LibraryImport("libc", EntryPoint = "open", StringMarshalling = StringMarshalling.Utf8, SetLastError = true)]
    public static partial int Open(string path, int flags);

    /// <summary>
    ///     Closes a file descriptor.
    /// </summary>
    /// <param name="fd">File descriptor to close.</param>
    /// <returns>0 on success, -1 on error.</returns>
    [LibraryImport("libc", EntryPoint = "close", SetLastError = true)]
    public static partial int Close(int fd);

    /// <summary>
    ///     Reads data from a file descriptor.
    /// </summary>
    /// <param name="fd">File descriptor to read from.</param>
    /// <param name="buf">Buffer to receive the data.</param>
    /// <param name="count">Maximum number of bytes to read.</param>
    /// <returns>Number of bytes read, 0 at EOF, -1 on error.</returns>
    [LibraryImport("libc", EntryPoint = "read", SetLastError = true)]
    public static unsafe partial nint Read(int fd, void* buf, nuint count);

    /// <summary>
    ///     Writes data to a file descriptor.
    /// </summary>
    /// <param name="fd">File descriptor to write to.</param>
    /// <param name="buf">Buffer containing the data to write.</param>
    /// <param name="count">Number of bytes to write.</param>
    /// <returns>Number of bytes written, -1 on error.</returns>
    [LibraryImport("libc", EntryPoint = "write", SetLastError = true)]
    public static unsafe partial nint Write(int fd, void* buf, nuint count);

    /// <summary>
    ///     Performs a device-specific control operation (with integer argument).
    /// </summary>
    /// <param name="fd">File descriptor of the device.</param>
    /// <param name="request">Device-specific request code.</param>
    /// <param name="value">Integer argument for the request.</param>
    /// <returns>0 on success (usually), -1 on error.</returns>
    [LibraryImport("libc", EntryPoint = "ioctl", SetLastError = true)]
    public static partial int Ioctl(int fd, nuint request, int value);

    /// <summary>
    ///     Performs a device-specific control operation (with pointer argument).
    /// </summary>
    /// <param name="fd">File descriptor of the device.</param>
    /// <param name="request">Device-specific request code.</param>
    /// <param name="arg">Pointer to argument structure.</param>
    /// <returns>0 on success (usually), -1 on error.</returns>
    [LibraryImport("libc", EntryPoint = "ioctl", SetLastError = true)]
    public static unsafe partial int Ioctl(int fd, nuint request, void* arg);

    /// <summary>
    ///     Waits for events on multiple file descriptors.
    ///     This is the core I/O multiplexing function used to efficiently wait
    ///     for input from multiple devices without busy-waiting.
    /// </summary>
    /// <param name="fds">Array of PollFd structures describing the file descriptors to monitor.</param>
    /// <param name="nfds">Number of file descriptors in the array.</param>
    /// <param name="timeout">Timeout in milliseconds (-1 for infinite, 0 for non-blocking).</param>
    /// <returns>Number of fds with events, 0 on timeout, -1 on error.</returns>
    [LibraryImport("libc", EntryPoint = "poll", SetLastError = true)]
    public static unsafe partial int Poll(PollFd* fds, nuint nfds, int timeout);

    [LibraryImport("libc", EntryPoint = "strerror", StringMarshalling = StringMarshalling.Utf8)]
    private static partial nint StrErrorInternal(int errnum);

    /// <summary>
    ///     Gets a human-readable error message for an error number.
    /// </summary>
    /// <param name="errnum">Error number (from GetLastError).</param>
    /// <returns>Error message string.</returns>
    public static string StrError(int errnum)
    {
        var ptr = StrErrorInternal(errnum);
        return Marshal.PtrToStringUTF8(ptr) ?? "Unknown error";
    }

    /// <summary>
    ///     Gets the last P/Invoke error code (errno).
    /// </summary>
    /// <returns>The errno value from the last failed system call.</returns>
    public static int GetLastError()
    {
        return Marshal.GetLastPInvokeError();
    }

    /// <summary>Open for reading only.</summary>
    public const int O_RDONLY = 0x0000;

    /// <summary>Open for writing only.</summary>
    public const int O_WRONLY = 0x0001;

    /// <summary>Open for reading and writing.</summary>
    public const int O_RDWR = 0x0002;

    /// <summary>Non-blocking I/O - read/write return immediately if no data available.</summary>
    public const int O_NONBLOCK = 0x0800;

    /// <summary>Data available to read.</summary>
    public const int POLLIN = 0x0001;

    /// <summary>Writing now will not block.</summary>
    public const int POLLOUT = 0x0004;

    /// <summary>Error condition on device.</summary>
    public const int POLLERR = 0x0008;

    /// <summary>Hang up (device disconnected).</summary>
    public const int POLLHUP = 0x0010;

    /// <summary>Interrupted system call - should retry.</summary>
    public const int EINTR = 4;

    /// <summary>Resource temporarily unavailable (would block).</summary>
    public const int EAGAIN = 11;
}

/// <summary>
///     Structure used by poll(2) to describe a file descriptor to monitor.
/// </summary>
/// <remarks>
///     This maps to the kernel's <c>struct pollfd</c> defined in <c>poll.h</c>.
///     This struct remains mutable because the kernel writes to <see cref="Revents" />.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
internal struct PollFd
{
    /// <summary>File descriptor to monitor.</summary>
    public int Fd;

    /// <summary>Events to monitor (POLLIN, POLLOUT, etc.).</summary>
    public short Events;

    /// <summary>Events that occurred (filled in by poll).</summary>
    public short Revents;
}