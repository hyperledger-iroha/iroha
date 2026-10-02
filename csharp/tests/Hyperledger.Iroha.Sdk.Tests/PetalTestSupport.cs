using System.IO.Compression;
using System.Text.Json;
using Hyperledger.Iroha.Petal;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Shared helpers of the Petal Stream tests.</summary>
internal static class PetalTestSupport
{
    private static readonly Lazy<JsonDocument> StreamFixture = new(() => Load("petal_stream_v1.json"));
    private static readonly Lazy<JsonDocument> CaptureFixture = new(() => Load("petal_captures_v1.json"));

    /// <summary><c>fixtures/petal/petal_stream_v1.json</c>.</summary>
    public static JsonElement Stream => StreamFixture.Value.RootElement;

    /// <summary><c>fixtures/petal/petal_captures_v1.json</c>.</summary>
    public static JsonElement Captures => CaptureFixture.Value.RootElement;

    /// <summary>Deterministic payload from xorshift32, as the Rust tests build it.</summary>
    public static byte[] Payload(int length, uint seed)
    {
        var rng = new PetalXorshift32(seed);
        var output = new byte[length];
        for (var i = 0; i < length; i++)
            output[i] = rng.NextByte();
        return output;
    }

    public static byte[] Hex(JsonElement element, string key) => Convert.FromHexString(element.GetProperty(key).GetString()!);

    public static long Number(JsonElement element, string key) => element.GetProperty(key).GetInt64();

    public static long[] Numbers(JsonElement element, string key) =>
        element.GetProperty(key).EnumerateArray().Select(static value => value.GetInt64()).ToArray();

    /// <summary>Inflates a <c>luma_zlib_base64</c> fixture entry.</summary>
    public static PetalLuma LumaOf(JsonElement entry)
    {
        var compressed = Convert.FromBase64String(entry.GetProperty("luma_zlib_base64").GetString()!);
        using var input = new MemoryStream(compressed);
        using var zlib = new ZLibStream(input, CompressionMode.Decompress);
        using var output = new MemoryStream();
        zlib.CopyTo(output);
        return new PetalLuma((int)Number(entry, "width"), (int)Number(entry, "height"), output.ToArray());
    }

    /// <summary>Renders a frame of <paramref name="encoder"/> to luma.</summary>
    public static PetalLuma RenderLuma(PetalStreamEncoder encoder, ushort frame, int size, int supersample) =>
        PetalRenderer.Render(encoder.Cells(frame), new PetalRenderOptions { Size = size, Supersample = supersample }).ToLuma();

    /// <summary>Applies a pixel remapping <c>target(x, y) = source(f(x, y))</c> to a square image.</summary>
    public static PetalLuma Transform(PetalLuma source, Func<int, int, (int X, int Y)> map)
    {
        var n = source.Width;
        var data = new byte[source.Data.Length];
        for (var y = 0; y < n; y++)
        {
            for (var x = 0; x < n; x++)
            {
                var (sx, sy) = map(x, y);
                data[y * n + x] = source.Data[sy * n + sx];
            }
        }

        return new PetalLuma(n, n, data);
    }

    public static PetalLuma Mirror(PetalLuma image)
    {
        var data = new byte[image.Data.Length];
        for (var y = 0; y < image.Height; y++)
        {
            for (var x = 0; x < image.Width; x++)
                data[y * image.Width + x] = image.Data[y * image.Width + image.Width - 1 - x];
        }

        return new PetalLuma(image.Width, image.Height, data);
    }

    /// <summary>Feeds the clean lanes of one encoder frame to an assembler.</summary>
    public static void FeedFrame(PetalStreamAssembler assembler, PetalStreamEncoder encoder, ushort frame, params PetalLane[] lanes)
    {
        var (p, k, d) = encoder.Words(frame);
        foreach (var lane in lanes)
        {
            var word = lane switch
            {
                PetalLane.P => p,
                PetalLane.K => k,
                _ => d,
            };
            Assert.True(PetalLanes.TryDecodeLane(lane, word, [], out var data), "clean lane");
            if (lane == PetalLane.D)
                assembler.PushDLane(PetalStream.ParseDLane(data) ?? throw new InvalidOperationException("d lane"));
            else
                assembler.PushAtoms(PetalStream.ParseAtomLane(lane, data) ?? throw new InvalidOperationException("atoms"));
        }
    }

