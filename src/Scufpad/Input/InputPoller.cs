using Scufpad.Interop;

namespace Scufpad.Input;

/// <summary>
///     Flags indicating which input sources have data available after polling.
/// </summary>
[Flags]
internal enum PollResult
{
    /// <summary>No events ready.</summary>
    None = 0,

    /// <summary>The evdev device has input events ready to read.</summary>
    EvdevReady = 1,

    /// <summary>The hidraw device has a HID report ready to read.</summary>
    HidrawReady = 2,

    /// <summary>An error occurred on a polled device.</summary>
    Error = 4,

    /// <summary>The poll timed out with no events.</summary>
    Timeout = 8
}

/// <summary>
///     Multiplexes input from evdev and hidraw devices using poll(2).
///     This allows the main loop to efficiently wait for input from multiple
///     sources without busy-waiting or using multiple threads.
/// </summary>
/// <remarks>
///     <para>
///         The poll timeout determines the maximum latency between input and response.
///         Lower timeouts provide more responsive input but use more CPU.
///     </para>
///     <para>
///         <b>Ownership:</b> This class does NOT own the file descriptors - they are owned
///         by the <see cref="EvdevReader" /> and <see cref="HidrawReader" /> instances passed
///         to the constructor. Do not dispose of this class's readers separately.
///     </para>
///     <para>
///         <b>Thread safety:</b> This class is not thread-safe. Use from a single thread only.
///     </para>
/// </remarks>
internal sealed class InputPoller
{
    private readonly int _evdevFd;
    private readonly bool _hasHidraw;
    private readonly int _hidrawFd;

    /// <summary>
    ///     Creates a new input poller for the given devices.
    /// </summary>
    /// <param name="evdev">The evdev reader (required).</param>
    /// <param name="hidraw">The hidraw reader (optional, may be null).</param>
    public InputPoller(EvdevReader evdev, HidrawReader? hidraw)
    {
        _evdevFd = evdev.FileDescriptor;
        _hidrawFd = hidraw?.FileDescriptor ?? -1;
        _hasHidraw = hidraw is not null;
    }

    /// <summary>
    ///     Waits for input to be available on any of the polled devices.
    ///     Uses the poll(2) system call to efficiently multiplex multiple file descriptors.
    /// </summary>
    /// <param name="timeoutMs">
    ///     Maximum time to wait in milliseconds. Lower values provide more responsive
    ///     input (4ms ≈ 250Hz polling is recommended for gaming). A value of -1 would
    ///     wait indefinitely, but this is not recommended as it prevents clean shutdown.
    /// </param>
    /// <returns>
    ///     A <see cref="PollResult" /> indicating which devices have data available,
    ///     or if an error/timeout occurred.
    /// </returns>
    public unsafe PollResult Poll(int timeoutMs)
    {
        var fdCount = _hasHidraw ? 2 : 1;
        var fds = stackalloc PollFd[2];

        fds[0] = new PollFd
        {
            Fd = _evdevFd,
            Events = Libc.POLLIN,
            Revents = 0
        };

        if (_hasHidraw)
        {
            fds[1] = new PollFd
            {
                Fd = _hidrawFd,
                Events = Libc.POLLIN,
                Revents = 0
            };
        }

        int result;
        do
        {
            result = Libc.Poll(fds, (nuint)fdCount, timeoutMs);
        } while (result < 0 && Libc.GetLastError() == Libc.EINTR);

        if (result < 0)
        {
            return PollResult.Error;
        }

        if (result == 0)
        {
            return PollResult.Timeout;
        }

        var pollResult = PollResult.None;

        if ((fds[0].Revents & (Libc.POLLIN | Libc.POLLERR | Libc.POLLHUP)) != 0)
        {
            if ((fds[0].Revents & Libc.POLLIN) != 0)
            {
                pollResult |= PollResult.EvdevReady;
            }

            if ((fds[0].Revents & (Libc.POLLERR | Libc.POLLHUP)) != 0)
            {
                pollResult |= PollResult.Error;
            }
        }

        if (_hasHidraw && (fds[1].Revents & (Libc.POLLIN | Libc.POLLERR | Libc.POLLHUP)) != 0)
        {
            if ((fds[1].Revents & Libc.POLLIN) != 0)
            {
                pollResult |= PollResult.HidrawReady;
            }
        }

        // Don't set error for hidraw issues, it's optional
        return pollResult;
    }
}