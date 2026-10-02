using System.Buffers;

namespace Hyperledger.Iroha.Petal;

/// <summary>A detected corner finder.</summary>
/// <param name="X">Centre x in pixel-edge coordinates.</param>
/// <param name="Y">Centre y in pixel-edge coordinates.</param>
/// <param name="Size">Apparent outer diameter in pixels.</param>
public readonly record struct PetalFinder(double X, double Y, double Size);

/// <summary>A 4-connected component of a binarised image.</summary>
public readonly struct PetalComponent
{
    internal PetalComponent(
        uint area,
        uint minX,
        uint maxX,
        uint minY,
        uint maxY,
        double sumX,
        double sumY,
        double sumXX,
        double sumYY,
        double sumXY)
    {
        Area = area;
        MinX = minX;
        MaxX = maxX;
        MinY = minY;
        MaxY = maxY;
        SumX = sumX;
        SumY = sumY;
        SumXX = sumXX;
        SumYY = sumYY;
        SumXY = sumXY;
    }

    /// <summary>Pixel count.</summary>
    public uint Area { get; }

    /// <summary>Left-most pixel column.</summary>
    public uint MinX { get; }

    /// <summary>Right-most pixel column.</summary>
    public uint MaxX { get; }

    /// <summary>Top-most pixel row.</summary>
    public uint MinY { get; }

    /// <summary>Bottom-most pixel row.</summary>
    public uint MaxY { get; }

    /// <summary>Bounding-box width in pixels.</summary>
    public double Width => MaxX - MinX + 1;

    /// <summary>Bounding-box height in pixels.</summary>
    public double Height => MaxY - MinY + 1;

    /// <summary>Centroid in pixel-edge coordinates (pixel centres at <c>+0.5</c>).</summary>
    public PetalPoint Centroid => new(SumX / Area, SumY / Area);

    internal double SumX { get; }

    internal double SumY { get; }

    internal double SumXX { get; }

    internal double SumYY { get; }

    internal double SumXY { get; }

    /// <summary>Ratio of the smaller to the larger principal axis of the blob.</summary>
    public double AxisRatio
    {
        get
        {
            double n = Area;
            var cx = SumX / Area;
            var cy = SumY / Area;
            var vxx = SumXX / n - cx * cx;
            var vyy = SumYY / n - cy * cy;
            var vxy = SumXY / n - cx * cy;
            var mean = 0.5 * (vxx + vyy);
            var spread = Math.Sqrt(0.25 * ((vxx - vyy) * (vxx - vyy)) + vxy * vxy);
            var major = mean + spread;
            var minor = PetalMath.Max(mean - spread, 0.0);
            return major <= 0.0 ? 0.0 : Math.Sqrt(minor / major);
        }
    }
}

/// <summary>
/// Finding the four corner finders in a camera luma plane.
/// </summary>
/// <remarks>
/// Pipeline: adaptive threshold (local mean via an integral image) → 4-connected
/// component labelling → blossom detection (a large, round, isolated blob) →
/// selection of the four finders that form a plausible, similarly sized
/// quadrilateral → intensity-weighted centre refinement. Solid blossoms survive
/// defocus that would fill in the gaps of a ring-shaped marker.
/// </remarks>
public static class PetalLocator
{
    private static readonly double[] Sensitivities = [0.12, 0.22, 0.34];

    /// <summary>Marks pixels that are clearly brighter than their neighbourhood.</summary>
    /// <param name="image">The luma plane.</param>
    /// <param name="sensitivity">
    /// Margin above the local mean in units of the image's dynamic range
    /// (≈ 0.12 for faint codes; larger values separate blurred blobs).
    /// </param>
    /// <returns>Row-major mask, <see langword="true"/> for bright pixels.</returns>
    public static bool[] AdaptiveBinarize(PetalLuma image, double sensitivity)
    {
        ArgumentNullException.ThrowIfNull(image);
        var mask = new bool[image.Width * image.Height];
        if (mask.Length == 0)
            return mask;
        var statistics = new LocalStatistics(image);
        try
        {
            statistics.Binarize(image, sensitivity, mask);
        }
        finally
        {
            statistics.Dispose();
        }

        return mask;
    }

