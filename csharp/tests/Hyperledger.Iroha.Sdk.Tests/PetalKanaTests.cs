using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Ports of the Rust kana layer, card layout, glyph, card render and band session tests
/// (<c>kana.rs</c>, <c>layout.rs</c>, <c>glyphs.rs</c>, <c>render.rs</c>, <c>session.rs</c>) plus
/// argument and robustness coverage.
/// </summary>
public sealed class PetalKanaTests
{
    /// <summary>The stream of the reference session tests: 3,000 bytes from xorshift32 seed 3.</summary>
    private static readonly PetalStreamEncoder SessionStream = new(PetalTestSupport.Payload(3000, 3), 2);

    /// <summary>A clean 768-pixel card of frame 6 of the session stream (2× supersampling), shared by several tests.</summary>
    private static readonly Lazy<PetalLuma> CleanCard = new(() => PetalTestSupport.RenderLuma(SessionStream, 6, 768, 2));

    /// <summary>The frame cells of the reference renderer tests: lane data derived from <paramref name="seed"/>.</summary>
    private static PetalFrameCells Cells(byte seed)
    {
        var d = Enumerable.Range(0, PetalLanes.DDataLength).Select(b => unchecked((byte)((b * 13) ^ seed))).ToArray();
        var fields = Enumerable.Range(0, PetalLayout.Sectors)
            .Select(sector => PetalLanes.EncodeLane(
                PetalLanes.Field(sector),
                Enumerable.Range(0, PetalLanes.FieldDataLength).Select(b => unchecked((byte)((b * 17 + sector) ^ seed))).ToArray()))
            .ToArray();
        return PetalFrameCells.FromWords(PetalLanes.EncodeLane(PetalLane.D, d), fields);
    }

    /// <summary>The header the lanes of a decoded capture name.</summary>
    private static PetalLaneHeader HeaderOf(PetalDecodedFrame frame) =>
        frame.AtomPackets().FirstOrDefault()?.Header ?? frame.Beacon()!.Value.Header;

    [Fact]
    public void LaneSizesAndIdsAreConsistent()
    {
        Assert.Equal(149, PetalKana.KanaGlyphWordLength);
        Assert.Equal(37, PetalKana.KanaPolarityWordLength);
        Assert.Equal(PetalKana.KanaGlyphWordLength - PetalKana.KanaGlyphParityLength, PetalKana.KanaGlyphAtoms * PetalLanes.AtomLength + PetalKana.KanaCheckLength);
        Assert.Equal(PetalKana.KanaPolarityWordLength - PetalKana.KanaPolarityParityLength, PetalKana.KanaPolarityAtoms * PetalLanes.AtomLength + PetalKana.KanaCheckLength);
        Assert.Equal(PetalLayout.KanaTileCount, PetalLayout.KanaTiles.Count);
        Assert.Equal(PetalKana.KanaDataTiles, PetalLayout.KanaDataTiles(0).Count);
        var ids = new HashSet<uint>();
        foreach (var frame in new ushort[] { 0, 1, 65_535 })
        {
            foreach (var lane in PetalKana.Lanes)
            {
                for (var k = 0u; k < PetalKana.Atoms(lane); k++)
                    Assert.True(ids.Add(PetalKana.KanaFirstId(frame, lane) + k), $"frame {frame} lane {lane} atom {k}");
            }
        }

        // above every turn atom id and below the u32 limit
        Assert.True(PetalKana.KanaFirstId(0, PetalKanaLane.K0) >= PetalTurns.TurnFirstId(65_535, 3) + 4);
        Assert.True(PetalKana.KanaFirstId(65_535, PetalKanaLane.P1) < uint.MaxValue - 16);
        Assert.Equal(["P0", "K0", "P1", "K1"], PetalKana.Lanes.Select(PetalKana.Name));
        Assert.Equal(new byte[] { 0x10, 0x11, 0x12, 0x13 }, PetalKana.Lanes.Select(PetalKana.Code));
        Assert.Equal([0, 0, 1, 1], PetalKana.Lanes.Select(PetalKana.Band));
        Assert.Equal([false, true, false, true], PetalKana.Lanes.Select(PetalKana.IsGlyph));
        Assert.Equal([20, 116, 20, 116], PetalKana.Lanes.Select(PetalKana.DataLength));
        Assert.Equal(PetalKanaLane.K1, PetalKana.Lane(1, glyph: true));
        Assert.Equal(PetalKanaLane.P0, PetalKana.Lane(0, glyph: false));
        Assert.Equal(PetalKana.KanaIdBase + 16u * 77 + 8 + 7, PetalKana.KanaFirstId(77, PetalKanaLane.P1));
    }

