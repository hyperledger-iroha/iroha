using System.Text.Json;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Field evidence and merging: the decoder's merge of captures and the session's bounded, header-checked holding.</summary>
public sealed class PetalMergeTests
{
    /// <summary>The 3000-byte payload of the reference decoder tests.</summary>
    private static readonly PetalStreamEncoder SetupEncoder = new(
        Enumerable.Range(0, 3000).Select(static i => (byte)(unchecked((uint)i * 2_654_435_761u) >> 11)).ToArray(),
        2);

    /// <summary>The <c>one-pass</c> stream of the golden captures.</summary>
    private static readonly PetalStreamEncoder OnePass = new(PetalTestSupport.Payload(3000, 2), 2);

    private static PetalLuma Setup(ushort frame) => PetalTestSupport.RenderLuma(SetupEncoder, frame, 768, 2);

    /// <summary>The 512-pixel render the session tests film.</summary>
    private static PetalRgbImage Source(PetalStreamEncoder encoder, ushort frame) =>
        PetalRenderer.Render(encoder.Cells(frame), new PetalRenderOptions { Size = 512, Supersample = 2 });

    private static PetalDecodedFrame With(PetalDecodedFrame frame, PetalLaneResult? d, PetalLaneResult?[] fields) =>
        new(frame.Pose, frame.Rotation, frame.Mirrored, d, fields, frame.InferredCorner);

    private static PetalDecodedFrame WithoutFields(PetalDecodedFrame frame) => With(frame, frame.D, new PetalLaneResult?[PetalLayout.Sectors]);

    [Fact]
    public void MergingOneCaptureReadsTheFieldLanesDecodeRead()
    {
        foreach (var (name, image) in new[] { ("clean", Setup(5)), ("hidden corner", PetalTestSupport.HideBlossom(Setup(2), 3)) })
        {
            var frame = PetalDecoder.Decode(image);
            Assert.True(frame.Success, $"{name}: {frame.Error}");
            Assert.All(frame.Frame.Fields, static field => Assert.NotNull(field));
            var evidence = PetalDecoder.FieldEvidence(image, frame.Frame);
            Assert.NotNull(evidence);
            Assert.Equal(frame.Frame.Fields, PetalDecoder.MergeFields([evidence]));
        }
    }

    [Fact]
    public void AveragingCapturesCancelsTheirIndependentNoise()
    {
        var truth = SetupEncoder.LaneData(5);
        var clean = Setup(5);
        var frame = PetalDecoder.Decode(clean);
        Assert.True(frame.Success, frame.Error.ToString());
        var cleanEvidence = PetalDecoder.FieldEvidence(clean, frame.Frame);
        Assert.NotNull(cleanEvidence);
        var rng = new PetalXorshift32(9);
        // uniform noise of ±0.6 of the lit level per cell: about one bit in eight flips in one
        // capture, far more than a field codeword corrects
        var captures = Enumerable.Range(0, 4)
            .Select(_ => new PetalFieldEvidence(cleanEvidence.Levels.Select(v => v + (rng.NextByte() - 127.5) / 213.0).ToArray()))
            .ToArray();
        foreach (var capture in captures)
        {
            Assert.True(
                PetalDecoder.MergeFields([capture]).All(static lane => lane is null),
                "one noisy capture must not read a field lane, or the test proves nothing");
        }

        var merged = PetalDecoder.MergeFields(captures);
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            Assert.Equal(truth.Fields[sector], merged[sector]?.Data);
        // skipped sectors are not read
        var skipped = PetalDecoder.MergeFields(captures, [true, false, true, true]);
        Assert.True(skipped[0] is null && skipped[1] is not null && skipped[2] is null && skipped[3] is null);
        Assert.Throws<ArgumentException>(() => PetalDecoder.MergeFields(captures, [true]));
    }

    [Fact]
    public void EvidenceNeedsAUsableImageAndARealCorner()
    {
        var image = Setup(5);
        var frame = PetalDecoder.Decode(image).Frame!;
        Assert.Null(PetalDecoder.FieldEvidence(new PetalLuma(20, 20), frame));
        foreach (var corner in new[] { 7, 4, -1 })
            Assert.Null(PetalDecoder.FieldEvidence(image, new PetalDecodedFrame(frame.Pose, 0, false, frame.D, frame.Fields.ToArray(), corner)));
        Assert.Equal(new PetalLaneResult?[4], PetalDecoder.MergeFields(Array.Empty<PetalFieldEvidence>()));
    }

