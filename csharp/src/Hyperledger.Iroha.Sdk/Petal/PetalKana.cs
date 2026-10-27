using System.Buffers.Binary;
using System.Diagnostics.CodeAnalysis;

namespace Hyperledger.Iroha.Petal;

/// <summary>One of the four kana lanes of a card, in decode order (see <see cref="PetalKana"/>).</summary>
public enum PetalKanaLane
{
    /// <summary>The light or dark tiles of the band above the square: RS(37, 20), one atom.</summary>
    P0,

    /// <summary>The katakana of the band above the square: RS(149, 116), seven atoms.</summary>
    K0,

    /// <summary>The light or dark tiles of the band below the square: RS(37, 20), one atom.</summary>
    P1,

    /// <summary>The katakana of the band below the square: RS(149, 116), seven atoms.</summary>
    K1,
}

/// <summary>What one katakana tile shows.</summary>
/// <param name="Glyph">Glyph (symbol), 0 to 15 (see <see cref="PetalGlyphs.GlyphChars"/>).</param>
/// <param name="Light">
/// Light (filled with the light colour, glyph in ink) rather than dark (black, glyph in pink).
/// </param>
public readonly record struct PetalKanaSymbol(byte Glyph, bool Light);

/// <summary>A kana lane read from a capture, with the header (stream tag and frame) its check matched.</summary>
public sealed class PetalKanaResult : IEquatable<PetalKanaResult>
{
    internal PetalKanaResult(PetalLaneResult result, PetalLaneHeader header)
    {
        Result = result;
        Header = header;
    }

    /// <summary>The lane data (atoms and check), with the Reed–Solomon counters.</summary>
    public PetalLaneResult Result { get; }

    /// <summary>The header the check matched: the square's frame, or the frame after or before it.</summary>
    public PetalLaneHeader Header { get; }

    /// <inheritdoc />
    public bool Equals(PetalKanaResult? other) => other is not null && Header == other.Header && Result.Equals(other.Result);

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as PetalKanaResult);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(Result, Header);
}

/// <summary>What a band reader decided for one data tile (see <see cref="PetalKana.ObserveBand"/>).</summary>
/// <param name="Tile">Index into <see cref="PetalLayout.KanaTiles"/>.</param>
/// <param name="Light">Decided polarity.</param>
/// <param name="LightMargin">How far the tile's mean level lay from the threshold, in half-contrasts.</param>
/// <param name="Glyph">Decided glyph, 0 to 15.</param>
/// <param name="GlyphLead">How far the glyph's correlation led the runner-up's.</param>
public readonly record struct PetalTileReading(int Tile, bool Light, double LightMargin, byte Glyph, double GlyphLead);

/// <summary>What a band reader saw of one band (see <see cref="PetalKana.ObserveBand"/>).</summary>
public sealed class PetalBandReading
{
    internal PetalBandReading(double blur, int pilots, PetalTileReading?[] tiles)
    {
        Blur = blur;
        Pilots = pilots;
        Tiles = Array.AsReadOnly(tiles);
    }

    /// <summary>The blur (Gaussian σ in canvas units) the pilots chose.</summary>
    public double Blur { get; }

    /// <summary>Pilots found.</summary>
    public int Pilots { get; }

    /// <summary>Every data tile of the band in data order, or <see langword="null"/> for a tile that was not read.</summary>
    public IReadOnlyList<PetalTileReading?> Tiles { get; }
}

/// <summary>
/// The kana layer: data in the katakana tiles of the bands above and below the square.
/// </summary>
/// <remarks>
/// <para>
/// The card (<see cref="PetalLayout.CardHeight"/>) is the square code with a band of katakana tiles
/// above and below it, in the part of an upright phone's camera frame that the square leaves
/// unused. Each band holds <see cref="PetalLayout.KanaBandTiles"/> tiles
/// (<see cref="PetalLayout.KanaTiles"/>); eight are pilots with fixed symbols
/// (<see cref="PetalLayout.KanaPilotSymbols"/>) and the other <see cref="KanaDataTiles"/> carry two
/// lanes:
/// </para>
/// <list type="table">
/// <listheader><term>lane</term><description>cells / codeword / data / parity</description></listheader>
/// <item><term>P0, P1 (polarity)</term><description>296 data tiles × 1 bit (light or dark) / 37 B / 20 B / 17 B</description></item>
/// <item><term>K0, K1 (glyph)</term><description>298 data tiles × 4 bits (one of sixteen katakana) / 149 B / 116 B / 33 B</description></item>
/// </list>
/// <para>
/// A light tile is filled light with its glyph in ink; a dark tile is black with its glyph in pink.
/// Data tile <c>i</c> of a band (its tiles that are not pilots, in reading order) shows bit
/// <c>i</c> of the band's polarity codeword (most significant bit first; the last two data tiles,
/// beyond the codeword, are light) and nibble <c>i</c> of its glyph codeword (the high nibble of
/// byte <c>i / 2</c> for even <c>i</c>).
/// </para>
/// <para>
/// Every kana lane is one Reed–Solomon codeword, XOR-whitened: fountain atoms from their own repair
/// range (<see cref="KanaIdBase"/>, see <see cref="KanaFirstId"/>) and a CRC-32C check that binds
/// them to the stream tag, the frame and the lane (<see cref="KanaCheck"/>). The lanes have no
/// header: a reader takes the frame from the square's lanes and also tries the neighbouring
/// frames, because a rolling shutter or a stale frame can show a band from the frame before or
/// after the square.
/// </para>
/// <para>
/// <see cref="ReadKana"/> reads the bands of a decoded capture. The square's pose, extended beyond
/// the square, places every tile; each band's pilots are found by a small search around their
/// places, and an affine correction fitted to their offsets moves every tile of the band. The
/// pilots also pick the blur of the glyph templates and the levels that tell light tiles from dark
/// ones. Each data tile then takes the polarity its mean level shows and the glyph whose template
/// correlates best with its samples. The reader repeats the reference's IEEE double operations in
/// the same order; its templates are built once, on first use, and its buffers are reused, so a read
/// allocates next to nothing.
/// </para>
/// </remarks>
public static class PetalKana
{
    /// <summary>Data tiles of each band.</summary>
    public const int KanaDataTiles = PetalLayout.KanaBandTiles - PetalLayout.KanaPilots;

    /// <summary>Codeword length of a glyph lane in bytes (two data tiles per byte).</summary>
    public const int KanaGlyphWordLength = KanaDataTiles / 2;

    /// <summary>Parity bytes of a glyph lane.</summary>
    public const int KanaGlyphParityLength = 33;

    /// <summary>Atoms carried by each glyph lane.</summary>
    public const int KanaGlyphAtoms = 7;

    /// <summary>Codeword length of a polarity lane in bytes (eight data tiles per byte).</summary>
    public const int KanaPolarityWordLength = KanaDataTiles / 8;

    /// <summary>Parity bytes of a polarity lane.</summary>
    public const int KanaPolarityParityLength = 17;

    /// <summary>Atoms carried by each polarity lane.</summary>
    public const int KanaPolarityAtoms = 1;

    /// <summary>Bytes of the check that closes a kana lane's data.</summary>
    public const int KanaCheckLength = 4;

    /// <summary>
    /// Fountain id of the first kana atom of frame 0, just above the turn lanes' range
    /// (<c>2³¹ + 2²⁰</c>).
    /// </summary>
    public const uint KanaIdBase = PetalTurns.TurnIdBase + (1u << 20);

    /// <summary>Samples per tile side: points <c>(i + ½) · 25 / 12</c> units from the tile's top-left corner.</summary>
    public const int KanaGrid = 12;

    /// <summary>Samples per tile side in the coarse pilot search: points <c>(i + ½) · 25 / 6</c>.</summary>
    public const int KanaCoarseGrid = 6;

    /// <summary>Kana atom ids reserved per frame (eight per band).</summary>
    private const uint KanaIdsPerFrame = 16;

    /// <summary>Step of the coarse pilot search in canvas units.</summary>
    private const double CoarseStep = 2.0;

    /// <summary>A pilot whose coarse correlation is below this is not followed further.</summary>
    private const double PilotCoarseMin = 0.35;

    /// <summary>A pilot whose final correlation is below this is not used.</summary>
    private const double PilotMin = 0.5;

    private const int Bands = PetalLayout.KanaBands;

    private const int Pilots = PetalLayout.KanaPilots;

