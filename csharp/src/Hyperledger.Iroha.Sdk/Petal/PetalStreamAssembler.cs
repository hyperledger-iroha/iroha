namespace Hyperledger.Iroha.Petal;

/// <summary>Receiver limits that bound memory and work.</summary>
public sealed record PetalAssemblerLimits
{
    private readonly int maxPayloadLength = PetalStream.DefaultMaxPayloadLength;
    private readonly int maxPendingAtoms = 128;

    /// <summary>Default limits: 64 KiB payloads, 128 pending atoms.</summary>
    public static PetalAssemblerLimits Default { get; } = new();

    /// <summary>Largest payload the receiver accepts; larger beacons are ignored.</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is negative.</exception>
    public int MaxPayloadLength
    {
        get => maxPayloadLength;
        init
        {
            ArgumentOutOfRangeException.ThrowIfNegative(value);
            maxPayloadLength = value;
        }
    }

    /// <summary>
    /// Atoms buffered while waiting for the first beacon (oldest dropped first);
    /// <c>0</c> buffers nothing and drops atoms that precede the first beacon.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is negative.</exception>
    public int MaxPendingAtoms
    {
        get => maxPendingAtoms;
        init
        {
            ArgumentOutOfRangeException.ThrowIfNegative(value);
            maxPendingAtoms = value;
        }
    }
}

/// <summary>A reassembled, CRC-verified payload.</summary>
public sealed class PetalCompletedPayload
{
    private readonly byte[] payload;

    internal PetalCompletedPayload(PetalStreamMeta meta, byte[] payload)
    {
        Meta = meta;
        this.payload = payload;
    }

    /// <summary>Stream identity.</summary>
    public PetalStreamMeta Meta { get; }

    /// <summary>The payload bytes.</summary>
    public ReadOnlyMemory<byte> Payload => payload;

    /// <summary>A copy of the payload bytes.</summary>
    /// <returns>A fresh array.</returns>
    public byte[] ToArray() => (byte[])payload.Clone();
}

/// <summary>Snapshot of receive progress for a UI.</summary>
/// <param name="Meta">Stream identity once a beacon was accepted.</param>
/// <param name="SourceAtoms">Source atoms of the active stream.</param>
/// <param name="Rank">Independent atoms collected so far.</param>
/// <param name="AtomsReceived">Atoms offered to the decoder (including duplicates).</param>
/// <param name="IntegrityFailures">
/// Reassembled payloads that failed the CRC check and were discarded (cumulative
/// over the assembler's lifetime, not cleared by <see cref="PetalStreamAssembler.Reset"/>).
/// </param>
/// <param name="Complete">Whether the payload is complete and verified.</param>
public readonly record struct PetalProgress(
    PetalStreamMeta? Meta,
    int SourceAtoms,
    int Rank,
    uint AtomsReceived,
    uint IntegrityFailures,
    bool Complete);

/// <summary>
/// Receiver side of a Petal stream: collects atoms from any lane of any frame
/// and reassembles the payload.
/// </summary>
/// <remarks>
/// <para>
/// The first accepted beacon starts a stream; a different stream replaces it
/// only after two consecutive sightings of the same new beacon. Atoms seen
/// before the first beacon wait in a bounded queue. A reassembled payload is
/// released only when its CRC-32C matches the beacon; otherwise the
/// elimination restarts and <see cref="PetalProgress.IntegrityFailures"/> grows.
/// </para>
/// <para>Instances are not thread-safe.</para>
/// </remarks>
public sealed class PetalStreamAssembler
{
    private readonly Queue<(byte Tag, uint Id, byte[] Atom)> pending = new();
    private Active? active;
    private (PetalStreamMeta Meta, byte Seen)? conflicting;
    private PetalCompletedPayload? completed;
    private uint atomsReceived;
    private uint integrityFailures;

    /// <summary>Creates an assembler.</summary>
    /// <param name="limits">Limits; <see cref="PetalAssemblerLimits.Default"/> when omitted.</param>
    public PetalStreamAssembler(PetalAssemblerLimits? limits = null)
    {
        Limits = limits ?? PetalAssemblerLimits.Default;
    }

    /// <summary>The limits in force.</summary>
    public PetalAssemblerLimits Limits { get; }

    /// <summary>Current progress.</summary>
    public PetalProgress Progress => active is null
        ? new PetalProgress(null, 0, 0, atomsReceived, integrityFailures, false)
        : new PetalProgress(
            active.Meta,
            active.Decoder.SourceAtoms,
            active.Decoder.Rank,
            atomsReceived,
            integrityFailures,
            active.Done);

