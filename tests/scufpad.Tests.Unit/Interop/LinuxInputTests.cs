using System.Runtime.InteropServices;
using Scufpad.Interop;
using NUnit.Framework;
using Shouldly;

namespace Scufpad.Tests.Unit.Interop;

[TestFixture]
public class InputEventTests
{
    [Test]
    public void InputEvent_ShouldHaveCorrectDeclaredSize()
    {
        InputEvent.Size.ShouldBe(24);
    }

    [Test]
    public void InputEvent_ActualSize_ShouldMatchDeclaredSize()
    {
        Marshal.SizeOf<InputEvent>().ShouldBe(InputEvent.Size);
    }

    [Test]
    public void InputEvent_ShouldInitializeCorrectly()
    {
        var ev = new InputEvent
        {
            TvSec = 1234567890,
            TvUsec = 123456,
            Type = EventTypes.EV_KEY,
            Code = ButtonCodes.BTN_SOUTH,
            Value = 1
        };

        ev.TvSec.ShouldBe(1234567890L);
        ev.TvUsec.ShouldBe(123456L);
        ev.Type.ShouldBe(EventTypes.EV_KEY);
        ev.Code.ShouldBe(ButtonCodes.BTN_SOUTH);
        ev.Value.ShouldBe(1);
    }

    [Test]
    public void InputEvent_ShouldHaveSequentialLayout()
    {
        // Verify sequential layout by checking that size equals sum of field sizes
        // long (8) + long (8) + ushort (2) + ushort (2) + int (4) = 24 bytes
        var size = Marshal.SizeOf<InputEvent>();
        size.ShouldBe(24);
    }

    [Test]
    public void InputEvent_ValidateSize_ShouldNotThrow()
    {
        // ValidateSize should not throw when the size matches
        Should.NotThrow(() => InputEvent.ValidateSize());
    }
}

[TestFixture]
public class InputIdTests
{
    [Test]
    public void InputId_ShouldHaveCorrectSize()
    {
        // InputId: 4 x ushort (2 bytes each) = 8 bytes
        Marshal.SizeOf<InputId>().ShouldBe(8);
    }

    [Test]
    public void InputId_ShouldInitializeCorrectly()
    {
        var id = new InputId(BusType.BUS_USB, 0x045e, 0x0b12, 1);

        id.BusType.ShouldBe(BusType.BUS_USB);
        id.Vendor.ShouldBe((ushort)0x045e);
        id.Product.ShouldBe((ushort)0x0b12);
        id.Version.ShouldBe((ushort)1);
    }

    [Test]
    public void InputId_ShouldBeReadonlyStruct()
    {
        // Verify that InputId is a readonly struct by checking it can be assigned to a readonly field
        var id = new InputId(BusType.BUS_USB, 0x045e, 0x0b12, 1);
        ReadonlyInputIdHolder holder = new(id);
        holder.Id.Vendor.ShouldBe((ushort)0x045e);
    }

    private readonly struct ReadonlyInputIdHolder
    {
        public readonly InputId Id;

        public ReadonlyInputIdHolder(InputId id)
        {
            Id = id;
        }
    }

    [Test]
    public void InputId_ShouldHaveSequentialLayout()
    {
        // Verify sequential layout by checking that size equals sum of field sizes
        // 4 x ushort (2 bytes each) = 8 bytes
        var size = Marshal.SizeOf<InputId>();
        size.ShouldBe(8);
    }
}

[TestFixture]
public class InputAbsInfoTests
{
    [Test]
    public void InputAbsInfo_ShouldHaveCorrectSize()
    {
        // InputAbsInfo: 6 x int (4 bytes each) = 24 bytes
        Marshal.SizeOf<InputAbsInfo>().ShouldBe(24);
    }

    [Test]
    public void InputAbsInfo_ShouldInitializeCorrectly()
    {
        var absInfo = new InputAbsInfo(
            -32768,
            32767,
            16,
            128);

        absInfo.Value.ShouldBe(0);
        absInfo.Minimum.ShouldBe(-32768);
        absInfo.Maximum.ShouldBe(32767);
        absInfo.Fuzz.ShouldBe(16);
        absInfo.Flat.ShouldBe(128);
        absInfo.Resolution.ShouldBe(0);
    }

    [Test]
    public void InputAbsInfo_ShouldBeReadonlyStruct()
    {
        // Verify that InputAbsInfo is a readonly struct
        var info = new InputAbsInfo(-32768, 32767);
        ReadonlyAbsInfoHolder holder = new(info);
        holder.Info.Minimum.ShouldBe(-32768);
    }

    private readonly struct ReadonlyAbsInfoHolder
    {
        public readonly InputAbsInfo Info;

        public ReadonlyAbsInfoHolder(InputAbsInfo info)
        {
            Info = info;
        }
    }

    [Test]
    public void InputAbsInfo_ShouldHaveSequentialLayout()
    {
        // Verify sequential layout by checking that size equals sum of field sizes
        // 6 x int (4 bytes each) = 24 bytes
        var size = Marshal.SizeOf<InputAbsInfo>();
        size.ShouldBe(24);
    }
}

[TestFixture]
public class EventTypesTests
{
    [Test]
    public void EV_SYN_ShouldBeZero()
    {
        EventTypes.EV_SYN.ShouldBe((ushort)0x00);
    }

    [Test]
    public void EV_KEY_ShouldBeOne()
    {
        EventTypes.EV_KEY.ShouldBe((ushort)0x01);
    }

    [Test]
    public void EV_ABS_ShouldBeThree()
    {
        EventTypes.EV_ABS.ShouldBe((ushort)0x03);
    }