    private static readonly double[] KanaBlurValues = [1.0, 1.6, 2.4, 3.4, 4.8];
    private static readonly PetalKanaLane[] LaneValues = [PetalKanaLane.P0, PetalKanaLane.K0, PetalKanaLane.P1, PetalKanaLane.K1];
    private static readonly PetalReedSolomon PolarityCode = new(KanaPolarityParityLength);
    private static readonly PetalReedSolomon GlyphCode = new(KanaGlyphParityLength);
    private static readonly byte[][] Whitenings = BuildWhitenings();
    private static readonly double[] RefineCoarse = [1.0];
    private static readonly double[] RefineFine = [1.0, 0.5, 0.25];

    /// <summary>The four kana lanes in decode order: <c>P0</c>, <c>K0</c>, <c>P1</c>, <c>K1</c>.</summary>
    public static IReadOnlyList<PetalKanaLane> Lanes { get; } = Array.AsReadOnly(LaneValues);

    /// <summary>
    /// Gaussian blurs (σ in canvas units) of the glyph templates, sharpest first; the pilots pick one
    /// per band.
    /// </summary>
    public static IReadOnlyList<double> KanaBlurs { get; } = Array.AsReadOnly(KanaBlurValues);

    /// <summary>The polarity (<paramref name="glyph"/> false) or glyph lane of <paramref name="band"/>.</summary>
    /// <param name="band">Band: 0 above the square, 1 below it.</param>
    /// <param name="glyph">Whether to name the glyph lane rather than the polarity lane.</param>
    /// <returns>The lane.</returns>
    public static PetalKanaLane Lane(int band, bool glyph)
    {
        if ((uint)band >= Bands)
            throw new ArgumentOutOfRangeException(nameof(band), "A band is 0 (above the square) or 1 (below it).");
        return (PetalKanaLane)(2 * band + (glyph ? 1 : 0));
    }

    /// <summary>The band of <paramref name="lane"/>: 0 above the square, 1 below it.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>0 or 1.</returns>
    public static int Band(PetalKanaLane lane) => (int)CheckLane(lane) / 2;

    /// <summary>Whether <paramref name="lane"/> carries the tiles' katakana (<c>K</c>) rather than their polarity (<c>P</c>).</summary>
    /// <param name="lane">The lane.</param>
    /// <returns><see langword="true"/> for <c>K0</c> and <c>K1</c>.</returns>
    public static bool IsGlyph(PetalKanaLane lane) => (int)CheckLane(lane) % 2 == 1;

    /// <summary>Codeword length of <paramref name="lane"/> in bytes.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>37 for a polarity lane, 149 for a glyph lane.</returns>
    public static int WordLength(PetalKanaLane lane) => IsGlyph(lane) ? KanaGlyphWordLength : KanaPolarityWordLength;

    /// <summary>Parity bytes of <paramref name="lane"/>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>17 for a polarity lane, 33 for a glyph lane.</returns>
    public static int ParityLength(PetalKanaLane lane) => IsGlyph(lane) ? KanaGlyphParityLength : KanaPolarityParityLength;

    /// <summary>Data bytes of <paramref name="lane"/>: the atoms, then their check.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>20 for a polarity lane, 116 for a glyph lane.</returns>
    public static int DataLength(PetalKanaLane lane) => WordLength(lane) - ParityLength(lane);

    /// <summary>Atoms carried by <paramref name="lane"/>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>1 for a polarity lane, 7 for a glyph lane.</returns>
    public static int Atoms(PetalKanaLane lane) => IsGlyph(lane) ? KanaGlyphAtoms : KanaPolarityAtoms;

    /// <summary>The byte that names <paramref name="lane"/> in its check: <c>0x10 | band &lt;&lt; 1 | kind</c> (polarity 0, glyph 1).</summary>
    /// <param name="lane">The lane.</param>
    /// <returns><c>0x10</c> to <c>0x13</c>.</returns>
    public static byte Code(PetalKanaLane lane) => (byte)(0x10 | (int)CheckLane(lane));

    /// <summary>The lane's name in reports: <c>P0</c>, <c>K0</c>, <c>P1</c> or <c>K1</c>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>The two-character name.</returns>
    public static string Name(PetalKanaLane lane) => CheckLane(lane) switch
    {
        PetalKanaLane.P0 => "P0",
        PetalKanaLane.K0 => "K0",
        PetalKanaLane.P1 => "P1",
        _ => "K1",
    };

    /// <summary>
    /// The whitening sequence of <paramref name="lane"/>: one byte per codeword byte from a
    /// <see cref="PetalXorshift32"/> seeded <c>0x504F4C30 + band</c> ("POL0", "POL1") for polarity
    /// and <c>0x4B414E30 + band</c> ("KAN0", "KAN1") for glyphs.
    /// </summary>
    /// <param name="lane">The lane.</param>
    /// <returns>A fresh copy of the <see cref="WordLength"/>-byte sequence.</returns>
    public static byte[] Whitening(PetalKanaLane lane) => (byte[])Whitenings[(int)CheckLane(lane)].Clone();

    /// <summary>
    /// Fountain id of the first atom <paramref name="lane"/> carries in <paramref name="frame"/>:
    /// <c>KanaIdBase + 16 · frame + 8 · band</c>, plus 7 for the polarity lane (the glyph lane's seven
    /// atoms come first).
    /// </summary>
    /// <param name="frame">Frame counter.</param>
    /// <param name="lane">The lane.</param>
    /// <returns>The id.</returns>
    public static uint KanaFirstId(ushort frame, PetalKanaLane lane)
    {
        var slot = IsGlyph(lane) ? 0u : KanaGlyphAtoms;
        return KanaIdBase + frame * KanaIdsPerFrame + (uint)Band(lane) * 8 + slot;
    }

    /// <summary>
    /// The check of a kana lane: CRC-32C over the stream tag, the frame (big-endian), the lane's
    /// <see cref="Code"/> and the atoms.
    /// </summary>
    /// <param name="header">The stream tag and frame the lane is bound to.</param>
    /// <param name="lane">The lane.</param>
    /// <param name="atoms">The atoms.</param>
    /// <returns>The check.</returns>
    public static uint KanaCheck(PetalLaneHeader header, PetalKanaLane lane, ReadOnlySpan<byte> atoms)
    {
        var code = Code(lane);
        var length = 4 + atoms.Length;
        Span<byte> bytes = length <= 256 ? stackalloc byte[length] : new byte[length];
        bytes[0] = header.Tag;
        BinaryPrimitives.WriteUInt16BigEndian(bytes[1..], header.Frame);
        bytes[3] = code;
        atoms.CopyTo(bytes[4..]);
        return PetalCrc32C.Compute(bytes);
    }

    /// <summary>The data of a kana lane: the atoms followed by their <see cref="KanaCheck"/> (big-endian).</summary>
    /// <param name="header">The stream tag and frame the lane is bound to.</param>
    /// <param name="lane">The lane.</param>
    /// <param name="atoms">The lane's <see cref="Atoms"/> atoms, 16 bytes each.</param>
    /// <returns><see cref="DataLength"/> bytes.</returns>
    /// <exception cref="ArgumentException"><paramref name="atoms"/> is not the lane's atom count long.</exception>
    public static byte[] KanaData(PetalLaneHeader header, PetalKanaLane lane, ReadOnlySpan<byte> atoms)
    {
        if (atoms.Length != Atoms(lane) * PetalLanes.AtomLength)
            throw new ArgumentException("A kana lane carries its atom count of 16-byte atoms.", nameof(atoms));
        var data = new byte[DataLength(lane)];
        atoms.CopyTo(data);
        BinaryPrimitives.WriteUInt32BigEndian(data.AsSpan(atoms.Length), KanaCheck(header, lane, atoms));
        return data;
    }

    /// <summary>The atoms of kana lane data whose check matches <paramref name="header"/> and <paramref name="lane"/>.</summary>
    /// <param name="header">The stream tag and frame to check against.</param>
    /// <param name="lane">The lane the data was read from.</param>
    /// <param name="data">Decoded kana lane data.</param>
    /// <returns>The atoms with their ids, or <see langword="null"/> for a wrong length or a check that does not match.</returns>
    public static PetalAtomPacket? KanaPacket(PetalLaneHeader header, PetalKanaLane lane, ReadOnlySpan<byte> data)
    {
        if (!Checks(header, lane, data))
            return null;
        var atoms = new byte[Atoms(lane)][];
        for (var i = 0; i < atoms.Length; i++)
            atoms[i] = data.Slice(i * PetalLanes.AtomLength, PetalLanes.AtomLength).ToArray();
        return PetalAtomPacket.Owned(header, KanaFirstId(header.Frame, lane), atoms);
    }

