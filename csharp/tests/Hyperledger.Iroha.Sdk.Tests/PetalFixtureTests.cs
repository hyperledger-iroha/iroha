using System.Text.Json;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Conformance against the shared golden vectors in
/// <c>fixtures/petal/petal_stream_v5.json</c> (every section).
/// </summary>
public sealed class PetalFixtureTests
{
    private static JsonElement Doc => PetalTestSupport.Stream;

    [Fact]
    public void ConstantsMatch()
    {
        Assert.Equal(5, PetalTestSupport.Number(Doc, "fixture_version"));
        Assert.Equal("petal-stream-v5", Doc.GetProperty("format").GetString());
        var constants = Doc.GetProperty("constants");
        Assert.Equal(PetalLayout.Canvas, constants.GetProperty("canvas").GetDouble());
        Assert.Equal(PetalLanes.AtomLength, PetalTestSupport.Number(constants, "atom_len"));
        Assert.Equal(PetalLanes.DWordLength, PetalTestSupport.Number(constants, "d_word"));
        Assert.Equal(PetalLanes.DParityLength, PetalTestSupport.Number(constants, "d_parity"));
        Assert.Equal(PetalLanes.FieldWordLength, PetalTestSupport.Number(constants, "field_word"));
        Assert.Equal(PetalLanes.FieldParityLength, PetalTestSupport.Number(constants, "field_parity"));
        Assert.Equal(PetalLanes.FieldAtoms, PetalTestSupport.Number(constants, "field_atoms"));
        Assert.Equal(PetalLayout.Sectors, PetalTestSupport.Number(constants, "sectors"));
        Assert.Equal(PetalLanes.AtomsPerFrame, PetalTestSupport.Number(constants, "atoms_per_frame"));
        Assert.Equal(PetalLayout.FieldCells, PetalTestSupport.Number(constants, "field_cells"));
        Assert.Equal(PetalStream.BeaconInterval, PetalTestSupport.Number(constants, "beacon_interval"));
        Assert.Equal(PetalStream.FormatVersion, PetalTestSupport.Number(constants, "format_version"));
        Assert.Equal(PetalTurns.TurnPositions, PetalTestSupport.Number(constants, "turn_positions"));
        Assert.Equal(PetalTurns.TurnCarriers, PetalTestSupport.Number(constants, "turn_carriers"));
        Assert.Equal(PetalTurns.TurnWordLength, PetalTestSupport.Number(constants, "turn_word"));
        Assert.Equal(PetalTurns.TurnParityLength, PetalTestSupport.Number(constants, "turn_parity"));
        Assert.Equal(PetalTurns.TurnAtoms, PetalTestSupport.Number(constants, "turn_atoms"));
        Assert.Equal(PetalTurns.TurnIdBase, PetalTestSupport.Number(constants, "turn_id_base"));
        foreach (var (key, value) in new[]
        {
            ("card_height", PetalLayout.CardHeight),
            ("square_top", PetalLayout.SquareTop),
            ("kana_tile", PetalLayout.KanaTile),
            ("kana_pitch", PetalLayout.KanaPitch),
            ("kana_glyph_box", PetalLayout.KanaGlyphBox),
            ("kana_corner", PetalLayout.KanaCorner),
            ("kana_gap", PetalLayout.KanaGap),
            ("kana_margin", PetalLayout.KanaMargin),
            ("kana_finder_clearance", PetalLayout.KanaFinderClearance),
        })
        {
            Assert.True(Math.Abs(constants.GetProperty(key).GetDouble() - value) < 1e-9, key);
        }

        foreach (var (key, value) in new[]
        {
            ("kana_rows", PetalLayout.KanaRows),
            ("kana_columns", PetalLayout.KanaColumns),
            ("kana_bands", PetalLayout.KanaBands),
            ("kana_pilots", PetalLayout.KanaPilots),
            ("kana_data_tiles", PetalKana.KanaDataTiles),
            ("kana_glyph_word", PetalKana.KanaGlyphWordLength),
            ("kana_glyph_parity", PetalKana.KanaGlyphParityLength),
            ("kana_glyph_atoms", PetalKana.KanaGlyphAtoms),
            ("kana_polarity_word", PetalKana.KanaPolarityWordLength),
            ("kana_polarity_parity", PetalKana.KanaPolarityParityLength),
            ("kana_polarity_atoms", PetalKana.KanaPolarityAtoms),
        })
        {
            Assert.True(value == PetalTestSupport.Number(constants, key), key);
        }

        Assert.Equal(PetalKana.KanaIdBase, PetalTestSupport.Number(constants, "kana_id_base"));
    }

