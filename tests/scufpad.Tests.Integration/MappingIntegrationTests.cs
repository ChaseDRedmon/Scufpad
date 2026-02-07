using Scufpad.Interop;
using Scufpad.Mapping;
using NUnit.Framework;
using Shouldly;

namespace Scufpad.Tests.Integration;

/// <summary>
///     Integration tests for the input mapping pipeline.
///     These tests verify that components work correctly together.
/// </summary>
[TestFixture]
public class MappingIntegrationTests
{
    [Test]
    public void FullInputPipeline_ShouldProcessEvdevEventsCorrectly()
    {
        // Setup
        var rawState = new InputState();
        var filteredState = new InputState();
        var filter = new InputFilter(3000, 10, 200);

        // Simulate a sequence of evdev events (like a real controller would produce)
        var events = new[]
        {
            // Left stick movement
            CreateAbsEvent(AbsCodes.ABS_X, 15000),
            CreateAbsEvent(AbsCodes.ABS_Y, -10000),

            // Right stick movement
            CreateAbsEvent(AbsCodes.ABS_Z, 8000), // Right X on Envision
            CreateAbsEvent(AbsCodes.ABS_RZ, -5000), // Right Y on Envision

            // Triggers
            CreateAbsEvent(AbsCodes.ABS_RX, 500), // Left trigger on Envision

            // Buttons
            CreateKeyEvent(ButtonCodes.BTN_SOUTH, 1), // A pressed
            CreateKeyEvent(ButtonCodes.BTN_Z, 1), // RB pressed (V2 uses BTN_Z for RB)

            // Sync
            CreateSynEvent()
        };

        // Process all events
        for (var i = 0; i < events.Length; i++)
        {
            EnvisionMapping.ProcessEvdevEvent(in events[i], rawState);
        }

        // Apply filter
        filter.Apply(rawState, filteredState);

        // Verify final state
        filteredState.LeftStickX.ShouldBe(15000);
        filteredState.LeftStickY.ShouldBe(-10000);
        filteredState.RightStickX.ShouldBe(8000);
        filteredState.RightStickY.ShouldBe(-5000);
        filteredState.LeftTrigger.ShouldBe(500);
        filteredState.ButtonA.ShouldBeTrue();
        filteredState.BumperRight.ShouldBeTrue();
    }

    [Test]
    public void FullInputPipeline_ShouldApplyRadialDeadzoneCorrectly()
    {
        var rawState = new InputState();
        var filteredState = new InputState();
        var filter = new InputFilter(5000);

        // Radial deadzone: magnitude = sqrt(x^2 + y^2) must exceed threshold
        // With X=4000, Y=6000: magnitude = sqrt(16M + 36M) = sqrt(52M) ≈ 7211 > 5000
        // So both values pass through (stick is outside radial deadzone)
        var evX = CreateAbsEvent(AbsCodes.ABS_X, 4000);
        var evY = CreateAbsEvent(AbsCodes.ABS_Y, 6000);
        var evSyn = CreateSynEvent();

        EnvisionMapping.ProcessEvdevEvent(in evX, rawState);
        EnvisionMapping.ProcessEvdevEvent(in evY, rawState);
        EnvisionMapping.ProcessEvdevEvent(in evSyn, rawState);

        filter.Apply(rawState, filteredState);

        // Both pass through because combined magnitude exceeds deadzone
        filteredState.LeftStickX.ShouldBe(4000);
        filteredState.LeftStickY.ShouldBe(6000);
    }

    [Test]
    public void FullInputPipeline_ShouldFilterWhenInsideRadialDeadzone()
    {
        var rawState = new InputState();
        var filteredState = new InputState();
        var filter = new InputFilter(5000);

        // With X=3000, Y=3000: magnitude = sqrt(9M + 9M) = sqrt(18M) ≈ 4243 < 5000
        // So both values are zeroed (stick is inside radial deadzone)
        var evX = CreateAbsEvent(AbsCodes.ABS_X, 3000);
        var evY = CreateAbsEvent(AbsCodes.ABS_Y, 3000);
        var evSyn = CreateSynEvent();

        EnvisionMapping.ProcessEvdevEvent(in evX, rawState);
        EnvisionMapping.ProcessEvdevEvent(in evY, rawState);
        EnvisionMapping.ProcessEvdevEvent(in evSyn, rawState);

        filter.Apply(rawState, filteredState);

        // Both zeroed because combined magnitude is within deadzone
        filteredState.LeftStickX.ShouldBe(0);
        filteredState.LeftStickY.ShouldBe(0);
    }

