using System.Numerics;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Ports of the Rust turn layer tests (<c>turns.rs</c>) plus argument and robustness coverage.</summary>
public sealed class PetalTurnTests
{
    /// <summary>The stream of the reference turn tests: 6,000 bytes from xorshift32 seed 5.</summary>
    private static readonly PetalStreamEncoder Stream = new(PetalTestSupport.Payload(6000, 5), 2);

    /// <summary>The clean render of frame 6, shared by several tests.</summary>
    private static readonly Lazy<PetalLuma> CleanSix = new(() => Rendered(Stream.Cells(6)).ToLuma());

    /// <summary>Four test atoms (64 bytes) derived from <paramref name="seed"/>.</summary>
    private static byte[] Atoms(byte seed) =>
        Enumerable.Range(0, PetalTurns.TurnAtoms * PetalLanes.AtomLength).Select(b => unchecked((byte)((b * 29) ^ seed))).ToArray();

    /// <summary>A 768-pixel render (2× supersampling), the size of the reference turn tests.</summary>
    private static PetalRgbImage Rendered(PetalFrameCells cells) =>
        PetalRenderer.Render(cells, new PetalRenderOptions { Size = 768, Supersample = 2 });

    private static PetalDecodedFrame WithFields(PetalDecodedFrame frame, PetalLaneResult?[] fields) =>
        new(frame.Pose, frame.Rotation, frame.Mirrored, frame.D, fields, frame.InferredCorner);

    [Fact]
    public void LaneSizesAreConsistent()
    {
        Assert.Equal((90, 68), (PetalTurns.TurnWordLength, PetalTurns.TurnDataLength));
        Assert.Equal(PetalTurns.TurnAtoms * PetalLanes.AtomLength + PetalTurns.TurnCheckLength, PetalTurns.TurnDataLength);
        Assert.Equal(1u << 31, PetalTurns.TurnFirstId(0, 0));
        Assert.Equal((1u << 31) + 16 + 12, PetalTurns.TurnFirstId(1, 3));
        Assert.Equal((1u << 31) + 16u * 65_535 + 15, PetalTurns.TurnFirstId(ushort.MaxValue, 3) + 3);
        // the base schedule stays far below the turn ids however long a stream runs
        Assert.True(PetalStream.FirstAtomId(ushort.MaxValue) + 21 < PetalTurns.TurnIdBase);
    }

    [Fact]
    public void GrayLabelsDifferInOneBitBetweenNeighbouringTurns()
    {
        for (byte position = 0; position < PetalTurns.TurnPositions; position++)
        {
            Assert.Equal(position, PetalTurns.TurnPosition(PetalTurns.TurnLabel(position)));
            var next = (byte)((position + 1) % PetalTurns.TurnPositions);
            Assert.Equal(1, BitOperations.PopCount((uint)(PetalTurns.TurnLabel(position) ^ PetalTurns.TurnLabel(next))));
        }

        Assert.True(Math.Abs(PetalTurns.TurnAngle(2) - Math.PI / 2.0) < 1e-12);
    }

    [Fact]
    public void CarrierOrdersShuffleEachSectorsCodewordCells()
    {
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            var codeword = PetalLayout.SectorCellOrder(sector).Take(PetalLanes.FieldWordLength * 8).ToArray();
            var order = PetalTurns.CarrierOrder(sector).ToArray();
            Assert.NotEqual(codeword, order);
            Assert.Equal(codeword.Order(), order.Order());
        }

