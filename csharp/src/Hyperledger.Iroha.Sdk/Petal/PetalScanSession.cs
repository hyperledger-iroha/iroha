namespace Hyperledger.Iroha.Petal;

/// <summary>Limits of a scan session.</summary>
public sealed record PetalScanLimits
{
    private readonly TimeSpan idleTimeout = TimeSpan.FromSeconds(30);
    private readonly TimeSpan absoluteTimeout = TimeSpan.FromSeconds(180);
    private readonly PetalAssemblerLimits assembler = PetalAssemblerLimits.Default;
    private readonly PetalDecodeOptions decode = PetalDecodeOptions.Default;

    /// <summary>The reference defaults: 30 s idle, 180 s absolute, default assembler and decoder.</summary>
    public static PetalScanLimits Default { get; } = new();

    /// <summary>Forget a half-received stream after this long without progress (whole milliseconds).</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is negative.</exception>
    public TimeSpan IdleTimeout
    {
        get => idleTimeout;
        init
        {
            ArgumentOutOfRangeException.ThrowIfLessThan(value, TimeSpan.Zero);
            idleTimeout = value;
        }
    }

    /// <summary>Forget a stream that has not finished this long after it started (whole milliseconds).</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is negative.</exception>
    public TimeSpan AbsoluteTimeout
    {
        get => absoluteTimeout;
        init
        {
            ArgumentOutOfRangeException.ThrowIfLessThan(value, TimeSpan.Zero);
            absoluteTimeout = value;
        }
    }

    /// <summary>Assembler memory and size limits.</summary>
    public PetalAssemblerLimits Assembler
    {
        get => assembler;
        init
        {
            ArgumentNullException.ThrowIfNull(value);
            assembler = value;
        }
    }

    /// <summary>Image decoder options.</summary>
    public PetalDecodeOptions Decode
    {
        get => decode;
        init
        {
            ArgumentNullException.ThrowIfNull(value);
            decode = value;
        }
    }
}

/// <summary>Counters for diagnostics and UI hints.</summary>
/// <param name="Frames">Camera frames offered.</param>
/// <param name="Located">Frames in which a code was located, whether or not a lane could be read.</param>
/// <param name="Readable">Frames in which at least one lane decoded.</param>
/// <param name="LaneD">Lane <c>D</c> successes.</param>
/// <param name="FieldLanes">Field lane successes (up to four per frame).</param>
/// <param name="Tracked">Frames read by tracking the previous pose instead of a full search.</param>
/// <param name="Inferred">Frames read with one corner finder hidden and inferred (a hint to move a thumb or the phone).</param>
/// <param name="Merges">Merges of two or more captures of one shown frame.</param>
/// <param name="MergedLanes">Field lanes recovered by merging captures.</param>
public readonly record struct PetalScanStats(
    uint Frames,
    uint Located,
    uint Readable,
    uint LaneD,
    uint FieldLanes,
    uint Tracked,
    uint Inferred,
    uint Merges,
    uint MergedLanes);

/// <summary>The result of offering one camera frame.</summary>
public sealed class PetalScanOutcome
{
    internal PetalScanOutcome(PetalDecodeError? error, string lanes, string merged, PetalProgress progress, PetalCompletedPayload? completed)
    {
        Error = error;
        Lanes = lanes;
        Merged = merged;
        Progress = progress;
        Completed = completed;
    }

    /// <summary>
    /// Why the frame produced nothing, when it did not.
    /// <see cref="PetalDecodeError.NoOrientation"/> means a code was located but no
    /// lane could be read (too far, too blurry).
    /// </summary>
    public PetalDecodeError? Error { get; }

    /// <summary>
    /// Lanes that decoded in this capture alone, as letters: <c>D</c> and the sector digits
    /// <c>0</c>–<c>3</c>.
    /// </summary>
    public string Lanes { get; }

    /// <summary>
    /// Field lanes recovered by merging this capture with earlier captures of its shown frame,
    /// as sector digits.
    /// </summary>
    public string Merged { get; }

    /// <summary>Receive progress after this frame.</summary>
    public PetalProgress Progress { get; }

    /// <summary>The finished payload, delivered exactly once.</summary>
    public PetalCompletedPayload? Completed { get; }
}

