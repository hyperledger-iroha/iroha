using System.Buffers.Binary;

namespace Hyperledger.Iroha.Petal;

/// <summary>
/// Sender side of a Petal stream: turns one payload into an endless sequence
/// of frames.
/// </summary>
/// <remarks>
/// Frame output is bit-identical to the Rust reference encoder (pinned by
/// <c>fixtures/petal/petal_stream_v3.json</c>), turn lanes included. The frame
/// counter wraps at 65536 on a beacon frame, so atom ids repeat cleanly.
/// </remarks>
public sealed class PetalStreamEncoder
{
    private readonly byte[][] source;

    /// <summary>Prepares <paramref name="payload"/> of application kind <paramref name="kind"/> for streaming.</summary>
    /// <param name="payload">Payload bytes, 1 to <see cref="PetalStream.MaxPayloadLength"/>.</param>
    /// <param name="kind">Application payload kind carried by the beacon.</param>
    /// <exception cref="PetalStreamException">The payload is empty or too large.</exception>
    public PetalStreamEncoder(ReadOnlySpan<byte> payload, byte kind)
    {
        if (payload.IsEmpty)
            throw new PetalStreamException(PetalStreamErrorCode.EmptyPayload, "Petal stream payload is empty.");
        if (payload.Length > PetalStream.MaxPayloadLength)
        {
            throw new PetalStreamException(
                PetalStreamErrorCode.PayloadTooLarge,
                "Petal stream payload exceeds the 24-bit length field.");
        }

        Meta = new PetalStreamMeta(kind, (uint)payload.Length, PetalCrc32C.Compute(payload));
        source = PetalFountain.SplitPayload(payload);
        var frames = 0;
        var atoms = 0;
        while (atoms < source.Length)
        {
            atoms += PetalStream.AtomsInFrame((ushort)frames);
            frames++;
        }

        SystematicFrames = frames;
    }

    /// <summary>Stream identity.</summary>
    public PetalStreamMeta Meta { get; }

    /// <summary>Frames needed to send every source atom once (no losses, no repair).</summary>
    public int SystematicFrames { get; }

    /// <summary>The data bytes of every lane of <paramref name="frame"/>, turn lanes included.</summary>
    /// <param name="frame">Frame counter.</param>
    /// <returns>
    /// Lane data for <c>D</c> (19 B), the four field lanes (83 B each) and the four turn lanes
    /// (68 B each).
    /// </returns>
    public PetalLaneData LaneData(ushort frame)
    {
        var fields = new byte[PetalLayout.Sectors][];
        foreach (var lane in PetalLanes.Fields)
        {
            var data = NewLane(lane, frame);
            WriteAtoms(data.AsSpan(PetalLanes.LaneHeaderLength), PetalStream.LaneFirstId(lane, frame), PetalLanes.FieldAtoms);
            fields[PetalLanes.Sector(lane)] = data;
        }

        var d = NewLane(PetalLane.D, frame);
        if (PetalStream.IsBeaconFrame(frame))
        {
            var body = d.AsSpan(PetalLanes.LaneHeaderLength);
            body[0] = PetalStream.FormatVersion;
            body[1] = Meta.Kind;
            body[2] = (byte)(Meta.Length >> 16);
            body[3] = (byte)(Meta.Length >> 8);
            body[4] = (byte)Meta.Length;
            BinaryPrimitives.WriteUInt32BigEndian(body[5..PetalStream.BeaconBodyLength], Meta.Crc);
        }
        else
        {
            WriteAtoms(d.AsSpan(PetalLanes.LaneHeaderLength), PetalStream.LaneFirstId(PetalLane.D, frame), PetalLanes.DAtoms);
        }

        var header = new PetalLaneHeader(Meta.Tag, frame);
        var turns = new byte[PetalLayout.Sectors][];
        Span<byte> atoms = stackalloc byte[PetalTurns.TurnAtoms * PetalLanes.AtomLength];
        for (var sector = 0; sector < turns.Length; sector++)
        {
            WriteAtoms(atoms, PetalTurns.TurnFirstId(frame, sector), PetalTurns.TurnAtoms);
            turns[sector] = PetalTurns.TurnData(header, sector, atoms);
        }

        return new PetalLaneData(d, fields, turns);
    }

    /// <summary>The transmitted (whitened Reed–Solomon) codewords of <paramref name="frame"/>.</summary>
    /// <param name="frame">Frame counter.</param>
    /// <returns>
    /// Codewords for <c>D</c> (30 B), the four field lanes (105 B each) and the four turn lanes
    /// (90 B each).
    /// </returns>
    public PetalLaneData Words(ushort frame)
    {
        var data = LaneData(frame);
        var fields = new byte[PetalLayout.Sectors][];
        var turns = new byte[PetalLayout.Sectors][];
        for (var sector = 0; sector < fields.Length; sector++)
        {
            fields[sector] = PetalLanes.EncodeLane(PetalLanes.Field(sector), data.Fields[sector]);
            turns[sector] = PetalTurns.EncodeTurnLane(sector, data.Turns[sector]);
        }

        return new PetalLaneData(PetalLanes.EncodeLane(PetalLane.D, data.D), fields, turns);
    }

    /// <summary>Every cell of <paramref name="frame"/>, lit and turned, ready to render.</summary>
    /// <param name="frame">Frame counter.</param>
    /// <returns>The frame cells.</returns>
    public PetalFrameCells Cells(ushort frame) => PetalFrameCells.FromWords(Words(frame));

    private byte[] NewLane(PetalLane lane, ushort frame)
    {
        var data = new byte[PetalLanes.DataLength(lane)];
        data[0] = Meta.Tag;
        data[1] = (byte)(frame >> 8);
        data[2] = (byte)frame;
        return data;
    }

    /// <summary>Writes <paramref name="count"/> consecutive atoms from <paramref name="firstId"/> to the start of <paramref name="target"/>.</summary>
    private void WriteAtoms(Span<byte> target, uint firstId, int count)
    {
        for (var i = 0; i < count; i++)
        {
            var atom = target.Slice(i * PetalLanes.AtomLength, PetalLanes.AtomLength);
            PetalFountain.EncodeAtom(source, Meta.Crc, firstId + (uint)i, atom);
        }
    }
}
