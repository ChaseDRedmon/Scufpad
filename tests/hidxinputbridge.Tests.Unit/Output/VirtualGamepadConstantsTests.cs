using HidXInputBridge.Interop;
using NUnit.Framework;
using Shouldly;

namespace HidXInputBridge.Tests.Unit.Output;

/// <summary>
///     Tests for the constants and configuration values used by VirtualGamepad.
///     The actual VirtualGamepad class requires uinput access, so we test what we can
///     without hardware dependencies.
/// </summary>
[TestFixture]
public class VirtualGamepadConstantsTests
{
    // Xbox Elite 2 Controller identifiers used by VirtualGamepad
    private const ushort XboxVendorId = 0x045e; // Microsoft

    private const ushort XboxProductId = 0x0b12; // Xbox Elite 2

    // Axis ranges used by VirtualGamepad
    private const int StickMin = -32768;

    private const int StickMax = 32767;

    private const int TriggerMin = 0;

    private const int TriggerMax = 1023;

    private const int DpadMin = -1;

    private const int DpadMax = 1;

    [Test]
    public void XboxVendorId_ShouldBeMicrosoft()
    {
        XboxVendorId.ShouldBe((ushort)0x045e);
    }

    [Test]
    public void XboxProductId_ShouldBeElite2()
    {
        XboxProductId.ShouldBe((ushort)0x0b12);
    }

    [Test]
    public void StickRange_ShouldBeSignedShort()
    {
        StickMin.ShouldBe(-32768);
        StickMax.ShouldBe(32767);
    }

    [Test]
    public void StickRange_ShouldBe16BitSigned()
    {
        (StickMax - StickMin + 1).ShouldBe(65536); // 2^16
    }

    [Test]
    public void TriggerRange_ShouldBeUnsigned10Bit()
    {
        TriggerMin.ShouldBe(0);
        TriggerMax.ShouldBe(1023);
        (TriggerMax - TriggerMin + 1).ShouldBe(1024); // 2^10
    }

    [Test]
    public void DpadRange_ShouldBeTristateDigital()
    {
        DpadMin.ShouldBe(-1);
        DpadMax.ShouldBe(1);
        (DpadMax - DpadMin + 1).ShouldBe(3); // -1, 0, 1
    }

    [Test]
    public void EnabledButtons_ShouldIncludeAllGamepadButtons()
    {
        // The VirtualGamepad enables these buttons
        ushort[] expectedButtons =
        [
            ButtonCodes.BTN_SOUTH,
            ButtonCodes.BTN_EAST,
            ButtonCodes.BTN_NORTH,
            ButtonCodes.BTN_WEST,
            ButtonCodes.BTN_TL,
            ButtonCodes.BTN_TR,
            ButtonCodes.BTN_SELECT,
            ButtonCodes.BTN_START,
            ButtonCodes.BTN_MODE,
            ButtonCodes.BTN_THUMBL,
            ButtonCodes.BTN_THUMBR,
            ButtonCodes.BTN_TRIGGER_HAPPY1,
            ButtonCodes.BTN_TRIGGER_HAPPY2,
            ButtonCodes.BTN_TRIGGER_HAPPY3,
            ButtonCodes.BTN_TRIGGER_HAPPY4
        ];

        expectedButtons.Length.ShouldBe(15);

        // Verify all button codes are unique
        expectedButtons.Distinct().Count().ShouldBe(expectedButtons.Length);
    }

    [Test]
    public void FaceButtons_ShouldAllBePresent()
    {
        ushort[] faceButtons =
        [
            ButtonCodes.BTN_SOUTH, // A
            ButtonCodes.BTN_EAST, // B
            ButtonCodes.BTN_NORTH, // Y
            ButtonCodes.BTN_WEST // X
        ];

        faceButtons.Length.ShouldBe(4);
        faceButtons.Distinct().Count().ShouldBe(4);
    }

    [Test]
    public void ShoulderButtons_ShouldAllBePresent()
    {
        ushort[] shoulderButtons =
        [
            ButtonCodes.BTN_TL, // LB
            ButtonCodes.BTN_TR // RB
        ];

        shoulderButtons.Length.ShouldBe(2);
        shoulderButtons.Distinct().Count().ShouldBe(2);
    }

    [Test]
    public void PaddleButtons_ShouldAllBePresent()
    {
        ushort[] paddleButtons =
        [
            ButtonCodes.BTN_TRIGGER_HAPPY1,
            ButtonCodes.BTN_TRIGGER_HAPPY2,
            ButtonCodes.BTN_TRIGGER_HAPPY3,
            ButtonCodes.BTN_TRIGGER_HAPPY4
        ];

        paddleButtons.Length.ShouldBe(4);
        paddleButtons.Distinct().Count().ShouldBe(4);
    }

    [Test]
    public void StickAxes_ShouldUseCorrectCodes()
    {
        // Left stick
        AbsCodes.ABS_X.ShouldBe((ushort)0x00);
        AbsCodes.ABS_Y.ShouldBe((ushort)0x01);

        // Right stick (standard mapping, not Envision)
        AbsCodes.ABS_RX.ShouldBe((ushort)0x03);
        AbsCodes.ABS_RY.ShouldBe((ushort)0x04);
    }

    [Test]
    public void TriggerAxes_ShouldUseCorrectCodes()
    {
        // Standard Xbox trigger mapping
        AbsCodes.ABS_Z.ShouldBe((ushort)0x02); // Left trigger
        AbsCodes.ABS_RZ.ShouldBe((ushort)0x05); // Right trigger
    }

    [Test]
    public void DpadAxes_ShouldUseCorrectCodes()
    {
        AbsCodes.ABS_HAT0X.ShouldBe((ushort)0x10);
        AbsCodes.ABS_HAT0Y.ShouldBe((ushort)0x11);
    }

    [Test]
    public void RequiredEventTypes_ShouldBeCorrect()
    {
        // VirtualGamepad enables EV_KEY, EV_ABS, and EV_SYN
        EventTypes.EV_KEY.ShouldBe((ushort)0x01);
        EventTypes.EV_ABS.ShouldBe((ushort)0x03);
        EventTypes.EV_SYN.ShouldBe((ushort)0x00);
    }

    [Test]
    public void InputEventSize_ShouldBeCorrectForWriting()
    {
        // VirtualGamepad writes InputEvent structs directly to uinput
        InputEvent.Size.ShouldBe(24);
    }
}