    /// <summary>
    /// Forgets the active stream, pending atoms and any completed payload.
    /// <see cref="PetalProgress.IntegrityFailures"/> stays cumulative and is not cleared.
    /// </summary>
    public void Reset()
    {
        active = null;
        pending.Clear();
        conflicting = null;
        completed = null;
        atomsReceived = 0;
    }

    /// <summary>Takes the completed payload, if any; it is delivered exactly once.</summary>
    /// <returns>The payload, or <see langword="null"/>.</returns>
    public PetalCompletedPayload? TakeCompleted()
    {
        var result = completed;
        completed = null;
        return result;
    }

    /// <summary>Offers a beacon read from lane <c>D</c>.</summary>
    /// <param name="beacon">The beacon.</param>
    public void PushBeacon(PetalBeacon beacon)
    {
        var meta = beacon.Meta;
        if (meta.Length == 0 || meta.Length > (uint)Limits.MaxPayloadLength)
            return;
        if (active is null)
        {
            Start(meta);
        }
        else if (active.Meta == meta)
        {
            conflicting = null;
        }
        else
        {
            // A different stream: switch only after two consecutive sightings.
            var seen = conflicting is { } candidate && candidate.Meta == meta ? (byte)(candidate.Seen + 1) : (byte)1;
            if (seen >= 2)
                Start(meta);
            else
                conflicting = (meta, seen);
        }
    }

    /// <summary>Offers atoms read from a lane.</summary>
    /// <param name="packet">The packet.</param>
    public void PushAtoms(PetalAtomPacket packet)
    {
        ArgumentNullException.ThrowIfNull(packet);
        for (var index = 0; index < packet.Atoms.Count; index++)
        {
            var id = packet.FirstId + (uint)index;
            var atom = packet.Atoms[index];
            if (active is not null)
            {
                if (active.Meta.Tag == packet.Header.Tag)
                    AddAtom(id, atom);
            }
            else
            {
                if (Limits.MaxPendingAtoms == 0)
                    continue;
                if (pending.Count >= Limits.MaxPendingAtoms)
                    pending.TryDequeue(out _);
                pending.Enqueue((packet.Header.Tag, id, (byte[])atom.Clone()));
            }
        }
    }

    /// <summary>Offers whatever lane <c>D</c> carried.</summary>
    /// <param name="lane">The parsed lane.</param>
    public void PushDLane(PetalDLane lane)
    {
        ArgumentNullException.ThrowIfNull(lane);
        if (lane.Beacon is { } beacon)
            PushBeacon(beacon);
        else if (lane.Atoms is { } atoms)
            PushAtoms(atoms);
    }

    private void Start(PetalStreamMeta meta)
    {
        active = new Active(meta, new PetalFountainDecoder(meta.SourceAtoms));
        conflicting = null;
        completed = null;
        atomsReceived = 0;
        var tag = meta.Tag;
        var drained = pending.ToArray();
        pending.Clear();
        foreach (var (pendingTag, id, atom) in drained)
        {
            if (pendingTag == tag)
                AddAtom(id, atom);
        }
    }

    private void AddAtom(uint id, byte[] atom)
    {
        if (active is null || active.Done)
            return;
        if (atomsReceived != uint.MaxValue)
            atomsReceived++;
        active.Decoder.AddEncoded(active.Meta.Crc, id, atom);
        if (!active.Decoder.IsComplete)
            return;
        var source = active.Decoder.Solve();
        if (source is null)
            return;
        var payload = new byte[active.Meta.Length];
        for (var i = 0; i < source.Length; i++)
        {
            var start = i * PetalLanes.AtomLength;
            var count = Math.Min(PetalLanes.AtomLength, payload.Length - start);
            if (count <= 0)
                break;
            source[i].AsSpan(0, count).CopyTo(payload.AsSpan(start));
        }

        if (PetalCrc32C.Compute(payload) == active.Meta.Crc)
        {
            active.Done = true;
            completed = new PetalCompletedPayload(active.Meta, payload);
        }
        else
        {
            // Corrupt atoms slipped through: start the elimination over.
            if (integrityFailures != uint.MaxValue)
                integrityFailures++;
            active.Decoder = new PetalFountainDecoder(active.Meta.SourceAtoms);
        }
    }

    private sealed class Active(PetalStreamMeta meta, PetalFountainDecoder decoder)
    {
        public PetalStreamMeta Meta { get; } = meta;

        public PetalFountainDecoder Decoder { get; set; } = decoder;

        public bool Done { get; set; }
    }
}