    [Test]
    public void FullInputPipeline_ShouldUseEvdevForBothTriggers()
    {
        var rawState = new InputState();
        var filteredState = new InputState();
        var filter = new InputFilter();

        // Evdev events - V2 hardware provides both triggers via evdev
        var evLt = CreateAbsEvent(AbsCodes.ABS_RX, 800); // Left trigger from evdev
        var evRt = CreateAbsEvent(AbsCodes.ABS_RY, 600); // Right trigger from evdev (V2)
        var evA = CreateKeyEvent(ButtonCodes.BTN_SOUTH, 1);
        var evSyn = CreateSynEvent();

        EnvisionMapping.ProcessEvdevEvent(in evLt, rawState);
        EnvisionMapping.ProcessEvdevEvent(in evRt, rawState);
        EnvisionMapping.ProcessEvdevEvent(in evA, rawState);
        EnvisionMapping.ProcessEvdevEvent(in evSyn, rawState);

        filter.Apply(rawState, filteredState);

        filteredState.LeftTrigger.ShouldBe(800);
        filteredState.RightTrigger.ShouldBe(600); // Evdev value, not hidraw
        filteredState.ButtonA.ShouldBeTrue();
    }

    [Test]
    public void ButtonSequence_PressAndRelease_ShouldTrackCorrectly()
    {
        var state = new InputState();

        // Press A
        var pressA = CreateKeyEvent(ButtonCodes.BTN_SOUTH, 1);
        EnvisionMapping.ProcessEvdevEvent(in pressA, state);
        state.ButtonA.ShouldBeTrue();

        // Press B (A still held)
        var pressB = CreateKeyEvent(ButtonCodes.BTN_EAST, 1);
        EnvisionMapping.ProcessEvdevEvent(in pressB, state);
        state.ButtonA.ShouldBeTrue();
        state.ButtonB.ShouldBeTrue();

        // Release A (B still held)
        var releaseA = CreateKeyEvent(ButtonCodes.BTN_SOUTH, 0);
        EnvisionMapping.ProcessEvdevEvent(in releaseA, state);
        state.ButtonA.ShouldBeFalse();
        state.ButtonB.ShouldBeTrue();

        // Release B
        var releaseB = CreateKeyEvent(ButtonCodes.BTN_EAST, 0);
        EnvisionMapping.ProcessEvdevEvent(in releaseB, state);
        state.ButtonA.ShouldBeFalse();
        state.ButtonB.ShouldBeFalse();
    }

    [Test]
    public void AllFaceButtons_SimultaneousPress_ShouldWork()
    {
        var state = new InputState();

        // Press all face buttons (V2 uses BTN_C for X, not BTN_WEST)
        var evA = CreateKeyEvent(ButtonCodes.BTN_SOUTH, 1);
        var evB = CreateKeyEvent(ButtonCodes.BTN_EAST, 1);
        var evX = CreateKeyEvent(ButtonCodes.BTN_C, 1);
        var evY = CreateKeyEvent(ButtonCodes.BTN_NORTH, 1);

        EnvisionMapping.ProcessEvdevEvent(in evA, state);
        EnvisionMapping.ProcessEvdevEvent(in evB, state);
        EnvisionMapping.ProcessEvdevEvent(in evX, state);
        EnvisionMapping.ProcessEvdevEvent(in evY, state);

        state.ButtonA.ShouldBeTrue();
        state.ButtonB.ShouldBeTrue();
        state.ButtonX.ShouldBeTrue();
        state.ButtonY.ShouldBeTrue();
    }