    [Fact]
    public void WhiteningIsBalancedAndDistinctPerLane()
    {
        var all = PetalKana.Lanes.Select(PetalKana.Whitening).ToArray();
        foreach (var (lane, whitening) in PetalKana.Lanes.Zip(all))
        {
            var ones = whitening.Sum(static b => System.Numerics.BitOperations.PopCount(b));
            var bits = whitening.Length * 8;
            Assert.True(ones > bits * 35 / 100 && ones < bits * 65 / 100, $"{lane}: {ones} of {bits}");
            Assert.Equal(PetalKana.WordLength(lane), whitening.Length);
        }

        Assert.NotEqual(all[1][..37], all[0]);
        Assert.NotEqual(all[1], all[3]);
        Assert.NotSame(PetalKana.Whitening(PetalKanaLane.K0), PetalKana.Whitening(PetalKanaLane.K0));
    }

    [Fact]
    public void SymbolsRoundtripThroughCodewords()
    {
        var header = new PetalLaneHeader(0x5A, 77);
        var symbols = PetalKana.PilotSymbols();
        var words = new List<(byte[] P, byte[] G, byte[] PAtoms, byte[] GAtoms)>();
        for (var band = 0; band < PetalLayout.KanaBands; band++)
        {
            var polarityLane = PetalKana.Lane(band, glyph: false);
            var glyphLane = PetalKana.Lane(band, glyph: true);
            var pAtoms = Enumerable.Range(0, 16).Select(i => (byte)(i ^ band)).ToArray();
            var gAtoms = Enumerable.Range(0, 112).Select(i => (byte)(unchecked((byte)(i * 7)) ^ band)).ToArray();
            var p = PetalKana.EncodeKanaLane(polarityLane, PetalKana.KanaData(header, polarityLane, pAtoms));
            var g = PetalKana.EncodeKanaLane(glyphLane, PetalKana.KanaData(header, glyphLane, gAtoms));
            PetalKana.SetKanaSymbols(symbols, band, p, g);
            words.Add((p, g, pAtoms, gAtoms));
        }

        for (var band = 0; band < words.Count; band++)
        {
            var (p, g, pAtoms, gAtoms) = words[band];
            Assert.Equal(p, PetalKana.PolarityWord(symbols, band));
            Assert.Equal(g, PetalKana.GlyphWord(symbols, band));
            var lane = PetalKana.Lane(band, glyph: true);
            Assert.True(PetalKana.TryDecodeKanaLane(lane, g, [], out var data, out var corrected));
            Assert.Equal(0, corrected);
            var packet = PetalKana.KanaPacket(header, lane, data);
            Assert.NotNull(packet);
            Assert.Equal(PetalKana.KanaFirstId(77, lane), packet.FirstId);
            Assert.Equal(gAtoms, packet.Atoms.SelectMany(static atom => atom));
            // the check refuses another frame, tag, band or kind
            Assert.Null(PetalKana.KanaPacket(header with { Frame = 78 }, lane, data));
            Assert.Null(PetalKana.KanaPacket(header with { Tag = 0x5B }, lane, data));
            Assert.Null(PetalKana.KanaPacket(header, PetalKana.Lane(1 - band, glyph: true), data));
            var polarity = PetalKana.Lane(band, glyph: false);
            Assert.True(PetalKana.TryDecodeKanaLane(polarity, p, [], out var polarityData, out _));
            Assert.Equal(pAtoms, PetalKana.KanaPacket(header, polarity, polarityData)!.Atoms.SelectMany(static atom => atom));
            // a codeword read under another lane's whitening does not decode
            Assert.False(PetalKana.TryDecodeKanaLane(PetalKana.Lane(1 - band, glyph: true), g, [], out _, out _));
        }

        // pilots keep their symbols; the data tiles beyond the polarity codeword are light
        for (var index = 0; index < PetalLayout.KanaTiles.Count; index++)
        {
            if (PetalLayout.KanaTiles[index].Pilot is { } pilot)
                Assert.Equal(PetalLayout.KanaPilotSymbols[pilot], symbols[index]);
        }

        for (var band = 0; band < PetalLayout.KanaBands; band++)
        {
            var data = PetalLayout.KanaDataTiles(band);
            Assert.True(symbols[data[296]].Light && symbols[data[297]].Light, $"band {band}");
        }

        // frame cells show the same codewords
        var cells = new PetalFrameCells();
        cells.SetKana([words[0].P, words[0].G, words[1].P, words[1].G]);
        Assert.Equal(symbols, cells.Kana);
        Assert.Equal(words[1].G, cells.KanaGlyphWord(1));
        Assert.Equal(cells, cells.Clone());
        Assert.NotEqual(new PetalFrameCells(), cells);
    }

