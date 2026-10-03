using System.Runtime.InteropServices;

namespace Hyperledger.Iroha.Petal;

/// <summary>
/// Floating-point helpers that reproduce the semantics of the Rust reference
/// (`crates/iroha_petal`) exactly, so every port makes the same decisions on
/// the same pixels.
/// </summary>
/// <remarks>
/// The reference never fuses multiply-adds and evaluates every expression left
/// to right in IEEE double precision; the C# code mirrors each expression
/// literally. Rust's <c>Iterator::sum</c> for floats starts from <c>-0.0</c>,
/// so ported sums start there too; it only matters for the sign of an all-zero
/// sum but keeps results bit-identical.
/// </remarks>
internal static class PetalMath
{
    /// <summary>Neutral element of Rust's float <c>Iterator::sum</c>.</summary>
    internal const double SumIdentity = -0.0;

    /// <summary>Rust <c>f64::total_cmp</c>: a total order over all bit patterns.</summary>
    internal static int TotalCompare(double a, double b)
    {
        var left = BitConverter.DoubleToInt64Bits(a);
        var right = BitConverter.DoubleToInt64Bits(b);
        left ^= (long)((ulong)(left >> 63) >> 1);
        right ^= (long)((ulong)(right >> 63) >> 1);
        return left.CompareTo(right);
    }

    /// <summary>Rust <c>f64::min</c>: returns the other operand when one is NaN.</summary>
    internal static double Min(double a, double b)
    {
        if (a < b)
            return a;
        if (b < a)
            return b;
        return double.IsNaN(a) ? b : a;
    }

    /// <summary>Rust <c>f64::max</c>: returns the other operand when one is NaN.</summary>
    internal static double Max(double a, double b)
    {
        if (a > b)
            return a;
        if (b > a)
            return b;
        return double.IsNaN(a) ? b : a;
    }

    /// <summary>Rust <c>f64::round</c>: halfway cases round away from zero.</summary>
    internal static double RoundHalfAwayFromZero(double value)
    {
        var truncated = Math.Truncate(value);
        if (Math.Abs(value - truncated) >= 0.5)
            truncated += Math.CopySign(1.0, value);
        return truncated;
    }

    /// <summary>
    /// Rust <c>value as usize</c> for a value already known to be at most
    /// <see cref="int.MaxValue"/>: NaN and negative values become zero.
    /// </summary>
    internal static int ToIndex(double value) => value > 0.0 ? (int)value : 0;

    /// <summary>
    /// Sorts <paramref name="values"/> ascending under <see cref="TotalCompare"/> (Rust
    /// <c>sort_by(f64::total_cmp)</c>) in place, without allocating.
    /// </summary>
    /// <remarks>
    /// The total order is the signed-integer order of the bit patterns once the magnitude bits
    /// of negative numbers are flipped; the flip is its own inverse, so the values are mapped
    /// to integer keys, sorted as integers and mapped back. Equal keys are bit-identical, so
    /// stability is irrelevant.
    /// </remarks>
    internal static void SortTotal(Span<double> values)
    {
        var keys = MemoryMarshal.Cast<double, long>(values);
        for (var i = 0; i < keys.Length; i++)
            keys[i] ^= (long)((ulong)(keys[i] >> 63) >> 1);
        keys.Sort();
        for (var i = 0; i < keys.Length; i++)
            keys[i] ^= (long)((ulong)(keys[i] >> 63) >> 1);
    }

    /// <summary>
    /// Stable ascending sort of <paramref name="order"/> by <paramref name="keys"/>
    /// under <see cref="TotalCompare"/> (Rust <c>sort_by</c> with <c>total_cmp</c>).
    /// </summary>
    internal static void StableSortByKey(Span<int> order, ReadOnlySpan<double> keys)
    {
        for (var i = 1; i < order.Length; i++)
        {
            var item = order[i];
            var key = keys[item];
            var j = i - 1;
            while (j >= 0 && TotalCompare(keys[order[j]], key) > 0)
            {
                order[j + 1] = order[j];
                j--;
            }

            order[j + 1] = item;
        }
    }
}
