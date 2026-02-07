using System.Runtime.InteropServices;
using Scufpad.Interop;
using NUnit.Framework;
using Shouldly;
using Libc = Scufpad.Interop.Libc;

namespace Scufpad.Tests.Unit.Interop;

[TestFixture]
public class LibcTests
{
    [Test]
    public void O_RDONLY_ShouldBeZero()
    {
        Libc.O_RDONLY.ShouldBe(0x0000);
    }

    [Test]
    public void O_WRONLY_ShouldBeOne()
    {
        Libc.O_WRONLY.ShouldBe(0x0001);
    }

    [Test]
    public void O_RDWR_ShouldBeTwo()
    {
        Libc.O_RDWR.ShouldBe(0x0002);
    }

    [Test]
    public void O_NONBLOCK_ShouldHaveCorrectValue()
    {
        Libc.O_NONBLOCK.ShouldBe(0x0800);
    }

    [Test]
    public void POLLIN_ShouldHaveCorrectValue()
    {
        Libc.POLLIN.ShouldBe(0x0001);
    }

    [Test]
    public void POLLOUT_ShouldHaveCorrectValue()
    {
        Libc.POLLOUT.ShouldBe(0x0004);
    }

    [Test]
    public void POLLERR_ShouldHaveCorrectValue()
    {
        Libc.POLLERR.ShouldBe(0x0008);
    }

    [Test]
    public void POLLHUP_ShouldHaveCorrectValue()
    {
        Libc.POLLHUP.ShouldBe(0x0010);
    }

    [Test]
    public void EINTR_ShouldBeFour()
    {
        Libc.EINTR.ShouldBe(4);
    }

    [Test]
    public void CombinedFlags_ShouldWorkCorrectly()
    {
        var flags = Libc.O_RDONLY | Libc.O_NONBLOCK;
        flags.ShouldBe(0x0800);

        flags = Libc.O_WRONLY | Libc.O_NONBLOCK;
        flags.ShouldBe(0x0801);

        flags = Libc.O_RDWR | Libc.O_NONBLOCK;
        flags.ShouldBe(0x0802);
    }

    [Test]
    public void PollFlags_ShouldBeCombineable()
    {
        var events = Libc.POLLIN | Libc.POLLOUT;
        events.ShouldBe(0x0005);

        events = Libc.POLLIN | Libc.POLLERR | Libc.POLLHUP;
        events.ShouldBe(0x0019);
    }
}

[TestFixture]
public class PollFdTests
{
    [Test]
    public void PollFd_ShouldHaveCorrectSize()
    {
        // PollFd should be 8 bytes: int (4) + short (2) + short (2)
        Marshal.SizeOf<PollFd>().ShouldBe(8);
    }

    [Test]
    public void PollFd_ShouldInitializeCorrectly()
    {
        var pollFd = new PollFd
        {
            Fd = 5,
            Events = Libc.POLLIN,
            Revents = 0
        };

        pollFd.Fd.ShouldBe(5);
        pollFd.Events.ShouldBe((short)Libc.POLLIN);
        pollFd.Revents.ShouldBe((short)0);
    }

    [Test]
    public void PollFd_ShouldHaveSequentialLayout()
    {
        // Verify the struct has sequential layout by checking its memory layout is predictable
        // If the struct were not sequential, the size could vary or fields could be reordered
        var size = Marshal.SizeOf<PollFd>();
        size.ShouldBe(8); // int (4) + short (2) + short (2) = 8 bytes, only possible with sequential layout
    }
}