    [Fact]
    public void TemplatesAreUnitAndTellGlyphsApart()
    {
        const int points = PetalKana.KanaGrid * PetalKana.KanaGrid;
        for (var blur = 0; blur < PetalKana.KanaBlurs.Count; blur++)
        {
            for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
            {
                foreach (var light in new[] { true, false })
                {
                    var norm = 0.0;
                    var sum = 0.0;
                    for (var p = 0; p < points; p++)
                    {
                        var value = PetalKana.TemplateProbe(blur, glyph, light, p % PetalKana.KanaGrid, p / PetalKana.KanaGrid);
                        norm += value * value;
                        sum += value;
                    }

                    Assert.True(Math.Abs(norm - 1.0) < 1e-9, $"blur {blur} glyph {glyph} light {light}: {norm}");
                    Assert.True(Math.Abs(sum) < 1e-9, $"blur {blur} glyph {glyph} light {light} is mean-centred: {sum}");
                }
            }
        }

        // at the sharpest blur no two light templates correlate above 0.8
        for (var a = 0; a < PetalGlyphs.GlyphCount; a++)
        {
            for (var b = a + 1; b < PetalGlyphs.GlyphCount; b++)
            {
                var dot = 0.0;
                for (var p = 0; p < points; p++)
                    dot += PetalKana.TemplateProbe(0, a, true, p % PetalKana.KanaGrid, p / PetalKana.KanaGrid) * PetalKana.TemplateProbe(0, b, true, p % PetalKana.KanaGrid, p / PetalKana.KanaGrid);
                Assert.True(dot < 0.8, $"glyphs {a} and {b}: {dot}");
            }
        }

        // the coarse templates are unit vectors too
        const int coarse = PetalKana.KanaCoarseGrid * PetalKana.KanaCoarseGrid;
        for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
        {
            foreach (var light in new[] { true, false })
            {
                var norm = 0.0;
                for (var p = 0; p < coarse; p++)
                    norm += Math.Pow(PetalKana.CoarseTemplateProbe(glyph, light, p % PetalKana.KanaCoarseGrid, p / PetalKana.KanaCoarseGrid), 2);
                Assert.True(Math.Abs(norm - 1.0) < 1e-9, $"coarse glyph {glyph} light {light}: {norm}");
            }
        }
    }

    [Fact]
    public void TheCardHolds306TilesPerBandAndEightPilotsEach()
    {
        Assert.Equal(322.0, PetalLayout.KanaBandHeight);
        Assert.Equal(1668.0, PetalLayout.CardHeight);
        Assert.Equal(PetalLayout.KanaBandHeight, PetalLayout.SquareTop);
        var tiles = PetalLayout.KanaTiles;
        for (var band = 0; band < PetalLayout.KanaBands; band++)
        {
            var inBand = tiles.Where(t => t.Band == band).ToArray();
            Assert.Equal(306, inBand.Length);
            Assert.Equal(PetalLayout.KanaPilots, inBand.Count(static t => t.IsPilot));
            Assert.Equal(306 - PetalLayout.KanaPilots, PetalLayout.KanaDataTiles(band).Count);
            var pilots = PetalLayout.KanaPilotTiles(band);
            for (var p = 0; p < pilots.Count; p++)
            {
                Assert.Equal(p, tiles[pilots[p]].Pilot);
                Assert.Equal(PetalLayout.KanaPilotCell(band, p), (tiles[pilots[p]].Row, tiles[pilots[p]].Column));
            }

            // the data tiles are the band's other tiles, in reading order
            Assert.Equal(
                Enumerable.Range(0, tiles.Count).Where(i => tiles[i].Band == band && !tiles[i].IsPilot),
                PetalLayout.KanaDataTiles(band));
        }

        // band 0 comes first, each band in reading order
        for (var i = 1; i < tiles.Count; i++)
            Assert.True((tiles[i - 1].Band, tiles[i - 1].Row, tiles[i - 1].Column).CompareTo((tiles[i].Band, tiles[i].Row, tiles[i].Column)) < 0);
        // the bands mirror each other about the square's middle
        foreach (var tile in tiles.Where(static t => t.Band == 0))
        {
            var mirror = tiles.Single(t => t.Band == 1 && t.Column == tile.Column && t.Row == PetalLayout.KanaRows - 1 - tile.Row);
            Assert.True(Math.Abs(mirror.Y + PetalLayout.KanaTile - PetalLayout.Canvas + tile.Y) < 1e-9);
            Assert.Equal(tile.Pilot, mirror.Pilot);
        }

        // every tile keeps the finder clearance and sits on the lattice of the specification
        foreach (var tile in tiles)
        {
            Assert.Equal(6.5 + 29.0 * tile.Column, tile.X);
            Assert.Equal((tile.Band == 0 ? -312.0 : 1050.0) + 29.0 * tile.Row, tile.Y);
            foreach (var finder in PetalLayout.FinderCenters)
                Assert.True(Math.Sqrt(Math.Pow(tile.Center.X - finder.X, 2) + Math.Pow(tile.Center.Y - finder.Y, 2)) >= PetalLayout.KanaFinderClearance);
        }

        // every tile centre maps back to its tile, and the square holds none
        for (var i = 0; i < tiles.Count; i++)
        {
            var center = tiles[i].Center;
            Assert.Equal((i, 12.5, 12.5), PetalLayout.KanaTileAt(center.X, center.Y));
        }

        Assert.Null(PetalLayout.KanaTileAt(512.0, 512.0));
        Assert.Null(PetalLayout.KanaTileAt(5.0, -300.0)); // a gutter
        Assert.Null(PetalLayout.KanaTileAt(double.NaN, -300.0));
        Assert.Null(PetalLayout.KanaTileAt(100.0, double.NaN));
        Assert.Null(PetalLayout.KanaTileAt(double.PositiveInfinity, -300.0));
        Assert.Null(PetalLayout.KanaTileAt(-1e300, 2000.0));
        // a cell near a finder has no tile: row 9 of band 0, column 0 is 198 units clear of nothing
        Assert.Null(PetalLayout.KanaTileAt(6.5 + 12.5, -312.0 + 29.0 * 9 + 12.5));
    }

