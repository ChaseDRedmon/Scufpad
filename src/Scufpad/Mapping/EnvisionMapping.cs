using Scufpad.Interop;

namespace Scufpad.Mapping;

/// <summary>
///     Maps Scuf Envision Pro V2 inputs to standard Xbox controller format.
/// </summary>
/// <remarks>
///     <para>
///         The Scuf Envision Pro V2 (VID: 0x1b1c, PID: 0x3a05) uses non-standard evdev
///         mappings that differ significantly from standard Xbox controllers. This class
///         translates those inputs to the standard Xbox Elite 2 format expected by games.
///     </para>
///     <para>
///         <b>VERIFIED V2 axis mappings:</b>
///     </para>
///     <list type="table">
///         <listheader>
///             <term>Envision Axis</term>
///             <description>Maps To</description>
///         </listheader>
///         <item>
///             <term>ABS_X (0)</term><description>Left stick X [-32768, 32767]</description>
///         </item>
///         <item>
///             <term>ABS_Y (1)</term><description>Left stick Y [-32768, 32767]</description>
///         </item>
///         <item>
///             <term>ABS_Z (2)</term><description>Right stick X (non-standard, should be ABS_RX)</description>
///         </item>
///         <item>
///             <term>ABS_RX (3)</term><description>Left trigger [0, 1023] (non-standard, should be ABS_Z)</description>
///         </item>
///         <item>
///             <term>ABS_RY (4)</term><description>Right trigger [0, 1023] (non-standard, should be ABS_RZ)</description>
///         </item>
///         <item>
///             <term>ABS_RZ (5)</term><description>Right stick Y (non-standard, should be ABS_RY)</description>
///         </item>
///         <item>
///             <term>ABS_HAT0X/Y</term><description>D-pad [-1, 1]</description>
///         </item>
///     </list>
///     <para>
///         <b>VERIFIED V2 button mappings (highly non-standard!):</b>
///     </para>
///     <list type="table">
///         <listheader>
///             <term>Envision Button</term>
///             <description>Xbox Function</description>
///         </listheader>
///         <item>
///             <term>BTN_SOUTH (0x130)</term><description>A button</description>
///         </item>
///         <item>
///             <term>BTN_EAST (0x131)</term><description>B button</description>
///         </item>
///         <item>
///             <term>BTN_C (0x132)</term><description>X button (non-standard!)</description>
///         </item>
///         <item>
///             <term>BTN_NORTH (0x133)</term><description>Y button</description>
///         </item>
///         <item>
///             <term>BTN_WEST (0x134)</term><description>Left Bumper (non-standard!)</description>
///         </item>
///         <item>
///             <term>BTN_Z (0x135)</term><description>Right Bumper (non-standard!)</description>
///         </item>
///         <item>
///             <term>BTN_TL2 (0x138)</term><description>L3 / Left Stick Click (non-standard!)</description>
///         </item>
///         <item>
///             <term>BTN_TR2 (0x139)</term><description>R3 / Right Stick Click (non-standard!)</description>
///         </item>
///         <item>
///             <term>BTN_TRIGGER_HAPPY1-3</term><description>Paddles 1-3 (V2 only has 3 paddles)</description>
///         </item>
///     </list>
///     <para>
///         Both triggers are available via evdev on V2 hardware, so the hidraw fallback
///         is disabled to prevent latency issues from stale data overwrites.
///     </para>
/// </remarks>
internal static class EnvisionMapping
{
    /// <summary>
    ///     Processes a single evdev input event and updates the input state.
    /// </summary>
    /// <param name="ev">The input event to process (passed by readonly reference to avoid copying).</param>
    /// <param name="state">The input state to update.</param>
    /// <remarks>
    ///     Handles EV_ABS (axis), EV_KEY (button), and EV_SYN (sync) events.
    ///     The sync event is ignored as state is processed continuously.
    ///     Uses 'in' parameter for the 24-byte struct to avoid defensive copies while preventing mutation.
    /// </remarks>
    public static void ProcessEvdevEvent(in InputEvent ev, InputState state)
    {
        switch (ev.Type)
        {
            case EventTypes.EV_ABS:
                ProcessAbsoluteAxis(ev.Code, ev.Value, state);
                break;

            case EventTypes.EV_KEY:
                ProcessButton(ev.Code, ev.Value != 0, state);
                break;

            case EventTypes.EV_SYN:
                break;
        }
    }