    [Fact]
    public void LayoutMatches()
    {
        var layout = Doc.GetProperty("layout");
        Assert.Equal(
            PetalLayout.FinderCenters.SelectMany(static p => new[] { p.X, p.Y }),
            layout.GetProperty("finder_centers").EnumerateArray().Select(static v => v.GetDouble()));
        Assert.Equal(PetalLayout.BandInner, layout.GetProperty("band_inner").GetDouble());
        Assert.Equal(PetalLayout.BandSpacing, layout.GetProperty("band_spacing").GetDouble());
        Assert.Equal(PetalLayout.BandSlots.Select(static n => (long)n), PetalTestSupport.Numbers(layout, "band_slots"));
        // the petal dimensions are single precision in the reference, so they compare as floats
        Assert.Equal(PetalLayout.PetalRadius, (float)layout.GetProperty("petal_radius").GetDouble());
        Assert.Equal(PetalLayout.PetalHalfLength, (float)layout.GetProperty("petal_half_length").GetDouble());
        Assert.Equal(PetalLayout.PetalHalfWidth, (float)layout.GetProperty("petal_half_width").GetDouble());
        Assert.Equal(PetalLayout.PetalNotchRadius, (float)layout.GetProperty("petal_notch_radius").GetDouble());
        Assert.Equal(PetalLayout.PetalNotchDistance, (float)layout.GetProperty("petal_notch_distance").GetDouble());
        var sectors = layout.GetProperty("sector_cells").EnumerateArray().ToArray();
        Assert.Equal(PetalLayout.Sectors, sectors.Length);
        for (var sector = 0; sector < sectors.Length; sector++)
        {
            Assert.Equal(
                PetalLayout.SectorCellOrder(sector).Select(static c => (long)c),
                sectors[sector].EnumerateArray().Select(static v => v.GetInt64()));
        }

        var orders = layout.GetProperty("carrier_orders").EnumerateArray().ToArray();
        Assert.Equal(PetalLayout.Sectors, orders.Length);
        for (var sector = 0; sector < orders.Length; sector++)
        {
            Assert.Equal(
                PetalTurns.CarrierOrder(sector).Select(static c => (long)c),
                orders[sector].EnumerateArray().Select(static v => v.GetInt64()));
        }

        Assert.Equal(
            layout.GetProperty("mask").EnumerateArray().Select(static row => row.GetString()!),
            PetalLayout.Mask);
        Assert.Equal(PetalLayout.MaskOrigin, layout.GetProperty("mask_origin").GetDouble());
        Assert.Equal(PetalLayout.MaskPitch, layout.GetProperty("mask_pitch").GetDouble());
        Assert.Equal(PetalLayout.BackdropRadius, layout.GetProperty("backdrop_radius").GetDouble());
        Assert.Equal(
            PetalTestSupport.Flags(PetalTestSupport.Hex(layout, "light_cells_hex")),
            Enumerable.Range(0, PetalLayout.FieldCells).Select(PetalLayout.CellLight));
        Assert.Equal(PetalLayout.RingRadii, layout.GetProperty("ring_radii").EnumerateArray().Select(static v => v.GetDouble()));
        Assert.Equal(PetalLayout.RingSlots.Select(static n => (long)n), PetalTestSupport.Numbers(layout, "ring_slots"));
        Assert.Equal(PetalLayout.DotRadius, layout.GetProperty("dot_radius").GetDouble());
        Assert.Equal(PetalLayout.DataSlots().Select(static s => (long)s), PetalTestSupport.Numbers(layout, "data_slots"));
        Assert.Equal(PetalLayout.GateSlots.Select(static s => (long)s), PetalTestSupport.Numbers(layout, "gate_slots"));
        Assert.Equal(PetalLayout.GuardSlots.Select(static s => (long)s), PetalTestSupport.Numbers(layout, "guard_slots"));
        // every katakana tile as [band, row, column, x, y, pilot], −1 for a data tile
        var tiles = layout.GetProperty("kana_tiles").EnumerateArray().ToArray();
        Assert.Equal(PetalLayout.KanaTiles.Count, tiles.Length);
        for (var index = 0; index < tiles.Length; index++)
        {
            var row = tiles[index].EnumerateArray().ToArray();
            var tile = PetalLayout.KanaTiles[index];
            Assert.Equal((tile.Band, tile.Row, tile.Column), (row[0].GetInt32(), row[1].GetInt32(), row[2].GetInt32()));
            Assert.True(tile.X == row[3].GetDouble() && tile.Y == row[4].GetDouble(), $"tile {index}: ({tile.X}, {tile.Y})");
            Assert.Equal(tile.Pilot ?? -1, row[5].GetInt32());
        }

        var pilots = layout.GetProperty("kana_pilot_symbols").EnumerateArray().ToArray();
        Assert.Equal(PetalLayout.KanaPilots, pilots.Length);
        for (var pilot = 0; pilot < pilots.Length; pilot++)
        {
            var row = pilots[pilot].EnumerateArray().Select(static v => v.GetInt32()).ToArray();
            Assert.Equal(new PetalKanaSymbol((byte)row[0], row[1] == 1), PetalLayout.KanaPilotSymbols[pilot]);
        }

        var glyphs = Doc.GetProperty("glyphs");
        Assert.Equal(PetalGlyphs.GlyphChars, glyphs.GetProperty("chars").GetString());
        Assert.Equal(PetalGlyphs.GlyphGrid, glyphs.GetProperty("grid").GetDouble());
        Assert.Equal(PetalGlyphs.StrokeWidth, glyphs.GetProperty("stroke_width").GetDouble());
        var strokes = glyphs.GetProperty("strokes").EnumerateArray().ToArray();
        Assert.Equal(PetalGlyphs.GlyphCount, strokes.Length);
        for (var glyph = 0; glyph < strokes.Length; glyph++)
        {
            var expected = PetalGlyphs.Strokes(glyph);
            var lines = strokes[glyph].EnumerateArray().ToArray();
            Assert.Equal(expected.Count, lines.Length);
            for (var stroke = 0; stroke < lines.Length; stroke++)
            {
                var points = lines[stroke].EnumerateArray()
                    .Select(static point => new PetalPoint(point[0].GetDouble(), point[1].GetDouble()))
                    .ToArray();
                Assert.Equal(expected[stroke], points);
            }
        }
    }

