using Scufpad.Interop;
using Scufpad.Mapping;
using NUnit.Framework;
using Shouldly;

namespace Scufpad.Tests.Unit.Mapping;

[TestFixture]
public class EnvisionMappingTests
{
    [SetUp]
    public void SetUp()
    {
        _state = new InputState();
    }

    private InputState _state = null!;

    [Test]
    public void ProcessEvdevEvent_ABS_X_ShouldSetLeftStickX()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_X, 15000);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.LeftStickX.ShouldBe(15000);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_X_ShouldHandleNegativeValues()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_X, -20000);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.LeftStickX.ShouldBe(-20000);
    }

    [Test]
    public void ProcessEvdevEvent_ABS_Y_ShouldSetLeftStickY()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_Y, -10000);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.LeftStickY.ShouldBe(-10000);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_Y_ShouldHandleMaxValue()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_Y, 32767);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.LeftStickY.ShouldBe(32767);
    }

    [Test]
    public void ProcessEvdevEvent_ABS_Z_ShouldSetRightStickX()
    {
        // Envision reports right stick X on ABS_Z instead of ABS_RX
        var ev = CreateAbsEvent(AbsCodes.ABS_Z, 25000);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.RightStickX.ShouldBe(25000);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_RZ_ShouldSetRightStickY()
    {
        // Envision reports right stick Y on ABS_RZ instead of ABS_RY
        var ev = CreateAbsEvent(AbsCodes.ABS_RZ, -15000);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.RightStickY.ShouldBe(-15000);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_RX_ShouldSetLeftTrigger()
    {
        // Envision reports left trigger on ABS_RX instead of ABS_Z
        var ev = CreateAbsEvent(AbsCodes.ABS_RX, 512);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.LeftTrigger.ShouldBe(512);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_RX_ShouldHandleFullTriggerPress()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_RX, 1023);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.LeftTrigger.ShouldBe(1023);
    }

    [Test]
    public void ProcessEvdevEvent_ABS_RY_ShouldSetRightTrigger()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_RY, 800);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.RightTrigger.ShouldBe(800);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_HAT0X_ShouldSetDpadX()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_HAT0X, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.DpadX.ShouldBe(1);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_HAT0X_NegativeOne_ShouldSetDpadLeft()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_HAT0X, -1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.DpadX.ShouldBe(-1);
    }

    [Test]
    public void ProcessEvdevEvent_ABS_HAT0X_Zero_ShouldSetDpadCentered()
    {
        _state.DpadX = 1; // Start with a value
        var ev = CreateAbsEvent(AbsCodes.ABS_HAT0X, 0);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.DpadX.ShouldBe(0);
    }

    [Test]
    public void ProcessEvdevEvent_ABS_HAT0Y_ShouldSetDpadY()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_HAT0Y, -1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.DpadY.ShouldBe(-1);
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_ABS_HAT0Y_PositiveOne_ShouldSetDpadDown()
    {
        var ev = CreateAbsEvent(AbsCodes.ABS_HAT0Y, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.DpadY.ShouldBe(1);
    }

    [Test]
    public void ProcessEvdevEvent_BTN_SOUTH_Pressed_ShouldSetButtonA()
    {
        var ev = CreateKeyEvent(ButtonCodes.BTN_SOUTH, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonA.ShouldBeTrue();
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_SOUTH_Released_ShouldClearButtonA()
    {
        _state.ButtonA = true;
        var ev = CreateKeyEvent(ButtonCodes.BTN_SOUTH, 0);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonA.ShouldBeFalse();
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_EAST_Pressed_ShouldSetButtonB()
    {
        var ev = CreateKeyEvent(ButtonCodes.BTN_EAST, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonB.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_EAST_Released_ShouldClearButtonB()
    {
        _state.ButtonB = true;
        var ev = CreateKeyEvent(ButtonCodes.BTN_EAST, 0);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonB.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_NORTH_Pressed_ShouldSetButtonY()
    {
        // BTN_NORTH maps to Y in Xbox naming convention
        var ev = CreateKeyEvent(ButtonCodes.BTN_NORTH, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonY.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_C_Pressed_ShouldSetButtonX()
    {
        // V2: BTN_C maps to X button (non-standard)
        var ev = CreateKeyEvent(ButtonCodes.BTN_C, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonX.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_WEST_Pressed_ShouldSetBumperLeft()
    {
        // V2: BTN_WEST maps to LB (non-standard)
        var ev = CreateKeyEvent(ButtonCodes.BTN_WEST, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.BumperLeft.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_Z_Pressed_ShouldSetBumperRight()
    {
        // V2: BTN_Z maps to RB (non-standard)
        var ev = CreateKeyEvent(ButtonCodes.BTN_Z, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.BumperRight.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TL_Pressed_ShouldSetButtonSelect()
    {
        // V2: BTN_TL maps to Select (non-standard)
        var ev = CreateKeyEvent(ButtonCodes.BTN_TL, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonSelect.ShouldBeTrue();
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TL_Released_ShouldClearButtonSelect()
    {
        // V2: BTN_TL maps to Select (non-standard)
        _state.ButtonSelect = true;
        var ev = CreateKeyEvent(ButtonCodes.BTN_TL, 0);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonSelect.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TR_Pressed_ShouldSetButtonStart()
    {
        // V2: BTN_TR maps to Start (non-standard)
        var ev = CreateKeyEvent(ButtonCodes.BTN_TR, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonStart.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TR_Released_ShouldClearButtonStart()
    {
        // V2: BTN_TR maps to Start (non-standard)
        _state.ButtonStart = true;
        var ev = CreateKeyEvent(ButtonCodes.BTN_TR, 0);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonStart.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TL2_Pressed_ShouldSetThumbLeft()
    {
        // V2: BTN_TL2 maps to left stick click (L3)
        var ev = CreateKeyEvent(ButtonCodes.BTN_TL2, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ThumbLeft.ShouldBeTrue();
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TR2_Pressed_ShouldSetThumbRight()
    {
        // V2: BTN_TR2 maps to right stick click (R3)
        var ev = CreateKeyEvent(ButtonCodes.BTN_TR2, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ThumbRight.ShouldBeTrue();
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_START_ShouldNotAffectState()
    {
        // V2 hardware uses BTN_TR for Start button, not BTN_START
        // BTN_START code is not emitted by V2 hardware
        var ev = CreateKeyEvent(ButtonCodes.BTN_START, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_SELECT_ShouldNotAffectState()
    {
        // V2 hardware uses BTN_TL for Select button, not BTN_SELECT
        // BTN_SELECT code is not emitted by V2 hardware
        var ev = CreateKeyEvent(ButtonCodes.BTN_SELECT, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_MODE_Pressed_ShouldSetButtonGuide()
    {
        var ev = CreateKeyEvent(ButtonCodes.BTN_MODE, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.ButtonGuide.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_THUMBL_ShouldNotAffectState()
    {
        // V2: BTN_THUMBL is not used - stick clicks are BTN_TL2/TR2
        var ev = CreateKeyEvent(ButtonCodes.BTN_THUMBL, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_THUMBR_ShouldNotAffectState()
    {
        // V2: BTN_THUMBR is not used - stick clicks are BTN_TL2/TR2
        var ev = CreateKeyEvent(ButtonCodes.BTN_THUMBR, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TRIGGER_HAPPY1_Pressed_ShouldSetPaddle1()
    {
        var ev = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY1, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.Paddle1.ShouldBeTrue();
        _state.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TRIGGER_HAPPY2_Pressed_ShouldSetPaddle2()
    {
        var ev = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY2, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.Paddle2.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TRIGGER_HAPPY3_Pressed_ShouldSetPaddle3()
    {
        var ev = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY3, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.Paddle3.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_BTN_TRIGGER_HAPPY4_Pressed_ShouldSetPaddle4()
    {
        var ev = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY4, 1);

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.Paddle4.ShouldBeTrue();
    }

    [Test]
    public void ProcessEvdevEvent_AllPaddles_ShouldBeIndependent()
    {
        var ev1 = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY1, 1);
        var ev2 = CreateKeyEvent(ButtonCodes.BTN_TRIGGER_HAPPY3, 1);
        EnvisionMapping.ProcessEvdevEvent(in ev1, _state);
        EnvisionMapping.ProcessEvdevEvent(in ev2, _state);

        _state.Paddle1.ShouldBeTrue();
        _state.Paddle2.ShouldBeFalse();
        _state.Paddle3.ShouldBeTrue();
        _state.Paddle4.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_EV_SYN_ShouldNotModifyState()
    {
        _state.LeftStickX = 1000;
        _state.ButtonA = true;

        var ev = new InputEvent
        {
            Type = EventTypes.EV_SYN,
            Code = SynCodes.SYN_REPORT,
            Value = 0
        };

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.LeftStickX.ShouldBe(1000);
        _state.ButtonA.ShouldBeTrue();
        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_UnknownEventType_ShouldNotModifyState()
    {
        var ev = new InputEvent
        {
            Type = 0xFF, // Unknown type
            Code = 0x00,
            Value = 100
        };

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_UnknownAbsCode_ShouldNotModifyState()
    {
        var ev = new InputEvent
        {
            Type = EventTypes.EV_ABS,
            Code = 0xFF, // Unknown axis
            Value = 100
        };

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessEvdevEvent_UnknownButtonCode_ShouldNotModifyState()
    {
        var ev = new InputEvent
        {
            Type = EventTypes.EV_KEY,
            Code = 0xFF, // Unknown button
            Value = 1
        };

        EnvisionMapping.ProcessEvdevEvent(in ev, _state);

        _state.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void ProcessMultipleEvents_ShouldAccumulateState()
    {
        // Left stick
        var evLsx = CreateAbsEvent(AbsCodes.ABS_X, 10000);
        var evLsy = CreateAbsEvent(AbsCodes.ABS_Y, -5000);
        EnvisionMapping.ProcessEvdevEvent(in evLsx, _state);
        EnvisionMapping.ProcessEvdevEvent(in evLsy, _state);

        // Face buttons
        var evA = CreateKeyEvent(ButtonCodes.BTN_SOUTH, 1);
        var evY = CreateKeyEvent(ButtonCodes.BTN_NORTH, 1);
        EnvisionMapping.ProcessEvdevEvent(in evA, _state);
        EnvisionMapping.ProcessEvdevEvent(in evY, _state);

        _state.LeftStickX.ShouldBe(10000);
        _state.LeftStickY.ShouldBe(-5000);
        _state.ButtonA.ShouldBeTrue();
        _state.ButtonY.ShouldBeTrue();
        _state.ButtonB.ShouldBeFalse();
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
}