    /// <summary>Encodes kana lane data into the transmitted (whitened) codeword.</summary>
    /// <param name="lane">The lane.</param>
    /// <param name="data">Exactly <see cref="DataLength"/> bytes.</param>
    /// <returns>The transmitted <see cref="WordLength"/>-byte codeword.</returns>
    /// <exception cref="ArgumentException">The data length does not match the lane.</exception>
    public static byte[] EncodeKanaLane(PetalKanaLane lane, ReadOnlySpan<byte> data)
    {
        if (data.Length != DataLength(lane))
            throw new ArgumentException("Kana lane data length mismatch.", nameof(data));
        var word = new byte[WordLength(lane)];
        data.CopyTo(word);
        CodeOf(lane).EncodeParity(data, word.AsSpan(data.Length));
        var whitening = Whitenings[(int)lane];
        for (var i = 0; i < word.Length; i++)
            word[i] ^= whitening[i];
        return word;
    }

    /// <summary>
    /// Decodes a transmitted kana codeword, also reporting how many byte positions the Reed–Solomon
    /// decoder rewrote. The check is not verified here (see <see cref="KanaPacket"/>).
    /// </summary>
    /// <param name="lane">The lane.</param>
    /// <param name="transmitted">The received, still whitened codeword.</param>
    /// <param name="erasures">Byte positions the caller distrusts.</param>
    /// <param name="data">The lane data on success.</param>
    /// <param name="corrected">The errata count on success (0 for a clean word).</param>
    /// <returns>
    /// <see langword="false"/> when the word has the wrong length, the erasure list is invalid, or
    /// the word is uncorrectable.
    /// </returns>
    public static bool TryDecodeKanaLane(
        PetalKanaLane lane,
        ReadOnlySpan<byte> transmitted,
        ReadOnlySpan<int> erasures,
        [NotNullWhen(true)] out byte[]? data,
        out int corrected)
    {
        data = null;
        corrected = 0;
        if (transmitted.Length != WordLength(lane))
            return false;
        Span<byte> word = stackalloc byte[transmitted.Length];
        var whitening = Whitenings[(int)lane];
        for (var i = 0; i < word.Length; i++)
            word[i] = (byte)(transmitted[i] ^ whitening[i]);
        if (!CodeOf(lane).TryDecode(word, erasures, out corrected))
            return false;
        data = word[..DataLength(lane)].ToArray();
        return true;
    }

    /// <summary>The symbols of every tile: the pilots' fixed symbols, every data tile light with glyph 0.</summary>
    /// <returns>A fresh array of <see cref="PetalLayout.KanaTileCount"/> symbols in tile order.</returns>
    public static PetalKanaSymbol[] PilotSymbols()
    {
        var symbols = new PetalKanaSymbol[PetalLayout.KanaTileCount];
        FillPilotSymbols(symbols);
        return symbols;
    }

    /// <summary>
    /// Shows the transmitted codewords of <paramref name="band"/> on its data tiles:
    /// <paramref name="polarity"/> (37 bytes) and <paramref name="glyphs"/> (149 bytes). Data tile
    /// <c>i</c> shows bit <c>i</c> of the polarity codeword (the last two tiles, beyond it, are
    /// light) and nibble <c>i</c> of the glyph codeword.
    /// </summary>
    /// <param name="symbols">The symbols of every tile, in tile order.</param>
    /// <param name="band">Band, 0 or 1.</param>
    /// <param name="polarity">The band's transmitted polarity codeword.</param>
    /// <param name="glyphs">The band's transmitted glyph codeword.</param>
    /// <exception cref="ArgumentException">A length is wrong.</exception>
    public static void SetKanaSymbols(Span<PetalKanaSymbol> symbols, int band, ReadOnlySpan<byte> polarity, ReadOnlySpan<byte> glyphs)
    {
        CheckSymbols(symbols.Length, nameof(symbols));
        var data = DataTiles(band);
        if (polarity.Length != KanaPolarityWordLength)
            throw new ArgumentException("A polarity codeword is 37 bytes.", nameof(polarity));
        if (glyphs.Length != KanaGlyphWordLength)
            throw new ArgumentException("A glyph codeword is 149 bytes.", nameof(glyphs));
        for (var i = 0; i < data.Length; i++)
        {
            var light = i >= KanaPolarityWordLength * 8 || ((polarity[i / 8] >> (7 - i % 8)) & 1) == 1;
            var value = glyphs[i / 2];
            var glyph = (byte)(i % 2 == 0 ? value >> 4 : value & 0x0F);
            symbols[data[i]] = new PetalKanaSymbol(glyph, light);
        }
    }

    /// <summary>Packs the polarity codeword the data tiles of <paramref name="band"/> show.</summary>
    /// <param name="symbols">The symbols of every tile, in tile order.</param>
    /// <param name="band">Band, 0 or 1.</param>
    /// <returns>37 bytes.</returns>
    public static byte[] PolarityWord(ReadOnlySpan<PetalKanaSymbol> symbols, int band)
    {
        CheckSymbols(symbols.Length, nameof(symbols));
        var data = DataTiles(band);
        var word = new byte[KanaPolarityWordLength];
        for (var i = 0; i < KanaPolarityWordLength * 8; i++)
        {
            if (symbols[data[i]].Light)
                word[i / 8] |= (byte)(1 << (7 - i % 8));
        }

        return word;
    }

    /// <summary>Packs the glyph codeword the data tiles of <paramref name="band"/> show.</summary>
    /// <param name="symbols">The symbols of every tile, in tile order.</param>
    /// <param name="band">Band, 0 or 1.</param>
    /// <returns>149 bytes.</returns>
    public static byte[] GlyphWord(ReadOnlySpan<PetalKanaSymbol> symbols, int band)
    {
        CheckSymbols(symbols.Length, nameof(symbols));
        var data = DataTiles(band);
        var word = new byte[KanaGlyphWordLength];
        for (var i = 0; i < data.Length; i++)
        {
            var nibble = symbols[data[i]].Glyph & 0x0F;
            word[i / 2] |= (byte)(i % 2 == 0 ? nibble << 4 : nibble);
        }

        return word;
    }

    /// <summary>
    /// The template value of <paramref name="glyph"/> (a light or dark tile) under blur
    /// <paramref name="blur"/> (an index into <see cref="KanaBlurs"/>) at fine grid point
    /// <c>(i, j)</c>, mean-centred and scaled to unit length: what conformance tests compare an SDK's
    /// tables with.
    /// </summary>
    /// <param name="blur">Index into <see cref="KanaBlurs"/>.</param>
    /// <param name="glyph">Glyph, 0 to 15.</param>
    /// <param name="light">A light tile's template (the inside less the ink) rather than a dark tile's (the ink).</param>
    /// <param name="i">Grid column, 0 to 11.</param>
    /// <param name="j">Grid row, 0 to 11.</param>
    /// <returns>The template value.</returns>
    public static double TemplateProbe(int blur, int glyph, bool light, int i, int j)
    {
        if ((uint)blur >= (uint)KanaBlurValues.Length)
            throw new ArgumentOutOfRangeException(nameof(blur));
        CheckProbe(glyph, i, j, KanaGrid);
        var table = light ? ReaderTables.Light : ReaderTables.Dark;
        return table[ReaderTables.Offset(blur, glyph) + j * KanaGrid + i];
    }

    /// <summary>
    /// The coarse template value of <paramref name="glyph"/> (a light or dark tile) at coarse grid
    /// point <c>(i, j)</c>: the template under blur 3.4 at the <see cref="KanaCoarseGrid"/> points,
    /// mean-centred and scaled to unit length.
    /// </summary>
    /// <param name="glyph">Glyph, 0 to 15.</param>
    /// <param name="light">A light tile's template rather than a dark tile's.</param>
    /// <param name="i">Coarse grid column, 0 to 5.</param>
    /// <param name="j">Coarse grid row, 0 to 5.</param>
    /// <returns>The template value.</returns>
    public static double CoarseTemplateProbe(int glyph, bool light, int i, int j)
    {
        CheckProbe(glyph, i, j, KanaCoarseGrid);
        var table = light ? ReaderTables.CoarseLight : ReaderTables.CoarseDark;
        return table[glyph * ReaderTables.CoarsePoints + j * KanaCoarseGrid + i];
    }