/// <summary>
/// The receive-side object an app holds while its camera is open: decodes
/// camera frames and reassembles the stream they carry.
/// </summary>
/// <remarks>
/// <para>
/// After a frame decodes, the next frames are first read by tracking the code from
/// its last pose (<see cref="PetalDecoder.Track"/>), which skips the finder search;
/// a full <see cref="PetalDecoder.Decode"/> runs when tracking fails or the last
/// pose is older than <see cref="TrackWindowMilliseconds"/>.
/// </para>
/// <para>
/// A camera sees each shown frame two to four times, and noise, display banding or
/// hand shake can cost every one of those captures a field lane. When a capture reads
/// some lane but not every field lane, the session keeps its
/// <see cref="PetalFieldEvidence"/> under the shown frame its decoded lane headers name
/// (a capture whose lanes name different frames is not kept), for the last
/// <see cref="MergeWindow"/> shown frames and at most <see cref="MergeCaptures"/>
/// captures each. From the second such capture on, it merges them
/// (<see cref="PetalDecoder.MergeFields"/>) and offers a recovered lane only when that
/// lane's own header names the same frame. Replacing the active stream drops the held
/// evidence.
/// </para>
/// <para>
/// A half-received stream is forgotten after <see cref="PetalScanLimits.IdleTimeout"/>
/// without new independent atoms, or <see cref="PetalScanLimits.AbsoluteTimeout"/>
/// after it started. Instances are not thread-safe; see
/// <see cref="PetalCameraAnalyzer"/> for a camera-callback wrapper.
/// </para>
/// </remarks>
public sealed class PetalScanSession
{
    /// <summary>How long (in milliseconds) a decoded pose stays usable for tracking the next frames.</summary>
    public const long TrackWindowMilliseconds = 500;

    /// <summary>Shown frames whose captures are held for merging.</summary>
    public const int MergeWindow = 6;

    /// <summary>Captures held per shown frame; a further capture replaces the oldest.</summary>
    public const int MergeCaptures = 4;

    private readonly List<Held> held = new();
    private readonly PetalStreamAssembler assembler;
    private long? startedMs;
    private long progressMs;
    private int lastRank;
    private uint frames;
    private uint located;
    private uint readable;
    private uint laneD;
    private uint fieldLanes;
    private uint tracked;
    private uint inferred;
    private uint merges;
    private uint mergedLanes;
    private PetalDecodedFrame? lastPose;
    private long lastPoseMs;

    /// <summary>Creates a session.</summary>
    /// <param name="limits">Limits; <see cref="PetalScanLimits.Default"/> when omitted.</param>
    public PetalScanSession(PetalScanLimits? limits = null)
    {
        Limits = limits ?? PetalScanLimits.Default;
        assembler = new PetalStreamAssembler(Limits.Assembler);
    }

    /// <summary>The limits in force.</summary>
    public PetalScanLimits Limits { get; }

    /// <summary>Diagnostic counters.</summary>
    public PetalScanStats Stats => new(frames, located, readable, laneD, fieldLanes, tracked, inferred, merges, mergedLanes);

    /// <summary>Current progress.</summary>
    public PetalProgress Progress => assembler.Progress;

    /// <summary>Drops all partial state, the held captures and the pose used for tracking (counters are kept).</summary>
    public void Reset()
    {
        assembler.Reset();
        startedMs = null;
        lastRank = 0;
        lastPose = null;
        held.Clear();
    }

    /// <summary>Offers one camera luma plane captured at monotonic time <paramref name="nowMilliseconds"/>.</summary>
    /// <param name="image">The luma plane.</param>
    /// <param name="nowMilliseconds">Monotonic capture time in milliseconds (for example <see cref="Environment.TickCount64"/>).</param>
    /// <returns>What this frame contributed.</returns>
    public PetalScanOutcome Push(PetalLuma image, long nowMilliseconds)
    {
        ArgumentNullException.ThrowIfNull(image);
        if (startedMs is { } start
            && (Elapsed(nowMilliseconds, progressMs) > WholeMilliseconds(Limits.IdleTimeout)
                || Elapsed(nowMilliseconds, start) > WholeMilliseconds(Limits.AbsoluteTimeout)))
        {
            Reset();
        }

        frames++;
        var followed = lastPose is not null && Elapsed(nowMilliseconds, lastPoseMs) <= TrackWindowMilliseconds
            ? PetalDecoder.Track(image, lastPose, Limits.Decode)
            : null;
        if (followed is not null)
            tracked++;
        var result = followed is not null ? PetalDecodeResult.Ok(followed) : PetalDecoder.Decode(image, Limits.Decode);
        var lanes = string.Empty;
        var merged = string.Empty;
        if (result.Success)
        {
            if (result.Frame.InferredCorner is not null)
                inferred++;
            lastPose = result.Frame;
            lastPoseMs = nowMilliseconds;
            lanes = Absorb(result.Frame);
            merged = Merge(image, result.Frame);
        }

        if (result.Error is not (PetalDecodeError.NoFinders or PetalDecodeError.UnsupportedImage))
            located++;
        var progress = assembler.Progress;
        if (progress.Rank > lastRank || (progress.Meta is not null && startedMs is null))
        {
            progressMs = nowMilliseconds;
            startedMs ??= nowMilliseconds;
        }

        lastRank = progress.Rank;
        return new PetalScanOutcome(result.Error, lanes, merged, progress, assembler.TakeCompleted());
    }

