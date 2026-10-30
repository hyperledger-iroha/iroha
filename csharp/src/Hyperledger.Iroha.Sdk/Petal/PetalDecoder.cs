using System.Diagnostics.CodeAnalysis;

namespace Hyperledger.Iroha.Petal;

/// <summary>Decoder tuning.</summary>
public sealed record PetalDecodeOptions
{
    private readonly long maxPixels = 12_000_000;

    /// <summary>The reference defaults.</summary>
    public static PetalDecodeOptions Default { get; } = new();

    /// <summary>Also try horizontally mirrored images (front-camera previews). Default <see langword="true"/>.</summary>
    public bool TryMirrored { get; init; } = true;

    /// <summary>
    /// Largest image (in pixels) the decoder accepts; larger frames should be
    /// downscaled by the caller. Bounds memory and work on hostile input.
    /// Default 12,000,000.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is negative.</exception>
    public long MaxPixels
    {
        get => maxPixels;
        init
        {
            ArgumentOutOfRangeException.ThrowIfNegative(value);
            maxPixels = value;
        }
    }
}

/// <summary>Why a camera frame could not be decoded at all.</summary>
public enum PetalDecodeError
{
    /// <summary>The image is smaller than 48 pixels on a side or larger than <see cref="PetalDecodeOptions.MaxPixels"/>.</summary>
    UnsupportedImage,

    /// <summary>No set of corner finders (four, or three forming a corner) was found.</summary>
    NoFinders,

    /// <summary>Finders were found but no orientation produced a readable lane.</summary>
    NoOrientation,
}

/// <summary>A lane that passed its Reed–Solomon check.</summary>
public sealed class PetalLaneResult : IEquatable<PetalLaneResult>
{
    internal PetalLaneResult(byte[] data, int corrected, int erasures)
    {
        Data = data;
        Corrected = corrected;
        Erasures = erasures;
    }

    /// <summary>Lane data bytes (header and atoms, or the beacon).</summary>
    public byte[] Data { get; }

    /// <summary>
    /// Byte positions the Reed–Solomon decoder rewrote: the erased bytes plus any
    /// errors it found among the others (a measure of how close the lane was to
    /// failing).
    /// </summary>
    public int Corrected { get; }

    /// <summary>Bytes that were passed to the Reed–Solomon decoder as erasures.</summary>
    public int Erasures { get; }

    /// <inheritdoc />
    public bool Equals(PetalLaneResult? other) => other is not null
        && Corrected == other.Corrected
        && Erasures == other.Erasures
        && Data.AsSpan().SequenceEqual(other.Data);

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as PetalLaneResult);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.AddBytes(Data);
        hash.Add(Corrected);
        hash.Add(Erasures);
        return hash.ToHashCode();
    }
}

/// <summary>Everything read from one camera frame.</summary>
public sealed class PetalDecodedFrame
{
    private readonly PetalLaneResult?[] fields;

    internal PetalDecodedFrame(
        PetalPose pose,
        int rotation,
        bool mirrored,
        PetalLaneResult? d,
        ReadOnlySpan<PetalLaneResult?> fields,
        int? inferredCorner)
    {
        if (fields.Length != PetalLayout.Sectors)
            throw new ArgumentException("A frame has four field lanes.", nameof(fields));
        Pose = pose;
        Rotation = rotation;
        Mirrored = mirrored;
        D = d;
        this.fields = fields.ToArray();
        Fields = Array.AsReadOnly(this.fields);
        InferredCorner = inferredCorner;
    }

    /// <summary>Canvas-to-pixel pose that was used (homography and lens distortion).</summary>
    public PetalPose Pose { get; }

    /// <summary>Orientation: how many quarter turns the finder order was rotated.</summary>
    public int Rotation { get; }

    /// <summary>Whether the image was mirrored.</summary>
    public bool Mirrored { get; }

    /// <summary>Lane <c>D</c> result.</summary>
    public PetalLaneResult? D { get; }

    /// <summary>Field lane results, sector 0 first (<see langword="null"/> where a lane did not decode).</summary>
    public IReadOnlyList<PetalLaneResult?> Fields { get; }

    /// <summary>
    /// The corner finder that was hidden (by a finger, a glare or the edge of the frame) and
    /// inferred from the other three, as its canonical index: 0 top-left, 1 top-right,
    /// 2 bottom-right, 3 bottom-left of the upright code; <see langword="null"/> when all four
    /// blossoms were seen.
    /// </summary>
    public int? InferredCorner { get; }

    /// <summary>Number of lanes that decoded.</summary>
    public int LanesOk
    {
        get
        {
            var count = D is null ? 0 : 1;
            foreach (var lane in fields)
                count += lane is null ? 0 : 1;
            return count;
        }
    }

    /// <summary>The lanes that decoded, as letters: <c>D</c> and the sector digits <c>0</c>–<c>3</c> (for example <c>"D0123"</c>).</summary>
    public string Lanes
    {
        get
        {
            Span<char> letters = stackalloc char[1 + PetalLayout.Sectors];
            var count = 0;
            foreach (var lane in PetalLanes.DecodeOrder)
            {
                if (Lane(lane) is not null)
                    letters[count++] = PetalLanes.Letter(lane);
            }

            return new string(letters[..count]);
        }
    }

    /// <summary>The result of <paramref name="lane"/>, when it decoded.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>The result, or <see langword="null"/>.</returns>
    public PetalLaneResult? Lane(PetalLane lane) => PetalLanes.IsField(lane) ? fields[PetalLanes.Sector(lane)] : D;

    /// <summary>What lane <c>D</c> carried, when it decoded.</summary>
    /// <returns>The parsed lane, or <see langword="null"/>.</returns>
    public PetalDLane? DLane() => D is null ? null : PetalStream.ParseDLane(D.Data);

    /// <summary>The beacon, when lane <c>D</c> decoded on a beacon frame.</summary>
    /// <returns>The beacon, or <see langword="null"/>.</returns>
    public PetalBeacon? Beacon() => DLane()?.Beacon;

    /// <summary>Atom packets from every lane that decoded (the field lanes in sector order, then <c>D</c>).</summary>
    /// <returns>The packets.</returns>
    public IReadOnlyList<PetalAtomPacket> AtomPackets()
    {
        var packets = new List<PetalAtomPacket>(1 + PetalLayout.Sectors);
        foreach (var lane in PetalLanes.Fields)
        {
            if (Lane(lane) is { } result && PetalStream.ParseAtomLane(lane, result.Data) is { } packet)
                packets.Add(packet);
        }

        if (DLane()?.Atoms is { } d)
            packets.Add(d);
        return packets;
    }

    /// <summary>Offers everything this frame carries to <paramref name="assembler"/> (lane <c>D</c> first).</summary>
    /// <param name="assembler">The stream assembler.</param>
    public void Feed(PetalStreamAssembler assembler)
    {
        ArgumentNullException.ThrowIfNull(assembler);
        if (DLane() is { } d)
            assembler.PushDLane(d);
        foreach (var lane in PetalLanes.Fields)
        {
            if (Lane(lane) is { } result && PetalStream.ParseAtomLane(lane, result.Data) is { } packet)
                assembler.PushAtoms(packet);
        }
    }
}

/// <summary>The outcome of <see cref="PetalDecoder.Decode"/>: a frame or the reason there is none.</summary>
public readonly struct PetalDecodeResult
{
    private PetalDecodeResult(PetalDecodedFrame? frame, PetalDecodeError? error)
    {
        Frame = frame;
        Error = error;
    }

    /// <summary>The decoded frame on success.</summary>
    public PetalDecodedFrame? Frame { get; }

    /// <summary>Why decoding failed, or <see langword="null"/> on success.</summary>
    public PetalDecodeError? Error { get; }

    /// <summary>Whether a frame was decoded (at least one lane passed its code).</summary>
    [MemberNotNullWhen(true, nameof(Frame))]
    [MemberNotNullWhen(false, nameof(Error))]
    public bool Success => Frame is not null;

    internal static PetalDecodeResult Ok(PetalDecodedFrame frame) => new(frame, null);

    internal static PetalDecodeResult Fail(PetalDecodeError error) => new(null, error);
}

