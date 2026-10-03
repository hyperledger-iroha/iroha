using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Ports of the Rust unit tests of the pure Petal codecs (crc, prng, rs, layout, glyphs, lanes, fountain).</summary>
public sealed class PetalCodecTests
{
    [Fact]
    public void Crc32CMatchesThePublishedCheckValue()
    {
        Assert.Equal(0xE306_9283u, PetalCrc32C.Compute("123456789"u8));
        Assert.Equal(0u, PetalCrc32C.Compute([]));
    }

    [Fact]
    public void Xorshift32MatchesTheReferenceSequenceAndRemapsZero()
    {
        var rng = new PetalXorshift32(1);
        Assert.Equal(270_369u, rng.NextUInt32());
        Assert.Equal(67_634_689u, rng.NextUInt32());
        Assert.Equal(2_647_435_461u, rng.NextUInt32());
        Assert.Equal(new PetalXorshift32(0xDEAD_BEEF), new PetalXorshift32(0));
        Assert.Equal(PetalXorshift32.ZeroSeedReplacement, new PetalXorshift32(0).State);
    }

    [Fact]
    public void ReedSolomonMatchesTheQrHelloWorldCheckVector()
    {
        // QR Code version 1-M "HELLO WORLD": 16 data codewords, 10 EC codewords.
        byte[] data = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17];
        byte[] expected = [196, 35, 39, 119, 235, 215, 231, 226, 93, 23];
        var word = new PetalReedSolomon(10).Encode(data);
        Assert.Equal(data, word[..16]);
        Assert.Equal(expected, word[16..]);
    }

    [Fact]
    public void ReedSolomonCorrectsRandomErrorsAndErasuresUpToCapacity()
    {
        var rng = new PetalLcg(7);
        foreach (var (k, nsym) in new[] { (16, 16), (60, 68), (12, 18), (13, 115) })
        {
            var rs = new PetalReedSolomon(nsym);
            for (var trial = 0; trial < 60; trial++)
            {
                var data = new byte[k];
                for (var i = 0; i < k; i++)
                    data[i] = rng.Byte();
                var clean = rs.Encode(data);
                var n = clean.Length;
                // pick f erasures and e errors with 2e + f <= nsym
                var f = rng.Below(Math.Min(nsym, n - 1) + 1);
                var e = rng.Below((nsym - f) / 2 + 1);
                var word = (byte[])clean.Clone();
                var positions = Enumerable.Range(0, n).ToArray();
                for (var i = 0; i < f + e; i++)
                {
                    var j = i + rng.Below(n - i);
                    (positions[i], positions[j]) = (positions[j], positions[i]);
                }

                var erased = positions[..f];
                var errored = positions[f..(f + e)];
                foreach (var p in erased)
                    word[p] = rng.Byte();
                foreach (var p in errored)
                    word[p] ^= (byte)(rng.Byte() | 1);
                Assert.True(rs.TryDecode(word, erased, out var corrected), $"k={k} nsym={nsym} f={f} e={e}");
                Assert.True(corrected <= f + e);
                Assert.Equal(clean, word);
            }
        }
    }

    [Fact]
    public void ReedSolomonRejectsWordsBeyondCapacityWithoutReturningWrongData()
    {
        var rng = new PetalLcg(99);
        var rs = new PetalReedSolomon(16);
        var wrongAccepts = 0;
        for (var trial = 0; trial < 200; trial++)
        {
            var data = new byte[16];
            for (var i = 0; i < data.Length; i++)
                data[i] = rng.Byte();
            var clean = rs.Encode(data);
            var word = (byte[])clean.Clone();
            // 20 random errors is far beyond t = 8
            var positions = Enumerable.Range(0, word.Length).ToArray();
            for (var i = 0; i < 20; i++)
            {
                var j = i + rng.Below(word.Length - i);
                (positions[i], positions[j]) = (positions[j], positions[i]);
            }

            foreach (var p in positions[..20])
                word[p] ^= (byte)(rng.Byte() | 1);
            if (rs.TryDecode(word, [], out _))
            {
                // a miscorrection must at least be a valid codeword
                Assert.True(rs.IsCodeword(word));
                if (!word.AsSpan().SequenceEqual(clean))
                    wrongAccepts++;
            }
        }

        Assert.True(wrongAccepts <= 2, $"miscorrection rate too high: {wrongAccepts}");
    }

    [Fact]
    public void ReedSolomonRejectsMalformedArguments()
    {
        var rs = new PetalReedSolomon(4);
        var shortWord = new byte[4];
        Assert.False(rs.TryDecode(shortWord, [], out _, out var error));
        Assert.Equal(PetalReedSolomonError.InvalidShape, error);
        var word = rs.Encode([1, 2, 3]);
        Assert.False(rs.TryDecode(word, [9], out _, out error));
        Assert.Equal(PetalReedSolomonError.InvalidShape, error);
        Assert.False(rs.TryDecode(word, [1, 1], out _, out error));
        Assert.Equal(PetalReedSolomonError.InvalidShape, error);
        Assert.False(rs.TryDecode(word, [-1], out _, out error));
        Assert.Equal(PetalReedSolomonError.InvalidShape, error);
        Assert.False(rs.TryDecode(new byte[256], [], out _, out error));
        Assert.Equal(PetalReedSolomonError.InvalidShape, error);
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalReedSolomon(0));
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalReedSolomon(255));
        Assert.Throws<ArgumentException>(() => new PetalReedSolomon(10).Encode(new byte[246]));
    }

    [Fact]
    public void ReedSolomonReportsUncorrectableWords()
    {
        var rs = new PetalReedSolomon(4);
        var word = rs.Encode([1, 2, 3, 4, 5]);
        var original = (byte[])word.Clone();
        word[0] ^= 1;
        word[1] ^= 2;
        word[2] ^= 4;
        var damaged = (byte[])word.Clone();
        if (!rs.TryDecode(word, [], out _, out var error))
        {
            Assert.Equal(PetalReedSolomonError.Uncorrectable, error);
            Assert.Equal(damaged, word); // untouched on failure
        }
        else
        {
            Assert.True(rs.IsCodeword(word));
            Assert.NotEqual(original, word);
        }

        Assert.Equal(1, PetalReedSolomon.GfExp(0));
        Assert.Equal(2, PetalReedSolomon.GfExp(1));
        Assert.Equal(PetalReedSolomon.GfExp(3), PetalReedSolomon.GfExp(258));
        Assert.Equal(0, PetalReedSolomon.GfMul(0, 77));
        Assert.Equal(1, PetalReedSolomon.GfMul(PetalReedSolomon.GfExp(200), PetalReedSolomon.GfExp(55)));
    }

    [Fact]
    public void MaskHas256MirrorSymmetricTilesAndShowsWhichWayIsUp()
    {
        Assert.Equal(PetalLayout.TileGrid, PetalLayout.Mask.Count);
        var count = 0;
        foreach (var row in PetalLayout.Mask)
        {
            Assert.Equal(PetalLayout.TileGrid, row.Length);
            for (var column = 0; column < PetalLayout.TileGrid / 2; column++)
                Assert.Equal(row[column], row[PetalLayout.TileGrid - 1 - column]);
            count += row.Count(static c => c == '#');
        }

        Assert.Equal(PetalLayout.TileCount, count);
        Assert.NotEqual(PetalLayout.Mask[0], PetalLayout.Mask[PetalLayout.TileGrid - 1]);
    }

    [Fact]
    public void TilesAreRowMajorAndInsideTheCanvas()
    {
        var previous = (Column: 0, Row: 0);
        for (var index = 0; index < PetalLayout.TileCount; index++)
        {
            var tile = PetalLayout.Tile(index);
            if (index > 0)
                Assert.True(tile.Row > previous.Row || (tile.Row == previous.Row && tile.Column > previous.Column));
            previous = tile;
            var center = PetalLayout.TileCenter(index);
            Assert.InRange(center.X, 0.0, PetalLayout.Canvas - 1e-9);
            Assert.InRange(center.Y, 0.0, PetalLayout.Canvas - 1e-9);
            Assert.Equal(PetalLayout.TileOrigin + PetalLayout.TilePitch * (tile.Column + 0.5), center.X);
        }
    }

    [Fact]
    public void RingSlotsProvideExactlyTheLaneDCapacity()
    {
        var roles = PetalLayout.SlotRoles();
        Assert.Equal(PetalLayout.TotalSlots, roles.Length);
        Assert.Equal(PetalLayout.DBits, roles.Count(static r => r.Kind == PetalSlotKind.Data));
        Assert.Equal(4 + 5 + 7, roles.Count(static r => r.Kind == PetalSlotKind.Gate));
        Assert.Equal(PetalLayout.DBits, PetalLayout.DataSlots().Length);
        // the two spare slots are the last non-reserved slots of the outer ring
        Assert.Equal(2, roles.Count(static r => r.Kind == PetalSlotKind.Spare));
        Assert.Equal(PetalLayout.GateSlots, Enumerable.Range(0, roles.Length).Where(i => roles[i].Kind == PetalSlotKind.Gate));
        Assert.Equal(PetalLayout.GuardSlots, Enumerable.Range(0, roles.Length).Where(i => roles[i].Kind == PetalSlotKind.Guard));
        var dataSlots = PetalLayout.DataSlots();
        for (var bit = 0; bit < dataSlots.Length; bit++)
            Assert.Equal(new PetalSlotRole(PetalSlotKind.Data, bit), roles[dataSlots[bit]]);
    }

    [Fact]
    public void GatesNeverTouchTheTop()
    {
        for (var ring = 0; ring < PetalLayout.RingCount; ring++)
        {
            var n = PetalLayout.RingSlots[ring];
            var (gates, guards) = PetalLayout.GateSlotsOfRing(ring);
            foreach (var slot in gates.Concat(guards))
                Assert.True(Math.Abs(slot - 3 * n / 4) > 2, $"ring {ring} slot {slot} near the top");
        }
    }

    [Fact]
    public void FindersAndRingsDoNotOverlap()
    {
        var outermost = PetalLayout.RingRadii[2] + PetalLayout.DotRadius;
        foreach (var finder in PetalLayout.FinderCenters)
        {
            var distance = Math.Sqrt(Math.Pow(finder.X - PetalLayout.Center, 2) + Math.Pow(finder.Y - PetalLayout.Center, 2));
            Assert.True(distance - PetalLayout.FinderOuter > outermost + 20.0);
        }

        var farthest = 0.0;
        for (var tile = 0; tile < PetalLayout.TileCount; tile++)
        {
            var c = PetalLayout.TileCenter(tile);
            var h = PetalLayout.TileSize / 2.0;
            foreach (var (x, y) in new[] { (c.X - h, c.Y - h), (c.X + h, c.Y - h), (c.X - h, c.Y + h), (c.X + h, c.Y + h) })
                farthest = Math.Max(farthest, Math.Sqrt(Math.Pow(x - PetalLayout.Center, 2) + Math.Pow(y - PetalLayout.Center, 2)));
        }

        Assert.True(farthest + 10.0 < PetalLayout.RingRadii[0] - PetalLayout.DotRadius);
    }

    [Fact]
    public void SlotCentersFollowTheSinglePrecisionReference()
    {
        Assert.Equal(new PetalPoint(872.0, 512.0), PetalLayout.SlotCenter(0, 0));
        Assert.Equal((2, 0), PetalLayout.SplitSlot(PetalLayout.RingOffset(2)));
        Assert.Equal((1, 91), PetalLayout.SplitSlot(PetalLayout.RingOffset(2) - 1));
        for (var ring = 0; ring < PetalLayout.RingCount; ring++)
        {
            for (var slot = 0; slot < PetalLayout.RingSlots[ring]; slot++)
            {
                var center = PetalLayout.SlotCenter(ring, slot);
                // every coordinate is an exactly widened single-precision value
                Assert.Equal(center.X, (double)(float)center.X);
                Assert.Equal(center.Y, (double)(float)center.Y);
                var angle = Math.Tau * slot / PetalLayout.RingSlots[ring];
                Assert.True(Math.Abs(center.X - (512.0 + PetalLayout.RingRadii[ring] * Math.Cos(angle))) < 1e-3);
                Assert.True(Math.Abs(center.Y - (512.0 + PetalLayout.RingRadii[ring] * Math.Sin(angle))) < 1e-3);
            }
        }
    }

    [Fact]
    public void FinderBlossomIsSolidWithNotchedPetals()
    {
        Assert.True(PetalLayout.FinderLit(0.0, 0.0));
        Assert.True(PetalLayout.FinderLit(0.0, -34.0)); // upper petal centre
        Assert.False(PetalLayout.FinderLit(0.0, -59.0)); // inside the upper notch
        Assert.False(PetalLayout.FinderLit(0.0, 59.0)); // between the two lower petals
        Assert.False(PetalLayout.FinderLit(70.0, 0.0));
    }

    [Fact]
    public void CheckedInTemplatesMatchTheStrokeDefinitions()
    {
        var generated = PetalGlyphs.GenerateTemplates();
        for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
            Assert.Equal(PetalGlyphs.Template(glyph).ToArray(), generated[glyph]);
        Assert.Equal(16, PetalGlyphs.Characters.Length);
    }

    [Fact]
    public void EveryGlyphHasInkInsideTheDesignGrid()
    {
        for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
        {
            var total = 0;
            foreach (var value in PetalGlyphs.Template(glyph))
                total += value;
            Assert.True(total > 255 * 6, $"glyph {glyph} has too little ink");
            foreach (var stroke in PetalGlyphs.Strokes(glyph))
            {
                Assert.True(stroke.Count >= 2);
                foreach (var point in stroke)
                {
                    Assert.InRange(point.X, 2.0, 30.0);
                    Assert.InRange(point.Y, 2.0, 30.0);
                }
            }
        }
    }

    [Fact]
    public void GlyphsArePairwiseDistinctUnderBlur()
    {
        static double[] Feature(int glyph)
        {
            var values = PetalGlyphs.Template(glyph).ToArray().Select(static c => (double)c).ToArray();
            var mean = values.Sum() / values.Length;
            var centered = values.Select(v => v - mean).ToArray();
            var norm = Math.Sqrt(centered.Sum(static v => v * v));
            return centered.Select(v => v / norm).ToArray();
        }

        for (var a = 0; a < PetalGlyphs.GlyphCount; a++)
        {
            for (var b = a + 1; b < PetalGlyphs.GlyphCount; b++)
            {
                var dot = Feature(a).Zip(Feature(b), static (x, y) => x * y).Sum();
                Assert.True(1.0 - dot > 0.2, $"glyphs {a} and {b} are too similar: {1.0 - dot}");
            }
        }
    }

    [Fact]
    public void LaneSizesAreConsistent()
    {
        Assert.Equal(32, PetalLanes.PWordLength);
        Assert.Equal(128, PetalLanes.KWordLength);
        Assert.Equal(30, PetalLanes.DWordLength);
        Assert.Equal((19, 83, 19), (PetalLanes.PDataLength, PetalLanes.KDataLength, PetalLanes.DDataLength));
        foreach (var lane in PetalLanes.DecodeOrder)
        {
            Assert.Equal(
                PetalLanes.LaneHeaderLength + PetalLanes.AtomLength * lane switch
                {
                    PetalLane.P => PetalLanes.PAtoms,
                    PetalLane.K => PetalLanes.KAtoms,
                    _ => PetalLanes.DAtoms,
                },
                PetalLanes.DataLength(lane));
        }

        Assert.Equal([PetalLane.P, PetalLane.D, PetalLane.K], PetalLanes.DecodeOrder);
    }

    [Fact]
    public void WhiteningIsDeterministicAndBalanced()
    {
        foreach (var lane in PetalLanes.DecodeOrder)
        {
            var a = PetalLanes.Whitening(lane);
            Assert.Equal(a, PetalLanes.Whitening(lane));
            var ones = a.Sum(static b => System.Numerics.BitOperations.PopCount(b));
            var bits = a.Length * 8;
            Assert.True(ones > bits * 38 / 100 && ones < bits * 62 / 100, $"{lane} ones {ones}/{bits}");
        }
    }

    [Fact]
    public void LanesRoundtripThroughCells()
    {
        var pData = Enumerable.Range(0, PetalLanes.PDataLength).Select(static b => (byte)b).ToArray();
        var kData = Enumerable.Range(0, PetalLanes.KDataLength).Select(static b => unchecked((byte)(b * 37))).ToArray();
        var dData = Enumerable.Range(0, PetalLanes.DDataLength).Select(static b => (byte)(b ^ 0xA5)).ToArray();
        var (p, k, d) = (
            PetalLanes.EncodeLane(PetalLane.P, pData),
            PetalLanes.EncodeLane(PetalLane.K, kData),
            PetalLanes.EncodeLane(PetalLane.D, dData));
        var cells = PetalFrameCells.FromWords(p, k, d);
        Assert.Equal(p, cells.PWord());
        Assert.Equal(k, cells.KWord());
        Assert.Equal(d, cells.DWord());
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.P, cells.PWord(), [], out var decodedP));
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.K, cells.KWord(), [], out var decodedK));
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.D, cells.DWord(), [], out var decodedD));
        Assert.Equal(pData, decodedP);
        Assert.Equal(kData, decodedK);
        Assert.Equal(dData, decodedD);
        Assert.Equal(cells, cells.Clone());
        Assert.Equal(cells.GetHashCode(), cells.Clone().GetHashCode());
    }

    [Fact]
    public void AllZeroDataStillLightsRoughlyHalfTheCells()
    {
        var cells = PetalFrameCells.FromWords(
            PetalLanes.EncodeLane(PetalLane.P, new byte[PetalLanes.PDataLength]),
            PetalLanes.EncodeLane(PetalLane.K, new byte[PetalLanes.KDataLength]),
            PetalLanes.EncodeLane(PetalLane.D, new byte[PetalLanes.DDataLength]));
        var lit = cells.Light.Count(static l => l);
        Assert.InRange(lit, 90, 166);
    }

    [Fact]
    public void GateDotsAreAlwaysLitAndGuardsDark()
    {
        var full = (byte)0xFF;
        var cells = PetalFrameCells.FromWords(
            Enumerable.Repeat(full, PetalLanes.PWordLength).ToArray(),
            Enumerable.Repeat(full, PetalLanes.KWordLength).ToArray(),
            Enumerable.Repeat(full, PetalLanes.DWordLength).ToArray());
        var roles = PetalLayout.SlotRoles();
        for (var slot = 0; slot < roles.Length; slot++)
        {
            var expected = roles[slot].Kind is PetalSlotKind.Gate or PetalSlotKind.Data;
            Assert.Equal(expected, cells.Dots[slot]);
        }
    }

    [Fact]
    public void CountedDecodingReportsTheRewrittenPositions()
    {
        var data = Enumerable.Range(0, PetalLanes.PDataLength).Select(static b => (byte)b).ToArray();
        var clean = PetalLanes.EncodeLane(PetalLane.P, data);
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.P, clean, [], out var decoded, out var corrected));
        Assert.Equal(data, decoded);
        Assert.Equal(0, corrected);
        var damaged = (byte[])clean.Clone();
        foreach (var position in new[] { 0, 7, 19, 31 })
            damaged[position] ^= 0xC3;
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.P, damaged, [], out decoded, out corrected));
        Assert.Equal(data, decoded);
        Assert.Equal(4, corrected);
        Assert.False(PetalLanes.TryDecodeLane(PetalLane.P, damaged, [0, 0], out _, out corrected));
        Assert.Equal(0, corrected);
    }

    [Fact]
    public void LaneDecodeSurvivesBurstDamage()
    {
        var kData = Enumerable.Range(0, PetalLanes.KDataLength).Select(static b => (byte)b).ToArray();
        var word = PetalLanes.EncodeLane(PetalLane.K, kData);
        for (var i = 0; i < PetalLanes.KParityLength / 2; i++)
            word[i] ^= 0x5A;
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.K, word, [], out var data));
        Assert.Equal(kData, data);
    }

    [Fact]
    public void LaneCodecsRejectWrongShapes()
    {
        Assert.Throws<ArgumentException>(() => PetalLanes.EncodeLane(PetalLane.P, new byte[18]));
        Assert.False(PetalLanes.TryDecodeLane(PetalLane.D, new byte[29], [], out _));
        Assert.Throws<ArgumentException>(() => PetalFrameCells.FromWords(new byte[31], new byte[128], new byte[30]));
        Assert.Throws<ArgumentException>(() => PetalFrameCells.FromWords(new byte[32], new byte[127], new byte[30]));
        Assert.Throws<ArgumentException>(() => PetalFrameCells.FromWords(new byte[32], new byte[128], new byte[31]));
    }

    [Fact]
    public void SystematicAtomsAloneRecoverThePayload()
    {
        var data = PetalTestSupport.Payload(100, 5);
        var source = PetalFountain.SplitPayload(data);
        var decoder = new PetalFountainDecoder(source.Length);
        for (var id = 0; id < source.Length; id++)
            Assert.True(decoder.AddEncoded(7, (uint)id, source[id]));
        Assert.Equal(data, Reassemble(decoder.Solve()!, 100));
    }

    [Fact]
    public void RepairAtomsCoverForLostSystematicAtoms()
    {
        var data = PetalTestSupport.Payload(1000, 9);
        var source = PetalFountain.SplitPayload(data);
        var k = source.Length;
        const uint crc = 0x1234_5678;
        var decoder = new PetalFountainDecoder(k);
        // lose every third systematic atom, then take repair atoms
        for (var id = 0u; id < k; id++)
        {
            if (id % 3 != 0)
                decoder.AddEncoded(crc, id, PetalFountain.EncodeAtom(source, crc, id));
        }

        var next = (uint)k;
        var used = 0;
        while (!decoder.IsComplete)
        {
            decoder.AddEncoded(crc, next, PetalFountain.EncodeAtom(source, crc, next));
            next++;
            used++;
            Assert.True(used < k, "decoder must converge");
        }

        var missing = Enumerable.Range(0, k).Count(static i => i % 3 == 0);
        Assert.True(used <= missing + 8, $"needed {used} repairs for {missing} missing");
        Assert.Equal(data, Reassemble(decoder.Solve()!, 1000));
    }

    [Fact]
    public void PureRepairStreamsDecodeWithSmallOverhead()
    {
        var data = PetalTestSupport.Payload(5000, 11);
        var source = PetalFountain.SplitPayload(data);
        var k = source.Length;
        const uint crc = 0xCAFE_F00D;
        var totalOverhead = 0;
        for (var trial = 0u; trial < 20; trial++)
        {
            var decoder = new PetalFountainDecoder(k);
            var id = (uint)k + trial * 1000;
            var received = 0;
            while (!decoder.IsComplete)
            {
                decoder.AddEncoded(crc, id, PetalFountain.EncodeAtom(source, crc, id));
                id++;
                received++;
            }

            totalOverhead += received - k;
            Assert.Equal(data, Reassemble(decoder.Solve()!, 5000));
        }

        Assert.True(totalOverhead <= 20 * 4, $"average overhead {totalOverhead / 20.0}");
    }

    [Fact]
    public void DuplicateAndDependentAtomsDoNotRaiseRank()
    {
        var source = PetalFountain.SplitPayload(PetalTestSupport.Payload(60, 3));
        var decoder = new PetalFountainDecoder(source.Length);
        Assert.True(decoder.AddEncoded(1, 0, source[0]));
        Assert.False(decoder.AddEncoded(1, 0, source[0]));
        Assert.Equal(1, decoder.Rank);
        Assert.Null(decoder.Solve());
        Assert.False(decoder.Add(new uint[2], source[0]));
        Assert.Throws<ArgumentException>(() => decoder.AddEncoded(1, 1, new byte[15]));
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalFountainDecoder(0));
    }

    [Fact]
    public void RepairMasksSpanFarMoreThanThirtyTwoDimensions()
    {
        // Regression: an xorshift-derived mask is GF(2)-linear in a 32-bit seed
        // and can never exceed rank 32.
        const int k = 200;
        var decoder = new PetalFountainDecoder(k);
        for (var id = (uint)k; id < k + 400; id++)
            decoder.Add(PetalFountain.MaskWords(k, 5, id), new byte[PetalLanes.AtomLength]);
        Assert.Equal(k, decoder.Rank);
    }

    [Fact]
    public void MasksAreNonZeroAndPaddedBitsAreClear()
    {
        foreach (var k in new[] { 1, 2, 31, 32, 33, 100 })
        {
            for (var id = 0u; id < 200; id++)
            {
                var mask = PetalFountain.MaskWords(k, 99, id);
                Assert.Equal(PetalFountain.MaskLength(k), mask.Length);
                Assert.Contains(mask, static w => w != 0);
                if (k % 32 != 0)
                    Assert.Equal(0u, mask[^1] >> (k % 32));
            }
        }

        Assert.Throws<ArgumentOutOfRangeException>(() => PetalFountain.MaskWords(0, 1, 1));
    }

    [Fact]
    public void FountainSurvivesHeavyLossAndArbitraryOrder()
    {
        var data = PetalTestSupport.Payload(2_000, 41);
        var source = PetalFountain.SplitPayload(data);
        var k = source.Length;
        const uint crc = 0x0BAD_F00D;
        var rng = new PetalLcg(1234);
        // ids 0..3k, a third of them lost, the rest delivered in a random order
        var ids = Enumerable.Range(0, 3 * k).Select(static i => (uint)i).Where(_ => rng.Below(3) != 0).ToArray();
        for (var i = ids.Length - 1; i > 0; i--)
        {
            var j = rng.Below(i + 1);
            (ids[i], ids[j]) = (ids[j], ids[i]);
        }

        var decoder = new PetalFountainDecoder(k);
        var offered = 0;
        foreach (var id in ids)
        {
            decoder.AddEncoded(crc, id, PetalFountain.EncodeAtom(source, crc, id));
            offered++;
            if (decoder.IsComplete)
                break;
        }

        Assert.True(decoder.IsComplete);
        Assert.True(offered <= k + 12, $"needed {offered} atoms for k = {k}");
        Assert.Equal(data, Reassemble(decoder.Solve()!, data.Length));
    }

    [Fact]
    public void Mix32IsTheMurmurFinalizer()
    {
        Assert.Equal(0u, PetalFountain.Mix32(0));
        // fmix32(1) from the MurmurHash3 reference implementation
        Assert.Equal(0x514E_28B7u, PetalFountain.Mix32(1));
    }

    private static byte[] Reassemble(byte[][] atoms, int length) => atoms.SelectMany(static a => a).Take(length).ToArray();
}
