using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Ports of the Rust stream tests plus assembler limit, loss and reorder coverage.</summary>
public sealed class PetalStreamTests
{
    private static readonly PetalLane[] AllLanes = [PetalLane.D, PetalLane.P, PetalLane.K];

    [Fact]
    public void AtomIdsAreContiguousAcrossFrames()
    {
        var expected = 0u;
        for (var frame = 0; frame <= 70; frame++)
        {
            Assert.Equal(expected, PetalStream.FirstAtomId((ushort)frame));
            expected += (uint)PetalStream.AtomsInFrame((ushort)frame);
        }

        Assert.Equal(6, PetalStream.AtomsInFrame(0));
        Assert.Equal(7, PetalStream.AtomsInFrame(1));
        Assert.Equal(PetalStream.FirstAtomId(4) + 1, PetalStream.LaneFirstId(PetalLane.K, 4));
        Assert.Equal(PetalStream.FirstAtomId(5) + 2, PetalStream.LaneFirstId(PetalLane.K, 5));
        Assert.Equal(PetalStream.FirstAtomId(5) + 1, PetalStream.LaneFirstId(PetalLane.D, 5));
        // the frame counter wraps on a beacon frame, so ids repeat cleanly
        Assert.True(PetalStream.IsBeaconFrame(0) && 65_536 % PetalStream.BeaconInterval == 0);
    }

    [Fact]
    public void CleanStreamCompletesAfterOneSystematicPass()
    {
        var data = PetalTestSupport.Payload(1_000, 21);
        var encoder = new PetalStreamEncoder(data, 2);
        var assembler = new PetalStreamAssembler();
        for (var frame = 0; frame < encoder.SystematicFrames; frame++)
            PetalTestSupport.FeedFrame(assembler, encoder, (ushort)frame, AllLanes);
        var done = assembler.TakeCompleted();
        Assert.NotNull(done);
        Assert.Equal(data, done.ToArray());
        Assert.Equal(2, done.Meta.Kind);
        Assert.True(assembler.Progress.Complete);
        Assert.Null(assembler.TakeCompleted());
    }

    [Fact]
    public void AnySingleLaneIsEnoughGivenABeacon()
    {
        var data = PetalTestSupport.Payload(400, 22);
        var encoder = new PetalStreamEncoder(data, 1);
        foreach (var lane in new[] { PetalLane.P, PetalLane.K, PetalLane.D })
        {
            var assembler = new PetalStreamAssembler();
            PetalTestSupport.FeedFrame(assembler, encoder, 0, PetalLane.D);
            for (var frame = 0; frame < 400; frame++)
            {
                PetalTestSupport.FeedFrame(assembler, encoder, (ushort)frame, lane);
                if (assembler.Progress.Complete)
                    break;
            }

            var done = assembler.TakeCompleted();
            Assert.True(done is not null, $"{lane} alone");
            Assert.Equal(data, done.ToArray());
        }
    }

    [Fact]
    public void AtomsSeenBeforeTheFirstBeaconAreNotLost()
    {
        var data = PetalTestSupport.Payload(300, 23);
        var encoder = new PetalStreamEncoder(data, 1);
        var assembler = new PetalStreamAssembler();
        // frames 1..: no beacon among them until frame 4
        for (var frame = 1; frame < 4; frame++)
            PetalTestSupport.FeedFrame(assembler, encoder, (ushort)frame, PetalLane.P, PetalLane.K, PetalLane.D);
        Assert.Null(assembler.Progress.Meta);
        for (var frame = 4; frame < encoder.SystematicFrames + 4; frame++)
            PetalTestSupport.FeedFrame(assembler, encoder, (ushort)frame, PetalLane.P, PetalLane.K, PetalLane.D);
        Assert.Equal(data, assembler.TakeCompleted()!.ToArray());
    }