    /// <summary>Labels 4-connected components; returns the components in label order.</summary>
    /// <param name="mask">Row-major mask.</param>
    /// <param name="width">Mask width.</param>
    /// <param name="height">Mask height.</param>
    /// <returns>Components with a non-zero area, ordered by their first pixel in raster order.</returns>
    /// <exception cref="ArgumentException">The mask size does not match the dimensions.</exception>
    public static PetalComponent[] LabelComponents(ReadOnlySpan<bool> mask, int width, int height)
    {
        if (width < 0 || height < 0 || (long)width * height != mask.Length)
            throw new ArgumentException("Mask size does not match width * height.", nameof(mask));
        return Label(mask, width, height);
    }

    /// <summary>Detects blossom finders: large, round, isolated blobs.</summary>
    /// <param name="components">Labelled components.</param>
    /// <returns>Candidate finders in component order.</returns>
    public static PetalFinder[] Blossoms(ReadOnlySpan<PetalComponent> components)
    {
        var found = new List<PetalFinder>();
        for (var index = 0; index < components.Length; index++)
        {
            var blob = components[index];
            var size = PetalMath.Max(blob.Width, blob.Height);
            var fill = blob.Area / (blob.Width * blob.Height);
            if (size < 14.0 || blob.Area < 100 || !(fill >= 0.45 && fill <= 0.9) || blob.AxisRatio < 0.5)
                continue;
            var x = blob.SumX / blob.Area;
            var y = blob.SumY / blob.Area;
            // isolation: nothing else of substance close by
            var crowded = false;
            for (var other = 0; other < components.Length; other++)
            {
                var c = components[other];
                if (other == index || c.Area < 8 || c.Area < 0.015 * blob.Area)
                    continue;
                var ox = c.SumX / c.Area;
                var oy = c.SumY / c.Area;
                if (Math.Sqrt((ox - x) * (ox - x) + (oy - y) * (oy - y)) < 0.8 * size)
                {
                    crowded = true;
                    break;
                }
            }

            if (!crowded)
                found.Add(new PetalFinder(x, y, size));
        }

        return found.ToArray();
    }

    /// <summary>Chooses four finders that look like the corners of one code.</summary>
    /// <remarks>
    /// Lit tiles and merged dots form blob candidates too, so the largest size
    /// class (at least 0.55 × the largest candidate) is tried first: the corner
    /// finders are always the biggest isolated round blobs in view. Within a pass
    /// the candidates are ranked largest first (ties in discovery order) and only
    /// the first ten are combined, so clutter discovered earlier in a busy scene
    /// cannot push the real finders out.
    /// </remarks>
    /// <param name="finders">Candidate finders.</param>
    /// <returns>Four finders clockwise from the top-left, or <see langword="null"/>.</returns>
    public static PetalFinder[]? SelectQuad(ReadOnlySpan<PetalFinder> finders)
    {
        var largest = 0.0;
        foreach (var finder in finders)
            largest = PetalMath.Max(largest, finder.Size);
        var strong = new List<PetalFinder>();
        foreach (var finder in finders)
        {
            if (finder.Size >= 0.55 * largest)
                strong.Add(finder);
        }

        return SelectQuadFrom(System.Runtime.InteropServices.CollectionsMarshal.AsSpan(strong))
            ?? SelectQuadFrom(finders);
    }

