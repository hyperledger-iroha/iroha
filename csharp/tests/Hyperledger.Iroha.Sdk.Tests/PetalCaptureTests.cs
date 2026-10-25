using System.Diagnostics;
using System.Text.Json;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Decodes the golden camera captures in <c>fixtures/petal/petal_captures_v2.json</c>:
/// every conforming decoder must read the lanes named in <c>must_decode</c>, must never
/// report wrong data for any lane, must report the recorded inferred corner, must follow
/// each tracking pair from its first frame into its second, must recover the
/// <c>must_merge</c> field lanes of every merge set from the merge of its captures, must
/// never let a session offer a lane of a mixed set, and must reject the negatives. A
/// bit-exact port reproduces every recorded reference result.
/// </summary>
public sealed class PetalCaptureTests(ITestOutputHelper output)
{
    /// <summary>The twelve captures of the fixture; the shadow band needs the local read, two need an inferred corner and the last a distortion search.</summary>
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
        "hidden-corner-540p",
        "cut-corner-720p",
        "barrel-480p",
    ];

    /// <summary>The two tracking pairs of the fixture.</summary>
    private static readonly string[] TrackNames = ["steady-hand-540p", "thumb-arrives-540p"];

    private static JsonElement Doc => PetalTestSupport.Captures;

    [Fact]
    public void GoldenCapturesDecodeAsRecorded()
    {
        Assert.Equal(2, PetalTestSupport.Number(Doc, "fixture_version"));
        Assert.Equal("petal-captures-v2", Doc.GetProperty("format").GetString());
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
            var read = PetalTestSupport.ReadLanes(name, decoded, PetalTestSupport.Truth(capture));
            var must = capture.GetProperty("must_decode").GetString()!;
            Assert.True(must.All(read.Contains), $"{name}: read {read}, must read {must}");
            // a bit-exact port reaches exactly the reference decoder's lanes and inferred corner
            Assert.Equal(capture.GetProperty("reference_decoded").GetString(), read);
            Assert.True(InferredCorner(capture) == decoded.InferredCorner, $"{name}: inferred corner {decoded.InferredCorner}");
            output.WriteLine(
                $"{name,-24} {image.Width}x{image.Height} lanes {read,-5} (reference {capture.GetProperty("reference_decoded").GetString()}) " +
                $"rotation {decoded.Rotation} mirrored {decoded.Mirrored} inferred {decoded.InferredCorner?.ToString() ?? "-"} " +
                $"distortion {decoded.Pose.Distortion} first decode {watch.Elapsed.TotalMilliseconds:F1} ms");
            decoded.Feed(assembler);
        }

        // captures of different frames of the same stream accumulate in one assembler
        Assert.True(assembler.Progress.AtomsReceived > 100);
    }

    [Fact]
    public void GoldenTracksFollowThePoseIntoTheNextFrame()
    {
        var tracks = Doc.GetProperty("tracks").EnumerateArray().ToArray();
        Assert.Equal(TrackNames, tracks.Select(static track => track.GetProperty("name").GetString()));
        foreach (var track in tracks)
        {
            var name = track.GetProperty("name").GetString()!;
            var from = PetalTestSupport.LumaOf(track, "from_luma_zlib_base64");
            var to = PetalTestSupport.LumaOf(track, "to_luma_zlib_base64");
            var first = PetalDecoder.Decode(from);
            Assert.True(first.Success, $"{name}: first frame {first.Error}");
            var watch = Stopwatch.StartNew();
            var followed = PetalDecoder.Track(to, first.Frame);
            watch.Stop();
            Assert.True(followed is not null, $"{name}: tracking lost the code");
            var read = PetalTestSupport.ReadLanes(name, followed, PetalTestSupport.Truth(track));
            var must = track.GetProperty("must_track").GetString()!;
            Assert.True(must.All(read.Contains), $"{name}: tracked {read}, must track {must}");
            Assert.Equal(track.GetProperty("reference_tracked").GetString(), read);
            Assert.True(InferredCorner(track) == followed.InferredCorner, $"{name}: inferred corner {followed.InferredCorner}");
            // the orientation is kept from the first frame
            Assert.Equal((first.Frame.Rotation, first.Frame.Mirrored), (followed.Rotation, followed.Mirrored));
            output.WriteLine(
                $"{name,-24} tracked lanes {read,-5} (reference {track.GetProperty("reference_tracked").GetString()}) " +
                $"inferred {followed.InferredCorner?.ToString() ?? "-"} track {watch.Elapsed.TotalMilliseconds:F1} ms");
        }
    }

    [Fact]
    public void GoldenMergeSetsRecoverLanesNoCaptureReadsAlone()
    {
        var sets = Doc.GetProperty("merges").EnumerateArray().ToArray();
        Assert.NotEmpty(sets);
        foreach (var set in sets)
        {
            var name = set.GetProperty("name").GetString()!;
            var (results, sessionMerged, stats) = MergeSet(set);
            var merged = MergedLetters(name, results, set);
            Assert.Equal(set.GetProperty("reference_merged").GetString(), merged);
            foreach (var letter in set.GetProperty("must_merge").GetString()!)
                Assert.True(sessionMerged.Contains(letter), $"{name}: the session did not recover lane {letter} (merged {sessionMerged})");
            Assert.True(stats.Merges > 0, $"{name}: no merge");
            Assert.Equal((uint)sessionMerged.Length, stats.MergedLanes);
            output.WriteLine($"{name,-24} merged {merged} (reference {set.GetProperty("reference_merged").GetString()}) session {sessionMerged}");
        }
    }

    [Fact]
    public void GoldenMixedSetsNeverOfferTheOtherFramesLanes()
    {
        var sets = Doc.GetProperty("mixed_merges").EnumerateArray().ToArray();
        Assert.NotEmpty(sets);
        foreach (var set in sets)
        {
            var name = set.GetProperty("name").GetString()!;
            var (results, sessionMerged, stats) = MergeSet(set);
            // the merge itself reads the petal field, which shows another frame than the rings
            var merged = MergedLetters(name, results, set);
            var ringsFrame = PetalTestSupport.Hex(set, "d_data")[1..3];
            foreach (var result in results)
            {
                if (result is not null)
                    Assert.False(result.Data.AsSpan(1, 2).SequenceEqual(ringsFrame), $"{name}: a merged lane names the rings' frame");
            }

            Assert.Equal(set.GetProperty("reference_merged").GetString(), merged);
            Assert.True(stats.Merges > 0, $"{name}: no merge was tried");
            Assert.True(sessionMerged.Length == 0, $"{name}: the session offered another frame's lanes ({sessionMerged})");
            Assert.Equal(0u, stats.MergedLanes);
        }
    }

    [Fact]
    public void TheHiddenAndCutCornersAreInferred()
    {
        foreach (var (name, corner) in new[] { ("hidden-corner-540p", 3), ("cut-corner-720p", 2) })
        {
            var capture = Doc.GetProperty("captures").EnumerateArray().Single(entry => entry.GetProperty("name").GetString() == name);
            var image = PetalTestSupport.LumaOf(capture);
            // the locator offers three blossoms with the fourth corner inferred, and that set reads
            var candidates = PetalLocator.LocateCandidates(image);
            Assert.Contains(candidates, static set => set.Inferred is not null);
            var decoded = PetalDecoder.Decode(image);
            Assert.True(decoded.Success, $"{name}: {decoded.Error}");
            Assert.Equal(corner, decoded.Frame.InferredCorner);
            // diagnostics and evidence extrapolate the levels of the inferred corner like the decoder
            Assert.NotNull(PetalDecoder.ObservedCells(image, decoded.Frame));
            Assert.NotNull(PetalDecoder.FieldEvidence(image, decoded.Frame));
        }
    }

    [Fact]
    public void TheShadowBandIsReadThroughTheLocalRead()
    {
        var capture = Doc.GetProperty("captures").EnumerateArray().Single(static entry => entry.GetProperty("name").GetString() == "shadow-band-540p");
        var image = PetalTestSupport.LumaOf(capture);
        var decoded = PetalDecoder.Decode(image);
        Assert.True(decoded.Success, decoded.Error.ToString());
        var evidence = PetalDecoder.FieldEvidence(image, decoded.Frame);
        Assert.NotNull(evidence);
        // judged against the finder levels alone, some field lane does not survive the shadow
        var levelRead = PetalDecoder.ReadFieldLanes(evidence.Levels, local: false);
        Assert.Contains(levelRead, static lane => lane is null);
        // rescaled by their neighbourhoods, the cells read every field lane the decoder read
        var localRead = PetalDecoder.ReadFieldLanes(evidence.Levels, local: true);
        Assert.Equal(decoded.Frame.Fields, localRead);
        // a lane the level read did decode is kept exactly as the level read left it
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            if (levelRead[sector] is { } kept)
                Assert.Equal(kept, localRead[sector]);
        }
    }

    [Fact]
    public void NegativeCapturesAreRejected()
    {
        var negatives = Doc.GetProperty("negatives").EnumerateArray().ToArray();
        Assert.Equal(2, negatives.Length);
        foreach (var negative in negatives)
            Assert.False(PetalDecoder.Decode(PetalTestSupport.LumaOf(negative)).Success, negative.GetProperty("name").GetString());
    }

    [Fact]
    public void RecordedLaneDataIsAValidCodewordOfItsStream()
    {
        var payload = PetalTestSupport.Hex(Doc, "payload_hex");
        var encoder = new PetalStreamEncoder(payload, (byte)PetalTestSupport.Number(Doc, "payload_kind"));
        foreach (var capture in Doc.GetProperty("captures").EnumerateArray())
        {
            foreach (var (lane, data) in PetalTestSupport.Truth(capture))
            {
                var word = PetalLanes.EncodeLane(lane, data);
                Assert.True(PetalLanes.TryDecodeLane(lane, word, [], out var decoded));
                Assert.Equal(data, decoded);
            }

            // the captures film the `one-pass` stream of the payload
            var truth = PetalTestSupport.Truth(capture);
            var lanes = encoder.LaneData((ushort)PetalTestSupport.Number(capture, "frame"));
            foreach (var (lane, data) in truth)
                Assert.Equal(lanes.Lane(lane), data);
        }
    }

    [Fact]
    public void ConcurrentDecodesAgreeWithSequentialOnes()
    {
        // the decoder keeps its scratch buffers per thread: decodes on several threads at once
        // must read exactly what one thread reads
        var images = Doc.GetProperty("captures").EnumerateArray().Select(static capture => PetalTestSupport.LumaOf(capture)).ToArray();
        var expected = images.Select(static image => Summary(PetalDecoder.Decode(image).Frame!)).ToArray();
        var actual = new string[3 * images.Length];
        Parallel.For(0, actual.Length, new ParallelOptions { MaxDegreeOfParallelism = 4 }, i =>
            actual[i] = Summary(PetalDecoder.Decode(images[i % images.Length]).Frame!));
        for (var i = 0; i < actual.Length; i++)
            Assert.Equal(expected[i % images.Length], actual[i]);

        static string Summary(PetalDecodedFrame frame) =>
            $"{frame.Lanes} {frame.Pose} {string.Join(",", frame.Fields.Select(static f => f is null ? "-" : Convert.ToHexString(f.Data)))}";
    }

    [Fact]
    public void GoldenCaptureDecodeTiming()
    {
        const int runs = 7;
        foreach (var capture in Doc.GetProperty("captures").EnumerateArray())
        {
            var name = capture.GetProperty("name").GetString()!;
            var image = PetalTestSupport.LumaOf(capture);
            PetalDecoder.Decode(image); // warm-up (JIT, tables)
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

    /// <summary>
    /// Decodes every capture of a merge set as recorded and pushes it through a fresh session
    /// (30 fps); returns the merge of their field evidence, the lanes the session merged and its
    /// counters.
    /// </summary>
    private static (PetalLaneResult?[] Results, string SessionMerged, PetalScanStats Stats) MergeSet(JsonElement set)
    {
        var name = set.GetProperty("name").GetString()!;
        var (width, height) = ((int)PetalTestSupport.Number(set, "width"), (int)PetalTestSupport.Number(set, "height"));
        var truth = PetalTestSupport.Truth(set);
        var evidence = new List<PetalFieldEvidence>();
        var session = new PetalScanSession();
        var merged = string.Empty;
        var captures = set.GetProperty("captures").EnumerateArray().ToArray();
        for (var i = 0; i < captures.Length; i++)
        {
            var capture = captures[i];
            var image = PetalTestSupport.LumaOf(capture, width, height);
            var decoded = PetalDecoder.Decode(image);
            Assert.True(decoded.Success, $"{name} capture {i}: {decoded.Error}");
            var read = PetalTestSupport.ReadLanes($"{name} capture {i}", decoded.Frame, truth);
            var must = capture.GetProperty("must_decode").GetString()!;
            Assert.True(must.All(read.Contains), $"{name} capture {i}: read {read}, must read {must}");
            Assert.Equal(capture.GetProperty("reference_decoded").GetString(), read);
            evidence.Add(PetalDecoder.FieldEvidence(image, decoded.Frame) ?? throw new InvalidOperationException("field evidence"));
            merged += session.Push(image, 33L * i).Merged;
        }

        return (PetalDecoder.MergeFields(evidence), merged, session.Stats);
    }

    /// <summary>Checks merged field lanes against the recorded data and returns their sector digits.</summary>
    private static string MergedLetters(string name, PetalLaneResult?[] results, JsonElement set)
    {
        var fields = PetalTestSupport.HexList(set, "field_data");
        var letters = string.Empty;
        for (var sector = 0; sector < results.Length; sector++)
        {
            if (results[sector] is not { } result)
                continue;
            Assert.True(fields[sector].AsSpan().SequenceEqual(result.Data), $"{name}: merged lane {sector} data");
            letters += PetalLanes.Letter(PetalLanes.Field(sector));
        }

        return letters;
    }

    private static int? InferredCorner(JsonElement entry) =>
        entry.GetProperty("inferred_corner").ValueKind == JsonValueKind.Null
            ? null
            : entry.GetProperty("inferred_corner").GetInt32();
}