    [Fact]
    public void TilesAreRoundedSquares()
    {
        Assert.True(PetalLayout.KanaInside(12.5, 12.5));
        Assert.True(PetalLayout.KanaInside(0.0, 12.5));
        Assert.True(PetalLayout.KanaInside(3.0, 0.0));
        Assert.False(PetalLayout.KanaInside(0.2, 0.2), "a rounded corner");
        Assert.True(PetalLayout.KanaInside(25.0 - 0.9, 25.0 - 0.9));
        Assert.False(PetalLayout.KanaInside(25.01, 12.5));
        Assert.False(PetalLayout.KanaInside(double.NaN, 12.5));
        // ロ is an outline: inked on its frame, not in its middle
        Assert.True(PetalLayout.KanaInked(1, 1.0 + 6.0 * 23.0 / 32.0, 12.5));
        Assert.False(PetalLayout.KanaInked(1, 12.5, 12.5));
        Assert.True(PetalGlyphs.IsInked(1, 6.0, 16.0));
        Assert.False(PetalGlyphs.IsInked(1, 16.0, 16.0));
        Assert.True(PetalGlyphs.IsInked(1, 6.0 - 3.25, 16.0));
        Assert.False(PetalGlyphs.IsInked(1, 6.0 - 3.26, 16.0));
    }

    [Fact]
    public void EveryGlyphHasInkInsideTheDesignGridAndStaysDistinct()
    {
        // inked share of every cell of an 8 × 8 grid over the design square
        static double[] Coverage(int glyph)
        {
            const double cell = PetalGlyphs.GlyphGrid / 8.0;
            var values = new double[64];
            for (var i = 0; i < 64; i++)
            {
                var (u, v) = ((double)(i % 8), (double)(i / 8));
                var inked = 0;
                for (var sy = 0; sy < 8; sy++)
                {
                    for (var sx = 0; sx < 8; sx++)
                    {
                        if (PetalGlyphs.IsInked(glyph, (u + (sx + 0.5) / 8.0) * cell, (v + (sy + 0.5) / 8.0) * cell))
                            inked++;
                    }
                }

                values[i] = inked / 64.0;
            }

            return values;
        }

        Assert.Equal(PetalGlyphs.GlyphCount, PetalGlyphs.GlyphChars.Length);
        for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
        {
            Assert.True(Coverage(glyph).Sum() > 6.0, $"glyph {glyph}");
            foreach (var stroke in PetalGlyphs.Strokes(glyph))
            {
                Assert.True(stroke.Count >= 2);
                Assert.All(stroke, static point => Assert.True(point.X is >= 2.0 and <= 30.0 && point.Y is >= 2.0 and <= 30.0));
            }
        }

        // zero-mean cosine distance of the coarse coverage must stay well apart
        static double[] Feature(int glyph)
        {
            var values = Coverage(glyph);
            var mean = values.Sum() / values.Length;
            var centred = values.Select(v => v - mean).ToArray();
            var norm = Math.Sqrt(centred.Sum(static v => v * v));
            return centred.Select(v => v / norm).ToArray();
        }

        for (var a = 0; a < PetalGlyphs.GlyphCount; a++)
        {
            for (var b = a + 1; b < PetalGlyphs.GlyphCount; b++)
            {
                var dot = Feature(a).Zip(Feature(b)).Sum(static pair => pair.First * pair.Second);
                Assert.True(1.0 - dot > 0.2, $"glyphs {a} and {b}: {1.0 - dot}");
            }
        }
    }