    [Test]
    public void AllPaddles_SimultaneousPress_ShouldWork()
    {
        var state = new InputState();

        var ev1 = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY1, 1);
        var ev2 = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY2, 1);
        var ev3 = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY3, 1);
        var ev4 = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY4, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev1, state);
        EnvisionMapping.ProcessEvdevEvent(in ev2, state);
        EnvisionMapping.ProcessEvdevEvent(in ev3, state);
        EnvisionMapping.ProcessEvdevEvent(in ev4, state);

        state.Paddle1.ShouldBeTrue();
        state.Paddle2.ShouldBeTrue();
        state.Paddle3.ShouldBeTrue();
        state.Paddle4.ShouldBeTrue();
    }

    [Test]
    public void StickMovement_CircularPattern_ShouldTrack()
    {
        var state = new InputState();
        var filter = new InputFilter(0, jitterThreshold: 0);
        var output = new InputState();

        // Simulate circular stick movement (8 positions)
        var positions = new (int x, int y)[]
        {
            (32767, 0), // Right
            (23170, 23170), // Right-Down
            (0, 32767), // Down
            (-23170, 23170), // Left-Down
            (-32768, 0), // Left
            (-23170, -23170), // Left-Up
            (0, -32768), // Up
            (23170, -23170) // Right-Up
        };

        foreach (var (x, y) in positions)
        {
            var evX = CreateAbsEvent(AbsCodes.ABS_X, x);
            var evY = CreateAbsEvent(AbsCodes.ABS_Y, y);
            EnvisionMapping.ProcessEvdevEvent(in evX, state);
            EnvisionMapping.ProcessEvdevEvent(in evY, state);

            filter.Apply(state, output);

            output.LeftStickX.ShouldBe(x);
            output.LeftStickY.ShouldBe(y);
        }
    }

    [Test]
    public void TriggerSweep_ShouldTrackSmoothly()
    {
        var state = new InputState();
        var filter = new InputFilter(triggerDeadzone: 0, jitterThreshold: 0);
        var output = new InputState();

        // Sweep from 0 to 1023
        for (var i = 0; i <= 1023; i += 100)
        {
            var ev = CreateAbsEvent(AbsCodes.ABS_RX, i);
            EnvisionMapping.ProcessEvdevEvent(in ev, state);
            filter.Apply(state, output);
            output.LeftTrigger.ShouldBe(i);
        }
    }

    [Test]
    public void Dpad_AllDirections_ShouldWork()
    {
        var state = new InputState();

        // Test all 8 directions plus center
        var directions = new (int x, int y, string name)[]
        {
            (0, 0, "Center"),
            (1, 0, "Right"),
            (-1, 0, "Left"),
            (0, 1, "Down"),
            (0, -1, "Up"),
            (1, 1, "Down-Right"),
            (1, -1, "Up-Right"),
            (-1, 1, "Down-Left"),
            (-1, -1, "Up-Left")
        };

        foreach (var (x, y, _) in directions)
        {
            var evX = CreateAbsEvent(AbsCodes.ABS_HAT0X, x);
            var evY = CreateAbsEvent(AbsCodes.ABS_HAT0Y, y);
            EnvisionMapping.ProcessEvdevEvent(in evX, state);
            EnvisionMapping.ProcessEvdevEvent(in evY, state);

            state.DpadX.ShouldBe(x);
            state.DpadY.ShouldBe(y);
        }
    }

    [Test]
    public void JitterFilter_ShouldStabilizeNoisyInput()
    {
        var state = new InputState();
        var filter = new InputFilter(0, jitterThreshold: 500);
        var output = new InputState();

        // Set initial position
        var evInit = CreateAbsEvent(AbsCodes.ABS_X, 20000);
        EnvisionMapping.ProcessEvdevEvent(in evInit, state);
        filter.Apply(state, output);
        output.LeftStickX.ShouldBe(20000);

        // Simulate jitter (small fluctuations)
        var jitteryValues = new[] { 20100, 19950, 20050, 20150, 19900, 20000 };
        foreach (var value in jitteryValues)
        {
            var ev = CreateAbsEvent(AbsCodes.ABS_X, value);
            EnvisionMapping.ProcessEvdevEvent(in ev, state);
            filter.Apply(state, output);
            output.LeftStickX.ShouldBe(20000); // All should be filtered
        }

        // Real movement should pass through
        var evMove = CreateAbsEvent(AbsCodes.ABS_X, 25000);
        EnvisionMapping.ProcessEvdevEvent(in evMove, state);
        filter.Apply(state, output);
        output.LeftStickX.ShouldBe(25000);
    }

    [Test]
    public void StateClone_ShouldCaptureSnapshot()
    {
        var state = new InputState
        {
            LeftStickX = 15000,
            ButtonA = true,
            Paddle1 = true
        };

        var snapshot = state.Clone();

        // Modify original
        state.LeftStickX = 0;
        state.ButtonA = false;
        state.Paddle1 = false;

        // Snapshot should be unchanged
        snapshot.LeftStickX.ShouldBe(15000);
        snapshot.ButtonA.ShouldBeTrue();
        snapshot.Paddle1.ShouldBeTrue();
    }

    private static InputEvent CreateAbsEvent(ushort code, int value)
    {
        return new InputEvent
        {
            Type = EventTypes.EV_ABS,
            Code = code,
            Value = value
        };
    }

    private static InputEvent CreateKeyEvent(ushort code, int value)
    {
        return new InputEvent
        {
            Type = EventTypes.EV_KEY,
            Code = code,
            Value = value
        };
    }

    private static InputEvent CreateSynEvent()
    {
        return new InputEvent
        {
            Type = EventTypes.EV_SYN,
            Code = SynCodes.SYN_REPORT,
            Value = 0
        };
    }
}