    private static JsonDocument Load(string name) =>
        JsonDocument.Parse(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "petal", name)));
}

/// <summary>The 64-bit LCG of the Rust Reed–Solomon tests.</summary>
internal sealed class PetalLcg(ulong state)
{
    private ulong state = state;

    public uint Next()
    {
        unchecked
        {
            state = state * 6_364_136_223_846_793_005UL + 1_442_695_040_888_963_407UL;
        }

        return (uint)(state >> 33);
    }

    public byte Byte() => (byte)Next();

    public int Below(int bound) => (int)(Next() % (uint)bound);

    public double NextDouble()
    {
        unchecked
        {
            state = state * 6_364_136_223_846_793_005UL + 1_442_695_040_888_963_407UL;
        }

        return (state >> 11) / (double)(1UL << 53);
    }
}

/// <summary>Camera and scene parameters of the capture simulator (test-only port of the reference <c>sim.rs</c>).</summary>
internal sealed record PetalCaptureConfig
{
    public int Width { get; init; } = 1280;
    public int Height { get; init; } = 720;
    public double Fill { get; init; } = 0.85;
    public double RotationDeg { get; init; }
    public double TiltXDeg { get; init; }
    public double TiltYDeg { get; init; }
    public double ShiftX { get; init; }
    public double ShiftY { get; init; }
    public double LensK1 { get; init; }
    public double BlurSigma { get; init; } = 0.7;
    public double MotionPx { get; init; }
    public double MotionDeg { get; init; }
    public double Bloom { get; init; } = 0.03;
    public double BloomSigma { get; init; } = 4.0;
    public double Exposure { get; init; } = 1.0;
    public double Ambient { get; init; } = 0.02;
    public double Gradient { get; init; } = 0.05;
    public double GradientDeg { get; init; } = 30.0;
    public double Vignette { get; init; } = 0.1;
    public double Glare { get; init; }
    public double GlareAtX { get; init; } = 0.5;
    public double GlareAtY { get; init; } = 0.5;
    public double GlareSigma { get; init; } = 40.0;
    public double Noise { get; init; } = 2.5;
    public double Sharpen { get; init; }
    public ulong Seed { get; init; } = 1;

    /// <summary>A recent phone in good light: sharp and quiet.</summary>
    public static PetalCaptureConfig Modern() => new();

    /// <summary>An older phone: soft focus, noisy, some tilt and barrel distortion.</summary>
    public static PetalCaptureConfig Legacy() => new()
    {
        Fill = 0.8,
        RotationDeg = 12.0,
        TiltXDeg = 12.0,
        TiltYDeg = -10.0,
        ShiftX = 0.02,
        ShiftY = -0.01,
        LensK1 = -0.06,
        BlurSigma = 1.3,
        Bloom = 0.08,
        BloomSigma = 5.0,
        Exposure = 1.1,
        Ambient = 0.06,
        Gradient = 0.15,
        GradientDeg = 120.0,
        Vignette = 0.25,
        GlareAtX = 0.3,
        GlareAtY = 0.3,
        GlareSigma = 50.0,
        Noise = 6.0,
        Sharpen = 0.4,
        Seed = 2,
    };
}

/// <summary>Deterministic camera-capture simulator (test-only port of the reference <c>sim.rs</c>).</summary>
internal static class PetalCaptureSimulator
{
    private const int Supersample = 3;

