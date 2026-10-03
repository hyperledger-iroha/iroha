namespace Hyperledger.Iroha.Petal;

/// <summary>A 3×3 projective plane transform, stored row-major.</summary>
/// <remarks>
/// Every operation evaluates the same IEEE double expressions in the same
/// order as the Rust reference, so fitted transforms are bit-identical.
/// </remarks>
public readonly struct PetalHomography : IEquatable<PetalHomography>
{
    private readonly double m0;
    private readonly double m1;
    private readonly double m2;
    private readonly double m3;
    private readonly double m4;
    private readonly double m5;
    private readonly double m6;
    private readonly double m7;
    private readonly double m8;

    /// <summary>Creates a transform from its nine row-major elements.</summary>
    /// <param name="m0">Row 0, column 0.</param>
    /// <param name="m1">Row 0, column 1.</param>
    /// <param name="m2">Row 0, column 2.</param>
    /// <param name="m3">Row 1, column 0.</param>
    /// <param name="m4">Row 1, column 1.</param>
    /// <param name="m5">Row 1, column 2.</param>
    /// <param name="m6">Row 2, column 0.</param>
    /// <param name="m7">Row 2, column 1.</param>
    /// <param name="m8">Row 2, column 2.</param>
    public PetalHomography(
        double m0, double m1, double m2,
        double m3, double m4, double m5,
        double m6, double m7, double m8)
    {
        this.m0 = m0;
        this.m1 = m1;
        this.m2 = m2;
        this.m3 = m3;
        this.m4 = m4;
        this.m5 = m5;
        this.m6 = m6;
        this.m7 = m7;
        this.m8 = m8;
    }

    /// <summary>Creates a transform from nine row-major elements.</summary>
    /// <param name="elements">Exactly nine values.</param>
    /// <exception cref="ArgumentException">The span does not hold nine values.</exception>
    public PetalHomography(ReadOnlySpan<double> elements)
        : this(
            Element(elements, 0), Element(elements, 1), Element(elements, 2),
            Element(elements, 3), Element(elements, 4), Element(elements, 5),
            Element(elements, 6), Element(elements, 7), Element(elements, 8))
    {
        if (elements.Length != 9)
            throw new ArgumentException("A homography has nine elements.", nameof(elements));
    }

    /// <summary>The identity transform.</summary>
    public static PetalHomography Identity { get; } = new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);

    /// <summary>Element <paramref name="index"/> in row-major order.</summary>
    /// <param name="index">0 to 8.</param>
    public double this[int index] => index switch
    {
        0 => m0,
        1 => m1,
        2 => m2,
        3 => m3,
        4 => m4,
        5 => m5,
        6 => m6,
        7 => m7,
        8 => m8,
        _ => throw new ArgumentOutOfRangeException(nameof(index)),
    };

    /// <summary>The nine elements in row-major order.</summary>
    /// <returns>A fresh array.</returns>
    public double[] ToArray() => [m0, m1, m2, m3, m4, m5, m6, m7, m8];

    /// <summary>Maps a point.</summary>
    /// <param name="x">Source x.</param>
    /// <param name="y">Source y.</param>
    /// <returns>The projected point.</returns>
    public PetalPoint Apply(double x, double y)
    {
        Apply(x, y, out var px, out var py);
        return new PetalPoint(px, py);
    }

    /// <summary>Maps a point without constructing a <see cref="PetalPoint"/>.</summary>
    internal void Apply(double x, double y, out double px, out double py)
    {
        var w = m6 * x + m7 * y + m8;
        px = (m0 * x + m1 * y + m2) / w;
        py = (m3 * x + m4 * y + m5) / w;
    }

    /// <summary>The inverse transform, or <see langword="null"/> when singular.</summary>
    /// <returns>The inverse.</returns>
    public PetalHomography? Inverse()
    {
        var c00 = m4 * m8 - m5 * m7;
        var c01 = m5 * m6 - m3 * m8;
        var c02 = m3 * m7 - m4 * m6;
        var det = m0 * c00 + m1 * c01 + m2 * c02;
        if (Math.Abs(det) < 1e-18)
            return null;
        var inv = 1.0 / det;
        return new PetalHomography(
            c00 * inv,
            (m2 * m7 - m1 * m8) * inv,
            (m1 * m5 - m2 * m4) * inv,
            c01 * inv,
            (m0 * m8 - m2 * m6) * inv,
            (m2 * m3 - m0 * m5) * inv,
            c02 * inv,
            (m1 * m6 - m0 * m7) * inv,
            (m0 * m4 - m1 * m3) * inv);
    }

    /// <summary><c>this * other</c>: the transform that applies <paramref name="other"/> first.</summary>
    /// <param name="other">The transform applied first.</param>
    /// <returns>The composition.</returns>
    public PetalHomography Compose(in PetalHomography other)
    {
        Span<double> a = stackalloc double[9];
        Span<double> b = stackalloc double[9];
        CopyTo(a);
        other.CopyTo(b);
        Span<double> output = stackalloc double[9];
        for (var r = 0; r < 3; r++)
        {
            for (var c = 0; c < 3; c++)
            {
                var sum = PetalMath.SumIdentity;
                for (var k = 0; k < 3; k++)
                    sum += a[r * 3 + k] * b[k * 3 + c];
                output[r * 3 + c] = sum;
            }
        }

        return new PetalHomography(output);
    }

    /// <summary>
    /// Fits the homography taking <paramref name="source"/>[i] to
    /// <paramref name="destination"/>[i]: exact for four pairs, least squares
    /// for more, using Hartley-normalised DLT and an 8×8 Gaussian elimination.
    /// </summary>
    /// <param name="source">Source points.</param>
    /// <param name="destination">Destination points, same count, at least four.</param>
    /// <returns>The transform, or <see langword="null"/> for degenerate input.</returns>
    public static PetalHomography? FromPoints(ReadOnlySpan<PetalPoint> source, ReadOnlySpan<PetalPoint> destination)
    {
        if (source.Length != destination.Length || source.Length < 4)
            return null;
        var (ts, scaleS) = Normalisation(source);
        var (td, scaleD) = Normalisation(destination);
        Span<double> ata = stackalloc double[64];
        Span<double> atb = stackalloc double[8];
        ata.Clear();
        atb.Clear();
        Span<double> row = stackalloc double[8];
        for (var i = 0; i < source.Length; i++)
        {
            var (x, y) = ts.ApplyAffine(source[i].X, source[i].Y);
            var (u, v) = td.ApplyAffine(destination[i].X, destination[i].Y);
            row[0] = x;
            row[1] = y;
            row[2] = 1.0;
            row[3] = 0.0;
            row[4] = 0.0;
            row[5] = 0.0;
            row[6] = -u * x;
            row[7] = -u * y;
            Accumulate(ata, atb, row, u);
            row[0] = 0.0;
            row[1] = 0.0;
            row[2] = 0.0;
            row[3] = x;
            row[4] = y;
            row[5] = 1.0;
            row[6] = -v * x;
            row[7] = -v * y;
            Accumulate(ata, atb, row, v);
        }

        Span<double> h = stackalloc double[8];
        if (!Solve8(ata, atb, h))
            return null;
        var normalised = new PetalHomography(h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0);
        // H = Td^-1 * Hn * Ts
        var tdInverse = new PetalHomography(
            1.0 / scaleD,
            0.0,
            -td.m2 / scaleD,
            0.0,
            1.0 / scaleD,
            -td.m5 / scaleD,
            0.0,
            0.0,
            1.0);
        var tsMatrix = new PetalHomography(scaleS, 0.0, ts.m2, 0.0, scaleS, ts.m5, 0.0, 0.0, 1.0);
        var fitted = tdInverse.Compose(normalised).Compose(tsMatrix);
        var norm = fitted.m8;
        if (Math.Abs(norm) < 1e-15)
            return null;
        return new PetalHomography(
            fitted.m0 / norm,
            fitted.m1 / norm,
            fitted.m2 / norm,
            fitted.m3 / norm,
            fitted.m4 / norm,
            fitted.m5 / norm,
            fitted.m6 / norm,
            fitted.m7 / norm,
            fitted.m8 / norm);
    }

    /// <inheritdoc />
    public bool Equals(PetalHomography other) =>
        m0.Equals(other.m0) && m1.Equals(other.m1) && m2.Equals(other.m2)
        && m3.Equals(other.m3) && m4.Equals(other.m4) && m5.Equals(other.m5)
        && m6.Equals(other.m6) && m7.Equals(other.m7) && m8.Equals(other.m8);

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is PetalHomography other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(m0);
        hash.Add(m1);
        hash.Add(m2);
        hash.Add(m3);
        hash.Add(m4);
        hash.Add(m5);
        hash.Add(m6);
        hash.Add(m7);
        hash.Add(m8);
        return hash.ToHashCode();
    }

    /// <summary>Element-wise equality.</summary>
    /// <param name="left">Left operand.</param>
    /// <param name="right">Right operand.</param>
    /// <returns><see langword="true"/> when all nine elements are equal.</returns>
    public static bool operator ==(PetalHomography left, PetalHomography right) => left.Equals(right);

    /// <summary>Element-wise inequality.</summary>
    /// <param name="left">Left operand.</param>
    /// <param name="right">Right operand.</param>
    /// <returns><see langword="true"/> when any element differs.</returns>
    public static bool operator !=(PetalHomography left, PetalHomography right) => !left.Equals(right);

    /// <inheritdoc />
    public override string ToString() =>
        $"[{m0}, {m1}, {m2}; {m3}, {m4}, {m5}; {m6}, {m7}, {m8}]";

    private void CopyTo(Span<double> target)
    {
        target[0] = m0;
        target[1] = m1;
        target[2] = m2;
        target[3] = m3;
        target[4] = m4;
        target[5] = m5;
        target[6] = m6;
        target[7] = m7;
        target[8] = m8;
    }

    private (double X, double Y) ApplyAffine(double x, double y) => (m0 * x + m2, m4 * y + m5);

    private static double Element(ReadOnlySpan<double> elements, int index) =>
        index < elements.Length ? elements[index] : 0.0;

    private static void Accumulate(Span<double> ata, Span<double> atb, ReadOnlySpan<double> row, double rhs)
    {
        for (var i = 0; i < 8; i++)
        {
            for (var j = 0; j < 8; j++)
                ata[i * 8 + j] += row[i] * row[j];
            atb[i] += row[i] * rhs;
        }
    }

    /// <summary>Translation to the centroid and isotropic scale to mean distance √2.</summary>
    private static (PetalHomography Transform, double Scale) Normalisation(ReadOnlySpan<PetalPoint> points)
    {
        double n = points.Length;
        var cx = 0.0;
        var cy = 0.0;
        foreach (var p in points)
        {
            cx += p.X / n;
            cy += p.Y / n;
        }

        var total = PetalMath.SumIdentity;
        foreach (var p in points)
            total += Math.Sqrt((p.X - cx) * (p.X - cx) + (p.Y - cy) * (p.Y - cy));
        var mean = total / n;
        var scale = mean > 1e-12 ? Math.Sqrt(2.0) / mean : 1.0;
        return (new PetalHomography(scale, 0.0, -scale * cx, 0.0, scale, -scale * cy, 0.0, 0.0, 1.0), scale);
    }

    /// <summary>Gaussian elimination with partial pivoting for an 8×8 system.</summary>
    private static bool Solve8(Span<double> a, Span<double> b, Span<double> x)
    {
        for (var column = 0; column < 8; column++)
        {
            // Rust `max_by` keeps the last of equal maxima.
            var pivot = column;
            for (var candidate = column + 1; candidate < 8; candidate++)
            {
                if (PetalMath.TotalCompare(Math.Abs(a[pivot * 8 + column]), Math.Abs(a[candidate * 8 + column])) <= 0)
                    pivot = candidate;
            }

            if (Math.Abs(a[pivot * 8 + column]) < 1e-14)
                return false;
            if (pivot != column)
            {
                for (var k = 0; k < 8; k++)
                    (a[column * 8 + k], a[pivot * 8 + k]) = (a[pivot * 8 + k], a[column * 8 + k]);
                (b[column], b[pivot]) = (b[pivot], b[column]);
            }

            for (var row = column + 1; row < 8; row++)
            {
                var factor = a[row * 8 + column] / a[column * 8 + column];
                for (var k = column; k < 8; k++)
                    a[row * 8 + k] -= factor * a[column * 8 + k];
                b[row] -= factor * b[column];
            }
        }

        for (var row = 7; row >= 0; row--)
        {
            var tail = PetalMath.SumIdentity;
            for (var k = row + 1; k < 8; k++)
                tail += a[row * 8 + k] * x[k];
            x[row] = (b[row] - tail) / a[row * 8 + row];
        }

        return true;
    }
}