    /// <summary>Counts the lanes of a decoded frame and feeds them to the assembler.</summary>
    internal string Absorb(PetalDecodedFrame frame)
    {
        var lanes = frame.Lanes;
        if (frame.D is not null)
            laneD++;
        foreach (var field in frame.Fields)
        {
            if (field is not null)
                fieldLanes++;
        }

        if (lanes.Length > 0)
            readable++;
        var previousMeta = assembler.Progress.Meta;
        frame.Feed(assembler);
        if (previousMeta is not null && assembler.Progress.Meta != previousMeta)
            held.Clear();
        return lanes;
    }

    /// <summary>
    /// Holds this capture's field evidence under its shown frame and, from the second capture
    /// on, merges the held captures of that frame.
    /// </summary>
    internal string Merge(PetalLuma image, PetalDecodedFrame frame)
    {
        if (ShownFrame(frame) is not { } shown)
            return string.Empty;
        var item = held.Find(h => h.Shown == shown);
        if (item is null)
        {
            if (held.Count == MergeWindow)
                held.RemoveAt(0);
            item = new Held(shown);
            held.Add(item);
        }

        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            item.Done[sector] |= frame.Fields[sector] is not null;
        if (item.AllDone)
        {
            item.Evidence.Clear();
            return string.Empty;
        }

        if (PetalDecoder.FieldEvidence(image, frame, Limits.Decode) is not { } evidence)
            return string.Empty;
        if (item.Evidence.Count == MergeCaptures)
            item.Evidence.RemoveAt(0);
        item.Evidence.Add(evidence);
        if (item.Evidence.Count < 2)
            return string.Empty;
        merges++;
        var results = PetalDecoder.MergeFields(item.Evidence, item.Done);
        var merged = string.Empty;
        foreach (var lane in PetalLanes.Fields)
        {
            var sector = PetalLanes.Sector(lane);
            if (results[sector] is not { } result || PetalStream.ParseAtomLane(lane, result.Data) is not { } packet)
                continue;
            // a merge offers only atoms of the shown frame it was made for
            if (packet.Header != shown)
                continue;
            item.Done[sector] = true;
            assembler.PushAtoms(packet);
            mergedLanes++;
            merged += PetalLanes.Letter(lane);
        }

        if (item.AllDone)
            item.Evidence.Clear();
        return merged;
    }

    /// <summary>The captures held for merging, oldest shown frame first (diagnostics and tests).</summary>
    internal IReadOnlyList<Held> HeldFrames => held;

    /// <summary>
    /// The shown frame every decoded lane header of <paramref name="frame"/> names;
    /// <see langword="null"/> when no lane decoded or the lanes of a torn capture name different
    /// frames.
    /// </summary>
    private static PetalLaneHeader? ShownFrame(PetalDecodedFrame frame)
    {
        PetalLaneHeader? shown = null;
        foreach (var lane in PetalLanes.DecodeOrder)
        {
            if (frame.Lane(lane) is not { } result)
                continue;
            if (result.Data.Length < PetalLanes.LaneHeaderLength)
                return null;
            var header = new PetalLaneHeader(result.Data[0], (ushort)((result.Data[1] << 8) | result.Data[2]));
            if (shown is { } previous && previous != header)
                return null;
            shown = header;
        }

        return shown;
    }

    /// <summary>Saturating <c>now - since</c> (Rust <c>u64::saturating_sub</c>).</summary>
    private static ulong Elapsed(long now, long since) => now <= since ? 0 : unchecked((ulong)now - (ulong)since);

    private static ulong WholeMilliseconds(TimeSpan timeout) => (ulong)(timeout.Ticks / TimeSpan.TicksPerMillisecond);

    /// <summary>The captures of one shown frame held for merging.</summary>
    internal sealed class Held(PetalLaneHeader shown)
    {
        public PetalLaneHeader Shown { get; } = shown;

        public List<PetalFieldEvidence> Evidence { get; } = new();

        /// <summary>The field lanes already read from this shown frame, sector 0 first.</summary>
        public bool[] Done { get; } = new bool[PetalLayout.Sectors];

        public bool AllDone => Array.TrueForAll(Done, static done => done);
    }
}
