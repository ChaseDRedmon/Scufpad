using Scufpad.Mapping;
using NUnit.Framework;
using Shouldly;

namespace Scufpad.Tests.Unit.Mapping;

[TestFixture]
public class InputStateTests
{
    [SetUp]
    public void SetUp()
    {
        _state = new InputState();
    }

    private InputState _state = null!;

    [Test]
    public void NewInputState_ShouldHaveZeroAnalogValues()
    {
        _state.LeftStickX.ShouldBe(0);
        _state.LeftStickY.ShouldBe(0);
        _state.RightStickX.ShouldBe(0);
        _state.RightStickY.ShouldBe(0);
        _state.LeftTrigger.ShouldBe(0);
        _state.RightTrigger.ShouldBe(0);
        _state.DpadX.ShouldBe(0);
        _state.DpadY.ShouldBe(0);
    }

    [Test]
    public void NewInputState_ShouldHaveAllButtonsFalse()
    {
        _state.ButtonA.ShouldBeFalse();
        _state.ButtonB.ShouldBeFalse();
        _state.ButtonX.ShouldBeFalse();
        _state.ButtonY.ShouldBeFalse();
        _state.BumperLeft.ShouldBeFalse();
        _state.BumperRight.ShouldBeFalse();
        _state.ButtonStart.ShouldBeFalse();
        _state.ButtonSelect.ShouldBeFalse();
        _state.ButtonGuide.ShouldBeFalse();
        _state.ThumbLeft.ShouldBeFalse();
        _state.ThumbRight.ShouldBeFalse();
        _state.Paddle1.ShouldBeFalse();
        _state.Paddle2.ShouldBeFalse();
        _state.Paddle3.ShouldBeFalse();
        _state.Paddle4.ShouldBeFalse();
    }