    /// <summary>Sharpens a finder centre with an intensity-weighted centroid.</summary>
    /// <param name="image">The luma plane.</param>
    /// <param name="finder">Coarse finder.</param>
    /// <returns>The refined finder (unchanged when the blob lacks contrast).</returns>
    public static PetalFinder RefineCenter(PetalLuma image, PetalFinder finder)
    {
        ArgumentNullException.ThrowIfNull(image);
        var half = finder.Size * 0.5;
        var radius = (int)Math.Ceiling(half);
        var cx = (int)Math.Floor(finder.X);
        var cy = (int)Math.Floor(finder.Y);
        var floor = double.MaxValue;
        var peak = 0.0;
        for (var dy = -radius; dy <= radius; dy++)
        {
            for (var dx = -radius; dx <= radius; dx++)
            {
                var (x, y) = (cx + dx, cy + dy);
                if (x < 0 || y < 0 || x >= image.Width || y >= image.Height)
                    continue;
                var (px, py) = (x + 0.5, y + 0.5);
                if (Math.Sqrt((px - finder.X) * (px - finder.X) + (py - finder.Y) * (py - finder.Y)) <= half)
                {
                    double value = image.Data[y * image.Width + x];
                    floor = PetalMath.Min(floor, value);
                    peak = PetalMath.Max(peak, value);
                }
            }
        }

        if (peak - floor < 20.0)
            return finder;
        var threshold = floor + 0.5 * (peak - floor);
        var (sw, sx, sy) = (0.0, 0.0, 0.0);
        for (var dy = -radius; dy <= radius; dy++)
        {
            for (var dx = -radius; dx <= radius; dx++)
            {
                var (x, y) = (cx + dx, cy + dy);
                if (x < 0 || y < 0 || x >= image.Width || y >= image.Height)
                    continue;
                var (px, py) = (x + 0.5, y + 0.5);
                if (Math.Sqrt((px - finder.X) * (px - finder.X) + (py - finder.Y) * (py - finder.Y)) <= half)
                {
                    double value = image.Data[y * image.Width + x];
                    var weight = PetalMath.Max(value - threshold, 0.0);
                    sw += weight;
                    sx += weight * px;
                    sy += weight * py;
                }
            }
        }

        return sw <= 0.0 ? finder : new PetalFinder(sx / sw, sy / sw, finder.Size);
    }

    /// <summary>
    /// Locates the four finders of a code, trying progressively stricter
    /// thresholds so blurred blobs still separate.
    /// </summary>
    /// <param name="image">The luma plane.</param>
    /// <returns>Four refined finders clockwise from the top-left, or <see langword="null"/>.</returns>
    public static PetalFinder[]? Locate(PetalLuma image)
    {
        ArgumentNullException.ThrowIfNull(image);
        var pixels = image.Width * image.Height;
        if (pixels == 0)
            return null;
        // The integral image and percentiles do not depend on the sensitivity;
        // computing them once leaves every threshold decision unchanged.
        var statistics = new LocalStatistics(image);
        var mask = ArrayPool<bool>.Shared.Rent(pixels);
        try
        {
            foreach (var sensitivity in Sensitivities)
            {
                var span = mask.AsSpan(0, pixels);
                statistics.Binarize(image, sensitivity, span);
                var components = Label(span, image.Width, image.Height);
                var finders = Blossoms(components);
                var quad = SelectQuad(finders);
                if (quad is null)
                    continue;
                for (var i = 0; i < quad.Length; i++)
                    quad[i] = RefineCenter(image, quad[i]);
                return quad;
            }

            return null;
        }
        finally
        {
            ArrayPool<bool>.Shared.Return(mask);
            statistics.Dispose();
        }
    }