        // the four shuffles differ
        int[] Positions(int sector) =>
            PetalTurns.CarrierOrder(sector).Select(cell => PetalLayout.SectorCellOrder(sector).ToList().IndexOf(cell)).ToArray();
        Assert.NotEqual(Positions(0), Positions(1));
    }

    [Fact]
    public void ASectorWithTooFewLitPetalsCarriesNothing()
    {
        var petals = new bool[PetalLayout.FieldCells];
        var order = PetalTurns.CarrierOrder(2);
        foreach (var cell in order.Take(PetalTurns.TurnCarriers - 1))
            petals[cell] = true;
        Assert.Null(PetalTurns.Carriers(petals, 2));
        petals[order[PetalTurns.TurnCarriers + 7]] = true;
        var chosen = PetalTurns.Carriers(petals, 2);
        Assert.NotNull(chosen);
        Assert.Equal(PetalTurns.TurnCarriers, chosen.Length);
        Assert.Equal(order[PetalTurns.TurnCarriers + 7], chosen[PetalTurns.TurnCarriers - 1]);
    }

    [Fact]
    public void TurnLanesRoundtripAndBindTheirHeader()
    {
        var header = new PetalLaneHeader(7, 300);
        var data = PetalTurns.TurnData(header, 1, Atoms(3));
        var word = PetalTurns.EncodeTurnLane(1, data);
        Assert.Equal(PetalTurns.TurnWordLength, word.Length);
        Assert.True(PetalTurns.TryDecodeTurnLane(1, word, [], out var decoded, out var corrected));
        Assert.Equal(data, decoded);
        Assert.Equal(0, corrected);
        var packet = PetalTurns.TurnPacket(header, 1, decoded);
        Assert.NotNull(packet);
        Assert.Equal(PetalTurns.TurnFirstId(300, 1), packet.FirstId);
        Assert.Equal(PetalTurns.TurnAtoms, packet.Atoms.Count);
        Assert.Equal(Atoms(3)[32..48], packet.Atoms[2]);
        // another frame, tag or sector does not match the check
        Assert.Null(PetalTurns.TurnPacket(new PetalLaneHeader(7, 301), 1, decoded));
        Assert.Null(PetalTurns.TurnPacket(new PetalLaneHeader(8, 300), 1, decoded));
        Assert.Null(PetalTurns.TurnPacket(header, 2, decoded));
        // a codeword whitened for another sector does not decode
        Assert.False(PetalTurns.TryDecodeTurnLane(0, word, [], out _, out _));
    }

    [Fact]
    public void CarriersShowTheCodewordTwoBitsAtATime()
    {
        var fields = Enumerable.Range(0, PetalLayout.Sectors)
            .Select(static s => PetalLanes.EncodeLane(
                PetalLanes.Field(s),
                Enumerable.Range(0, PetalLanes.FieldDataLength).Select(b => unchecked((byte)((b * 13) ^ s))).ToArray()))
            .ToArray();
        var cells = PetalFrameCells.FromWords(new byte[PetalLanes.DWordLength], fields);
        Assert.All(cells.Turns, static turn => Assert.Equal(0, turn));
        var words = Enumerable.Range(0, PetalLayout.Sectors)
            .Select(static s => PetalTurns.EncodeTurnLane(s, PetalTurns.TurnData(new PetalLaneHeader(1, 2), s, Atoms((byte)s))))
            .ToArray();
        cells.SetTurns(words);
        for (var sector = 0; sector < words.Length; sector++)
        {
            Assert.Equal(words[sector], cells.TurnWord(sector));
            var chosen = PetalTurns.Carriers(cells.Petals, sector);
            Assert.NotNull(chosen);
            for (var j = 0; j < chosen.Length; j++)
                Assert.Equal(PetalTurns.CarrierPosition(words[sector], j), cells.Turns[chosen[j]]);
        }

        // dark petals and lit petals beyond the carriers stay unturned
        var carried = Enumerable.Range(0, PetalLayout.Sectors)
            .Sum(s => PetalTurns.Carriers(cells.Petals, s)!.Count(cell => cells.Turns[cell] != 0));
        var turned = cells.Turns.Count(static turn => turn != 0);
        Assert.Equal(carried, turned);
        Assert.True(turned > PetalLayout.Sectors * PetalTurns.TurnCarriers * 6 / 10, $"{turned} turned petals");
    }

    [Fact]
    public void ATurnedPetalStaysOutOfItsNeighboursSamplingDiscs()
    {
        // the nearest petal centres (radially adjacent cells) are 11 units apart; a petal reaches
        // its half-length from its centre whichever way it is turned
        var nearest = double.MaxValue;
        for (var near = 0; near < PetalTurns.ReaderTables.NearCells.Length; near++)
            nearest = Math.Min(nearest, double.Hypot(PetalTurns.ReaderTables.NearAlong[near], PetalTurns.ReaderTables.NearAcross[near]));
        Assert.True(Math.Abs(nearest - 11.0) < 1e-9, $"{nearest}");
        Assert.True(nearest > PetalLayout.PetalHalfLength + PetalLayout.PetalRadius, $"{nearest}");
    }

    [Fact]
    public void ACleanRenderReadsEveryTurnLaneAndSkipsWhatItIsTold()
    {
        var truth = Stream.LaneData(6);
        var image = CleanSix.Value;
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.Equal("D0123", decoded.Frame.Lanes);
        var turns = PetalTurns.ReadTurns(image, decoded.Frame, [false, false, false, false]);
        for (var sector = 0; sector < turns.Length; sector++)
        {
            var result = turns[sector];
            Assert.True(result is not null, $"turn lane {sector}");
            Assert.Equal(truth.Turns[sector], result.Data);
            Assert.Equal(0, result.Corrected);
        }

        turns = PetalTurns.ReadTurns(image, decoded.Frame, [true, false, true, true]);
        Assert.Equal([false, true, false, false], turns.Select(static turn => turn is not null));
        // a sector whose field lane did not decode has no turn lane
        var fields = decoded.Frame.Fields.ToArray();
        fields[2] = null;
        turns = PetalTurns.ReadTurns(image, WithFields(decoded.Frame, fields));
        Assert.True(turns[2] is null && turns[3] is not null);
    }

    [Fact]
    public void AModernCameraReadsTheTurnLanes()
    {
        var truth = Stream.LaneData(9);
        var source = Rendered(Stream.Cells(9));
        foreach (var (seed, rotation) in new[] { (1UL, 17.0), (2UL, 205.0) })
        {
            var config = PetalCaptureSimulator.FitToFrame(
                PetalCaptureConfig.Modern() with { RotationDeg = rotation, TiltXDeg = 8.0, Seed = seed },
                4.0);
            var image = PetalCaptureSimulator.Capture(source, config);
            var decoded = PetalDecoder.Decode(image);
            Assert.True(decoded.Success, $"seed {seed}: {decoded.Error}");
            var turns = PetalTurns.ReadTurns(image, decoded.Frame);
            for (var sector = 0; sector < turns.Length; sector++)
            {
                if (decoded.Frame.Fields[sector] is null)
                    continue;
                var turn = turns[sector];
                Assert.True(turn is not null && truth.Turns[sector].AsSpan().SequenceEqual(turn.Data), $"seed {seed} sector {sector}");
            }
        }
    }

    [Fact]
    public void UnturnedPetalsNeverPassForATurnLane()
    {
        var cells = Stream.Cells(3);
        Array.Clear(cells.Turns);
        var image = Rendered(cells).ToLuma();
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, $"the field lanes read without turns: {decoded.Error}");
        Assert.Equal("D0123", decoded.Frame.Lanes);
        Assert.All(PetalTurns.ReadTurns(image, decoded.Frame), static turn => Assert.Null(turn));
    }

    [Fact]
    public void TemplatesHoldAUnitPetalAndKernelsTellTheTurnsApart()
    {
        double Sharp(double along, double across) => PetalTurns.TurnTemplate(0, along, across);
        Assert.True(Sharp(0.0, 0.0) > 0.99);
        Assert.True(Sharp(0.0, 6.0) < 0.05 && Sharp(8.5, 0.0) < 0.01);
        Assert.True(Sharp(4.5, 0.0) > Sharp(0.0, 3.4));
        const int block = PetalTurns.TurnPositions * 169;
        for (var blur = 0; blur < PetalTurns.TurnBlurs.Count; blur++)
        {
            var kernels = PetalTurns.ReaderTables.Kernels.AsSpan(blur * block, block);
            for (var position = 0; position < PetalTurns.TurnPositions; position++)
            {
                // a window showing exactly the petal at turn k picks turn k
                var (decided, lead) = PetalTurns.Decide(kernels.Slice(position * 169, 169), kernels);
                Assert.Equal(position, decided);
                Assert.True(lead > 0.0, $"blur {blur} position {position}");
            }
        }
    }

    [Fact]
    public void ObservedTurnsOfACleanRenderAreTheDrawnTurns()
    {
        var cells = Stream.Cells(6);
        var image = CleanSix.Value;
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, decoded.Error.ToString());
        var observed = PetalTurns.ObservedTurns(image, decoded.Frame);
        Assert.NotNull(observed);
        Assert.Contains(observed.Blur, PetalTurns.TurnBlurs.Take(3));
        Assert.Equal(PetalLayout.Sectors * PetalTurns.TurnCarriers, observed.Carriers.Count);
        // sector by sector in carrier order, every one at its drawn turn
        var expected = Enumerable.Range(0, PetalLayout.Sectors).SelectMany(s => PetalTurns.Carriers(cells.Petals, s)!);
        Assert.Equal(expected, observed.Carriers.Select(static carrier => carrier.Cell));
        Assert.All(observed.Carriers, carrier => Assert.Equal(cells.Turns[carrier.Cell], carrier.Position));
        Assert.All(observed.Carriers, static carrier => Assert.True(carrier.Lead > 0.0));
        // a frame without field lanes has nothing to observe
        Assert.Null(PetalTurns.ObservedTurns(image, WithFields(decoded.Frame, new PetalLaneResult?[PetalLayout.Sectors])));
        Assert.All(PetalTurns.ReadTurns(image, WithFields(decoded.Frame, new PetalLaneResult?[PetalLayout.Sectors])), static turn => Assert.Null(turn));
    }

    [Fact]
    public void GarbageImagesNeverThrowAndReadNothing()
    {
        var image = CleanSix.Value;
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, decoded.Error.ToString());
        // another image under the decoded pose: blank, noise, tiny or empty
        var noise = new PetalLuma(image.Width, image.Height);
        new Random(7).NextBytes(noise.Data);
        foreach (var other in new[] { new PetalLuma(image.Width, image.Height), noise, new PetalLuma(3, 2), new PetalLuma(0, 0) })
        {
            Assert.All(PetalTurns.ReadTurns(other, decoded.Frame), static turn => Assert.Null(turn));
            _ = PetalTurns.ObservedTurns(other, decoded.Frame);
        }
    }

    [Fact]
    public void ArgumentsAreValidated()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalTurns.CarrierOrder(4));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalTurns.Carriers(new bool[PetalLayout.FieldCells], -1));
        Assert.Throws<ArgumentException>(() => PetalTurns.Carriers(new bool[10], 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalTurns.TurnFirstId(0, 4));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalTurns.TurnWhitening(4));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalTurns.TurnTemplate(6, 0.0, 0.0));
        Assert.Throws<ArgumentException>(() => PetalTurns.TurnData(new PetalLaneHeader(1, 2), 0, new byte[63]));
        Assert.Throws<ArgumentException>(() => PetalTurns.EncodeTurnLane(0, new byte[PetalTurns.TurnDataLength - 1]));
        Assert.False(PetalTurns.TryDecodeTurnLane(0, new byte[PetalTurns.TurnWordLength - 1], [], out _, out _));
        Assert.Null(PetalTurns.TurnPacket(new PetalLaneHeader(1, 2), 4, new byte[PetalTurns.TurnDataLength]));
        Assert.Null(PetalTurns.TurnPacket(new PetalLaneHeader(1, 2), 0, new byte[PetalTurns.TurnDataLength - 1]));
        var image = CleanSix.Value;
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.Throws<ArgumentException>(() => PetalTurns.ReadTurns(image, decoded.Frame, [false, false]));
        Assert.Equal(PetalTurns.TurnWhitening(1), PetalTurns.TurnWhitening(1));
        Assert.NotSame(PetalTurns.TurnWhitening(1), PetalTurns.TurnWhitening(1));
    }
}
