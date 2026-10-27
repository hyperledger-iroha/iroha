using System.Buffers.Binary;

namespace Hyperledger.Iroha.Petal;

/// <summary>Why a Petal stream could not be prepared.</summary>
public enum PetalStreamErrorCode
{
    /// <summary>The payload is empty.</summary>
    EmptyPayload,

    /// <summary>The payload exceeds <see cref="PetalStream.MaxPayloadLength"/> (the 24-bit length field).</summary>
    PayloadTooLarge,
}

/// <summary>Raised by <see cref="PetalStreamEncoder"/> for payloads it cannot stream.</summary>
public sealed class PetalStreamException : ArgumentException
{
    /// <summary>Creates the exception.</summary>
    /// <param name="code">Failure reason.</param>
    /// <param name="message">Human-readable message.</param>
    public PetalStreamException(PetalStreamErrorCode code, string message)
        : base(message)
    {
        Code = code;
    }

    /// <summary>Failure reason.</summary>
    public PetalStreamErrorCode Code { get; }
}

/// <summary>Identity of a stream, as carried by every beacon.</summary>
/// <param name="Kind">Application payload kind.</param>
/// <param name="Length">Payload length in bytes.</param>
/// <param name="Crc">CRC-32C of the payload.</param>
public readonly record struct PetalStreamMeta(byte Kind, uint Length, uint Crc)
{
    /// <summary>The one-byte stream tag repeated in every lane header (low byte of the CRC).</summary>
    public byte Tag => (byte)Crc;

    /// <summary>Number of fountain source atoms.</summary>
    public int SourceAtoms => (int)((Length + (ulong)PetalLanes.AtomLength - 1) / PetalLanes.AtomLength);
}

/// <summary>The common three-byte header of every lane.</summary>
/// <param name="Tag">Stream tag.</param>
/// <param name="Frame">Frame counter (wraps at 65536).</param>
public readonly record struct PetalLaneHeader(byte Tag, ushort Frame);

/// <summary>A decoded stream beacon.</summary>
/// <param name="Header">Lane header.</param>
/// <param name="Meta">Stream identity.</param>
public readonly record struct PetalBeacon(PetalLaneHeader Header, PetalStreamMeta Meta);

/// <summary>Atoms read from one lane.</summary>
public sealed class PetalAtomPacket : IEquatable<PetalAtomPacket>
{
    private readonly byte[][] atoms;

    /// <summary>Creates a packet.</summary>
    /// <param name="header">Lane header.</param>
    /// <param name="firstId">Fountain id of the first atom; the rest follow consecutively.</param>
    /// <param name="atoms">The 16-byte atoms (copied).</param>
    /// <exception cref="ArgumentException">An atom is not 16 bytes.</exception>
    public PetalAtomPacket(PetalLaneHeader header, uint firstId, IEnumerable<byte[]> atoms)
    {
        ArgumentNullException.ThrowIfNull(atoms);
        var copies = new List<byte[]>();
        foreach (var atom in atoms)
        {
            ArgumentNullException.ThrowIfNull(atom, nameof(atoms));
            if (atom.Length != PetalLanes.AtomLength)
                throw new ArgumentException("A fountain atom is 16 bytes.", nameof(atoms));
            copies.Add((byte[])atom.Clone());
        }

        Header = header;
        FirstId = firstId;
        this.atoms = copies.ToArray();
    }

    private PetalAtomPacket(PetalLaneHeader header, uint firstId, byte[][] owned)
    {
        Header = header;
        FirstId = firstId;
        atoms = owned;
    }

    /// <summary>Lane header.</summary>
    public PetalLaneHeader Header { get; }

    /// <summary>Fountain id of the first atom; the rest follow consecutively.</summary>
    public uint FirstId { get; }

    /// <summary>The 16-byte atoms.</summary>
    public IReadOnlyList<byte[]> Atoms => atoms;

    /// <summary>Wraps freshly parsed atoms without copying.</summary>
    internal static PetalAtomPacket Owned(PetalLaneHeader header, uint firstId, byte[][] owned) =>
        new(header, firstId, owned);