    [Fact]
    public void PaletteMatches()
    {
        var palette = Doc.GetProperty("palette");
        PetalColor Colour(string key)
        {
            var rgb = PetalTestSupport.Numbers(palette, key);
            return new PetalColor((byte)rgb[0], (byte)rgb[1], (byte)rgb[2]);
        }

        Assert.Equal(PetalPalette.Default, new PetalPalette(Colour("background"), Colour("light"), Colour("pink"), Colour("backdrop"), Colour("ink")));
    }

    [Fact]
    public void ChecksumsPrngAndWhiteningMatch()
    {
        foreach (var testCase in Doc.GetProperty("crc32c").EnumerateArray())
        {
            Assert.Equal(
                PetalTestSupport.Number(testCase, "crc32c"),
                PetalCrc32C.Compute(PetalTestSupport.Hex(testCase, "input_hex")));
        }

        var prng = Doc.GetProperty("prng");
        var rng = new PetalXorshift32(1);
        var expected = PetalTestSupport.Numbers(prng, "xorshift32_seed1");
        Assert.Equal(6, expected.Length);
        foreach (var value in expected)
            Assert.Equal(value, rng.NextUInt32());
        foreach (var testCase in prng.GetProperty("mix32").EnumerateArray())
        {
            Assert.Equal(
                PetalTestSupport.Number(testCase, "out"),
                PetalFountain.Mix32((uint)PetalTestSupport.Number(testCase, "in")));
        }

        var whitening = Doc.GetProperty("whitening");
        Assert.Equal(PetalTestSupport.Hex(whitening, "D"), PetalLanes.Whitening(PetalLane.D));
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            Assert.Equal(PetalTestSupport.Hex(whitening, $"F{sector}"), PetalLanes.Whitening(PetalLanes.Field(sector)));
            Assert.Equal(PetalTestSupport.Hex(whitening, $"T{sector}"), PetalTurns.TurnWhitening(sector));
        }

