namespace Hyperledger.Iroha.Norito;

/// <summary>
/// A Norito payload layout (<c>norito.md</c>, "Header Flags" and "Dense layout").
/// </summary>
/// <remarks>
/// A frame declares its layout in the header minor version and flag byte. The owner of a
/// headerless payload names the layout explicitly. Nothing infers a layout from payload bytes.
/// The numeric values of this enumeration are not wire values.
/// </remarks>
public enum NoritoWireLayout
{
    /// <summary>
    /// Header minor <c>0</c> with flags <c>0x02</c> (<c>COMPACT_LEN</c>): varint per-value
    /// lengths, fixed <c>u64</c> sequence counts. This is the default encode layout.
    /// </summary>
    Compact = 0,

    /// <summary>
    /// Header minor <c>0</c> with flags <c>0x00</c>: fixed-width <c>u64</c> per-value lengths.
    /// Frames are read and written; the typed readers and writers do not speak it.
    /// </summary>
    Fixed = 1,

    /// <summary>
    /// Header minor <c>1</c> with flags <c>0x00</c>: varint lengths and counts, raw byte
    /// sequences and arrays, varint enum discriminants and no header alignment padding.
    /// </summary>
    Dense = 2,
}

/// <summary>Header mapping and validation of <see cref="NoritoWireLayout"/>.</summary>
internal static class NoritoWireLayoutRules
{
    internal const byte CurrentMinor = 0;
    internal const byte DenseMinor = 1;
    internal const byte CompactLengthFlag = 0x02;

    /// <summary>The header minor version that declares <paramref name="layout"/>.</summary>
    internal static byte HeaderMinor(this NoritoWireLayout layout) => layout switch
    {
        NoritoWireLayout.Compact or NoritoWireLayout.Fixed => CurrentMinor,
        NoritoWireLayout.Dense => DenseMinor,
        _ => throw Undefined(layout, nameof(layout)),
    };

    /// <summary>The header flag byte that declares <paramref name="layout"/>.</summary>
    internal static byte HeaderFlags(this NoritoWireLayout layout) => layout switch
    {
        NoritoWireLayout.Compact => CompactLengthFlag,
        NoritoWireLayout.Fixed or NoritoWireLayout.Dense => 0,
        _ => throw Undefined(layout, nameof(layout)),
    };

    /// <summary>
    /// The layout a header's minor version and flag byte declare. Minor <c>0</c> admits flags
    /// <c>0x00</c> and <c>0x02</c>; minor <c>1</c> admits only <c>0x00</c>; no other minor exists.
    /// </summary>
    internal static bool TryFromHeader(byte minor, byte flags, out NoritoWireLayout layout)
    {
        switch (minor, flags)
        {
            case (CurrentMinor, CompactLengthFlag):
                layout = NoritoWireLayout.Compact;
                return true;
            case (CurrentMinor, 0):
                layout = NoritoWireLayout.Fixed;
                return true;
            case (DenseMinor, 0):
                layout = NoritoWireLayout.Dense;
                return true;
            default:
                layout = default;
                return false;
        }
    }

    /// <summary>
    /// Requires one of the two layouts the typed readers and writers implement.
    /// </summary>
    internal static NoritoWireLayout RequireTyped(NoritoWireLayout layout, string parameterName) =>
        layout switch
        {
            NoritoWireLayout.Compact or NoritoWireLayout.Dense => layout,
            NoritoWireLayout.Fixed => throw new ArgumentOutOfRangeException(
                parameterName,
                layout,
                "Typed Norito codecs speak the compact and dense layouts; fixed-width lengths are frame-level only."),
            _ => throw Undefined(layout, parameterName),
        };

    private static ArgumentOutOfRangeException Undefined(NoritoWireLayout layout, string parameterName) =>
        new(parameterName, layout, "Unknown Norito wire layout.");
}
