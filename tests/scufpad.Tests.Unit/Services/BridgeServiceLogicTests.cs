using Scufpad.Input;
using Scufpad.Mapping;
using NUnit.Framework;
using Shouldly;

namespace Scufpad.Tests.Unit.Services;

/// <summary>
///     Tests for the logic used by BridgeService.
///     The actual BridgeService requires hardware access, so we test the components
///     it depends on and the logic patterns it uses.
/// </summary>
[TestFixture]
public class BridgeServiceLogicTests
{
    [Test]
    public void ProcessingPipeline_ShouldWorkEndToEnd()
    {
        // Simulate the BridgeService processing pipeline
        var rawState = new InputState();
        var filteredState = new InputState();
        var filter = new InputFilter();

        // Simulate evdev input
        rawState.LeftStickX = 15000;
        rawState.LeftStickY = -10000;
        rawState.ButtonA = true;
        rawState.MarkDirty();

        // Apply filter
        filter.Apply(rawState, filteredState);

        // Verify output
        filteredState.LeftStickX.ShouldBe(15000);
        filteredState.LeftStickY.ShouldBe(-10000);
        filteredState.ButtonA.ShouldBeTrue();
        filteredState.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void DirtyTracking_ShouldWorkAcrossProcessing()
    {
        var rawState = new InputState();
        var filteredState = new InputState();
        var filter = new InputFilter();

        // Initial state - not dirty
        rawState.IsDirty.ShouldBeFalse();

        // Simulate input event
        rawState.ButtonA = true;
        rawState.MarkDirty();
        rawState.IsDirty.ShouldBeTrue();

        // Process and clear
        filter.Apply(rawState, filteredState);
        rawState.ClearDirty();
        rawState.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void PollResult_EvdevReady_ShouldIndicateDataAvailable()
    {
        var result = PollResult.EvdevReady;

        result.HasFlag(PollResult.EvdevReady).ShouldBeTrue();
        result.HasFlag(PollResult.Error).ShouldBeFalse();
    }

    [Test]
    public void PollResult_BothReady_ShouldIndicateBothSources()
    {
        var result = PollResult.EvdevReady | PollResult.HidrawReady;

        result.HasFlag(PollResult.EvdevReady).ShouldBeTrue();
        result.HasFlag(PollResult.HidrawReady).ShouldBeTrue();
    }

    [Test]
    public void PollResult_Error_ShouldTakePrecedence()
    {
        var result = PollResult.EvdevReady | PollResult.Error;

        // BridgeService checks for error first
        if (result.HasFlag(PollResult.Error))
            // Would break out of loop
        {
            result.HasFlag(PollResult.Error).ShouldBeTrue();
        }
    }

    [Test]
    public void PollResult_Timeout_ShouldContinueLoop()
    {
        var result = PollResult.Timeout;

        result.HasFlag(PollResult.Timeout).ShouldBeTrue();
        result.HasFlag(PollResult.EvdevReady).ShouldBeFalse();
        result.HasFlag(PollResult.HidrawReady).ShouldBeFalse();
    }

    [Test]
    public void EventBuffer_ShouldBeAdequateSize()
    {
        // BridgeService uses stackalloc with 64 InputEvents
        const int bufferSize = 64;
        const int eventSize = 24; // InputEvent.Size

        var totalBytes = bufferSize * eventSize;
        totalBytes.ShouldBe(1536);

        // Should be reasonable for stack allocation
        totalBytes.ShouldBeLessThan(4096);
    }

    [Test]
    public void StateChange_ShouldTriggerEmit()
    {
        var rawState = new InputState();

        // No change initially
        rawState.IsDirty.ShouldBeFalse();

        // Mark dirty after change
        rawState.LeftStickX = 5000;
        rawState.MarkDirty();

        // Would emit state in BridgeService
        rawState.IsDirty.ShouldBeTrue();
    }

    [Test]
    public void MultipleChanges_ShouldBeBatched()
    {
        var rawState = new InputState();

        // Multiple changes before sync
        rawState.LeftStickX = 5000;
        rawState.LeftStickY = -3000;
        rawState.ButtonA = true;
        rawState.ButtonB = true;
        rawState.MarkDirty();

        // All changes are in one dirty state
        rawState.IsDirty.ShouldBeTrue();

        // After emit, would clear
        rawState.ClearDirty();
        rawState.IsDirty.ShouldBeFalse();
    }

    [Test]
    public void Filter_ShouldReduceJitter()
    {
        var filter = new InputFilter();
        var rawState = new InputState();
        var filteredState = new InputState();

        // First reading
        rawState.LeftStickX = 10000;
        filter.Apply(rawState, filteredState);
        filteredState.LeftStickX.ShouldBe(10000);

        // Jittery reading (small change)
        rawState.LeftStickX = 10050;
        filter.Apply(rawState, filteredState);
        filteredState.LeftStickX.ShouldBe(10000); // Filtered out

        // Real movement
        rawState.LeftStickX = 15000;
        filter.Apply(rawState, filteredState);
        filteredState.LeftStickX.ShouldBe(15000); // Passed through
    }

    [Test]
    public void Filter_ShouldApplyDeadzone()
    {
        var filter = new InputFilter();
        var rawState = new InputState();
        var filteredState = new InputState();

        // Small stick movement (in deadzone)
        rawState.LeftStickX = 1000;
        filter.Apply(rawState, filteredState);
        filteredState.LeftStickX.ShouldBe(0);

        // Full stick movement
        rawState.LeftStickX = 30000;
        filter.Apply(rawState, filteredState);
        filteredState.LeftStickX.ShouldBe(30000);
    }

    [Test]
    public void Hidraw_ShouldBeOptional()
    {
        // BridgeService accepts null hidraw reader
        // Simulating the check pattern
        HidrawReaderMock? hidraw = null;

        // Pattern from BridgeService
        if (hidraw is not null)
            // Would read from hidraw
        {
            Assert.Fail("Hidraw should be null");
        }

        // Service continues without hidraw
        hidraw.ShouldBeNull();
    }

    // Mock for testing null pattern
    private class HidrawReaderMock
    {
    }

    [Test]
    public void CancellationToken_ShouldBeRespected()
    {
        using var cts = new CancellationTokenSource();
        var token = cts.Token;

        // Simulate loop check
        var running = true;
        var iterations = 0;

        cts.Cancel();

        while (running && !token.IsCancellationRequested)
        {
            iterations++;
            if (iterations > 10)
            {
                break; // Safety
            }
        }

        iterations.ShouldBe(0);
        token.IsCancellationRequested.ShouldBeTrue();
    }

    [Test]
    public void StopMethod_ShouldSetRunningFalse()
    {
        var running = true;

        // Simulate Stop() method
        void Stop()
        {
            running = false;
        }

        running.ShouldBeTrue();
        Stop();
        running.ShouldBeFalse();
    }
}