    public static PetalHomography CameraHomography(PetalCaptureConfig config)
    {
        var (w, h) = ((double)config.Width, (double)config.Height);
        var focal = 0.8 * Math.Max(w, h);
        var distance = focal * 1024.0 / (config.Fill * Math.Min(w, h));
        var (rz, rx, ry) = (Radians(config.RotationDeg), Radians(config.TiltXDeg), Radians(config.TiltYDeg));
        var (sz, cz) = Math.SinCos(rz);
        var (sx, cx) = Math.SinCos(rx);
        var (sy, cy) = Math.SinCos(ry);
        double[,] rotX = { { 1.0, 0.0, 0.0 }, { 0.0, cx, -sx }, { 0.0, sx, cx } };
        double[,] rotY = { { cy, 0.0, sy }, { 0.0, 1.0, 0.0 }, { -sy, 0.0, cy } };
        double[,] rotZ = { { cz, -sz, 0.0 }, { sz, cz, 0.0 }, { 0.0, 0.0, 1.0 } };
        var r = Multiply(rotZ, Multiply(rotY, rotX));
        double[] t =
        [
            distance * 0.0 - 512.0 * r[0, 0] - 512.0 * r[0, 1],
            -512.0 * r[1, 0] - 512.0 * r[1, 1],
            distance - 512.0 * r[2, 0] - 512.0 * r[2, 1],
        ];
        var cxImage = w / 2.0 + config.ShiftX * w;
        var cyImage = h / 2.0 + config.ShiftY * h;
        double[,] k = { { focal, 0.0, cxImage }, { 0.0, focal, cyImage }, { 0.0, 0.0, 1.0 } };
        double[,] m = { { r[0, 0], r[0, 1], t[0] }, { r[1, 0], r[1, 1], t[1] }, { r[2, 0], r[2, 1], t[2] } };
        var hm = Multiply(k, m);
        return new PetalHomography(hm[0, 0], hm[0, 1], hm[0, 2], hm[1, 0], hm[1, 1], hm[1, 2], hm[2, 0], hm[2, 1], hm[2, 2]);
    }

    /// <summary>Shrinks the fill until the four canvas corners lie inside the frame.</summary>
    public static PetalCaptureConfig FitToFrame(PetalCaptureConfig config, double margin)
    {
        bool Inside(PetalCaptureConfig c)
        {
            var h = CameraHomography(c);
            foreach (var (x, y) in new[] { (0.0, 0.0), (1024.0, 0.0), (1024.0, 1024.0), (0.0, 1024.0) })
            {
                var p = h.Apply(x, y);
                if (!(p.X >= margin && p.Y >= margin && p.X <= c.Width - margin && p.Y <= c.Height - margin))
                    return false;
            }

            return true;
        }

        var fitted = config;
        for (var i = 0; i < 40; i++)
        {
            if (Inside(fitted))
                break;
            fitted = fitted with { Fill = fitted.Fill * 0.97 };
        }

        return fitted;
    }