    /// <summary>
    /// Reads the kana lanes of the bands not flagged in <paramref name="skip"/> of a capture whose
    /// square <paramref name="frame"/> decoded under <paramref name="header"/> (the stream tag and
    /// frame its lanes name).
    /// </summary>
    /// <remarks>
    /// A lane passes when its Reed–Solomon codeword decodes, with 0, ⅛, ¼, ⅓ or ½ of its parity
    /// bytes' worth of its least confident bytes erased (polarity 0, 2, 4, 5, 8; glyphs 0, 4, 8, 11,
    /// 16), into data whose <see cref="KanaCheck"/> matches the header's frame, the frame after it or
    /// the frame before it (in that order). Never throws on image content.
    /// </remarks>
    /// <param name="image">The luma plane <paramref name="frame"/> was decoded from.</param>
    /// <param name="frame">The decoded square.</param>
    /// <param name="header">The stream tag and frame the square's lanes name.</param>
    /// <param name="skip">Bands not to read (two flags, band 0 first); empty reads both.</param>
    /// <returns>Four results in <see cref="Lanes"/> order (<c>P0</c>, <c>K0</c>, <c>P1</c>, <c>K1</c>), <see langword="null"/> where no lane was read.</returns>
    /// <exception cref="ArgumentException"><paramref name="skip"/> is neither empty nor two flags long.</exception>
    public static PetalKanaResult?[] ReadKana(PetalLuma image, PetalDecodedFrame frame, PetalLaneHeader header, ReadOnlySpan<bool> skip = default)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(frame);
        if (!skip.IsEmpty && skip.Length != Bands)
            throw new ArgumentException("Skip flags name the two bands.", nameof(skip));
        var results = new PetalKanaResult?[LaneValues.Length];
        var reader = Reader.Lease();
        try
        {
            for (var band = 0; band < Bands; band++)
            {
                if (skip.IsEmpty || !skip[band])
                    reader.ReadBand(image, frame, header, band, results);
            }
        }
        finally
        {
            Reader.Return(reader);
        }

