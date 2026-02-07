using Scufpad.Mapping;
using NUnit.Framework;
using Shouldly;

namespace Scufpad.Tests.Unit.Mapping;

[TestFixture]
public class InputFilterTests
{
    [SetUp]
    public void SetUp()
    {
        _filter = new InputFilter();
        _input = new InputState();
        _output = new InputState();
    }

    private InputFilter _filter = null!;

    private InputState _input = null!;

    private InputState _output = null!;

    [Test]
    public void DefaultConstructor_ShouldUseDefaultDeadzones()
    {
        // Default stick deadzone is 3500
        _input.LeftStickX = 3499;
        _filter.Apply(_input, _output);

        // Value within deadzone should become 0
        _output.LeftStickX.ShouldBe(0);
    }

    [Test]
    public void DefaultConstructor_StickValueAboveDeadzone_ShouldPassThrough()
    {
        _input.LeftStickX = 3501;
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(3501);
    }

    [Test]
    public void CustomStickDeadzone_ShouldBeRespected()
    {
        var filter = new InputFilter(1000);
        _input.LeftStickX = 999;

        filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(0);
    }

    [Test]
    public void CustomStickDeadzone_ValueAbove_ShouldPassThrough()
    {
        var filter = new InputFilter(1000);
        _input.LeftStickX = 1001;

        filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(1001);
    }

    [Test]
    public void CustomTriggerDeadzone_ShouldBeRespected()
    {
        var filter = new InputFilter(triggerDeadzone: 50);
        _input.LeftTrigger = 49;

        filter.Apply(_input, _output);

        _output.LeftTrigger.ShouldBe(0);
    }

    [Test]
    public void CustomTriggerDeadzone_ValueAbove_ShouldPassThrough()
    {
        var filter = new InputFilter(triggerDeadzone: 50, jitterThreshold: 0);
        _input.LeftTrigger = 51;

        filter.Apply(_input, _output);

        _output.LeftTrigger.ShouldBe(51);
    }

    [Test]
    public void ZeroDeadzone_ShouldPassThroughAllValues()
    {
        var filter = new InputFilter(0, 0, 0, 0);
        _input.LeftStickX = 1;
        _input.LeftTrigger = 1;

        filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(1);
        _output.LeftTrigger.ShouldBe(1);
    }

    [Test]
    public void LeftStickX_WithinDeadzone_ShouldBeZero()
    {
        _input.LeftStickX = 2000;
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(0);
    }

    [Test]
    public void LeftStickX_NegativeWithinDeadzone_ShouldBeZero()
    {
        _input.LeftStickX = -2000;
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(0);
    }

    [Test]
    public void LeftStickX_ExactlyAtDeadzone_ShouldBeZero()
    {
        _input.LeftStickX = 3499; // Just under default 3500
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(0);
    }

    [Test]
    public void LeftStickY_WithinDeadzone_ShouldBeZero()
    {
        _input.LeftStickY = 1500;
        _filter.Apply(_input, _output);

        _output.LeftStickY.ShouldBe(0);
    }

    [Test]
    public void RightStickX_WithinDeadzone_ShouldBeZero()
    {
        _input.RightStickX = -3000;
        _filter.Apply(_input, _output);

        _output.RightStickX.ShouldBe(0);
    }

    [Test]
    public void RightStickY_WithinDeadzone_ShouldBeZero()
    {
        _input.RightStickY = 2500;
        _filter.Apply(_input, _output);

        _output.RightStickY.ShouldBe(0);
    }

    [Test]
    public void AllSticks_OutsideDeadzone_ShouldPassThrough()
    {
        _input.LeftStickX = 10000;
        _input.LeftStickY = -15000;
        _input.RightStickX = 20000;
        _input.RightStickY = -25000;

        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(10000);
        _output.LeftStickY.ShouldBe(-15000);
        _output.RightStickX.ShouldBe(20000);
        _output.RightStickY.ShouldBe(-25000);
    }

    [Test]
    public void LeftTrigger_WithinDeadzone_ShouldBeZero()
    {
        _input.LeftTrigger = 5; // Default deadzone is 10
        _filter.Apply(_input, _output);

        _output.LeftTrigger.ShouldBe(0);
    }

    [Test]
    public void LeftTrigger_AboveDeadzone_ShouldPassThrough()
    {
        _input.LeftTrigger = 100;
        _filter.Apply(_input, _output);

        _output.LeftTrigger.ShouldBe(100);
    }

    [Test]
    public void RightTrigger_WithinDeadzone_ShouldBeZero()
    {
        _input.RightTrigger = 9;
        _filter.Apply(_input, _output);

        _output.RightTrigger.ShouldBe(0);
    }

    [Test]
    public void RightTrigger_AboveDeadzone_ShouldPassThrough()
    {
        _input.RightTrigger = 500;
        _filter.Apply(_input, _output);

        _output.RightTrigger.ShouldBe(500);
    }

