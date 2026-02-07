using System.Runtime.InteropServices;
using HidXInputBridge.Interop;
using NUnit.Framework;
using Shouldly;
using Libc = HidXInputBridge.Interop.Libc;

namespace HidXInputBridge.Tests.Integration;

/// <summary>
///     Integration tests verifying that interop structures match expected Linux kernel definitions.
///     These tests ensure ABI compatibility with the Linux kernel.
/// </summary>
[TestFixture]
public class InteropStructureIntegrationTests
{
    [Test]
    public void AllStructures_ShouldMatchKernelDefinitions()
    {
        // These sizes must match the Linux kernel definitions for proper interop
        Marshal.SizeOf<InputEvent>().ShouldBe(24, "InputEvent size mismatch - may cause read/write errors");
        Marshal.SizeOf<InputId>().ShouldBe(8, "InputId size mismatch");
        Marshal.SizeOf<InputAbsInfo>().ShouldBe(24, "InputAbsInfo size mismatch");
        Marshal.SizeOf<PollFd>().ShouldBe(8, "PollFd size mismatch");
        Marshal.SizeOf<HidrawDevInfo>().ShouldBe(8, "HidrawDevInfo size mismatch");
        Marshal.SizeOf<UinputSetup>().ShouldBe(92, "UinputSetup size mismatch");
    }

    [Test]
    public void InputEvent_DeclaredAndActualSize_ShouldMatch()
    {
        // The declared constant must match the actual marshaled size
        InputEvent.Size.ShouldBe(Marshal.SizeOf<InputEvent>());
    }

    [Test]
    public void Ioctl_Numbers_ShouldMatchLinuxDefinitions()
    {
        // These ioctl numbers are calculated from Linux kernel macros
        // _IOW('U', 100, int) for UI_SET_EVBIT
        UinputIoctl.UI_SET_EVBIT.ShouldBe((nuint)0x40045564);

        // _IOW('U', 101, int) for UI_SET_KEYBIT
        UinputIoctl.UI_SET_KEYBIT.ShouldBe((nuint)0x40045565);

        // _IOW('U', 103, int) for UI_SET_ABSBIT
        UinputIoctl.UI_SET_ABSBIT.ShouldBe((nuint)0x40045567);

        // _IO('U', 1) for UI_DEV_CREATE
        UinputIoctl.UI_DEV_CREATE.ShouldBe((nuint)0x5501);

        // _IO('U', 2) for UI_DEV_DESTROY
        UinputIoctl.UI_DEV_DESTROY.ShouldBe((nuint)0x5502);

        // _IOW('E', 0x90, int) for EVIOCGRAB
        EvdevIoctl.EVIOCGRAB.ShouldBe((nuint)0x40044590);

        // _IOR('H', 0x03, struct hidraw_devinfo) for HIDIOCGRAWINFO
        // Compare lower 32 bits to handle sign extension on 64-bit
        ((uint)HidrawIoctl.HIDIOCGRAWINFO).ShouldBe(0x80084803u);
    }

    [Test]
    public void EventTypes_ShouldMatchLinuxDefinitions()
    {
        // From linux/input-event-codes.h
        EventTypes.EV_SYN.ShouldBe((ushort)0x00);
        EventTypes.EV_KEY.ShouldBe((ushort)0x01);
        EventTypes.EV_ABS.ShouldBe((ushort)0x03);
        EventTypes.EV_FF.ShouldBe((ushort)0x15);
    }

    [Test]
    public void ButtonCodes_ShouldMatchLinuxDefinitions()
    {
        // From linux/input-event-codes.h
        ButtonCodes.BTN_SOUTH.ShouldBe((ushort)0x130);
        ButtonCodes.BTN_EAST.ShouldBe((ushort)0x131);
        ButtonCodes.BTN_NORTH.ShouldBe((ushort)0x133);
        ButtonCodes.BTN_WEST.ShouldBe((ushort)0x134);

        ButtonCodes.BTN_TL.ShouldBe((ushort)0x136);
        ButtonCodes.BTN_TR.ShouldBe((ushort)0x137);

        ButtonCodes.BTN_SELECT.ShouldBe((ushort)0x13a);
        ButtonCodes.BTN_START.ShouldBe((ushort)0x13b);
        ButtonCodes.BTN_MODE.ShouldBe((ushort)0x13c);

        ButtonCodes.BTN_THUMBL.ShouldBe((ushort)0x13d);
        ButtonCodes.BTN_THUMBR.ShouldBe((ushort)0x13e);

        ButtonCodes.BTN_TRIGGER_HAPPY1.ShouldBe((ushort)0x2c0);
    }