    [Fact]
    public void EvidenceIsOwnedByItsCapture()
    {
        var capture = PetalTestSupport.Captures.GetProperty("captures")[0];
        var image = PetalTestSupport.LumaOf(capture);
        var decoded = PetalDecoder.Decode(image).Frame!;
        var evidence = PetalDecoder.FieldEvidence(image, decoded)!;
        var expected = PetalDecoder.MergeFields([evidence]);
        Assert.Equal(decoded.Fields, expected);
        Array.Clear(image.Data);
        Assert.Equal(expected, PetalDecoder.MergeFields([evidence]));
    }

    [Theory]
    [InlineData("merges", "noisy-480p")]
    [InlineData("mixed_merges", "mixed-frames-480p")]
    public void EveryCaptureOfAMergeSetMergesAloneToWhatItDecoded(string group, string name)
    {
        var set = PetalTestSupport.Captures.GetProperty(group).EnumerateArray().Single(s => s.GetProperty("name").GetString() == name);
        var (width, height) = (set.GetProperty("width").GetInt32(), set.GetProperty("height").GetInt32());
        foreach (var capture in set.GetProperty("captures").EnumerateArray())
        {
            var image = PetalTestSupport.LumaOf(capture, width, height);
            var decoded = PetalDecoder.Decode(image);
            Assert.True(decoded.Success, decoded.Error.ToString());
            var evidence = PetalDecoder.FieldEvidence(image, decoded.Frame);
            Assert.NotNull(evidence);
            Assert.Equal(decoded.Frame.Fields, PetalDecoder.MergeFields([evidence]));
        }
    }

    [Fact]
    public void MergingRecoversFieldLanesThatNoSingleCaptureReads()
    {
        var shown = Source(OnePass, 5);
        // a soft 480p camera in very dim light: every capture loses its field lanes to sensor
        // noise, which differs from capture to capture
        var config = PetalCaptureSimulator.FitToFrame(
            PetalCaptureConfig.Modern() with
            {
                Width = 640,
                Height = 480,
                Fill = 0.85,
                RotationDeg = 31.0,
                TiltXDeg = -7.0,
                BlurSigma = 1.4,
                Noise = 50.0,
            },
            4.0);
        var session = new PetalScanSession();
        var merged = string.Empty;
        var singles = 0;
        for (var i = 0; i < 4; i++)
        {
            var image = PetalCaptureSimulator.Capture(shown, config with { Seed = 1201UL + (ulong)i });
            var single = PetalDecoder.Decode(image);
            Assert.True(single.Success, $"capture {i}: lane D reads ({single.Error})");
            singles += single.Frame.Fields.Count(static f => f is not null);
            merged += session.Push(image, 33L * i).Merged;
        }

        Assert.True(singles <= 1, $"the captures read {singles} field lanes alone");
        Assert.True(merged.Length >= 3, $"merging recovered {merged}");
        var stats = session.Stats;
        Assert.Equal((uint)merged.Length, stats.MergedLanes);
        Assert.True(stats.Merges >= 1);
        // the merged atoms wait for the beacon, whose lane D the same camera reads
        Assert.Equal(0u, session.Progress.AtomsReceived);
        var beacon = PetalCaptureSimulator.Capture(Source(OnePass, 4), config with { Seed = 1205 });
        session.Push(beacon, 200);
        Assert.Equal(OnePass.Meta, session.Progress.Meta);
        Assert.True(session.Progress.AtomsReceived >= 5 * merged.Length);
    }