    /// <inheritdoc />
    public bool Equals(PetalAtomPacket? other)
    {
        if (other is null || Header != other.Header || FirstId != other.FirstId || atoms.Length != other.atoms.Length)
            return false;
        for (var i = 0; i < atoms.Length; i++)
        {
            if (!atoms[i].AsSpan().SequenceEqual(other.atoms[i]))
                return false;
        }

        return true;
    }

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as PetalAtomPacket);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Header);
        hash.Add(FirstId);
        foreach (var atom in atoms)
            hash.AddBytes(atom);
        return hash.ToHashCode();
    }
}

/// <summary>What lane <c>D</c> carried: the stream beacon or one payload atom.</summary>
public sealed class PetalDLane
{
    private PetalDLane(PetalBeacon? beacon, PetalAtomPacket? atoms)
    {
        Beacon = beacon;
        Atoms = atoms;
    }

    /// <summary>The beacon, on beacon frames.</summary>
    public PetalBeacon? Beacon { get; }

    /// <summary>The atom packet, on other frames.</summary>
    public PetalAtomPacket? Atoms { get; }

    /// <summary>Whether this lane carried the beacon.</summary>
    public bool IsBeacon => Beacon.HasValue;

    /// <summary>Wraps a beacon.</summary>
    /// <param name="beacon">The beacon.</param>
    /// <returns>A beacon lane.</returns>
    public static PetalDLane FromBeacon(PetalBeacon beacon) => new(beacon, null);

    /// <summary>Wraps an atom packet.</summary>
    /// <param name="atoms">The packet.</param>
    /// <returns>An atom lane.</returns>
    public static PetalDLane FromAtoms(PetalAtomPacket atoms)
    {
        ArgumentNullException.ThrowIfNull(atoms);
        return new PetalDLane(null, atoms);
    }
}

/// <summary>
/// The bytes of every lane of one frame, turn and kana lanes included: lane data, or transmitted
/// codewords.
/// </summary>
public sealed class PetalLaneData : IEquatable<PetalLaneData>
{
    private readonly byte[][] fields;
    private readonly byte[][] turns;
    private readonly byte[][] kana;

    /// <summary>Creates the bytes of one frame.</summary>
    /// <param name="d">Lane <c>D</c>.</param>
    /// <param name="fields">The four field lanes, sector 0 first.</param>
    /// <param name="turns">The four turn lanes, sector 0 first.</param>
    /// <param name="kana">The four kana lanes in <see cref="PetalKana.Lanes"/> order: <c>P0</c>, <c>K0</c>, <c>P1</c>, <c>K1</c>.</param>
    /// <exception cref="ArgumentException">There are not exactly four lanes of each kind.</exception>
    public PetalLaneData(byte[] d, IReadOnlyList<byte[]> fields, IReadOnlyList<byte[]> turns, IReadOnlyList<byte[]> kana)
    {
        ArgumentNullException.ThrowIfNull(d);
        D = d;
        this.fields = Sectors(fields, nameof(fields));
        this.turns = Sectors(turns, nameof(turns));
        this.kana = Sectors(kana, nameof(kana));
        Fields = Array.AsReadOnly(this.fields);
        Turns = Array.AsReadOnly(this.turns);
        Kana = Array.AsReadOnly(this.kana);
    }

    /// <summary>Lane <c>D</c>.</summary>
    public byte[] D { get; }

    /// <summary>The field lanes, sector 0 first.</summary>
    public IReadOnlyList<byte[]> Fields { get; }

    /// <summary>The turn lanes, sector 0 first (see <see cref="PetalTurns"/>).</summary>
    public IReadOnlyList<byte[]> Turns { get; }

    /// <summary>
    /// The kana lanes in <see cref="PetalKana.Lanes"/> order: <c>P0</c>, <c>K0</c> (the band above the
    /// square), <c>P1</c>, <c>K1</c> (the band below it); see <see cref="PetalKana"/>.
    /// </summary>
    public IReadOnlyList<byte[]> Kana { get; }

    /// <summary>The bytes of <paramref name="lane"/>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>The lane's bytes.</returns>
    public byte[] Lane(PetalLane lane) => PetalLanes.IsField(lane) ? fields[PetalLanes.Sector(lane)] : D;

    /// <summary>The bytes of kana lane <paramref name="lane"/>.</summary>
    /// <param name="lane">The kana lane.</param>
    /// <returns>The lane's bytes.</returns>
    public byte[] Lane(PetalKanaLane lane)
    {
        if ((uint)lane >= (uint)kana.Length)
            throw new ArgumentOutOfRangeException(nameof(lane));
        return kana[(int)lane];
    }