    /// <summary>Orders four finders clockwise (as displayed) from the one nearest the top-left.</summary>
    internal static PetalFinder[]? OrderClockwise(ReadOnlySpan<PetalFinder> set)
    {
        Span<PetalFinder> quad = stackalloc PetalFinder[4];
        set[..4].CopyTo(quad);
        var cx = PetalMath.SumIdentity;
        var cy = PetalMath.SumIdentity;
        foreach (var finder in quad)
        {
            cx += finder.X;
            cy += finder.Y;
        }

        cx /= 4.0;
        cy /= 4.0;
        Span<double> angles = stackalloc double[4];
        for (var i = 0; i < 4; i++)
            angles[i] = Math.Atan2(quad[i].Y - cy, quad[i].X - cx);
        // Stable insertion sort by angle (Rust `sort_by` with `total_cmp`).
        for (var i = 1; i < 4; i++)
        {
            var item = quad[i];
            var angle = angles[i];
            var j = i - 1;
            while (j >= 0 && PetalMath.TotalCompare(angles[j], angle) > 0)
            {
                quad[j + 1] = quad[j];
                angles[j + 1] = angles[j];
                j--;
            }

            quad[j + 1] = item;
            angles[j + 1] = angle;
        }

        // atan2 grows clockwise on screen because y points down; verify convexity
        for (var i = 0; i < 4; i++)
        {
            var o = quad[i];
            var a = quad[(i + 1) % 4];
            var b = quad[(i + 2) % 4];
            if ((a.X - o.X) * (b.Y - o.Y) - (a.Y - o.Y) * (b.X - o.X) <= 0.0)
                return null;
        }

        var start = 0;
        for (var i = 1; i < 4; i++)
        {
            if (PetalMath.TotalCompare(quad[i].X + quad[i].Y, quad[start].X + quad[start].Y) < 0)
                start = i;
        }

        return [quad[start], quad[(start + 1) % 4], quad[(start + 2) % 4], quad[(start + 3) % 4]];
    }

    private static PetalFinder[]? SelectQuadFrom(ReadOnlySpan<PetalFinder> candidates)
    {
        if (candidates.Length < 4)
            return null;
        // Largest first (ties keep discovery order) so that clutter in a busy scene
        // cannot push the real finders out of the ten candidates that are combined.
        var sizes = new double[candidates.Length];
        var order = new int[candidates.Length];
        for (var i = 0; i < candidates.Length; i++)
        {
            sizes[i] = candidates[i].Size;
            order[i] = i;
        }

        Array.Sort(order, (a, b) =>
        {
            var bySize = PetalMath.TotalCompare(sizes[b], sizes[a]);
            return bySize != 0 ? bySize : a.CompareTo(b);
        });
        var n = Math.Min(candidates.Length, 10);
        Span<PetalFinder> finders = stackalloc PetalFinder[n];
        for (var i = 0; i < n; i++)
            finders[i] = candidates[order[i]];
        PetalFinder[]? best = null;
        var bestScore = 0.0;
        Span<PetalFinder> set = stackalloc PetalFinder[4];
        Span<double> sides = stackalloc double[4];
        for (var a = 0; a < n; a++)
        {
            for (var b = a + 1; b < n; b++)
            {
                for (var c = b + 1; c < n; c++)
                {
                    for (var d = c + 1; d < n; d++)
                    {
                        set[0] = finders[a];
                        set[1] = finders[b];
                        set[2] = finders[c];
                        set[3] = finders[d];
                        var smin = double.MaxValue;
                        var smax = 0.0;
                        var sizeSum = PetalMath.SumIdentity;
                        foreach (var finder in set)
                        {
                            smin = PetalMath.Min(smin, finder.Size);
                            smax = PetalMath.Max(smax, finder.Size);
                        }

                        if (smax / smin > 1.9)
                            continue;
                        var quad = OrderClockwise(set);
                        if (quad is null)
                            continue;
                        var lmin = double.MaxValue;
                        var lmax = 0.0;
                        var sideSum = PetalMath.SumIdentity;
                        for (var i = 0; i < 4; i++)
                        {
                            var (p, q) = (quad[i], quad[(i + 1) % 4]);
                            sides[i] = Math.Sqrt((p.X - q.X) * (p.X - q.X) + (p.Y - q.Y) * (p.Y - q.Y));
                        }

                        foreach (var side in sides)
                        {
                            lmin = PetalMath.Min(lmin, side);
                            lmax = PetalMath.Max(lmax, side);
                            sideSum += side;
                        }

                        foreach (var finder in set)
                            sizeSum += finder.Size;
                        var meanSize = sizeSum / 4.0;
                        // canvas geometry: side / finder diameter = 880 / 120
                        var ratio = sideSum / 4.0 / meanSize;
                        if (lmax / lmin > 2.6 || !(ratio >= 4.8 && ratio <= 10.5))
                            continue;
                        var score = (smax / smin - 1.0) + (lmax / lmin - 1.0) + Math.Abs((ratio - 7.33) / 7.33);
                        if (best is null || score < bestScore)
                        {
                            best = quad;
                            bestScore = score;
                        }
                    }
                }
            }
        }

        return best;
    }