    /// <summary>
    /// A clean capture of frame <paramref name="petals"/> whose decoded lanes are replaced by lane
    /// <c>D</c> of frame <paramref name="rings"/>, as if the rings showed another frame than the
    /// petal field.
    /// </summary>
    private static (PetalLuma Image, PetalDecodedFrame Frame) RingsOfAnotherFrame(ushort petals, ushort rings, ulong seed)
    {
        var config = PetalCaptureSimulator.FitToFrame(
            PetalCaptureConfig.Modern() with { Width = 640, Height = 480, RotationDeg = 25.0, Seed = seed },
            4.0);
        var image = PetalCaptureSimulator.Capture(Source(OnePass, petals), config);
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.All(decoded.Frame.Fields, static field => Assert.NotNull(field));
        var frame = With(decoded.Frame, new PetalLaneResult(OnePass.LaneData(rings).D, 0, 0), new PetalLaneResult?[PetalLayout.Sectors]);
        return (image, frame);
    }

    [Fact]
    public void AMergeNeverOffersAtomsOfAnotherShownFrame()
    {
        var session = new PetalScanSession();
        for (var seed = 1UL; seed <= 3; seed++)
        {
            var (image, frame) = RingsOfAnotherFrame(11, 10, seed);
            Assert.True(session.Merge(image, frame).Length == 0, $"capture {seed}");
        }

        Assert.Equal((2u, 0u), (session.Stats.Merges, session.Stats.MergedLanes));
        Assert.Equal(0u, session.Progress.AtomsReceived);
        // what was refused: the merged field lanes name frame 11, not frame 10
        var held = session.HeldFrames[0];
        Assert.Equal(10, held.Shown.Frame);
        var merged = PetalDecoder.MergeFields(held.Evidence);
        var truth = OnePass.LaneData(11);
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            Assert.Equal(truth.Fields[sector], merged[sector]?.Data);
    }

    [Fact]
    public void TornCapturesWhoseLanesNameDifferentFramesAreNotHeld()
    {
        var session = new PetalScanSession();
        var (image, frame) = RingsOfAnotherFrame(11, 10, 1);
        var fields = new PetalLaneResult?[PetalLayout.Sectors];
        fields[2] = new PetalLaneResult(OnePass.LaneData(11).Fields[2], 0, 0);
        Assert.Empty(session.Merge(image, With(frame, frame.D, fields)));
        Assert.Empty(session.HeldFrames);
        // a frame without any lane names no shown frame either
        Assert.Empty(session.Merge(image, With(frame, null, new PetalLaneResult?[PetalLayout.Sectors])));
        Assert.Empty(session.HeldFrames);
    }

    [Fact]
    public void TheSessionHoldsAtMostFourCapturesOfSixShownFrames()
    {
        var session = new PetalScanSession();
        // refused merges never finish a shown frame, so its captures stay held
        for (var seed = 1UL; seed <= PetalScanSession.MergeCaptures + 2; seed++)
        {
            var (image, frame) = RingsOfAnotherFrame(11, 10, seed);
            session.Merge(image, frame);
        }

        Assert.Single(session.HeldFrames);
        Assert.Equal(PetalScanSession.MergeCaptures, session.HeldFrames[0].Evidence.Count);
        Assert.Equal((uint)PetalScanSession.MergeCaptures + 1, session.Stats.Merges);
        var (first, _) = RingsOfAnotherFrame(11, 10, 1);
        for (var rings = (ushort)12; rings < 12 + PetalScanSession.MergeWindow; rings++)
        {
            var (_, frame) = RingsOfAnotherFrame(11, rings, 1);
            session.Merge(first, frame);
        }

        Assert.Equal(PetalScanSession.MergeWindow, session.HeldFrames.Count);
        Assert.All(session.HeldFrames, static held => Assert.True(held.Shown.Frame >= 12));
        var merges = session.Stats.Merges;
        session.Reset();
        Assert.Empty(session.HeldFrames);
        Assert.Equal(merges, session.Stats.Merges);
    }