    /// <inheritdoc />
    public bool Equals(PetalLaneData? other)
    {
        if (other is null || !D.AsSpan().SequenceEqual(other.D))
            return false;
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            if (!fields[sector].AsSpan().SequenceEqual(other.fields[sector]) || !turns[sector].AsSpan().SequenceEqual(other.turns[sector]))
                return false;
        }

        for (var lane = 0; lane < kana.Length; lane++)
        {
            if (!kana[lane].AsSpan().SequenceEqual(other.kana[lane]))
                return false;
        }

        return true;
    }

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as PetalLaneData);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.AddBytes(D);
        foreach (var field in fields)
            hash.AddBytes(field);
        foreach (var turn in turns)
            hash.AddBytes(turn);
        foreach (var lane in kana)
            hash.AddBytes(lane);
        return hash.ToHashCode();
    }

    /// <summary>Copies the four lanes of one kind (one per sector, or the four kana lanes).</summary>
    private static byte[][] Sectors(IReadOnlyList<byte[]> lanes, string name)
    {
        ArgumentNullException.ThrowIfNull(lanes, name);
        if (lanes.Count != PetalLayout.Sectors)
            throw new ArgumentException("A frame has four lanes of each kind.", name);
        var copies = new byte[PetalLayout.Sectors][];
        for (var sector = 0; sector < copies.Length; sector++)
            copies[sector] = lanes[sector] ?? throw new ArgumentNullException(name);
        return copies;
    }
}

/// <summary>
/// Petal payload-stream framing shared by <see cref="PetalStreamEncoder"/> and
/// <see cref="PetalStreamAssembler"/>.
/// </summary>
/// <remarks>
/// <para>
/// Every frame carries a handful of fountain atoms: each of the four field
/// lanes five atoms, and lane <c>D</c> one atom — except on every fourth frame
/// (<c>frame % 4 == 0</c>), when lane <c>D</c> carries the stream beacon
/// instead, so a receiver can join at any frame within a fraction of a second.
/// Atom ids run contiguously over the atoms actually sent (see
/// <see cref="FirstAtomId"/>). Any single readable lane is useful on its own.
/// On top of every field lane, the turns of its lit petals carry four more
/// atoms from the repair range (see <see cref="PetalTurns"/>), so a sharp
/// camera reads up to 37 atoms (592 bytes) from the square. The katakana bands
/// above and below the square add eight atoms each (see <see cref="PetalKana"/>):
/// up to 53 atoms (848 bytes) a frame.
/// </para>
/// <para>
/// Lane data layouts (all big-endian): every lane but the turn and kana lanes
/// starts with <c>tag:u8, frame:u16</c> where <c>tag</c> is the low byte of the
/// payload CRC-32C; the field lanes and non-beacon <c>D</c> continue with
/// 16-byte atoms; the beacon continues with
/// <c>version:u8, kind:u8, len:u24, crc:u32</c>, zero padded. A turn lane is
/// <c>atoms… check:u32</c>, where the check binds the atoms to the header of
/// the field lane they ride on; a kana lane is <c>atoms… check:u32</c>, where
/// the check binds the atoms to the stream tag, the frame and the lane.
/// </para>
/// </remarks>
public static class PetalStream
{
    /// <summary>Version/profile byte of the beacon: format version 5, layout profile 0.</summary>
    /// <remarks>
    /// Any other version is refused, the retired formats 3 (<c>0x30</c>, no katakana bands),
    /// 2 (<c>0x20</c>, unturned petals) and 1 (<c>0x10</c>, katakana tiles only) included.
    /// </remarks>
    public const byte FormatVersion = 0x50;

    /// <summary>Largest payload a beacon can describe (<c>u24</c>).</summary>
    public const int MaxPayloadLength = (1 << 24) - 1;

    /// <summary>Default receiver payload limit; override with <see cref="PetalAssemblerLimits"/>.</summary>
    public const int DefaultMaxPayloadLength = 65_536;

    /// <summary>A beacon replaces the lane-<c>D</c> atom on frames divisible by this interval.</summary>
    public const int BeaconInterval = 4;

    /// <summary>Bytes of the beacon body before its zero padding.</summary>
    internal const int BeaconBodyLength = 9;

    /// <summary>Whether <paramref name="frame"/> carries the beacon in lane <c>D</c>.</summary>
    /// <param name="frame">Frame counter.</param>
    /// <returns><see langword="true"/> on every fourth frame.</returns>
    public static bool IsBeaconFrame(ushort frame) => frame % BeaconInterval == 0;

