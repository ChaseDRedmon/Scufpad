namespace Scufpad.Mapping;

/// <summary>
///     Represents the complete state of a gamepad at a point in time.
///     Used to pass controller state between the input, mapping, filtering, and output stages.
/// </summary>
/// <remarks>
///     This class is mutable and designed to be reused across multiple frames to avoid allocations.
///     The <see cref="IsDirty" /> flag tracks whether the state has changed since the last update.
/// </remarks>
internal sealed class InputState
{
    /// <summary>
    ///     Creates a shallow copy of this state.
    ///     Useful for comparing previous and current states.
    /// </summary>
    /// <returns>A new <see cref="InputState" /> with the same values (dirty flag is NOT copied).</returns>
    /// <remarks>
    ///     Uses MemberwiseClone for maintainability - automatically includes any new fields.
    ///     The dirty flag is reset to false in the clone.
    /// </remarks>
    public InputState Clone()
    {
        var clone = (InputState)MemberwiseClone();
        clone.IsDirty = false;
        return clone;
    }

    /// <summary>
    ///     Copies all values from this state to the target state.
    /// </summary>
    /// <param name="target">The target state to copy values to.</param>
    public void CopyTo(InputState target)
    {
        target.LeftStickX = LeftStickX;
        target.LeftStickY = LeftStickY;
        target.RightStickX = RightStickX;
        target.RightStickY = RightStickY;
        target.LeftTrigger = LeftTrigger;
        target.RightTrigger = RightTrigger;
        target.DpadX = DpadX;
        target.DpadY = DpadY;
        target.ButtonA = ButtonA;
        target.ButtonB = ButtonB;
        target.ButtonX = ButtonX;
        target.ButtonY = ButtonY;
        target.BumperLeft = BumperLeft;
        target.BumperRight = BumperRight;
        target.ButtonStart = ButtonStart;
        target.ButtonSelect = ButtonSelect;
        target.ButtonGuide = ButtonGuide;
        target.ThumbLeft = ThumbLeft;
        target.ThumbRight = ThumbRight;
        target.Paddle1 = Paddle1;
        target.Paddle2 = Paddle2;
        target.Paddle3 = Paddle3;
        target.Paddle4 = Paddle4;
    }

    /// <summary>
    ///     Left stick X-axis position.
    ///     Range: -32768 (full left) to 32767 (full right), 0 is center.
    /// </summary>
    public int LeftStickX { get; set; }

    /// <summary>
    ///     Left stick Y-axis position.
    ///     Range: -32768 (full up) to 32767 (full down), 0 is center.
    /// </summary>
    public int LeftStickY { get; set; }

    /// <summary>
    ///     Right stick X-axis position.
    ///     Range: -32768 (full left) to 32767 (full right), 0 is center.
    /// </summary>
    public int RightStickX { get; set; }

    /// <summary>
    ///     Right stick Y-axis position.
    ///     Range: -32768 (full up) to 32767 (full down), 0 is center.
    /// </summary>
    public int RightStickY { get; set; }

    /// <summary>
    ///     Left trigger (LT/L2) position.
    ///     Range: 0 (released) to 1023 (fully pressed).
    /// </summary>
    public int LeftTrigger { get; set; }

    /// <summary>
    ///     Right trigger (RT/R2) position.
    ///     Range: 0 (released) to 1023 (fully pressed).
    /// </summary>
    public int RightTrigger { get; set; }

    /// <summary>
    ///     D-pad X-axis.
    ///     Values: -1 (left), 0 (center), 1 (right).
    /// </summary>
    public int DpadX { get; set; }

    /// <summary>
    ///     D-pad Y-axis.
    ///     Values: -1 (up), 0 (center), 1 (down).
    /// </summary>
    public int DpadY { get; set; }

    /// <summary>A button (Xbox) / Cross (PlayStation) state.</summary>
    public bool ButtonA { get; set; }

    /// <summary>B button (Xbox) / Circle (PlayStation) state.</summary>
    public bool ButtonB { get; set; }

    /// <summary>X button (Xbox) / Square (PlayStation) state.</summary>
    public bool ButtonX { get; set; }

    /// <summary>Y button (Xbox) / Triangle (PlayStation) state.</summary>
    public bool ButtonY { get; set; }

    /// <summary>Left bumper (LB/L1) state.</summary>
    public bool BumperLeft { get; set; }

    /// <summary>Right bumper (RB/R1) state.</summary>
    public bool BumperRight { get; set; }

    /// <summary>Start / Options button state.</summary>
    public bool ButtonStart { get; set; }

    /// <summary>Back / Select / Share button state.</summary>
    public bool ButtonSelect { get; set; }

    /// <summary>Guide / Home / Xbox button state.</summary>
    public bool ButtonGuide { get; set; }

    /// <summary>Left stick click (L3/LS) state.</summary>
    public bool ThumbLeft { get; set; }

    /// <summary>Right stick click (R3/RS) state.</summary>
    public bool ThumbRight { get; set; }

    /// <summary>Paddle 1 (P1) state.</summary>
    public bool Paddle1 { get; set; }

    /// <summary>Paddle 2 (P2) state.</summary>
    public bool Paddle2 { get; set; }

    /// <summary>Paddle 3 (P3) state.</summary>
    public bool Paddle3 { get; set; }

    /// <summary>Paddle 4 (P4) state. Note: Scuf V2 only has 3 paddles.</summary>
    public bool Paddle4 { get; set; }

    /// <summary>
    ///     Gets whether the state has been modified since the last <see cref="ClearDirty" /> call.
    ///     Used to determine if the virtual gamepad needs to emit new events.
    /// </summary>
    public bool IsDirty { get; private set; }

    /// <summary>
    ///     Marks the state as modified. Called by mapping code when any value changes.
    /// </summary>
    public void MarkDirty()
    {
        IsDirty = true;
    }

    /// <summary>
    ///     Clears the dirty flag. Called after the state has been processed and emitted.
    /// </summary>
    public void ClearDirty()
    {
        IsDirty = false;
    }
}