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

/// <summary>One plausible set of corner finders for a frame.</summary>
public sealed class PetalFinderSet
{
    private readonly PetalFinder[] corners;

    internal PetalFinderSet(PetalFinder[] corners, int? inferred)
    {
        this.corners = corners;
        Corners = Array.AsReadOnly(corners);
        Inferred = inferred;
    }

    /// <summary>The four corners, clockwise from the one nearest the top-left of the image.</summary>
    public IReadOnlyList<PetalFinder> Corners { get; }

    /// <summary>
    /// Index into <see cref="Corners"/> of a corner that was not seen but inferred from the
    /// other three, or <see langword="null"/> when all four were seen.
    /// </summary>
    public int? Inferred { get; }

    /// <summary>The corners without a copy (callers must not modify them).</summary>
    internal PetalFinder[] CornerArray => corners;
}

/// <summary>
/// Finding the four corner finders in a camera luma plane.
/// </summary>
/// <remarks>
/// <para>
/// Pipeline: adaptive threshold (local mean via an integral image) → 4-connected
/// component labelling → blossom detection (a large, round, isolated blob) →
/// selection of the four finders that form a plausible, similarly sized
/// quadrilateral → intensity-weighted centre refinement. Solid blossoms survive
/// defocus that would fill in the gaps of a ring-shaped marker.
/// </para>
/// <para>
/// When a finger, a glare or the edge of the frame hides one blossom, three large
/// blossoms that form a corner still identify the code: the fourth corner is
/// inferred (and later refined by the decoder).
/// </para>
/// </remarks>
public static class PetalLocator
{
    private static readonly double[] SensitivityValues = [0.12, 0.22, 0.34];

    /// <summary>
    /// Binarisation thresholds, from the most to the least permissive: a higher sensitivity
    /// separates blurred blossoms from their surroundings.
    /// </summary>
    public static IReadOnlyList<double> Sensitivities { get; } = Array.AsReadOnly(SensitivityValues);

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
    public static PetalFinder[]? SelectQuad(ReadOnlySpan<PetalFinder> finders) =>
        SelectQuadFrom(StrongFinders(finders)) ?? SelectQuadFrom(finders);

    /// <summary>
    /// Chooses three finders that look like three corners of one code and completes the
    /// fourth corner as a parallelogram.
    /// </summary>
    /// <remarks>
    /// The three form an <c>L</c>: sizes within a factor 1.9, and one of them (the vertex)
    /// with two legs whose lengths are within a factor 2, at an angle with
    /// <c>|cos| ≤ 0.5</c>, and whose mean length is 4.8 to 10.5 mean finder sizes. The
    /// lowest <c>(size ratio − 1) + (leg ratio − 1) + |cos| + |mean leg / mean size − 7.33| / 7.33</c>
    /// wins (the first of equal scores); the fourth corner is <c>p + q − vertex</c> with the
    /// mean size of the three. Like <see cref="SelectQuad"/>, only the ten largest candidates
    /// are combined.
    /// </remarks>
    /// <param name="finders">Candidate finders.</param>
    /// <returns>
    /// The four corners clockwise from the top-left and the index of the inferred one among
    /// them, or <see langword="null"/>.
    /// </returns>
    public static (PetalFinder[] Quad, int Inferred)? SelectTriple(ReadOnlySpan<PetalFinder> finders)
    {
        var ranked = Ranked(finders);
        var n = ranked.Length;
        PetalFinder[]? best = null;
        var bestInferred = 0;
        var bestScore = 0.0;
        Span<PetalFinder> set = stackalloc PetalFinder[3];
        Span<PetalFinder> quad = stackalloc PetalFinder[4];
        for (var a = 0; a < n; a++)
        {
            for (var b = a + 1; b < n; b++)
            {
                for (var c = b + 1; c < n; c++)
                {
                    set[0] = ranked[a];
                    set[1] = ranked[b];
                    set[2] = ranked[c];
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
                    foreach (var finder in set)
                        sizeSum += finder.Size;
                    var meanSize = sizeSum / 3.0;
                    for (var corner = 0; corner < 3; corner++)
                    {
                        var k = set[corner];
                        var p = set[(corner + 1) % 3];
                        var q = set[(corner + 2) % 3];
                        var (ux, uy) = (p.X - k.X, p.Y - k.Y);
                        var (vx, vy) = (q.X - k.X, q.Y - k.Y);
                        var lu = Math.Sqrt(ux * ux + uy * uy);
                        var lv = Math.Sqrt(vx * vx + vy * vy);
                        if (lu <= 0.0 || lv <= 0.0)
                            continue;
                        var legs = PetalMath.Max(lu, lv) / PetalMath.Min(lu, lv);
                        var cos = (ux * vx + uy * vy) / (lu * lv);
                        // canvas geometry: side / finder diameter = 880 / 120
                        var ratio = 0.5 * (lu + lv) / meanSize;
                        if (legs > 2.0 || Math.Abs(cos) > 0.5 || !(ratio >= 4.8 && ratio <= 10.5))
                            continue;
                        var fourth = new PetalFinder(p.X + q.X - k.X, p.Y + q.Y - k.Y, meanSize);
                        quad[0] = k;
                        quad[1] = p;
                        quad[2] = q;
                        quad[3] = fourth;
                        var ordered = OrderClockwise(quad);
                        if (ordered is null)
                            continue;
                        var inferred = -1;
                        for (var i = 0; i < 4; i++)
                        {
                            if (BitConverter.DoubleToInt64Bits(ordered[i].X) == BitConverter.DoubleToInt64Bits(fourth.X)
                                && BitConverter.DoubleToInt64Bits(ordered[i].Y) == BitConverter.DoubleToInt64Bits(fourth.Y))
                            {
                                inferred = i;
                                break;
                            }
                        }

                        if (inferred < 0)
                            continue;
                        var score = (smax / smin - 1.0)
                            + (legs - 1.0)
                            + Math.Abs(cos)
                            + Math.Abs((ratio - 7.33) / 7.33);
                        if (best is null || score < bestScore)
                        {
                            best = ordered;
                            bestInferred = inferred;
                            bestScore = score;
                        }
                    }
                }
            }
        }

        return best is null ? null : (best, bestInferred);
    }

