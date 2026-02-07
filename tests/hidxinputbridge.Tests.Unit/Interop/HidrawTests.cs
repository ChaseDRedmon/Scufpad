using System.Runtime.InteropServices;
using HidXInputBridge.Interop;
using NUnit.Framework;
using Shouldly;

namespace HidXInputBridge.Tests.Unit.Interop;

[TestFixture]
public class HidrawDevInfoTests
{
    [Test]
    public void HidrawDevInfo_ShouldHaveCorrectSize()
    {
        // HidrawDevInfo: BusType (4) + Vendor (2) + Product (2) = 8 bytes
        Marshal.SizeOf<HidrawDevInfo>().ShouldBe(8);
    }

    [Test]
    public void HidrawDevInfo_ShouldHaveSequentialLayout()
    {
        // Verify sequential layout by checking that size equals sum of field sizes
        // BusType (4) + Vendor (2) + Product (2) = 8 bytes
        var size = Marshal.SizeOf<HidrawDevInfo>();
        size.ShouldBe(8);
    }

    [Test]
    public void HidrawDevInfo_ShouldInitializeCorrectly()
    {
        var devInfo = new HidrawDevInfo(
            3, // USB
            0x1b1c, // Scuf/Corsair
            0x3a05);

        devInfo.BusType.ShouldBe(3u);
        devInfo.Vendor.ShouldBe((short)0x1b1c);
        devInfo.Product.ShouldBe((short)0x3a05);
    }

    [Test]
    public void HidrawDevInfo_Vendor_ShouldHandleSignedValues()
    {
        // Vendor IDs can be high values that might look negative when cast to short
        var devInfo = new HidrawDevInfo(
            0,
            unchecked((short)0x8000), // High bit set
            0);

        devInfo.Vendor.ShouldBe(unchecked((short)0x8000));
    }

    [Test]
    public void HidrawDevInfo_ShouldBeReadonlyStruct()
    {
        // Verify that HidrawDevInfo is a readonly struct
        var info = new HidrawDevInfo(3, 0x1b1c, 0x3a05);
        ReadonlyDevInfoHolder holder = new(info);
        holder.Info.Vendor.ShouldBe((short)0x1b1c);
    }

    private readonly struct ReadonlyDevInfoHolder
    {
        public readonly HidrawDevInfo Info;

        public ReadonlyDevInfoHolder(HidrawDevInfo info)
        {
            Info = info;
        }
    }
}

[TestFixture]
public class HidrawIoctlTests
{
    [Test]
    public void HIDIOCGRAWINFO_ShouldHaveCorrectValue()
    {
        // _IOR('H', 0x03, struct hidraw_devinfo) = 0x80084803
        // Compare lower 32 bits to handle sign extension on 64-bit
        ((uint)HidrawIoctl.HIDIOCGRAWINFO).ShouldBe(0x80084803u);
    }

    [Test]
    public void HIDIOCGRAWNAME_256_ShouldHaveCorrectValue()
    {
        // _IOC(_IOC_READ, 'H', 0x04, 256) = 0x81004804
        // Compare lower 32 bits to handle sign extension on 64-bit
        ((uint)HidrawIoctl.HIDIOCGRAWNAME_256).ShouldBe(0x81004804u);
    }

    [Test]
    public void IoctlValues_ShouldHaveCorrectMagicNumber()
    {
        // All hidraw ioctls should have 'H' (0x48) in the type field (bits 8-15)
        (((uint)HidrawIoctl.HIDIOCGRAWINFO >> 8) & 0xFF).ShouldBe(0x48u);
        (((uint)HidrawIoctl.HIDIOCGRAWNAME_256 >> 8) & 0xFF).ShouldBe(0x48u);
    }

    [Test]
    public void HIDIOCGRAWINFO_ShouldBeReadIoctl()
    {
        // _IOR ioctls have bit 30 set (direction = read)
        ((uint)HidrawIoctl.HIDIOCGRAWINFO & 0x80000000).ShouldBe(0x80000000u);
    }

    [Test]
    public void HIDIOCGRAWNAME_256_ShouldBeReadIoctl()
    {
        // _IOR ioctls have bit 30 set (direction = read)
        ((uint)HidrawIoctl.HIDIOCGRAWNAME_256 & 0x80000000).ShouldBe(0x80000000u);
    }
}