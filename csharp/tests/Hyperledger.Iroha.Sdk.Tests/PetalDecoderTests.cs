using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Ports of the Rust decoder and session tests plus robustness and platform-glue coverage.</summary>
public sealed class PetalDecoderTests
{
    /// <summary>The 3000-byte payload of the reference decoder tests.</summary>
    private static readonly byte[] SetupPayload = Enumerable.Range(0, 3000)
        .Select(static i => (byte)(unchecked((uint)i * 2_654_435_761u) >> 11))
        .ToArray();

    private static readonly PetalStreamEncoder SetupEncoder = new(SetupPayload, 2);

    /// <summary>A 768-pixel render of frame <paramref name="frame"/> of the reference test stream (2× supersampling).</summary>
    private static PetalLuma Setup(ushort frame) => PetalTestSupport.RenderLuma(SetupEncoder, frame, 768, 2);

    /// <summary>A 768-pixel render of frame <paramref name="frame"/> with the blossom of canonical corner <paramref name="corner"/> painted over.</summary>
    private static PetalLuma HiddenBlossom(ushort frame, int corner) => PetalTestSupport.HideBlossom(Setup(frame), corner);

    /// <summary>The exact canvas-to-pixel pose of the 768-pixel test renders.</summary>
    private static PetalPose RenderPose()
    {
        var canonical = PetalLayout.FinderCenters.ToArray();
        const double scale = 768.0 / 1024.0;
        var pixels = canonical.Select(static point => new PetalPoint(point.X * scale, point.Y * scale)).ToArray();
        return PetalPose.Plain(PetalHomography.FromPoints(canonical, pixels) ?? throw new InvalidOperationException("render homography"));
    }

    private static PetalDecodedFrame WithLanes(PetalDecodedFrame frame, PetalLaneResult? d, PetalLaneResult?[] fields, int? inferred) =>
        new(frame.Pose, frame.Rotation, frame.Mirrored, d, fields, inferred);

    [Fact]
    public void CleanRenderDecodesEveryLane()
    {
        var luma = Setup(5);
        var decoded = PetalDecoder.Decode(luma);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.Null(decoded.Error);
        PetalTestSupport.AssertAllLanes(decoded.Frame, SetupEncoder.LaneData(5), "clean");
        Assert.Equal((0, false), (decoded.Frame.Rotation, decoded.Frame.Mirrored));
        Assert.Equal("D0123", decoded.Frame.Lanes);
        Assert.Equal(5, decoded.Frame.LanesOk);
        Assert.Equal(BitConverter.DoubleToInt64Bits(0.0), BitConverter.DoubleToInt64Bits(decoded.Frame.Pose.Distortion));
        Assert.Null(decoded.Frame.Beacon());
        Assert.Equal(5, decoded.Frame.AtomPackets().Count);
        Assert.Equal(PetalStream.LaneFirstId(PetalLane.F0, 5), decoded.Frame.AtomPackets()[0].FirstId);
        Assert.Equal(PetalStream.LaneFirstId(PetalLane.D, 5), decoded.Frame.AtomPackets()[4].FirstId);
        Assert.Equal(decoded.Frame.Fields[2], decoded.Frame.Lane(PetalLane.F2));
    }

    [Fact]
    public void ObservedCellsMatchTheDrawnCellsOfACleanRender()
    {
        var luma = Setup(3);
        var decoded = PetalDecoder.Decode(luma);
        Assert.True(decoded.Success, decoded.Error.ToString());
        var observed = PetalDecoder.ObservedCells(luma, decoded.Frame);
        Assert.NotNull(observed);
        var drawn = SetupEncoder.Cells(3);
        Assert.Equal(drawn.Dots, observed.Dots);
        Assert.Equal(drawn.Petals, observed.Petals);
        // the petals are left unturned
        Assert.All(observed.Turns, static turn => Assert.Equal(0, turn));
    }