    [Test]
    public void StickJitter_SmallChange_ShouldBeFiltered()
    {
        // First apply to set previous value
        _input.LeftStickX = 10000;
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(10000);

        // Small change should be filtered (default jitter threshold is 300)
        _input.LeftStickX = 10100;
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(10000); // Should keep previous value
    }

    [Test]
    public void StickJitter_LargeChange_ShouldPassThrough()
    {
        _input.LeftStickX = 10000;
        _filter.Apply(_input, _output);

        _input.LeftStickX = 10500; // Change > 300
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(10500);
    }

    [Test]
    public void TriggerJitter_SmallChange_ShouldBeFiltered()
    {
        _input.LeftTrigger = 500;
        _filter.Apply(_input, _output);

        // Default trigger jitter threshold is 20 units
        _input.LeftTrigger = 510;
        _filter.Apply(_input, _output);
        _output.LeftTrigger.ShouldBe(500);
    }

    [Test]
    public void TriggerJitter_LargeChange_ShouldPassThrough()
    {
        _input.LeftTrigger = 500;
        _filter.Apply(_input, _output);

        _input.LeftTrigger = 530; // Change > 20
        _filter.Apply(_input, _output);
        _output.LeftTrigger.ShouldBe(530);
    }

    [Test]
    public void CustomTriggerJitterThreshold_ShouldBeRespected()
    {
        var filter = new InputFilter(triggerJitterThreshold: 50);
        _input.LeftTrigger = 500;
        filter.Apply(_input, _output);

        _input.LeftTrigger = 530; // Change < 50
        filter.Apply(_input, _output);
        _output.LeftTrigger.ShouldBe(500); // Filtered

        _input.LeftTrigger = 560; // Change > 50
        filter.Apply(_input, _output);
        _output.LeftTrigger.ShouldBe(560); // Passed through
    }

    [Test]
    public void CustomJitterThreshold_ShouldBeRespected()
    {
        var filter = new InputFilter(0, jitterThreshold: 100);
        _input.LeftStickX = 10000;
        filter.Apply(_input, _output);

        _input.LeftStickX = 10050; // Change < 100
        filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(10000); // Filtered

        _input.LeftStickX = 10150; // Change > 100
        filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(10150); // Passed through
    }

    [Test]
    public void FaceButtons_ShouldPassThroughDirectly()
    {
        _input.ButtonA = true;
        _input.ButtonB = true;
        _input.ButtonX = false;
        _input.ButtonY = true;

        _filter.Apply(_input, _output);

        _output.ButtonA.ShouldBeTrue();
        _output.ButtonB.ShouldBeTrue();
        _output.ButtonX.ShouldBeFalse();
        _output.ButtonY.ShouldBeTrue();
    }

    [Test]
    public void ShoulderButtons_ShouldPassThroughDirectly()
    {
        _input.BumperLeft = true;
        _input.BumperRight = false;

        _filter.Apply(_input, _output);

        _output.BumperLeft.ShouldBeTrue();
        _output.BumperRight.ShouldBeFalse();
    }

    [Test]
    public void MenuButtons_ShouldPassThroughDirectly()
    {
        _input.ButtonStart = true;
        _input.ButtonSelect = true;
        _input.ButtonGuide = true;

        _filter.Apply(_input, _output);

        _output.ButtonStart.ShouldBeTrue();
        _output.ButtonSelect.ShouldBeTrue();
        _output.ButtonGuide.ShouldBeTrue();
    }

    [Test]
    public void ThumbButtons_ShouldPassThroughDirectly()
    {
        _input.ThumbLeft = true;
        _input.ThumbRight = false;

        _filter.Apply(_input, _output);

        _output.ThumbLeft.ShouldBeTrue();
        _output.ThumbRight.ShouldBeFalse();
    }

    [Test]
    public void Paddles_ShouldPassThroughDirectly()
    {
        _input.Paddle1 = true;
        _input.Paddle2 = false;
        _input.Paddle3 = true;
        _input.Paddle4 = false;

        _filter.Apply(_input, _output);

        _output.Paddle1.ShouldBeTrue();
        _output.Paddle2.ShouldBeFalse();
        _output.Paddle3.ShouldBeTrue();
        _output.Paddle4.ShouldBeFalse();
    }

    [Test]
    public void Dpad_ShouldPassThroughDirectly()
    {
        _input.DpadX = -1;
        _input.DpadY = 1;

        _filter.Apply(_input, _output);

        _output.DpadX.ShouldBe(-1);
        _output.DpadY.ShouldBe(1);
    }

    [Test]
    public void Reset_ShouldClearPreviousValues()
    {
        // Set up some previous state
        _input.LeftStickX = 20000;
        _filter.Apply(_input, _output);

        // Reset the filter
        _filter.Reset();

        // Small change from 0 should pass through after reset
        _input.LeftStickX = 4000;
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(4000);
    }