    [Fact]
    public void JoiningMidStreamAndLosingFramesStillCompletes()
    {
        var data = PetalTestSupport.Payload(2_500, 24);
        var encoder = new PetalStreamEncoder(data, 3);
        var assembler = new PetalStreamAssembler();
        var rng = new PetalXorshift32(77);
        var frame = (ushort)15; // join late
        var shown = 0;
        while (!assembler.Progress.Complete)
        {
            if (rng.NextUInt32() % 10 < 6)
            {
                // 60 % of the frames are readable
                PetalTestSupport.FeedFrame(assembler, encoder, frame, AllLanes);
            }

            frame = unchecked((ushort)(frame + 1));
            shown++;
            Assert.True(shown < 600, "stream failed to complete");
        }

        Assert.Equal(data, assembler.TakeCompleted()!.ToArray());
    }

    [Fact]
    public void ShuffledFramesWithLossesStillComplete()
    {
        var data = PetalTestSupport.Payload(3_000, 31);
        var encoder = new PetalStreamEncoder(data, 1);
        var assembler = new PetalStreamAssembler();
        var rng = new PetalLcg(2024);
        // a pool of six systematic passes, shuffled; atoms seen before the first
        // beacon wait in the bounded pending queue
        var frames = Enumerable.Range(0, 6 * encoder.SystematicFrames).Select(static f => (ushort)f).ToArray();
        for (var i = frames.Length - 1; i > 0; i--)
        {
            var j = rng.Below(i + 1);
            (frames[i], frames[j]) = (frames[j], frames[i]);
        }

        var used = 0;
        foreach (var frame in frames)
        {
            // half of the frames are lost; surviving frames lose lanes at random too
            if (rng.Below(2) == 0)
                continue;
            var lanes = AllLanes.Where(_ => rng.Below(4) != 0).ToArray();
            PetalTestSupport.FeedFrame(assembler, encoder, frame, lanes);
            used++;
            if (assembler.Progress.Complete)
                break;
        }

        Assert.True(assembler.Progress.Complete, $"incomplete after {used} frames");
        Assert.Equal(data, assembler.TakeCompleted()!.ToArray());
        Assert.Equal(0u, assembler.Progress.IntegrityFailures);
    }

    [Fact]
    public void ADifferentStreamReplacesTheActiveOneAfterTwoBeacons()
    {
        var first = new PetalStreamEncoder(PetalTestSupport.Payload(100, 1), 1);
        var second = new PetalStreamEncoder(PetalTestSupport.Payload(100, 2), 1);
        var assembler = new PetalStreamAssembler();
        PetalTestSupport.FeedFrame(assembler, first, 0, PetalLane.D);
        Assert.Equal(first.Meta, assembler.Progress.Meta);
        PetalTestSupport.FeedFrame(assembler, second, 0, PetalLane.D);
        Assert.Equal(first.Meta, assembler.Progress.Meta);
        PetalTestSupport.FeedFrame(assembler, second, 4, PetalLane.D);
        Assert.Equal(second.Meta, assembler.Progress.Meta);
    }

    [Fact]
    public void ARepeatedActiveBeaconClearsAPendingSwitch()
    {
        var first = new PetalStreamEncoder(PetalTestSupport.Payload(100, 1), 1);
        var second = new PetalStreamEncoder(PetalTestSupport.Payload(100, 2), 1);
        var assembler = new PetalStreamAssembler();
        PetalTestSupport.FeedFrame(assembler, first, 0, PetalLane.D);
        PetalTestSupport.FeedFrame(assembler, second, 0, PetalLane.D);
        PetalTestSupport.FeedFrame(assembler, first, 4, PetalLane.D);
        PetalTestSupport.FeedFrame(assembler, second, 8, PetalLane.D);
        Assert.Equal(first.Meta, assembler.Progress.Meta);
    }