    /// <summary>Captures a rendered frame with the simulated camera.</summary>
    public static PetalLuma Capture(PetalRgbImage source, PetalCaptureConfig config)
    {
        var (w, h) = (config.Width, config.Height);
        var focal = 0.8 * Math.Max(w, h);
        var forward = CameraHomography(config);
        var backward = forward.Inverse() ?? throw new InvalidOperationException("camera homography is invertible");
        var cxImage = w / 2.0 + config.ShiftX * w;
        var cyImage = h / 2.0 + config.ShiftY * h;
        var sourceWidth = source.Width;
        var linear = new double[source.Width * source.Height];
        for (var i = 0; i < linear.Length; i++)
        {
            var (r, g, b) = (source.Data[3 * i], source.Data[3 * i + 1], source.Data[3 * i + 2]);
            linear[i] = Math.Pow((0.299 * r + 0.587 * g + 0.114 * b) / 255.0, 2.2);
        }

        var scale = sourceWidth / 1024.0;
        double SampleSource(double x, double y)
        {
            var (sx, sy) = (x * scale, y * scale);
            if (sx < 0.0 || sy < 0.0 || sx >= sourceWidth || sy >= source.Height)
                return 0.0;
            var fx = Math.Clamp(sx - 0.5, 0.0, sourceWidth - 1);
            var fy = Math.Clamp(sy - 0.5, 0.0, source.Height - 1);
            var (x0, y0) = ((int)Math.Floor(fx), (int)Math.Floor(fy));
            var (x1, y1) = (Math.Min(x0 + 1, sourceWidth - 1), Math.Min(y0 + 1, source.Height - 1));
            var (tx, ty) = (fx - x0, fy - y0);
            double P(int xx, int yy) => linear[yy * sourceWidth + xx];
            return (P(x0, y0) * (1.0 - tx) + P(x1, y0) * tx) * (1.0 - ty)
                + (P(x0, y1) * (1.0 - tx) + P(x1, y1) * tx) * ty;
        }

        var plane = new double[w * h];
        for (var y = 0; y < h; y++)
        {
            for (var x = 0; x < w; x++)
            {
                var acc = 0.0;
                for (var sy = 0; sy < Supersample; sy++)
                {
                    for (var sx = 0; sx < Supersample; sx++)
                    {
                        var px = x + (sx + 0.5) / Supersample;
                        var py = y + (sy + 0.5) / Supersample;
                        // undo lens distortion: p_d = c + (p_u - c)(1 + k1 r^2)
                        if (config.LensK1 != 0.0)
                        {
                            var (dx, dy) = (px - cxImage, py - cyImage);
                            var (ux, uy) = (dx, dy);
                            for (var i = 0; i < 5; i++)
                            {
                                var r2 = (ux * ux + uy * uy) / (focal * focal);
                                var factor = 1.0 + config.LensK1 * r2;
                                ux = dx / factor;
                                uy = dy / factor;
                            }

                            px = cxImage + ux;
                            py = cyImage + uy;
                        }

                        var canvas = backward.Apply(px, py);
                        acc += SampleSource(canvas.X, canvas.Y);
                    }
                }

                plane[y * w + x] = acc / (Supersample * Supersample);
            }
        }

        plane = MotionBlur(plane, w, h, config.MotionPx, config.MotionDeg);
        var optical = Blur(plane, w, h, config.BlurSigma);
        if (config.Bloom > 0.0)
        {
            var halo = Blur(plane, w, h, config.BloomSigma);
            for (var i = 0; i < plane.Length; i++)
                plane[i] = (1.0 - config.Bloom) * optical[i] + config.Bloom * halo[i];
        }
        else
        {
            plane = optical;
        }

        // auto exposure: map the 99.5th percentile to 0.85, then apply the multiplier
        var sorted = (double[])plane.Clone();
        Array.Sort(sorted);
        var p995 = Math.Max(sorted[(int)((sorted.Length - 1) * 0.995)], 1e-4);
        var litLevel = p995;
        var gain = 0.85 / p995 * config.Exposure;
        var (gx, gy) = (Math.Cos(Radians(config.GradientDeg)), Math.Sin(Radians(config.GradientDeg)));
        var rng = new GaussianRng(config.Seed);
        var (diagX, diagY) = (w / 2.0, h / 2.0);
        var maxR2 = diagX * diagX + diagY * diagY;
        var encoded = new double[w * h];
        for (var y = 0; y < h; y++)
        {
            for (var x = 0; x < w; x++)
            {
                var (dx, dy) = (x - diagX, y - diagY);
                var value = plane[y * w + x];
                value *= 1.0 - config.Vignette * (dx * dx + dy * dy) / maxR2;
                var along = (dx * gx + dy * gy) / Math.Sqrt(maxR2);
                value += litLevel * (config.Ambient + config.Gradient * (0.5 + 0.5 * Math.Clamp(along, -1.0, 1.0)));
                if (config.Glare > 0.0)
                {
                    var (ex, ey) = (x - config.GlareAtX * w, y - config.GlareAtY * h);
                    value += litLevel * config.Glare * Math.Exp(-(ex * ex + ey * ey) / (2.0 * config.GlareSigma * config.GlareSigma));
                }

                var level = Math.Pow(Math.Max(Math.Clamp(value * gain, 0.0, 1.0), 0.0), 1.0 / 2.2) * 255.0;
                var sigma = config.Noise * Math.Sqrt(0.3 + 0.7 * level / 255.0);
                encoded[y * w + x] = level + sigma * rng.Gaussian();
            }
        }

        if (config.Sharpen > 0.0)
        {
            var soft = Blur(encoded, w, h, 1.4);
            for (var i = 0; i < encoded.Length; i++)
                encoded[i] += config.Sharpen * (encoded[i] - soft[i]);
        }

        var data = new byte[w * h];
        for (var i = 0; i < data.Length; i++)
            data[i] = (byte)Math.Clamp(Math.Round(encoded[i], MidpointRounding.AwayFromZero), 0.0, 255.0);
        return new PetalLuma(w, h, data);
    }

    /// <summary>Mixes two captures to model an exposure that straddles a frame change.</summary>
    public static PetalLuma Blend(PetalLuma a, PetalLuma b, double alpha)
    {
        var data = new byte[a.Data.Length];
        for (var i = 0; i < data.Length; i++)
            data[i] = (byte)Math.Round(a.Data[i] * (1.0 - alpha) + b.Data[i] * alpha, MidpointRounding.AwayFromZero);
        return new PetalLuma(a.Width, a.Height, data);
    }

