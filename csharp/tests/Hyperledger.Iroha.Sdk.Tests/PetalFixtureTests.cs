using System.Text.Json;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Conformance against the shared golden vectors in
/// <c>fixtures/petal/petal_stream_v1.json</c> (every section).
/// </summary>
public sealed class PetalFixtureTests
{
    private static JsonElement Doc => PetalTestSupport.Stream;

    [Fact]
    public void ConstantsMatch()
    {
        Assert.Equal(1, PetalTestSupport.Number(Doc, "fixture_version"));
        Assert.Equal("petal-stream-v1", Doc.GetProperty("format").GetString());
        var constants = Doc.GetProperty("constants");
        Assert.Equal(PetalLayout.Canvas, constants.GetProperty("canvas").GetDouble());
        Assert.Equal(PetalLayout.TileOrigin, constants.GetProperty("tile_origin").GetDouble());
        Assert.Equal(PetalLayout.TilePitch, constants.GetProperty("tile_pitch").GetDouble());
        Assert.Equal(PetalLayout.TileSize, constants.GetProperty("tile_size").GetDouble());
        Assert.Equal(PetalLayout.DotRadius, constants.GetProperty("dot_radius").GetDouble());
        Assert.Equal(PetalLanes.AtomLength, PetalTestSupport.Number(constants, "atom_len"));
        Assert.Equal(PetalLanes.PWordLength, PetalTestSupport.Number(constants, "p_word"));
        Assert.Equal(PetalLanes.KWordLength, PetalTestSupport.Number(constants, "k_word"));
        Assert.Equal(PetalLanes.DWordLength, PetalTestSupport.Number(constants, "d_word"));
        Assert.Equal(PetalLanes.PParityLength, PetalTestSupport.Number(constants, "p_parity"));
        Assert.Equal(PetalLanes.KParityLength, PetalTestSupport.Number(constants, "k_parity"));
        Assert.Equal(PetalLanes.DParityLength, PetalTestSupport.Number(constants, "d_parity"));
        Assert.Equal(PetalStream.BeaconInterval, PetalTestSupport.Number(constants, "beacon_interval"));
        Assert.Equal(PetalStream.FormatVersion, PetalTestSupport.Number(constants, "format_version"));
    }

    [Fact]
    public void LayoutMatches()
    {
        var layout = Doc.GetProperty("layout");
        Assert.Equal(
            layout.GetProperty("mask").EnumerateArray().Select(static row => row.GetString()!),
            PetalLayout.Mask);
        var expectedTiles = Enumerable.Range(0, PetalLayout.TileCount)
            .SelectMany(static i =>
            {
                var (column, row) = PetalLayout.Tile(i);
                return new long[] { column, row };
            });
        Assert.Equal(expectedTiles, PetalTestSupport.Numbers(layout, "tiles_col_row"));
        Assert.Equal(PetalLayout.RingSlots.Select(static n => (long)n), PetalTestSupport.Numbers(layout, "ring_slots"));
        Assert.Equal(PetalLayout.RingRadii, layout.GetProperty("ring_radii").EnumerateArray().Select(static v => v.GetDouble()));
        Assert.Equal(
            PetalLayout.FinderCenters.SelectMany(static p => new[] { p.X, p.Y }),
            layout.GetProperty("finder_centers").EnumerateArray().Select(static v => v.GetDouble()));
        Assert.Equal(PetalLayout.DataSlots().Select(static s => (long)s), PetalTestSupport.Numbers(layout, "data_slots"));
        Assert.Equal(PetalLayout.GateSlots.Select(static s => (long)s), PetalTestSupport.Numbers(layout, "gate_slots"));
        Assert.Equal(PetalLayout.GuardSlots.Select(static s => (long)s), PetalTestSupport.Numbers(layout, "guard_slots"));
    }