    /// <summary>
    ///     Processes an absolute axis event with Scuf V2 non-standard mappings.
    /// </summary>
    /// <param name="code">The axis code (ABS_X, ABS_Y, etc.).</param>
    /// <param name="value">The axis value.</param>
    /// <param name="state">The input state to update.</param>
    private static void ProcessAbsoluteAxis(ushort code, int value, InputState state)
    {
        switch (code)
        {
            case AbsCodes.ABS_X:
                state.LeftStickX = value;
                state.MarkDirty();
                break;

            case AbsCodes.ABS_Y:
                state.LeftStickY = value;
                state.MarkDirty();
                break;

            // Right stick X - Envision reports this on ABS_Z (non-standard!)
            case AbsCodes.ABS_Z:
                state.RightStickX = value;
                state.MarkDirty();
                break;

            // Left trigger - Envision reports this on ABS_RX (non-standard!)
            case AbsCodes.ABS_RX:
                state.LeftTrigger = value;
                state.MarkDirty();
                break;

            // Right stick Y - Envision reports this on ABS_RZ (non-standard!)
            case AbsCodes.ABS_RZ:
                state.RightStickY = value;
                state.MarkDirty();
                break;

            // Right trigger - Envision reports this on ABS_RY (non-standard!)
            case AbsCodes.ABS_RY:
                state.RightTrigger = value;
                state.MarkDirty();
                break;

            case AbsCodes.ABS_HAT0X:
                state.DpadX = value;
                state.MarkDirty();
                break;

            case AbsCodes.ABS_HAT0Y:
                state.DpadY = value;
                state.MarkDirty();
                break;
        }
    }

    /// <summary>
    ///     Processes a button event with Scuf V2 non-standard mappings.
    /// </summary>
    /// <param name="code">The button code.</param>
    /// <param name="pressed">Whether the button is pressed.</param>
    /// <param name="state">The input state to update.</param>
    /// <remarks>
    ///     The Scuf V2 uses highly non-standard button codes:
    ///     - BTN_C for X button (instead of BTN_WEST)
    ///     - BTN_WEST for LB (instead of BTN_TL)
    ///     - BTN_Z for RB (instead of BTN_TR)
    ///     - BTN_TL2 for L3 (instead of BTN_THUMBL)
    ///     - BTN_TR2 for R3 (instead of BTN_THUMBR)
    /// </remarks>
    private static void ProcessButton(ushort code, bool pressed, InputState state)
    {
        switch (code)
        {
            // Face buttons
            case ButtonCodes.BTN_SOUTH: // A
                state.ButtonA = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_EAST: // B
                state.ButtonB = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_C: // X (non-standard!)
                state.ButtonX = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_NORTH: // Y
                state.ButtonY = pressed;
                state.MarkDirty();
                break;

            // Shoulder buttons (non-standard!)
            case ButtonCodes.BTN_WEST: // LB (normally X button position)
                state.BumperLeft = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_Z: // RB (non-standard!)
                state.BumperRight = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_TL2: // Left stick click (L3)
                state.ThumbLeft = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_TR2: // Right stick click (R3)
                state.ThumbRight = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_TL: // Select on V2
                state.ButtonSelect = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_TR: // Start on V2
                state.ButtonStart = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_MODE:
                state.ButtonGuide = pressed;
                state.MarkDirty();
                break;

            // Legacy standard mappings (not used on V2)
            case ButtonCodes.BTN_THUMBL:
            case ButtonCodes.BTN_THUMBR:
                break;

            case ButtonCodes.BTN_TRIGGER_HAPPY1:
                state.Paddle1 = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_TRIGGER_HAPPY2:
                state.Paddle2 = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_TRIGGER_HAPPY3:
                state.Paddle3 = pressed;
                state.MarkDirty();
                break;

            case ButtonCodes.BTN_TRIGGER_HAPPY4:
                state.Paddle4 = pressed;
                state.MarkDirty();
                break;

            case 0x13f: // Unknown Scuf button (code 319)
                // Currently ignored
                break;
        }
    }
}