    [Fact]
    public void EveryGlyphStrokeLiesInsideItsRoundedTile()
    {
        // what lets a native canvas stroke the glyphs without clipping them to the tile
        const double inset = (PetalLayout.KanaTile - PetalLayout.KanaGlyphBox) / 2.0;
        const double scale = PetalGlyphs.GlyphGrid / PetalLayout.KanaGlyphBox;
        for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
        {
            for (var i = 0; i <= 540; i++)
            {
                var u = -1.0 + i * 0.05;
                for (var j = 0; j <= 540; j++)
                {
                    var v = -1.0 + j * 0.05;
                    if (PetalGlyphs.IsInked(glyph, (u - inset) * scale, (v - inset) * scale))
                        Assert.True(PetalLayout.KanaInside(u, v), $"glyph {glyph} inked outside its tile at ({u}, {v})");
                }
            }
        }

        Assert.Equal(PetalGlyphs.StrokeWidth * PetalLayout.KanaGlyphBox / PetalGlyphs.GlyphGrid, PetalDrawList.KanaStrokeWidth);
        Assert.Equal(4.671875, PetalDrawList.KanaStrokeWidth);
        Assert.Equal(new PetalPoint(inset + 22.0 / scale, inset + 4.0 / scale), PetalDrawList.KanaGlyphStrokes(0)[0][0]);
        Assert.Equal((25.0, 3.0), (PetalDrawList.KanaTileSize, PetalDrawList.KanaCornerRadius));
    }

    [Fact]
    public void KatakanaTilesShowTheirPolarityAndGlyph()
    {
        var frame = Cells(4);
        for (var i = 0; i < frame.Kana.Length; i++)
            frame.Kana[i] = new PetalKanaSymbol((byte)(i % 16), i % 3 != 0);
        var image = PetalRenderer.Render(frame, new PetalRenderOptions { Size = 1024, Supersample = 1 });
        Assert.Equal((1024, 1668), (image.Width, image.Height));
        var palette = PetalPalette.Default;
        for (var index = 0; index < PetalLayout.KanaTiles.Count; index++)
        {
            var tile = PetalLayout.KanaTiles[index];
            var symbol = frame.Kana[index];
            // the pixel centres of the tile's square, against the definition
            var (x0, y0) = ((int)Math.Ceiling(tile.X), (int)Math.Ceiling(tile.Y + PetalLayout.SquareTop));
            foreach (var (dx, dy) in new[] { (3, 3), (12, 12), (8, 17), (20, 6) })
            {
                var (px, py) = (x0 + dx, y0 + dy);
                var (u, v) = (px + 0.5 - tile.X, py + 0.5 - PetalLayout.SquareTop - tile.Y);
                var expected = (PetalLayout.KanaInked(symbol.Glyph, u, v), symbol.Light) switch
                {
                    (true, true) => palette.Ink,
                    (false, true) => palette.Light,
                    (true, false) => palette.Pink,
                    _ => palette.Background,
                };
                var at = (py * 1024 + px) * 3;
                Assert.True(
                    expected == new PetalColor(image.Data[at], image.Data[at + 1], image.Data[at + 2]),
                    $"tile {index} at ({dx}, {dy})");
            }
        }

        // the square below the top band is the bare square
        var square = PetalTestSupport.Square(frame, 1024, 1);
        Assert.True(square.Data.AsSpan().SequenceEqual(image.Data.AsSpan(322 * 1024 * 3, 1024 * 1024 * 3)));
        // a glyph outside the alphabet is refused
        frame.Kana[7] = new PetalKanaSymbol(16, true);
        Assert.Throws<ArgumentException>(() => PetalRenderer.Render(frame, new PetalRenderOptions { Size = 64, Supersample = 1 }));
        Assert.Throws<ArgumentException>(() => PetalDrawList.Create(frame));
    }

    [Fact]
    public void SoftwareRendersAreByteIdenticalToTheReference()
    {
        // CRC-32C of the Rust reference `render` output (the whole card) for frames of a 700-byte
        // stream (turned petals and katakana tiles included)
        var encoder = new PetalStreamEncoder(PetalTestSupport.Payload(700, 2), 2);
        foreach (var (frame, size, supersample, crc) in new[]
        {
            ((ushort)0, 512, 2, 0xFBDF_7940u),
            ((ushort)1, 300, 3, 0xB52F_B2C1u),
            ((ushort)6, 777, 4, 0xCA8B_70B1u),
        })
        {
            var rgb = PetalRenderer.Render(encoder.Cells(frame), new PetalRenderOptions { Size = size, Supersample = supersample });
            Assert.Equal((size, PetalRenderer.CardHeightPx(size)), (rgb.Width, rgb.Height));
            Assert.True(crc == PetalCrc32C.Compute(rgb.Data), $"frame {frame} at {size} px, {supersample}x: {PetalCrc32C.Compute(rgb.Data):X8}");
        }

        Assert.Equal((512, 834), (512, PetalRenderer.CardHeightPx(512)));
        // the turn tests' frames: 6,000 bytes from xorshift32 seed 5 at 768 px
        var turns = new PetalStreamEncoder(PetalTestSupport.Payload(6000, 5), 2);
        var plain = new PetalFrameCells();
        turns.Cells(3).Dots.CopyTo(plain.Dots, 0);
        turns.Cells(3).Petals.CopyTo(plain.Petals, 0);
        foreach (var (cells, crc) in new[] { (turns.Cells(6), 0x913B_4A27u), (turns.Cells(9), 0x4AE3_1996u), (plain, 0xB234_98B5u) })
        {
            var rgb = PetalRenderer.Render(cells, new PetalRenderOptions { Size = 768, Supersample = 2 });
            Assert.True(crc == PetalCrc32C.Compute(rgb.Data), $"turn test frame: {PetalCrc32C.Compute(rgb.Data):X8}, expected {crc:X8}");
        }
    }