        foreach (var lane in PetalKana.Lanes)
            Assert.Equal(PetalTestSupport.Hex(whitening, PetalKana.Name(lane)), PetalKana.Whitening(lane));
    }

    [Fact]
    public void KanaIdsAndReaderTemplatesMatch()
    {
        var ids = Doc.GetProperty("kana_first_ids");
        Assert.Equal(PetalKana.Lanes.Select(PetalKana.Name), ids.GetProperty("lanes").EnumerateArray().Select(static v => v.GetString()));
        var frames = PetalTestSupport.Numbers(ids, "frames");
        var rows = ids.GetProperty("ids").EnumerateArray().ToArray();
        Assert.Equal(frames.Length, rows.Length);
        for (var i = 0; i < frames.Length; i++)
        {
            Assert.Equal(
                rows[i].EnumerateArray().Select(static v => v.GetInt64()),
                PetalKana.Lanes.Select(lane => (long)PetalKana.KanaFirstId((ushort)frames[i], lane)));
        }

        var reader = Doc.GetProperty("kana_reader");
        Assert.Equal(PetalKana.KanaBlurs, reader.GetProperty("blurs").EnumerateArray().Select(static v => v.GetDouble()));
        Assert.Equal(PetalKana.KanaGrid, PetalTestSupport.Number(reader, "grid"));
        var probes = reader.GetProperty("template_probes").EnumerateArray().ToArray();
        Assert.True(probes.Length >= PetalKana.KanaBlurs.Count, $"{probes.Length} probes");
        foreach (var probe in probes)
        {
            var blur = PetalKana.KanaBlurs.ToList().FindIndex(b => Math.Abs(b - probe.GetProperty("blur").GetDouble()) < 1e-12);
            Assert.True(blur >= 0, $"unknown blur {probe.GetProperty("blur").GetDouble()}");
            var got = PetalKana.TemplateProbe(
                blur,
                (int)PetalTestSupport.Number(probe, "glyph"),
                PetalTestSupport.Number(probe, "light") == 1,
                (int)PetalTestSupport.Number(probe, "i"),
                (int)PetalTestSupport.Number(probe, "j"));
            var expected = probe.GetProperty("value").GetDouble();
            Assert.True(Math.Abs(got - expected) < 1e-12, $"probe {probe}: {got:R}, expected {expected:R}");
        }

        Assert.Equal(PetalKana.KanaCoarseGrid, PetalTestSupport.Number(reader, "coarse_grid"));
        var coarse = reader.GetProperty("coarse_probes").EnumerateArray().ToArray();
        Assert.NotEmpty(coarse);
        foreach (var probe in coarse)
        {
            var got = PetalKana.CoarseTemplateProbe(
                (int)PetalTestSupport.Number(probe, "glyph"),
                PetalTestSupport.Number(probe, "light") == 1,
                (int)PetalTestSupport.Number(probe, "i"),
                (int)PetalTestSupport.Number(probe, "j"));
            var expected = probe.GetProperty("value").GetDouble();
            Assert.True(Math.Abs(got - expected) < 1e-12, $"coarse probe {probe}: {got:R}, expected {expected:R}");
        }
    }

    [Fact]
    public void TurnIdsAndReaderTemplatesMatch()
    {
        var ids = Doc.GetProperty("turn_first_ids");
        var frames = PetalTestSupport.Numbers(ids, "frames");
        var sectors = PetalTestSupport.Numbers(ids, "sectors");
        var expected = PetalTestSupport.Numbers(ids, "ids");
        Assert.Equal(frames.Length, sectors.Length);
        Assert.Equal(frames.Length, expected.Length);
        for (var i = 0; i < frames.Length; i++)
            Assert.Equal(expected[i], PetalTurns.TurnFirstId((ushort)frames[i], (int)sectors[i]));
        var reader = Doc.GetProperty("turn_reader");
        Assert.Equal(PetalTurns.TurnBlurs, reader.GetProperty("blurs").EnumerateArray().Select(static v => v.GetDouble()));
        var probes = reader.GetProperty("template_probes").EnumerateArray().ToArray();
        Assert.Equal(6 * PetalTurns.TurnBlurs.Count, probes.Length);
        foreach (var probe in probes)
        {
            double Value(string key) => probe.GetProperty(key).GetDouble();
            var blur = PetalTurns.TurnBlurs.ToList().FindIndex(b => Math.Abs(b - Value("blur")) < 1e-12);
            Assert.True(blur >= 0, $"unknown blur {Value("blur")}");
            var got = PetalTurns.TurnTemplate(blur, Value("along"), Value("across"));
            Assert.True(
                Math.Abs(got - Value("value")) < 1e-12,
                $"blur {Value("blur")} at ({Value("along")}, {Value("across")}): {got:R}, expected {Value("value"):R}");
        }
    }

    [Fact]
    public void ReedSolomonVectorsEncodeAndCorrect()
    {
        foreach (var testCase in Doc.GetProperty("reed_solomon").EnumerateArray())
        {
            var nsym = (int)PetalTestSupport.Number(testCase, "nsym");
            var data = PetalTestSupport.Hex(testCase, "data_hex");
            var word = PetalTestSupport.Hex(testCase, "codeword_hex");
            var rs = new PetalReedSolomon(nsym);
            Assert.Equal(word, rs.Encode(data));
            // damage up to the correction capacity and recover
            var damaged = (byte[])word.Clone();
            for (var i = 0; i < nsym / 2; i++)
                damaged[i * 3 % word.Length] ^= 0x5A;
            Assert.True(rs.TryDecode(damaged, [], out _), $"nsym {nsym}");
            Assert.Equal(word, damaged);
        }
    }

    [Fact]
    public void FountainMasksAndAtomIdsMatch()
    {
        foreach (var testCase in Doc.GetProperty("fountain_masks").EnumerateArray())
        {
            var mask = PetalFountain.MaskWords(
                (int)PetalTestSupport.Number(testCase, "k"),
                (uint)PetalTestSupport.Number(testCase, "crc"),
                (uint)PetalTestSupport.Number(testCase, "id"));
            Assert.Equal(PetalTestSupport.Numbers(testCase, "mask"), mask.Select(static w => (long)w));
        }

        var ids = Doc.GetProperty("first_atom_ids");
        var frames = PetalTestSupport.Numbers(ids, "frames");
        var expected = PetalTestSupport.Numbers(ids, "ids");
        Assert.Equal(frames.Length, expected.Length);
        for (var i = 0; i < frames.Length; i++)
            Assert.Equal(expected[i], PetalStream.FirstAtomId((ushort)frames[i]));
    }

    [Fact]
    public void StreamsEncodeIdenticallyAndReassemble()
    {
        var streams = Doc.GetProperty("streams").EnumerateArray().ToArray();
        Assert.Equal(["tiny", "one-pass", "wrap"], streams.Select(static stream => stream.GetProperty("name").GetString()));
        foreach (var stream in streams)
        {
            var name = stream.GetProperty("name").GetString()!;
            var payload = PetalTestSupport.Hex(stream, "payload_hex");
            var encoder = new PetalStreamEncoder(payload, (byte)PetalTestSupport.Number(stream, "kind"));
            var meta = encoder.Meta;
            Assert.Equal(PetalTestSupport.Number(stream, "len"), meta.Length);
            Assert.Equal(PetalTestSupport.Number(stream, "crc32c"), meta.Crc);
            Assert.Equal(PetalTestSupport.Number(stream, "tag"), meta.Tag);
            Assert.Equal(PetalTestSupport.Number(stream, "source_atoms"), meta.SourceAtoms);
            Assert.Equal(PetalTestSupport.Number(stream, "systematic_frames"), encoder.SystematicFrames);
            foreach (var frame in stream.GetProperty("frames").EnumerateArray())
            {
                var number = (ushort)PetalTestSupport.Number(frame, "frame");
                var lanes = encoder.LaneData(number);
                Assert.True(PetalTestSupport.Hex(frame, "d_data").AsSpan().SequenceEqual(lanes.D), $"{name} frame {number} d");
                var fieldData = PetalTestSupport.HexList(frame, "field_data");
                Assert.Equal(PetalLayout.Sectors, fieldData.Length);
                for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                    Assert.True(fieldData[sector].AsSpan().SequenceEqual(lanes.Fields[sector]), $"{name} frame {number} field {sector}");
                var turnData = PetalTestSupport.HexList(frame, "turn_data");
                Assert.Equal(PetalLayout.Sectors, turnData.Length);
                for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                    Assert.True(turnData[sector].AsSpan().SequenceEqual(lanes.Turns[sector]), $"{name} frame {number} turn {sector}");
                var kanaData = PetalTestSupport.HexList(frame, "kana_data");
                var kanaWords = PetalTestSupport.HexList(frame, "kana_words");
                Assert.Equal(PetalKana.Lanes.Count, kanaData.Length);
                var kanaHeader = new PetalLaneHeader(meta.Tag, number);
                foreach (var lane in PetalKana.Lanes)
                {
                    var laneName = $"{name} frame {number} {PetalKana.Name(lane)}";
                    Assert.True(kanaData[(int)lane].AsSpan().SequenceEqual(lanes.Lane(lane)), laneName);
                    Assert.Equal(kanaWords[(int)lane], PetalKana.EncodeKanaLane(lane, kanaData[(int)lane]));
                    var packet = PetalKana.KanaPacket(kanaHeader, lane, kanaData[(int)lane]);
                    Assert.True(packet is not null, $"{laneName} check");
                    Assert.Equal(PetalKana.KanaFirstId(number, lane), packet.FirstId);
                    Assert.Equal(PetalKana.Atoms(lane), packet.Atoms.Count);
                }

                Assert.Equal(PetalTestSupport.LaneData(frame), lanes);
                var dWord = PetalTestSupport.Hex(frame, "d_word");
                var fieldWords = PetalTestSupport.HexList(frame, "field_words");
                var turnWords = PetalTestSupport.HexList(frame, "turn_words");
                for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                {
                    Assert.Equal(turnWords[sector], PetalTurns.EncodeTurnLane(sector, turnData[sector]));
                    var packet = PetalTurns.TurnPacket(new PetalLaneHeader(meta.Tag, number), sector, turnData[sector]);
                    Assert.True(packet is not null, $"{name} frame {number} turn check {sector}");
                    Assert.Equal(PetalTurns.TurnFirstId(number, sector), packet.FirstId);
                }

                Assert.Equal(dWord, PetalLanes.EncodeLane(PetalLane.D, lanes.D));
                for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                    Assert.Equal(fieldWords[sector], PetalLanes.EncodeLane(PetalLanes.Field(sector), lanes.Fields[sector]));
                Assert.Equal(new PetalLaneData(dWord, fieldWords, turnWords, kanaWords), encoder.Words(number));
                var cells = PetalFrameCells.FromWords(dWord, fieldWords);
                Assert.All(cells.Turns, static turn => Assert.Equal(0, turn));
                Assert.Equal(PetalKana.PilotSymbols(), cells.Kana);
                cells.SetTurns(turnWords);
                Assert.Equal(PetalTestSupport.Text(frame, "turns"), string.Concat(cells.Turns.Select(static t => (char)('0' + t))));
                cells.SetKana(kanaWords);
                Assert.Equal(PetalTestSupport.Text(frame, "kana_glyphs"), string.Concat(cells.Kana.Select(static k => "0123456789abcdef"[k.Glyph])));
                var lightBits = PetalTestSupport.Hex(frame, "kana_light_hex");
                Assert.Equal(
                    Enumerable.Range(0, PetalLayout.KanaTileCount).Select(i => ((lightBits[i / 8] >> (7 - i % 8)) & 1) == 1),
                    cells.Kana.Select(static k => k.Light));
                for (var band = 0; band < PetalLayout.KanaBands; band++)
                {
                    Assert.Equal(kanaWords[(int)PetalKana.Lane(band, false)], cells.KanaPolarityWord(band));
                    Assert.Equal(kanaWords[(int)PetalKana.Lane(band, true)], cells.KanaGlyphWord(band));
                }
                Assert.Equal(
                    PetalTestSupport.Numbers(frame, "lit_dots"),
                    Enumerable.Range(0, cells.Dots.Length).Where(i => cells.Dots[i]).Select(static i => (long)i));
                Assert.Equal(PetalTestSupport.Flags(PetalTestSupport.Hex(frame, "lit_petals_hex")), cells.Petals);
                Assert.Equal(cells, encoder.Cells(number));
                Assert.Equal(cells, PetalFrameCells.FromWords(new PetalLaneData(dWord, fieldWords, turnWords, kanaWords)));
                Assert.Equal(dWord, cells.DWord());
                for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                {
                    Assert.Equal(fieldWords[sector], cells.FieldWord(sector));
                    Assert.Equal(turnWords[sector], cells.TurnWord(sector));
                }
            }

            // push every fixture frame through lane decoding and the assembler, without the turn and
            // kana lanes, with the turn lanes, and with both: each layer finishes the one-pass stream
            // in fewer frames
            if (name == "one-pass")
            {
                var needed = new int[3];
                for (var layers = 0; layers < 3; layers++)
                {
                    var (withTurns, withKana) = (layers >= 1, layers >= 2);
                    var assembler = new PetalStreamAssembler();
                    foreach (var frame in stream.GetProperty("frames").EnumerateArray())
                    {
                        needed[layers]++;
                        Assert.True(PetalLanes.TryDecodeLane(PetalLane.D, PetalTestSupport.Hex(frame, "d_word"), [], out var d));
                        assembler.PushDLane(PetalStream.ParseDLane(d)!);
                        var fieldWords = PetalTestSupport.HexList(frame, "field_words");
                        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                        {
                            var lane = PetalLanes.Field(sector);
                            Assert.True(PetalLanes.TryDecodeLane(lane, fieldWords[sector], [], out var data));
                            assembler.PushAtoms(PetalStream.ParseAtomLane(lane, data)!);
                        }

                        if (withTurns)
                        {
                            var header = new PetalLaneHeader(meta.Tag, (ushort)PetalTestSupport.Number(frame, "frame"));
                            var turnWords = PetalTestSupport.HexList(frame, "turn_words");
                            for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                            {
                                Assert.True(PetalTurns.TryDecodeTurnLane(sector, turnWords[sector], [], out var data, out _));
                                assembler.PushAtoms(PetalTurns.TurnPacket(header, sector, data)!);
                            }
                        }

                        if (withKana)
                        {
                            var header = new PetalLaneHeader(meta.Tag, (ushort)PetalTestSupport.Number(frame, "frame"));
                            var kanaWords = PetalTestSupport.HexList(frame, "kana_words");
                            foreach (var lane in PetalKana.Lanes)
                            {
                                Assert.True(PetalKana.TryDecodeKanaLane(lane, kanaWords[(int)lane], [], out var data, out _));
                                assembler.PushAtoms(PetalKana.KanaPacket(header, lane, data)!);
                            }
                        }

                        if (assembler.Progress.Complete)
                            break;
                    }

                    Assert.Equal(payload, assembler.TakeCompleted()!.ToArray());
                }

                Assert.True(
                    needed[2] < needed[1] && needed[1] < needed[0],
                    $"frames needed without the turn and kana lanes, with the turn lanes and with both: {string.Join(", ", needed)}");
            }
        }
    }

    [Fact]
    public void FixtureFramesSurviveRenderingAndDecoding()
    {
        foreach (var stream in Doc.GetProperty("streams").EnumerateArray())
        {
            var payload = PetalTestSupport.Hex(stream, "payload_hex");
            var encoder = new PetalStreamEncoder(payload, (byte)PetalTestSupport.Number(stream, "kind"));
            var frame = stream.GetProperty("frames").EnumerateArray().Last();
            var number = (ushort)PetalTestSupport.Number(frame, "frame");
            var name = stream.GetProperty("name").GetString()!;
            var result = PetalDecoder.Decode(PetalTestSupport.RenderLuma(encoder, number, 512, 2));
            Assert.True(result.Success, result.Error.ToString());
            PetalTestSupport.AssertAllLanes(result.Frame, PetalTestSupport.LaneData(frame), name);
            // at 768 px the render resolves the petals' shapes and the glyphs: the turn and kana lanes read too
            var image = PetalTestSupport.RenderLuma(encoder, number, 768, 2);
            result = PetalDecoder.Decode(image);
            Assert.True(result.Success, result.Error.ToString());
            PetalTestSupport.AssertAllLanes(result.Frame, PetalTestSupport.LaneData(frame), name);
            PetalTestSupport.AssertAllTurnLanes(image, result.Frame, PetalTestSupport.LaneData(frame), name);
            var kana = PetalKana.ReadKana(image, result.Frame, new PetalLaneHeader(encoder.Meta.Tag, number));
            var truth = PetalTestSupport.LaneData(frame);
            foreach (var lane in PetalKana.Lanes)
            {
                var read = kana[(int)lane];
                Assert.True(read is not null && truth.Lane(lane).AsSpan().SequenceEqual(read.Result.Data), $"{name}: kana lane {PetalKana.Name(lane)}");
                Assert.Equal(number, read.Header.Frame);
            }
        }
    }
}