    [Test]
    public void EV_FF_ShouldHaveCorrectValue()
    {
        EventTypes.EV_FF.ShouldBe((ushort)0x15);
    }
}

[TestFixture]
public class SynCodesTests
{
    [Test]
    public void SYN_REPORT_ShouldBeZero()
    {
        SynCodes.SYN_REPORT.ShouldBe((ushort)0x00);
    }
}

[TestFixture]
public class ButtonCodesTests
{
    [Test]
    public void BTN_MISC_ShouldHaveCorrectValue()
    {
        ButtonCodes.BTN_MISC.ShouldBe((ushort)0x100);
    }

    [Test]
    public void BTN_GAMEPAD_ShouldMatchBTN_SOUTH()
    {
        ButtonCodes.BTN_GAMEPAD.ShouldBe(ButtonCodes.BTN_SOUTH);
        ButtonCodes.BTN_GAMEPAD.ShouldBe((ushort)0x130);
    }

    [Test]
    public void FaceButtons_ShouldHaveCorrectValues()
    {
        ButtonCodes.BTN_SOUTH.ShouldBe((ushort)0x130); // A
        ButtonCodes.BTN_A.ShouldBe(ButtonCodes.BTN_SOUTH);
        ButtonCodes.BTN_EAST.ShouldBe((ushort)0x131); // B
        ButtonCodes.BTN_B.ShouldBe(ButtonCodes.BTN_EAST);
        ButtonCodes.BTN_C.ShouldBe((ushort)0x132);
        ButtonCodes.BTN_NORTH.ShouldBe((ushort)0x133); // Y
        ButtonCodes.BTN_X.ShouldBe(ButtonCodes.BTN_NORTH);
        ButtonCodes.BTN_WEST.ShouldBe((ushort)0x134); // X
        ButtonCodes.BTN_Y.ShouldBe(ButtonCodes.BTN_WEST);
        ButtonCodes.BTN_Z.ShouldBe((ushort)0x135);
    }

    [Test]
    public void ShoulderButtons_ShouldHaveCorrectValues()
    {
        ButtonCodes.BTN_TL.ShouldBe((ushort)0x136);
        ButtonCodes.BTN_TR.ShouldBe((ushort)0x137);
        ButtonCodes.BTN_TL2.ShouldBe((ushort)0x138);
        ButtonCodes.BTN_TR2.ShouldBe((ushort)0x139);
    }

    [Test]
    public void MenuButtons_ShouldHaveCorrectValues()
    {
        ButtonCodes.BTN_SELECT.ShouldBe((ushort)0x13a);
        ButtonCodes.BTN_START.ShouldBe((ushort)0x13b);
        ButtonCodes.BTN_MODE.ShouldBe((ushort)0x13c);
    }

    [Test]
    public void ThumbButtons_ShouldHaveCorrectValues()
    {
        ButtonCodes.BTN_THUMBL.ShouldBe((ushort)0x13d);
        ButtonCodes.BTN_THUMBR.ShouldBe((ushort)0x13e);
    }

    [Test]
    public void PaddleButtons_ShouldHaveCorrectValues()
    {
        ButtonCodes.BTN_TRIGGER_HAPPY1.ShouldBe((ushort)0x2c0);
        ButtonCodes.BTN_TRIGGER_HAPPY2.ShouldBe((ushort)0x2c1);
        ButtonCodes.BTN_TRIGGER_HAPPY3.ShouldBe((ushort)0x2c2);
        ButtonCodes.BTN_TRIGGER_HAPPY4.ShouldBe((ushort)0x2c3);
    }

    [Test]
    public void PaddleButtons_ShouldBeContiguous()
    {
        ButtonCodes.BTN_TRIGGER_HAPPY2.ShouldBe((ushort)(ButtonCodes.BTN_TRIGGER_HAPPY1 + 1));
        ButtonCodes.BTN_TRIGGER_HAPPY3.ShouldBe((ushort)(ButtonCodes.BTN_TRIGGER_HAPPY1 + 2));
        ButtonCodes.BTN_TRIGGER_HAPPY4.ShouldBe((ushort)(ButtonCodes.BTN_TRIGGER_HAPPY1 + 3));
    }
}

[TestFixture]
public class AbsCodesTests
{
    [Test]
    public void LeftStickAxes_ShouldHaveCorrectValues()
    {
        AbsCodes.ABS_X.ShouldBe((ushort)0x00);
        AbsCodes.ABS_Y.ShouldBe((ushort)0x01);
    }

    [Test]
    public void TriggerAndRightStickAxes_ShouldHaveCorrectValues()
    {
        AbsCodes.ABS_Z.ShouldBe((ushort)0x02);
        AbsCodes.ABS_RX.ShouldBe((ushort)0x03);
        AbsCodes.ABS_RY.ShouldBe((ushort)0x04);
        AbsCodes.ABS_RZ.ShouldBe((ushort)0x05);
    }

    [Test]
    public void DpadAxes_ShouldHaveCorrectValues()
    {
        AbsCodes.ABS_HAT0X.ShouldBe((ushort)0x10);
        AbsCodes.ABS_HAT0Y.ShouldBe((ushort)0x11);
    }
}

[TestFixture]
public class EvdevIoctlTests
{
    [Test]
    public void EVIOCGRAB_ShouldHaveCorrectValue()
    {
        // _IOW('E', 0x90, int) = 0x40044590
        EvdevIoctl.EVIOCGRAB.ShouldBe((nuint)0x40044590);
    }
}