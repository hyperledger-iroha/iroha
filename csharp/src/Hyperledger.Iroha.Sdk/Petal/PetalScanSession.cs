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
/// <param name="TurnTries">Turn lanes tried (each at most once per shown frame).</param>
/// <param name="TurnLanes">Turn lanes read.</param>
/// <param name="KanaTries">Bands tried (each at most once per shown frame).</param>
/// <param name="KanaLanes">Kana lanes read (up to four per shown frame).</param>
public readonly record struct PetalScanStats(
    uint Frames,
    uint Located,
    uint Readable,
    uint LaneD,
    uint FieldLanes,
    uint Tracked,
    uint Inferred,
    uint Merges,
    uint MergedLanes,
    uint TurnTries,
    uint TurnLanes,
    uint KanaTries,
    uint KanaLanes);

/// <summary>The result of offering one camera frame.</summary>
public sealed class PetalScanOutcome
{
    internal PetalScanOutcome(
        PetalDecodeError? error,
        string lanes,
        string merged,
        string turned,
        string kana,
        PetalProgress progress,
        PetalCompletedPayload? completed)
    {
        Error = error;
        Lanes = lanes;
        Merged = merged;
        Turned = turned;
        Kana = kana;
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

    /// <summary>
    /// Turn lanes read from this capture, as the sector digits of the field lanes they ride on
    /// (see <see cref="PetalTurns"/>).
    /// </summary>
    public string Turned { get; }

    /// <summary>
    /// Kana lanes read from this capture, as their names in <see cref="PetalKana.Lanes"/> order
    /// (<c>P0</c>, <c>K0</c>, <c>P1</c>, <c>K1</c>, concatenated; see <see cref="PetalKana"/>).
    /// </summary>
    public string Kana { get; }

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
/// Every field lane a capture decodes also lets the session read the turn lane on top of it
/// (<see cref="PetalTurns.ReadTurns"/>). A turn lane is tried once per shown frame: the session
/// remembers the last <see cref="TurnMemory"/> it tried and does not look at those petals again.
/// Reading turns costs a slow phone core several times a frame's decode, so after a try that
/// reads no turn lane (a camera too soft or too shaky for the turns) the session passes over the
/// next captures that would try one, one more after every such try, at most
/// <see cref="TurnBackoff"/>, until a try reads a lane again.
/// </para>
/// <para>
/// The katakana bands above and below the square are read the same way
/// (<see cref="PetalKana.ReadKana"/>): each band once per shown frame (the last
/// <see cref="KanaMemory"/> tried are remembered), with its own back-off of at most
/// <see cref="KanaBackoff"/> captures after tries that read no kana lane (a camera that does not see
/// the bands, or too blurred for their glyphs and polarity).
/// </para>
/// <para>
/// The bands go first, and a capture whose bands read a kana lane tries no turn lane: the turn lanes
/// of that shown frame wait for its next capture. On a slow phone core a turn try costs five tracked
/// decodes and a band try one, so reading both on one capture overran the time a frame is shown at
/// 12 fps and the session missed the next shown frames. A decoder that sees each shown frame two or
/// three times still reads both; one that sees it once reads the cheaper bands and every frame's
/// field lanes. A band try that reads nothing (bands out of view, too blurred, or a torn capture)
/// leaves the turn lanes to the same capture.
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

    /// <summary>Turn lanes remembered as tried (the four of each of the last <see cref="MergeWindow"/> shown frames).</summary>
    public const int TurnMemory = MergeWindow * PetalLayout.Sectors;

    /// <summary>Most captures passed over after tries that read no turn lane.</summary>
    public const int TurnBackoff = 8;

    /// <summary>Bands remembered as tried (the two of each of the last <see cref="MergeWindow"/> shown frames).</summary>
    public const int KanaMemory = MergeWindow * PetalLayout.KanaBands;

    /// <summary>Most captures passed over after tries that read no kana lane.</summary>
    public const int KanaBackoff = 8;

    private readonly List<Held> held = new();
    private readonly Queue<(PetalLaneHeader Header, int Sector)> tried = new(TurnMemory);
    private readonly Queue<(PetalLaneHeader Header, int Band)> kanaTried = new(KanaMemory);
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
    private uint turnTries;
    private uint turnLanes;
    private int turnMisses;
    private int turnWait;
    private uint kanaTries;
    private uint kanaLanes;
    private int kanaMisses;
    private int kanaWait;
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
    public PetalScanStats Stats =>
        new(frames, located, readable, laneD, fieldLanes, tracked, inferred, merges, mergedLanes, turnTries, turnLanes, kanaTries, kanaLanes);

    /// <summary>Current progress.</summary>
    public PetalProgress Progress => assembler.Progress;

    /// <summary>
    /// Drops all partial state, the held captures, the turn lanes and bands remembered as tried, any
    /// back-off and the pose used for tracking (counters are kept).
    /// </summary>
    public void Reset()
    {
        assembler.Reset();
        startedMs = null;
        lastRank = 0;
        lastPose = null;
        held.Clear();
        ForgetTurns();
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
        var turnedLanes = string.Empty;
        var kanaLanesRead = string.Empty;
        if (result.Success)
        {
            if (result.Frame.InferredCorner is not null)
                inferred++;
            lastPose = result.Frame;
            lastPoseMs = nowMilliseconds;
            lanes = Absorb(result.Frame);
            merged = Merge(image, result.Frame);
            // a capture whose bands read a lane tries no turn lane (see the class remarks)
            kanaLanesRead = Kana(image, result.Frame);
            if (kanaLanesRead.Length == 0)
                turnedLanes = Turn(image, result.Frame);
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
        return new PetalScanOutcome(result.Error, lanes, merged, turnedLanes, kanaLanesRead, progress, assembler.TakeCompleted());
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
        {
            held.Clear();
            ForgetTurns();
        }

        return lanes;
    }

    /// <summary>
    /// Tries the turn lanes on top of the field lanes this capture decoded, except those already
    /// tried for their shown frame, and offers the atoms of those it reads; after tries that read
    /// nothing it passes over the next captures that would try one (see the class remarks).
    /// </summary>
    internal string Turn(PetalLuma image, PetalDecodedFrame frame)
    {
        Span<bool> skip = stackalloc bool[PetalLayout.Sectors];
        Span<PetalLaneHeader> headers = stackalloc PetalLaneHeader[PetalLayout.Sectors];
        var trying = false;
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            skip[sector] = true;
            if (frame.Fields[sector] is not { Data.Length: >= PetalLanes.LaneHeaderLength } field)
                continue;
            headers[sector] = new PetalLaneHeader(field.Data[0], (ushort)((field.Data[1] << 8) | field.Data[2]));
            skip[sector] = tried.Contains((headers[sector], sector));
            trying |= !skip[sector];
        }

        if (!trying)
            return string.Empty;
        if (turnWait > 0)
        {
            turnWait--;
            return string.Empty;
        }

        // a skipped sector is one without a header or one already tried
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            if (skip[sector])
                continue;
            if (tried.Count == TurnMemory)
                tried.Dequeue();
            tried.Enqueue((headers[sector], sector));
            turnTries++;
        }

        var results = PetalTurns.ReadTurns(image, frame, skip);
        var letters = string.Empty;
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            if (results[sector] is not { } result || PetalTurns.TurnPacket(headers[sector], sector, result.Data) is not { } packet)
                continue;
            assembler.PushAtoms(packet);
            turnLanes++;
            letters += PetalLanes.Letter(PetalLanes.Field(sector));
        }