    [Fact]
    public void SwitchingStreamsWithTheSameShortTagDiscardsOldMergeState()
    {
        static PetalStreamEncoder Encoder(byte last)
        {
            var payload = new byte[100];
            payload[3] = last;
            return new PetalStreamEncoder(payload, 2);
        }

        var old = Encoder(49);
        var next = Encoder(128);
        Assert.Equal((61, 61), (old.Meta.Tag, next.Meta.Tag));
        Assert.NotEqual(old.Meta, next.Meta);
        var oldImage = Source(old, 0).ToLuma();
        var nextImage = Source(next, 0).ToLuma();
        var oldFull = PetalDecoder.Decode(oldImage).Frame!;
        var nextPartial = WithoutFields(PetalDecoder.Decode(nextImage).Frame!);
        foreach (var wasComplete in new[] { false, true })
        {
            var session = new PetalScanSession();
            var previous = wasComplete ? oldFull : WithoutFields(oldFull);
            session.Absorb(previous);
            session.Merge(oldImage, previous);
            Assert.NotEmpty(session.HeldFrames);
            session.Absorb(nextPartial);
            session.Absorb(nextPartial);
            Assert.Equal(next.Meta, session.Progress.Meta);
            Assert.Empty(session.HeldFrames);
            Assert.Empty(session.Merge(nextImage, nextPartial));
            Assert.Single(session.HeldFrames[0].Evidence);
            Assert.All(session.HeldFrames[0].Done, static done => Assert.False(done));
            Assert.Equal("0123", session.Merge(nextImage, nextPartial));
        }
    }

    [Fact]
    public void TheFirstBeaconPreservesEvidenceReceivedBeforeIt()
    {
        var image = Source(OnePass, 1).ToLuma();
        var partial = WithoutFields(PetalDecoder.Decode(image).Frame!);
        var session = new PetalScanSession();
        session.Absorb(partial);
        session.Merge(image, partial);
        Assert.Null(session.Progress.Meta);
        Assert.Single(session.HeldFrames[0].Evidence);
        var beaconImage = Source(OnePass, 0).ToLuma();
        session.Absorb(WithoutFields(PetalDecoder.Decode(beaconImage).Frame!));
        Assert.Equal(OnePass.Meta, session.Progress.Meta);
        Assert.Single(session.HeldFrames[0].Evidence);
        Assert.Equal("0123", session.Merge(image, partial));
        Assert.Equal(4u, session.Stats.MergedLanes);
    }

    [Fact]
    public void HeldEvidenceIsBoundedPerShownFrameAndDroppedOnceItsFieldIsRead()
    {
        var image = Setup(5);
        var original = PetalDecoder.Decode(image).Frame!;
        PetalDecodedFrame OnlyD(ushort frame)
        {
            var data = (byte[])original.D!.Data.Clone();
            data[1] = (byte)(frame >> 8);
            data[2] = (byte)frame;
            return With(original, new PetalLaneResult(data, 0, 0), new PetalLaneResult?[PetalLayout.Sectors]);
        }

        var session = new PetalScanSession();
        for (ushort frame = 100; frame < 110; frame++)
        {
            for (var sample = 0; sample < 6; sample++)
                Assert.Empty(session.Merge(image, OnlyD(frame)));
        }

        Assert.Equal(PetalScanSession.MergeWindow, session.HeldFrames.Count);
        Assert.All(session.HeldFrames, static held => Assert.Equal(PetalScanSession.MergeCaptures, held.Evidence.Count));
        // a capture that reads every field lane of a held frame finishes it and drops its evidence
        var complete = With(OnlyD(109), OnlyD(109).D, original.Fields.ToArray());
        Assert.False(complete.Lanes.Length == 0);
        var merges = session.Stats.Merges;
        Assert.Empty(session.Merge(image, ShownAs(complete, 109)));
        Assert.Empty(session.HeldFrames.Single(static held => held.Shown.Frame == 109).Evidence);
        Assert.Equal(merges, session.Stats.Merges);
    }

    /// <summary>The frame with every lane header rewritten to name shown frame <paramref name="shown"/>.</summary>
    private static PetalDecodedFrame ShownAs(PetalDecodedFrame frame, ushort shown)
    {
        PetalLaneResult? Rename(PetalLaneResult? lane)
        {
            if (lane is null)
                return null;
            var data = (byte[])lane.Data.Clone();
            data[1] = (byte)(shown >> 8);
            data[2] = (byte)shown;
            return new PetalLaneResult(data, lane.Corrected, lane.Erasures);
        }

        return With(frame, Rename(frame.D), frame.Fields.Select(Rename).ToArray());
    }
}
