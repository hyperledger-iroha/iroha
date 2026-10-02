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
/// Petal payload-stream framing shared by <see cref="PetalStreamEncoder"/> and
/// <see cref="PetalStreamAssembler"/>.
/// </summary>
/// <remarks>
/// <para>
/// Every frame carries a handful of fountain atoms: lane <c>P</c> one atom,
/// lane <c>K</c> five, and lane <c>D</c> one atom — except on every fourth
/// frame (<c>frame % 4 == 0</c>), when lane <c>D</c> carries the stream beacon
/// instead, so a receiver can join at any frame within a fraction of a second.
/// Atom ids run contiguously over the atoms actually sent (see
/// <see cref="FirstAtomId"/>). Any single readable lane is useful on its own.
/// </para>
/// <para>
/// Lane data layouts (all big-endian): every lane starts with
/// <c>tag:u8, frame:u16</c> where <c>tag</c> is the low byte of the payload
/// CRC-32C; lanes <c>P</c>, <c>K</c> and non-beacon <c>D</c> continue with
/// 16-byte atoms; the beacon continues with
/// <c>version:u8, kind:u8, len:u24, crc:u32</c>, zero padded.
/// </para>
/// </remarks>
public static class PetalStream
{
    /// <summary>Version/profile byte of the beacon: format version 1, layout profile 0.</summary>
    public const byte FormatVersion = 0x10;

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
    /// <returns>6 on beacon frames, otherwise 7.</returns>
    public static int AtomsInFrame(ushort frame) => IsBeaconFrame(frame)
        ? PetalLanes.PAtoms + PetalLanes.KAtoms
        : PetalLanes.PAtoms + PetalLanes.DAtoms + PetalLanes.KAtoms;

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
    /// <param name="lane">The lane.</param>
    /// <param name="frame">Frame counter.</param>
    /// <returns>The lane's first atom id.</returns>
    public static uint LaneFirstId(PetalLane lane, ushort frame)
    {
        var start = FirstAtomId(frame);
        return lane switch
        {
            PetalLane.P => start,
            PetalLane.D => start + PetalLanes.PAtoms,
            PetalLane.K => start + (uint)(PetalLanes.PAtoms + (IsBeaconFrame(frame) ? 0 : PetalLanes.DAtoms)),
            _ => throw new ArgumentOutOfRangeException(nameof(lane)),
        };
    }

    /// <summary>Parses the data bytes of lane <c>P</c> or lane <c>K</c>.</summary>
    /// <param name="lane">The lane (<c>D</c> is rejected).</param>
    /// <param name="data">Decoded lane data.</param>
    /// <returns>The atoms, or <see langword="null"/> for lane <c>D</c> or a wrong length.</returns>
    public static PetalAtomPacket? ParseAtomLane(PetalLane lane, ReadOnlySpan<byte> data)
    {
        var count = lane switch
        {
            PetalLane.P => PetalLanes.PAtoms,
            PetalLane.K => PetalLanes.KAtoms,
            _ => 0,
        };
        if (count == 0 || data.Length != PetalLanes.DataLength(lane))
            return null;
        return ParseAtoms(lane, data, ParseHeader(data), count);
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