        return results;
    }

    /// <summary>
    /// What <see cref="ReadKana"/> sees of band <paramref name="band"/> before any Reed–Solomon
    /// correction, for diagnostics.
    /// </summary>
    /// <param name="image">The luma plane <paramref name="frame"/> was decoded from.</param>
    /// <param name="frame">The decoded square.</param>
    /// <param name="band">Band, 0 or 1.</param>
    /// <returns>
    /// The band's readings, or <see langword="null"/> when fewer than two pilots are found or the
    /// pilots show no contrast between light and dark tiles.
    /// </returns>
    public static PetalBandReading? ObserveBand(PetalLuma image, PetalDecodedFrame frame, int band)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(frame);
        if ((uint)band >= Bands)
            throw new ArgumentOutOfRangeException(nameof(band), "A band is 0 (above the square) or 1 (below it).");
        var reader = Reader.Lease();
        try
        {
            if (!reader.Observe(image, frame, band))
                return null;
            var data = DataTiles(band);
            var tiles = new PetalTileReading?[KanaDataTiles];
            for (var i = 0; i < tiles.Length; i++)
            {
                if (reader.TileRead[i])
                    tiles[i] = new PetalTileReading(data[i], reader.TileLight[i], reader.TileMargin[i], reader.TileGlyph[i], reader.TileLead[i]);
            }

            return new PetalBandReading(KanaBlurValues[reader.Blur], reader.FoundCount, tiles);
        }
        finally
        {
            Reader.Return(reader);
        }
    }

    /// <summary>Fills <paramref name="symbols"/> with the pilots' fixed symbols and every data tile light with glyph 0.</summary>
    internal static void FillPilotSymbols(Span<PetalKanaSymbol> symbols)
    {
        symbols.Fill(new PetalKanaSymbol(0, true));
        var pilots = PetalLayout.TilePilotTable;
        var fixedSymbols = PetalLayout.KanaPilotSymbolTable;
        for (var tile = 0; tile < pilots.Length; tile++)
        {
            if (pilots[tile] >= 0)
                symbols[tile] = fixedSymbols[pilots[tile]];
        }
    }

    /// <summary>
    /// Tries Reed–Solomon with 0, n/8, n/4, n/3 and n/2 of the least confident bytes erased (n the
    /// parity bytes; ties keep byte order, a stable sort like Rust's <c>sort_by</c>) and accepts the
    /// first result whose check matches the square's header or the frame after or before it.
    /// </summary>
    internal static PetalKanaResult? DecodeKanaWord(PetalKanaLane lane, PetalLaneHeader header, ReadOnlySpan<byte> word, ReadOnlySpan<double> confidence)
    {
        var length = WordLength(lane);
        Span<int> order = stackalloc int[length];
        for (var i = 0; i < order.Length; i++)
            order[i] = i;
        PetalMath.StableSortByKey(order, confidence);
        var nsym = ParityLength(lane);
        Span<int> schedule = [0, nsym / 8, nsym / 4, nsym / 3, nsym / 2];
        Span<byte> trial = stackalloc byte[length];
        var previous = -1;
        foreach (var erasures in schedule)
        {
            // Rust `dedup` drops consecutive repeats.
            if (erasures == previous)
                continue;
            previous = erasures;
            var positions = order[..erasures];
            word.CopyTo(trial);
            foreach (var position in positions)
                trial[position] = 0;
            if (!TryDecodeKanaLane(lane, trial, positions, out var data, out var corrected))
                continue;
            for (var step = 0; step < 3; step++)
            {
                // the square's frame, the frame after it, the frame before it
                var frame = step switch
                {
                    0 => header.Frame,
                    1 => unchecked((ushort)(header.Frame + 1)),
                    _ => unchecked((ushort)(header.Frame - 1)),
                };
                var candidate = header with { Frame = frame };
                if (Checks(candidate, lane, data))
                    return new PetalKanaResult(new PetalLaneResult(data, corrected, positions.Length), candidate);
            }
        }

        return null;
    }

    /// <summary>The data tiles of <paramref name="band"/> in data order.</summary>
    private static ReadOnlySpan<int> DataTiles(int band)
    {
        if ((uint)band >= Bands)
            throw new ArgumentOutOfRangeException(nameof(band), "A band is 0 (above the square) or 1 (below it).");
        return PetalLayout.KanaDataTileTable.Slice(band * KanaDataTiles, KanaDataTiles);
    }

    /// <summary>Whether <paramref name="data"/> is kana lane data whose check matches <paramref name="header"/> and <paramref name="lane"/>.</summary>
    private static bool Checks(PetalLaneHeader header, PetalKanaLane lane, ReadOnlySpan<byte> data)
    {
        if (data.Length != DataLength(lane))
            return false;
        var atoms = Atoms(lane) * PetalLanes.AtomLength;
        return BinaryPrimitives.ReadUInt32BigEndian(data[atoms..]) == KanaCheck(header, lane, data[..atoms]);
    }

    private static PetalKanaLane CheckLane(PetalKanaLane lane)
    {
        if ((uint)lane > (uint)PetalKanaLane.K1)
            throw new ArgumentOutOfRangeException(nameof(lane));
        return lane;
    }

    private static void CheckSymbols(int length, string paramName)
    {
        if (length != PetalLayout.KanaTileCount)
            throw new ArgumentException("Kana symbols name every one of the 612 tiles.", paramName);
    }

    private static void CheckProbe(int glyph, int i, int j, int side)
    {
        if ((uint)glyph >= PetalGlyphs.GlyphCount)
            throw new ArgumentOutOfRangeException(nameof(glyph), "A glyph is 0 to 15.");
        if ((uint)i >= (uint)side)
            throw new ArgumentOutOfRangeException(nameof(i));
        if ((uint)j >= (uint)side)
            throw new ArgumentOutOfRangeException(nameof(j));
    }

    private static PetalReedSolomon CodeOf(PetalKanaLane lane) => IsGlyph(lane) ? GlyphCode : PolarityCode;

    private static byte[][] BuildWhitenings()
    {
        var whitenings = new byte[LaneValues.Length][];
        foreach (var lane in LaneValues)
        {
            var band = (uint)Band(lane);
            var seed = IsGlyph(lane) ? 0x4B41_4E30u + band : 0x504F_4C30u + band; // "KAN0", "KAN1" / "POL0", "POL1"
            var rng = new PetalXorshift32(seed);
            var sequence = new byte[WordLength(lane)];
            for (var i = 0; i < sequence.Length; i++)
                sequence[i] = rng.NextByte();
            whitenings[(int)lane] = sequence;
        }

        return whitenings;
    }

    /// <summary>
    /// <c>a.midpoint(b)</c> of Rust's <c>f64</c>: <c>(a + b) / 2</c> unless that could overflow or a
    /// half could underflow.
    /// </summary>
    private static double Midpoint(double a, double b)
    {
        // twice the smallest positive normal value (Rust `f64::MIN_POSITIVE * 2.`)
        const double lo = 2.2250738585072014E-308 * 2.0;
        const double hi = double.MaxValue / 2.0;
        var (absA, absB) = (Math.Abs(a), Math.Abs(b));
        if (absA <= hi && absB <= hi)
            return (a + b) / 2.0;
        if (absA < lo)
            return a + b / 2.0;
        if (absB < lo)
            return a / 2.0 + b;
        return a / 2.0 + b / 2.0;
    }

    /// <summary>
    /// The band reader's templates, built once on first use (the type initializer runs exactly once,
    /// thread-safely): for every blur of <see cref="KanaBlurs"/>, the coverage rasters of the tile's
    /// inside and of every glyph's ink, blurred and sampled at the fine grid points and
    /// mean-centred; a light tile's template is the inside less the ink, a dark tile's the ink, each
    /// also scaled to unit length. The coarse templates are the same under blur 3.4 at the coarse grid
    /// points. About 0.2 MB.
    /// </summary>
    internal static class ReaderTables
    {
        /// <summary>Samples of a tile on the fine grid.</summary>
        internal const int Points = KanaGrid * KanaGrid;

        /// <summary>Samples of a tile on the coarse grid.</summary>
        internal const int CoarsePoints = KanaCoarseGrid * KanaCoarseGrid;

        /// <summary>The blur of the coarse templates (3.4 units), coarse enough to find a pilot on a 2-unit grid under any blur.</summary>
        internal const int CoarseBlur = 3;

        /// <summary>Template raster step in canvas units.</summary>
        private const double RasterStep = 0.5;

        /// <summary>The raster starts this far outside the tile square, so blur reaches in from the gutter.</summary>
        private const double RasterMargin = 4.0;

        /// <summary>Raster side in steps.</summary>
        private const int RasterSide = 67;

        private const int Glyphs = PetalGlyphs.GlyphCount;

        /// <summary>Tile-local coordinate of every fine grid index on either axis: <c>(i + ½) · 25 / 12</c>.</summary>
        internal static readonly double[] Grid = new double[KanaGrid];

        /// <summary>Tile-local coordinate of every coarse grid index: <c>(i + ½) · 25 / 6</c>.</summary>
        internal static readonly double[] CoarseGrid = new double[KanaCoarseGrid];

        /// <summary>Per blur (from <c>blur · 144</c>): the tile's inside at the grid points, mean-centred.</summary>
        internal static readonly double[] Inside = new double[KanaBlurValues.Length * Points];

        /// <summary>Per blur and glyph (at <see cref="Offset"/>): the ink at the grid points, mean-centred.</summary>
        internal static readonly double[] Ink = new double[KanaBlurValues.Length * Glyphs * Points];

        /// <summary>Per blur and glyph (<c>blur · 16 + glyph</c>): the length of the light template (inside less ink).</summary>
        internal static readonly double[] LightNorm = new double[KanaBlurValues.Length * Glyphs];

        /// <summary>Per blur and glyph (<c>blur · 16 + glyph</c>): the length of the dark template (ink).</summary>
        internal static readonly double[] DarkNorm = new double[KanaBlurValues.Length * Glyphs];

        /// <summary>Per blur and glyph (at <see cref="Offset"/>): the light template scaled to unit length.</summary>
        internal static readonly double[] Light = new double[KanaBlurValues.Length * Glyphs * Points];

        /// <summary>Per blur and glyph (at <see cref="Offset"/>): the dark template scaled to unit length.</summary>
        internal static readonly double[] Dark = new double[KanaBlurValues.Length * Glyphs * Points];

        /// <summary>Per glyph (from <c>glyph · 36</c>): the coarse light template, unit length.</summary>
        internal static readonly double[] CoarseLight = new double[Glyphs * CoarsePoints];

        /// <summary>Per glyph (from <c>glyph · 36</c>): the coarse dark template, unit length.</summary>
        internal static readonly double[] CoarseDark = new double[Glyphs * CoarsePoints];

        static ReaderTables()
        {
            for (var i = 0; i < KanaGrid; i++)
                Grid[i] = GridPoint(i, KanaGrid);
            for (var i = 0; i < KanaCoarseGrid; i++)
                CoarseGrid[i] = GridPoint(i, KanaCoarseGrid);
            var inside = Raster(-1);
            var inks = new double[Glyphs][];
            for (var glyph = 0; glyph < Glyphs; glyph++)
                inks[glyph] = Raster(glyph);
            var pass = new double[RasterSide * RasterSide];
            var insideBlurred = new double[RasterSide * RasterSide];
            var inkBlurred = new double[RasterSide * RasterSide];
            var light = new double[Points];
            var coarseInside = new double[CoarsePoints];
            var coarseInk = new double[CoarsePoints];
            var coarseLight = new double[CoarsePoints];
            for (var blur = 0; blur < KanaBlurValues.Length; blur++)
            {
                var sigma = KanaBlurValues[blur];
                BlurRaster(inside, sigma, pass, insideBlurred);
                SampleRaster(insideBlurred, KanaGrid, Inside.AsSpan(blur * Points, Points));
                for (var glyph = 0; glyph < Glyphs; glyph++)
                {
                    BlurRaster(inks[glyph], sigma, pass, inkBlurred);
                    var at = Offset(blur, glyph);
                    var ink = Ink.AsSpan(at, Points);
                    SampleRaster(inkBlurred, KanaGrid, ink);
                    for (var p = 0; p < Points; p++)
                        light[p] = Inside[blur * Points + p] - ink[p];
                    var lightNorm = Norm(light);
                    var darkNorm = Norm(ink);
                    LightNorm[blur * Glyphs + glyph] = lightNorm;
                    DarkNorm[blur * Glyphs + glyph] = darkNorm;
                    for (var p = 0; p < Points; p++)
                    {
                        Light[at + p] = light[p] / lightNorm;
                        Dark[at + p] = ink[p] / darkNorm;
                    }

                    if (blur != CoarseBlur)
                        continue;
                    SampleRaster(insideBlurred, KanaCoarseGrid, coarseInside);
                    SampleRaster(inkBlurred, KanaCoarseGrid, coarseInk);
                    for (var p = 0; p < CoarsePoints; p++)
                        coarseLight[p] = coarseInside[p] - coarseInk[p];
                    var coarseLightNorm = Norm(coarseLight);
                    var coarseDarkNorm = Norm(coarseInk);
                    for (var p = 0; p < CoarsePoints; p++)
                    {
                        CoarseLight[glyph * CoarsePoints + p] = coarseLight[p] / coarseLightNorm;
                        CoarseDark[glyph * CoarsePoints + p] = coarseInk[p] / coarseDarkNorm;
                    }
                }
            }
        }

        /// <summary>Offset of the templates of <paramref name="glyph"/> under blur <paramref name="blur"/> in <see cref="Ink"/>, <see cref="Light"/> and <see cref="Dark"/>.</summary>
        internal static int Offset(int blur, int glyph) => (blur * Glyphs + glyph) * Points;

        /// <summary>Tile-local coordinate of grid index <paramref name="i"/> on either axis of a grid of <paramref name="side"/> points.</summary>
        private static double GridPoint(int i, int side) => (i + 0.5) * PetalLayout.KanaTile / side;

        /// <summary>
        /// Coverage on the template raster of the tile's inside (<paramref name="glyph"/> −1) or of a
        /// glyph's ink: raster point <c>a</c> is at <c>−4 + a / 2</c> units; each value is the share of
        /// its 4 × 4 sub-points <c>(m − 1.5) / 8</c> units around it that are covered.
        /// </summary>
        private static double[] Raster(int glyph)
        {
            var table = new double[RasterSide * RasterSide];
            for (var b = 0; b < RasterSide; b++)
            {
                for (var a = 0; a < RasterSide; a++)
                {
                    var u0 = -RasterMargin + a * RasterStep;
                    var v0 = -RasterMargin + b * RasterStep;
                    var hits = 0;
                    for (var n = 0; n < 4; n++)
                    {
                        for (var m = 0; m < 4; m++)
                        {
                            var u = u0 + (m - 1.5) / 8.0;
                            var v = v0 + (n - 1.5) / 8.0;
                            var covered = glyph < 0 ? PetalLayout.KanaInside(u, v) : PetalLayout.KanaInkedUnchecked(glyph, u, v);
                            if (covered)
                                hits++;
                        }
                    }

                    table[b * RasterSide + a] = hits / 16.0;
                }
            }

            return table;
        }

        /// <summary>
        /// Separable Gaussian blur of a raster into <paramref name="output"/> (<paramref name="pass"/>
        /// is scratch): taps every half unit out to <c>⌈3σ / ½⌉</c>, weights <c>exp(−d² / (2σ²))</c>
        /// normalised by their sum, points outside counting as zero; rows first.
        /// </summary>
        private static void BlurRaster(double[] table, double sigma, double[] pass, double[] output)
        {
            var reach = (int)Math.Ceiling(3.0 * sigma / RasterStep);
            var weights = new double[2 * reach + 1];
            for (var k = -reach; k <= reach; k++)
            {
                var d = k * RasterStep;
                weights[k + reach] = Math.Exp(-(d * d) / (2.0 * sigma * sigma));
            }

            var sum = PetalMath.SumIdentity;
            foreach (var weight in weights)
                sum += weight;
            for (var i = 0; i < weights.Length; i++)
                weights[i] /= sum;
            Pass(table, pass, weights, reach, alongRows: true);
            Pass(pass, output, weights, reach, alongRows: false);
        }

        private static void Pass(double[] source, double[] output, double[] weights, int reach, bool alongRows)
        {
            for (var b = 0; b < RasterSide; b++)
            {
                for (var a = 0; a < RasterSide; a++)
                {
                    var acc = 0.0;
                    for (var k = -reach; k <= reach; k++)
                    {
                        var (aa, bb) = alongRows ? (a + k, b) : (a, b + k);
                        if (aa < 0 || bb < 0 || aa >= RasterSide || bb >= RasterSide)
                            continue;
                        acc += weights[k + reach] * source[bb * RasterSide + aa];
                    }

                    output[b * RasterSide + a] = acc;
                }
            }
        }

        /// <summary>A blurred raster at the <paramref name="side"/> × <paramref name="side"/> grid points (bilinear) into <paramref name="output"/>, mean-centred.</summary>
        private static void SampleRaster(double[] table, int side, Span<double> output)
        {
            for (var j = 0; j < side; j++)
            {
                for (var i = 0; i < side; i++)
                {
                    var fa = (GridPoint(i, side) + RasterMargin) / RasterStep;
                    var fb = (GridPoint(j, side) + RasterMargin) / RasterStep;
                    var a0 = (int)Math.Floor(fa);
                    var b0 = (int)Math.Floor(fb);
                    var ta = fa - a0;
                    var tb = fb - b0;
                    var top = table[b0 * RasterSide + a0] * (1.0 - ta) + table[b0 * RasterSide + a0 + 1] * ta;
                    var bottom = table[(b0 + 1) * RasterSide + a0] * (1.0 - ta) + table[(b0 + 1) * RasterSide + a0 + 1] * ta;
                    output[j * side + i] = top * (1.0 - tb) + bottom * tb;
                }
            }

            var n = side * side;
            var sum = PetalMath.SumIdentity;
            for (var p = 0; p < n; p++)
                sum += output[p];
            var mean = sum / n;
            for (var p = 0; p < n; p++)
                output[p] -= mean;
        }

        /// <summary>The length of a vector, at least <c>1e-12</c>.</summary>
        private static double Norm(ReadOnlySpan<double> values)
        {
            var sum = PetalMath.SumIdentity;
            foreach (var value in values)
                sum += value * value;
            return PetalMath.Max(Math.Sqrt(sum), 1e-12);
        }
    }

    /// <summary>
    /// The working memory of the band reader (<c>observe_band</c> and <c>read_kana</c> of the
    /// reference), reused across captures: a few kilobytes. One idle reader is kept; a reader is used
    /// by one thread at a time.
    /// </summary>
    private sealed class Reader
    {
        private static Reader? idle;

        private readonly double[] samples = new double[ReaderTables.Points];

        // the pilots kept after the coarse search, then the pilots found
        private readonly int[] coarseTile = new int[Pilots];
        private readonly double[] coarseX = new double[Pilots];
        private readonly double[] coarseY = new double[Pilots];
        private readonly double[] foundCentreX = new double[Pilots];
        private readonly double[] foundCentreY = new double[Pilots];
        private readonly double[] foundShiftX = new double[Pilots];
        private readonly double[] foundShiftY = new double[Pilots];
        private readonly double[] foundLevel = new double[Pilots];
        private readonly bool[] foundLight = new bool[Pilots];
        private readonly double[] lineX = new double[Pilots];
        private readonly double[] lineY = new double[Pilots];

        // the shift field: per axis a₀ + a₁·x + a₂·y
        private readonly double[] fieldX = new double[3];
        private readonly double[] fieldY = new double[3];
        private readonly double[] scores = new double[PetalGlyphs.GlyphCount];

        // codewords and their byte confidences
        private readonly byte[] polarity = new byte[KanaPolarityWordLength];
        private readonly double[] polarityConfidence = new double[KanaPolarityWordLength];
        private readonly byte[] glyphs = new byte[KanaGlyphWordLength];
        private readonly double[] glyphConfidence = new double[KanaGlyphWordLength];

        private PetalLuma? image;
        private PetalPose pose;
        private int count;
        private int coarseCount;
        private double fieldCentre;

        /// <summary>The index into <see cref="KanaBlurs"/> the pilots chose.</summary>
        public int Blur { get; private set; }

        /// <summary>Pilots found.</summary>
        public int FoundCount { get; private set; }

        /// <summary>Per data tile of the observed band: whether it was read, and what was decided.</summary>
        public bool[] TileRead { get; } = new bool[KanaDataTiles];

        public bool[] TileLight { get; } = new bool[KanaDataTiles];

        public double[] TileMargin { get; } = new double[KanaDataTiles];

        public byte[] TileGlyph { get; } = new byte[KanaDataTiles];

        public double[] TileLead { get; } = new double[KanaDataTiles];

        public static Reader Lease() => Interlocked.Exchange(ref idle, null) ?? new Reader();

        public static void Return(Reader reader)
        {
            // an idle reader keeps no image alive
            reader.image = null;
            Volatile.Write(ref idle, reader);
        }

        /// <summary>Reads band <paramref name="band"/>'s two lanes into <paramref name="results"/> (at <c>2 · band</c> and <c>2 · band + 1</c>).</summary>
        public void ReadBand(PetalLuma luma, PetalDecodedFrame frame, PetalLaneHeader header, int band, PetalKanaResult?[] results)
        {
            if (!Observe(luma, frame, band))
                return;
            Array.Clear(polarity);
            Array.Fill(polarityConfidence, double.MaxValue);
            Array.Clear(glyphs);
            Array.Fill(glyphConfidence, double.MaxValue);
            for (var i = 0; i < KanaDataTiles; i++)
            {
                var read = TileRead[i];
                var light = read && TileLight[i];
                var margin = read ? TileMargin[i] : 0.0;
                var glyph = read ? TileGlyph[i] : 0;
                var lead = read ? TileLead[i] : 0.0;
                if (i < KanaPolarityWordLength * 8)
                {
                    if (light)
                        polarity[i / 8] |= (byte)(1 << (7 - i % 8));
                    polarityConfidence[i / 8] = PetalMath.Min(polarityConfidence[i / 8], margin);
                }

                glyphs[i / 2] |= (byte)(i % 2 == 0 ? glyph << 4 : glyph);
                glyphConfidence[i / 2] = PetalMath.Min(glyphConfidence[i / 2], lead);
            }

            results[2 * band] = DecodeKanaWord(Lane(band, glyph: false), header, polarity, polarityConfidence);
            results[2 * band + 1] = DecodeKanaWord(Lane(band, glyph: true), header, glyphs, glyphConfidence);
        }

        /// <summary>
        /// Reads band <paramref name="band"/>'s data tiles into the tile arrays; <see langword="false"/>
        /// when fewer than two pilots are found or the pilots show no contrast between light and dark
        /// tiles.
        /// </summary>
        public bool Observe(PetalLuma luma, PetalDecodedFrame frame, int band)
        {
            image = luma;
            pose = frame.Pose;
            var tileX = PetalLayout.TileXTable;
            var tileY = PetalLayout.TileYTable;
            var pilotTiles = PetalLayout.KanaPilotTileTable.Slice(band * Pilots, Pilots);
            // coarse search of every pilot in view
            coarseCount = 0;
            foreach (var tile in pilotTiles)
            {
                if (!TileInImage(tile))
                    continue;
                CoarsePilot(tile, out var shiftX, out var shiftY, out var score);
                if (score < PilotCoarseMin)
                    continue;
                coarseTile[coarseCount] = tile;
                coarseX[coarseCount] = shiftX;
                coarseY[coarseCount] = shiftY;
                coarseCount++;
            }

            if (coarseCount < 2)
                return false;
            // the blur under which the pilots, each at its best shift within a unit, correlate best
            Blur = 0;
            var best = double.MinValue;
            for (var index = 0; index < KanaBlurValues.Length; index++)
            {
                var sum = PetalMath.SumIdentity;
                for (var c = 0; c < coarseCount; c++)
                {
                    RefinePilot(coarseTile[c], index, coarseX[c], coarseY[c], RefineCoarse, out _, out _, out var score);
                    sum += score;
                }

                if (sum > best)
                {
                    best = sum;
                    Blur = index;
                }
            }

            const double half = PetalLayout.KanaTile / 2.0;
            var found = 0;
            for (var c = 0; c < coarseCount; c++)
            {
                var tile = coarseTile[c];
                RefinePilot(tile, Blur, coarseX[c], coarseY[c], RefineFine, out var shiftX, out var shiftY, out var score);
                if (score < PilotMin)
                    continue;
                var level = SampleTile(tileX[tile], tileY[tile], shiftX, shiftY, ReaderTables.Grid, out _);
                foundCentreX[found] = tileX[tile] + half;
                foundCentreY[found] = tileY[tile] + half;
                foundShiftX[found] = shiftX;
                foundShiftY[found] = shiftY;
                foundLevel[found] = level;
                foundLight[found] = PetalLayout.KanaPilotSymbolTable[PetalLayout.TilePilotTable[tile]].Light;
                found++;
            }

            FoundCount = found;
            if (found < 2)
                return false;
            FitShiftField();
            if (!LevelLine(light: true, out var lightIntercept, out var lightSlope)
                || !LevelLine(light: false, out var darkIntercept, out var darkSlope))
            {
                return false;
            }

            var inside = ReaderTables.Inside.AsSpan(Blur * ReaderTables.Points, ReaderTables.Points);
            var dataTiles = PetalLayout.KanaDataTileTable.Slice(band * KanaDataTiles, KanaDataTiles);
            for (var i = 0; i < KanaDataTiles; i++)
            {
                var tile = dataTiles[i];
                TileRead[i] = false;
                if (!TileInImage(tile))
                    continue;
                var (x0, y0) = (tileX[tile], tileY[tile]);
                var (centreX, centreY) = (x0 + half, y0 + half);
                ShiftAt(centreX, centreY, out var shiftX, out var shiftY);
                var mean = SampleTile(x0, y0, shiftX, shiftY, ReaderTables.Grid, out var energy);
                var x = (centreX - PetalLayout.Canvas / 2.0) / 512.0;
                var high = lightIntercept + lightSlope * x;
                var low = darkIntercept + darkSlope * x;
                var halfContrast = (high - low) / 2.0;
                if (halfContrast <= 1.0)
                    continue;
                var threshold = Midpoint(high, low);
                // a light template is the inside less the ink, so one dot product with the inside and
                // one with each glyph's ink give both polarities' correlations
                Array.Clear(scores);
                if (energy > 1e-9)
                {
                    var root = Math.Sqrt(energy);
                    var insideDot = Dot(inside);
                    for (var g = 0; g < PetalGlyphs.GlyphCount; g++)
                    {
                        var inkDot = Dot(ReaderTables.Ink.AsSpan(ReaderTables.Offset(Blur, g), ReaderTables.Points));
                        var norms = Blur * PetalGlyphs.GlyphCount + g;
                        var light = (insideDot - inkDot) / ReaderTables.LightNorm[norms] / root;
                        var dark = inkDot / ReaderTables.DarkNorm[norms] / root;
                        scores[g] = PetalMath.Max(light, dark);
                    }
                }

                var first = 0;
                for (var g = 1; g < PetalGlyphs.GlyphCount; g++)
                {
                    if (scores[g] > scores[first])
                        first = g;
                }

                // the runner-up: Rust `fold(f64::MIN, f64::max)` over the other glyphs in order
                var second = double.MinValue;
                for (var g = 0; g < PetalGlyphs.GlyphCount; g++)
                {
                    if (g != first)
                        second = PetalMath.Max(second, scores[g]);
                }

                TileRead[i] = true;
                TileLight[i] = mean > threshold;
                TileMargin[i] = Math.Abs(mean - threshold) / halfContrast;
                TileGlyph[i] = (byte)first;
                TileLead[i] = scores[first] - second;
            }

            return true;
        }

        /// <summary>
        /// Samples the tile whose top-left corner is <c>(x, y)</c> (square coordinates), moved by the
        /// shift, at the grid points into <see cref="samples"/>: the pose maps the tile's centre
        /// <c>o</c> and the points one unit to its right and below it (<c>o + u</c>, <c>o + v</c>), and
        /// grid point <c>(a, b)</c> from the centre is sampled at <c>(o + b·v) + a·u</c>. The samples
        /// are left minus their mean; returns that mean, with their energy (the sum of their squares).
        /// </summary>
        private double SampleTile(double x, double y, double shiftX, double shiftY, ReadOnlySpan<double> grid, out double energy)
        {
            var luma = image!;
            const double half = PetalLayout.KanaTile / 2.0;
            var cx = x + half + shiftX;
            var cy = y + half + shiftY;
            pose.Apply(cx, cy, out var ox, out var oy);
            pose.Apply(cx + 1.0, cy, out var rightX, out var rightY);
            pose.Apply(cx, cy + 1.0, out var downX, out var downY);
            var (ux, uy) = (rightX - ox, rightY - oy);
            var (vx, vy) = (downX - ox, downY - oy);
            var side = grid.Length;
            var sum = 0.0;
            for (var j = 0; j < side; j++)
            {
                var b = grid[j] - half;
                var (rowX, rowY) = (ox + b * vx, oy + b * vy);
                for (var i = 0; i < side; i++)
                {
                    var a = grid[i] - half;
                    var value = luma.Sample(rowX + a * ux, rowY + a * uy);
                    samples[j * side + i] = value;
                    sum += value;
                }
            }

            var n = side * side;
            var mean = sum / n;
            energy = 0.0;
            for (var k = 0; k < n; k++)
            {
                var centred = samples[k] - mean;
                samples[k] = centred;
                energy += centred * centred;
            }

            count = n;
            return mean;
        }

        /// <summary>The dot product of the last samples with a template (summed from −0 like Rust's <c>Iterator::sum</c>).</summary>
        private double Dot(ReadOnlySpan<double> template)
        {
            var sum = PetalMath.SumIdentity;
            for (var k = 0; k < count; k++)
                sum += samples[k] * template[k];
            return sum;
        }

        /// <summary>Correlation of the last samples (mean-centred, energy <paramref name="energy"/>) with a unit, mean-centred template.</summary>
        private double Correlate(ReadOnlySpan<double> template, double energy) =>
            energy <= 1e-9 ? 0.0 : Dot(template) / Math.Sqrt(energy);

        /// <summary>Whether every corner of <paramref name="tile"/> maps inside the image.</summary>
        private bool TileInImage(int tile)
        {
            var luma = image!;
            const double side = PetalLayout.KanaTile;
            var (x, y) = (PetalLayout.TileXTable[tile], PetalLayout.TileYTable[tile]);
            for (var corner = 0; corner < 4; corner++)
            {
                var u = corner % 2 == 1 ? side : 0.0;
                var v = corner >= 2 ? side : 0.0;
                pose.Apply(x + u, y + v, out var px, out var py);
                if (!(px >= 0.0 && py >= 0.0 && px < luma.Width && py < luma.Height))
                    return false;
            }

            return true;
        }

        /// <summary>
        /// How far (canvas units) the pose can miss a pilot centred <paramref name="y"/> units down the
        /// square (negative above it): 4 units at the square's edge, growing by 10 over a band's
        /// 290-unit height, because a lens's distortion grows away from the square the pose was fitted to.
        /// </summary>
        private static double PilotReach(double y)
        {
            var beyond = y < 0.0 ? -y : y - PetalLayout.Canvas;
            return 4.0 + 10.0 * PetalMath.Min(PetalMath.Max(beyond, 0.0) / 290.0, 1.2);
        }

        /// <summary>The template a pilot tile is matched with under blur <paramref name="blur"/> (unit, fine grid).</summary>
        private static ReadOnlySpan<double> PilotTemplate(int tile, int blur)
        {
            var symbol = PetalLayout.KanaPilotSymbolTable[PetalLayout.TilePilotTable[tile]];
            var table = symbol.Light ? ReaderTables.Light : ReaderTables.Dark;
            return table.AsSpan(ReaderTables.Offset(blur, symbol.Glyph), ReaderTables.Points);
        }

        /// <summary>The correlation of a pilot tile's samples, moved by the shift, with its template under <paramref name="blur"/>.</summary>
        private double PilotScore(int tile, int blur, double shiftX, double shiftY)
        {
            SampleTile(PetalLayout.TileXTable[tile], PetalLayout.TileYTable[tile], shiftX, shiftY, ReaderTables.Grid, out var energy);
            return Correlate(PilotTemplate(tile, blur), energy);
        }

        /// <summary>
        /// Coarse search of a pilot: shifts on a <see cref="CoarseStep"/> grid within its
        /// <see cref="PilotReach"/>, <c>dy</c> outer and <c>dx</c> inner, sampled on the coarse grid
        /// under the coarse template; a later shift wins only with a larger correlation.
        /// </summary>
        private void CoarsePilot(int tile, out double bestX, out double bestY, out double bestScore)
        {
            var symbol = PetalLayout.KanaPilotSymbolTable[PetalLayout.TilePilotTable[tile]];
            var template = (symbol.Light ? ReaderTables.CoarseLight : ReaderTables.CoarseDark)
                .AsSpan(symbol.Glyph * ReaderTables.CoarsePoints, ReaderTables.CoarsePoints);
            var (x, y) = (PetalLayout.TileXTable[tile], PetalLayout.TileYTable[tile]);
            var reach = PilotReach(y + PetalLayout.KanaTile / 2.0);
            var steps = (int)Math.Floor(reach / CoarseStep);
            (bestX, bestY, bestScore) = (0.0, 0.0, double.MinValue);
            for (var dy = -steps; dy <= steps; dy++)
            {
                for (var dx = -steps; dx <= steps; dx++)
                {
                    var (shiftX, shiftY) = (CoarseStep * dx, CoarseStep * dy);
                    SampleTile(x, y, shiftX, shiftY, ReaderTables.CoarseGrid, out var energy);
                    var score = Correlate(template, energy);
                    if (score > bestScore)
                        (bestX, bestY, bestScore) = (shiftX, shiftY, score);
                }
            }
        }

        /// <summary>
        /// Refines a pilot's shift under <paramref name="blur"/>: the best of the shifts one step around
        /// it, for each of <paramref name="steps"/> in turn (ties keep the earlier).
        /// </summary>
        private void RefinePilot(int tile, int blur, double startX, double startY, double[] steps, out double bestX, out double bestY, out double bestScore)
        {
            (bestX, bestY) = (startX, startY);
            bestScore = PilotScore(tile, blur, startX, startY);
            foreach (var step in steps)
            {
                var (centreX, centreY) = (bestX, bestY);
                for (var dy = -1; dy <= 1; dy++)
                {
                    for (var dx = -1; dx <= 1; dx++)
                    {
                        if (dx == 0 && dy == 0)
                            continue;
                        var (shiftX, shiftY) = (centreX + step * dx, centreY + step * dy);
                        var score = PilotScore(tile, blur, shiftX, shiftY);
                        if (score > bestScore)
                            (bestX, bestY, bestScore) = (shiftX, shiftY, score);
                    }
                }
            }
        }

        /// <summary>
        /// Fits the band's shift field to the found pilots: per axis an affine function of the tile
        /// centre (<c>x</c> scaled by 512 around the square's middle, <c>y</c> by 145 around the
        /// pilots' mean) by least squares, or their mean shift when fewer than three pilots (or a
        /// singular system) fit one.
        /// </summary>
        private void FitShiftField()
        {
            var n = (double)FoundCount;
            var sumY = PetalMath.SumIdentity;
            for (var p = 0; p < FoundCount; p++)
                sumY += foundCentreY[p];
            fieldCentre = sumY / n;
            var meanX = PetalMath.SumIdentity;
            for (var p = 0; p < FoundCount; p++)
                meanX += foundShiftX[p];
            var meanY = PetalMath.SumIdentity;
            for (var p = 0; p < FoundCount; p++)
                meanY += foundShiftY[p];
            (fieldX[0], fieldX[1], fieldX[2]) = (meanX / n, 0.0, 0.0);
            (fieldY[0], fieldY[1], fieldY[2]) = (meanY / n, 0.0, 0.0);
            if (FoundCount < 3)
                return;
            Span<double> normal = stackalloc double[9];
            Span<double> rightX = stackalloc double[3];
            Span<double> rightY = stackalloc double[3];
            Span<double> basis = stackalloc double[3];
            normal.Clear();
            rightX.Clear();
            rightY.Clear();
            for (var p = 0; p < FoundCount; p++)
            {
                basis[0] = 1.0;
                basis[1] = (foundCentreX[p] - PetalLayout.Canvas / 2.0) / 512.0;
                basis[2] = (foundCentreY[p] - fieldCentre) / 145.0;
                for (var i = 0; i < 3; i++)
                {
                    for (var j = 0; j < 3; j++)
                        normal[i * 3 + j] += basis[i] * basis[j];
                    rightX[i] += basis[i] * foundShiftX[p];
                    rightY[i] += basis[i] * foundShiftY[p];
                }
            }

            Span<double> solvedX = stackalloc double[3];
            Span<double> solvedY = stackalloc double[3];
            if (!Solve3(normal, rightX, solvedX) || !Solve3(normal, rightY, solvedY))
                return;
            solvedX.CopyTo(fieldX);
            solvedY.CopyTo(fieldY);
        }

        /// <summary>The shift of the field at the tile centre <c>(x, y)</c>.</summary>
        private void ShiftAt(double x, double y, out double shiftX, out double shiftY)
        {
            var b1 = (x - PetalLayout.Canvas / 2.0) / 512.0;
            var b2 = (y - fieldCentre) / 145.0;
            shiftX = fieldX[0] * 1.0 + fieldX[1] * b1 + fieldX[2] * b2;
            shiftY = fieldY[0] * 1.0 + fieldY[1] * b1 + fieldY[2] * b2;
        }

        /// <summary>
        /// Solves the 3 × 3 system <c>normal · s = right</c> by Gauss–Jordan elimination with partial
        /// pivoting into <paramref name="solution"/>; <see langword="false"/> when a pivot is below
        /// <c>1e-9</c>. Of equal pivots the last wins, as Rust's <c>max_by</c> picks.
        /// </summary>
        private static bool Solve3(ReadOnlySpan<double> normal, ReadOnlySpan<double> right, Span<double> solution)
        {
            Span<double> m = stackalloc double[9];
            Span<double> r = stackalloc double[3];
            normal.CopyTo(m);
            right.CopyTo(r);
            for (var col = 0; col < 3; col++)
            {
                var pivot = col;
                for (var row = col + 1; row < 3; row++)
                {
                    if (PetalMath.TotalCompare(Math.Abs(m[row * 3 + col]), Math.Abs(m[pivot * 3 + col])) >= 0)
                        pivot = row;
                }

                if (Math.Abs(m[pivot * 3 + col]) < 1e-9)
                    return false;
                for (var k = 0; k < 3; k++)
                    (m[col * 3 + k], m[pivot * 3 + k]) = (m[pivot * 3 + k], m[col * 3 + k]);
                (r[col], r[pivot]) = (r[pivot], r[col]);
                for (var row = 0; row < 3; row++)
                {
                    if (row == col)
                        continue;
                    var factor = m[row * 3 + col] / m[col * 3 + col];
                    for (var k = col; k < 3; k++)
                        m[row * 3 + k] -= factor * m[col * 3 + k];
                    r[row] -= factor * r[col];
                }
            }

            solution[0] = r[0] / m[0];
            solution[1] = r[1] / m[4];
            solution[2] = r[2] / m[8];
            return true;
        }

        /// <summary>
        /// A level that varies linearly across the band, fitted by least squares to the levels of the
        /// light (<paramref name="light"/>) or dark found pilots against <c>(x − 512) / 512</c>; a
        /// constant when the pilots do not spread. <see langword="false"/> without such pilots.
        /// </summary>
        private bool LevelLine(bool light, out double intercept, out double slope)
        {
            (intercept, slope) = (0.0, 0.0);
            var points = 0;
            for (var p = 0; p < FoundCount; p++)
            {
                if (foundLight[p] != light)
                    continue;
                lineX[points] = (foundCentreX[p] - PetalLayout.Canvas / 2.0) / 512.0;
                lineY[points] = foundLevel[p];
                points++;
            }

            if (points == 0)
                return false;
            var n = (double)points;
            var sumX = PetalMath.SumIdentity;
            for (var p = 0; p < points; p++)
                sumX += lineX[p];
            var mx = sumX / n;
            var sumY = PetalMath.SumIdentity;
            for (var p = 0; p < points; p++)
                sumY += lineY[p];
            var my = sumY / n;
            var sxx = PetalMath.SumIdentity;
            for (var p = 0; p < points; p++)
                sxx += (lineX[p] - mx) * (lineX[p] - mx);
            if (sxx < 1e-6)
            {
                intercept = my;
                return true;
            }

            var sxy = PetalMath.SumIdentity;
            for (var p = 0; p < points; p++)
                sxy += (lineX[p] - mx) * (lineY[p] - my);
            slope = sxy / sxx;
            intercept = my - slope * mx;
            return true;
        }
    }
}
