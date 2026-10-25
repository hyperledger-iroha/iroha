using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Ports of the Rust unit tests of the pure Petal codecs (crc, prng, rs, layout, lanes, fountain).</summary>
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
    public void MaskIsMirrorSymmetricAndShowsWhichWayIsUp()
    {
        Assert.Equal(PetalLayout.MaskGrid, PetalLayout.Mask.Count);
        foreach (var row in PetalLayout.Mask)
        {
            Assert.Equal(PetalLayout.MaskGrid, row.Length);
            for (var column = 0; column < PetalLayout.MaskGrid / 2; column++)
                Assert.True(row[column] == row[PetalLayout.MaskGrid - 1 - column], $"row {row} not mirrored");
        }

        Assert.NotEqual(PetalLayout.Mask[0], PetalLayout.Mask[PetalLayout.MaskGrid - 1]);
    }

    [Fact]
    public void BandsHoldEveryCellAndSplitIntoEqualSectors()
    {
        Assert.Equal(PetalLayout.FieldCells, PetalLayout.BandOffsets[PetalLayout.BandCount]);
        Assert.Equal(PetalLayout.BandCount + 1, PetalLayout.BandOffsets.Count);
        var seen = new bool[PetalLayout.FieldCells];
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            var cells = PetalLayout.SectorCellOrder(sector);
            Assert.Equal(PetalLayout.SectorCells, cells.Count);
            foreach (var cell in cells)
            {
                Assert.False(seen[cell], $"cell {cell} in two sectors");
                seen[cell] = true;
                // a cell sits in its own quarter turn (odd bands lean half a slot forward)
                var (band, slot) = PetalLayout.CellBandSlot(cell);
                var quarter = PetalLayout.BandSlots[band] / PetalLayout.Sectors;
                Assert.Equal(sector, slot / quarter);
                var turn = PetalLayout.CellAngle(cell) / Math.Tau * PetalLayout.Sectors;
                Assert.True(turn > sector - 1e-9 && turn < sector + 1.0, $"cell {cell} turn {turn}");
            }

            // bit order: band by band from the centre out, ascending slot within a band
            for (var i = 1; i < cells.Count; i++)
                Assert.True(cells[i] > cells[i - 1]);
        }

        Assert.All(seen, static s => Assert.True(s));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLayout.SectorCellOrder(4));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLayout.CellCenter(PetalLayout.FieldCells));
        Assert.Equal((33, 191), PetalLayout.CellBandSlot(PetalLayout.FieldCells - 1));
        Assert.Equal((1, 0), PetalLayout.CellBandSlot(8));
    }

    [Fact]
    public void BandSlotsFollowThePitchRule()
    {
        for (var band = 0; band < PetalLayout.BandCount; band++)
        {
            var raw = Math.Tau * PetalLayout.BandRadius(band) / 50.0;
            Assert.Equal(4 * Math.Max((int)PetalMath.RoundHalfAwayFromZero(raw), 1), PetalLayout.BandSlots[band]);
            Assert.Equal(16.0 + 11.0 * band, PetalLayout.BandRadius(band));
            Assert.Equal(band % 2 == 1 ? 0.5 : 0.0, PetalLayout.BandPhase(band));
        }
    }

    [Fact]
    public void CellCentresFollowTheBandFormulaInDoublePrecision()
    {
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            var (band, slot) = PetalLayout.CellBandSlot(cell);
            var angle = Math.Tau * (slot + PetalLayout.BandPhase(band)) / PetalLayout.BandSlots[band];
            Assert.Equal(angle, PetalLayout.CellAngle(cell));
            var radius = PetalLayout.BandRadius(band);
            Assert.Equal(new PetalPoint(512.0 + radius * Math.Cos(angle), 512.0 + radius * Math.Sin(angle)), PetalLayout.CellCenter(cell));
            var center = PetalLayout.CellCenter(cell);
            Assert.Equal(PetalLayout.InMask(center.X, center.Y), PetalLayout.CellLight(cell));
        }

        // the single-precision petal constants widen exactly, as the reference widens them
        Assert.Equal(4.599999904632568359375, (double)PetalLayout.PetalRadius);
        Assert.Equal(0.4 * 4.599999904632568359375, 0.4 * PetalLayout.PetalRadius);
    }

    [Fact]
    public void TheFieldMapsOntoItselfUnderAQuarterTurn()
    {
        const double c = PetalLayout.Center;
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            var (band, slot) = PetalLayout.CellBandSlot(cell);
            var n = PetalLayout.BandSlots[band];
            var turned = PetalLayout.BandOffsets[band] + (slot + n / 4) % n;
            var (x, y) = PetalLayout.CellCenter(cell);
            var (tx, ty) = PetalLayout.CellCenter(turned);
            // a quarter turn clockwise on screen about the centre
            Assert.True(Math.Abs(tx - (c - (y - c))) < 1e-9 && Math.Abs(ty - (c + (x - c))) < 1e-9, $"cell {cell}");
        }
    }

    [Fact]
    public void PetalsStayClearOfTheirNeighboursAndTheRings()
    {
        // a petal reaches at most its half-length from its centre, whichever way it is turned
        var outer = PetalLayout.BandRadius(PetalLayout.BandCount - 1) + PetalLayout.PetalHalfLength;
        Assert.True(outer + 10.0 < PetalLayout.RingRadii[0] - PetalLayout.DotRadius);
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            var (x, y) = PetalLayout.CellCenter(cell);
            var (band, _) = PetalLayout.CellBandSlot(cell);
            var first = PetalLayout.BandOffsets[Math.Max(band - 1, 0)];
            var last = PetalLayout.BandOffsets[Math.Min(band + 2, PetalLayout.BandCount)];
            for (var other = first; other < last; other++)
            {
                if (other == cell)
                    continue;
                var (ox, oy) = PetalLayout.CellCenter(other);
                var distance = Math.Sqrt((x - ox) * (x - ox) + (y - oy) * (y - oy));
                Assert.True(distance > PetalLayout.PetalHalfLength + PetalLayout.PetalRadius, $"cells {cell} and {other} are {distance} apart");
            }
        }
    }

    [Fact]
    public void TheLightCellsDrawTheCharacter()
    {
        var light = Enumerable.Range(0, PetalLayout.FieldCells).Count(PetalLayout.CellLight);
        Assert.InRange(light, 1200, 2399);
        Assert.True(PetalLayout.InMask(512.0, 190.0), "the top bar");
        Assert.False(PetalLayout.InMask(512.0, 350.0), "the gap under the second bar");
        Assert.False(PetalLayout.InMask(181.9, 512.0));
        Assert.False(PetalLayout.InMask(182.0 + 20 * 33.0, 512.0));
        Assert.False(PetalLayout.InMask(double.PositiveInfinity, 512.0));
    }

    [Fact]
    public void RingSlotsProvideExactlyTheLaneDCapacity()
    {
        var roles = PetalLayout.SlotRoles();
        Assert.Equal(PetalLayout.TotalSlots, roles.Length);
        Assert.Equal(PetalLayout.DBits, roles.Count(static r => r.Kind == PetalSlotKind.Data));
        Assert.Equal(4 + 5 + 7, roles.Count(static r => r.Kind == PetalSlotKind.Gate));
        Assert.Equal(18, roles.Count(static r => r.Kind == PetalSlotKind.Guard));
        Assert.Equal(PetalLayout.DBits, PetalLayout.DataSlots().Length);
        Assert.Equal(PetalLayout.TotalSlots - PetalLayout.DBits - 16 - 18, roles.Count(static r => r.Kind == PetalSlotKind.Spare));
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
    }

    [Fact]
    public void APetalIsLitAtItsCentreAndNotchedAtItsTip()
    {
        Assert.True(PetalLayout.PetalLit(0.0, 0.0, 0.3));
        double tip = PetalLayout.PetalNotchDistance;
        Assert.False(PetalLayout.PetalLit(tip * Math.Cos(0.3), tip * Math.Sin(0.3), 0.3));
        Assert.False(PetalLayout.PetalLit(0.0, 10.0, 0.0));
        // half-length 6 along the petal, half-width 3.73 across it
        Assert.True(PetalLayout.PetalLit(0.0, 3.7, 0.0));
        Assert.False(PetalLayout.PetalLit(0.0, 3.76, 0.0));
        Assert.True(PetalLayout.PetalLit(-5.9, 0.0, 0.0));
        Assert.False(PetalLayout.PetalLit(-6.1, 0.0, 0.0));
        // the notch (radius 1.12, 6.24 out) cuts the tip: what tells a petal from its half turn
        Assert.True(PetalLayout.PetalLit(5.0, 0.0, 0.0));
        Assert.False(PetalLayout.PetalLit(5.5, 0.0, 0.0));
        Assert.True(PetalLayout.PetalLit(-5.5, 0.0, 0.0));
        // a petal turned by a quarter turn points down the screen
        Assert.True(PetalLayout.PetalLit(0.0, 5.0, Math.PI / 2.0));
        Assert.False(PetalLayout.PetalLit(0.0, 5.5, Math.PI / 2.0));
        Assert.False(PetalLayout.PetalLit(5.0, 0.0, Math.PI / 2.0));
    }

    [Fact]
    public void SlotCentersFollowTheSinglePrecisionReference()
    {
        Assert.Equal(new PetalPoint(917.0, 512.0), PetalLayout.SlotCenter(0, 0));
        // the slot angles use the single-precision TAU of the reference (`f32::consts::TAU`),
        // not twice a rounded single-precision pi, which is one ulp lower
        Assert.Equal(0x40C9_0FDB, BitConverter.SingleToInt32Bits(MathF.Tau));
        Assert.Equal((2, 0), PetalLayout.SplitSlot(PetalLayout.RingOffset(2)));
        Assert.Equal((1, 99), PetalLayout.SplitSlot(PetalLayout.RingOffset(2) - 1));
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

    private static byte[] FieldData(byte seed) =>
        Enumerable.Range(0, PetalLanes.FieldDataLength).Select(b => unchecked((byte)((b * 37) ^ seed))).ToArray();

    [Fact]
    public void LaneSizesAreConsistent()
    {
        Assert.Equal(30, PetalLanes.DWordLength);
        Assert.Equal((19, 83), (PetalLanes.DDataLength, PetalLanes.FieldDataLength));
        Assert.Equal(105, PetalLanes.FieldWordLength);
        Assert.Equal(4, PetalLanes.SpareSectorCells);
        Assert.Equal(21, PetalLanes.AtomsPerFrame);
        foreach (var lane in PetalLanes.DecodeOrder)
            Assert.Equal(PetalLanes.LaneHeaderLength + PetalLanes.AtomLength * PetalLanes.Atoms(lane), PetalLanes.DataLength(lane));
        Assert.Equal([PetalLane.D, PetalLane.F0, PetalLane.F1, PetalLane.F2, PetalLane.F3], PetalLanes.DecodeOrder);
        Assert.Equal([PetalLane.F0, PetalLane.F1, PetalLane.F2, PetalLane.F3], PetalLanes.Fields);
        Assert.Equal("D0123", string.Concat(PetalLanes.DecodeOrder.Select(PetalLanes.Letter)));
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            var lane = PetalLanes.Field(sector);
            Assert.True(PetalLanes.IsField(lane));
            Assert.Equal(sector, PetalLanes.Sector(lane));
        }

        Assert.False(PetalLanes.IsField(PetalLane.D));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLanes.Sector(PetalLane.D));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLanes.Field(4));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLanes.WordLength((PetalLane)5));
    }

    [Fact]
    public void WhiteningIsDeterministicAndBalanced()
    {
        foreach (var lane in PetalLanes.DecodeOrder)
        {
            var a = PetalLanes.Whitening(lane);
            Assert.Equal(a, PetalLanes.Whitening(lane));
            Assert.Equal(PetalLanes.WordLength(lane), a.Length);
            var ones = a.Sum(static b => System.Numerics.BitOperations.PopCount(b));
            var bits = a.Length * 8;
            Assert.True(ones > bits * 38 / 100 && ones < bits * 62 / 100, $"{lane} ones {ones}/{bits}");
        }

        Assert.NotEqual(PetalLanes.Whitening(PetalLane.F0), PetalLanes.Whitening(PetalLane.F1));
    }

    [Fact]
    public void LanesRoundtripThroughCells()
    {
        var dData = Enumerable.Range(0, PetalLanes.DDataLength).Select(static b => (byte)(b ^ 0xA5)).ToArray();
        var d = PetalLanes.EncodeLane(PetalLane.D, dData);
        var fields = Enumerable.Range(0, PetalLayout.Sectors)
            .Select(static s => PetalLanes.EncodeLane(PetalLanes.Field(s), FieldData((byte)s)))
            .ToArray();
        var cells = PetalFrameCells.FromWords(d, fields);
        Assert.Equal(d, cells.DWord());
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.D, cells.DWord(), [], out var decodedD));
        Assert.Equal(dData, decodedD);
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            Assert.Equal(fields[sector], cells.FieldWord(sector));
            Assert.True(PetalLanes.TryDecodeLane(PetalLanes.Field(sector), cells.FieldWord(sector), [], out var decoded));
            Assert.Equal(FieldData((byte)sector), decoded);
        }

        // the spare cells at the end of every sector stay dark
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            foreach (var cell in PetalLayout.SectorCellOrder(sector).Skip(PetalLanes.FieldWordLength * 8))
                Assert.False(cells.Petals[cell]);
        }

        // without turn codewords every petal is unturned; turned by the four codewords, the carriers turn
        Assert.All(cells.Turns, static turn => Assert.Equal(0, turn));
        var turns = Enumerable.Range(0, PetalLayout.Sectors)
            .Select(static s => PetalTurns.EncodeTurnLane(s, new byte[PetalTurns.TurnDataLength]))
            .ToArray();
        var turned = PetalFrameCells.FromWords(new PetalLaneData(d, fields, turns));
        Assert.Equal(cells.Petals, turned.Petals);
        Assert.NotEqual(cells, turned);
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            Assert.Equal(turns[sector], turned.TurnWord(sector));
        cells.SetTurns(turns);
        Assert.Equal(turned, cells);
        Assert.Equal(cells, cells.Clone());
        Assert.Equal(cells.GetHashCode(), cells.Clone().GetHashCode());
    }

    [Fact]
    public void AllZeroDataStillLightsAboutHalfThePetals()
    {
        var zero = new byte[PetalLanes.FieldDataLength];
        var fields = Enumerable.Range(0, PetalLayout.Sectors).Select(s => PetalLanes.EncodeLane(PetalLanes.Field(s), zero)).ToArray();
        var cells = PetalFrameCells.FromWords(PetalLanes.EncodeLane(PetalLane.D, new byte[PetalLanes.DDataLength]), fields);
        var lit = cells.Petals.Count(static l => l);
        Assert.InRange(lit, 1300, 2050);
    }

    [Fact]
    public void GateDotsAreAlwaysLitAndGuardsDark()
    {
        var field = Enumerable.Repeat((byte)0xFF, PetalLanes.FieldWordLength).ToArray();
        var cells = PetalFrameCells.FromWords(
            Enumerable.Repeat((byte)0xFF, PetalLanes.DWordLength).ToArray(),
            [field, field, field, field]);
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
        var data = FieldData(9);
        var clean = PetalLanes.EncodeLane(PetalLane.F2, data);
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.F2, clean, [], out var decoded, out var corrected));
        Assert.Equal(data, decoded);
        Assert.Equal(0, corrected);
        var damaged = (byte[])clean.Clone();
        foreach (var position in new[] { 0, 7, 19, 31, 60, 104 })
            damaged[position] ^= 0xC3;
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.F2, damaged, [], out decoded, out corrected));
        Assert.Equal(data, decoded);
        Assert.Equal(6, corrected);
        Assert.False(PetalLanes.TryDecodeLane(PetalLane.F2, damaged, [0, 0], out _, out corrected));
        Assert.Equal(0, corrected);
    }

    [Fact]
    public void AWordDecodedUnderAnotherSectorIsRefused()
    {
        var word = PetalLanes.EncodeLane(PetalLane.F0, FieldData(1));
        foreach (var lane in new[] { PetalLane.F1, PetalLane.F2, PetalLane.F3 })
            Assert.False(PetalLanes.TryDecodeLane(lane, word, [], out _), $"{lane}");
    }

    [Fact]
    public void LaneDecodeSurvivesBurstDamage()
    {
        var data = FieldData(3);
        var word = PetalLanes.EncodeLane(PetalLane.F1, data);
        for (var i = 0; i < PetalLanes.FieldParityLength / 2; i++)
            word[i] ^= 0x5A;
        Assert.True(PetalLanes.TryDecodeLane(PetalLane.F1, word, [], out var decoded));
        Assert.Equal(data, decoded);
    }

    [Fact]
    public void LaneCodecsRejectWrongShapes()
    {
        var field = new byte[PetalLanes.FieldWordLength];
        Assert.Throws<ArgumentException>(() => PetalLanes.EncodeLane(PetalLane.D, new byte[18]));
        Assert.Throws<ArgumentException>(() => PetalLanes.EncodeLane(PetalLane.F0, new byte[PetalLanes.DDataLength]));
        Assert.False(PetalLanes.TryDecodeLane(PetalLane.D, new byte[29], [], out _));
        Assert.False(PetalLanes.TryDecodeLane(PetalLane.F3, new byte[PetalLanes.DWordLength], [], out _));
        Assert.Throws<ArgumentException>(() => PetalFrameCells.FromWords(new byte[31], [field, field, field, field]));
        Assert.Throws<ArgumentException>(() => PetalFrameCells.FromWords(new byte[30], [field, field, field]));
        Assert.Throws<ArgumentException>(() => PetalFrameCells.FromWords(new byte[30], [field, field, field, new byte[104]]));
        Assert.Throws<ArgumentException>(() => new PetalLaneData(new byte[19], [field], [field, field, field, field]));
        Assert.Throws<ArgumentException>(() => new PetalLaneData(new byte[19], [field, field, field, field], [field]));
        Assert.Throws<ArgumentException>(() => new PetalFrameCells().SetTurns([field, field, field, field]));
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalFrameCells().TurnWord(4));
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalFrameCells().FieldWord(4));
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