    /// <summary>Sharpens a finder centre with an intensity-weighted centroid.</summary>
    /// <param name="image">The luma plane.</param>
    /// <param name="finder">Coarse finder.</param>
    /// <returns>The refined finder (unchanged when the blob lacks contrast).</returns>
    public static PetalFinder RefineCenter(PetalLuma image, PetalFinder finder)
    {
        ArgumentNullException.ThrowIfNull(image);
        return Centroid(image, finder) ?? finder;
    }

    /// <summary>Re-finds a finder near where it is expected (from the previous frame's pose).</summary>
    /// <remarks>
    /// A first centroid over a disc twice the finder's diameter catches a blossom that moved
    /// up to about one diameter (nothing else bright is that close to a corner finder);
    /// centroids over the finder's own disc then repeat, at most five times, until the centre
    /// moves less than a quarter pixel.
    /// </remarks>
    /// <param name="image">The luma plane.</param>
    /// <param name="expected">Where the finder is expected, with its expected diameter.</param>
    /// <returns>
    /// The finder, or <see langword="null"/> when nothing bright is there or the result is more
    /// than 0.75 diameters from the expected centre, which means the code moved too far for
    /// tracking.
    /// </returns>
    public static PetalFinder? Follow(PetalLuma image, PetalFinder expected)
    {
        ArgumentNullException.ThrowIfNull(image);
        if (Centroid(image, expected with { Size = 2.0 * expected.Size }) is not { } wide)
            return null;
        var current = wide with { Size = expected.Size };
        for (var i = 0; i < 5; i++)
        {
            if (Centroid(image, current) is not { } next)
                return null;
            var step = Math.Sqrt((next.X - current.X) * (next.X - current.X) + (next.Y - current.Y) * (next.Y - current.Y));
            current = next;
            if (step < 0.25)
                break;
        }

        var moved = Math.Sqrt((current.X - expected.X) * (current.X - expected.X) + (current.Y - expected.Y) * (current.Y - expected.Y));
        return moved <= 0.75 * expected.Size ? current : null;
    }

    /// <summary>
    /// Locates four seen finders of a code: the first candidate of <see cref="Candidates"/>
    /// without an inferred corner.
    /// </summary>
    /// <param name="image">The luma plane.</param>
    /// <returns>Four refined finders clockwise from the top-left, or <see langword="null"/>.</returns>
    public static PetalFinder[]? Locate(PetalLuma image)
    {
        ArgumentNullException.ThrowIfNull(image);
        foreach (var set in CandidateSets(image))
        {
            if (set.Inferred is null)
                return set.CornerArray;
        }

        return null;
    }

    /// <summary>All candidate finder sets for one frame, in the order of <see cref="Candidates"/>.</summary>
    /// <param name="image">The luma plane.</param>
    /// <returns>The candidate sets.</returns>
    public static PetalFinderSet[] LocateCandidates(PetalLuma image)
    {
        ArgumentNullException.ThrowIfNull(image);
        return CandidateSets(image).ToArray();
    }