    [Fact]
    public void ACleanCardReadsBothBandsAndSkipsWhatItIsTold()
    {
        var image = CleanCard.Value;
        var truth = SessionStream.LaneData(6);
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, decoded.Error.ToString());
        Assert.Equal("D0123", decoded.Frame.Lanes);
        var header = HeaderOf(decoded.Frame);
        var kana = PetalKana.ReadKana(image, decoded.Frame, header);
        Assert.Equal(PetalKana.Lanes.Count, kana.Length);
        foreach (var lane in PetalKana.Lanes)
        {
            var result = kana[(int)lane];
            Assert.True(result is not null, $"kana lane {PetalKana.Name(lane)}");
            Assert.Equal(truth.Lane(lane), result.Result.Data);
            Assert.Equal(header, result.Header);
            Assert.Equal(0, result.Result.Corrected);
            Assert.NotNull(PetalKana.KanaPacket(result.Header, lane, result.Result.Data));
        }

        // what the reader decided: every tile of both bands, eight pilots each
        var cells = SessionStream.Cells(6);
        for (var band = 0; band < PetalLayout.KanaBands; band++)
        {
            var reading = PetalKana.ObserveBand(image, decoded.Frame, band);
            Assert.NotNull(reading);
            Assert.Equal(PetalLayout.KanaPilots, reading.Pilots);
            Assert.Contains(reading.Blur, PetalKana.KanaBlurs);
            Assert.Equal(PetalKana.KanaDataTiles, reading.Tiles.Count);
            var data = PetalLayout.KanaDataTiles(band);
            for (var i = 0; i < reading.Tiles.Count; i++)
            {
                var tile = reading.Tiles[i];
                Assert.True(tile is not null, $"band {band} tile {i}");
                Assert.Equal(data[i], tile.Value.Tile);
                Assert.Equal(cells.Kana[data[i]], new PetalKanaSymbol(tile.Value.Glyph, tile.Value.Light));
                Assert.True(tile.Value.GlyphLead > 0.0 && tile.Value.LightMargin > 0.0, $"band {band} tile {i}");
            }
        }