    private static PetalComponent[] Label(ReadOnlySpan<bool> mask, int width, int height)
    {
        var pixels = width * height;
        if (pixels == 0)
            return [];
        var labels = ArrayPool<int>.Shared.Rent(pixels);
        var parent = ArrayPool<int>.Shared.Rent(1024);
        var parentCount = 1;
        parent[0] = 0;
        try
        {
            for (var y = 0; y < height; y++)
            {
                for (var x = 0; x < width; x++)
                {
                    var i = y * width + x;
                    if (!mask[i])
                    {
                        labels[i] = 0;
                        continue;
                    }

                    var left = x > 0 ? labels[i - 1] : 0;
                    var up = y > 0 ? labels[i - width] : 0;
                    int label;
                    if (left == 0 && up == 0)
                    {
                        if (parentCount == parent.Length)
                            parent = Grow(parent, parentCount);
                        label = parentCount;
                        parent[parentCount++] = label;
                    }
                    else if (up == 0)
                    {
                        label = left;
                    }
                    else if (left == 0)
                    {
                        label = up;
                    }
                    else
                    {
                        var a = FindRoot(parent, left);
                        var b = FindRoot(parent, up);
                        var (keep, drop) = a < b ? (a, b) : (b, a);
                        parent[drop] = keep;
                        label = keep;
                    }

                    labels[i] = label;
                }
            }

            var stats = ArrayPool<ComponentAccumulator>.Shared.Rent(parentCount);
            try
            {
                Array.Clear(stats, 0, parentCount);
                for (var y = 0; y < height; y++)
                {
                    for (var x = 0; x < width; x++)
                    {
                        var label = labels[y * width + x];
                        if (label == 0)
                            continue;
                        ref var c = ref stats[FindRoot(parent, label)];
                        if (c.Area == 0)
                        {
                            c.MinX = (uint)x;
                            c.MaxX = (uint)x;
                            c.MinY = (uint)y;
                            c.MaxY = (uint)y;
                        }

                        c.Area++;
                        c.MinX = Math.Min(c.MinX, (uint)x);
                        c.MaxX = Math.Max(c.MaxX, (uint)x);
                        c.MinY = Math.Min(c.MinY, (uint)y);
                        c.MaxY = Math.Max(c.MaxY, (uint)y);
                        var (px, py) = (x + 0.5, y + 0.5);
                        c.SumX += px;
                        c.SumY += py;
                        c.SumXX += px * px;
                        c.SumYY += py * py;
                        c.SumXY += px * py;
                    }
                }

                var count = 0;
                for (var i = 0; i < parentCount; i++)
                {
                    if (stats[i].Area > 0)
                        count++;
                }

                var components = new PetalComponent[count];
                var next = 0;
                for (var i = 0; i < parentCount; i++)
                {
                    ref var c = ref stats[i];
                    if (c.Area > 0)
                        components[next++] = new PetalComponent(c.Area, c.MinX, c.MaxX, c.MinY, c.MaxY, c.SumX, c.SumY, c.SumXX, c.SumYY, c.SumXY);
                }

                return components;
            }
            finally
            {
                ArrayPool<ComponentAccumulator>.Shared.Return(stats);
            }
        }
        finally
        {
            ArrayPool<int>.Shared.Return(labels);
            ArrayPool<int>.Shared.Return(parent);
        }
    }

