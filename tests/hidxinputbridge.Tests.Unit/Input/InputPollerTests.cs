using HidXInputBridge.Input;
using NUnit.Framework;
using Shouldly;

namespace HidXInputBridge.Tests.Unit.Input;

[TestFixture]
public class PollResultTests
{
    [Test]
    public void PollResult_None_ShouldBeZero()
    {
        PollResult.None.ShouldBe((PollResult)0);
    }

    [Test]
    public void PollResult_EvdevReady_ShouldBeOne()
    {
        PollResult.EvdevReady.ShouldBe((PollResult)1);
    }

    [Test]
    public void PollResult_HidrawReady_ShouldBeTwo()
    {
        PollResult.HidrawReady.ShouldBe((PollResult)2);
    }

    [Test]
    public void PollResult_Error_ShouldBeFour()
    {
        PollResult.Error.ShouldBe((PollResult)4);
    }

    [Test]
    public void PollResult_Timeout_ShouldBeEight()
    {
        PollResult.Timeout.ShouldBe((PollResult)8);
    }

    [Test]
    public void PollResult_ShouldBeFlagsEnum()
    {
        var attribute = typeof(PollResult).GetCustomAttributes(typeof(FlagsAttribute), false)
                                          .FirstOrDefault();

        attribute.ShouldNotBeNull();
    }

    [Test]
    public void PollResult_Combined_ShouldWorkCorrectly()
    {
        var result = PollResult.EvdevReady | PollResult.HidrawReady;

        result.HasFlag(PollResult.EvdevReady).ShouldBeTrue();
        result.HasFlag(PollResult.HidrawReady).ShouldBeTrue();
        result.HasFlag(PollResult.Error).ShouldBeFalse();
        result.HasFlag(PollResult.Timeout).ShouldBeFalse();
    }

    [Test]
    public void PollResult_AllFlags_ShouldBeCombineable()
    {
        var result = PollResult.EvdevReady | PollResult.HidrawReady | PollResult.Error | PollResult.Timeout;

        result.HasFlag(PollResult.EvdevReady).ShouldBeTrue();
        result.HasFlag(PollResult.HidrawReady).ShouldBeTrue();
        result.HasFlag(PollResult.Error).ShouldBeTrue();
        result.HasFlag(PollResult.Timeout).ShouldBeTrue();
    }

    [Test]
    public void PollResult_None_ShouldHaveNoFlags()
    {
        var result = PollResult.None;

        result.HasFlag(PollResult.EvdevReady).ShouldBeFalse();
        result.HasFlag(PollResult.HidrawReady).ShouldBeFalse();
        result.HasFlag(PollResult.Error).ShouldBeFalse();
        result.HasFlag(PollResult.Timeout).ShouldBeFalse();
    }

    [Test]
    public void PollResult_EvdevAndError_ShouldBeDistinct()
    {
        var result = PollResult.EvdevReady | PollResult.Error;

        result.ShouldBe((PollResult)5); // 1 | 4
        result.HasFlag(PollResult.EvdevReady).ShouldBeTrue();
        result.HasFlag(PollResult.Error).ShouldBeTrue();
        result.HasFlag(PollResult.HidrawReady).ShouldBeFalse();
    }
}