/// <summary>
/// What one capture shows of a frame's petal field: the level of every field cell sampled under
/// the capture's decoded pose and normalised by that capture's finder levels (about 27 KB).
/// </summary>
/// <remarks>Captures of one shown frame merge with <see cref="PetalDecoder.MergeFields"/>.</remarks>
public sealed class PetalFieldEvidence
{
    internal PetalFieldEvidence(double[] levels)
    {
        Levels = levels;
    }

    /// <summary>The normalised level of every field cell (owned, never shared).</summary>
    internal double[] Levels { get; }
}

/// <summary>
/// From a camera luma plane to lane data.
/// </summary>
/// <remarks>
/// <para>
/// The decoder locates the corner finders (four, or three with the fourth inferred), derives a
/// pose for each orientation hypothesis, ranks the orientations by how well the ring gates line
/// up (lane <c>D</c> must check out), then reads the dots and the petal field. Every petal is
/// one bit: its level, normalised by the finder levels, is compared with a two-means threshold
/// computed for its codeword and colour class (the petals inside the <c>天</c> are lighter than
/// the pink ones). Bytes the decoder is unsure about become Reed–Solomon erasures.
/// </para>
/// <para>
/// When some field lanes do not decode, the pose is searched for radial lens distortion: a
/// cheap camera's barrel distortion moves the petals by a third of their pitch at the edge of a
/// 480p frame, while the finders that fix the homography sit far outside the field. Sectors
/// still unread are read again from <em>local levels</em>: every cell rescaled by the darkest and
/// brightest levels of its 32-unit neighbourhood, which cancels shadows, glare, banding and
/// veiling light.
/// </para>
/// <para>
/// <see cref="Track"/> reads the next camera frame by following the pose of the previous one,
/// which skips the finder search. A camera sees each shown frame two to four times:
/// <see cref="FieldEvidence"/> keeps what one capture shows of the petal field, and
/// <see cref="MergeFields"/> reads the field lanes from the average of several captures of one
/// shown frame.
/// </para>
/// <para>
/// Untrusted images never raise exceptions: every failure is reported through
/// <see cref="PetalDecodeResult"/>. A lane that decodes is protected by its Reed–Solomon code
/// and, end to end, by the payload CRC-32C. The decoder repeats the reference's IEEE double
/// operations in the same order. Scratch buffers are kept per thread, so concurrent decodes on
/// different threads are safe and a camera loop does not allocate them per frame.
/// </para>
/// </remarks>
public static class PetalDecoder
{
    /// <summary>Smallest accepted image side in pixels.</summary>
    public const int MinimumImageSide = 48;

    /// <summary>Final refinement step of the distortion search.</summary>
    private const double DistortionRefine = 0.015;

    /// <summary>A colour class with fewer cells in a codeword than this takes the codeword's threshold.</summary>
    private const int ClassMinCells = 16;

    /// <summary>Field cells within this distance of a cell are its neighbourhood for the local read.</summary>
    private const double Neighbourhood = 32.0;

    /// <summary>Cells averaged at each end of a sorted neighbourhood for its local dark and lit levels.</summary>
    private const int NeighbourhoodEnds = 5;

    /// <summary>Sample spread of a ring dot, in canvas units (<c>0.32 × 9</c>).</summary>
    private const double DotSpread = 0.32 * PetalLayout.DotRadius;

    /// <summary>Sample spread of a petal, in canvas units: <c>0.4 ×</c> the single-precision petal radius widened.</summary>
    private const double PetalSpread = 0.4 * PetalLayout.PetalRadius;

    private const int AllSectors = (1 << PetalLayout.Sectors) - 1;

    /// <summary>
    /// Distortion steps tried, relative to the starting pose, when a plain pose leaves field
    /// lanes unread; the best is then refined by ±<see cref="DistortionRefine"/>.
    /// </summary>
    private static readonly double[] DistortionSteps = [-0.03, -0.06, -0.09, -0.12, -0.16, 0.03];

    private static readonly double[] ReferenceCos = new double[8];
    private static readonly double[] ReferenceSin = new double[8];
    private static readonly bool[] Mirrorings = [false, true];

    /// <summary>The bit cells of every sector (840 each, bit order) and whether each is light.</summary>
    private static readonly int[] SectorBitCells = new int[PetalLayout.Sectors * PetalLanes.FieldBits];
    private static readonly bool[] SectorBitLights = new bool[PetalLayout.Sectors * PetalLanes.FieldBits];
    private static readonly int[] SectorLightCounts = new int[PetalLayout.Sectors];

    /// <summary>Canvas position of every cell as a fraction of the canvas, for the bilinear reference levels.</summary>
    private static readonly double[] CellU = new double[PetalLayout.FieldCells];
    private static readonly double[] CellV = new double[PetalLayout.FieldCells];

    /// <summary>The neighbourhood of every field cell, cell <c>c</c> at <c>[NeighbourStarts[c], NeighbourStarts[c + 1])</c>.</summary>
    private static readonly int[] NeighbourStarts = new int[PetalLayout.FieldCells + 1];
    private static readonly ushort[] NeighbourCells;
    private static readonly int LargestNeighbourhood;

    static PetalDecoder()
    {
        for (var k = 0; k < 8; k++)
        {
            var angle = Math.Tau * k / 8.0;
            ReferenceCos[k] = Math.Cos(angle);
            ReferenceSin[k] = Math.Sin(angle);
        }

        var order = PetalLayout.SectorOrderTable;
        var lights = PetalLayout.CellLightTable;
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            for (var bit = 0; bit < PetalLanes.FieldBits; bit++)
            {
                var cell = order[sector * PetalLayout.SectorCells + bit];
                SectorBitCells[sector * PetalLanes.FieldBits + bit] = cell;
                SectorBitLights[sector * PetalLanes.FieldBits + bit] = lights[cell];
                SectorLightCounts[sector] += lights[cell] ? 1 : 0;
            }
        }

