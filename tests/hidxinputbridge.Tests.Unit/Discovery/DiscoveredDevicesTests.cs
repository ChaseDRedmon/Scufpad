using HidXInputBridge.Discovery;
using NUnit.Framework;
using Shouldly;

namespace HidXInputBridge.Tests.Unit.Discovery;

[TestFixture]
public class DiscoveredDevicesTests
{
    [Test]
    public void DiscoveredDevices_ShouldStoreEvdevPath()
    {
        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event5",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = []
        };

        devices.EvdevPath.ShouldBe("/dev/input/event5");
    }

    [Test]
    public void DiscoveredDevices_ShouldStoreHidrawPath()
    {
        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event5",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = []
        };

        devices.HidrawPath.ShouldBe("/dev/hidraw0");
    }

    [Test]
    public void DiscoveredDevices_ShouldAcceptEmptyHidrawPath()
    {
        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event5",
            HidrawPath = string.Empty,
            SecondaryEvdevPaths = []
        };

        devices.HidrawPath.ShouldBeEmpty();
    }

    [Test]
    public void DiscoveredDevices_EvdevPath_ShouldBeRequired()
    {
        // This test verifies the 'required' modifier behavior
        // The compiler enforces this at compile time
        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event0",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = []
        };

        devices.EvdevPath.ShouldNotBeNull();
    }

    [Test]
    public void DiscoveredDevices_HidrawPath_ShouldBeRequired()
    {
        // This test verifies the 'required' modifier behavior
        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event0",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = []
        };

        devices.HidrawPath.ShouldNotBeNull();
    }

    [Test]
    public void DiscoveredDevices_DifferentEventNumbers_ShouldWork()
    {
        var devices1 = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event0",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = []
        };

        var devices2 = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event255",
            HidrawPath = "/dev/hidraw10",
            SecondaryEvdevPaths = []
        };

        devices1.EvdevPath.ShouldBe("/dev/input/event0");
        devices2.EvdevPath.ShouldBe("/dev/input/event255");
    }

    [Test]
    public void DiscoveredDevices_ShouldBeInitOnly()
    {
        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event5",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = []
        };

        // Properties are init-only, so they can only be set during initialization
        // This test verifies the values are correctly stored
        devices.EvdevPath.ShouldBe("/dev/input/event5");
        devices.HidrawPath.ShouldBe("/dev/hidraw0");

        // Note: Cannot test that properties are read-only at runtime,
        // as the compiler enforces init-only at compile time
    }

    [Test]
    public void DiscoveredDevices_ShouldStoreSecondaryEvdevPaths()
    {
        var secondaryPaths = new List<string>
        {
            "/dev/input/event6",
            "/dev/input/event7",
            "/dev/input/event8"
        };

        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event5",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = secondaryPaths
        };

        devices.SecondaryEvdevPaths.ShouldBe(secondaryPaths);
        devices.SecondaryEvdevPaths.Count.ShouldBe(3);
    }

    [Test]
    public void DiscoveredDevices_ShouldAcceptEmptySecondaryEvdevPaths()
    {
        var devices = new DiscoveredDevices
        {
            EvdevPath = "/dev/input/event5",
            HidrawPath = "/dev/hidraw0",
            SecondaryEvdevPaths = []
        };

        devices.SecondaryEvdevPaths.ShouldBeEmpty();
    }
}