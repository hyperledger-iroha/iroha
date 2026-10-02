using System.Diagnostics;
using System.Text.Json;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Decodes the golden camera captures in <c>fixtures/petal/petal_captures_v1.json</c>:
/// every conforming decoder must read the lanes named in <c>must_decode</c>,
/// must never report wrong data for any lane, and must reject the negatives.
/// </summary>
public sealed class PetalCaptureTests(ITestOutputHelper output)
{
    /// <summary>The nine captures of the fixture; the last three need the normalised tile read.</summary>
    private static readonly string[] CaptureNames =
    [
        "clean-512",
        "modern-720p-rotated",
        "legacy-540p-tilted",
        "soft-480p-blur1.9",
        "small-480p",
        "selfie-mirrored-540p",
        "overexposed-540p",
        "veiled-720p",
        "shadow-band-540p",
    ];

    private static JsonElement Doc => PetalTestSupport.Captures;

    [Fact]
    public void GoldenCapturesDecodeAsRecorded()
    {
        var captures = Doc.GetProperty("captures").EnumerateArray().ToArray();
        Assert.Equal(CaptureNames, captures.Select(static capture => capture.GetProperty("name").GetString()));
        var assembler = new PetalStreamAssembler();
        foreach (var capture in captures)
        {
            var name = capture.GetProperty("name").GetString()!;
            var image = PetalTestSupport.LumaOf(capture);
            var watch = Stopwatch.StartNew();
            var result = PetalDecoder.Decode(image);
            watch.Stop();
            Assert.True(result.Success, $"{name}: {result.Error}");
            var decoded = result.Frame;
            Assert.Equal(capture.GetProperty("mirrored").GetBoolean(), decoded.Mirrored);
            var must = capture.GetProperty("must_decode").GetString()!;
            foreach (var (letter, lane, key) in new[] { ('P', decoded.P, "p_data"), ('K', decoded.K, "k_data"), ('D', decoded.D, "d_data") })
            {
                if (lane is not null)
                    Assert.True(PetalTestSupport.Hex(capture, key).AsSpan().SequenceEqual(lane.Data), $"{name}: lane {letter} data");
                else
                    Assert.False(must.Contains(letter), $"{name}: required lane {letter} was not decoded");
            }

            // a bit-exact port reaches exactly the reference decoder's lanes
            Assert.Equal(capture.GetProperty("reference_decoded").GetString(), decoded.Lanes);
            output.WriteLine(
                $"{name,-24} {image.Width}x{image.Height} lanes {decoded.Lanes,-3} (reference {capture.GetProperty("reference_decoded").GetString()}) " +
                $"rotation {decoded.Rotation} mirrored {decoded.Mirrored} first decode {watch.Elapsed.TotalMilliseconds:F1} ms");
            decoded.Feed(assembler);
        }

        // captures of different frames of the same stream accumulate in one assembler
        Assert.True(assembler.Progress.AtomsReceived > 10);
    }

    [Theory]
    [InlineData("overexposed-540p")]
    [InlineData("veiled-720p")]
    [InlineData("shadow-band-540p")]
    public void TheNormalisedReadRecoversLaneKWhereTheLevelReadLosesIt(string name)
    {
        var capture = Doc.GetProperty("captures").EnumerateArray().Single(entry => entry.GetProperty("name").GetString() == name);
        Assert.Contains('K', capture.GetProperty("must_decode").GetString()!);
        var image = PetalTestSupport.LumaOf(capture);
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, $"{name}: {decoded.Error}");
        var sigmas = PetalDecodeOptions.Default.TemplateSigmas;
        var pose = decoded.Frame.Homography;
        var reference = PetalDecoder.ReferenceLevels(image, pose);
        Assert.NotNull(reference);
        var patches = new double[PetalLayout.TileCount * PetalGlyphs.TemplateSize * PetalGlyphs.TemplateSize];
        PetalDecoder.SamplePatches(image, pose, patches);

        // judged against the finder levels, lane K does not survive these cameras
        var (pWord, pConfidence, kWord, kConfidence) = PetalDecoder.TileWords(
            PetalDecoder.ReadTiles(patches, reference.Value, sigmas));
        Assert.Null(PetalDecoder.DecodeWithErasures(PetalLane.K, kWord, kConfidence));

        // judged by its own contrast, every tile is readable again
        (_, _, kWord, kConfidence) = PetalDecoder.TileWords(PetalDecoder.ReadTilesNormalised(patches, sigmas));
        var k = PetalDecoder.DecodeWithErasures(PetalLane.K, kWord, kConfidence);
        Assert.NotNull(k);
        Assert.Equal(PetalTestSupport.Hex(capture, "k_data"), k.Data);
        Assert.Equal(k, decoded.Frame.K);

        // a lane the level read did decode is kept exactly as the level read left it
        if (PetalDecoder.DecodeWithErasures(PetalLane.P, pWord, pConfidence) is { } levelP)
            Assert.Equal(levelP, decoded.Frame.P);
        else
            Assert.Equal(PetalTestSupport.Hex(capture, "p_data"), decoded.Frame.P?.Data);
    }

    [Fact]
    public void NegativeCapturesAreRejected()
    {
        foreach (var negative in Doc.GetProperty("negatives").EnumerateArray())
            Assert.False(PetalDecoder.Decode(PetalTestSupport.LumaOf(negative)).Success, negative.GetProperty("name").GetString());
    }

    [Fact]
    public void RecordedLaneDataMatchesTheOnePassStream()
    {
        var payload = PetalTestSupport.Hex(Doc, "payload_hex");
        var encoder = new PetalStreamEncoder(payload, (byte)PetalTestSupport.Number(Doc, "payload_kind"));
        foreach (var capture in Doc.GetProperty("captures").EnumerateArray())
        {
            var data = PetalTestSupport.Hex(capture, "p_data");
            var word = PetalLanes.EncodeLane(PetalLane.P, data);
            Assert.True(PetalLanes.TryDecodeLane(PetalLane.P, word, [], out var decoded));
            Assert.Equal(data, decoded);
            var (p, k, d) = encoder.LaneData((ushort)PetalTestSupport.Number(capture, "frame"));
            Assert.Equal(p, data);
            Assert.Equal(k, PetalTestSupport.Hex(capture, "k_data"));
            Assert.Equal(d, PetalTestSupport.Hex(capture, "d_data"));
        }
    }

    [Fact]
    public void GoldenCaptureDecodeTiming()
    {
        const int runs = 7;
        foreach (var capture in Doc.GetProperty("captures").EnumerateArray())
        {
            var name = capture.GetProperty("name").GetString()!;
            var image = PetalTestSupport.LumaOf(capture);
            PetalDecoder.Decode(image); // warm-up (JIT, pattern cache)
            var times = new double[runs];
            for (var run = 0; run < runs; run++)
            {
                var watch = Stopwatch.StartNew();
                var result = PetalDecoder.Decode(image);
                times[run] = watch.Elapsed.TotalMilliseconds;
                Assert.True(result.Success);
            }

            Array.Sort(times);
            output.WriteLine($"timing {name,-24} {image.Width}x{image.Height} median {times[runs / 2]:F2} ms min {times[0]:F2} ms");
        }
    }
}