        if (letters.Length == 0)
        {
            turnMisses = Math.Min(turnMisses + 1, TurnBackoff);
            turnWait = turnMisses;
        }
        else
        {
            turnMisses = 0;
        }

        return letters;
    }

    /// <summary>
    /// Tries the bands not yet tried for this capture's shown frame and offers the atoms of the kana
    /// lanes they read; after tries that read nothing it passes over the next captures that would try
    /// one (see the class remarks).
    /// </summary>
    internal string Kana(PetalLuma image, PetalDecodedFrame frame)
    {
        if (ShownFrame(frame) is not { } header)
            return string.Empty;
        Span<bool> skip = stackalloc bool[PetalLayout.KanaBands];
        var trying = false;
        for (var band = 0; band < PetalLayout.KanaBands; band++)
        {
            skip[band] = kanaTried.Contains((header, band));
            trying |= !skip[band];
        }

        if (!trying)
            return string.Empty;
        if (kanaWait > 0)
        {
            kanaWait--;
            return string.Empty;
        }

        for (var band = 0; band < PetalLayout.KanaBands; band++)
        {
            if (skip[band])
                continue;
            if (kanaTried.Count == KanaMemory)
                kanaTried.Dequeue();
            kanaTried.Enqueue((header, band));
            kanaTries++;
        }

        var results = PetalKana.ReadKana(image, frame, header, skip);
        var names = string.Empty;
        foreach (var lane in PetalKana.Lanes)
        {
            if (results[(int)lane] is not { } result || PetalKana.KanaPacket(result.Header, lane, result.Result.Data) is not { } packet)
                continue;
            assembler.PushAtoms(packet);
            kanaLanes++;
            names += PetalKana.Name(lane);
        }

        if (names.Length == 0)
        {
            kanaMisses = Math.Min(kanaMisses + 1, KanaBackoff);
            kanaWait = kanaMisses;
        }
        else
        {
            kanaMisses = 0;
        }

        return names;
    }

    /// <summary>The bands remembered as tried as <c>(shown frame, band)</c>, oldest first (diagnostics and tests).</summary>
    internal IReadOnlyCollection<(PetalLaneHeader Header, int Band)> TriedBands => kanaTried;

    /// <summary>Tries in a row that read no kana lane, at most <see cref="KanaBackoff"/> (diagnostics and tests).</summary>
    internal int KanaMisses => kanaMisses;

    /// <summary>Captures that would try a band still to be passed over (diagnostics and tests).</summary>
    internal int KanaWait
    {
        get => kanaWait;
        set => kanaWait = value;
    }

    /// <summary>The turn lanes remembered as tried, oldest first (diagnostics and tests).</summary>
    internal IReadOnlyCollection<(PetalLaneHeader Header, int Sector)> TriedLanes => tried;

    /// <summary>Tries in a row that read no turn lane, at most <see cref="TurnBackoff"/> (diagnostics and tests).</summary>
    internal int TurnMisses => turnMisses;

    /// <summary>Captures that would try a turn lane still to be passed over (diagnostics and tests).</summary>
    internal int TurnWait
    {
        get => turnWait;
        set => turnWait = value;
    }

    /// <summary>Forgets the turn lanes and bands tried and any back-off.</summary>
    private void ForgetTurns()
    {
        tried.Clear();
        turnMisses = 0;
        turnWait = 0;
        kanaTried.Clear();
        kanaMisses = 0;
        kanaWait = 0;
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