    /// <summary>
    /// Candidate finder sets for one frame, produced lazily in the order a decoder should try
    /// them, so that a clean frame costs one binarisation.
    /// </summary>
    /// <remarks>
    /// For each threshold of <see cref="Sensitivities"/> in turn: four finders of the largest
    /// size class that form a quad. Then, from the first threshold that had them, three large
    /// finders forming a corner — completed by the nearest smaller blob within 0.3 legs of
    /// where the fourth corner belongs (steep tilt makes the far finder small) — then the first
    /// quad that smaller blobs form, and last the same three finders with the fourth corner
    /// inferred (the nearby blob may have been merged ring dots, a quad may have been clutter).
    /// Seen corners are refined with <see cref="RefineCenter"/>; an inferred corner is not.
    /// </remarks>
    /// <param name="image">The luma plane.</param>
    /// <returns>The candidate sets, computed as they are enumerated.</returns>
    public static IEnumerable<PetalFinderSet> Candidates(PetalLuma image)
    {
        ArgumentNullException.ThrowIfNull(image);
        return CandidateSets(image);
    }

    private static IEnumerable<PetalFinderSet> CandidateSets(PetalLuma image)
    {
        if (image.Width == 0 || image.Height == 0)
            yield break;
        using var search = new CandidateSearch(image);
        foreach (var sensitivity in SensitivityValues)
        {
            if (search.Stage(sensitivity) is { } quad)
                yield return new PetalFinderSet(quad, null);
        }

        foreach (var set in search.Tail())
            yield return set;
    }

    /// <summary>
    /// The finders of the largest size class: lit tiles and merged dots form blob candidates
    /// too, but the corner finders are the biggest isolated round blobs in view.
    /// </summary>
    internal static PetalFinder[] StrongFinders(ReadOnlySpan<PetalFinder> finders)
    {
        var largest = 0.0;
        foreach (var finder in finders)
            largest = PetalMath.Max(largest, finder.Size);
        var strong = new List<PetalFinder>(finders.Length);
        foreach (var finder in finders)
        {
            if (finder.Size >= 0.55 * largest)
                strong.Add(finder);
        }

        return strong.ToArray();
    }

    /// <summary>
    /// A blob of at least 0.3 × the finder size within 0.3 legs of the inferred corner of a
    /// triple completes it into a seen quad.
    /// </summary>
    internal static PetalFinder[]? CompleteTriple(ReadOnlySpan<PetalFinder> finders, PetalFinder[] quad, int missing)
    {
        var d = quad[missing];
        var leg = 0.5 * (Distance(quad[(missing + 1) % 4], d) + Distance(quad[(missing + 3) % 4], d));
        var found = false;
        var fourth = default(PetalFinder);
        var nearest = 0.0;
        foreach (var finder in finders)
        {
            var distance = Distance(finder, d);
            if (!(finder.Size >= 0.3 * d.Size && distance <= 0.3 * leg))
                continue;
            // Rust `min_by` keeps the first of equal minima
            if (!found || PetalMath.TotalCompare(distance, nearest) < 0)
            {
                found = true;
                fourth = finder;
                nearest = distance;
            }
        }

        if (!found)
            return null;
        Span<PetalFinder> full = stackalloc PetalFinder[4];
        quad.CopyTo(full);
        full[missing] = fourth;
        return OrderClockwise(full);
    }

    /// <summary>
    /// The intensity-weighted centroid of the bright part of the disc of diameter
    /// <c>finder.Size</c> around the finder, or <see langword="null"/> when that disc has less
    /// than 20 levels of contrast (nothing bright is there).
    /// </summary>
    /// <remarks>
    /// A finder with a non-finite centre or size (a broken pose) has no centroid. Only the part
    /// of the enclosing square inside the image is visited, in raster order, with saturating
    /// bounds, so a huge or far-away finder never iterates over pixels that do not exist.
    /// </remarks>
    internal static PetalFinder? Centroid(PetalLuma image, PetalFinder finder)
    {
        if (!(double.IsFinite(finder.X) && double.IsFinite(finder.Y) && double.IsFinite(finder.Size)))
            return null;
        var half = finder.Size * 0.5;
        var radius = PetalMath.ToIsize(Math.Ceiling(half));
        var cx = PetalMath.ToIsize(Math.Floor(finder.X));
        var cy = PetalMath.ToIsize(Math.Floor(finder.Y));
        var x0 = Math.Max(PetalMath.SaturatingSubtract(cx, radius), 0L);
        var x1 = Math.Min(PetalMath.SaturatingAdd(cx, radius), image.Width - 1L);
        var y0 = Math.Max(PetalMath.SaturatingSubtract(cy, radius), 0L);
        var y1 = Math.Min(PetalMath.SaturatingAdd(cy, radius), image.Height - 1L);
        var data = image.Data;
        var width = image.Width;
        var floor = double.MaxValue;
        var peak = 0.0;
        for (var y = y0; y <= y1; y++)
        {
            for (var x = x0; x <= x1; x++)
            {
                var (px, py) = (x + 0.5, y + 0.5);
                if (Math.Sqrt((px - finder.X) * (px - finder.X) + (py - finder.Y) * (py - finder.Y)) <= half)
                {
                    double value = data[y * width + x];
                    floor = PetalMath.Min(floor, value);
                    peak = PetalMath.Max(peak, value);
                }
            }
        }

        if (peak - floor < 20.0)
            return null;
        var threshold = floor + 0.5 * (peak - floor);
        var (sw, sx, sy) = (0.0, 0.0, 0.0);
        for (var y = y0; y <= y1; y++)
        {
            for (var x = x0; x <= x1; x++)
            {
                var (px, py) = (x + 0.5, y + 0.5);
                if (Math.Sqrt((px - finder.X) * (px - finder.X) + (py - finder.Y) * (py - finder.Y)) <= half)
                {
                    double value = data[y * width + x];
                    var weight = PetalMath.Max(value - threshold, 0.0);
                    sw += weight;
                    sx += weight * px;
                    sy += weight * py;
                }
            }
        }

        return sw > 0.0 ? new PetalFinder(sx / sw, sy / sw, finder.Size) : null;
    }

