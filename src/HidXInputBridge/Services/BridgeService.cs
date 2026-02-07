using HidXInputBridge.Input;
using HidXInputBridge.Interop;
using HidXInputBridge.Mapping;
using HidXInputBridge.Output;

namespace HidXInputBridge.Services;

/// <summary>
///     Orchestrates the main input bridging loop.
///     Reads input from the physical Scuf controller, maps it to Xbox format,
///     applies filtering, and emits to the virtual Xbox controller.
/// </summary>
/// <remarks>
///     <para>
///         <b>Data flow pipeline:</b>
///     </para>
///     <code>
/// Physical Scuf Controller
///         ↓
/// EvdevReader (reads evdev events) / HidrawReader (reads HID reports)
///         ↓
/// InputPoller (multiplexes with poll(2), 4ms timeout)
///         ↓
/// EnvisionMapping (translates Scuf's non-standard mappings to standard Xbox)
///         ↓
/// InputFilter (applies deadzone and jitter filtering)
///         ↓
/// VirtualGamepad (emits to virtual Xbox Elite 2 via uinput)
///         ↓
/// Games see standard Xbox controller
/// </code>
///     <para>
///         The service runs a tight loop polling at ~250Hz (4ms timeout) for responsive input.
///         This provides sub-frame latency at 60fps while being CPU-efficient.
///     </para>
/// </remarks>
internal sealed class BridgeService : IDisposable
{
    private readonly EvdevReader _evdev;

    private readonly InputFilter _filter;

    /// <summary>
    ///     Filtered input state (after deadzone/jitter filtering).
    /// </summary>
    private readonly InputState _filteredState = new();
    private readonly HidrawReader? _hidraw;
    private readonly InputPoller _poller;

    /// <summary>
    ///     Raw input state from the controller (before filtering).
    /// </summary>
    private readonly InputState _rawState = new();
    private readonly VirtualGamepad _virtualGamepad;

    /// <summary>
    ///     Creates a new bridge service.
    /// </summary>
    /// <param name="evdev">The evdev reader for the physical controller.</param>
    /// <param name="hidraw">Optional hidraw reader (currently unused for V2 hardware).</param>
    /// <param name="virtualGamepad">The virtual Xbox controller to emit to.</param>
    public BridgeService(
        EvdevReader evdev,
        HidrawReader? hidraw,
        VirtualGamepad virtualGamepad)
    {
        _evdev = evdev;
        _hidraw = hidraw;
        _virtualGamepad = virtualGamepad;
        _poller = new InputPoller(evdev, hidraw);
        _filter = new InputFilter();
    }

    /// <summary>
    ///     Disposes of all resources (evdev reader, hidraw reader, virtual gamepad).
    /// </summary>
    public void Dispose()
    {
        _evdev.Dispose();
        _hidraw?.Dispose();
        _virtualGamepad.Dispose();
    }

    /// <summary>
    ///     Runs the main bridging loop until cancellation is requested.
    /// </summary>
    /// <param name="cancellationToken">Token to signal shutdown (e.g., from Ctrl+C).</param>
    /// <remarks>
    ///     The loop polls for input at ~250Hz (4ms timeout) and processes events as they arrive.
    ///     On each iteration:
    ///     1. Poll for available data on evdev/hidraw
    ///     2. Read and process any evdev events through EnvisionMapping
    ///     3. If state changed, apply filtering and emit to virtual gamepad
    /// </remarks>
    public void Run(CancellationToken cancellationToken)
    {
        Console.WriteLine("Bridge service started. Press Ctrl+C to exit.");

        Span<InputEvent> eventBuffer = stackalloc InputEvent[64];

        while (!cancellationToken.IsCancellationRequested)
        {
            var pollResult = _poller.Poll(4);

            if (pollResult.HasFlag(PollResult.Error))
            {
                Console.Error.WriteLine("Poll error on evdev device");
                break;
            }

            var stateChanged = false;
            if (pollResult.HasFlag(PollResult.EvdevReady))
            {
                var eventCount = _evdev.ReadEvents(eventBuffer);
                for (var i = 0; i < eventCount; i++)
                {
                    EnvisionMapping.ProcessEvdevEvent(in eventBuffer[i], _rawState);
                }

                if (eventCount > 0)
                {
                    stateChanged = true;
                }
            }

            if (stateChanged && _rawState.IsDirty)
            {
                _filter.Apply(_rawState, _filteredState);
                _virtualGamepad.EmitState(_filteredState);
                _rawState.ClearDirty();
            }
        }

        Console.WriteLine("Bridge service stopped.");
    }
}