    /// <summary>Fountain atoms carried by <paramref name="frame"/>.</summary>
    /// <param name="frame">Frame counter.</param>
    /// <returns>20 on beacon frames, otherwise 21.</returns>
    public static int AtomsInFrame(ushort frame) => IsBeaconFrame(frame)
        ? PetalLanes.AtomsPerFrame - PetalLanes.DAtoms
        : PetalLanes.AtomsPerFrame;

    /// <summary>Fountain id of the first atom of <paramref name="frame"/>.</summary>
    /// <remarks>
    /// Frame <c>f</c> follows <c>f</c> earlier frames, <c>ceil(f / 4)</c> of
    /// which were beacon frames with one atom fewer.
    /// </remarks>
    /// <param name="frame">Frame counter.</param>
    /// <returns>The id of the frame's first atom.</returns>
    public static uint FirstAtomId(ushort frame)
    {
        uint f = frame;
        return f * PetalLanes.AtomsPerFrame - (f + BeaconInterval - 1) / BeaconInterval;
    }

    /// <summary>Fountain id of the first atom <paramref name="lane"/> carries on <paramref name="frame"/>.</summary>
    /// <remarks>The field lanes come first, in sector order, then lane <c>D</c>.</remarks>
    /// <param name="lane">The lane.</param>
    /// <param name="frame">Frame counter.</param>
    /// <returns>The lane's first atom id.</returns>
    public static uint LaneFirstId(PetalLane lane, ushort frame)
    {
        var start = FirstAtomId(frame);
        return PetalLanes.IsField(lane)
            ? start + (uint)(PetalLanes.Sector(lane) * PetalLanes.FieldAtoms)
            : start + (uint)(PetalLayout.Sectors * PetalLanes.FieldAtoms);
    }

    /// <summary>Parses the data bytes of a field lane.</summary>
    /// <param name="lane">The lane (<c>D</c> is rejected).</param>
    /// <param name="data">Decoded lane data.</param>
    /// <returns>The atoms, or <see langword="null"/> for lane <c>D</c> or a wrong length.</returns>
    public static PetalAtomPacket? ParseAtomLane(PetalLane lane, ReadOnlySpan<byte> data)
    {
        if (!PetalLanes.IsField(lane) || data.Length != PetalLanes.DataLength(lane))
            return null;
        return ParseAtoms(lane, data, ParseHeader(data), PetalLanes.FieldAtoms);
    }

    /// <summary>Parses the data bytes of lane <c>D</c>.</summary>
    /// <param name="data">Decoded lane data.</param>
    /// <returns>
    /// The beacon or atom, or <see langword="null"/> for a wrong length, an
    /// unknown format version or a zero-length beacon.
    /// </returns>
    public static PetalDLane? ParseDLane(ReadOnlySpan<byte> data)
    {
        if (data.Length != PetalLanes.DDataLength)
            return null;
        var header = ParseHeader(data);
        if (!IsBeaconFrame(header.Frame))
            return PetalDLane.FromAtoms(ParseAtoms(PetalLane.D, data, header, PetalLanes.DAtoms));
        var body = data[PetalLanes.LaneHeaderLength..];
        if (body[0] != FormatVersion)
            return null;
        var length = (uint)((body[2] << 16) | (body[3] << 8) | body[4]);
        if (length == 0)
            return null;
        var meta = new PetalStreamMeta(body[1], length, BinaryPrimitives.ReadUInt32BigEndian(body[5..9]));
        return PetalDLane.FromBeacon(new PetalBeacon(header, meta));
    }

    private static PetalLaneHeader ParseHeader(ReadOnlySpan<byte> data) =>
        new(data[0], BinaryPrimitives.ReadUInt16BigEndian(data[1..3]));

    private static PetalAtomPacket ParseAtoms(PetalLane lane, ReadOnlySpan<byte> data, PetalLaneHeader header, int count)
    {
        var atoms = new byte[count][];
        for (var i = 0; i < count; i++)
            atoms[i] = data.Slice(PetalLanes.LaneHeaderLength + i * PetalLanes.AtomLength, PetalLanes.AtomLength).ToArray();
        return PetalAtomPacket.Owned(header, LaneFirstId(lane, header.Frame), atoms);
    }
}