        // bands told to be skipped are not read
        kana = PetalKana.ReadKana(image, decoded.Frame, header, [true, false]);
        Assert.Equal([false, false, true, true], kana.Select(static k => k is not null));
        Assert.All(PetalKana.ReadKana(image, decoded.Frame, header, [true, true]), static k => Assert.Null(k));
        // a header of another stream finds no check that matches
        Assert.All(PetalKana.ReadKana(image, decoded.Frame, header with { Tag = (byte)(header.Tag + 1) }), static k => Assert.Null(k));
    }

    [Fact]
    public void ABandOfTheFrameBeforeOrAfterTheSquareStillReads()
    {
        // a rolling shutter or a stale display can show a band from a neighbouring frame: the check
        // matches the frame after or before the square's, which names the atoms
        var image = CleanCard.Value;
        var decoded = PetalDecoder.Decode(image).Frame!;
        var header = HeaderOf(decoded);
        Assert.Equal(6, header.Frame);
        foreach (var (square, shown) in new[] { ((ushort)5, (ushort)6), ((ushort)7, (ushort)6) })
        {
            var kana = PetalKana.ReadKana(image, decoded, header with { Frame = square });
            foreach (var lane in PetalKana.Lanes)
            {
                var result = kana[(int)lane];
                Assert.True(result is not null && result.Header.Frame == shown, $"square {square}: lane {PetalKana.Name(lane)}");
                Assert.Equal(PetalKana.KanaFirstId(shown, lane), PetalKana.KanaPacket(result.Header, lane, result.Result.Data)!.FirstId);
            }
        }

        // two frames away is too far
        Assert.All(PetalKana.ReadKana(image, decoded, header with { Frame = 8 }), static k => Assert.Null(k));
    }

    /// <summary>A session whose work costs what <paramref name="costs"/> says, however long it took.</summary>
    private static PetalScanSession Costed(Func<PetalWork, double> costs) => new(workCost: (work, _) => costs(work));

    /// <summary>A Pixel 6 held to its Cortex-A55 cores (the reference simulator's model).</summary>
    private static double A55(PetalWork work) => work.Kind switch
    {
        PetalWorkKind.Search => 105,
        PetalWorkKind.Track => 14.5,
        PetalWorkKind.Merge => 10,
        PetalWorkKind.Bands => 15 * work.Count,
        _ => 75,
    };

    [Fact]
    public void AnUprightCameraReadsBothBandsOncePerShownFrame()
    {
        // a phone held upright delivers landscape frames with the card turned a quarter
        var config = PetalCaptureSimulator.FitToFrame(PetalCaptureConfig.Modern() with { RotationDeg = 92.0 }, 4.0);
        PetalRgbImage Card(ushort frame) => PetalRenderer.Render(SessionStream.Cells(frame), new PetalRenderOptions { Size = 768, Supersample = 2 });
        // a fast decoder: every capture has time to spare for the turn lanes too
        var session = Costed(static _ => 0);
        var first = session.Push(PetalCaptureSimulator.Capture(Card(1), config), 0);
        Assert.Equal(("P0K0P1K1", "0123"), (first.Kana, first.Turned));
        // a second capture of the same shown frame tries no band and no turn lane again
        var again = session.Push(PetalCaptureSimulator.Capture(Card(1), config), 33);
        Assert.Equal((string.Empty, string.Empty), (again.Kana, again.Turned));
        var next = session.Push(PetalCaptureSimulator.Capture(Card(2), config), 83);
        Assert.Equal(("P0K0P1K1", "0123"), (next.Kana, next.Turned));
        Assert.Equal((4u, 8u), (session.Stats.KanaTries, session.Stats.KanaLanes));
        Assert.Equal((8u, 2u), (session.Stats.TurnLanes, session.Stats.SpareTurns));
        Assert.Equal(4, session.TriedBands.Count);
        session.Reset();
        Assert.Empty(session.TriedBands);
    }

    [Fact]
    public void ASlowDecoderLeavesTheTurnLanesToTheNextCapture()
    {
        // clean, undistorted cards pushed straight into a session on a slow phone core: the bands read
        // on the first capture of a shown frame and take its spare time, so that capture tries no turn
        // lane; the turn lanes read on the next capture of the same shown frame
        var session = Costed(A55);
        var card = CleanCard.Value;
        var first = session.Push(card, 0);
        Assert.Equal(("D0123", "P0K0P1K1", string.Empty), (first.Lanes, first.Kana, first.Turned));
        var again = session.Push(card, 33);
        Assert.Equal(("D0123", string.Empty, "0123"), (again.Lanes, again.Kana, again.Turned));
        var stats = session.Stats;
        Assert.Equal((2u, 4u, 4u, 4u), (stats.KanaTries, stats.KanaLanes, stats.TurnTries, stats.TurnLanes));
        Assert.Equal(0u, stats.SpareTurns);
        // the kana and turn atoms reached the assembler alongside the field lanes' (frame 6 has no beacon, so they wait)
        Assert.Equal(0u, session.Progress.AtomsReceived);
    }

    [Fact]
    public void TheLastTurnTryTellsWhetherACaptureHasTimeToSpare()
    {
        var config = PetalCaptureSimulator.FitToFrame(PetalCaptureConfig.Modern() with { RotationDeg = 92.0 }, 4.0);
        PetalLuma Shown(ushort frame) =>
            PetalCaptureSimulator.Capture(PetalRenderer.Render(SessionStream.Cells(frame), new PetalRenderOptions { Size = 768, Supersample = 2 }), config);
        var session = Costed(static work => work.Kind switch
        {
            PetalWorkKind.Search or PetalWorkKind.Track => 4,
            PetalWorkKind.Bands => 3 * work.Count,
            PetalWorkKind.Merge => 0,
            _ => 30,
        });
        // 10 ms of work, and twice that for the turn lanes before any try: they fit
        var first = session.Push(Shown(1), 0);
        Assert.Equal(("P0K0P1K1", "0123"), (first.Kana, first.Turned));
        // the try took 30 ms, which with the next capture's 10 ms does not fit
        var shown = Shown(2);
        var next = session.Push(shown, 83);
        Assert.Equal(("P0K0P1K1", string.Empty), (next.Kana, next.Turned));
        var again = session.Push(shown, 116);
        Assert.Equal((string.Empty, "0123"), (again.Kana, again.Turned));
        Assert.Equal(1u, session.Stats.SpareTurns);
    }

    [Fact]
    public void BandTriesThatReadNothingBackOff()
    {
        // bare squares: every field lane reads, no band is in view
        var config = PetalCaptureSimulator.FitToFrame(PetalCaptureConfig.Modern() with { Width = 640, Height = 480, RotationDeg = 12.0 }, 4.0);
        var session = new PetalScanSession();
        var triedAt = new List<int>();
        for (ushort frame = 1; frame <= 30; frame++)
        {
            var before = session.Stats.KanaTries;
            var image = PetalCaptureSimulator.Capture(PetalTestSupport.Square(SessionStream.Cells(frame), 512, 2), config with { Seed = frame });
            var outcome = session.Push(image, frame * 83L);
            Assert.True(outcome.Lanes.EndsWith("0123", StringComparison.Ordinal), $"frame {frame}: {outcome.Lanes}");
            Assert.Equal(string.Empty, outcome.Kana);
            if (session.Stats.KanaTries > before)
                triedAt.Add(frame);
        }

        // after the n-th try that reads nothing, the next min(n, 8) captures that would try are passed
        // over; each try covers both bands, and the turn lanes still run on every capture
        Assert.Equal([1, 3, 6, 10, 15, 21, 28], triedAt);
        Assert.Equal((uint)(2 * triedAt.Count), session.Stats.KanaTries);
        Assert.Equal(0u, session.Stats.KanaLanes);
        Assert.Equal(7, session.KanaMisses);
        Assert.True(session.Stats.TurnLanes > 0);
        // a try that reads a lane ends the back-off
        session.KanaWait = 0;
        var upright = PetalCaptureSimulator.FitToFrame(PetalCaptureConfig.Modern() with { RotationDeg = 92.0 }, 4.0);
        var card = PetalRenderer.Render(SessionStream.Cells(31), new PetalRenderOptions { Size = 768, Supersample = 2 });
        Assert.Equal("P0K0P1K1", session.Push(PetalCaptureSimulator.Capture(card, upright), 31 * 83L).Kana);
        Assert.Equal(0, session.KanaMisses);
    }

    [Fact]
    public void GarbageImagesAndBrokenPosesNeverThrowAndReadNothing()
    {
        var image = CleanCard.Value;
        var decoded = PetalDecoder.Decode(image).Frame!;
        var header = HeaderOf(decoded);
        var noise = new PetalLuma(image.Width, image.Height);
        new Random(7).NextBytes(noise.Data);
        foreach (var other in new[] { new PetalLuma(image.Width, image.Height), noise, new PetalLuma(3, 2), new PetalLuma(0, 0) })
        {
            Assert.All(PetalKana.ReadKana(other, decoded, header), static k => Assert.Null(k));
            Assert.Null(PetalKana.ObserveBand(other, decoded, 0));
        }

        foreach (var pose in new[]
        {
            PetalPose.Plain(new PetalHomography(Enumerable.Repeat(double.NaN, 9).ToArray())),
            PetalPose.Plain(new PetalHomography(double.PositiveInfinity, 0, 0, 0, 1, 0, 0, 0, 1)),
            PetalPose.Plain(new PetalHomography(new double[9])),
            PetalPose.Plain(new PetalHomography(1e300, 0, 0, 0, 1e300, 0, 0, 0, 1)),
        })
        {
            var broken = new PetalDecodedFrame(pose, decoded.Rotation, decoded.Mirrored, decoded.D, decoded.Fields.ToArray(), null);
            Assert.All(PetalKana.ReadKana(image, broken, header), static k => Assert.Null(k));
        }
    }

    [Fact]
    public void ArgumentsAreValidated()
    {
        var image = CleanCard.Value;
        var decoded = PetalDecoder.Decode(image).Frame!;
        Assert.Throws<ArgumentException>(() => PetalKana.ReadKana(image, decoded, default, [false, false, false]));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.ObserveBand(image, decoded, 2));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.Lane(2, glyph: false));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.Name((PetalKanaLane)4));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.Whitening((PetalKanaLane)(-1)));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.TemplateProbe(5, 0, true, 0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.TemplateProbe(0, 16, true, 0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.TemplateProbe(0, 0, true, 12, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.CoarseTemplateProbe(0, false, 0, 6));
        Assert.Throws<ArgumentException>(() => PetalKana.KanaData(default, PetalKanaLane.K0, new byte[16]));
        Assert.Throws<ArgumentException>(() => PetalKana.EncodeKanaLane(PetalKanaLane.P0, new byte[19]));
        Assert.False(PetalKana.TryDecodeKanaLane(PetalKanaLane.P0, new byte[36], [], out _, out _));
        Assert.Null(PetalKana.KanaPacket(default, PetalKanaLane.P0, new byte[19]));
        Assert.Throws<ArgumentException>(() => PetalKana.SetKanaSymbols(new PetalKanaSymbol[611], 0, new byte[37], new byte[149]));
        Assert.Throws<ArgumentException>(() => PetalKana.SetKanaSymbols(PetalKana.PilotSymbols(), 0, new byte[36], new byte[149]));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.SetKanaSymbols(PetalKana.PilotSymbols(), 2, new byte[37], new byte[149]));
        Assert.Throws<ArgumentException>(() => PetalKana.PolarityWord(new PetalKanaSymbol[10], 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalKana.GlyphWord(PetalKana.PilotSymbols(), -1));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalGlyphs.IsInked(16, 0.0, 0.0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalGlyphs.Strokes(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLayout.KanaInked(16, 0.0, 0.0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLayout.KanaPilotCell(2, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLayout.KanaPilotCell(0, 8));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLayout.KanaDataTiles(2));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLayout.KanaPilotTiles(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalDrawList.KanaGlyphStrokes(16));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalRenderer.CardHeightPx(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => SessionStream.LaneData(0).Lane((PetalKanaLane)4));
    }
}