    [Fact]
    public void BeaconFramesExposeTheBeacon()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 8, 600, 2);
        var decoded = PetalDecoder.Decode(luma);
        Assert.True(decoded.Success, decoded.Error.ToString());
        var beacon = decoded.Frame.Beacon();
        Assert.NotNull(beacon);
        Assert.Equal(SetupEncoder.Meta, beacon.Value.Meta);
        Assert.Equal(8, beacon.Value.Header.Frame);
        Assert.Equal(4, decoded.Frame.AtomPackets().Count);
    }

    [Theory]
    [InlineData("rot90", 1, false)]
    [InlineData("rot180", 2, false)]
    [InlineData("rot270", 3, false)]
    // mirrored hypotheses enumerate corners in the opposite direction, so the
    // unrotated mirror reports quarter-turn index 1
    [InlineData("mirror", 1, true)]
    [InlineData("mirror-rot90", -1, true)]
    public void RotatedAndMirroredRendersDecodeWithTheRightOrientation(string name, int rotation, bool mirrored)
    {
        var source = Setup(7);
        var n = source.Width;
        Func<int, int, (int, int)> map = name switch
        {
            "rot90" => (x, y) => (y, n - 1 - x),
            "rot180" => (x, y) => (n - 1 - x, n - 1 - y),
            "rot270" => (x, y) => (n - 1 - y, x),
            "mirror" => (x, y) => (n - 1 - x, y),
            _ => (x, y) => (y, x),
        };
        var image = PetalTestSupport.Transform(source, map);
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, $"{name}: {decoded.Error}");
        Assert.Equal(mirrored, decoded.Frame.Mirrored);
        if (rotation >= 0)
            Assert.Equal(rotation, decoded.Frame.Rotation);
        PetalTestSupport.AssertAllLanes(decoded.Frame, SetupEncoder.LaneData(7), name);
        // without mirror hypotheses a mirrored picture cannot be read
        if (mirrored)
            Assert.False(PetalDecoder.Decode(image, PetalDecodeOptions.Default with { TryMirrored = false }).Success);
    }

    [Fact]
    public void TheFieldAloneTellsTheOrientationWhenTheRingsAreGone()
    {
        // black out the ring band: no gates, no lane D; a wrong quarter turn reads
        // permuted petals that no field codeword accepts
        var luma = Setup(9);
        var n = luma.Width;
        var scale = n / 1024.0;
        for (var y = 0; y < n; y++)
        {
            for (var x = 0; x < n; x++)
            {
                var (dx, dy) = ((x + 0.5) / scale - 512.0, (y + 0.5) / scale - 512.0);
                var r = Math.Sqrt(dx * dx + dy * dy);
                if (r is >= 392.0 and <= 490.0)
                    luma.Data[y * n + x] = 0;
            }
        }

        var rotated = PetalTestSupport.Transform(luma, (x, y) => (y, n - 1 - x));
        var decoded = PetalDecoder.Decode(rotated);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.Null(decoded.Frame.D);
        Assert.Equal(1, decoded.Frame.Rotation);
        var truth = SetupEncoder.LaneData(9);
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            Assert.Equal(truth.Fields[sector], decoded.Frame.Fields[sector]?.Data);
    }

    [Fact]
    public void AFieldLaneAloneIsEnoughToAcceptAnOrientation()
    {
        var cells = SetupEncoder.Cells(5).Clone();
        // sixteen flips in sixteen different bytes of sectors 1-3: no choice of erasures lets their
        // 22 parity bytes repair that, while sector 0 is intact
        for (var sector = 1; sector < PetalLayout.Sectors; sector++)
        {
            var order = PetalLayout.SectorCellOrder(sector);
            for (var j = 0; j < 16; j++)
            {
                var cell = order[8 * (5 * j + 2) + 3];
                cells.Petals[cell] = !cells.Petals[cell];
            }
        }

        // scramble the data dots: lane D fails its code while the gates and guards stay put
        var rng = new PetalXorshift32(5);
        var roles = PetalLayout.SlotRoleTable;
        for (var slot = 0; slot < roles.Length; slot++)
        {
            if (roles[slot].Kind == PetalSlotKind.Data)
                cells.Dots[slot] = rng.NextUInt32() % 2 == 0;
        }

        var luma = PetalRenderer.Render(cells, new PetalRenderOptions { Size = 768, Supersample = 2 }).ToLuma();
        var result = PetalDecoder.Decode(luma);
        Assert.True(result.Success, result.Error.ToString());
        Assert.Equal("0", result.Frame.Lanes);
        Assert.Equal(SetupEncoder.LaneData(5).Fields[0], result.Frame.Fields[0]?.Data);
        Assert.Equal((0, false), (result.Frame.Rotation, result.Frame.Mirrored));
    }

    [Fact]
    public void CorrectedCountsRewrittenBytesNotJustErasures()
    {
        var data = Enumerable.Range(0, PetalLanes.FieldDataLength).Select(static b => (byte)b).ToArray();
        var lane = PetalLane.F1;
        var word = PetalLanes.EncodeLane(lane, data);
        foreach (var position in new[] { 2, 11, 30, 77 })
            word[position] ^= 0x5A;
        var confidence = Enumerable.Repeat(1.0, word.Length).ToArray();
        var result = PetalDecoder.DecodeWithErasures(lane, word, confidence);
        Assert.NotNull(result);
        Assert.Equal(data, result.Data);
        Assert.Equal(0, result.Erasures);
        Assert.Equal(4, result.Corrected);
        // with the damaged bytes flagged as least confident, they become erasures
        var flagged = Enumerable.Repeat(1.0, word.Length).ToArray();
        foreach (var position in new[] { 2, 11, 30, 77 })
            flagged[position] = 0.0;
        result = PetalDecoder.DecodeWithErasures(lane, word, flagged);
        Assert.NotNull(result);
        Assert.Equal(data, result.Data);
        Assert.True(result.Corrected >= 4);
    }

    [Fact]
    public void RandomWordsAreAlmostNeverAccepted()
    {
        // Reed–Solomon with erasures can accept a word that is not a transmission. Lane D has only
        // 11 parity bytes, so its schedule stops at five erasures; a field lane has 22 and stops at
        // eleven. The counts are exact so that every port, fed the same xorshift32 words and
        // byte-valued confidences (many ties, so the ranking must be stable), reproduces the
        // decoder bit for bit.
        var rng = new PetalXorshift32(0x5EED);
        const int trials = 40_000;
        foreach (var (lane, expected) in new[] { (PetalLane.D, 3), (PetalLane.F0, 0) })
        {
            var length = PetalLanes.WordLength(lane);
            var word = new byte[length];
            var confidence = new double[length];
            var accepted = 0;
            for (var trial = 0; trial < trials; trial++)
            {
                for (var i = 0; i < length; i++)
                    word[i] = rng.NextByte();
                for (var i = 0; i < length; i++)
                    confidence[i] = rng.NextByte();
                if (PetalDecoder.DecodeWithErasures(lane, word, confidence) is not null)
                    accepted++;
            }

            Assert.True(expected == accepted, $"lane {lane}: {accepted} of {trials} random words accepted, expected {expected}");
        }
    }

    [Fact]
    public void ErasuresStopAtHalfTheParity()
    {
        // damaged bytes: `flagged` of them marked least confident, two more hidden: beyond half the
        // parity the decoder must refuse instead of risking a wrong codeword
        foreach (var (lane, flagged) in new[] { (PetalLane.D, 7), (PetalLane.F2, 16) })
        {
            var data = Enumerable.Range(0, PetalLanes.DataLength(lane)).Select(static b => (byte)b).ToArray();
            var word = PetalLanes.EncodeLane(lane, data);
            var damaged = (byte[])word.Clone();
            var confidence = Enumerable.Repeat(1.0, word.Length).ToArray();
            for (var position = 0; position < flagged; position++)
            {
                damaged[position] ^= 0xA5;
                confidence[position] = 0.0;
            }

            damaged[20] ^= 0x3C;
            damaged[21] ^= 0x3C;
            Assert.True(PetalDecoder.DecodeWithErasures(lane, damaged, confidence) is null, $"lane {lane}");

            // half the parity flagged plus one hidden error stays comfortably repairable
            damaged = (byte[])word.Clone();
            confidence = Enumerable.Repeat(1.0, damaged.Length).ToArray();
            for (var position = 0; position < PetalLanes.ParityLength(lane) / 2; position++)
            {
                damaged[position] ^= 0xA5;
                confidence[position] = 0.0;
            }

            damaged[24] ^= 0x3C;
            var result = PetalDecoder.DecodeWithErasures(lane, damaged, confidence);
            Assert.True(result is not null, $"lane {lane}");
            Assert.Equal(data, result.Data);
            Assert.True(result.Erasures <= PetalLanes.ParityLength(lane) / 2, $"lane {lane}");
        }
    }

    [Fact]
    public void EqualConfidencesAreErasedInPositionOrder()
    {
        // Ties must be broken by position or ports disagree about which bytes are erased. Three
        // damaged bytes at the front plus four hidden ones fit lane D only if exactly the first
        // three positions are erased (3 erasures + 4 errors = all 11 parity bytes).
        var data = Enumerable.Range(0, PetalLanes.DDataLength).Select(static b => (byte)b).ToArray();
        var word = PetalLanes.EncodeLane(PetalLane.D, data);
        foreach (var position in Enumerable.Range(0, 3).Concat(Enumerable.Range(20, 4)))
            word[position] ^= 0x5A;
        var confidence = Enumerable.Repeat(1.0, word.Length).ToArray();
        var result = PetalDecoder.DecodeWithErasures(PetalLane.D, word, confidence);
        Assert.NotNull(result);
        Assert.Equal(data, result.Data);
        Assert.Equal(3, result.Erasures);
        Assert.Equal(7, result.Corrected);
    }

    [Theory]
    [InlineData(PetalLane.D, 6)]
    [InlineData(PetalLane.F0, 12)]
    [InlineData(PetalLane.F3, 12)]
    public void FirstErasureStepFollowsByteOrderWhenConfidencesTie(PetalLane lane, int damagedBytes)
    {
        // One more damaged byte than the code corrects unaided, all at the front of the word and
        // all equally confident. Only the first erasure step can repair it, and only if the tie is
        // resolved in byte order: that step erases the first bytes, damaged ones. A ranking that
        // reorders ties erases intact bytes instead and every later step is out of budget.
        var nsym = PetalLanes.ParityLength(lane);
        Assert.True(2 * damagedBytes > nsym && 2 * damagedBytes - nsym / 8 <= nsym);
        var data = Enumerable.Range(0, PetalLanes.DataLength(lane)).Select(static b => (byte)(b * 7 + 1)).ToArray();
        var word = PetalLanes.EncodeLane(lane, data);
        for (var position = 0; position < damagedBytes; position++)
            word[position] ^= 0x5A;
        var confidence = Enumerable.Repeat(1.0, word.Length).ToArray();
        var result = PetalDecoder.DecodeWithErasures(lane, word, confidence);
        Assert.NotNull(result);
        Assert.Equal(data, result.Data);
        Assert.Equal(nsym / 8, result.Erasures);
        Assert.Equal(damagedBytes, result.Corrected);
    }

    [Fact]
    public void ConfidenceTiesKeepTheirByteOrderWhenRanking()
    {
        // byte-valued confidences tie everywhere, so the ranking must be a stable sort like
        // Rust's sort_by: Array.Sort would reorder the ties and change which bytes get erased
        var rng = new PetalXorshift32(0x71E5);
        foreach (var length in new[] { PetalLanes.DWordLength, PetalLanes.FieldWordLength })
        {
            for (var trial = 0; trial < 200; trial++)
            {
                var keys = new double[length];
                for (var i = 0; i < length; i++)
                    keys[i] = rng.NextByte() % 6;
                var order = Enumerable.Range(0, length).ToArray();
                PetalMath.StableSortByKey(order, keys);
                // LINQ's OrderBy is documented as stable
                Assert.Equal(Enumerable.Range(0, length).OrderBy(i => keys[i]).ToArray(), order);
            }
        }
    }

    [Fact]
    public void TwoMeansSplitsTwoClustersAndSurvivesOne()
    {
        var values = Enumerable.Repeat(0.05, 40).Concat(Enumerable.Repeat(0.9, 60)).ToArray();
        var t = PetalDecoder.TwoMeans(values);
        Assert.True(t > 0.3 && t < 0.6, $"{t}");
        var flat = Enumerable.Repeat(0.4, 10).ToArray();
        Assert.True(Math.Abs(PetalDecoder.TwoMeans(flat) - 0.4) < 1e-12);
        // the threshold starts at the mean, not the median, and splits strictly above it
        Assert.Equal(0.5 * (0.0 + 1.0), PetalDecoder.TwoMeans([0.0, 1.0]));
        Assert.Equal(0.5 * (0.0 + (1.0 + 1.0 + 1.0) / 3.0), PetalDecoder.TwoMeans([0.0, 1.0, 1.0, 1.0]));
    }

    [Fact]
    public void TotalOrderSortMatchesTheReferenceOrdering()
    {
        var rng = new PetalLcg(2026);
        var values = new List<double>
        {
            0.0, -0.0, 1.0, -1.0, double.Epsilon, -double.Epsilon, double.MaxValue, double.MinValue,
            double.PositiveInfinity, double.NegativeInfinity,
            BitConverter.Int64BitsToDouble(0x7FF8_0000_0000_0000), // positive quiet NaN
            BitConverter.Int64BitsToDouble(unchecked((long)0xFFF8_0000_0000_0000UL)), // negative quiet NaN
            BitConverter.Int64BitsToDouble(0x7FF0_0000_0000_0001), // positive signalling NaN
        };
        for (var i = 0; i < 200; i++)
            values.Add((rng.NextDouble() - 0.5) * Math.Pow(10.0, rng.Below(40) - 20));
        for (var i = 0; i < 20; i++)
            values.Add(values[rng.Below(values.Count)]); // duplicates
        var shuffled = values.OrderBy(_ => rng.Next()).ToArray();
        var expected = (double[])shuffled.Clone();
        Array.Sort(expected, static (a, b) => PetalMath.TotalCompare(a, b));
        var actual = (double[])shuffled.Clone();
        PetalMath.SortTotal(actual);
        Assert.Equal(
            expected.Select(BitConverter.DoubleToInt64Bits),
            actual.Select(BitConverter.DoubleToInt64Bits));
        PetalMath.SortTotal([]);
        PetalMath.SortTotal(new[] { 3.0 });
    }

    [Fact]
    public void BlankFramesReportNoFinders()
    {
        var result = PetalDecoder.Decode(new PetalLuma(320, 240));
        Assert.False(result.Success);
        Assert.Equal(PetalDecodeError.NoFinders, result.Error);
    }

    [Fact]
    public void UnusableSizesAreRejectedWithoutWork()
    {
        Assert.Equal(PetalDecodeError.UnsupportedImage, PetalDecoder.Decode(new PetalLuma(1, 1)).Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, PetalDecoder.Decode(new PetalLuma(47, 400)).Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, PetalDecoder.Decode(new PetalLuma(400, 47)).Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, PetalDecoder.Decode(new PetalLuma(0, 0)).Error);
        var smallBudget = PetalDecodeOptions.Default with { MaxPixels = 1_000 };
        Assert.Equal(PetalDecodeError.UnsupportedImage, PetalDecoder.Decode(new PetalLuma(100, 100), smallBudget).Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, PetalDecoder.Decode(new PetalLuma(4_000, 3_001)).Error);
        var identity = PetalPose.Plain(PetalHomography.Identity);
        Assert.Null(PetalDecoder.DecodeAt(new PetalLuma(8, 8), identity));
        var decoded = PetalDecoder.Decode(Setup(5));
        Assert.True(decoded.Success, decoded.Error.ToString());
        // an image the decoder would refuse never reaches the sampler
        Assert.Null(PetalDecoder.ObservedCells(new PetalLuma(20, 20), decoded.Frame));
        Assert.Null(PetalDecoder.FieldEvidence(new PetalLuma(20, 20), decoded.Frame));
        Assert.Null(PetalDecoder.Track(new PetalLuma(47, 47), decoded.Frame));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalDecodeOptions.Default with { MaxPixels = -1 });
    }

    [Fact]
    public void GarbageImagesNeverThrowOrDecode()
    {
        var rng = new PetalXorshift32(99);
        foreach (var (w, h) in new[] { (64, 48), (257, 129), (320, 240), (480, 480) })
        {
            for (var style = 0; style < 4; style++)
            {
                var data = new byte[w * h];
                for (var i = 0; i < data.Length; i++)
                {
                    data[i] = style switch
                    {
                        0 => rng.NextByte(), // white noise
                        1 => (byte)(i % w * 255 / w), // gradient
                        2 => (i / w / 8 + i % w / 8) % 2 == 0 ? (byte)230 : (byte)20, // checkerboard
                        _ => rng.NextUInt32() % 50 == 0 ? (byte)255 : (byte)0, // sparse specks
                    };
                }

                Assert.False(PetalDecoder.Decode(new PetalLuma(w, h, data)).Success, $"{w}x{h} style {style}");
            }
        }
    }

    [Fact]
    public void ExtremeAndSyntheticImagesReturnErrors()
    {
        var rng = new PetalXorshift32(7);
        var cases = new List<PetalLuma>
        {
            new(48, 48),
            new(48, 4_000),
            new(4_000, 48),
            new(64, 64, Enumerable.Repeat((byte)255, 64 * 64).ToArray()),
            new(1_000, 750, Enumerable.Range(0, 750_000).Select(_ => rng.NextByte()).ToArray()),
        };
        // a single bright pixel, bright lines and isolated round blobs without a code
        var specks = new byte[200 * 200];
        specks[100 * 200 + 100] = 255;
        cases.Add(new PetalLuma(200, 200, specks));
        var lines = new byte[300 * 300];
        for (var i = 0; i < 300; i++)
            lines[i * 300 + 150] = lines[150 * 300 + i] = 255;
        cases.Add(new PetalLuma(300, 300, lines));
        var blobs = new byte[400 * 400];
        foreach (var (cx, cy) in new[] { (60, 60), (340, 60), (340, 340), (60, 340) })
        {
            for (var y = cy - 25; y <= cy + 25; y++)
            {
                for (var x = cx - 25; x <= cx + 25; x++)
                {
                    if ((x - cx) * (x - cx) + (y - cy) * (y - cy) <= 625)
                        blobs[y * 400 + x] = 250;
                }
            }
        }

        cases.Add(new PetalLuma(400, 400, blobs));
        foreach (var image in cases)
        {
            var result = PetalDecoder.Decode(image);
            Assert.False(result.Success, $"{image.Width}x{image.Height}");
            Assert.NotNull(result.Error);
        }
    }

    [Fact]
    public void RandomBlobScenesNeverThrow()
    {
        // Scenes with several random bright ellipses (some finder-sized) on noise:
        // exercises the locator, quad selection and pose fitting on degenerate layouts.
        var rng = new PetalXorshift32(2024);
        for (var scene = 0; scene < 60; scene++)
        {
            var w = 160 + (int)(rng.NextUInt32() % 400);
            var h = 120 + (int)(rng.NextUInt32() % 300);
            var data = new byte[w * h];
            for (var i = 0; i < data.Length; i++)
                data[i] = (byte)(rng.NextUInt32() % 40);
            var blobs = 3 + rng.NextUInt32() % 8;
            for (var blob = 0u; blob < blobs; blob++)
            {
                double cx = rng.NextUInt32() % (uint)w;
                double cy = rng.NextUInt32() % (uint)h;
                var rx = 6.0 + rng.NextUInt32() % 40;
                var ry = 6.0 + rng.NextUInt32() % 40;
                // the bounding box holds every pixel that can satisfy the ellipse test
                for (var y = Math.Max(0, (int)Math.Floor(cy - ry)); y <= Math.Min(h - 1, (int)Math.Ceiling(cy + ry)); y++)
                {
                    for (var x = Math.Max(0, (int)Math.Floor(cx - rx)); x <= Math.Min(w - 1, (int)Math.Ceiling(cx + rx)); x++)
                    {
                        var (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
                        if (dx * dx + dy * dy <= 1.0)
                            data[y * w + x] = 230;
                    }
                }
            }

            // a lucky layout may locate finders but cannot yield lanes
            var result = PetalDecoder.Decode(new PetalLuma(w, h, data));
            if (result.Success)
                Assert.True(result.Frame.LanesOk == 0, $"scene {scene} produced lane data from blobs");
        }
    }

    [Fact]
    public void DecodeAtSurvivesHostilePoses()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 3, 300, 1);
        var hostile = new[]
        {
            PetalPose.Plain(new PetalHomography(new double[9])),
            PetalPose.Plain(new PetalHomography(double.NaN, 0, 0, 0, double.NaN, 0, 0, 0, 1)),
            PetalPose.Plain(new PetalHomography(double.PositiveInfinity, 0, 0, 0, 1, 0, 0, 0, 1)),
            PetalPose.Plain(new PetalHomography(1, 0, 0, 0, 1, 0, 1, 1, -600)),
            PetalPose.Plain(new PetalHomography(1e300, 1e300, 1e300, -1e300, 1e300, 1, 0, 0, 1e-300)),
            new PetalPose(PetalHomography.Identity, double.NaN, new PetalPoint(150, 150), 1.0),
            new PetalPose(PetalHomography.Identity, -1e300, new PetalPoint(150, 150), 0.0),
        };
        foreach (var pose in hostile)
        {
            var frame = PetalDecoder.DecodeAt(luma, pose);
            if (frame is not null)
                Assert.Equal(0, frame.LanesOk);
        }
    }

    [Fact]
    public void DecodeAtWithTheTruePoseReadsEveryLane()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 6, 512, 2);
        var pose = PetalPose.Plain(new PetalHomography(0.5, 0, 0, 0, 0.5, 0, 0, 0, 1));
        var frame = PetalDecoder.DecodeAt(luma, pose);
        Assert.NotNull(frame);
        PetalTestSupport.AssertAllLanes(frame, SetupEncoder.LaneData(6), "true pose");
        Assert.Equal(pose, frame.Pose);
    }

    [Fact]
    public void AHiddenBlossomIsInferredAndEveryLaneStillReads()
    {
        for (var corner = 0; corner < 4; corner++)
        {
            var decoded = PetalDecoder.Decode(HiddenBlossom(2, corner));
            Assert.True(decoded.Success, $"corner {corner}: {decoded.Error}");
            Assert.True(corner == decoded.Frame.InferredCorner, $"corner {corner}: inferred {decoded.Frame.InferredCorner}");
            Assert.Equal((0, false), (decoded.Frame.Rotation, decoded.Frame.Mirrored));
            PetalTestSupport.AssertAllLanes(decoded.Frame, SetupEncoder.LaneData(2), $"corner {corner}");
        }
    }

    [Fact]
    public void TheInferredCornerIsReportedInCodeCoordinatesWhenMirrored()
    {
        // hide the top-right blossom of the code, then mirror the picture: the hidden
        // blossom appears top-left in the image but is still corner 1 of the code
        var decoded = PetalDecoder.Decode(PetalTestSupport.Mirror(HiddenBlossom(3, 1)));
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.True(decoded.Frame.Mirrored);
        Assert.Equal(1, decoded.Frame.InferredCorner);
        Assert.Equal(SetupEncoder.LaneData(3).D, decoded.Frame.D?.Data);
    }

    [Fact]
    public void ALargeHiddenRegionNeverReadsWrongData()
    {
        // the whole bottom-right quarter is gone: rings and petals with it
        var luma = Setup(2);
        var n = luma.Width;
        for (var y = n * 3 / 4; y < n; y++)
        {
            for (var x = n * 3 / 4; x < n; x++)
                luma.Data[y * n + x] = 0;
        }

        var truth = SetupEncoder.LaneData(2);
        var result = PetalDecoder.Decode(luma);
        if (!result.Success)
            return;
        foreach (var lane in PetalLanes.DecodeOrder)
        {
            if (result.Frame.Lane(lane) is { } decoded)
                Assert.Equal(truth.Lane(lane), decoded.Data);
        }
    }

    [Fact]
    public void LensDistortionIsFoundWhenThePlainPoseMissesThePetals()
    {
        // a small, strongly distorted picture: 384 px of code with the corner blossoms far out
        var luma = Setup(6);
        var small = new PetalLuma(640, 480);
        // downscale 768 → 384 by averaging 2×2 blocks, then frame it in 640×480
        for (var y = 0; y < 384; y++)
        {
            for (var x = 0; x < 384; x++)
            {
                var sum = luma.Data[2 * y * 768 + 2 * x] + luma.Data[2 * y * 768 + 2 * x + 1]
                    + luma.Data[(2 * y + 1) * 768 + 2 * x] + luma.Data[(2 * y + 1) * 768 + 2 * x + 1];
                small.Data[(y + 48) * 640 + x + 128] = (byte)((sum + 2) / 4);
            }
        }

        var distorted = PetalTestSupport.Barrel(small, -0.12);
        // the plain pose misses petals at the edge of the field
        var quad = PetalLocator.Locate(distorted);
        Assert.NotNull(quad);
        var corners = quad.Select(static f => new PetalPoint(f.X, f.Y)).ToArray();
        var pose = PetalDecoder.FitPose(distorted, corners, 0.0);
        Assert.NotNull(pose);
        var plain = PetalDecoder.DecodeAt(distorted, pose.Value);
        Assert.NotNull(plain);
        Assert.True(plain.Fields.Any(static f => f is null), plain.Lanes);
        var decoded = PetalDecoder.Decode(distorted);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.True(decoded.Frame.Pose.Distortion < 0.0, $"{decoded.Frame.Pose.Distortion}");
        PetalTestSupport.AssertAllLanes(decoded.Frame, SetupEncoder.LaneData(6), "barrel");
    }

    [Fact]
    public void TrackingFollowsASmallMovementAndGivesUpOnAJump()
    {
        var luma = PetalTestSupport.Padded(Setup(6), 100);
        var first = PetalDecoder.Decode(luma);
        Assert.True(first.Success, first.Error.ToString());
        var followed = PetalDecoder.Track(PetalTestSupport.Shifted(luma, 9, -6), first.Frame);
        Assert.NotNull(followed);
        PetalTestSupport.AssertAllLanes(followed, SetupEncoder.LaneData(6), "tracked");
        Assert.Null(followed.InferredCorner);
        // more than a finder diameter: tracking refuses, a full decode is needed
        var jumped = PetalTestSupport.Shifted(luma, 95, 0);
        Assert.Null(PetalDecoder.Track(jumped, first.Frame));
        Assert.True(PetalDecoder.Decode(jumped).Success);
    }

    [Fact]
    public void BrokenPosesAreRefusedWithoutThrowing()
    {
        var luma = Setup(4);
        var decoded = PetalDecoder.Decode(luma);
        Assert.True(decoded.Success, decoded.Error.ToString());
        var previous = decoded.Frame;
        Assert.NotNull(PetalDecoder.Track(luma, previous));
        // a good pose that names a corner that does not exist
        foreach (var corner in new[] { 4, 255, -1 })
            Assert.Null(PetalDecoder.Track(luma, WithLanes(previous, previous.D, previous.Fields.ToArray(), corner)));
        var nonFinite = new[]
        {
            new PetalHomography(Enumerable.Repeat(double.NaN, 9).ToArray()),
            new PetalHomography(double.PositiveInfinity, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
        };
        foreach (var broken in nonFinite)
            Assert.Null(PetalDecoder.DecodeAt(luma, PetalPose.Plain(broken)));
        // the last one makes every finder far larger than the image
        var huge = new PetalHomography(50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 1.0);
        foreach (var broken in nonFinite.Append(huge))
        {
            foreach (var inferred in new int?[] { null, 2 })
            {
                var frame = new PetalDecodedFrame(PetalPose.Plain(broken), previous.Rotation, previous.Mirrored, previous.D, previous.Fields.ToArray(), inferred);
                Assert.Null(PetalDecoder.Track(luma, frame));
            }
        }
    }

    [Fact]
    public void TrackingSurvivesABlossomThatDisappears()
    {
        var first = PetalDecoder.Decode(Setup(4));
        Assert.True(first.Success, first.Error.ToString());
        // the same code, slightly moved, now with the bottom-left blossom covered
        var moved = PetalTestSupport.Shifted(HiddenBlossom(4, 3), -5, 4);
        var followed = PetalDecoder.Track(moved, first.Frame);
        Assert.NotNull(followed);
        Assert.Equal(3, followed.InferredCorner);
        Assert.Equal(SetupEncoder.LaneData(4).D, followed.D?.Data);
    }

    [Fact]
    public void ABlossomThatReappearsIsSeenAgain()
    {
        var covered = HiddenBlossom(4, 3);
        var first = PetalDecoder.Decode(covered);
        Assert.True(first.Success, first.Error.ToString());
        Assert.Equal(3, first.Frame.InferredCorner);
        // the thumb moves away and the hand moves a little
        var followed = PetalDecoder.Track(PetalTestSupport.Shifted(Setup(4), 4, -3), first.Frame);
        Assert.NotNull(followed);
        Assert.Null(followed.InferredCorner);
        Assert.Equal(SetupEncoder.LaneData(4).D, followed.D?.Data);
        // still covered: still inferred
        followed = PetalDecoder.Track(PetalTestSupport.Shifted(covered, 4, -3), first.Frame);
        Assert.NotNull(followed);
        Assert.Equal(3, followed.InferredCorner);
    }

    [Fact]
    public void CanonicalCornersFollowTheHypothesisPermutation()
    {
        // the hypothesis (rotation, mirrored) maps canonical corner i to quad corner
        // (i + rotation) % 4, or (rotation + 4 - i) % 4 when mirrored; CanonicalCorner inverts it
        for (var rotation = 0; rotation < 4; rotation++)
        {
            for (var i = 0; i < 4; i++)
            {
                Assert.Equal(i, PetalDecoder.CanonicalCorner((i + rotation) % 4, rotation, false));
                Assert.Equal(i, PetalDecoder.CanonicalCorner((rotation + 4 - i) % 4, rotation, true));
            }
        }

        var quad = Enumerable.Range(0, 4).Select(static i => new PetalFinder(10.0 * i, 0.0, 1.0)).ToArray();
        var hypotheses = PetalDecoder.Hypotheses(quad, true);
        Assert.Equal(8, hypotheses.Count);
        Assert.Equal(4, PetalDecoder.Hypotheses(quad, false).Count);
        foreach (var (rotation, mirrored, destination) in hypotheses)
        {
            for (var i = 0; i < 4; i++)
            {
                var q = mirrored ? (rotation + 4 - i) % 4 : (i + rotation) % 4;
                Assert.Equal(10.0 * q, destination[i].X);
            }
        }
    }

    /// <summary>A canvas-sized image with finder (lit) and reference-canvas (dark) levels painted where they are sampled.</summary>
    private static PetalLuma LevelCard((byte Lit, byte Dark)[] levels)
    {
        var pixels = new byte[1024 * 1024];
        void Paint(int cx, int cy, int radius, byte value)
        {
            for (var y = cy - radius; y <= cy + radius; y++)
                for (var x = cx - radius; x <= cx + radius; x++)
                    pixels[y * 1024 + x] = value;
        }

        (int X, int Y)[] centers = [(72, 72), (952, 72), (952, 952), (72, 952)];
        for (var corner = 0; corner < 4; corner++)
        {
            var (cx, cy) = centers[corner];
            var (sx, sy) = (cx < 512 ? 1 : -1, cy < 512 ? 1 : -1);
            Paint(cx, cy, 30, levels[corner].Lit);
            Paint(cx + sx * 100, cy, 12, levels[corner].Dark);
            Paint(cx, cy + sy * 100, 12, levels[corner].Dark);
        }

        return new PetalLuma(1024, 1024, pixels);
    }

    [Fact]
    public void AnInferredCornerNeedsContrastToo()
    {
        var pose = PetalPose.Plain(PetalHomography.Identity);
        // even light: the hidden corner (3) gets levels between the others'
        var even = PetalDecoder.ReferenceLevels(LevelCard([(230, 30), (220, 25), (210, 20), (0, 0)]), pose, 3);
        Assert.NotNull(even);
        Assert.True(even.Value.Lit(3) - even.Value.Dark(3) >= 12.0);
        // the hidden corner's neighbours disagree (one dim, one veiled): the estimates cross
        Assert.Null(PetalDecoder.ReferenceLevels(LevelCard([(60, 45), (250, 20), (200, 185), (0, 0)]), pose, 3));
        // with every corner seen, the same light is fine
        Assert.NotNull(PetalDecoder.ReferenceLevels(LevelCard([(60, 45), (250, 20), (200, 185), (240, 20)]), pose, null));
    }

    [Fact]
    public void AnInferredCornerTakesItsLevelsFromTheOtherThree()
    {
        var luma = Setup(5);
        var pose = RenderPose();
        var seen = PetalDecoder.ReferenceLevels(luma, pose, null);
        Assert.NotNull(seen);
        for (var corner = 0; corner < 4; corner++)
        {
            var hidden = PetalTestSupport.HideBlossom(luma, corner);
            // the covered blossom has no contrast of its own: refused unless it is inferred
            Assert.Null(PetalDecoder.ReferenceLevels(hidden, pose, null));
            var inferred = PetalDecoder.ReferenceLevels(hidden, pose, corner);
            Assert.NotNull(inferred);
            var (n1, opposite, n2) = ((corner + 1) % 4, (corner + 2) % 4, (corner + 3) % 4);
            var expected = Math.Clamp(
                seen.Value.Lit(n1) + seen.Value.Lit(n2) - seen.Value.Lit(opposite),
                Math.Min(Math.Min(seen.Value.Lit(n1), seen.Value.Lit(opposite)), seen.Value.Lit(n2)),
                Math.Max(Math.Max(seen.Value.Lit(n1), seen.Value.Lit(opposite)), seen.Value.Lit(n2)));
            Assert.Equal(expected, inferred.Value.Lit(corner));
            Assert.Equal(seen.Value.Dark(opposite), inferred.Value.Dark(opposite));
        }
    }

    [Fact]
    public void SimulatedCamerasDecodeWithoutWrongData()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(900, 8), 2);
        var configs = new[]
        {
            PetalCaptureConfig.Modern() with { Width = 640, Height = 480, RotationDeg = 33.0, TiltXDeg = 8.0, Seed = 4 },
            PetalCaptureConfig.Legacy() with { Width = 800, Height = 600, RotationDeg = 250.0, Seed = 5 },
            PetalCaptureConfig.Modern() with { Width = 640, Height = 480, RotationDeg = 140.0, BlurSigma = 1.6, Noise = 5.0, Seed = 6 },
        };
        var decodedLanes = 0;
        foreach (var (config, index) in configs.Select(static (c, i) => (c, i)))
        {
            var frame = (ushort)(11 + index);
            var source = PetalRenderer.Render(encoder.Cells(frame), new PetalRenderOptions { Size = 768, Supersample = 2 });
            var fitted = PetalCaptureSimulator.FitToFrame(config, 4.0);
            var image = PetalCaptureSimulator.Capture(source, fitted);
            var result = PetalDecoder.Decode(image);
            Assert.True(result.Success, $"config {index}: {result.Error}");
            var truth = encoder.LaneData(frame);
            foreach (var lane in PetalLanes.DecodeOrder)
            {
                if (result.Frame.Lane(lane) is { } decoded)
                    Assert.Equal(truth.Lane(lane), decoded.Data);
            }

            decodedLanes += result.Frame.LanesOk;
            // the decoder's pose maps the canvas centre near the simulated one
            var expected = PetalCaptureSimulator.CameraHomography(fitted).Apply(512, 512);
            var actual = result.Frame.Pose.Apply(512, 512);
            if (config.LensK1 == 0.0)
                Assert.True(Math.Abs(expected.X - actual.X) < 3.0 && Math.Abs(expected.Y - actual.Y) < 3.0, $"config {index}");
        }

        Assert.True(decodedLanes >= 10, $"{decodedLanes} lanes");
    }

    /// <summary>The simulated 640×480 camera of the reference session tests at <paramref name="rotationDeg"/>.</summary>
    private static PetalCaptureConfig Camera(double rotationDeg) => PetalCaptureSimulator.FitToFrame(
        PetalCaptureConfig.Modern() with { Width = 640, Height = 480, RotationDeg = rotationDeg },
        4.0);

    /// <summary>The 512-pixel render a simulated camera films in the reference session tests.</summary>
    private static PetalRgbImage Source(PetalStreamEncoder encoder, ushort frame) =>
        PetalRenderer.Render(encoder.Cells(frame), new PetalRenderOptions { Size = 512, Supersample = 2 });

    [Fact]
    public void ASteadyCameraIsTrackedAfterTheFirstFrame()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(3000, 3), 2);
        var config = Camera(8.0);
        PetalLuma Shot(int frame) => PetalCaptureSimulator.Capture(Source(encoder, (ushort)frame), config);
        var session = new PetalScanSession();
        for (var frame = 0; frame < 6; frame++)
        {
            var outcome = session.Push(Shot(frame), frame * 100L);
            Assert.True(outcome.Error is null, $"frame {frame}: {outcome.Error}");
        }

        Assert.Equal(6u, session.Stats.Readable);
        // every frame after the first follows the pose
        Assert.Equal(5u, session.Stats.Tracked);
        // a pause longer than the tracking window forces a full search again
        session.Push(Shot(6), 5 * 100L + PetalScanSession.TrackWindowMilliseconds + 1);
        Assert.Equal(5u, session.Stats.Tracked);
        Assert.Equal(7u, session.Stats.Readable);
        // so does a reset, which forgets the pose
        session.Reset();
        session.Push(Shot(7), 5 * 100L + PetalScanSession.TrackWindowMilliseconds + 100);
        Assert.Equal(5u, session.Stats.Tracked);
        Assert.Equal(8u, session.Stats.Readable);
    }

    [Fact]
    public void ASessionReceivesAPayloadFromSimulatedCaptures()
    {
        var data = PetalTestSupport.Payload(5000, 3);
        var encoder = new PetalStreamEncoder(data, 2);
        var config = Camera(20.0);
        var session = new PetalScanSession();
        PetalCompletedPayload? done = null;
        for (var frame = 0; frame < 40; frame++)
        {
            var outcome = session.Push(PetalCaptureSimulator.Capture(Source(encoder, (ushort)frame), config), frame * 100L);
            if (outcome.Completed is not null)
            {
                done = outcome.Completed;
                break;
            }
        }

        Assert.NotNull(done);
        Assert.Equal(data, done.ToArray());
        Assert.Equal(2, done.Meta.Kind);
        var stats = session.Stats;
        Assert.True(stats.Readable > 0 && stats.LaneD > 0 && stats.FieldLanes > 0);
        Assert.True(stats.TurnLanes > 0, $"{stats}");
        // 5,000 bytes need 313 atoms: the field lanes and lane D alone carry them in 15 or 16
        // frames, the turn lanes on top in 9 or 10
        Assert.True(stats.Frames <= 11, $"{stats}");
        Assert.True(session.Progress.Complete);
    }

    [Fact]
    public void ATurnLaneIsReadOncePerShownFrame()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(3000, 3), 2);
        var config = Camera(12.0);
        var session = new PetalScanSession();
        var shown = Source(encoder, 5);
        var first = session.Push(PetalCaptureSimulator.Capture(shown, config with { Seed = 1 }), 0);
        Assert.Equal("D0123", first.Lanes);
        Assert.Equal("0123", first.Turned);
        var again = session.Push(PetalCaptureSimulator.Capture(shown, config with { Seed = 2 }), 33);
        Assert.Equal(("D0123", string.Empty), (again.Lanes, again.Turned));
        Assert.Equal(4u, session.Stats.TurnLanes);
        // the next shown frame's turn lanes are new
        var next = session.Push(PetalCaptureSimulator.Capture(Source(encoder, 6), config with { Seed = 3 }), 83);
        Assert.Equal("0123", next.Turned);
        Assert.Equal((8u, 8u), (session.Stats.TurnTries, session.Stats.TurnLanes));
        Assert.Equal(8, session.TriedLanes.Count);
        session.Reset();
        Assert.Empty(session.TriedLanes);
    }

    [Fact]
    public void TriesThatReadNothingBackOff()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(3000, 3), 2);
        // unturned petals: every field lane reads, no turn lane ever does
        PetalRgbImage Unturned(ushort frame)
        {
            var cells = encoder.Cells(frame);
            Array.Clear(cells.Turns);
            return PetalRenderer.Render(cells, new PetalRenderOptions { Size = 512, Supersample = 2 });
        }

        var config = Camera(12.0);
        var session = new PetalScanSession();
        var triedAt = new List<int>();
        for (ushort frame = 1; frame <= 30; frame++)
        {
            var before = session.Stats.TurnTries;
            var image = PetalCaptureSimulator.Capture(Unturned(frame), config with { Seed = frame });
            var outcome = session.Push(image, frame * 83L);
            Assert.True(outcome.Lanes.EndsWith("0123", StringComparison.Ordinal), $"frame {frame}: {outcome.Lanes}");
            Assert.Equal(string.Empty, outcome.Turned);
            if (session.Stats.TurnTries > before)
                triedAt.Add(frame);
        }

        // after the n-th try that reads nothing, the next min(n, 8) captures that would try are
        // passed over
        Assert.Equal([1, 3, 6, 10, 15, 21, 28], triedAt);
        // a try that reads a lane ends the back-off
        session.TurnWait = 0;
        var turned = PetalCaptureSimulator.Capture(Source(encoder, 31), config with { Seed = 31 });
        Assert.Equal("0123", session.Push(turned, 31 * 83L).Turned);
        Assert.Equal(0, session.TurnMisses);
        turned = PetalCaptureSimulator.Capture(Source(encoder, 32), config with { Seed = 32 });
        Assert.Equal("0123", session.Push(turned, 32 * 83L).Turned);
    }

    [Fact]
    public void ASessionCountsAThumbThatArrivesAndLeaves()
    {
        var session = new PetalScanSession();
        PetalLuma Frame(int frame, bool covered)
        {
            var render = PetalTestSupport.RenderLuma(SetupEncoder, (ushort)frame, 600, 2);
            return covered ? PetalTestSupport.HideBlossom(render, 1) : render;
        }

        session.Push(Frame(0, false), 0);
        // the thumb arrives: tracked, with the top-right corner inferred
        Assert.Equal("D0123", session.Push(Frame(1, true), 125).Lanes);
        Assert.Equal((1u, 1u), (session.Stats.Tracked, session.Stats.Inferred));
        Assert.Equal("D0123", session.Push(Frame(2, true), 250).Lanes);
        Assert.Equal((2u, 2u), (session.Stats.Tracked, session.Stats.Inferred));
        // the thumb leaves: the blossom is seen again
        Assert.Equal("D0123", session.Push(Frame(3, false), 375).Lanes);
        Assert.Equal((3u, 2u), (session.Stats.Tracked, session.Stats.Inferred));
        Assert.Equal(4u, session.Stats.Readable);
        Assert.Equal(4u, session.Stats.LaneD);
        Assert.Equal(16u, session.Stats.FieldLanes);
    }

    [Fact]
    public void IdleSessionsForgetPartialStreams()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(12_000, 3), 1);
        var session = new PetalScanSession(PetalScanLimits.Default with { IdleTimeout = TimeSpan.FromSeconds(1) });
        session.Push(PetalCaptureSimulator.Capture(Source(encoder, 0), Camera(0.0)), 0);
        Assert.True(session.Progress.Rank > 0);
        // a frame much later with nothing readable resets the session first
        var outcome = session.Push(new PetalLuma(640, 480), 60_000);
        Assert.Equal(PetalDecodeError.NoFinders, outcome.Error);
        Assert.Equal(0, outcome.Progress.Rank);
        Assert.Equal(2u, session.Stats.Frames);
    }

    [Fact]
    public void AbsoluteTimeoutForgetsSlowStreams()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(12_000, 3), 1);
        var session = new PetalScanSession(PetalScanLimits.Default with { AbsoluteTimeout = TimeSpan.FromSeconds(2) });
        session.Push(PetalTestSupport.RenderLuma(encoder, 0, 400, 1), 0);
        var rank = session.Progress.Rank;
        Assert.True(rank > 0);
        session.Push(PetalTestSupport.RenderLuma(encoder, 1, 400, 1), 1_500);
        Assert.True(session.Progress.Rank > rank);
        // still progressing, but the stream started more than two seconds ago
        var outcome = session.Push(PetalTestSupport.RenderLuma(encoder, 2, 400, 1), 2_100);
        Assert.Null(outcome.Progress.Meta); // frame 2 has no beacon: the restarted stream waits for one
        Assert.True(outcome.Lanes.Length > 0);
    }

    [Fact]
    public void LocatedCountsCodesThatWereSeenButCouldNotBeRead()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(100, 3), 1);
        var frame = Source(encoder, 1);
        // keep only the four blossoms: finders are located, no lane can be read
        const double scale = 512.0 / 1024.0;
        for (var y = 0; y < 512; y++)
        {
            for (var x = 0; x < 512; x++)
            {
                var nearFinder = PetalLayout.FinderCenters.Any(f =>
                    Math.Sqrt((x - f.X * scale) * (x - f.X * scale) + (y - f.Y * scale) * (y - f.Y * scale)) < 34.0);
                if (!nearFinder)
                    frame.Data.AsSpan((y * 512 + x) * 3, 3).Clear();
            }
        }

        var session = new PetalScanSession();
        var outcome = session.Push(frame.ToLuma(), 0);
        Assert.Equal(PetalDecodeError.NoOrientation, outcome.Error);
        Assert.Equal(1u, session.Stats.Located);
        Assert.Equal(0u, session.Stats.Readable);
        // a frame with no code at all is not "located"
        session.Push(new PetalLuma(320, 240), 100);
        Assert.Equal(1u, session.Stats.Located);
        Assert.Equal(2u, session.Stats.Frames);
    }

    [Fact]
    public void UnreadableFramesDoNotDisturbProgress()
    {
        var session = new PetalScanSession();
        var outcome = session.Push(new PetalLuma(320, 240), 5);
        Assert.True(outcome.Completed is null && outcome.Lanes.Length == 0);
        Assert.Equal(1u, session.Stats.Frames);
        Assert.Equal(0u, session.Stats.Located);
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalScanLimits.Default with { IdleTimeout = TimeSpan.FromSeconds(-1) });
    }

    [Fact]
    public void FramePlayerDefaultsToTwelveFramesPerSecondAdvancesAtItsRateAndWrapsTheFrameCounter()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(100, 1), 1);
        Assert.Equal(12.0, PetalFramePlayer.DefaultFramesPerSecond);
        Assert.Equal(12.0, new PetalFramePlayer(encoder).FramesPerSecond);
        var player = new PetalFramePlayer(encoder, 8.0, 65_534);
        Assert.Equal(65_534, player.FrameAt(TimeSpan.Zero));
        Assert.Equal(65_534, player.FrameAt(TimeSpan.FromSeconds(-3)));
        Assert.Equal(65_534, player.FrameAt(TimeSpan.FromMilliseconds(124)));
        Assert.Equal(65_535, player.FrameAt(TimeSpan.FromMilliseconds(125)));
        Assert.Equal(0, player.FrameAt(TimeSpan.FromMilliseconds(250)));
        Assert.Equal(TimeSpan.FromMilliseconds(125), player.FrameDuration);
        var first = player.DrawListAt(TimeSpan.FromMilliseconds(10));
        Assert.Same(first, player.DrawListAt(TimeSpan.FromMilliseconds(100)));
        Assert.NotSame(first, player.DrawListAt(TimeSpan.FromMilliseconds(130)));
        var image = player.RenderAt(TimeSpan.Zero, new PetalRenderOptions { Size = 128, Supersample = 1 });
        Assert.Equal(128, image.Width);
        var canvas = new CountingCanvas();
        player.Draw(canvas, TimeSpan.Zero, 512.0);
        var list = player.DrawListAt(TimeSpan.Zero);
        Assert.Equal(1 + list.BackdropCells.Count, canvas.Rectangles);
        Assert.Equal(list.Petals.Count, canvas.Petals);
        Assert.Equal(4 * 11 + list.Dots.Count, canvas.Circles);
        Assert.Equal(1, canvas.Clips);
        Assert.Equal(0, canvas.OpenClips);
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalFramePlayer(encoder, 0.0));
    }

    [Fact]
    public void CameraAnalyzerReassemblesFromPaddedYPlanes()
    {
        var data = PetalTestSupport.Payload(700, 2);
        var encoder = new PetalStreamEncoder(data, 2);
        var player = new PetalFramePlayer(encoder);
        var analyzer = new PetalCameraAnalyzer();
        PetalCompletedPayload? completed = null;
        analyzer.PayloadCompleted += (_, payload) => completed = payload;
        const int side = 512;
        const int stride = side + 40;
        var plane = new byte[stride * side];
        var cache = new Dictionary<ushort, PetalLuma>();
        for (var tick = 0; tick < 200 && completed is null; tick++)
        {
            // a 30 fps camera watching the default 12 fps display
            var elapsed = TimeSpan.FromSeconds(tick / 30.0);
            var frame = player.FrameAt(elapsed);
            if (!cache.TryGetValue(frame, out var luma))
                cache[frame] = luma = PetalTestSupport.RenderLuma(encoder, frame, side, 1);
            for (var row = 0; row < side; row++)
                luma.Data.AsSpan(row * side, side).CopyTo(plane.AsSpan(row * stride));
            var outcome = analyzer.AnalyzeYPlane(plane, side, side, stride, 1, (long)elapsed.TotalMilliseconds);
            Assert.NotNull(outcome);
        }

        Assert.NotNull(completed);
        Assert.Equal(data, completed.ToArray());
        Assert.True(analyzer.Session.Stats.FieldLanes > 0);
    }

    [Fact]
    public void CameraAnalyzerReportsInconsistentGeometryWithoutThrowing()
    {
        var analyzer = new PetalCameraAnalyzer();
        Assert.Equal(PetalDecodeError.UnsupportedImage, analyzer.AnalyzeYPlane(new byte[10], 100, 100, 100, 1, 0)!.Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, analyzer.AnalyzeYPlane(new byte[10_000], 100, 100, 50, 1, 0)!.Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, analyzer.AnalyzeYPlane(new byte[10_000], 0, 100, 100, 1, 0)!.Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, analyzer.AnalyzeYPlane(new byte[10_000], 100, 100, 100, 0, 0)!.Error);
        Assert.Equal(PetalDecodeError.UnsupportedImage, analyzer.AnalyzeYPlane(new byte[10], -5, -5, -5, 1, 0)!.Error);
        Assert.Equal(PetalDecodeError.NoFinders, analyzer.AnalyzeYPlane(new byte[10_000], 100, 100, 100, 1, 0)!.Error);
        Assert.Equal(PetalDecodeError.NoFinders, analyzer.Analyze(new PetalLuma(100, 100), 1)!.Error);
        analyzer.Reset();
        Assert.Equal(2u, analyzer.Session.Stats.Frames);
    }

    /// <summary>Counts draw calls and checks clip balance.</summary>
    private sealed class CountingCanvas : IPetalCanvas
    {
        public int Rectangles { get; private set; }

        public int Circles { get; private set; }

        public int Petals { get; private set; }

        public int Clips { get; private set; }

        public int OpenClips { get; private set; }

        public void FillRectangle(double x, double y, double width, double height, PetalColor color) => Rectangles++;

        public void FillCircle(double centerX, double centerY, double radius, PetalColor color) => Circles++;

        public void FillPetal(
            double centerX,
            double centerY,
            double radiusAlong,
            double radiusAcross,
            double angle,
            double notchDistance,
            double notchRadius,
            PetalColor color)
        {
            // a 512-pixel side is half the canvas
            Assert.True(Math.Abs(radiusAlong - 0.5 * PetalLayout.PetalHalfLength) < 1e-12);
            Assert.True(Math.Abs(radiusAcross - 0.5 * PetalLayout.PetalHalfWidth) < 1e-12);
            Assert.True(Math.Abs(notchDistance - 0.5 * PetalLayout.PetalNotchDistance) < 1e-12);
            Assert.True(Math.Abs(notchRadius - 0.5 * PetalLayout.PetalNotchRadius) < 1e-12);
            // petals are drawn after the backdrop's clip is popped
            Assert.Equal(0, OpenClips);
            Petals++;
        }

        public void PushCircleClip(double centerX, double centerY, double radius)
        {
            Assert.Equal((256.0, 256.0, 195.0), (centerX, centerY, radius));
            Clips++;
            OpenClips++;
        }

        public void PopClip() => OpenClips--;
    }
}