    private static double Distance(PetalFinder f, PetalFinder d) =>
        Math.Sqrt((f.X - d.X) * (f.X - d.X) + (f.Y - d.Y) * (f.Y - d.Y));

    private static PetalFinder[] RefineAll(PetalLuma image, PetalFinder[] quad)
    {
        for (var i = 0; i < quad.Length; i++)
            quad[i] = RefineCenter(image, quad[i]);
        return quad;
    }

    /// <summary>
    /// The ten largest candidates, largest first (ties keep discovery order), so that clutter
    /// in a busy scene cannot push the real finders out of the set that is combined.
    /// </summary>
    private static PetalFinder[] Ranked(ReadOnlySpan<PetalFinder> candidates)
    {
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
        var ranked = new PetalFinder[Math.Min(candidates.Length, 10)];
        for (var i = 0; i < ranked.Length; i++)
            ranked[i] = candidates[order[i]];
        return ranked;
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
        var finders = Ranked(candidates);
        var n = finders.Length;
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

    /// <summary>
    /// The state of <see cref="Candidates"/> between sensitivities: the shared integral image,
    /// the binarisation buffer and the sets kept for after the last sensitivity.
    /// </summary>
    private sealed class CandidateSearch : IDisposable
    {
        private readonly PetalLuma image;
        private readonly int pixels;
        private LocalStatistics statistics;
        private bool[]? mask;
        private PetalFinder[]? completed;
        private PetalFinder[]? smaller;
        private PetalFinder[]? inferred;
        private int missing;

        public CandidateSearch(PetalLuma image)
        {
            this.image = image;
            pixels = image.Width * image.Height;
            // The integral image and percentiles do not depend on the sensitivity;
            // computing them once leaves every threshold decision unchanged.
            statistics = new LocalStatistics(image);
            mask = ArrayPool<bool>.Shared.Rent(pixels);
        }

        /// <summary>
        /// One sensitivity: records the triple (and its completion) and the quad of all
        /// candidates when none is recorded yet, and returns the refined quad of the large
        /// candidates, if any.
        /// </summary>
        public PetalFinder[]? Stage(double sensitivity)
        {
            ObjectDisposedException.ThrowIf(mask is null, this);
            var span = mask.AsSpan(0, pixels);
            statistics.Binarize(image, sensitivity, span);
            var components = Label(span, image.Width, image.Height);
            var finders = Blossoms(components);
            var strong = StrongFinders(finders);
            if (inferred is null && SelectTriple(strong) is var (quad, corner))
            {
                completed = CompleteTriple(finders, quad, corner) is { } full ? RefineAll(image, full) : null;
                var corners = (PetalFinder[])quad.Clone();
                for (var i = 0; i < corners.Length; i++)
                {
                    if (i != corner)
                        corners[i] = RefineCenter(image, corners[i]);
                }

                inferred = corners;
                missing = corner;
            }

            if (smaller is null && SelectQuadFrom(finders) is { } small)
                smaller = RefineAll(image, small);
            return SelectQuadFrom(strong) is { } large ? RefineAll(image, large) : null;
        }

        /// <summary>After the last sensitivity: the completed triple, the smaller quad, the inferred triple.</summary>
        public List<PetalFinderSet> Tail()
        {
            var tail = new List<PetalFinderSet>(3);
            if (completed is not null)
                tail.Add(new PetalFinderSet(completed, null));
            if (smaller is not null)
                tail.Add(new PetalFinderSet(smaller, null));
            if (inferred is not null)
                tail.Add(new PetalFinderSet(inferred, missing));
            return tail;
        }

        public void Dispose()
        {
            if (mask is null)
                return;
            ArrayPool<bool>.Shared.Return(mask);
            mask = null;
            statistics.Dispose();
        }
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
