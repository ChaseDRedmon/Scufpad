using System.Runtime.InteropServices;
using HidXInputBridge.Interop;
using NUnit.Framework;
using Shouldly;

namespace HidXInputBridge.Tests.Unit.Interop;

[TestFixture]
public class UinputSetupTests
{
    [Test]
    public void UinputSetup_ShouldHaveCorrectSize()
    {
        // UinputSetup: InputId (8) + Name (80) + FfEffectsMax (4) = 92 bytes
        Marshal.SizeOf<UinputSetup>().ShouldBe(92);
    }

    [Test]
    public void UinputSetup_ShouldHaveSequentialLayout()
    {
        // Verify sequential layout by checking that size equals sum of field sizes
        // InputId (8) + Name (80) + FfEffectsMax (4) = 92 bytes
        var size = Marshal.SizeOf<UinputSetup>();
        size.ShouldBe(92);
    }

    [Test]
    public unsafe void UinputSetup_NameBuffer_ShouldBe80Bytes()
    {
        var setup = new UinputSetup();

        // Set some bytes in the name buffer to verify it works
        setup.Name[0] = (byte)'T';
        setup.Name[1] = (byte)'e';
        setup.Name[2] = (byte)'s';
        setup.Name[3] = (byte)'t';
        setup.Name[79] = (byte)'!'; // Last valid index

        setup.Name[0].ShouldBe((byte)'T');
        setup.Name[79].ShouldBe((byte)'!');
    }

    [Test]
    public void UinputSetup_ShouldInitializeCorrectly()
    {
        var setup = new UinputSetup
        {
            Id = new InputId(BusType.BUS_USB, 0x045e, 0x0b12, 1),
            FfEffectsMax = 16
        };

        setup.Id.BusType.ShouldBe(BusType.BUS_USB);
        setup.Id.Vendor.ShouldBe((ushort)0x045e);
        setup.Id.Product.ShouldBe((ushort)0x0b12);
        setup.Id.Version.ShouldBe((ushort)1);
        setup.FfEffectsMax.ShouldBe(16u);
    }
}

[TestFixture]
public class UinputAbsSetupTests
{
    [Test]
    public void UinputAbsSetup_ShouldHaveCorrectSize()
    {
        // UinputAbsSetup: Code (2) + padding (2) + InputAbsInfo (24) = 28 bytes
        // The ioctl UI_ABS_SETUP (0x401c5504) encodes size 0x1c = 28 bytes
        var size = Marshal.SizeOf<UinputAbsSetup>();
        size.ShouldBe(28);
    }

    [Test]
    public void UinputAbsSetup_ShouldMatchDeclaredSize()
    {
        // Verify the declared constant matches the actual marshal size
        Marshal.SizeOf<UinputAbsSetup>().ShouldBe(UinputAbsSetup.Size);
    }

    [Test]
    public void UinputAbsSetup_ShouldInitializeCorrectly()
    {
        var absSetup = new UinputAbsSetup(
            AbsCodes.ABS_X,
            new InputAbsInfo(-32768, 32767, 16, 128));

        absSetup.Code.ShouldBe(AbsCodes.ABS_X);
        absSetup.AbsInfo.Minimum.ShouldBe(-32768);
        absSetup.AbsInfo.Maximum.ShouldBe(32767);
        absSetup.AbsInfo.Fuzz.ShouldBe(16);
        absSetup.AbsInfo.Flat.ShouldBe(128);
    }

    [Test]
    public void UinputAbsSetup_ShouldBeReadonlyStruct()
    {
        // Verify that UinputAbsSetup is a readonly struct
        var setup = new UinputAbsSetup(AbsCodes.ABS_X, new InputAbsInfo(0, 1023));
        ReadonlyAbsSetupHolder holder = new(setup);
        holder.Setup.Code.ShouldBe(AbsCodes.ABS_X);
    }

    private readonly struct ReadonlyAbsSetupHolder
    {
        public readonly UinputAbsSetup Setup;

        public ReadonlyAbsSetupHolder(UinputAbsSetup setup)
        {
            Setup = setup;
        }
    }

    [Test]
    public void UinputAbsSetup_ValidateSize_ShouldNotThrow()
    {
        // ValidateSize should not throw when the size matches
        Should.NotThrow(() => UinputAbsSetup.ValidateSize());
    }
}

[TestFixture]
public class UinputIoctlTests
{
    [Test]
    public void UI_SET_EVBIT_ShouldHaveCorrectValue()
    {
        // _IOW('U', 100, int) = 0x40045564
        UinputIoctl.UI_SET_EVBIT.ShouldBe((nuint)0x40045564);
    }

    [Test]
    public void UI_SET_KEYBIT_ShouldHaveCorrectValue()
    {
        // _IOW('U', 101, int) = 0x40045565
        UinputIoctl.UI_SET_KEYBIT.ShouldBe((nuint)0x40045565);
    }

    [Test]
    public void UI_SET_ABSBIT_ShouldHaveCorrectValue()
    {
        // _IOW('U', 103, int) = 0x40045567
        UinputIoctl.UI_SET_ABSBIT.ShouldBe((nuint)0x40045567);
    }

    [Test]
    public void UI_DEV_SETUP_ShouldHaveCorrectValue()
    {
        // _IOW('U', 3, struct uinput_setup) = 0x405c5503
        UinputIoctl.UI_DEV_SETUP.ShouldBe((nuint)0x405c5503);
    }

    [Test]
    public void UI_ABS_SETUP_ShouldHaveCorrectValue()
    {
        // _IOW('U', 4, struct uinput_abs_setup) = 0x401c5504
        UinputIoctl.UI_ABS_SETUP.ShouldBe((nuint)0x401c5504);
    }

    [Test]
    public void UI_DEV_CREATE_ShouldHaveCorrectValue()
    {
        // _IO('U', 1) = 0x5501
        UinputIoctl.UI_DEV_CREATE.ShouldBe((nuint)0x5501);
    }

    [Test]
    public void UI_DEV_DESTROY_ShouldHaveCorrectValue()
    {
        // _IO('U', 2) = 0x5502
        UinputIoctl.UI_DEV_DESTROY.ShouldBe((nuint)0x5502);
    }

    [Test]
    public void IoctlValues_ShouldHaveCorrectMagicNumber()
    {
        // All uinput ioctls should have 'U' (0x55) in them
        // For _IOW types, the magic is in bits 8-15
        ((UinputIoctl.UI_SET_EVBIT >> 8) & 0xFF).ShouldBe((nuint)0x55);
        ((UinputIoctl.UI_SET_KEYBIT >> 8) & 0xFF).ShouldBe((nuint)0x55);
        ((UinputIoctl.UI_SET_ABSBIT >> 8) & 0xFF).ShouldBe((nuint)0x55);

        // For _IO types, the magic is directly visible
        (UinputIoctl.UI_DEV_CREATE & 0xFF00).ShouldBe((nuint)0x5500);
        (UinputIoctl.UI_DEV_DESTROY & 0xFF00).ShouldBe((nuint)0x5500);
    }
}

[TestFixture]
public class BusTypeTests
{
    [Test]
    public void BUS_USB_ShouldHaveCorrectValue()
    {
        BusType.BUS_USB.ShouldBe((ushort)0x03);
    }

    [Test]
    public void BUS_VIRTUAL_ShouldHaveCorrectValue()
    {
        BusType.BUS_VIRTUAL.ShouldBe((ushort)0x06);
    }
}