    [Test]
    public void NewInputState_ShouldNotBeDirty()
    {
        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void LeftStickX_ShouldAcceptNegativeValues()
    {
        _state.LeftStickX = -32768;
        _state.LeftStickX.ShouldBe(-32768);
    }

    [Test]
    public void LeftStickX_ShouldAcceptPositiveValues()
    {
        _state.LeftStickX = 32767;
        _state.LeftStickX.ShouldBe(32767);
    }

    [Test]
    public void LeftStickY_ShouldAcceptFullRange()
    {
        _state.LeftStickY = -32768;
        _state.LeftStickY.ShouldBe(-32768);

        _state.LeftStickY = 32767;
        _state.LeftStickY.ShouldBe(32767);
    }

    [Test]
    public void RightStickX_ShouldAcceptFullRange()
    {
        _state.RightStickX = -32768;
        _state.RightStickX.ShouldBe(-32768);

        _state.RightStickX = 32767;
        _state.RightStickX.ShouldBe(32767);
    }

    [Test]
    public void RightStickY_ShouldAcceptFullRange()
    {
        _state.RightStickY = -32768;
        _state.RightStickY.ShouldBe(-32768);

        _state.RightStickY = 32767;
        _state.RightStickY.ShouldBe(32767);
    }

    [Test]
    public void LeftTrigger_ShouldAcceptZero()
    {
        _state.LeftTrigger = 0;
        _state.LeftTrigger.ShouldBe(0);
    }

    [Test]
    public void LeftTrigger_ShouldAcceptMaxValue()
    {
        _state.LeftTrigger = 1023;
        _state.LeftTrigger.ShouldBe(1023);
    }

    [Test]
    public void RightTrigger_ShouldAcceptFullRange()
    {
        _state.RightTrigger = 0;
        _state.RightTrigger.ShouldBe(0);

        _state.RightTrigger = 1023;
        _state.RightTrigger.ShouldBe(1023);
    }

    [Test]
    public void DpadX_ShouldAcceptNegativeOne()
    {
        _state.DpadX = -1;
        _state.DpadX.ShouldBe(-1);
    }

    [Test]
    public void DpadX_ShouldAcceptZero()
    {
        _state.DpadX = 0;
        _state.DpadX.ShouldBe(0);
    }

    [Test]
    public void DpadX_ShouldAcceptPositiveOne()
    {
        _state.DpadX = 1;
        _state.DpadX.ShouldBe(1);
    }

    [Test]
    public void DpadY_ShouldAcceptAllValidValues()
    {
        _state.DpadY = -1;
        _state.DpadY.ShouldBe(-1);

        _state.DpadY = 0;
        _state.DpadY.ShouldBe(0);

        _state.DpadY = 1;
        _state.DpadY.ShouldBe(1);
    }

    [Test]
    public void FaceButtons_ShouldBeSettable()
    {
        _state.ButtonA = true;
        _state.ButtonB = true;
        _state.ButtonX = true;
        _state.ButtonY = true;

        _state.ButtonA.ShouldBeTrue();
        _state.ButtonB.ShouldBeTrue();
        _state.ButtonX.ShouldBeTrue();
        _state.ButtonY.ShouldBeTrue();
    }

    [Test]
    public void ShoulderButtons_ShouldBeSettable()
    {
        _state.BumperLeft = true;
        _state.BumperRight = true;

        _state.BumperLeft.ShouldBeTrue();
        _state.BumperRight.ShouldBeTrue();
    }

    [Test]
    public void MenuButtons_ShouldBeSettable()
    {
        _state.ButtonStart = true;
        _state.ButtonSelect = true;
        _state.ButtonGuide = true;

        _state.ButtonStart.ShouldBeTrue();
        _state.ButtonSelect.ShouldBeTrue();
        _state.ButtonGuide.ShouldBeTrue();
    }

    [Test]
    public void ThumbButtons_ShouldBeSettable()
    {
        _state.ThumbLeft = true;
        _state.ThumbRight = true;

        _state.ThumbLeft.ShouldBeTrue();
        _state.ThumbRight.ShouldBeTrue();
    }

    [Test]
    public void Paddles_ShouldBeSettable()
    {
        _state.Paddle1 = true;
        _state.Paddle2 = true;
        _state.Paddle3 = true;
        _state.Paddle4 = true;

        _state.Paddle1.ShouldBeTrue();
        _state.Paddle2.ShouldBeTrue();
        _state.Paddle3.ShouldBeTrue();
        _state.Paddle4.ShouldBeTrue();
    }

    [Test]
    public void Buttons_ShouldToggleCorrectly()
    {
        _state.ButtonA = true;
        _state.ButtonA.ShouldBeTrue();

        _state.ButtonA = false;
        _state.ButtonA.ShouldBeFalse();
    }

    [Test]
    public void MarkDirty_ShouldSetIsDirtyToTrue()
    {
        _state.MarkDirty();
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ClearDirty_ShouldSetIsDirtyToFalse()
    {
        _state.MarkDirty();
        _state.IsDirty.ShouldBeTrue();

        _state.ClearDirty();
        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void MarkDirty_ShouldBeIdempotent()
    {
        _state.MarkDirty();
        _state.MarkDirty();
        _state.MarkDirty();

        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ClearDirty_ShouldBeIdempotent()
    {
        _state.ClearDirty();
        _state.ClearDirty();

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void Clone_ShouldCreateNewInstance()
    {
        var clone = _state.Clone();
        clone.ShouldNotBeSameAs(_state);
    }

    [Test]
    public void Clone_ShouldCopyAnalogValues()
    {
        _state.LeftStickX = 1000;
        _state.LeftStickY = -2000;
        _state.RightStickX = 3000;
        _state.RightStickY = -4000;
        _state.LeftTrigger = 500;
        _state.RightTrigger = 750;
        _state.DpadX = -1;
        _state.DpadY = 1;

        var clone = _state.Clone();

        clone.LeftStickX.ShouldBe(1000);
        clone.LeftStickY.ShouldBe(-2000);
        clone.RightStickX.ShouldBe(3000);
        clone.RightStickY.ShouldBe(-4000);
        clone.LeftTrigger.ShouldBe(500);
        clone.RightTrigger.ShouldBe(750);
        clone.DpadX.ShouldBe(-1);
        clone.DpadY.ShouldBe(1);
    }

    [Test]
    public void Clone_ShouldCopyButtonStates()
    {
        _state.ButtonA = true;
        _state.ButtonB = true;
        _state.ButtonX = false;
        _state.ButtonY = true;
        _state.BumperLeft = true;
        _state.BumperRight = false;
        _state.ButtonStart = true;
        _state.ButtonSelect = false;
        _state.ButtonGuide = true;
        _state.ThumbLeft = true;
        _state.ThumbRight = false;
        _state.Paddle1 = true;
        _state.Paddle2 = false;
        _state.Paddle3 = true;
        _state.Paddle4 = false;

        var clone = _state.Clone();

        clone.ButtonA.ShouldBeTrue();
        clone.ButtonB.ShouldBeTrue();
        clone.ButtonX.ShouldBeFalse();
        clone.ButtonY.ShouldBeTrue();
        clone.BumperLeft.ShouldBeTrue();
        clone.BumperRight.ShouldBeFalse();
        clone.ButtonStart.ShouldBeTrue();
        clone.ButtonSelect.ShouldBeFalse();
        clone.ButtonGuide.ShouldBeTrue();
        clone.ThumbLeft.ShouldBeTrue();
        clone.ThumbRight.ShouldBeFalse();
        clone.Paddle1.ShouldBeTrue();
        clone.Paddle2.ShouldBeFalse();
        clone.Paddle3.ShouldBeTrue();
        clone.Paddle4.ShouldBeFalse();
    }

    [Test]
    public void Clone_ShouldNotCopyDirtyFlag()
    {
        _state.MarkDirty();
        var clone = _state.Clone();

        // Clone should start with IsDirty = false (new instance)
        clone.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void Clone_ShouldBeIndependent()
    {
        _state.LeftStickX = 1000;
        _state.ButtonA = true;

        var clone = _state.Clone();

        // Modify original
        _state.LeftStickX = 2000;
        _state.ButtonA = false;

        // Clone should retain original values
        clone.LeftStickX.ShouldBe(1000);
        clone.ButtonA.ShouldBeTrue();
    }

    [Test]
    public void Clone_ModifyingClone_ShouldNotAffectOriginal()
    {
        _state.LeftStickX = 1000;
        _state.ButtonA = true;

        var clone = _state.Clone();

        // Modify clone
        clone.LeftStickX = 5000;
        clone.ButtonA = false;

        // Original should be unchanged
        _state.LeftStickX.ShouldBe(1000);
        _state.ButtonA.ShouldBeTrue();
    }

    [Test]
    public void AllMaxValues_ShouldBeStorable()
    {
        _state.LeftStickX = int.MaxValue;
        _state.LeftStickY = int.MaxValue;
        _state.RightStickX = int.MaxValue;
        _state.RightStickY = int.MaxValue;
        _state.LeftTrigger = int.MaxValue;
        _state.RightTrigger = int.MaxValue;
        _state.DpadX = int.MaxValue;
        _state.DpadY = int.MaxValue;

        _state.LeftStickX.ShouldBe(int.MaxValue);
        _state.RightTrigger.ShouldBe(int.MaxValue);
    }

    [Test]
    public void AllMinValues_ShouldBeStorable()
    {
        _state.LeftStickX = int.MinValue;
        _state.LeftStickY = int.MinValue;
        _state.RightStickX = int.MinValue;
        _state.RightStickY = int.MinValue;
        _state.LeftTrigger = int.MinValue;
        _state.RightTrigger = int.MinValue;
        _state.DpadX = int.MinValue;
        _state.DpadY = int.MinValue;

        _state.LeftStickX.ShouldBe(int.MinValue);
        _state.LeftTrigger.ShouldBe(int.MinValue);
    }
}