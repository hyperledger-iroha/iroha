using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Ports of the Rust decoder and session tests plus robustness and platform-glue coverage.</summary>
public sealed class PetalDecoderTests
{
    private static readonly byte[] SetupPayload = Enumerable.Range(0, 300)
        .Select(static i => (byte)(unchecked((uint)i * 2_654_435_761u) >> 11))
        .ToArray();

    private static readonly PetalStreamEncoder SetupEncoder = new(SetupPayload, 2);

    private const int Cells = PetalGlyphs.TemplateSize * PetalGlyphs.TemplateSize;
    private const int PatchValues = PetalLayout.TileCount * Cells;

    [Fact]
    public void CleanRenderDecodesEveryLane()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 5, 768, 2);
        var decoded = PetalDecoder.Decode(luma);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.Null(decoded.Error);
        var (p, k, d) = SetupEncoder.LaneData(5);
        Assert.Equal(p, decoded.Frame.P?.Data);
        Assert.Equal(k, decoded.Frame.K?.Data);
        Assert.Equal(d, decoded.Frame.D?.Data);
        Assert.Equal((0, false), (decoded.Frame.Rotation, decoded.Frame.Mirrored));
        Assert.Equal(3, decoded.Frame.LanesOk);
        Assert.Equal("PKD", decoded.Frame.Lanes);
        Assert.Null(decoded.Frame.Beacon());
        Assert.Equal(3, decoded.Frame.AtomPackets().Count);
        var observed = PetalDecoder.ObservedCells(luma, decoded.Frame);
        Assert.Equal(SetupEncoder.Cells(5), observed);
        var error = PetalDecoder.TileMatchError(luma, decoded.Frame);
        Assert.NotNull(error);
        Assert.InRange(error.Value, 0.0, 1.0);
    }

    [Fact]
    public void BeaconFramesExposeTheBeacon()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 8, 600, 2);
        var decoded = PetalDecoder.Decode(luma);
        Assert.True(decoded.Success);
        var beacon = decoded.Frame.Beacon();
        Assert.NotNull(beacon);
        Assert.Equal(SetupEncoder.Meta, beacon.Value.Meta);
        Assert.Equal(8, beacon.Value.Header.Frame);
        Assert.Equal(2, decoded.Frame.AtomPackets().Count);
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
        var source = PetalTestSupport.RenderLuma(SetupEncoder, 7, 768, 2);
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
        var (p, _, d) = SetupEncoder.LaneData(7);
        Assert.Equal(d, decoded.Frame.D?.Data);
        Assert.Equal(p, decoded.Frame.P?.Data);
        // without mirror hypotheses a mirrored picture cannot be read
        if (mirrored)
            Assert.False(PetalDecoder.Decode(image, PetalDecodeOptions.Default with { TryMirrored = false }).Success);
    }

    [Fact]
    public void CorrectedCountsRewrittenBytesNotJustErasures()
    {
        var data = Enumerable.Range(0, PetalLanes.PDataLength).Select(static b => (byte)b).ToArray();
        var word = PetalLanes.EncodeLane(PetalLane.P, data);
        foreach (var position in new[] { 2, 11, 30 })
            word[position] ^= 0x5A;
        var confidence = Enumerable.Repeat(1.0, word.Length).ToArray();
        var result = PetalDecoder.DecodeWithErasures(PetalLane.P, word, confidence);
        Assert.NotNull(result);
        Assert.Equal(data, result.Data);
        Assert.Equal(0, result.Erasures);
        Assert.Equal(3, result.Corrected);
        // with the damaged bytes flagged as least confident, they become erasures
        var flagged = Enumerable.Repeat(1.0, word.Length).ToArray();
        foreach (var position in new[] { 2, 11, 30 })
            flagged[position] = 0.0;
        result = PetalDecoder.DecodeWithErasures(PetalLane.P, word, flagged);
        Assert.NotNull(result);
        Assert.Equal(data, result.Data);
        Assert.True(result.Corrected >= 3);
    }

    [Fact]
    public void RandomWordsAreAlmostNeverAccepted()
    {
        // Reed–Solomon with erasures can accept a word that is not a transmission. Lane D has only
        // 11 parity bytes, so its schedule stops at five erasures; at seven it let through about
        // one random word in 250 (150 of these 40 000). Lane P stops at six for the same reason.
        // The counts are exact so that every port, fed the same xorshift32 words and byte-valued
        // confidences, reproduces the decoder bit for bit. They do not prove the ranking is
        // stable (a reversed tie-break gives the same counts): see the two tie tests below.
        var rng = new PetalXorshift32(0x5EED);
        const int trials = 40_000;
        foreach (var (lane, expected) in new[] { (PetalLane.D, 3), (PetalLane.P, 0) })
        {
            var length = PetalLanes.DataLength(lane) + PetalLanes.ParityLength(lane);
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
    public void OnlyLaneKUsesTwoThirdsOfItsParityAsErasures()
    {
        // damaged bytes: `flagged` of them marked least confident, two more hidden. With the extra
        // erasure step of the old schedule the decoder would repair them (2·2 + flagged parity
        // bytes); the capped schedule must refuse instead of risking a wrong codeword.
        foreach (var (lane, flagged) in new[] { (PetalLane.D, 7), (PetalLane.P, 8) })
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

            damaged[20] ^= 0x3C;
            var result = PetalDecoder.DecodeWithErasures(lane, damaged, confidence);
            Assert.True(result is not null, $"lane {lane}");
            Assert.Equal(data, result.Data);
            Assert.True(result.Erasures <= PetalLanes.ParityLength(lane) / 2, $"lane {lane}");
        }

        // lane K keeps the two-thirds step: 30 flagged bytes plus 7 hidden errors need it
        // (2·7 + 30 = 44 of 45 parity bytes)
        var kData = Enumerable.Range(0, PetalLanes.KDataLength).Select(static i => (byte)i).ToArray();
        var kDamaged = PetalLanes.EncodeLane(PetalLane.K, kData);
        var kConfidence = Enumerable.Repeat(1.0, kDamaged.Length).ToArray();
        for (var position = 0; position < 30; position++)
        {
            kDamaged[position] ^= 0xA5;
            kConfidence[position] = 0.0;
        }

        for (var position = 60; position < 67; position++)
            kDamaged[position] ^= 0x3C;
        var kResult = PetalDecoder.DecodeWithErasures(PetalLane.K, kDamaged, kConfidence);
        Assert.NotNull(kResult);
        Assert.Equal(kData, kResult.Data);
        Assert.Equal(30, kResult.Erasures);
    }

    [Fact]
    public void EqualConfidencesAreErasedInPositionOrder()
    {
        // Every tile the normalised read erases has confidence exactly 0, so ties are the rule, and
        // the ranking must be stable or ports disagree about which bytes are erased. Three damaged
        // bytes at the front plus four hidden ones fit lane D only if exactly the first three
        // positions are erased (3 erasures + 4 errors = all 11 parity bytes): a step that erased
        // the last positions instead would see seven errors.
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
    [InlineData(PetalLane.P, 7)]
    [InlineData(PetalLane.K, 23)]
    public void FirstErasureStepFollowsByteOrderWhenConfidencesTie(PetalLane lane, int damagedBytes)
    {
        // One more damaged byte than the code corrects unaided, all at the front of the word and
        // all equally confident. Only the first erasure step can repair it, and only if the tie is
        // resolved in byte order: that step erases byte 0, one of the damaged ones. A ranking that
        // reorders ties erases an intact byte instead and every later step is out of budget.
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
        foreach (var length in new[] { PetalLanes.DWordLength, PetalLanes.PWordLength, PetalLanes.KWordLength })
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

    /// <summary>The exact canvas-to-pixel homography of the 768-pixel test renders.</summary>
    private static PetalHomography RenderHomography()
    {
        var canonical = PetalLayout.FinderCenters.ToArray();
        const double scale = 768.0 / 1024.0;
        var pixels = canonical.Select(static point => new PetalPoint(point.X * scale, point.Y * scale)).ToArray();
        return PetalHomography.FromPoints(canonical, pixels) ?? throw new InvalidOperationException("render homography");
    }

    /// <summary>A clean 768-pixel render with the exact canvas-to-pixel homography and its raw patches.</summary>
    private static (PetalLuma Luma, PetalHomography Pose, double[] Patches) CleanPatches(ushort frame)
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, frame, 768, 2);
        var pose = RenderHomography();
        var patches = new double[PatchValues];
        PetalDecoder.SamplePatches(luma, pose, patches);
        return (luma, pose, patches);
    }

    private static PetalDecoder.Reference LevelsOf(PetalLuma luma, PetalHomography pose)
    {
        var reference = PetalDecoder.ReferenceLevels(luma, pose);
        Assert.NotNull(reference);
        return reference.Value;
    }

    /// <summary>Lane <c>P</c> or <c>K</c> as the reads of one tile pass rank it, or <see langword="null"/>.</summary>
    private static PetalLaneResult? LaneOf(PetalLane lane, PetalDecoder.TileRead[] reads)
    {
        var (p, pConfidence, k, kConfidence) = PetalDecoder.TileWords(reads);
        return lane == PetalLane.P
            ? PetalDecoder.DecodeWithErasures(PetalLane.P, p, pConfidence)
            : PetalDecoder.DecodeWithErasures(PetalLane.K, k, kConfidence);
    }

    [Fact]
    public void LevelAndNormalisedReadsAgreeOnACleanRender()
    {
        var (luma, pose, patches) = CleanPatches(5);
        var (pData, kData, _) = SetupEncoder.LaneData(5);
        var sigmas = PetalDecodeOptions.Default.TemplateSigmas;
        var reference = LevelsOf(luma, pose);
        foreach (var (name, reads) in new[]
        {
            ("level", PetalDecoder.ReadTiles(patches, reference, sigmas)),
            ("normalised", PetalDecoder.ReadTilesNormalised(patches, sigmas)),
        })
        {
            var p = LaneOf(PetalLane.P, reads);
            var k = LaneOf(PetalLane.K, reads);
            Assert.True(p is not null && k is not null, name);
            Assert.Equal(pData, p.Data);
            Assert.Equal(0, p.Corrected);
            Assert.Equal(kData, k.Data);
            Assert.Equal(0, k.Corrected);
        }
    }

    [Fact]
    public void NormalisedReadCancelsGainAndOffsetPerTile()
    {
        var (luma, pose, patches) = CleanPatches(5);
        var (pData, kData, _) = SetupEncoder.LaneData(5);
        var sigmas = PetalDecodeOptions.Default.TemplateSigmas;
        // every tile gets its own gain and offset, as under glare, shadows and saturation
        var distorted = new double[patches.Length];
        for (var tile = 0; tile < PetalLayout.TileCount; tile++)
        {
            var gain = 0.35 + 0.65 * (tile * 37 % 101 / 100.0);
            var offset = 5.0 + tile * 53 % 61;
            for (var cell = 0; cell < Cells; cell++)
                distorted[tile * Cells + cell] = gain * patches[tile * Cells + cell] + offset;
        }

        var reads = PetalDecoder.ReadTiles(distorted, LevelsOf(luma, pose), sigmas);
        Assert.Null(LaneOf(PetalLane.P, reads));
        reads = PetalDecoder.ReadTilesNormalised(distorted, sigmas);
        Assert.Equal(pData, LaneOf(PetalLane.P, reads)?.Data);
        Assert.Equal(kData, LaneOf(PetalLane.K, reads)?.Data);
    }

    [Fact]
    public void NormalisedReadErasesTilesThatLostTheirContrast()
    {
        var (_, _, patches) = CleanPatches(5);
        Array.Fill(patches, 100.0, 5 * Cells, Cells);
        Array.Fill(patches, 30.0, 9 * Cells, Cells);
        var reads = PetalDecoder.ReadTilesNormalised(patches, PetalDecodeOptions.Default.TemplateSigmas);
        foreach (var tile in new[] { 5, 9 })
        {
            Assert.True(Math.Abs(reads[tile].PolarityMargin) < 1e-12, $"tile {tile}");
            Assert.True(Math.Abs(reads[tile].GlyphMargin) < 1e-12, $"tile {tile}");
        }

        Assert.True(reads[6].PolarityMargin > 0.0 && reads[6].GlyphMargin > 0.0);
    }

    [Fact]
    public void PatchLevelsIgnoreTheExtremeCells()
    {
        var values = new double[Cells];
        Array.Fill(values, 10.0);
        for (var i = 0; i < Cells / 2; i++)
            values[i] = 200.0 + i % 3;
        values[0] = 255.0; // one hot cell
        values[Cells - 1] = 0.0; // one dead cell
        var (low, high) = PetalDecoder.PatchLevels(values);
        Assert.True(Math.Abs(low - 10.0) < 1e-12);
        Assert.InRange(high, 200.0, 202.0);
        var scaled = new double[Cells];
        PetalDecoder.Rescale(values, 1.0, scaled);
        Assert.All(scaled, static value => Assert.InRange(value, -0.25, 1.25));
        // a flat patch stays flat instead of dividing by nothing
        var flat = new double[Cells];
        Array.Fill(flat, 7.0);
        PetalDecoder.Rescale(flat, 1.0, scaled);
        Assert.All(scaled, static value => Assert.True(Math.Abs(value) < 1e-12));
        Assert.Throws<ArgumentException>(() => PetalDecoder.PatchLevels(new double[Cells - 1]));
    }

    [Fact]
    public void ShadowedPartOfARenderDecodesThroughTheNormalisedRead()
    {
        var luma = ShadowedRender(5, out var pData, out var kData);
        // the finder levels cannot describe a step in the light: the level read loses lane K
        var pose = RenderHomography();
        var sigmas = PetalDecodeOptions.Default.TemplateSigmas;
        var patches = new double[PatchValues];
        PetalDecoder.SamplePatches(luma, pose, patches);
        var levelReads = PetalDecoder.ReadTiles(patches, LevelsOf(luma, pose), sigmas);
        Assert.Null(LaneOf(PetalLane.K, levelReads));
        var decoded = PetalDecoder.Decode(luma);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.Equal(pData, decoded.Frame.P?.Data);
        Assert.Equal(kData, decoded.Frame.K?.Data);
    }

    /// <summary>A 768-pixel render with a vertical band of the picture at 30 % light.</summary>
    private static PetalLuma ShadowedRender(ushort frame, out byte[] pData, out byte[] kData)
    {
        (pData, kData, _) = SetupEncoder.LaneData(frame);
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, frame, 768, 2);
        var width = luma.Width;
        for (var row = 0; row < luma.Height; row++)
        {
            for (var x = width * 7 / 20; x < width * 3 / 5; x++)
            {
                var at = row * width + x;
                luma.Data[at] = (byte)PetalMath.RoundHalfAwayFromZero(luma.Data[at] * 0.3);
            }
        }

        return luma;
    }

    [Fact]
    public void TheNormalisedReadOnlyFillsInTheLanesTheLevelReadMissed()
    {
        var luma = ShadowedRender(5, out var pData, out var kData);
        var pose = RenderHomography();
        var sigmas = PetalDecodeOptions.Default.TemplateSigmas;
        var patches = new double[PatchValues];
        PetalDecoder.SamplePatches(luma, pose, patches);
        var reference = LevelsOf(luma, pose);
        var levelReads = PetalDecoder.ReadTiles(patches, reference, sigmas);
        var (p, k) = PetalDecoder.ReadTileLanes(patches, reference, sigmas);
        Assert.Equal(kData, k?.Data);
        // a lane the level read decoded is kept exactly as the level read left it
        if (LaneOf(PetalLane.P, levelReads) is { } levelP)
            Assert.Equal(levelP, p);
        else
            Assert.Equal(pData, p?.Data);

        // on a clean render the level read decodes both lanes and is the final answer
        var (cleanLuma, cleanPose, cleanPatches) = CleanPatches(5);
        var cleanReference = LevelsOf(cleanLuma, cleanPose);
        var (cleanP, cleanK) = PetalDecoder.ReadTileLanes(cleanPatches, cleanReference, sigmas);
        var cleanReads = PetalDecoder.ReadTiles(cleanPatches, cleanReference, sigmas);
        Assert.Equal(LaneOf(PetalLane.P, cleanReads), cleanP);
        Assert.Equal(LaneOf(PetalLane.K, cleanReads), cleanK);
    }

    [Fact]
    public void DecodeAtReadsTheShadowedRenderThroughTheNormalisedRead()
    {
        var luma = ShadowedRender(5, out var pData, out var kData);
        var (_, _, dData) = SetupEncoder.LaneData(5);
        var frame = PetalDecoder.DecodeAt(luma, RenderHomography());
        Assert.NotNull(frame);
        Assert.Equal(pData, frame.P?.Data);
        Assert.Equal(kData, frame.K?.Data);
        Assert.Equal(dData, frame.D?.Data);
    }

    [Fact]
    public void ATileLaneAloneIsEnoughToAcceptAnOrientation()
    {
        var cells = SetupEncoder.Cells(5).Clone();
        // fourteen polarity flips in fourteen different bytes of lane P: no choice of erasures
        // lets its 13 parity bytes repair that, yet every glyph is still drawn, so lane K is intact
        for (var j = 0; j < 14; j++)
        {
            var tile = 8 * j + 3;
            cells.Light[tile] = !cells.Light[tile];
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
        Assert.Equal("K", result.Frame.Lanes);
        Assert.Equal(SetupEncoder.LaneData(5).K, result.Frame.K?.Data);
    }

    [Fact]
    public void NormalisedReadWorksWithEveryTemplateBlurList()
    {
        var (_, _, patches) = CleanPatches(5);
        var (pData, kData, _) = SetupEncoder.LaneData(5);
        foreach (var sigmas in new double[][] { [0.0], [0.8], [1.5, 1.1, 0.8, 0.5, 0.0], [0.3, 0.6, 0.9, 1.2, 1.5, 1.8] })
        {
            var reads = PetalDecoder.ReadTilesNormalised(patches, sigmas);
            Assert.Equal(pData, LaneOf(PetalLane.P, reads)?.Data);
            Assert.Equal(kData, LaneOf(PetalLane.K, reads)?.Data);
        }
    }

    [Fact]
    public void GarbagePatchesNeverDecodeThroughEitherRead()
    {
        var reference = new PetalDecoder.Reference();
        for (var corner = 0; corner < 4; corner++)
            reference.Set(corner, 230.0 - corner, 12.0 + corner);
        var rng = new PetalXorshift32(31);
        var sigmas = PetalDecodeOptions.Default.TemplateSigmas;
        for (var style = 0; style < 6; style++)
        {
            var patches = new double[PatchValues];
            for (var i = 0; i < patches.Length; i++)
            {
                patches[i] = style switch
                {
                    0 => rng.NextByte(), // white noise
                    1 => 100.0, // flat
                    2 => i % Cells < Cells / 2 ? 240.0 : 5.0, // identical stripes in every tile
                    3 => double.NaN,
                    4 => rng.NextUInt32() % 7 == 0 ? 255.0 : 0.0, // sparse specks
                    _ => i * 0.37 % 255.0, // slowly varying ramp
                };
            }

            var (p, k) = PetalDecoder.ReadTileLanes(patches, reference, sigmas);
            Assert.True(p is null && k is null, $"style {style}");
        }
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
        Assert.Null(PetalDecoder.DecodeAt(new PetalLuma(8, 8), PetalHomography.Identity));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalDecodeOptions.Default with { MaxPixels = -1 });
        Assert.Throws<ArgumentException>(() => PetalDecodeOptions.Default with { TemplateSigmas = [] });
        Assert.Throws<ArgumentException>(() => PetalDecodeOptions.Default with { TemplateSigmas = [double.NaN] });
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
        // exercises the locator, quad selection and homography on degenerate layouts.
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
                Assert.Equal(0, result.Frame.LanesOk);
        }
    }

    [Fact]
    public void DecodeAtSurvivesHostileHomographies()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 3, 300, 1);
        var hostile = new[]
        {
            new PetalHomography(new double[9]),
            new PetalHomography(double.NaN, 0, 0, 0, double.NaN, 0, 0, 0, 1),
            new PetalHomography(double.PositiveInfinity, 0, 0, 0, 1, 0, 0, 0, 1),
            new PetalHomography(1, 0, 0, 0, 1, 0, 1, 1, -600),
            new PetalHomography(1e300, 1e300, 1e300, -1e300, 1e300, 1, 0, 0, 1e-300),
        };
        foreach (var h in hostile)
        {
            var frame = PetalDecoder.DecodeAt(luma, h);
            if (frame is not null)
                Assert.Equal(0, frame.LanesOk);
        }
    }

    [Fact]
    public void DecodeAtWithTheTruePoseReadsEveryLane()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 6, 512, 2);
        var pose = new PetalHomography(0.5, 0, 0, 0, 0.5, 0, 0, 0, 1);
        var frame = PetalDecoder.DecodeAt(luma, pose);
        Assert.NotNull(frame);
        var (p, k, d) = SetupEncoder.LaneData(6);
        Assert.Equal(p, frame.P?.Data);
        Assert.Equal(k, frame.K?.Data);
        Assert.Equal(d, frame.D?.Data);
        Assert.Equal(pose, frame.Homography);
    }

    [Fact]
    public void AValidCodeWithAMissingFinderIsNotMisread()
    {
        var luma = PetalTestSupport.RenderLuma(SetupEncoder, 2, 768, 2);
        var n = luma.Width;
        // erase the bottom-right blossom
        for (var y = n * 3 / 4; y < n; y++)
        {
            for (var x = n * 3 / 4; x < n; x++)
                luma.Data[y * n + x] = 0;
        }

        Assert.False(PetalDecoder.Decode(luma).Success);
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
            var image = PetalCaptureSimulator.Capture(source, PetalCaptureSimulator.FitToFrame(config, 4.0));
            var result = PetalDecoder.Decode(image);
            Assert.True(result.Success, $"config {index}: {result.Error}");
            var (p, k, d) = encoder.LaneData(frame);
            if (result.Frame.P is { } lp)
                Assert.Equal(p, lp.Data);
            if (result.Frame.K is { } lk)
                Assert.Equal(k, lk.Data);
            if (result.Frame.D is { } ld)
                Assert.Equal(d, ld.Data);
            decodedLanes += result.Frame.LanesOk;
            // the decoder's homography maps the canvas centre near the simulated one
            var expected = PetalCaptureSimulator.CameraHomography(PetalCaptureSimulator.FitToFrame(config, 4.0)).Apply(512, 512);
            var actual = result.Frame.Homography.Apply(512, 512);
            if (config.LensK1 == 0.0)
                Assert.True(Math.Abs(expected.X - actual.X) < 3.0 && Math.Abs(expected.Y - actual.Y) < 3.0, $"config {index}");
        }

        Assert.True(decodedLanes >= 6);
    }

    [Fact]
    public void ASessionReceivesAPayloadFromSimulatedCaptures()
    {
        var data = PetalTestSupport.Payload(500, 3);
        var encoder = new PetalStreamEncoder(data, 2);
        var config = PetalCaptureSimulator.FitToFrame(
            PetalCaptureConfig.Modern() with { Width = 640, Height = 480, RotationDeg = 20.0 },
            4.0);
        var session = new PetalScanSession();
        PetalCompletedPayload? done = null;
        for (var frame = 0; frame < 40; frame++)
        {
            var source = PetalRenderer.Render(encoder.Cells((ushort)frame), new PetalRenderOptions { Size = 512, Supersample = 2 });
            var outcome = session.Push(PetalCaptureSimulator.Capture(source, config), frame * 125L);
            if (outcome.Completed is not null)
            {
                done = outcome.Completed;
                break;
            }
        }

        Assert.NotNull(done);
        Assert.Equal(data, done.ToArray());
        Assert.Equal(2, done.Meta.Kind);
        Assert.True(session.Stats.Readable > 0 && session.Stats.LaneD > 0);
        Assert.True(session.Progress.Complete);
    }

    [Fact]
    public void IdleSessionsForgetPartialStreams()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(4_000, 3), 1);
        var config = PetalCaptureSimulator.FitToFrame(PetalCaptureConfig.Modern() with { Width = 640, Height = 480 }, 4.0);
        var session = new PetalScanSession(PetalScanLimits.Default with { IdleTimeout = TimeSpan.FromSeconds(1) });
        var source = PetalRenderer.Render(encoder.Cells(0), new PetalRenderOptions { Size = 512, Supersample = 2 });
        session.Push(PetalCaptureSimulator.Capture(source, config), 0);
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
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(4_000, 3), 1);
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
        var frame = PetalRenderer.Render(encoder.Cells(1), new PetalRenderOptions { Size = 512, Supersample = 2 });
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
    public void FramePlayerAdvancesAndWrapsTheFrameCounter()
    {
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(100, 1), 1);
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
        Assert.Equal(1, canvas.Rectangles);
        Assert.Equal(PetalLayout.TileCount, canvas.Clips);
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
            // a 30 fps camera watching an 8 fps display
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
        Assert.True(analyzer.Session.Stats.LaneK > 0);
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

        public int Clips { get; private set; }

        public int OpenClips { get; private set; }

        public void FillRectangle(double x, double y, double width, double height, PetalColor color) => Rectangles++;

        public void FillRoundedRectangle(double x, double y, double width, double height, double cornerRadius, PetalColor color)
        {
        }

        public void FillCircle(double centerX, double centerY, double radius, PetalColor color)
        {
        }

        public void StrokePolyline(ReadOnlySpan<PetalPoint> points, double strokeWidth, PetalColor color) =>
            Assert.True(points.Length >= 2 && Math.Abs(strokeWidth - 6.5 / 32.0 * 23.0 * 0.5) < 1e-9);

        public void PushClip(double x, double y, double width, double height)
        {
            Clips++;
            OpenClips++;
        }

        public void PopClip() => OpenClips--;
    }
}
