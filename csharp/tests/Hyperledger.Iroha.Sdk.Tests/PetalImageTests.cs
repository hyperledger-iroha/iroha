using System.Buffers.Binary;
using System.IO.Compression;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Luma buffers, homographies and poses, PNG output, the renderer, the draw list and finder location.</summary>
public sealed class PetalImageTests
{
    [Fact]
    public void BilinearSamplingInterpolatesBetweenPixelCentres()
    {
        var image = new PetalLuma(2, 1, [0, 100]);
        Assert.Equal(0.0, image.Sample(0.5, 0.5), 9);
        Assert.Equal(100.0, image.Sample(1.5, 0.5), 9);
        Assert.Equal(50.0, image.Sample(1.0, 0.5), 9);
        Assert.Equal(0.0, image.Sample(-5.0, 9.0), 9);
        Assert.True(double.IsNaN(image.Sample(double.NaN, 0.5)));
        Assert.Equal(100.0, image.Sample(double.PositiveInfinity, double.NegativeInfinity), 9);
        Assert.Equal(0.0, new PetalLuma(0, 0).Sample(1.0, 1.0));
    }

    [Fact]
    public void StridedPlanesDropThePadding()
    {
        byte[] plane = [1, 2, 9, 9, 3, 4, 9, 9];
        Assert.Equal(new byte[] { 1, 2, 3, 4 }, PetalLuma.FromYPlane(plane, 2, 2, 4).Data);
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalLuma.FromYPlane(plane, 2, 2, 1));
        Assert.Throws<ArgumentException>(() => PetalLuma.FromYPlane(plane, 2, 3, 4));
        // interleaved planes (pixel stride 2), e.g. a YUYV-packed or semi-planar view
        byte[] packed = [10, 0, 20, 0, 77, 30, 0, 40, 0, 77];
        Assert.Equal(new byte[] { 10, 20, 30, 40 }, PetalLuma.FromYPlane(packed, 2, 2, 5, 2).Data);
        var reused = new PetalLuma(2, 2);
        reused.LoadYPlane(plane, 4);
        Assert.Equal(new byte[] { 1, 2, 3, 4 }, reused.Data);
        Assert.Throws<ArgumentException>(() => new PetalLuma(2, 2, new byte[3]));
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalLuma(-1, 2));
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalLuma(100_000, 100_000));
        Assert.Throws<ArgumentOutOfRangeException>(() => reused.At(2, 0));
        Assert.Equal(4, reused.At(1, 1));
    }

    [Fact]
    public void ColourLumaUsesRec601Weights()
    {
        Assert.Equal(new byte[] { 76, 150, 29 }, PetalLuma.FromRgb([255, 0, 0, 0, 255, 0, 0, 0, 255], 3, 1).Data);
        Assert.Equal(new byte[] { 76, 150, 29 }, PetalLuma.FromRgba([255, 0, 0, 1, 0, 255, 0, 2, 0, 0, 255, 3], 3, 1).Data);
        Assert.Equal(new byte[] { 76, 150, 29 }, PetalLuma.FromBgra([0, 0, 255, 1, 0, 255, 0, 2, 255, 0, 0, 3], 3, 1).Data);
        // padded rows
        Assert.Equal(new byte[] { 76, 29 }, PetalLuma.FromRgb([255, 0, 0, 9, 0, 0, 255, 9], 1, 2, 4).Data);
        var image = new PetalRgbImage(1, 1, [255, 0, 0]);
        Assert.Equal(new byte[] { 255, 0, 0, 255 }, image.ToRgba());
        Assert.Equal(new byte[] { 0, 0, 255, 7 }, image.ToBgra(7));
        Assert.Equal(new byte[] { 76 }, image.ToLuma().Data);
    }

    [Fact]
    public void FourPointsAreMappedExactly()
    {
        PetalPoint[] source = [new(0.0, 0.0), new(1024.0, 0.0), new(1024.0, 1024.0), new(0.0, 1024.0)];
        PetalPoint[] destination = [new(103.5, 40.25), new(590.0, 70.0), new(560.0, 420.0), new(80.0, 380.0)];
        var h = PetalHomography.FromPoints(source, destination);
        Assert.NotNull(h);
        for (var i = 0; i < 4; i++)
        {
            var mapped = h.Value.Apply(source[i].X, source[i].Y);
            Assert.True(Math.Abs(mapped.X - destination[i].X) < 1e-7 && Math.Abs(mapped.Y - destination[i].Y) < 1e-7);
        }

        Assert.Equal(1.0, h.Value[8]);
    }

    [Fact]
    public void InverseRoundtripsAndLeastSquaresAveragesNoise()
    {
        var truth = new PetalHomography(0.4, -0.1, 130.0, 0.12, 0.38, 60.0, 1e-4, -2e-5, 1.0);
        var source = Enumerable.Range(0, 30).Select(static i => new PetalPoint(50.0 + 31.0 * (i % 6), 90.0 + 47.0 * (i / 6))).ToArray();
        var destination = source.Select((p, i) =>
        {
            var mapped = truth.Apply(p.X, p.Y);
            var jitter = i % 2 == 0 ? 0.05 : -0.05;
            return new PetalPoint(mapped.X + jitter, mapped.Y - jitter);
        }).ToArray();
        var fit = PetalHomography.FromPoints(source, destination)!.Value;
        foreach (var p in source)
        {
            var (a, b) = (truth.Apply(p.X, p.Y), fit.Apply(p.X, p.Y));
            Assert.True(Math.Abs(a.X - b.X) < 0.1 && Math.Abs(a.Y - b.Y) < 0.1);
        }

        var inverse = truth.Inverse()!.Value;
        var forward = truth.Apply(300.0, 200.0);
        var back = inverse.Apply(forward.X, forward.Y);
        Assert.True(Math.Abs(back.X - 300.0) < 1e-6 && Math.Abs(back.Y - 200.0) < 1e-6);
        var composed = truth.Compose(inverse);
        var identity = composed.Apply(17.0, 23.0);
        Assert.True(Math.Abs(identity.X - 17.0) < 1e-9 && Math.Abs(identity.Y - 23.0) < 1e-9);
        Assert.Equal(truth, new PetalHomography(truth.ToArray()));
    }

    [Fact]
    public void DegenerateHomographyInputsAreRejected()
    {
        var p = Enumerable.Repeat(new PetalPoint(1.0, 1.0), 4).ToArray();
        Assert.Null(PetalHomography.FromPoints(p, p));
        Assert.Null(PetalHomography.FromPoints(p[..3], p[..3]));
        Assert.Null(PetalHomography.FromPoints(p, p[..3]));
        Assert.Null(new PetalHomography(1, 2, 3, 2, 4, 6, 0, 0, 0).Inverse());
        Assert.Throws<ArgumentException>(() => new PetalHomography(new double[8]));
    }

    [Fact]
    public void AFittedPoseTakesItsSourcePointsOntoDistortedImagePoints()
    {
        PetalPoint[] source = [new(72.0, 72.0), new(952.0, 72.0), new(952.0, 952.0), new(72.0, 952.0)];
        PetalPoint[] destination = [new(380.0, 90.0), new(900.0, 110.0), new(880.0, 640.0), new(360.0, 620.0)];
        foreach (var distortion in new[] { 0.0, -0.08, 0.03 })
        {
            var pose = PetalPose.Fit(source, destination, distortion, 1280, 720);
            Assert.NotNull(pose);
            Assert.Equal(distortion, pose.Value.Distortion);
            Assert.Equal(new PetalPoint(640.0, 360.0), pose.Value.Center);
            Assert.Equal(1024.0 * 1024.0, pose.Value.Scale2);
            for (var i = 0; i < 4; i++)
            {
                var mapped = pose.Value.Apply(source[i].X, source[i].Y);
                Assert.True(Math.Abs(mapped.X - destination[i].X) < 1e-3 && Math.Abs(mapped.Y - destination[i].Y) < 1e-3, $"{distortion}");
            }
        }

        // barrel distortion pulls a mid-edge point toward the image centre
        var plain = PetalPose.Fit(source, destination, 0.0, 1280, 720)!.Value;
        var barrel = PetalPose.Fit(source, destination, -0.08, 1280, 720)!.Value;
        Assert.True(Math.Abs(plain.Apply(512.0, 72.0).X - barrel.Apply(512.0, 72.0).X) < 5.0);
        Assert.Equal(new PetalPoint(3.0, 4.0), PetalPose.Plain(PetalHomography.Identity).Apply(3.0, 4.0));
        // without distortion the homography is fitted to c + (p − c), exactly as the reference does
        PetalPoint[] centred = [.. destination.Select(static p => new PetalPoint(640.0 + (p.X - 640.0), 360.0 + (p.Y - 360.0)))];
        Assert.Equal(PetalHomography.FromPoints(source, centred), plain.Homography);
        Assert.Null(PetalPose.Fit(source, source[..3], 0.0, 100, 100));
    }

    [Fact]
    public void PngChecksumsMatchKnownValues()
    {
        Assert.Equal(0xCBF4_3926u, ~PetalPng.Crc32("123456789"u8));
        Assert.Equal(0x11E6_0398u, PetalPng.Adler32("Wikipedia"u8));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void PngFilesHaveValidChunksAndPixels(bool compress)
    {
        var rgb = new byte[3 * 300 * 250];
        new Random(5).NextBytes(rgb);
        foreach (var (channels, pixels, width, height) in new[]
        {
            (1, new byte[] { 0, 64, 128, 255 }, 2, 2),
            (3, rgb, 300, 250),
            (4, new byte[] { 1, 2, 3, 4 }, 1, 1),
        })
        {
            var png = PetalPng.Encode(width, height, channels, pixels, compress);
            Assert.Equal(new byte[] { 0x89, (byte)'P', (byte)'N', (byte)'G', 0x0D, 0x0A, 0x1A, 0x0A }, png[..8]);
            Assert.Equal("IHDR"u8.ToArray(), png[12..16]);
            Assert.Equal(new byte[] { 0xAE, 0x42, 0x60, 0x82 }, png[^4..]);
            Assert.Equal(pixels, DecodePng(png, out var w, out var h, out var c));
            Assert.Equal((width, height, channels), (w, h, c));
        }

        Assert.Throws<ArgumentOutOfRangeException>(() => PetalPng.Encode(1, 1, 2, new byte[2]));
        Assert.Throws<ArgumentException>(() => PetalPng.Encode(2, 2, 1, new byte[3]));
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalPng.Encode(0, 2, 1, []));
    }

    [Fact]
    public void FindersAreSolidBlossomsAndCornersAreOtherwiseBlack()
    {
        var image = PetalRenderer.Render(Cells(1), new PetalRenderOptions { Size = 256, Supersample = 2 });
        byte At(int x, int y) => image.Data[(y * 256 + x) * 3];
        const double scale = 256.0 / 1024.0;
        var (fx, fy) = (72.0 * scale, 72.0 * scale);
        Assert.True(At((int)fx, (int)fy) > 200, "core must be lit");
        Assert.True(At((int)fx, (int)(fy - 34.0 * scale)) > 200, "upper petal must be lit");
        Assert.True(At((int)(fx + 20.0 * scale), (int)(fy + 20.0 * scale)) > 200, "blossom body must be lit");
        Assert.Equal(0, At(2, 255));
    }

    [Fact]
    public void LitPetalsAreDrawnInTheirColourAndDarkCellsShowTheBackdrop()
    {
        var frame = Cells(2);
        var image = PetalRenderer.Render(frame, new PetalRenderOptions { Size = 1024, Supersample = 1 });
        var palette = PetalPalette.Default;
        var counts = new int[4];
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            var (x, y) = PetalLayout.CellCenter(cell);
            var at = ((int)y * 1024 + (int)x) * 3;
            var pixel = new PetalColor(image.Data[at], image.Data[at + 1], image.Data[at + 2]);
            var (expected, kind) = (frame.Petals[cell], PetalLayout.CellLight(cell)) switch
            {
                (true, true) => (palette.Light, 0),
                (true, false) => (palette.Pink, 1),
                (false, _) when PetalLayout.InMask(Math.Floor(x) + 0.5, Math.Floor(y) + 0.5) => (palette.Backdrop, 2),
                _ => (palette.Background, 3),
            };
            Assert.True(expected == pixel, $"cell {cell}: {pixel}, expected {expected}");
            counts[kind]++;
        }

        // every kind of cell occurs
        Assert.All(counts, static count => Assert.True(count > 100));
    }

    [Fact]
    public void LitDotsAreDrawnAndUnlitSlotsAreBlack()
    {
        var frame = Cells(3);
        var image = PetalRenderer.Render(frame, new PetalRenderOptions { Size = 1024, Supersample = 1 });
        var checkedSlots = 0;
        for (var ring = 0; ring < PetalLayout.RingCount; ring++)
        {
            for (var slot = 0; slot < PetalLayout.RingSlots[ring]; slot++)
            {
                var c = PetalLayout.SlotCenter(ring, slot);
                var value = image.Data[((int)c.Y * 1024 + (int)c.X) * 3];
                if (frame.Dots[PetalLayout.RingOffset(ring) + slot])
                    Assert.True(value > 150, $"ring {ring} slot {slot} should be lit");
                else
                    Assert.Equal(0, value);
                checkedSlots++;
            }
        }

        Assert.Equal(PetalLayout.TotalSlots, checkedSlots);
    }

    [Fact]
    public void RenderOptionsAreValidated()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalRenderOptions { Size = 0 });
        Assert.Throws<ArgumentOutOfRangeException>(() => new PetalRenderOptions { Supersample = 5 });
        Assert.Equal(new PetalColor(245, 175, 208), PetalPalette.Default.Pink);
        Assert.Equal(new PetalColor(25, 24, 24), PetalPalette.Default.Backdrop);
        Assert.Equal(PetalPalette.Default.Light, PetalRenderer.Shade(Cells(1), 72.0, 72.0));
        Assert.Equal(PetalPalette.Default.Background, PetalRenderer.Shade(Cells(1), 2.0, 1000.0));
        // a dark frame shows the faint 天 inside the rings and black elsewhere
        Assert.Equal(PetalPalette.Default.Backdrop, PetalRenderer.Shade(new PetalFrameCells(), 512.0, 190.0));
        Assert.Equal(PetalPalette.Default.Background, PetalRenderer.Shade(new PetalFrameCells(), 512.0, 350.0));
    }

    [Fact]
    public void DrawListDescribesEveryShape()
    {
        var cells = Cells(4);
        var list = PetalDrawList.Create(cells);
        Assert.Equal(1024.0, PetalDrawList.CanvasSize);
        Assert.Equal(4, list.Finders.Count);
        var finder = list.Finders[0];
        Assert.Equal(new PetalPoint(72.0, 72.0), finder.Center);
        Assert.Equal(5, finder.PetalCenters.Count);
        Assert.True(Math.Abs(finder.PetalCenters[0].X - 72.0) < 1e-9 && Math.Abs(finder.PetalCenters[0].Y - 38.0) < 1e-9);
        Assert.True(Math.Abs(finder.NotchCenters[0].Y - 12.0) < 1e-9);
        // the backdrop is every # cell of the 天 lattice
        Assert.Equal(PetalLayout.Mask.Sum(static row => row.Count(static c => c == '#')), list.BackdropCells.Count);
        var top = list.BackdropCells[0];
        Assert.Equal((4, 0), (top.Column, top.Row));
        Assert.Equal((182.0 + 4 * 33.0, 182.0, 33.0), (top.X, top.Y, top.Size));
        Assert.All(list.BackdropCells, static cell => Assert.Equal('#', PetalLayout.Mask[cell.Row][cell.Column]));
        // lit petals in cell order, with the outward angle and colour class of their cell
        Assert.Equal(cells.Petals.Count(static p => p), list.Petals.Count);
        Assert.Equal(
            Enumerable.Range(0, PetalLayout.FieldCells).Where(cell => cells.Petals[cell]),
            list.Petals.Select(static petal => petal.Cell));
        foreach (var petal in list.Petals)
        {
            Assert.Equal(PetalLayout.CellCenter(petal.Cell), petal.Center);
            Assert.Equal(PetalLayout.CellAngle(petal.Cell), petal.Angle);
            Assert.Equal(PetalLayout.CellLight(petal.Cell), petal.Light);
        }

        Assert.Equal(cells.Dots.Count(static d => d), list.Dots.Count);
        Assert.All(list.Dots, static dot => Assert.Equal(9.0, dot.Radius));
        Assert.All(list.Dots, dot => Assert.True(cells.Dots[PetalLayout.RingOffset(dot.Ring) + dot.Slot]));
        Assert.Equal((double)PetalLayout.PetalHalfLength, PetalDrawList.PetalHalfLength);
        Assert.Equal((double)PetalLayout.PetalHalfWidth, PetalDrawList.PetalHalfWidth);
        Assert.Equal((double)PetalLayout.PetalNotchRadius, PetalDrawList.PetalNotchRadius);
        Assert.Equal((double)PetalLayout.PetalNotchDistance, PetalDrawList.PetalNotchDistance);
        // the finder and backdrop shapes are shared between frames
        Assert.Same(list.Finders, PetalDrawList.Create(Cells(5)).Finders);
        Assert.Same(list.BackdropCells, PetalDrawList.Create(Cells(5)).BackdropCells);
    }

    [Fact]
    public void DrawListRasterizationMatchesTheRendererAndDecodes()
    {
        var payload = PetalTestSupport.Payload(600, 44);
        var encoder = new PetalStreamEncoder(payload, 2);
        const ushort frame = 9;
        const int size = 640;
        var cells = encoder.Cells(frame);
        var canvas = new RasterCanvas(size, size, 2);
        PetalDrawList.Create(cells).Draw(canvas, size / 1024.0);
        var vector = canvas.Resolve();
        var reference = PetalRenderer.Render(cells, new PetalRenderOptions { Size = size, Supersample = 2 });
        var close = 0;
        for (var i = 0; i < size * size; i++)
        {
            var same = true;
            for (var channel = 0; channel < 3; channel++)
                same &= Math.Abs(vector.Data[3 * i + channel] - reference.Data[3 * i + channel]) <= 8;
            if (same)
                close++;
        }

        Assert.True(close >= size * size * 99 / 100, $"only {close} of {size * size} pixels agree");
        var decoded = PetalDecoder.Decode(vector.ToLuma());
        Assert.True(decoded.Success, decoded.Error.ToString());
        PetalTestSupport.AssertAllLanes(decoded.Frame, encoder.LaneData(frame), "draw list");
        // scale and offset place the frame anywhere on a larger surface
        var offset = new RasterCanvas(300, 200, 1);
        PetalDrawList.Create(cells).Draw(offset, 0.125, 100.0, 50.0);
        var shifted = offset.Resolve();
        Assert.True(shifted.Data[(50 * 300 + 100) * 3] < 10, "the canvas corner is background");
        Assert.True(shifted.Data[(59 * 300 + 109) * 3] > 200, "the top-left finder core is lit");
        Assert.Throws<ArgumentOutOfRangeException>(() => PetalDrawList.Create(cells).Draw(offset, 0.0));
    }

    [Fact]
    public void FindsTheFourCornerBlossomsInACleanRender()
    {
        var encoder = new PetalStreamEncoder(Enumerable.Repeat((byte)9, 200).ToArray(), 1);
        var luma = PetalTestSupport.RenderLuma(encoder, 1, 512, 2);
        var quad = PetalLocator.Locate(luma);
        Assert.NotNull(quad);
        (double X, double Y)[] expected = [(36.0, 36.0), (476.0, 36.0), (476.0, 476.0), (36.0, 476.0)];
        for (var i = 0; i < 4; i++)
        {
            Assert.True(Math.Abs(quad[i].X - expected[i].X) < 1.5 && Math.Abs(quad[i].Y - expected[i].Y) < 1.5, $"{quad[i]}");
            Assert.True(Math.Abs(quad[i].Size - 60.0) < 6.0, $"size {quad[i].Size}");
        }
    }

    [Fact]
    public void ComponentsAreLabelledWithCorrectGeometry()
    {
        var mask = new bool[8 * 8];
        for (var y = 1; y < 4; y++)
        {
            for (var x = 2; x < 6; x++)
                mask[y * 8 + x] = true;
        }

        mask[6 * 8 + 6] = true;
        var components = PetalLocator.LabelComponents(mask, 8, 8);
        Assert.Equal(2, components.Length);
        var big = components.Single(static c => c.Area == 12);
        Assert.Equal((2u, 5u, 1u, 3u), (big.MinX, big.MaxX, big.MinY, big.MaxY));
        Assert.True(Math.Abs(big.Centroid.X - 4.0) < 1e-9 && Math.Abs(big.Centroid.Y - 2.5) < 1e-9);
        Assert.Equal(4.0, big.Width);
        Assert.Equal(3.0, big.Height);
        Assert.Throws<ArgumentException>(() => PetalLocator.LabelComponents(mask, 8, 7));
    }

    [Fact]
    public void LabellingMergesUShapesIntoOneComponent()
    {
        // a U shape: the two arms get different provisional labels and merge at the bottom
        var rows = new[] { "#...#", "#...#", "#####", "....." };
        var mask = rows.SelectMany(static r => r.Select(static c => c == '#')).ToArray();
        var components = PetalLocator.LabelComponents(mask, 5, 4);
        Assert.Single(components);
        Assert.Equal(9u, components[0].Area);
    }

    [Fact]
    public void OrderingIsClockwiseFromTheTopLeft()
    {
        static PetalFinder F(double x, double y) => new(x, y, 10.0);
        var quad = PetalLocator.OrderClockwise([F(90.0, 90.0), F(10.0, 12.0), F(88.0, 8.0), F(12.0, 92.0)]);
        Assert.NotNull(quad);
        Assert.Equal((10.0, 12.0), (quad[0].X, quad[0].Y));
        Assert.Equal((88.0, 8.0), (quad[1].X, quad[1].Y));
        Assert.Equal((90.0, 90.0), (quad[2].X, quad[2].Y));
        Assert.Null(PetalLocator.OrderClockwise([F(0, 0), F(10, 0), F(20, 0), F(30, 0)]));
    }

    [Theory]
    // the reference test: small decoys are already removed by the 0.55 × largest size class
    [InlineData(18.0)]
    // decoys inside the size class: only largest-first ranking keeps the real finders in the top ten
    [InlineData(40.0)]
    public void TheLargestCandidatesWinWhenClutterPrecedesThem(double decoySize)
    {
        // twelve collinear decoys discovered before the four real finders
        var candidates = Enumerable.Range(0, 12)
            .Select(i => new PetalFinder(10.0 + 7.0 * i, 5.0, decoySize))
            .Concat(
            [
                new PetalFinder(100.0, 100.0, 60.0),
                new PetalFinder(700.0, 110.0, 62.0),
                new PetalFinder(690.0, 520.0, 58.0),
                new PetalFinder(95.0, 510.0, 61.0),
            ])
            .ToArray();
        var quad = PetalLocator.SelectQuad(candidates);
        Assert.NotNull(quad);
        Assert.Equal(new long[] { 95, 100, 690, 700 }, quad.Select(static f => (long)f.X).Order());
    }

    [Fact]
    public void ThreeFindersFormingACornerInferTheFourth()
    {
        static PetalFinder Blob(double x, double y) => new(x, y, 60.0);
        // top-left, top-right and bottom-left of a slightly rotated square, plus clutter
        var finders = new List<PetalFinder> { Blob(100.0, 110.0), Blob(540.0, 90.0), Blob(120.0, 550.0) };
        finders.AddRange(Enumerable.Range(0, 5).Select(static i => new PetalFinder(300.0 + 10.0 * i, 300.0, 14.0)));
        var triple = PetalLocator.SelectTriple(finders.ToArray());
        Assert.NotNull(triple);
        var (quad, inferred) = triple.Value;
        var fourth = quad[inferred];
        Assert.True(Math.Abs(fourth.X - 560.0) < 1e-9 && Math.Abs(fourth.Y - 530.0) < 1e-9, $"{fourth}");
        // the inferred corner is bottom-right in clockwise order
        Assert.Equal(2, inferred);
        // three blossoms in a row are no corner
        Assert.Null(PetalLocator.SelectTriple([Blob(0.0, 0.0), Blob(440.0, 0.0), Blob(880.0, 0.0)]));
        Assert.Null(PetalLocator.SelectTriple([Blob(0.0, 0.0), Blob(440.0, 0.0)]));
    }

    [Fact]
    public void ASmallerBlobAtTheInferredCornerCompletesTheQuad()
    {
        // steep tilt: the far finder is under 0.55 of the largest, but it is where the
        // fourth corner belongs
        PetalFinder[] finders =
        [
            new(100.0, 100.0, 64.0),
            new(540.0, 100.0, 60.0),
            new(100.0, 540.0, 62.0),
            new(520.0, 515.0, 30.0),
        ];
        var strong = PetalLocator.StrongFinders(finders);
        Assert.Equal(3, strong.Length);
        var triple = PetalLocator.SelectTriple(strong);
        Assert.NotNull(triple);
        var full = PetalLocator.CompleteTriple(finders, triple.Value.Quad, triple.Value.Inferred);
        Assert.NotNull(full);
        Assert.Contains(full, static f => Math.Abs(f.X - 520.0) < 1e-9 && Math.Abs(f.Y - 515.0) < 1e-9);
        // a blob too far from the parallelogram point does not complete it
        finders[3] = new PetalFinder(400.0, 400.0, 30.0);
        Assert.Null(PetalLocator.CompleteTriple(finders, triple.Value.Quad, triple.Value.Inferred));
    }

    [Fact]
    public void AHiddenBlossomYieldsAnInferredCandidate()
    {
        var encoder = new PetalStreamEncoder(Enumerable.Repeat((byte)9, 200).ToArray(), 1);
        var luma = PetalTestSupport.RenderLuma(encoder, 1, 512, 2);
        // a clean frame: the first candidate is the quad of four seen blossoms
        var clean = PetalLocator.Candidates(luma).First();
        Assert.Null(clean.Inferred);
        Assert.Equal(PetalLocator.Locate(luma), clean.Corners.ToArray());
        // paint over the bottom-left blossom (centre 36, 476 at this size)
        var n = luma.Width;
        for (var y = 420; y < n; y++)
            luma.Data.AsSpan(y * n, 92).Clear();
        var candidates = PetalLocator.LocateCandidates(luma);
        var inferred = candidates.First(static set => set.Inferred is not null);
        var corner = inferred.Corners[inferred.Inferred ?? 0];
        Assert.True(Math.Abs(corner.X - 36.0) < 4.0 && Math.Abs(corner.Y - 476.0) < 4.0, $"{corner}");
        // nothing else forms a code, so nothing is located with four seen blossoms
        Assert.Null(PetalLocator.Locate(luma));
    }

    [Fact]
    public void FollowingFindsAMovedBlossomAndRefusesALostOne()
    {
        var encoder = new PetalStreamEncoder(Enumerable.Repeat((byte)9, 200).ToArray(), 1);
        var luma = PetalTestSupport.RenderLuma(encoder, 1, 512, 2);
        var found = PetalLocator.Follow(luma, new PetalFinder(48.0, 27.0, 60.0));
        Assert.NotNull(found);
        Assert.True(Math.Abs(found.Value.X - 36.0) < 1.5 && Math.Abs(found.Value.Y - 36.0) < 1.5, $"{found}");
        // nothing bright in the canvas corner gap between a blossom and the rings
        Assert.Null(PetalLocator.Follow(luma, new PetalFinder(100.0, 15.0, 30.0)));
        // absurd expectations (a broken pose) are refused and never overflow
        foreach (var (x, y, size) in new[]
        {
            (1e300, 36.0, 60.0),
            (-1e300, -1e300, 60.0),
            (double.NaN, 36.0, 60.0),
            (36.0, double.PositiveInfinity, 60.0),
            (36.0, 36.0, double.NaN),
        })
        {
            Assert.Null(PetalLocator.Follow(luma, new PetalFinder(x, y, size)));
        }

        // a huge disc just covers the whole image
        _ = PetalLocator.Follow(luma, new PetalFinder(36.0, 36.0, 1e300));
    }

    [Fact]
    public void SaturatingIndexArithmeticMatchesTheReference()
    {
        Assert.Equal(long.MaxValue, PetalMath.ToIsize(1e300));
        Assert.Equal(long.MinValue, PetalMath.ToIsize(double.NegativeInfinity));
        Assert.Equal(0L, PetalMath.ToIsize(double.NaN));
        Assert.Equal(-3L, PetalMath.ToIsize(-3.0));
        Assert.Equal(long.MaxValue, PetalMath.SaturatingAdd(long.MaxValue - 1, 5));
        Assert.Equal(long.MinValue, PetalMath.SaturatingAdd(long.MinValue + 1, -5));
        Assert.Equal(long.MinValue, PetalMath.SaturatingSubtract(long.MinValue + 1, 5));
        Assert.Equal(long.MaxValue, PetalMath.SaturatingSubtract(long.MaxValue - 1, -5));
        Assert.Equal(7L, PetalMath.SaturatingAdd(3, 4));
        Assert.Equal(-1L, PetalMath.SaturatingSubtract(3, 4));
    }

    [Fact]
    public void ABlankImageHasNoFinders()
    {
        Assert.Null(PetalLocator.Locate(new PetalLuma(200, 200)));
        Assert.Null(PetalLocator.Locate(new PetalLuma(0, 0)));
        Assert.Empty(PetalLocator.AdaptiveBinarize(new PetalLuma(0, 0), 0.12));
        Assert.DoesNotContain(true, PetalLocator.AdaptiveBinarize(new PetalLuma(64, 64), 0.12));
    }

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

    private static byte[] DecodePng(byte[] png, out int width, out int height, out int channels)
    {
        var offset = 8;
        width = height = channels = 0;
        using var idat = new MemoryStream();
        while (offset < png.Length)
        {
            var length = (int)BinaryPrimitives.ReadUInt32BigEndian(png.AsSpan(offset));
            var kind = System.Text.Encoding.ASCII.GetString(png, offset + 4, 4);
            var body = png.AsSpan(offset + 8, length);
            var crc = BinaryPrimitives.ReadUInt32BigEndian(png.AsSpan(offset + 8 + length));
            Assert.Equal(~PetalPng.Crc32(png.AsSpan(offset + 4, 4 + length)), crc);
            if (kind == "IHDR")
            {
                width = (int)BinaryPrimitives.ReadUInt32BigEndian(body);
                height = (int)BinaryPrimitives.ReadUInt32BigEndian(body[4..]);
                channels = body[9] switch { 0 => 1, 2 => 3, 6 => 4, _ => -1 };
                Assert.Equal(8, body[8]);
            }
            else if (kind == "IDAT")
            {
                idat.Write(body);
            }

            offset += 12 + length;
        }

        idat.Position = 0;
        using var zlib = new ZLibStream(idat, CompressionMode.Decompress);
        using var raw = new MemoryStream();
        zlib.CopyTo(raw);
        var bytes = raw.ToArray();
        var rowBytes = width * channels;
        var pixels = new byte[rowBytes * height];
        for (var row = 0; row < height; row++)
        {
            Assert.Equal(0, bytes[row * (rowBytes + 1)]);
            Array.Copy(bytes, row * (rowBytes + 1) + 1, pixels, row * rowBytes, rowBytes);
        }

        return pixels;
    }

    /// <summary>A supersampled software <see cref="IPetalCanvas"/> used to check the draw list.</summary>
    private sealed class RasterCanvas(int width, int height, int supersample) : IPetalCanvas
    {
        private readonly PetalColor[] samples = new PetalColor[width * height * supersample * supersample];
        private readonly Stack<(double X, double Y, double Radius)> clips = new();

        public void FillRectangle(double x, double y, double w, double h, PetalColor color) =>
            Paint(x, y, x + w, y + h, color, (px, py) => px >= x && px < x + w && py >= y && py < y + h);

        public void FillCircle(double centerX, double centerY, double radius, PetalColor color) =>
            Paint(centerX - radius, centerY - radius, centerX + radius, centerY + radius, color,
                (px, py) => (px - centerX) * (px - centerX) + (py - centerY) * (py - centerY) <= radius * radius);

        public void FillEllipse(double centerX, double centerY, double radiusAlong, double radiusAcross, double angle, PetalColor color)
        {
            var (sin, cos) = (Math.Sin(angle), Math.Cos(angle));
            var reach = Math.Max(radiusAlong, radiusAcross);
            Paint(centerX - reach, centerY - reach, centerX + reach, centerY + reach, color, (px, py) =>
            {
                var (dx, dy) = (px - centerX, py - centerY);
                var (along, across) = ((dx * cos + dy * sin) / radiusAlong, (-dx * sin + dy * cos) / radiusAcross);
                return along * along + across * across <= 1.0;
            });
        }

        public void PushCircleClip(double centerX, double centerY, double radius) => clips.Push((centerX, centerY, radius));

        public void PopClip() => clips.Pop();

        public PetalRgbImage Resolve()
        {
            var data = new byte[width * height * 3];
            var n = supersample * supersample;
            for (var pixel = 0; pixel < width * height; pixel++)
            {
                var (r, g, b) = (0, 0, 0);
                for (var s = 0; s < n; s++)
                {
                    var c = samples[pixel * n + s];
                    (r, g, b) = (r + c.R, g + c.G, b + c.B);
                }

                data[3 * pixel] = (byte)((r + n / 2) / n);
                data[3 * pixel + 1] = (byte)((g + n / 2) / n);
                data[3 * pixel + 2] = (byte)((b + n / 2) / n);
            }

            return new PetalRgbImage(width, height, data);
        }

        private void Paint(double x0, double y0, double x1, double y1, PetalColor color, Func<double, double, bool> inside)
        {
            var (minX, minY) = (Math.Max(0, (int)Math.Floor(x0)), Math.Max(0, (int)Math.Floor(y0)));
            var (maxX, maxY) = (Math.Min(width - 1, (int)Math.Ceiling(x1)), Math.Min(height - 1, (int)Math.Ceiling(y1)));
            var n = supersample * supersample;
            for (var py = minY; py <= maxY; py++)
            {
                for (var px = minX; px <= maxX; px++)
                {
                    for (var sy = 0; sy < supersample; sy++)
                    {
                        for (var sx = 0; sx < supersample; sx++)
                        {
                            var (x, y) = (px + (sx + 0.5) / supersample, py + (sy + 0.5) / supersample);
                            if (clips.Count > 0)
                            {
                                var clip = clips.Peek();
                                if ((x - clip.X) * (x - clip.X) + (y - clip.Y) * (y - clip.Y) > clip.Radius * clip.Radius)
                                    continue;
                            }

                            if (inside(x, y))
                                samples[(py * width + px) * n + sy * supersample + sx] = color;
                        }
                    }
                }
            }
        }
    }
}