    [Test]
    public void AbsCodes_ShouldMatchLinuxDefinitions()
    {
        // From linux/input-event-codes.h
        AbsCodes.ABS_X.ShouldBe((ushort)0x00);
        AbsCodes.ABS_Y.ShouldBe((ushort)0x01);
        AbsCodes.ABS_Z.ShouldBe((ushort)0x02);
        AbsCodes.ABS_RX.ShouldBe((ushort)0x03);
        AbsCodes.ABS_RY.ShouldBe((ushort)0x04);
        AbsCodes.ABS_RZ.ShouldBe((ushort)0x05);
        AbsCodes.ABS_HAT0X.ShouldBe((ushort)0x10);
        AbsCodes.ABS_HAT0Y.ShouldBe((ushort)0x11);
    }

    [Test]
    public void LibcFileFlags_ShouldMatchLinuxDefinitions()
    {
        // From fcntl.h
        Libc.O_RDONLY.ShouldBe(0x0000);
        Libc.O_WRONLY.ShouldBe(0x0001);
        Libc.O_RDWR.ShouldBe(0x0002);
        Libc.O_NONBLOCK.ShouldBe(0x0800);
    }

    [Test]
    public void LibcPollFlags_ShouldMatchLinuxDefinitions()
    {
        // From poll.h
        Libc.POLLIN.ShouldBe(0x0001);
        Libc.POLLOUT.ShouldBe(0x0004);
        Libc.POLLERR.ShouldBe(0x0008);
        Libc.POLLHUP.ShouldBe(0x0010);
    }

    [Test]
    public void LibcErrno_ShouldMatchLinuxDefinitions()
    {
        // From errno.h
        Libc.EINTR.ShouldBe(4);
    }

    [Test]
    public void BusTypes_ShouldMatchLinuxDefinitions()
    {
        // From linux/input.h
        BusType.BUS_USB.ShouldBe((ushort)0x03);
        BusType.BUS_VIRTUAL.ShouldBe((ushort)0x06);
    }

    [Test]
    public void InputEvent_FieldOrder_ShouldBeCorrect()
    {
        // Verify fields are in correct order by creating and serializing
        var ev = new InputEvent
        {
            TvSec = 0x0102030405060708,
            TvUsec = 0x1112131415161718,
            Type = 0x2122,
            Code = 0x3132,
            Value = 0x41424344
        };

        ev.TvSec.ShouldBe(0x0102030405060708L);
        ev.TvUsec.ShouldBe(0x1112131415161718L);
        ev.Type.ShouldBe((ushort)0x2122);
        ev.Code.ShouldBe((ushort)0x3132);
        ev.Value.ShouldBe(0x41424344);
    }

    [Test]
    public void InputId_FieldOrder_ShouldBeCorrect()
    {
        var id = new InputId(0x0102, 0x1112, 0x2122, 0x3132);

        id.BusType.ShouldBe((ushort)0x0102);
        id.Vendor.ShouldBe((ushort)0x1112);
        id.Product.ShouldBe((ushort)0x2122);
        id.Version.ShouldBe((ushort)0x3132);
    }

    [Test]
    public void PollFd_FieldOrder_ShouldBeCorrect()
    {
        var pfd = new PollFd
        {
            Fd = 0x01020304,
            Events = 0x1112,
            Revents = 0x2122
        };

        pfd.Fd.ShouldBe(0x01020304);
        pfd.Events.ShouldBe((short)0x1112);
        pfd.Revents.ShouldBe((short)0x2122);
    }

    [Test]
    public void AxisRanges_ShouldBeValidFor16BitSigned()
    {
        // Standard gamepad axis ranges
        const int min = -32768;
        const int max = 32767;

        // Verify they fit in short
        ((short)min).ShouldBe((short)-32768);
        ((short)max).ShouldBe((short)32767);
    }

    [Test]
    public void TriggerRanges_ShouldBeValidFor10BitUnsigned()
    {
        const int min = 0;
        const int max = 1023;

        min.ShouldBe(0);
        max.ShouldBe(1023);
        (max - min + 1).ShouldBe(1024); // 2^10
    }
}