    [Fact]
    public void OversizedBeaconsAreIgnored()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(4_000, 5), 1);
        var assembler = new PetalStreamAssembler(new PetalAssemblerLimits { MaxPayloadLength = 1_000 });
        PetalTestSupport.FeedFrame(assembler, encoder, 0, PetalLane.D);
        Assert.Null(assembler.Progress.Meta);
        var fitting = new PetalStreamAssembler(new PetalAssemblerLimits { MaxPayloadLength = 4_000 });
        PetalTestSupport.FeedFrame(fitting, encoder, 0, PetalLane.D);
        Assert.Equal(encoder.Meta, fitting.Progress.Meta);
    }

    [Fact]
    public void PendingAtomsAreBoundedAndOldestAreDropped()
    {
        var data = PetalTestSupport.Payload(200, 26);
        var encoder = new PetalStreamEncoder(data, 1);
        var assembler = new PetalStreamAssembler(new PetalAssemblerLimits { MaxPendingAtoms = 3 });
        // frame 1 carries atoms 6..12: only the newest three (10, 11, 12) survive
        PetalTestSupport.FeedFrame(assembler, encoder, 1, PetalLane.P, PetalLane.D, PetalLane.K);
        PetalTestSupport.FeedFrame(assembler, encoder, 0, PetalLane.D);
        var progress = assembler.Progress;
        Assert.Equal(encoder.Meta, progress.Meta);
        Assert.Equal(3u, progress.AtomsReceived);
        Assert.Equal(3, progress.Rank);
    }

    [Fact]
    public void AZeroPendingLimitBuffersNothing()
    {
        var data = PetalTestSupport.Payload(300, 41);
        var encoder = new PetalStreamEncoder(data, 1);
        var assembler = new PetalStreamAssembler(new PetalAssemblerLimits { MaxPendingAtoms = 0 });
        // atoms before any beacon are dropped, not buffered
        for (var frame = 1; frame < 4; frame++)
            PetalTestSupport.FeedFrame(assembler, encoder, (ushort)frame, PetalLane.P, PetalLane.K, PetalLane.D);
        PetalTestSupport.FeedFrame(assembler, encoder, 4, PetalLane.D);
        Assert.Equal(encoder.Meta, assembler.Progress.Meta);
        Assert.Equal(0, assembler.Progress.Rank);
        Assert.Equal(0u, assembler.Progress.AtomsReceived);
    }

    [Fact]
    public void RandomStreamsAlwaysCompleteAndNeverDeliverWrongData()
    {
        // port of tests/streams.rs: arbitrary sizes, join points, loss and lane subsets
        var rng = new PetalXorshift32(0xC0FF_EE11);
        for (var trial = 0; trial < 400; trial++)
        {
            var length = (trial % 8) switch
            {
                0 => 1 + (int)(rng.NextUInt32() % 16),
                1 => 17 + (int)(rng.NextUInt32() % 100),
                _ => 1 + (int)(rng.NextUInt32() % 3_000),
            };
            var payload = new byte[length];
            for (var i = 0; i < length; i++)
                payload[i] = rng.NextByte();
            var kind = rng.NextByte();
            var encoder = new PetalStreamEncoder(payload, kind);
            var lossPercent = rng.NextUInt32() % 70;
            PetalLane[] lanes = (rng.NextUInt32() % 5) switch
            {
                0 => [PetalLane.P],
                1 => [PetalLane.D, PetalLane.P],
                2 => [PetalLane.K, PetalLane.D],
                _ => [PetalLane.P, PetalLane.K, PetalLane.D],
            };
            var assembler = new PetalStreamAssembler();
            var frame = (ushort)(rng.NextUInt32() & 0xFFFF);
            var shown = 0u;
            while (!assembler.Progress.Complete)
            {
                if (rng.NextUInt32() % 100 >= lossPercent)
                {
                    // only lane D carries the beacon: offer it on beacon frames whatever else is readable
                    var readable = lanes.ToList();
                    if (PetalStream.IsBeaconFrame(frame) && !readable.Contains(PetalLane.D))
                        readable.Add(PetalLane.D);
                    PetalTestSupport.FeedFrame(assembler, encoder, frame, readable.ToArray());
                }

                frame = unchecked((ushort)(frame + 1));
                shown++;
                var budget = 40 + 8 * ((uint)length / 13 + 2) * 100 / (100 - lossPercent);
                Assert.True(shown < budget, $"trial {trial}: {length} bytes, loss {lossPercent} %, exceeded {budget} frames");
            }

            var done = assembler.TakeCompleted();
            Assert.NotNull(done);
            Assert.Equal(payload, done.ToArray());
            Assert.Equal(kind, done.Meta.Kind);
        }
    }

    [Fact]
    public void CounterWraparoundKeepsAtomIdsConsistent()
    {
        var payload = Enumerable.Range(0, 2_000).Select(static i => unchecked((byte)(i * 7 + 3))).ToArray();
        var encoder = new PetalStreamEncoder(payload, 1);
        var assembler = new PetalStreamAssembler();
        // start a few frames before the 16-bit counter wraps and run across it
        var frame = (ushort)65_530;
        for (var i = 0; i < 200; i++)
        {
            PetalTestSupport.FeedFrame(assembler, encoder, frame, PetalLane.P, PetalLane.K, PetalLane.D);
            frame = unchecked((ushort)(frame + 1));
            if (assembler.Progress.Complete)
                break;
        }

        Assert.Equal(payload, assembler.TakeCompleted()!.ToArray());
    }

    [Fact]
    public void PendingAtomsOfAnotherStreamAreDiscardedAtStart()
    {
        var first = new PetalStreamEncoder(PetalTestSupport.Payload(100, 1), 1);
        var second = new PetalStreamEncoder(PetalTestSupport.Payload(100, 2), 1);
        Assert.NotEqual(first.Meta.Tag, second.Meta.Tag);
        var assembler = new PetalStreamAssembler();
        PetalTestSupport.FeedFrame(assembler, second, 1, PetalLane.P, PetalLane.K);
        PetalTestSupport.FeedFrame(assembler, first, 1, PetalLane.P);
        PetalTestSupport.FeedFrame(assembler, first, 0, PetalLane.D);
        Assert.Equal(1u, assembler.Progress.AtomsReceived);
        // atoms of a foreign stream are ignored once a stream is active
        PetalTestSupport.FeedFrame(assembler, second, 2, PetalLane.K);
        Assert.Equal(1u, assembler.Progress.AtomsReceived);
    }

    [Fact]
    public void AssemblerLimitsRejectNegativeValues()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalAssemblerLimits { MaxPayloadLength = -1 });
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalAssemblerLimits { MaxPendingAtoms = -1 });
        Assert.Equal(PetalStream.DefaultMaxPayloadLength, PetalAssemblerLimits.Default.MaxPayloadLength);
        Assert.Equal(128, PetalAssemblerLimits.Default.MaxPendingAtoms);
    }

    [Fact]
    public void CorruptAtomsAreCaughtByThePayloadCrc()
    {
        var data = PetalTestSupport.Payload(200, 25);
        var encoder = new PetalStreamEncoder(data, 1);
        var assembler = new PetalStreamAssembler();
        PetalTestSupport.FeedFrame(assembler, encoder, 0, PetalLane.D, PetalLane.K);
        // atom 0 arrives with a valid header but a wrong body
        assembler.PushAtoms(new PetalAtomPacket(
            new PetalLaneHeader(encoder.Meta.Tag, 0),
            0,
            [Enumerable.Repeat((byte)0xEE, PetalLanes.AtomLength).ToArray()]));
        for (var frame = 1; frame < encoder.SystematicFrames; frame++)
            PetalTestSupport.FeedFrame(assembler, encoder, (ushort)frame, PetalLane.P, PetalLane.K, PetalLane.D);
        Assert.Null(assembler.TakeCompleted());
        Assert.Equal(1u, assembler.Progress.IntegrityFailures);
        // clean repair frames after the reset recover the payload
        for (var frame = 100; frame < 600; frame++)
        {
            PetalTestSupport.FeedFrame(assembler, encoder, (ushort)frame, PetalLane.P, PetalLane.K, PetalLane.D);
            if (assembler.Progress.Complete)
                break;
        }

        Assert.Equal(data, assembler.TakeCompleted()!.ToArray());
        // integrity failures survive a reset; everything else is forgotten
        assembler.Reset();
        Assert.Equal(new PetalProgress(null, 0, 0, 0, 1, false), assembler.Progress);
    }

    [Fact]
    public void EncoderRejectsEmptyAndOversizedPayloads()
    {
        var empty = Assert.Throws<PetalStreamException>(() => new PetalStreamEncoder([], 0));
        Assert.Equal(PetalStreamErrorCode.EmptyPayload, empty.Code);
        var large = Assert.Throws<PetalStreamException>(() => new PetalStreamEncoder(new byte[PetalStream.MaxPayloadLength + 1], 0));
        Assert.Equal(PetalStreamErrorCode.PayloadTooLarge, large.Code);
        Assert.IsAssignableFrom<ArgumentException>(large);
    }

    [Fact]
    public void BeaconRoundtripsThroughLaneD()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(77, 9), 3);
        var (_, _, d) = encoder.LaneData(512);
        var lane = PetalStream.ParseDLane(d);
        Assert.NotNull(lane);
        Assert.True(lane.IsBeacon);
        var beacon = lane.Beacon!.Value;
        Assert.Equal(encoder.Meta, beacon.Meta);
        Assert.Equal(512, beacon.Header.Frame);
        Assert.Null(PetalStream.ParseDLane(d.AsSpan(0, 11)));
        var bad = (byte[])d.Clone();
        bad[3] = 0x20;
        Assert.Null(PetalStream.ParseDLane(bad));
        var zeroLength = (byte[])d.Clone();
        zeroLength[5] = zeroLength[6] = zeroLength[7] = 0;
        Assert.Null(PetalStream.ParseDLane(zeroLength));
        // non-beacon frames carry an atom instead
        var (_, _, atomLane) = encoder.LaneData(513);
        var parsed = PetalStream.ParseDLane(atomLane);
        Assert.NotNull(parsed);
        Assert.False(parsed.IsBeacon);
        Assert.Equal(PetalStream.LaneFirstId(PetalLane.D, 513), parsed.Atoms!.FirstId);
        Assert.Single(parsed.Atoms.Atoms);
    }

    [Fact]
    public void AtomLaneParsingChecksLaneAndLength()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(500, 10), 1);
        var (p, k, d) = encoder.LaneData(6);
        var packet = PetalStream.ParseAtomLane(PetalLane.K, k);
        Assert.NotNull(packet);
        Assert.Equal(5, packet.Atoms.Count);
        Assert.Equal(new PetalLaneHeader(encoder.Meta.Tag, 6), packet.Header);
        Assert.Equal(PetalStream.LaneFirstId(PetalLane.K, 6), packet.FirstId);
        Assert.Null(PetalStream.ParseAtomLane(PetalLane.D, d));
        Assert.Null(PetalStream.ParseAtomLane(PetalLane.P, k));
        Assert.Single(PetalStream.ParseAtomLane(PetalLane.P, p)!.Atoms);
        Assert.Equal(packet, PetalStream.ParseAtomLane(PetalLane.K, k));
        Assert.Throws<ArgumentException>(() => new PetalAtomPacket(default, 0, [new byte[3]]));
    }

    [Fact]
    public void StreamMetaDerivesTagAndSourceAtoms()
    {
        var meta = new PetalStreamMeta(2, 700, 0x1234_5683);
        Assert.Equal(0x83, meta.Tag);
        Assert.Equal(44, meta.SourceAtoms);
        Assert.Equal(1, new PetalStreamMeta(0, 1, 0).SourceAtoms);
        Assert.Equal(1, new PetalStreamMeta(0, 16, 0).SourceAtoms);
        Assert.Equal(2, new PetalStreamMeta(0, 17, 0).SourceAtoms);
    }
}