        var xs = PetalLayout.CellXTable;
        var ys = PetalLayout.CellYTable;
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            CellU[cell] = Math.Clamp(xs[cell] / 1024.0, 0.0, 1.0);
            CellV[cell] = Math.Clamp(ys[cell] / 1024.0, 0.0, 1.0);
        }

        // Bands are 11 units apart, so only the three bands on either side can hold neighbours.
        var bands = PetalLayout.CellBandTable;
        var offsets = PetalLayout.BandOffsetTable;
        var neighbours = new List<ushort>(PetalLayout.FieldCells * 24);
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            NeighbourStarts[cell] = neighbours.Count;
            var (x, y) = (xs[cell], ys[cell]);
            var band = bands[cell];
            var first = offsets[Math.Max(band - 3, 0)];
            var last = offsets[Math.Min(band + 4, PetalLayout.BandCount)];
            for (var other = first; other < last; other++)
            {
                var (dx, dy) = (xs[other] - x, ys[other] - y);
                if (dx * dx + dy * dy <= Neighbourhood * Neighbourhood)
                    neighbours.Add((ushort)other);
            }

            LargestNeighbourhood = Math.Max(LargestNeighbourhood, neighbours.Count - NeighbourStarts[cell]);
        }

        NeighbourStarts[PetalLayout.FieldCells] = neighbours.Count;
        NeighbourCells = neighbours.ToArray();
    }

    /// <summary>Decodes one camera frame.</summary>
    /// <remarks>
    /// Tries the finder candidates of <see cref="PetalLocator.Candidates"/> in order and returns
    /// the first that reads. For each, the orientation hypotheses (four quarter turns,
    /// optionally mirrored) are ranked by the ring gates; lane <c>D</c> is tried under the best
    /// three whose gate score is at least 0.2, then the field lanes under the best four.
    /// </remarks>
    /// <param name="image">The luma plane.</param>
    /// <param name="options">Decoder options; <see cref="PetalDecodeOptions.Default"/> when omitted.</param>
    /// <returns>
    /// The frame, or <see cref="PetalDecodeError.UnsupportedImage"/>,
    /// <see cref="PetalDecodeError.NoFinders"/> when no code is visible, or
    /// <see cref="PetalDecodeError.NoOrientation"/> when no orientation yields a readable lane.
    /// </returns>
    public static PetalDecodeResult Decode(PetalLuma image, PetalDecodeOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(image);
        options ??= PetalDecodeOptions.Default;
        if (!Supported(image, options))
            return PetalDecodeResult.Fail(PetalDecodeError.UnsupportedImage);
        var located = false;
        foreach (var set in PetalLocator.Candidates(image))
        {
            located = true;
            if (DecodeCandidate(image, options, set) is { } frame)
                return PetalDecodeResult.Ok(frame);
        }

        return PetalDecodeResult.Fail(located ? PetalDecodeError.NoOrientation : PetalDecodeError.NoFinders);
    }

    /// <summary>
    /// Follows a code from the previous frame that decoded, without searching the whole image
    /// for finders (the most expensive part of <see cref="Decode"/>).
    /// </summary>
    /// <remarks>
    /// Each corner finder seen in the previous frame is re-found near where the previous pose
    /// puts it (see <see cref="PetalLocator.Follow"/>); the mean movement of those predicts the
    /// rest. A corner inferred in the previous frame counts as seen again only when its blossom
    /// is re-found within a quarter diameter of that prediction (so a thumb beside it does not
    /// count). When exactly one corner is missing it is placed at the prediction and refined
    /// against the rings like an inferred corner. The orientation and the lens distortion are
    /// kept from the previous frame (the distortion is searched again when a field lane does
    /// not decode).
    /// </remarks>
    /// <param name="image">The luma plane of the next camera frame.</param>
    /// <param name="previous">The last frame that decoded.</param>
    /// <param name="options">Decoder options; <see cref="PetalDecodeOptions.Default"/> when omitted.</param>
    /// <returns>
    /// The frame, or <see langword="null"/> when the image is unusable, the previous pose is
    /// broken, two corners are lost or no lane decodes; the caller then runs <see cref="Decode"/>.
    /// </returns>
    public static PetalDecodedFrame? Track(PetalLuma image, PetalDecodedFrame previous, PetalDecodeOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(previous);
        options ??= PetalDecodeOptions.Default;
        // a frame built by the caller may name a corner that does not exist
        if (!Supported(image, options) || previous.InferredCorner is { } named && (uint)named > 3)
            return null;
        var p0 = previous.Pose;
        Span<PetalFinder> expected = stackalloc PetalFinder[4];
        double shortSide = Math.Min(image.Width, image.Height);
        for (var i = 0; i < 4; i++)
        {
            var center = PetalLayout.FinderCenterTable[i];
            var (cx, cy) = (center.X, center.Y);
            p0.Apply(cx, cy, out var x, out var y);
            var size = PetalMath.Max(
                ProjectedLength(p0, cx - 60.0, cy, cx + 60.0, cy),
                ProjectedLength(p0, cx, cy - 60.0, cx, cy + 60.0));
            expected[i] = new PetalFinder(x, y, size);
        }

        foreach (var f in expected)
        {
            if (!(double.IsFinite(f.X) && double.IsFinite(f.Y) && double.IsFinite(f.Size)) || f.Size > shortSide)
                return null;
        }

        var previouslyInferred = previous.InferredCorner ?? -1;
        Span<PetalFinder> found = stackalloc PetalFinder[4];
        Span<bool> seen = stackalloc bool[4];
        for (var i = 0; i < 4; i++)
        {
            if (i != previouslyInferred && PetalLocator.Follow(image, expected[i]) is { } f)
            {
                found[i] = f;
                seen[i] = true;
            }
        }

        // the mean movement of the corners that were followed predicts the others
        var (sumX, sumY, followed) = (PetalMath.SumIdentity, PetalMath.SumIdentity, 0);
        for (var i = 0; i < 4; i++)
        {
            if (!seen[i])
                continue;
            sumX += found[i].X - expected[i].X;
            sumY += found[i].Y - expected[i].Y;
            followed++;
        }

        if (followed < 3)
            return null;
        var (shiftX, shiftY) = (sumX / followed, sumY / followed);
        // a corner that was hidden is seen again only when its blossom is found right where
        // the others say it is (a bright thumb beside it must not count)
        if (previouslyInferred >= 0)
        {
            var at = Predicted(expected[previouslyInferred], shiftX, shiftY);
            if (PetalLocator.Follow(image, at) is { } f
                && Math.Sqrt((f.X - at.X) * (f.X - at.X) + (f.Y - at.Y) * (f.Y - at.Y)) <= 0.25 * at.Size)
            {
                found[previouslyInferred] = f;
                seen[previouslyInferred] = true;
            }
        }

        int? inferred = null;
        for (var i = 0; i < 4; i++)
        {
            if (seen[i])
                continue;
            if (inferred is not null)
                return null;
            inferred = i;
        }

        var corners = found.ToArray();
        if (inferred is { } m)
        {
            corners[m] = Predicted(expected[m], shiftX, shiftY);
            corners = RefineInferredCorner(image, corners, m);
        }

        Span<PetalPoint> points = stackalloc PetalPoint[4];
        for (var i = 0; i < 4; i++)
            points[i] = new PetalPoint(corners[i].X, corners[i].Y);
        var workspace = Workspace.Current;
        if (ReadFieldSearching(image, points, previous.Pose.Distortion, inferred, workspace) is not { } read)
            return null;
        var d = ReadLaneD(image, read.Pose, read.Reference);
        if (d is null && read.Decoded == 0)
            return null;
        return new PetalDecodedFrame(read.Pose, previous.Rotation, previous.Mirrored, d, read.Fields, inferred);
    }

    /// <summary>Reads all lanes with a known canvas-to-pixel pose (no finder search, no distortion search).</summary>
    /// <remarks>
    /// Used by refinement passes and by qualification tooling with a ground-truth pose. The
    /// reference levels come from all four corner finders, so a frame whose corner blossom is
    /// hidden is refused here (the hidden corner has no contrast); <see cref="Decode"/> and
    /// <see cref="Track"/> read such frames by inferring that corner. Field lanes the levels
    /// leave unread are read again from their local levels.
    /// </remarks>
    /// <param name="image">The luma plane.</param>
    /// <param name="pose">Canvas-to-pixel pose.</param>
    /// <param name="options">Decoder options; <see cref="PetalDecodeOptions.Default"/> when omitted.</param>
    /// <returns>
    /// The frame, or <see langword="null"/> when the image is unusable (see
    /// <see cref="PetalDecodeError.UnsupportedImage"/>) or the finder reference levels are too weak.
    /// </returns>
    public static PetalDecodedFrame? DecodeAt(PetalLuma image, PetalPose pose, PetalDecodeOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(image);
        options ??= PetalDecodeOptions.Default;
        if (!Supported(image, options))
            return null;
        if (ReferenceLevels(image, pose, null) is not { } reference)
            return null;
        var workspace = Workspace.Current;
        var levels = workspace.Levels;
        FieldLevels(image, pose, reference, levels);
        Span<PetalLaneResult?> fields = new PetalLaneResult?[PetalLayout.Sectors];
        ReadFields(levels, 0, fields, workspace);
        return new PetalDecodedFrame(pose, 0, false, ReadLaneD(image, pose, reference), fields, null);
    }

    /// <summary>
    /// Builds the cells a decoder believes it saw under <paramref name="frame"/>'s pose, for
    /// diagnostics: every codeword thresholded as the first read of <see cref="Decode"/>
    /// thresholds it (no local read). The petals are left unturned (see
    /// <see cref="PetalTurns.ObservedTurns"/>).
    /// </summary>
    /// <remarks>The levels of a corner the frame inferred are extrapolated as in the decoder.</remarks>
    /// <param name="image">The luma plane the frame was decoded from.</param>
    /// <param name="frame">The decoded frame.</param>
    /// <param name="options">Decoder options; <see cref="PetalDecodeOptions.Default"/> when omitted.</param>
    /// <returns>
    /// The observed cells, or <see langword="null"/> when the image is unusable, the frame names
    /// a corner that does not exist, or the reference levels are too weak.
    /// </returns>
    public static PetalFrameCells? ObservedCells(PetalLuma image, PetalDecodedFrame frame, PetalDecodeOptions? options = null)
    {
        if (FieldEvidence(image, frame, options) is not { } evidence)
            return null;
        if (ReferenceLevels(image, frame.Pose, frame.InferredCorner) is not { } reference)
            return null;
        var (d, _) = ReadDots(image, frame.Pose, reference);
        var workspace = Workspace.Current;
        var words = new byte[PetalLayout.Sectors][];
        var confidence = new double[PetalLanes.FieldWordLength];
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            var margins = 0.0;
            words[sector] = new byte[PetalLanes.FieldWordLength];
            ReadWord(evidence.Levels, sector, ref margins, words[sector], confidence, workspace);
        }

        return PetalFrameCells.FromWords(d, words);
    }

    /// <summary>The field evidence of <paramref name="image"/> under the pose <paramref name="frame"/> was decoded with.</summary>
    /// <param name="image">The luma plane the frame was decoded from.</param>
    /// <param name="frame">The decoded frame.</param>
    /// <param name="options">Decoder options; <see cref="PetalDecodeOptions.Default"/> when omitted.</param>
    /// <returns>
    /// The evidence, or <see langword="null"/> when the image is unusable (see
    /// <see cref="PetalDecodeError.UnsupportedImage"/>), the frame names a corner that does not
    /// exist, or the finder levels under that pose are too weak.
    /// </returns>
    public static PetalFieldEvidence? FieldEvidence(PetalLuma image, PetalDecodedFrame frame, PetalDecodeOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(frame);
        options ??= PetalDecodeOptions.Default;
        // a frame built by the caller may name a corner that does not exist
        if (!Supported(image, options) || frame.InferredCorner is { } corner && (uint)corner > 3)
            return null;
        if (ReferenceLevels(image, frame.Pose, frame.InferredCorner) is not { } reference)
            return null;
        var levels = new double[PetalLayout.FieldCells];
        FieldLevels(image, frame.Pose, reference, levels);
        return new PetalFieldEvidence(levels);
    }

    /// <summary>
    /// The field lanes from several captures of one shown frame: the cell-by-cell mean of their
    /// levels (summed in capture order, then divided by the number of captures), read as one
    /// capture's levels are, local read included.
    /// </summary>
    /// <remarks>
    /// Averaging cancels the sensor noise, banding and shake that differ between captures; one
    /// evidence reads exactly the field lanes <see cref="Decode"/> read from that capture's pose.
    /// The caller checks that every evidence comes from captures of the same shown frame: a lane
    /// read here names its frame in its own header.
    /// </remarks>
    /// <param name="evidence">The captures' field evidence.</param>
    /// <param name="skip">Sectors not to read (four flags, sector 0 first); empty reads every sector.</param>
    /// <returns>The field lane results, sector 0 first.</returns>
    /// <exception cref="ArgumentException"><paramref name="skip"/> is neither empty nor four flags long.</exception>
    public static PetalLaneResult?[] MergeFields(IReadOnlyList<PetalFieldEvidence> evidence, ReadOnlySpan<bool> skip = default)
    {
        ArgumentNullException.ThrowIfNull(evidence);
        if (!skip.IsEmpty && skip.Length != PetalLayout.Sectors)
            throw new ArgumentException("Skip flags name the four sectors.", nameof(skip));
        var mask = 0;
        for (var sector = 0; sector < skip.Length; sector++)
            mask |= skip[sector] ? 1 << sector : 0;
        var results = new PetalLaneResult?[PetalLayout.Sectors];
        if (evidence.Count == 0)
            return results;
        var workspace = Workspace.Current;
        var mean = workspace.Levels;
        Array.Clear(mean);
        foreach (var capture in evidence)
        {
            ArgumentNullException.ThrowIfNull(capture, nameof(evidence));
            var levels = capture.Levels;
            for (var cell = 0; cell < mean.Length; cell++)
                mean[cell] += levels[cell];
        }

        double count = evidence.Count;
        for (var cell = 0; cell < mean.Length; cell++)
            mean[cell] /= count;
        ReadFields(mean, mask, results, workspace);
        return results;
    }

    /// <summary>
    /// The field lanes read from normalised cell levels: the level read alone, or with
    /// <paramref name="local"/> also the local read of the sectors it leaves unread (diagnostics).
    /// </summary>
    internal static PetalLaneResult?[] ReadFieldLanes(double[] levels, bool local)
    {
        var workspace = Workspace.Current;
        var results = new PetalLaneResult?[PetalLayout.Sectors];
        if (local)
            ReadFields(levels, 0, results, workspace);
        else
            ReadSectors(levels, 0, results, workspace);
        return results;
    }

    /// <summary>
    /// Whether <paramref name="image"/> is something the decoder may work on: at least 48 pixels
    /// on a side and within <see cref="PetalDecodeOptions.MaxPixels"/>.
    /// </summary>
    private static bool Supported(PetalLuma image, PetalDecodeOptions options) =>
        image.Width >= MinimumImageSide
        && image.Height >= MinimumImageSide
        && (long)image.Width * image.Height <= options.MaxPixels
        // the locator's integral image must fit in one array, whatever MaxPixels says
        && (long)(image.Width + 1) * (image.Height + 1) <= Array.MaxLength;

    /// <summary>
    /// One finder candidate: refines an inferred corner against the rings, ranks the orientation
    /// hypotheses by gate score, tries lane <c>D</c> under the best three whose gate score is at
    /// least 0.2 and the field lanes under the best four.
    /// </summary>
    private static PetalDecodedFrame? DecodeCandidate(PetalLuma image, PetalDecodeOptions options, PetalFinderSet set)
    {
        var corners = set.Inferred is { } index
            ? RefineInferredCorner(image, set.CornerArray, index)
            : set.CornerArray;
        var scored = new List<Scored>(8);
        foreach (var (rotation, mirrored, destination) in Hypotheses(corners, options.TryMirrored))
        {
            int? inferred = set.Inferred is { } corner ? CanonicalCorner(corner, rotation, mirrored) : null;
            if (FitPose(image, destination, 0.0) is not { } pose)
                continue;
            if (ReferenceLevels(image, pose, inferred) is not { } reference)
                continue;
            var gate = GateScore(image, pose, reference);
            var candidate = new Scored(gate, rotation, mirrored, destination, pose, reference, inferred);
            // Stable descending insertion by gate score (Rust `sort_by` with `b.total_cmp(a)`).
            var at = scored.Count;
            while (at > 0 && PetalMath.TotalCompare(gate, scored[at - 1].Gate) > 0)
                at--;
            scored.Insert(at, candidate);
        }

        var workspace = Workspace.Current;
        // 1. the ring beacon is the cheapest and strongest orientation check
        for (var i = 0; i < Math.Min(3, scored.Count); i++)
        {
            var s = scored[i];
            if (s.Gate < 0.2)
                continue;
            if (ReadLaneD(image, s.Pose, s.Reference) is { } d)
            {
                var read = ReadFieldSearching(image, s.Destination, 0.0, s.InferredCorner, workspace);
                return read is null
                    ? new PetalDecodedFrame(s.Pose, s.Rotation, s.Mirrored, d, new PetalLaneResult?[PetalLayout.Sectors], s.InferredCorner)
                    : new PetalDecodedFrame(read.Pose, s.Rotation, s.Mirrored, d, read.Fields, s.InferredCorner);
            }
        }

        // 2. fall back to the field lanes under the most promising orientations: a wrong
        //    orientation reads permuted petals, which no field codeword accepts. Strong lens
        //    distortion can hide both lane D and the field from a plain pose, so the distortion
        //    is searched under the best orientation whose gates line up even when its plain pose
        //    reads nothing.
        for (var i = 0; i < Math.Min(4, scored.Count); i++)
        {
            var s = scored[i];
            var plain = workspace.First;
            if (!ReadFieldAt(image, s.Destination, 0.0, s.InferredCorner, plain, workspace))
                continue;
            var confident = i == 0 && s.Gate >= 0.2;
            FieldRead read;
            if (plain.Decoded == PetalLayout.Sectors)
            {
                read = plain;
            }
            else if (plain.Decoded == 0 && !confident)
            {
                // the local read costs a sort per cell: only the best orientation gets it for free
                read = i == 0 ? Finish(plain, workspace) : plain;
            }
            else
            {
                // the reference re-reads the plain pose to start its search; the read is the same
                read = SearchDistortion(image, s.Destination, 0.0, s.InferredCorner, plain, workspace.Second, workspace);
            }

            if (read.Decoded == 0)
                continue;
            return new PetalDecodedFrame(read.Pose, s.Rotation, s.Mirrored, ReadLaneD(image, read.Pose, read.Reference), read.Fields, s.InferredCorner);
        }

        return null;
    }

    /// <summary>Canonical index of the corner at <paramref name="index"/> of a finder quad under one orientation hypothesis.</summary>
    internal static int CanonicalCorner(int index, int rotation, bool mirrored) =>
        mirrored ? (rotation + 4 - index) % 4 : (index + 4 - rotation) % 4;

    /// <summary>The orientation hypotheses of a finder quad: four quarter turns, then (optionally) four mirrored ones, with the finder centres in canonical corner order.</summary>
    internal static List<(int Rotation, bool Mirrored, PetalPoint[] Destination)> Hypotheses(PetalFinder[] finders, bool tryMirrored)
    {
        var output = new List<(int, bool, PetalPoint[])>(8);
        foreach (var mirrored in Mirrorings)
        {
            if (mirrored && !tryMirrored)
                continue;
            for (var rotation = 0; rotation < 4; rotation++)
            {
                var destination = new PetalPoint[4];
                for (var i = 0; i < 4; i++)
                {
                    var q = mirrored ? finders[(rotation + 4 - i) % 4] : finders[(i + rotation) % 4];
                    destination[i] = new PetalPoint(q.X, q.Y);
                }

                output.Add((rotation, mirrored, destination));
            }
        }

        return output;
    }

    /// <summary>The pose with lens distortion <paramref name="distortion"/> through the four finder centres (image pixels, canonical corner order).</summary>
    internal static PetalPose? FitPose(PetalLuma image, ReadOnlySpan<PetalPoint> destination, double distortion) =>
        PetalPose.Fit(PetalLayout.FinderCenterTable, destination, distortion, image.Width, image.Height);

    /// <summary>The length of the image of the canvas segment from <c>(x0, y0)</c> to <c>(x1, y1)</c>.</summary>
    private static double ProjectedLength(in PetalPose pose, double x0, double y0, double x1, double y1)
    {
        pose.Apply(x0, y0, out var ax, out var ay);
        pose.Apply(x1, y1, out var bx, out var by);
        return Math.Sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by));
    }

    private static PetalFinder Predicted(PetalFinder expected, double shiftX, double shiftY) =>
        new(expected.X + shiftX, expected.Y + shiftY, expected.Size);

    /// <summary>
    /// The brightness summed over all ring slots under the plain pose that maps the canonical
    /// corners onto <paramref name="corners"/> (in quad order).
    /// </summary>
    /// <remarks>
    /// The slots form the same set of points under every quarter turn and mirror of the canvas
    /// (88, 100 and 108 are multiples of four), so the value does not depend on the orientation.
    /// </remarks>
    internal static double? RingBrightness(PetalLuma image, ReadOnlySpan<PetalPoint> corners)
    {
        if (FitPose(image, corners, 0.0) is not { } pose)
            return null;
        var sum = PetalMath.SumIdentity;
        for (var flat = 0; flat < PetalLayout.TotalSlots; flat++)
        {
            var center = PetalLayout.SlotCenterFlat(flat);
            sum += DotSamples(image, pose, center.X, center.Y, DotSpread);
        }

        return sum;
    }

    /// <summary>
    /// Moves an inferred corner to where the three dotted rings line up best: a 13 × 13 search
    /// in steps of 2 % of the mean leg around the parallelogram estimate, then a 9 × 9 search
    /// in steps of 0.5 % around the best point.
    /// </summary>
    /// <remarks>
    /// The rings fix the geometry only; the orientation is decided afterwards by the gates and
    /// the lanes. A candidate replaces the best point only when it is strictly brighter.
    /// </remarks>
    internal static PetalFinder[] RefineInferredCorner(PetalLuma image, PetalFinder[] corners, int inferred)
    {
        Span<PetalPoint> points = stackalloc PetalPoint[4];
        for (var i = 0; i < 4; i++)
            points[i] = new PetalPoint(corners[i].X, corners[i].Y);
        var start = points[inferred];
        var leg = 0.5 * (Distance(points[(inferred + 1) % 4], start) + Distance(points[(inferred + 3) % 4], start));
        var bestBrightness = double.MinValue;
        var best = start;
        SearchRings(image, points, inferred, start, 0.02 * leg, 6, ref bestBrightness, ref best);
        SearchRings(image, points, inferred, best, 0.005 * leg, 4, ref bestBrightness, ref best);
        var refined = (PetalFinder[])corners.Clone();
        refined[inferred] = new PetalFinder(best.X, best.Y, corners[inferred].Size);
        return refined;
    }

    private static double Distance(PetalPoint point, PetalPoint start) =>
        Math.Sqrt((point.X - start.X) * (point.X - start.X) + (point.Y - start.Y) * (point.Y - start.Y));

    /// <summary>One grid of <see cref="RefineInferredCorner"/>: <c>dy</c> outer, <c>dx</c> inner.</summary>
    private static void SearchRings(
        PetalLuma image,
        Span<PetalPoint> points,
        int inferred,
        PetalPoint centre,
        double step,
        int reach,
        ref double bestBrightness,
        ref PetalPoint best)
    {
        for (var dy = -reach; dy <= reach; dy++)
        {
            for (var dx = -reach; dx <= reach; dx++)
            {
                var candidate = new PetalPoint(centre.X + dx * step, centre.Y + dy * step);
                points[inferred] = candidate;
                if (RingBrightness(image, points) is { } brightness && brightness > bestBrightness)
                {
                    bestBrightness = brightness;
                    best = candidate;
                }
            }
        }
    }

    /// <summary>The mean of five samples: the centre and four at ±<paramref name="spread"/> on x, then y.</summary>
    private static double DotSamples(PetalLuma image, in PetalPose pose, double x, double y, double spread)
    {
        var sum = 0.0;
        pose.Apply(x + 0.0, y + 0.0, out var px, out var py);
        sum += image.Sample(px, py);
        pose.Apply(x + spread, y + 0.0, out px, out py);
        sum += image.Sample(px, py);
        pose.Apply(x + -spread, y + 0.0, out px, out py);
        sum += image.Sample(px, py);
        pose.Apply(x + 0.0, y + spread, out px, out py);
        sum += image.Sample(px, py);
        pose.Apply(x + 0.0, y + -spread, out px, out py);
        sum += image.Sample(px, py);
        return sum / 5.0;
    }

    /// <summary>
    /// Light and dark levels at the four corners: the solid blossom core, and the black canvas
    /// 100 units inward of it.
    /// </summary>
    /// <remarks>
    /// An <paramref name="inferred"/> corner (canonical index) was not seen, so its levels are
    /// extrapolated from the other three by the parallelogram rule and kept within their range.
    /// </remarks>
    /// <returns>
    /// The levels, or <see langword="null"/> when a corner has less than 12 levels of contrast
    /// or a non-finite one (only a broken pose produces that).
    /// </returns>
    internal static Reference? ReferenceLevels(PetalLuma image, in PetalPose pose, int? inferred)
    {
        var missing = inferred is { } m && (uint)m < 4 ? m : -1;
        var reference = new Reference();
        for (var i = 0; i < 4; i++)
        {
            if (i == missing)
                continue;
            var center = PetalLayout.FinderCenterTable[i];
            var (cx, cy) = (center.X, center.Y);
            // the blossom is solid out to radius 24 around its centre
            pose.Apply(cx, cy, out var px, out var py);
            var sum = image.Sample(px, py);
            for (var k = 0; k < 8; k++)
            {
                pose.Apply(cx + 20.0 * ReferenceCos[k], cy + 20.0 * ReferenceSin[k], out px, out py);
                sum += image.Sample(px, py);
            }

            var lit = sum / 9.0;
            var (sx, sy) = (cx < 512.0 ? 1.0 : -1.0, cy < 512.0 ? 1.0 : -1.0);
            var a = DotSamples(image, pose, cx + sx * 100.0, cy, 5.0);
            var b = DotSamples(image, pose, cx, cy + sy * 100.0, 5.0);
            var dark = 0.5 * (a + b);
            // also refuses NaN levels, which only a non-finite pose can produce
            if (!double.IsFinite(lit - dark) || lit - dark < 12.0)
                return null;
            reference.Set(i, lit, dark);
        }

        if (missing >= 0)
        {
            reference.Extrapolate(missing);
            // uneven light can push the estimates past each other; an inferred corner
            // needs the same contrast as a seen one
            if (reference.Lit(missing) - reference.Dark(missing) < 12.0)
                return null;
        }

        return reference;
    }

    private static double NormalisedDot(PetalLuma image, in PetalPose pose, in Reference reference, int flat)
    {
        var center = PetalLayout.SlotCenterFlat(flat);
        var (x, y) = (center.X, center.Y);
        reference.At(x, y, out var lit, out var dark);
        return (DotSamples(image, pose, x, y, DotSpread) - dark) / (lit - dark);
    }

    /// <summary>The mean normalised level of the gate dots minus that of the guards.</summary>
    internal static double GateScore(PetalLuma image, in PetalPose pose, in Reference reference)
    {
        var gates = PetalLayout.GateSlotTable;
        var guards = PetalLayout.GuardSlotTable;
        var gateSum = PetalMath.SumIdentity;
        foreach (var slot in gates)
            gateSum += NormalisedDot(image, pose, reference, slot);
        var guardSum = PetalMath.SumIdentity;
        foreach (var slot in guards)
            guardSum += NormalisedDot(image, pose, reference, slot);
        return gateSum / gates.Length - guardSum / guards.Length;
    }

    /// <summary>Reads lane <c>D</c>: the transmitted bytes and per-byte confidence.</summary>
    private static (byte[] Word, double[] Confidence) ReadDots(PetalLuma image, in PetalPose pose, in Reference reference)
    {
        var gates = PetalLayout.GateSlotTable;
        var guards = PetalLayout.GuardSlotTable;
        // per-ring thresholds from the gates (lit) and guards (dark)
        Span<double> thresholds = [0.5, 0.5, 0.5];
        for (var ring = 0; ring < PetalLayout.RingCount; ring++)
        {
            var litSum = PetalMath.SumIdentity;
            var litCount = 0;
            foreach (var slot in gates)
            {
                if (PetalLayout.SplitSlot(slot).Ring != ring)
                    continue;
                litSum += NormalisedDot(image, pose, reference, slot);
                litCount++;
            }

            var darkSum = PetalMath.SumIdentity;
            var darkCount = 0;
            foreach (var slot in guards)
            {
                if (PetalLayout.SplitSlot(slot).Ring != ring)
                    continue;
                darkSum += NormalisedDot(image, pose, reference, slot);
                darkCount++;
            }

            if (litCount > 0 && darkCount > 0)
            {
                var (l, d) = (litSum / litCount, darkSum / darkCount);
                if (l - d > 0.2)
                    thresholds[ring] = 0.5 * (l + d);
            }
        }

        var bytes = new byte[PetalLanes.DWordLength];
        var confidence = new double[PetalLanes.DWordLength];
        Array.Fill(confidence, double.MaxValue);
        var dataSlots = PetalLayout.DataSlotTable;
        for (var bit = 0; bit < dataSlots.Length; bit++)
        {
            var slot = dataSlots[bit];
            var value = NormalisedDot(image, pose, reference, slot);
            var threshold = thresholds[PetalLayout.SplitSlot(slot).Ring];
            if (value > threshold)
                bytes[bit / 8] |= (byte)(1 << (7 - bit % 8));
            confidence[bit / 8] = PetalMath.Min(confidence[bit / 8], Math.Abs(value - threshold));
        }

        return (bytes, confidence);
    }

    /// <summary>Lane <c>D</c> under one pose.</summary>
    private static PetalLaneResult? ReadLaneD(PetalLuma image, in PetalPose pose, in Reference reference)
    {
        var (word, confidence) = ReadDots(image, pose, reference);
        return DecodeWithErasures(PetalLane.D, word, confidence);
    }

    /// <summary>Tries Reed–Solomon with growing numbers of erasures, least confident first.</summary>
    /// <remarks>
    /// The schedule erases 0, ⅛, ¼, ⅓ and ½ of the parity bytes (integer division: lane
    /// <c>D</c> 0, 1, 2, 3, 5; a field lane 0, 2, 5, 7, 11). Lane <c>D</c> has only 11 parity
    /// bytes, and a further step leaves so few spare ones that it accepts wrong codewords (at 7
    /// erasures about 0.4 % of random words). Bytes of equal confidence keep their order (the
    /// sort is stable, like Rust's <c>sort_by</c>), which decides which of them are erased.
    /// </remarks>
    internal static PetalLaneResult? DecodeWithErasures(PetalLane lane, byte[] word, double[] confidence)
    {
        var nsym = PetalLanes.ParityLength(lane);
        Span<int> order = stackalloc int[word.Length];
        for (var i = 0; i < order.Length; i++)
            order[i] = i;
        PetalMath.StableSortByKey(order, confidence);
        Span<int> schedule = [0, nsym / 8, nsym / 4, nsym / 3, nsym / 2];
        Span<byte> trial = stackalloc byte[word.Length];
        var previous = -1;
        foreach (var erasures in schedule)
        {
            // Rust `dedup` drops consecutive repeats.
            if (erasures == previous)
                continue;
            previous = erasures;
            var positions = order[..erasures];
            word.CopyTo(trial);
            // zero the erased bytes so stale values cannot leak through
            foreach (var position in positions)
                trial[position] = 0;
            if (PetalLanes.TryDecodeLane(lane, trial, positions, out var data, out var corrected))
                return new PetalLaneResult(data, corrected, positions.Length);
        }

        return null;
    }

    /// <summary>
    /// The two-means threshold of a set of values: start at their mean, then move to the
    /// midpoint of the means of the values above and at or below the threshold, at most eight
    /// times, stopping when it moves less than 10⁻⁴ or one side is empty. Sums run in the order
    /// given.
    /// </summary>
    internal static double TwoMeans(ReadOnlySpan<double> values)
    {
        var total = PetalMath.SumIdentity;
        foreach (var value in values)
            total += value;
        var threshold = total / values.Length;
        for (var round = 0; round < 8; round++)
        {
            var (low, lows, high, highs) = (0.0, 0, 0.0, 0);
            foreach (var value in values)
            {
                if (value > threshold)
                {
                    high += value;
                    highs++;
                }
                else
                {
                    low += value;
                    lows++;
                }
            }

            if (lows == 0 || highs == 0)
                break;
            var next = 0.5 * (low / lows + high / highs);
            var moved = Math.Abs(next - threshold);
            threshold = next;
            if (moved < 1e-4)
                break;
        }

        return threshold;
    }

    /// <summary>The level of every field cell under one pose, normalised by the finder levels.</summary>
    internal static void FieldLevels(PetalLuma image, in PetalPose pose, in Reference reference, Span<double> levels)
    {
        var xs = PetalLayout.CellXTable;
        var ys = PetalLayout.CellYTable;
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            reference.AtFraction(CellU[cell], CellV[cell], out var lit, out var dark);
            levels[cell] = (DotSamples(image, pose, xs[cell], ys[cell], PetalSpread) - dark) / (lit - dark);
        }
    }

    /// <summary>
    /// Cell levels rescaled by their neighbourhood: the mean of the lowest and of the highest
    /// <see cref="NeighbourhoodEnds"/> levels around a cell become 0 and 1 (a span below 0.05
    /// counts as 0.05).
    /// </summary>
    /// <remarks>
    /// About half the petals around any cell are lit, so shadows, glare, banding and veiling
    /// light, which the finder levels cannot describe, cancel.
    /// </remarks>
    internal static void LocalLevels(ReadOnlySpan<double> levels, Span<double> local)
    {
        Span<double> values = stackalloc double[LargestNeighbourhood];
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            var start = NeighbourStarts[cell];
            var count = NeighbourStarts[cell + 1] - start;
            var around = values[..count];
            for (var i = 0; i < count; i++)
                around[i] = levels[NeighbourCells[start + i]];
            PetalMath.SortTotal(around);
            var ends = Math.Max(Math.Min(NeighbourhoodEnds, count / 2), 1);
            var low = PetalMath.SumIdentity;
            for (var i = 0; i < ends; i++)
                low += around[i];
            low /= ends;
            var high = PetalMath.SumIdentity;
            for (var i = count - ends; i < count; i++)
                high += around[i];
            high /= ends;
            local[cell] = (levels[cell] - low) / PetalMath.Max(high - low, 0.05);
        }
    }

    /// <summary>
    /// The field lanes read from normalised cell levels, skipping the sectors in
    /// <paramref name="skip"/>; sectors the levels leave unread are read again from their
    /// <see cref="LocalLevels"/>. Returns how far the cells of the first read sit from their
    /// thresholds on average.
    /// </summary>
    private static double ReadFields(double[] levels, int skip, Span<PetalLaneResult?> results, Workspace workspace)
    {
        var separation = ReadSectors(levels, skip, results, workspace);
        var unread = 0;
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            if ((skip >> sector & 1) == 0 && results[sector] is null)
                unread |= 1 << sector;
        }

        if (unread != 0)
        {
            LocalLevels(levels, workspace.Local);
            Span<PetalLaneResult?> retry = new PetalLaneResult?[PetalLayout.Sectors];
            ReadSectors(workspace.Local, ~unread & AllSectors, retry, workspace);
            for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                results[sector] ??= retry[sector];
        }

        return separation;
    }

    /// <summary>
    /// One read of the field lanes from cell levels with per-codeword, per-colour thresholds;
    /// returns how far the cells read sit from their thresholds on average (a pose that lands on
    /// the petals separates lit from dark cells best).
    /// </summary>
    private static double ReadSectors(ReadOnlySpan<double> levels, int skip, Span<PetalLaneResult?> results, Workspace workspace)
    {
        var separation = 0.0;
        var counted = 0;
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            results[sector] = null;
            if ((skip >> sector & 1) != 0)
                continue;
            ReadWord(levels, sector, ref separation, workspace.Word, workspace.Confidence, workspace);
            counted += PetalLanes.FieldBits;
            results[sector] = DecodeWithErasures(PetalLanes.Field(sector), workspace.Word, workspace.Confidence);
        }

        return separation / Math.Max(counted, 1);
    }

    /// <summary>
    /// The transmitted word of one sector read from cell levels, and each byte's confidence (the
    /// smallest margin of its bits). Every bit's margin from its threshold is added to
    /// <paramref name="margins"/>.
    /// </summary>
    private static void ReadWord(
        ReadOnlySpan<double> levels,
        int sector,
        ref double margins,
        byte[] bytes,
        double[] confidence,
        Workspace workspace)
    {
        var cells = SectorBitCells.AsSpan(sector * PetalLanes.FieldBits, PetalLanes.FieldBits);
        var lights = SectorBitLights.AsSpan(sector * PetalLanes.FieldBits, PetalLanes.FieldBits);
        var values = workspace.SectorValues.AsSpan();
        for (var bit = 0; bit < cells.Length; bit++)
            values[bit] = levels[cells[bit]];
        var all = TwoMeans(values);
        // the light (`天`) and pink petals are thresholded apart: their lit levels differ
        var lightThreshold = ClassThreshold(values, lights, true, SectorLightCounts[sector], all, workspace.ClassValues);
        var pinkThreshold = ClassThreshold(values, lights, false, PetalLanes.FieldBits - SectorLightCounts[sector], all, workspace.ClassValues);
        Array.Clear(bytes);
        Array.Fill(confidence, double.MaxValue);
        for (var bit = 0; bit < values.Length; bit++)
        {
            var value = values[bit];
            var threshold = lights[bit] ? lightThreshold : pinkThreshold;
            if (value > threshold)
                bytes[bit / 8] |= (byte)(1 << (7 - bit % 8));
            var margin = Math.Abs(value - threshold);
            margins += margin;
            confidence[bit / 8] = PetalMath.Min(confidence[bit / 8], margin);
        }
    }

    /// <summary>The two-means threshold of one colour class of a codeword, or <paramref name="all"/> for a class of fewer than <see cref="ClassMinCells"/> cells.</summary>
    private static double ClassThreshold(
        ReadOnlySpan<double> values,
        ReadOnlySpan<bool> lights,
        bool light,
        int members,
        double all,
        double[] scratch)
    {
        if (members < ClassMinCells)
            return all;
        var count = 0;
        for (var bit = 0; bit < values.Length; bit++)
        {
            if (lights[bit] == light)
                scratch[count++] = values[bit];
        }

        return TwoMeans(scratch.AsSpan(0, count));
    }

    /// <summary>Fits the pose with distortion <paramref name="distortion"/> and reads the field under it into <paramref name="into"/> (no local read).</summary>
    private static bool ReadFieldAt(
        PetalLuma image,
        ReadOnlySpan<PetalPoint> destination,
        double distortion,
        int? inferred,
        FieldRead into,
        Workspace workspace)
    {
        if (FitPose(image, destination, distortion) is not { } pose)
            return false;
        if (ReferenceLevels(image, pose, inferred) is not { } reference)
            return false;
        FieldLevels(image, pose, reference, into.Levels);
        into.Pose = pose;
        into.Reference = reference;
        into.Separation = ReadSectors(into.Levels, 0, into.Fields, workspace);
        into.Decoded = Count(into.Fields);
        return true;
    }

    /// <summary>Reads the sectors this pose left unread again from their <see cref="LocalLevels"/>.</summary>
    private static FieldRead Finish(FieldRead read, Workspace workspace)
    {
        if (read.Decoded < PetalLayout.Sectors)
        {
            var unread = 0;
            for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                unread |= read.Fields[sector] is null ? 1 << sector : 0;
            LocalLevels(read.Levels, workspace.Local);
            Span<PetalLaneResult?> retry = new PetalLaneResult?[PetalLayout.Sectors];
            ReadSectors(workspace.Local, ~unread & AllSectors, retry, workspace);
            for (var sector = 0; sector < PetalLayout.Sectors; sector++)
                read.Fields[sector] ??= retry[sector];
            read.Decoded = Count(read.Fields);
        }

        return read;
    }

    /// <summary>
    /// Reads the field through the finder centres <paramref name="destination"/>, starting from
    /// lens distortion <paramref name="start"/>, and searches the distortion while some field lane
    /// stays unread.
    /// </summary>
    private static FieldRead? ReadFieldSearching(
        PetalLuma image,
        ReadOnlySpan<PetalPoint> destination,
        double start,
        int? inferred,
        Workspace workspace)
    {
        var best = workspace.First;
        if (!ReadFieldAt(image, destination, start, inferred, best, workspace))
            return null;
        return SearchDistortion(image, destination, start, inferred, best, workspace.Second, workspace);
    }

    /// <summary>
    /// The distortion search from the read <paramref name="best"/> at <paramref name="start"/>:
    /// <paramref name="start"/> plus each distortion step in turn, then the best
    /// ±<see cref="DistortionRefine"/>. The pose that decodes the most lanes wins (ties: the
    /// clearer separation); the search stops once every lane decodes. Lanes the winning pose
    /// still leaves unread are read again from its <see cref="LocalLevels"/>.
    /// </summary>
    private static FieldRead SearchDistortion(
        PetalLuma image,
        ReadOnlySpan<PetalPoint> destination,
        double start,
        int? inferred,
        FieldRead best,
        FieldRead candidate,
        Workspace workspace)
    {
        if (best.Decoded == PetalLayout.Sectors)
            return best;
        foreach (var step in DistortionSteps)
        {
            if (ReadFieldAt(image, destination, start + step, inferred, candidate, workspace) && candidate.BetterThan(best))
            {
                (best, candidate) = (candidate, best);
                if (best.Decoded == PetalLayout.Sectors)
                    return best;
            }
        }

        var centre = best.Pose.Distortion;
        Span<double> refinements = [centre - DistortionRefine, centre + DistortionRefine];
        foreach (var k in refinements)
        {
            if (ReadFieldAt(image, destination, k, inferred, candidate, workspace) && candidate.BetterThan(best))
                (best, candidate) = (candidate, best);
        }

        return Finish(best, workspace);
    }

    private static int Count(ReadOnlySpan<PetalLaneResult?> fields)
    {
        var count = 0;
        foreach (var field in fields)
            count += field is null ? 0 : 1;
        return count;
    }

    /// <summary>One orientation hypothesis: gate score, orientation, finder centres, plain pose, levels and inferred corner.</summary>
    private readonly record struct Scored(
        double Gate,
        int Rotation,
        bool Mirrored,
        PetalPoint[] Destination,
        PetalPose Pose,
        Reference Reference,
        int? InferredCorner);

    /// <summary>A read of the field under one pose.</summary>
    private sealed class FieldRead
    {
        public double[] Levels { get; } = new double[PetalLayout.FieldCells];

        public PetalLaneResult?[] Fields { get; } = new PetalLaneResult?[PetalLayout.Sectors];

        public PetalPose Pose { get; set; }

        public Reference Reference { get; set; }

        public int Decoded { get; set; }

        public double Separation { get; set; }

        /// <summary>More lanes, then a clearer separation (Rust tuple <c>&gt;</c>).</summary>
        public bool BetterThan(FieldRead other) =>
            Decoded != other.Decoded ? Decoded > other.Decoded : Separation > other.Separation;
    }

    /// <summary>Per-thread scratch buffers, so a camera loop does not allocate them for every frame.</summary>
    private sealed class Workspace
    {
        [ThreadStatic]
        private static Workspace? current;

        public static Workspace Current => current ??= new Workspace();

        /// <summary>The two reads of the distortion search: the best so far and the one being read.</summary>
        public FieldRead First { get; } = new();

        public FieldRead Second { get; } = new();

        /// <summary>Levels outside a field read: <see cref="DecodeAt"/> and the merge mean.</summary>
        public double[] Levels { get; } = new double[PetalLayout.FieldCells];

        /// <summary>The local levels of the local read.</summary>
        public double[] Local { get; } = new double[PetalLayout.FieldCells];

        /// <summary>The levels of the bit cells of the sector being read, in bit order.</summary>
        public double[] SectorValues { get; } = new double[PetalLanes.FieldBits];

        /// <summary>The levels of one colour class of the sector being read.</summary>
        public double[] ClassValues { get; } = new double[PetalLanes.FieldBits];

        /// <summary>The transmitted word of the sector being read and the confidence of each byte.</summary>
        public byte[] Word { get; } = new byte[PetalLanes.FieldWordLength];

        public double[] Confidence { get; } = new double[PetalLanes.FieldWordLength];
    }

    /// <summary>Lit and dark levels at the four finders, interpolated over the canvas.</summary>
    internal struct Reference
    {
        private double lit0;
        private double lit1;
        private double lit2;
        private double lit3;
        private double dark0;
        private double dark1;
        private double dark2;
        private double dark3;

        public void Set(int corner, double lit, double dark)
        {
            switch (corner)
            {
                case 0:
                    (lit0, dark0) = (lit, dark);
                    break;
                case 1:
                    (lit1, dark1) = (lit, dark);
                    break;
                case 2:
                    (lit2, dark2) = (lit, dark);
                    break;
                default:
                    (lit3, dark3) = (lit, dark);
                    break;
            }
        }

        /// <summary>
        /// Fills in the levels of corner <paramref name="missing"/> from the other three by the
        /// parallelogram rule (neighbours minus the opposite corner), kept within their range.
        /// </summary>
        public void Extrapolate(int missing)
        {
            var (n1, opposite, n2) = ((missing + 1) % 4, (missing + 2) % 4, (missing + 3) % 4);
            var lit = Extrapolated(Lit(n1), Lit(opposite), Lit(n2));
            var dark = Extrapolated(Dark(n1), Dark(opposite), Dark(n2));
            Set(missing, lit, dark);
        }

        /// <summary>The light level measured (or extrapolated) at corner <paramref name="corner"/>.</summary>
        public readonly double Lit(int corner) => corner switch
        {
            0 => lit0,
            1 => lit1,
            2 => lit2,
            _ => lit3,
        };

        /// <summary>The dark level measured (or extrapolated) at corner <paramref name="corner"/>.</summary>
        public readonly double Dark(int corner) => corner switch
        {
            0 => dark0,
            1 => dark1,
            2 => dark2,
            _ => dark3,
        };

        /// <summary>Bilinear interpolation over the canvas of the four corner estimates.</summary>
        public readonly void At(double x, double y, out double lit, out double dark) =>
            AtFraction(Math.Clamp(x / 1024.0, 0.0, 1.0), Math.Clamp(y / 1024.0, 0.0, 1.0), out lit, out dark);

        /// <summary><see cref="At"/> with the canvas position already divided by 1024 and clamped.</summary>
        public readonly void AtFraction(double u, double v, out double lit, out double dark)
        {
            lit = Mix(lit0, lit1, lit2, lit3, u, v);
            dark = Mix(dark0, dark1, dark2, dark3, u, v);
        }

        private static double Mix(double c0, double c1, double c2, double c3, double u, double v)
        {
            var top = c0 * (1.0 - u) + c1 * u;
            var bottom = c3 * (1.0 - u) + c2 * u;
            return top * (1.0 - v) + bottom * v;
        }

        /// <summary><c>(a + b − opposite)</c> clamped to the range of the three (Rust <c>f64::clamp</c>).</summary>
        private static double Extrapolated(double a, double opposite, double b)
        {
            var low = PetalMath.Min(PetalMath.Min(a, opposite), b);
            var high = PetalMath.Max(PetalMath.Max(a, opposite), b);
            var value = a + b - opposite;
            if (value < low)
                return low;
            return value > high ? high : value;
        }
    }
}