    /// <summary>Rolling-shutter tearing: rows above <paramref name="row"/> come from <paramref name="a"/>.</summary>
    public static PetalLuma Tear(PetalLuma a, PetalLuma b, int row)
    {
        var split = Math.Min(row, a.Height) * a.Width;
        var data = (byte[])b.Data.Clone();
        Array.Copy(a.Data, data, split);
        return new PetalLuma(a.Width, a.Height, data);
    }

    private static double Radians(double degrees) => degrees * (Math.PI / 180.0);

    private static double[,] Multiply(double[,] a, double[,] b)
    {
        var output = new double[3, 3];
        for (var i = 0; i < 3; i++)
        {
            for (var j = 0; j < 3; j++)
            {
                var sum = -0.0;
                for (var k = 0; k < 3; k++)
                    sum += a[i, k] * b[k, j];
                output[i, j] = sum;
            }
        }

        return output;
    }

    private static double[] Kernel(double sigma)
    {
        var radius = (int)Math.Max(Math.Ceiling(3.0 * sigma), 1.0);
        var kernel = new double[2 * radius + 1];
        var sum = -0.0;
        for (var i = -radius; i <= radius; i++)
        {
            kernel[i + radius] = Math.Exp(-(double)(i * i) / (2.0 * sigma * sigma));
            sum += kernel[i + radius];
        }

        for (var i = 0; i < kernel.Length; i++)
            kernel[i] /= sum;
        return kernel;
    }

    private static double[] Blur(double[] plane, int width, int height, double sigma)
    {
        if (sigma < 0.05)
            return (double[])plane.Clone();
        var kernel = Kernel(sigma);
        var radius = kernel.Length / 2;
        var horizontal = new double[plane.Length];
        for (var y = 0; y < height; y++)
        {
            for (var x = 0; x < width; x++)
            {
                var acc = 0.0;
                for (var k = 0; k < kernel.Length; k++)
                    acc += kernel[k] * plane[y * width + Math.Clamp(x + k - radius, 0, width - 1)];
                horizontal[y * width + x] = acc;
            }
        }

        var output = new double[plane.Length];
        for (var y = 0; y < height; y++)
        {
            for (var x = 0; x < width; x++)
            {
                var acc = 0.0;
                for (var k = 0; k < kernel.Length; k++)
                    acc += kernel[k] * horizontal[Math.Clamp(y + k - radius, 0, height - 1) * width + x];
                output[y * width + x] = acc;
            }
        }

        return output;
    }

    private static double[] MotionBlur(double[] plane, int width, int height, double length, double degrees)
    {
        if (length < 0.5)
            return plane;
        var steps = Math.Max((int)Math.Ceiling(length), 2);
        var (dx, dy) = (Math.Cos(Radians(degrees)), Math.Sin(Radians(degrees)));
        var output = new double[plane.Length];
        for (var y = 0; y < height; y++)
        {
            for (var x = 0; x < width; x++)
            {
                var acc = 0.0;
                for (var s = 0; s < steps; s++)
                {
                    var t = ((double)s / (steps - 1) - 0.5) * length;
                    var sx = (int)Math.Clamp(Math.Round(x + dx * t, MidpointRounding.AwayFromZero), 0.0, width - 1.0);
                    var sy = (int)Math.Clamp(Math.Round(y + dy * t, MidpointRounding.AwayFromZero), 0.0, height - 1.0);
                    acc += plane[sy * width + sx];
                }

                output[y * width + x] = acc / steps;
            }
        }

        return output;
    }

    /// <summary>SplitMix64 with a Box–Muller Gaussian.</summary>
    private sealed class GaussianRng(ulong seed)
    {
        private ulong state = seed ^ 0x9E37_79B9_7F4A_7C15UL;
        private double? spare;

        public double Gaussian()
        {
            if (spare is { } value)
            {
                spare = null;
                return value;
            }

            var (u1, u2) = (Uniform(), Uniform());
            var radius = Math.Sqrt(-2.0 * Math.Log(u1));
            var angle = Math.Tau * u2;
            spare = radius * Math.Sin(angle);
            return radius * Math.Cos(angle);
        }

        private double Uniform() => ((NextUInt64() >> 11) + 0.5) / (1UL << 53);

        private ulong NextUInt64()
        {
            unchecked
            {
                state += 0x9E37_79B9_7F4A_7C15UL;
                var z = state;
                z = (z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9UL;
                z = (z ^ (z >> 27)) * 0x94D0_49BB_1331_11EBUL;
                return z ^ (z >> 31);
            }
        }
    }
}