    [Test]
    public void Reset_ShouldAllowRefiltering()
    {
        _input.LeftTrigger = 800;
        _filter.Apply(_input, _output);

        _filter.Reset();

        // After reset, small values should be in deadzone again
        _input.LeftTrigger = 5;
        _filter.Apply(_input, _output);
        _output.LeftTrigger.ShouldBe(0);
    }

    [Test]
    public void Apply_ShouldMarkOutputDirty()
    {
        _output.ClearDirty();

        _filter.Apply(_input, _output);

        _output.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void Apply_MultipleTime_ShouldAlwaysMarkDirty()
    {
        _filter.Apply(_input, _output);
        _output.ClearDirty();

        _filter.Apply(_input, _output);

        _output.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void MaxStickValues_ShouldPassThrough()
    {
        _input.LeftStickX = 32767;
        _input.LeftStickY = -32768;

        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(32767);
        _output.LeftStickY.ShouldBe(-32768);
    }

    [Test]
    public void MaxTriggerValues_ShouldPassThrough()
    {
        _input.LeftTrigger = 1023;
        _input.RightTrigger = 1023;

        _filter.Apply(_input, _output);

        _output.LeftTrigger.ShouldBe(1023);
        _output.RightTrigger.ShouldBe(1023);
    }

    [Test]
    public void ZeroValues_ShouldRemainZero()
    {
        _input.LeftStickX = 0;
        _input.LeftStickY = 0;
        _input.LeftTrigger = 0;

        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(0);
        _output.LeftStickY.ShouldBe(0);
        _output.LeftTrigger.ShouldBe(0);
    }

    [Test]
    public void TransitionFromDeadzoneToActive_ShouldWork()
    {
        // Start in deadzone
        _input.LeftStickX = 1000;
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(0);

        // Move out of deadzone
        _input.LeftStickX = 10000;
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(10000);
    }

    [Test]
    public void TransitionFromActiveToDeadzone_ShouldWork()
    {
        // Start active
        _input.LeftStickX = 20000;
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(20000);

        // Move into deadzone
        _input.LeftStickX = 1000;
        _filter.Apply(_input, _output);
        _output.LeftStickX.ShouldBe(0);
    }

    [Test]
    public void LeftStickAxes_UseRadialDeadzone()
    {
        // With radial deadzone, magnitude = sqrt(x^2 + y^2) determines if stick is in deadzone
        // X=20000, Y=1000: magnitude ≈ 20025 > 3500, so both pass through
        _input.LeftStickX = 20000;
        _input.LeftStickY = 1000;
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(20000);
        _output.LeftStickY.ShouldBe(1000); // Passes through due to radial deadzone
    }

    [Test]
    public void LeftAndRightSticks_AreIndependent()
    {
        // Left and right sticks have independent radial deadzones
        // Left: X=20000, Y=0 -> magnitude=20000 > 3500 (passes)
        // Right: X=1000, Y=0 -> magnitude=1000 < 3500 (zeroed)
        _input.LeftStickX = 20000;
        _input.RightStickX = 1000;
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(20000);
        _output.RightStickX.ShouldBe(0); // Inside radial deadzone
    }

    [Test]
    public void Triggers_ShouldBeIndependent()
    {
        _input.LeftTrigger = 500;
        _input.RightTrigger = 5; // In deadzone
        _filter.Apply(_input, _output);

        _output.LeftTrigger.ShouldBe(500);
        _output.RightTrigger.ShouldBe(0);
    }

    [Test]
    public void RadialDeadzone_BothAxesSmall_ShouldZeroBoth()
    {
        // Both axes small but combined magnitude still in deadzone
        // sqrt(2000^2 + 2000^2) = sqrt(8M) ≈ 2828 < 3500
        _input.LeftStickX = 2000;
        _input.LeftStickY = 2000;
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(0);
        _output.LeftStickY.ShouldBe(0);
    }

    [Test]
    public void RadialDeadzone_CombinedMagnitudeExceedsDeadzone_ShouldPassBoth()
    {
        // Each axis individually would be in deadzone (3000 < 3500)
        // But combined magnitude sqrt(3000^2 + 3000^2) ≈ 4243 > 3500
        _input.LeftStickX = 3000;
        _input.LeftStickY = 3000;
        _filter.Apply(_input, _output);

        _output.LeftStickX.ShouldBe(3000);
        _output.LeftStickY.ShouldBe(3000);
    }

    [Test]
    public void RadialDeadzone_DiagonalMovement_ShouldPassBothWhenOutside()
    {
        // Diagonal movement: X and Y both moderate, combined exceeds deadzone
        // sqrt(2500^2 + 2500^2) ≈ 3536 > 3500
        _input.RightStickX = 2500;
        _input.RightStickY = 2500;
        _filter.Apply(_input, _output);

        _output.RightStickX.ShouldBe(2500);
        _output.RightStickY.ShouldBe(2500);
    }
}