    private static int[] Grow(int[] parent, int count)
    {
        var larger = ArrayPool<int>.Shared.Rent(parent.Length * 2);
        Array.Copy(parent, larger, count);
        ArrayPool<int>.Shared.Return(parent);
        return larger;
    }

    private static int FindRoot(int[] parent, int label)
    {
        while (parent[label] != label)
        {
            parent[label] = parent[parent[label]];
            label = parent[label];
        }

        return label;
    }

    private struct ComponentAccumulator
    {
        public uint Area;
        public uint MinX;
        public uint MaxX;
        public uint MinY;
        public uint MaxY;
        public double SumX;
        public double SumY;
        public double SumXX;
        public double SumYY;
        public double SumXY;
    }

    /// <summary>Integral image and range statistics shared by every sensitivity.</summary>
    private readonly struct LocalStatistics : IDisposable
    {
        private readonly uint[] integral;
        private readonly double low;
        private readonly double range;
        private readonly int radius;

        public LocalStatistics(PetalLuma image)
        {
            var (w, h) = (image.Width, image.Height);
            var data = image.Data;
            var cells = (long)(w + 1) * (h + 1);
            if (cells > Array.MaxLength)
                throw new ArgumentOutOfRangeException(nameof(image), "Image is too large to locate finders in.");
            // Window sums never exceed 129 * 129 * 255 < 2^32, so a wrapping
            // 32-bit integral image yields exactly the reference's 64-bit sums.
            integral = ArrayPool<uint>.Shared.Rent((int)cells);
            Array.Clear(integral, 0, w + 1);
            for (var y = 0; y < h; y++)
            {
                uint row = 0;
                var above = y * (w + 1);
                var current = (y + 1) * (w + 1);
                integral[current] = 0;
                for (var x = 0; x < w; x++)
                {
                    unchecked
                    {
                        row += data[y * w + x];
                        integral[current + x + 1] = integral[above + x + 1] + row;
                    }
                }
            }

            Span<uint> histogram = stackalloc uint[256];
            histogram.Clear();
            foreach (var value in data)
                histogram[value]++;
            double total = data.Length;
            low = Percentile(histogram, total, 0.02);
            var high = Percentile(histogram, total, 0.995);
            range = PetalMath.Max(high - low, 8.0);
            radius = Math.Clamp(Math.Min(w, h) / 8, 12, 64);
        }

        public void Binarize(PetalLuma image, double sensitivity, Span<bool> mask)
        {
            var (w, h) = (image.Width, image.Height);
            var data = image.Data;
            var margin = PetalMath.Max(sensitivity * range, 5.0);
            var floor = low + 0.2 * range;
            var stride = w + 1;
            for (var y = 0; y < h; y++)
            {
                var (y0, y1) = (Math.Max(y - radius, 0), Math.Min(y + radius + 1, h));
                for (var x = 0; x < w; x++)
                {
                    var (x0, x1) = (Math.Max(x - radius, 0), Math.Min(x + radius + 1, w));
                    uint sum;
                    unchecked
                    {
                        sum = integral[y1 * stride + x1] + integral[y0 * stride + x0]
                            - integral[y0 * stride + x1]
                            - integral[y1 * stride + x0];
                    }

                    var mean = (double)sum / ((x1 - x0) * (y1 - y0));
                    double value = data[y * w + x];
                    mask[y * w + x] = value > mean + margin && value > floor;
                }
            }
        }

        public void Dispose()
        {
            if (integral is not null)
                ArrayPool<uint>.Shared.Return(integral);
        }

        private static double Percentile(ReadOnlySpan<uint> histogram, double total, double p)
        {
            var target = total * p;
            var seen = 0.0;
            for (var level = 0; level < 256; level++)
            {
                seen += histogram[level];
                if (seen >= target)
                    return level;
            }

            return 255.0;
        }
    }
}