    [Fact]
    public void GlyphAlphabetStrokesAndTemplatesMatch()
    {
        var glyphs = Doc.GetProperty("glyphs");
        Assert.Equal(PetalGlyphs.Characters, glyphs.GetProperty("chars").GetString());
        Assert.Equal(PetalGlyphs.StrokeWidth, glyphs.GetProperty("stroke_width").GetDouble(), 9);
        var strokes = glyphs.GetProperty("strokes").EnumerateArray().ToArray();
        Assert.Equal(PetalGlyphs.GlyphCount, strokes.Length);
        for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
        {
            var polylines = strokes[glyph].EnumerateArray().ToArray();
            var ours = PetalGlyphs.Strokes(glyph);
            Assert.Equal(polylines.Length, ours.Count);
            for (var s = 0; s < polylines.Length; s++)
            {
                Assert.Equal(
                    polylines[s].EnumerateArray().Select(static v => v.GetDouble()),
                    ours[s].SelectMany(static p => new[] { p.X, p.Y }));
            }
        }

        var templates = glyphs.GetProperty("templates").EnumerateArray().ToArray();
        Assert.Equal(PetalGlyphs.GlyphCount, templates.Length);
        for (var glyph = 0; glyph < templates.Length; glyph++)
        {
            Assert.Equal(
                templates[glyph].EnumerateArray().Select(static v => (byte)v.GetInt32()),
                PetalGlyphs.Template(glyph).ToArray());
        }
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
        Assert.Equal(PetalTestSupport.Hex(whitening, "P"), PetalLanes.Whitening(PetalLane.P));
        Assert.Equal(PetalTestSupport.Hex(whitening, "K"), PetalLanes.Whitening(PetalLane.K));
        Assert.Equal(PetalTestSupport.Hex(whitening, "D"), PetalLanes.Whitening(PetalLane.D));
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
        foreach (var stream in Doc.GetProperty("streams").EnumerateArray())
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
                var (p, k, d) = encoder.LaneData(number);
                Assert.True(PetalTestSupport.Hex(frame, "p_data").AsSpan().SequenceEqual(p), $"{name} frame {number} p");
                Assert.True(PetalTestSupport.Hex(frame, "k_data").AsSpan().SequenceEqual(k), $"{name} frame {number} k");
                Assert.True(PetalTestSupport.Hex(frame, "d_data").AsSpan().SequenceEqual(d), $"{name} frame {number} d");
                var (pw, kw, dw) = (PetalTestSupport.Hex(frame, "p_word"), PetalTestSupport.Hex(frame, "k_word"), PetalTestSupport.Hex(frame, "d_word"));
                Assert.Equal(pw, PetalLanes.EncodeLane(PetalLane.P, p));
                Assert.Equal(kw, PetalLanes.EncodeLane(PetalLane.K, k));
                Assert.Equal(dw, PetalLanes.EncodeLane(PetalLane.D, d));
                var words = encoder.Words(number);
                Assert.Equal(pw, words.P);
                Assert.Equal(kw, words.K);
                Assert.Equal(dw, words.D);
                var cells = PetalFrameCells.FromWords(pw, kw, dw);
                Assert.Equal(frame.GetProperty("glyphs").GetString(), string.Concat(cells.Glyph.Select(static g => g.ToString("x"))));
                Assert.Equal(
                    PetalTestSupport.Numbers(frame, "lit_dots"),
                    Enumerable.Range(0, cells.Dots.Length).Where(i => cells.Dots[i]).Select(static i => (long)i));
                Assert.Equal(cells, encoder.Cells(number));
            }

            // push every fixture frame through lane decoding and the assembler
            if (name == "one-pass")
            {
                var assembler = new PetalStreamAssembler();
                foreach (var frame in stream.GetProperty("frames").EnumerateArray())
                {
                    Assert.True(PetalLanes.TryDecodeLane(PetalLane.D, PetalTestSupport.Hex(frame, "d_word"), [], out var d));
                    assembler.PushDLane(PetalStream.ParseDLane(d)!);
                    foreach (var (lane, key) in new[] { (PetalLane.P, "p_word"), (PetalLane.K, "k_word") })
                    {
                        Assert.True(PetalLanes.TryDecodeLane(lane, PetalTestSupport.Hex(frame, key), [], out var data));
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
            Assert.True(result.Success);
            Assert.Equal(PetalTestSupport.Hex(frame, "p_data"), result.Frame.P?.Data);
            Assert.Equal(PetalTestSupport.Hex(frame, "k_data"), result.Frame.K?.Data);
            Assert.Equal(PetalTestSupport.Hex(frame, "d_data"), result.Frame.D?.Data);
        }
    }
}
