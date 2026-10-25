using System.Text.Json;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Conformance against the shared golden vectors in
/// <c>fixtures/petal/petal_stream_v2.json</c> (every section).
/// </summary>
public sealed class PetalFixtureTests
{
    private static JsonElement Doc => PetalTestSupport.Stream;

    [Fact]
    public void ConstantsMatch()
    {
        Assert.Equal(2, PetalTestSupport.Number(Doc, "fixture_version"));
        Assert.Equal("petal-stream-v2", Doc.GetProperty("format").GetString());
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

        Assert.Equal(PetalPalette.Default, new PetalPalette(Colour("background"), Colour("light"), Colour("pink"), Colour("backdrop")));
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
            Assert.Equal(PetalTestSupport.Hex(whitening, $"F{sector}"), PetalLanes.Whitening(PetalLanes.Field(sector)));
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
                var dWord = PetalTestSupport.Hex(frame, "d_word");
                var fieldWords = PetalTestSupport.HexList(frame, "field_words");
                Assert.Equal(dWord, PetalLanes.EncodeLane(PetalLane.D, lanes.D));
                for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                    Assert.Equal(fieldWords[sector], PetalLanes.EncodeLane(PetalLanes.Field(sector), lanes.Fields[sector]));
                Assert.Equal(new PetalLaneData(dWord, fieldWords), encoder.Words(number));
                var cells = PetalFrameCells.FromWords(dWord, fieldWords);
                Assert.Equal(
                    PetalTestSupport.Numbers(frame, "lit_dots"),
                    Enumerable.Range(0, cells.Dots.Length).Where(i => cells.Dots[i]).Select(static i => (long)i));
                Assert.Equal(PetalTestSupport.Flags(PetalTestSupport.Hex(frame, "lit_petals_hex")), cells.Petals);
                Assert.Equal(cells, encoder.Cells(number));
                Assert.Equal(dWord, cells.DWord());
                for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                    Assert.Equal(fieldWords[sector], cells.FieldWord(sector));
            }

            // push every fixture frame through lane decoding and the assembler
            if (name == "one-pass")
            {
                var assembler = new PetalStreamAssembler();
                foreach (var frame in stream.GetProperty("frames").EnumerateArray())
                {
                    Assert.True(PetalLanes.TryDecodeLane(PetalLane.D, PetalTestSupport.Hex(frame, "d_word"), [], out var d));
                    assembler.PushDLane(PetalStream.ParseDLane(d)!);
                    var fieldWords = PetalTestSupport.HexList(frame, "field_words");
                    for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                    {
                        var lane = PetalLanes.Field(sector);
                        Assert.True(PetalLanes.TryDecodeLane(lane, fieldWords[sector], [], out var data));
                        assembler.PushAtoms(PetalStream.ParseAtomLane(lane, data)!);
                    }
                }

                Assert.Equal(payload, assembler.TakeCompleted()!.ToArray());
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
            var result = PetalDecoder.Decode(PetalTestSupport.RenderLuma(encoder, number, 512, 2));
            Assert.True(result.Success, result.Error.ToString());
            PetalTestSupport.AssertAllLanes(
                result.Frame,
                new PetalLaneData(PetalTestSupport.Hex(frame, "d_data"), PetalTestSupport.HexList(frame, "field_data")),
                stream.GetProperty("name").GetString()!);
        }
    }
}
