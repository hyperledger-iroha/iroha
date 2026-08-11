using System.Buffers.Binary;
using System.Collections.ObjectModel;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Closed public operation schema carried by an Exact12 capability row.</summary>
public enum PrivacyOperationSchemaV1 : uint
{
    ZkAceAuthorizationActionV1 = 0,
    AnonymousPgcPaymentActionV1 = 1,
    VeRangeRangeProofV1 = 2,
    ZkAmsAdmissionAndProvisioningV1 = 3,
    VegaCredentialPresentationV1 = 4,
    ZkX509IdentityPresentationV1 = 5,
    JindoPolynomialEvaluationV1 = 6,
    BootleLanternCredentialPresentationV1 = 7,
    OrchardNoteActionV1 = 8,
    FcmpMembershipPaymentV1 = 9,
    IvmPrivateNoteActionV1 = 10,
    PqMaspNoteActionV1 = 11,
}

/// <summary>Closed execution classification carried by an Exact12 capability row.</summary>
public enum PrivacyExecutionModeV1 : uint
{
    AuthorizationAction = 0,
    PaymentAction = 1,
    Component = 2,
    AdmissionAction = 3,
    PresentationAction = 4,
    NoteAction = 5,
}

/// <summary>Evidence-derived readiness projected from a committed compiled profile.</summary>
public enum PrivacyCapabilityReadinessV1
{
    Available,
    AvailableExperimental,
    Unavailable,
}

/// <summary>Projection of the committed governance lifecycle.</summary>
public enum PrivacyCapabilityActivationStateV1 : uint
{
    NotRegistered = 0,
    Proposed = 1,
    Active = 2,
    Suspended = 3,
    Retired = 4,
}

/// <summary>Explicit limitation carried only by revised Jindo.</summary>
public enum PrivacyCapabilityLimitationV1
{
    MissingDistributionWideKnowledgeSoundnessEvidence,
}

/// <summary>Bounded manifest-validation result. It is not native release authority.</summary>
public enum PrivacyExact12CapabilityManifestValidationStatusV1
{
    Valid = 0,
    Empty = 1,
    ArchiveTooLarge = 2,
    NativeUnavailable = 3,
    InvalidManifest = 4,
    LocalCompiledTupleMismatch = 5,
}

/// <summary>Strict canonical or semantic manifest validation failure.</summary>
public class PrivacyExact12CapabilityManifestException : IOException
{
    internal PrivacyExact12CapabilityManifestException(string message)
        : base(message)
    {
    }
}

/// <summary>One validated row from the committed Exact12 capability manifest.</summary>
public sealed class PrivacyExact12CapabilityRowV1
{
    private readonly byte[] compiledProfile;
    private readonly byte[] activation;

    internal PrivacyExact12CapabilityRowV1(
        PrivacyProtocolIdV1 protocolId,
        PrivacyOperationSchemaV1 operationSchema,
        PrivacyExecutionModeV1 executionMode,
        byte privacyFeatureMask,
        PrivacyCapabilityReadinessV1 readiness,
        PrivacyCapabilityActivationStateV1 activationState,
        PrivacyCapabilityLimitationV1? limitation,
        byte[] compiledProfile,
        byte[] activation,
        bool localCompiledTupleMatches)
    {
        ProtocolId = protocolId;
        OperationSchema = operationSchema;
        ExecutionMode = executionMode;
        PrivacyFeatureMask = privacyFeatureMask;
        Readiness = readiness;
        ActivationState = activationState;
        Limitation = limitation;
        this.compiledProfile = (byte[])compiledProfile.Clone();
        this.activation = (byte[])activation.Clone();
        LocalCompiledTupleMatches = localCompiledTupleMatches;
    }

    public PrivacyProtocolIdV1 ProtocolId { get; }

    public PrivacyOperationSchemaV1 OperationSchema { get; }

    public PrivacyExecutionModeV1 ExecutionMode { get; }

    public byte PrivacyFeatureMask { get; }

    public PrivacyCapabilityReadinessV1 Readiness { get; }

    public PrivacyCapabilityActivationStateV1 ActivationState { get; }

    public PrivacyCapabilityLimitationV1? Limitation { get; }

    /// <summary>
    /// True only after byte-exact comparison with the native-validated local compiled catalog.
    /// </summary>
    public bool LocalCompiledTupleMatches { get; }

    /// <summary>Defensive copy of the canonical committed compiled-profile result field.</summary>
    public byte[] CompiledProfileCanonicalBytes => (byte[])compiledProfile.Clone();

    /// <summary>
    /// Defensive copy of the complete canonical committed activation option, including immutable
    /// profile bindings, lifecycle, current and pending limits, and assurance.
    /// </summary>
    public byte[] ActivationCanonicalBytes => (byte[])activation.Clone();

    /// <summary>Committed readiness plus active governance; never derived from a local catalog.</summary>
    public bool IsNetworkAvailable =>
        Readiness is PrivacyCapabilityReadinessV1.Available
            or PrivacyCapabilityReadinessV1.AvailableExperimental
        && ActivationState == PrivacyCapabilityActivationStateV1.Active;
}

/// <summary>
/// Native-tuple-matched canonical manifest obtained from authenticated Torii transport.
/// </summary>
/// <remarks>
/// <see cref="ManifestDigest"/> is content identity, not producer authentication. Public instances
/// can only be issued by the HTTPS, canonical-request-authenticated Torii fetch path.
/// </remarks>
public sealed class PrivacyExact12CapabilityManifestV1
{
    public const uint VersionV1 = 1;
    public const int MaxArchiveBytes = 256 * 1024;

    private const string CapabilitiesPath = "/v1/privacy/capabilities";
    private const string NoritoMediaType = "application/x-norito";

    private readonly byte[] canonicalArchive;
    private readonly byte[] consensusPolicy;
    private readonly byte[] manifestDigest;

    private PrivacyExact12CapabilityManifestV1(
        uint version,
        ulong committedHeight,
        byte[] consensusPolicy,
        IReadOnlyList<PrivacyExact12CapabilityRowV1> protocols,
        byte[] manifestDigest,
        byte[] canonicalArchive)
    {
        Version = version;
        CommittedHeight = committedHeight;
        this.consensusPolicy = (byte[])consensusPolicy.Clone();
        Protocols = new ReadOnlyCollection<PrivacyExact12CapabilityRowV1>(protocols.ToArray());
        this.manifestDigest = (byte[])manifestDigest.Clone();
        this.canonicalArchive = (byte[])canonicalArchive.Clone();
    }

    public uint Version { get; }

    public ulong CommittedHeight { get; }

    public IReadOnlyList<PrivacyExact12CapabilityRowV1> Protocols { get; }

    public byte[] ManifestDigest => (byte[])manifestDigest.Clone();

    public byte[] ConsensusPolicyCanonicalBytes => (byte[])consensusPolicy.Clone();

    public byte[] CanonicalBytes => (byte[])canonicalArchive.Clone();

    public PrivacyExact12CapabilityRowV1 RowFor(PrivacyProtocolIdV1 protocol)
    {
        var tag = (uint)protocol;
        if (tag >= Protocols.Count)
        {
            throw new ArgumentOutOfRangeException(nameof(protocol));
        }
        var index = (int)tag;
        if (Protocols[index].ProtocolId != protocol)
        {
            throw new InvalidOperationException("Exact12 protocol registry order drifted.");
        }
        return Protocols[index];
    }

    /// <summary>
    /// Authority-bearing construction is intentionally confined to this method: it takes a
    /// configured Torii client, never caller-supplied manifest or catalog bytes.
    /// </summary>
    internal static async Task<PrivacyExact12CapabilityManifestV1> FetchAuthenticatedToriiAsync(
        ToriiClient client,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(client);
        if (!string.Equals(client.BaseUri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidOperationException(
                "Exact12 privacy capabilities require an HTTPS Torii endpoint.");
        }
        if (client.Options.CanonicalRequestCredentials is null)
        {
            throw new InvalidOperationException(
                "Exact12 privacy capabilities require canonical request credentials.");
        }

        // Resolve and native-validate the immutable local tuple before any authority-bearing read.
        var localCatalog = PrivacyNative.CompiledProfileCatalogV1().NoritoBytes;
        var expectedUri = new Uri(client.BaseUri, CapabilitiesPath.TrimStart('/'));
        using var response = await client.SendAsync(
            HttpMethod.Get,
            CapabilitiesPath,
            content: null,
            accept: NoritoMediaType,
            cancellationToken: cancellationToken);
        if (response.RequestMessage?.RequestUri is not Uri responseUri
            || !string.Equals(responseUri.AbsoluteUri, expectedUri.AbsoluteUri, StringComparison.Ordinal))
        {
            throw new HttpRequestException(
                "Exact12 privacy capability requests must not follow redirects.");
        }
        RequireExactNoritoContentType(response.Content.Headers.ContentType);
        var archive = await ReadBoundedManifestAsync(response.Content, cancellationToken);
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
            archive,
            localCatalog);
        return new PrivacyExact12CapabilityManifestV1(
            decoded.Version,
            decoded.CommittedHeight,
            decoded.ConsensusPolicy,
            decoded.Protocols,
            decoded.ManifestDigest,
            decoded.CanonicalArchive);
    }

    private static void RequireExactNoritoContentType(MediaTypeHeaderValue? contentType)
    {
        if (contentType is null
            || !string.Equals(contentType.MediaType, NoritoMediaType, StringComparison.Ordinal)
            || contentType.Parameters.Count != 0)
        {
            throw new InvalidDataException(
                "Exact12 privacy capabilities require exact application/x-norito content.");
        }
    }

    private static async Task<byte[]> ReadBoundedManifestAsync(
        HttpContent content,
        CancellationToken cancellationToken)
    {
        var declaredLength = content.Headers.ContentLength;
        if (!declaredLength.HasValue
            || declaredLength.Value <= 0
            || declaredLength.Value > MaxArchiveBytes)
        {
            throw new InvalidDataException(
                "Exact12 privacy capability Content-Length is absent or outside its bound.");
        }

        await using var stream = await content.ReadAsStreamAsync(cancellationToken);
        using var output = new MemoryStream(checked((int)declaredLength.Value));
        var buffer = new byte[8192];
        while (true)
        {
            var count = await stream.ReadAsync(buffer, cancellationToken);
            if (count == 0)
            {
                break;
            }
            if (output.Length > MaxArchiveBytes - count)
            {
                throw new InvalidDataException(
                    "Exact12 privacy capability response exceeds its byte bound.");
            }
            output.Write(buffer, 0, count);
        }
        if (output.Length != declaredLength.Value)
        {
            throw new InvalidDataException(
                "Exact12 privacy capability response length differs from Content-Length.");
        }
        return output.ToArray();
    }
}

/// <summary>Opaque admission token issued only after committed/native tuple agreement.</summary>
public sealed class PrivacyExact12CapabilityTupleAdmissionV1
{
    private static readonly object AdmissionSeal = new();
    private readonly byte[] manifestDigest;
    private readonly object seal;

    private PrivacyExact12CapabilityTupleAdmissionV1(
        PrivacyProtocolIdV1 protocolId,
        ulong committedHeight,
        byte[] manifestDigest,
        PrivacyOperationSchemaV1 operationSchema)
    {
        ProtocolId = protocolId;
        CommittedHeight = committedHeight;
        this.manifestDigest = (byte[])manifestDigest.Clone();
        OperationSchema = operationSchema;
        seal = AdmissionSeal;
    }

    public PrivacyProtocolIdV1 ProtocolId { get; }

    public ulong CommittedHeight { get; }

    public byte[] ManifestDigest => (byte[])manifestDigest.Clone();

    public PrivacyOperationSchemaV1 OperationSchema { get; }

    internal static PrivacyExact12CapabilityTupleAdmissionV1 IssueValidated(
        PrivacyExact12CapabilityManifestV1 manifest,
        PrivacyProtocolIdV1 protocol)
    {
        ArgumentNullException.ThrowIfNull(manifest);
        var row = manifest.RowFor(protocol);
        if (!row.IsNetworkAvailable)
        {
            throw new InvalidOperationException(
                $"Exact12 protocol {protocol.CanonicalLabel()} is not active and ready.");
        }
        if (!row.LocalCompiledTupleMatches)
        {
            throw new InvalidOperationException(
                "Exact12 committed profile differs from CompiledProfileCatalogV1.");
        }
        return new PrivacyExact12CapabilityTupleAdmissionV1(
            row.ProtocolId,
            manifest.CommittedHeight,
            manifest.ManifestDigest,
            row.OperationSchema);
    }

    internal void RequireAuthentic(PrivacyProtocolIdV1 protocol)
    {
        if (!ReferenceEquals(seal, AdmissionSeal) || ProtocolId != protocol)
        {
            throw new InvalidOperationException(
                "Exact12 capability admission is absent, invalid, or protocol-substituted.");
        }
    }
}

/// <summary>Fail-closed bridge from authenticated committed state to privacy construction.</summary>
public static class PrivacyExact12CapabilityAdmissionV1
{
    /// <summary>Require active committed readiness and the exact local native tuple.</summary>
    public static PrivacyExact12CapabilityTupleAdmissionV1 RequireExact12CapabilityTupleV1(
        PrivacyExact12CapabilityManifestV1 manifest,
        PrivacyProtocolIdV1 protocol)
    {
        return PrivacyExact12CapabilityTupleAdmissionV1.IssueValidated(manifest, protocol);
    }

    /// <summary>Verify a sealed token immediately before retained privacy construction.</summary>
    public static void RequireForConstruction(
        PrivacyExact12CapabilityTupleAdmissionV1 admission,
        PrivacyProtocolIdV1 protocol)
    {
        ArgumentNullException.ThrowIfNull(admission);
        admission.RequireAuthentic(protocol);
    }
}

internal static class PrivacyExact12CapabilityManifestCodecV1
{
    internal const string ManifestSchemaName =
        "iroha.privacy.exact12-capability-manifest.v1";
    internal const string CatalogSchemaName =
        "iroha.privacy.compiled-profile-catalog.v1";

    private const byte CanonicalFlags = NoritoCodec.CanonicalLayoutFlags;
    private const int RowCount = 12;
    private const ulong MinimumPolicyDelayBlocks = 300;

    private static readonly byte[] DigestDomain =
        Encoding.UTF8.GetBytes("iroha:privacy:exact12-capability-manifest:v1");

    private static readonly PrivacyExecutionModeV1[] ExpectedExecutionModes =
    [
        PrivacyExecutionModeV1.AuthorizationAction,
        PrivacyExecutionModeV1.PaymentAction,
        PrivacyExecutionModeV1.Component,
        PrivacyExecutionModeV1.AdmissionAction,
        PrivacyExecutionModeV1.PresentationAction,
        PrivacyExecutionModeV1.PresentationAction,
        PrivacyExecutionModeV1.Component,
        PrivacyExecutionModeV1.PresentationAction,
        PrivacyExecutionModeV1.NoteAction,
        PrivacyExecutionModeV1.PaymentAction,
        PrivacyExecutionModeV1.NoteAction,
        PrivacyExecutionModeV1.NoteAction,
    ];

    private static readonly byte[] ExpectedFeatureMasks =
    [
        0, 6, 1, 2, 2, 2, 0, 2, 7, 2, 7, 31,
    ];

    internal static void Validate(byte[] archive, byte[] nativeValidatedCatalog) =>
        _ = Decode(archive, nativeValidatedCatalog);

    internal static DecodedManifest DecodeValidated(
        byte[] archive,
        byte[] nativeValidatedCatalog) => Decode(archive, nativeValidatedCatalog);

    private static DecodedManifest Decode(byte[] archive, byte[] nativeValidatedCatalog)
    {
        ArgumentNullException.ThrowIfNull(archive);
        ArgumentNullException.ThrowIfNull(nativeValidatedCatalog);
        if (archive.Length == 0 || archive.Length > PrivacyExact12CapabilityManifestV1.MaxArchiveBytes)
        {
            throw Invalid("Exact12 capability manifest length is outside its bound.");
        }

        var localProfiles = DecodeCatalog(nativeValidatedCatalog);
        var payload = DecodeExactArchive(ManifestSchemaName, archive);
        var reader = new FrameReader(payload);
        var version = ReadUInt32Field(ref reader, "manifest.version");
        if (version != PrivacyExact12CapabilityManifestV1.VersionV1)
        {
            throw Invalid("Exact12 capability manifest version must be 1.");
        }
        var committedHeight = ReadUInt64Field(ref reader, "manifest.committed_height");
        var consensusPolicy = reader.ReadField(
            "manifest.consensus_policy",
            out _).ToArray();
        ValidateConsensusPolicy(consensusPolicy, committedHeight);

        var protocolSequence = reader.ReadField("manifest.protocols", out _);
        var protocolReader = new FrameReader(protocolSequence);
        if (protocolReader.ReadUInt64Raw("manifest.protocols.length") != RowCount)
        {
            throw Invalid("Exact12 capability manifest must contain exactly twelve rows.");
        }

        var rows = new PrivacyExact12CapabilityRowV1[RowCount];
        for (var index = 0; index < RowCount; index++)
        {
            var row = protocolReader.ReadField($"manifest.protocols[{index}]", out _);
            rows[index] = DecodeRow(row, index, committedHeight, localProfiles[index]);
        }
        protocolReader.RequireEnd("manifest.protocols");

        var digestWrapper = reader.ReadField("manifest.manifest_digest", out var digestWrapperOffset);
        var digestReader = new FrameReader(digestWrapper);
        var digest = digestReader.ReadField("manifest.manifest_digest.value", out var digestOffset);
        if (digest.Length != 32)
        {
            throw Invalid("Exact12 manifest digest must contain exactly 32 bytes.");
        }
        digestReader.RequireEnd("manifest.manifest_digest");
        reader.RequireEnd("manifest");

        var normalizedPayload = (byte[])payload.Clone();
        normalizedPayload.AsSpan(digestWrapperOffset + digestOffset, 32).Clear();
        var normalizedArchive = NoritoCodec.Encode(
            ManifestSchemaName,
            normalizedPayload,
            CanonicalFlags);
        Span<byte> encodedLength = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(
            encodedLength,
            checked((ulong)normalizedArchive.Length));
        var digestInput = new byte[
            DigestDomain.Length + encodedLength.Length + normalizedArchive.Length];
        DigestDomain.CopyTo(digestInput, 0);
        encodedLength.CopyTo(digestInput.AsSpan(DigestDomain.Length));
        normalizedArchive.CopyTo(digestInput, DigestDomain.Length + encodedLength.Length);
        var computed = SHA256.HashData(digestInput);
        if (!CryptographicOperations.FixedTimeEquals(computed, digest))
        {
            throw Invalid("Exact12 manifest self-digest does not match canonical bytes.");
        }

        return new DecodedManifest(
            version,
            committedHeight,
            consensusPolicy,
            rows,
            digest.ToArray(),
            (byte[])archive.Clone());
    }

    private static byte[][] DecodeCatalog(byte[] archive)
    {
        if (archive.Length == 0 || archive.Length > PrivacyNative.PrivacyCompiledProfileCatalogArchiveMaxBytes)
        {
            throw Invalid("Native compiled-profile catalog length is outside its bound.");
        }
        var payload = DecodeExactArchive(CatalogSchemaName, archive);
        var reader = new FrameReader(payload);
        if (ReadUInt32Field(ref reader, "catalog.version") != 1)
        {
            throw Invalid("Native compiled-profile catalog version must be 1.");
        }
        var sequence = reader.ReadField("catalog.protocols", out _);
        reader.RequireEnd("catalog");
        var sequenceReader = new FrameReader(sequence);
        if (sequenceReader.ReadUInt64Raw("catalog.protocols.length") != RowCount)
        {
            throw Invalid("Native compiled-profile catalog must contain exactly twelve rows.");
        }
        var profiles = new byte[RowCount][];
        for (var index = 0; index < RowCount; index++)
        {
            var row = sequenceReader.ReadField($"catalog.protocols[{index}]", out _);
            var rowReader = new FrameReader(row);
            RequireUnitEnum(
                rowReader.ReadField($"catalog.protocols[{index}].protocol_id", out _),
                checked((uint)index),
                $"catalog.protocols[{index}].protocol_id");
            profiles[index] = rowReader.ReadField(
                $"catalog.protocols[{index}].compiled_profile",
                out _).ToArray();
            rowReader.RequireEnd($"catalog.protocols[{index}]");
        }
        sequenceReader.RequireEnd("catalog.protocols");
        return profiles;
    }

    private static PrivacyExact12CapabilityRowV1 DecodeRow(
        ReadOnlySpan<byte> encoded,
        int index,
        ulong committedHeight,
        byte[] localCompiledProfile)
    {
        var reader = new FrameReader(encoded);
        var protocol = (PrivacyProtocolIdV1)checked((uint)index);
        RequireUnitEnum(
            reader.ReadField($"row[{index}].protocol_id", out _),
            checked((uint)index),
            $"row[{index}].protocol_id");

        var operationSchema = (PrivacyOperationSchemaV1)ReadUnitEnum(
            reader.ReadField($"row[{index}].operation_schema", out _),
            $"row[{index}].operation_schema");
        if ((uint)operationSchema != (uint)index)
        {
            throw Invalid($"Exact12 row {index} operation schema is not canonical.");
        }

        var executionMode = (PrivacyExecutionModeV1)ReadUnitEnum(
            reader.ReadField($"row[{index}].execution_mode", out _),
            $"row[{index}].execution_mode");
        if (executionMode != ExpectedExecutionModes[index])
        {
            throw Invalid($"Exact12 row {index} execution mode is not canonical.");
        }

        var featureMask = ReadByteNewtype(
            reader.ReadField($"row[{index}].privacy_feature_mask", out _),
            $"row[{index}].privacy_feature_mask");
        if (featureMask != ExpectedFeatureMasks[index])
        {
            throw Invalid($"Exact12 row {index} privacy feature mask is not canonical.");
        }

        var compiledProfile = reader.ReadField(
            $"row[{index}].compiled_profile",
            out _).ToArray();
        if (!compiledProfile.AsSpan().SequenceEqual(localCompiledProfile))
        {
            throw new LocalTupleMismatchException(
                $"Exact12 row {index} differs from the native compiled-profile catalog.");
        }
        var compiled = ParseCompiledResult(compiledProfile, protocol, index);

        var readiness = ParseReadiness(
            reader.ReadField($"row[{index}].readiness", out _),
            compiled,
            protocol,
            index);
        var activationState = (PrivacyCapabilityActivationStateV1)ReadUnitEnum(
            reader.ReadField($"row[{index}].activation_state", out _),
            $"row[{index}].activation_state");
        if (!Enum.IsDefined(activationState))
        {
            throw Invalid($"Exact12 row {index} activation state is unknown.");
        }

        var activation = reader.ReadField($"row[{index}].activation", out _).ToArray();
        var projectedActivation = ParseActivation(
            activation,
            compiled,
            protocol,
            committedHeight,
            index);
        if (activationState != projectedActivation)
        {
            throw Invalid($"Exact12 row {index} activation-state projection is inconsistent.");
        }

        var limitation = ParseLimitation(
            reader.ReadField($"row[{index}].limitation", out _),
            protocol,
            index);
        reader.RequireEnd($"row[{index}]");

        return new PrivacyExact12CapabilityRowV1(
            protocol,
            operationSchema,
            executionMode,
            featureMask,
            readiness,
            activationState,
            limitation,
            compiledProfile,
            activation,
            localCompiledTupleMatches: true);
    }

    private static CompiledResult ParseCompiledResult(
        ReadOnlySpan<byte> encoded,
        PrivacyProtocolIdV1 protocol,
        int index)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"row[{index}].compiled_profile.status");
        if (tag == 1)
        {
            var reason = reader.ReadField($"row[{index}].compiled_profile.reason", out _).ToArray();
            ValidateUnavailableReason(reason, index);
            reader.RequireEnd($"row[{index}].compiled_profile");
            return new CompiledResult(false, reason, null);
        }
        if (tag != 0)
        {
            throw Invalid($"Exact12 row {index} compiled-profile status is unknown.");
        }

        var profile = reader.ReadField($"row[{index}].compiled_profile.value", out _);
        reader.RequireEnd($"row[{index}].compiled_profile");
        var profileReader = new FrameReader(profile);
        var fields = new byte[9][];
        for (var field = 0; field < fields.Length; field++)
        {
            fields[field] = profileReader.ReadField(
                $"row[{index}].compiled_profile.value.field[{field}]",
                out _).ToArray();
        }
        profileReader.RequireEnd($"row[{index}].compiled_profile.value");
        RequireUnitEnum(fields[0], (uint)protocol, $"row[{index}].compiled_profile.protocol_id");
        for (var field = 3; field <= 7; field++)
        {
            RequireNonzeroDigestNewtype(
                fields[field],
                $"row[{index}].compiled_profile.digest[{field - 3}]");
        }
        _ = ParseProtocolLimits(fields[8], protocol, $"row[{index}].compiled_profile.protocol_limits");
        return new CompiledResult(true, null, fields);
    }

    private static PrivacyCapabilityReadinessV1 ParseReadiness(
        ReadOnlySpan<byte> encoded,
        CompiledResult compiled,
        PrivacyProtocolIdV1 protocol,
        int index)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"row[{index}].readiness.tag");
        if (!compiled.IsAvailable)
        {
            if (tag != 2)
            {
                throw Invalid($"Exact12 row {index} unavailable profile has available readiness.");
            }
            var reason = reader.ReadField($"row[{index}].readiness.reason", out _);
            reader.RequireEnd($"row[{index}].readiness");
            if (!reason.SequenceEqual(compiled.UnavailableReason))
            {
                throw Invalid($"Exact12 row {index} readiness reason differs from its profile.");
            }
            return PrivacyCapabilityReadinessV1.Unavailable;
        }

        var expected = protocol == PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0 ? 1U : 0U;
        if (tag != expected)
        {
            throw Invalid($"Exact12 row {index} readiness is not evidence-derived.");
        }
        reader.RequireEnd($"row[{index}].readiness");
        return expected == 1
            ? PrivacyCapabilityReadinessV1.AvailableExperimental
            : PrivacyCapabilityReadinessV1.Available;
    }

    private static PrivacyCapabilityActivationStateV1 ParseActivation(
        ReadOnlySpan<byte> encoded,
        CompiledResult compiled,
        PrivacyProtocolIdV1 protocol,
        ulong committedHeight,
        int index)
    {
        var option = new FrameReader(encoded);
        var optionTag = option.ReadByteRaw($"row[{index}].activation.option");
        if (optionTag == 0)
        {
            option.RequireEnd($"row[{index}].activation");
            return PrivacyCapabilityActivationStateV1.NotRegistered;
        }
        if (optionTag != 1)
        {
            throw Invalid($"Exact12 row {index} activation option tag is unknown.");
        }
        if (!compiled.IsAvailable || compiled.AvailableFields is null)
        {
            throw Invalid($"Exact12 row {index} cannot activate an unavailable profile.");
        }

        var activation = option.ReadField($"row[{index}].activation.value", out _);
        option.RequireEnd($"row[{index}].activation");
        var reader = new FrameReader(activation);
        var fields = new byte[12][];
        for (var field = 0; field < fields.Length; field++)
        {
            fields[field] = reader.ReadField(
                $"row[{index}].activation.value.field[{field}]",
                out _).ToArray();
        }
        reader.RequireEnd($"row[{index}].activation.value");
        for (var field = 0; field <= 7; field++)
        {
            if (!fields[field].AsSpan().SequenceEqual(compiled.AvailableFields[field]))
            {
                throw Invalid($"Exact12 row {index} activation binding field {field} drifted.");
            }
        }

        var ceiling = ParseProtocolLimits(
            compiled.AvailableFields[8],
            protocol,
            $"row[{index}].compiled_profile.protocol_limits");
        var current = ParseProtocolLimits(
            fields[9],
            protocol,
            $"row[{index}].activation.protocol_limits");
        RequireLimitCeiling(current, ceiling, $"row[{index}].activation.protocol_limits");
        ValidatePendingProtocolLimits(fields[10], current, protocol, committedHeight, index);
        RequireUnitEnum(fields[11], 0, $"row[{index}].activation.assurance");
        return ParseLifecycle(fields[8], committedHeight, index);
    }

    private static PrivacyCapabilityActivationStateV1 ParseLifecycle(
        ReadOnlySpan<byte> encoded,
        ulong committedHeight,
        int index)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"row[{index}].activation.lifecycle.state");
        if (tag > 3)
        {
            throw Invalid($"Exact12 row {index} lifecycle state is unknown.");
        }
        var record = reader.ReadField($"row[{index}].activation.lifecycle.record", out _);
        reader.RequireEnd($"row[{index}].activation.lifecycle");
        var fields = new FrameReader(record);
        var proposed = ReadUInt64Field(ref fields, $"row[{index}].lifecycle.proposed_at_height");
        if (proposed == 0 || proposed > committedHeight)
        {
            throw Invalid($"Exact12 row {index} lifecycle proposal height is invalid.");
        }

        if (tag == 0)
        {
            var activate = ReadUInt64Field(ref fields, $"row[{index}].lifecycle.activate_at_height");
            fields.RequireEnd($"row[{index}].lifecycle");
            if (activate <= proposed || activate <= committedHeight)
            {
                throw Invalid($"Exact12 row {index} proposed activation height is invalid.");
            }
            return PrivacyCapabilityActivationStateV1.Proposed;
        }

        ulong? activated;
        if (tag == 3)
        {
            activated = ReadOptionalUInt64Field(
                ref fields,
                $"row[{index}].lifecycle.activated_at_height");
        }
        else
        {
            activated = ReadUInt64Field(
                ref fields,
                $"row[{index}].lifecycle.activated_at_height");
        }
        var stateSince = ReadUInt64Field(ref fields, $"row[{index}].lifecycle.state_since_height");
        fields.RequireEnd($"row[{index}].lifecycle");
        if (stateSince == 0 || stateSince > committedHeight)
        {
            throw Invalid($"Exact12 row {index} lifecycle state height is invalid.");
        }
        if (activated.HasValue)
        {
            if (activated.Value <= proposed || activated.Value > committedHeight)
            {
                throw Invalid($"Exact12 row {index} activation height is invalid.");
            }
            var validStateOrder = tag == 1
                ? stateSince >= activated.Value
                : stateSince > activated.Value;
            if (!validStateOrder)
            {
                throw Invalid($"Exact12 row {index} lifecycle state ordering is invalid.");
            }
        }
        else if (tag != 3 || stateSince <= proposed)
        {
            throw Invalid($"Exact12 row {index} retired lifecycle ordering is invalid.");
        }

        return tag switch
        {
            1 => PrivacyCapabilityActivationStateV1.Active,
            2 => PrivacyCapabilityActivationStateV1.Suspended,
            _ => PrivacyCapabilityActivationStateV1.Retired,
        };
    }

    private static PrivacyCapabilityLimitationV1? ParseLimitation(
        ReadOnlySpan<byte> encoded,
        PrivacyProtocolIdV1 protocol,
        int index)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadByteRaw($"row[{index}].limitation.option");
        var jindo = protocol == PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0;
        if (tag == 0)
        {
            reader.RequireEnd($"row[{index}].limitation");
            if (jindo)
            {
                throw Invalid("Revised Jindo must expose its missing knowledge-soundness evidence.");
            }
            return null;
        }
        if (tag != 1 || !jindo)
        {
            throw Invalid($"Exact12 row {index} limitation is non-canonical.");
        }
        RequireUnitEnum(
            reader.ReadField($"row[{index}].limitation.value", out _),
            0,
            $"row[{index}].limitation.value");
        reader.RequireEnd($"row[{index}].limitation");
        return PrivacyCapabilityLimitationV1.MissingDistributionWideKnowledgeSoundnessEvidence;
    }

    private static void ValidateConsensusPolicy(ReadOnlySpan<byte> encoded, ulong committedHeight)
    {
        var reader = new FrameReader(encoded);
        var current = ParseConsensusLimits(
            reader.ReadField("manifest.consensus_policy.current_limits", out _),
            "manifest.consensus_policy.current_limits");
        var pending = reader.ReadField("manifest.consensus_policy.pending_tightening", out _);
        reader.RequireEnd("manifest.consensus_policy");
        var option = new FrameReader(pending);
        var tag = option.ReadByteRaw("manifest.consensus_policy.pending_tightening.option");
        if (tag == 0)
        {
            option.RequireEnd("manifest.consensus_policy.pending_tightening");
            return;
        }
        if (tag != 1)
        {
            throw Invalid("Privacy consensus pending-tightening option tag is unknown.");
        }
        var value = option.ReadField("manifest.consensus_policy.pending_tightening.value", out _);
        option.RequireEnd("manifest.consensus_policy.pending_tightening");
        var tightening = new FrameReader(value);
        var scheduled = ReadUInt64Field(ref tightening, "privacy policy scheduled_at_height");
        var effective = ReadUInt64Field(ref tightening, "privacy policy effective_at_height");
        var next = ParseConsensusLimits(
            tightening.ReadField("privacy policy next_limits", out _),
            "privacy policy next_limits");
        tightening.RequireEnd("privacy policy tightening");
        RequireSchedule(scheduled, effective, committedHeight, "privacy policy tightening");
        RequireLimitCeiling(next, current, "privacy policy tightening");
        if (next.Values.SequenceEqual(current.Values))
        {
            throw Invalid("Privacy policy tightening must not be a no-op.");
        }
    }

    private static LimitSet ParseConsensusLimits(ReadOnlySpan<byte> encoded, string context)
    {
        var reader = new FrameReader(encoded);
        var values = new uint[10];
        for (var index = 0; index < values.Length; index++)
        {
            values[index] = ReadUInt32Field(ref reader, $"{context}.field[{index}]");
        }
        reader.RequireEnd(context);
        uint[] maxima =
        [
            1,
            2,
            9 * 1024 * 1024,
            9 * 1024 * 1024,
            9 * 1024 * 1024,
            18 * 1024 * 1024,
            256 * 1024,
            8,
            8,
            2_048,
        ];
        for (var index = 0; index < values.Length; index++)
        {
            if (values[index] == 0 || values[index] > maxima[index])
            {
                throw Invalid($"{context}.field[{index}] is outside the first-release ceiling.");
            }
        }
        if (values[0] > values[1]
            || values[2] > values[3]
            || values[3] > values[4]
            || values[4] > values[5]
            || values[6] > values[3])
        {
            throw Invalid($"{context} has inconsistent containing-scope limits.");
        }
        return new LimitSet(uint.MaxValue, values);
    }

    private static LimitSet ParseProtocolLimits(
        ReadOnlySpan<byte> encoded,
        PrivacyProtocolIdV1 expectedProtocol,
        string context)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"{context}.protocol");
        if (tag != (uint)expectedProtocol)
        {
            throw Invalid($"{context} targets another protocol.");
        }
        var fieldCount = tag switch
        {
            1 or 3 or 9 or 10 or 11 => 2,
            2 or 6 or 8 => 1,
            0 or 4 or 5 or 7 => 0,
            _ => throw Invalid($"{context} has an unknown protocol tag."),
        };
        if (fieldCount == 0)
        {
            reader.RequireEnd(context);
            return new LimitSet(tag, Array.Empty<uint>());
        }
        var limits = reader.ReadField($"{context}.limits", out _);
        reader.RequireEnd(context);
        var limitReader = new FrameReader(limits);
        var values = new uint[fieldCount];
        for (var index = 0; index < fieldCount; index++)
        {
            values[index] = ReadUInt32Field(ref limitReader, $"{context}.limits[{index}]");
        }
        limitReader.RequireEnd($"{context}.limits");
        uint[] maxima = tag switch
        {
            1 => [64, 8],
            2 => [16],
            3 => [8, 64],
            6 => [4],
            8 => [2],
            9 => [2, 4],
            10 => [2, 2],
            11 => [2, 2],
            _ => throw Invalid($"{context} has an unsupported limit shape."),
        };
        for (var index = 0; index < values.Length; index++)
        {
            if (values[index] == 0 || values[index] > maxima[index])
            {
                throw Invalid($"{context}.limits[{index}] exceeds the first-release ceiling.");
            }
        }
        if (tag == 1 && values[0] is not (16 or 32 or 64))
        {
            throw Invalid($"{context} uses a non-canonical Anonymous PGC set size.");
        }
        if (tag == 3 && values[1] is not (16 or 32 or 64))
        {
            throw Invalid($"{context} uses a non-canonical ZK-AMS ring size.");
        }
        return new LimitSet(tag, values);
    }

    private static void ValidatePendingProtocolLimits(
        ReadOnlySpan<byte> encoded,
        LimitSet current,
        PrivacyProtocolIdV1 protocol,
        ulong committedHeight,
        int index)
    {
        var option = new FrameReader(encoded);
        var tag = option.ReadByteRaw($"row[{index}].activation.pending_limits.option");
        if (tag == 0)
        {
            option.RequireEnd($"row[{index}].activation.pending_limits");
            return;
        }
        if (tag != 1)
        {
            throw Invalid($"Exact12 row {index} pending-limit option tag is unknown.");
        }
        var value = option.ReadField($"row[{index}].activation.pending_limits.value", out _);
        option.RequireEnd($"row[{index}].activation.pending_limits");
        var reader = new FrameReader(value);
        var scheduled = ReadUInt64Field(ref reader, $"row[{index}].pending_limits.scheduled_at_height");
        var effective = ReadUInt64Field(ref reader, $"row[{index}].pending_limits.effective_at_height");
        var next = ParseProtocolLimits(
            reader.ReadField($"row[{index}].pending_limits.next_limits", out _),
            protocol,
            $"row[{index}].pending_limits.next_limits");
        reader.RequireEnd($"row[{index}].pending_limits");
        RequireSchedule(scheduled, effective, committedHeight, $"row[{index}].pending_limits");
        RequireLimitCeiling(next, current, $"row[{index}].pending_limits");
        if (next.Values.SequenceEqual(current.Values))
        {
            throw Invalid($"Exact12 row {index} pending protocol limits are a no-op.");
        }
    }

    private static void RequireSchedule(
        ulong scheduled,
        ulong effective,
        ulong committedHeight,
        string context)
    {
        if (scheduled == 0
            || scheduled > committedHeight
            || effective <= committedHeight
            || effective <= scheduled
            || scheduled > ulong.MaxValue - MinimumPolicyDelayBlocks
            || effective < scheduled + MinimumPolicyDelayBlocks)
        {
            throw Invalid($"{context} has invalid notice or committed-height binding.");
        }
    }

    private static void RequireLimitCeiling(LimitSet value, LimitSet ceiling, string context)
    {
        if (value.ProtocolTag != ceiling.ProtocolTag || value.Values.Length != ceiling.Values.Length)
        {
            throw Invalid($"{context} has a mismatched protocol-limit shape.");
        }
        for (var index = 0; index < value.Values.Length; index++)
        {
            if (value.Values[index] > ceiling.Values[index])
            {
                throw Invalid($"{context}.field[{index}] exceeds its compiled ceiling.");
            }
        }
    }

    private static void ValidateUnavailableReason(ReadOnlySpan<byte> encoded, int index)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"row[{index}].compiled_profile.reason.tag");
        if (tag <= 1)
        {
            reader.RequireEnd($"row[{index}].compiled_profile.reason");
            return;
        }
        if (tag != 2)
        {
            throw Invalid($"Exact12 row {index} unavailable reason is unknown.");
        }
        var schemaError = reader.ReadField(
            $"row[{index}].compiled_profile.reason.schema_error",
            out _);
        reader.RequireEnd($"row[{index}].compiled_profile.reason");
        var schemaTag = ReadUnitEnum(schemaError, $"row[{index}].compiled_profile.reason.schema_error");
        if (schemaTag > 1)
        {
            throw Invalid($"Exact12 row {index} statement-schema error is unknown.");
        }
    }

    private static void RequireNonzeroDigestNewtype(ReadOnlySpan<byte> encoded, string context)
    {
        var reader = new FrameReader(encoded);
        var digest = reader.ReadField($"{context}.value", out _);
        reader.RequireEnd(context);
        if (digest.Length != 32 || digest.IndexOfAnyExcept((byte)0) < 0)
        {
            throw Invalid($"{context} must be one nonzero 32-byte value.");
        }
    }

    private static byte ReadByteNewtype(ReadOnlySpan<byte> encoded, string context)
    {
        var reader = new FrameReader(encoded);
        var value = reader.ReadField($"{context}.value", out _);
        reader.RequireEnd(context);
        if (value.Length != 1)
        {
            throw Invalid($"{context} must contain exactly one byte.");
        }
        return value[0];
    }

    private static uint ReadUnitEnum(ReadOnlySpan<byte> encoded, string context)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"{context}.tag");
        reader.RequireEnd(context);
        return tag;
    }

    private static void RequireUnitEnum(ReadOnlySpan<byte> encoded, uint expected, string context)
    {
        if (ReadUnitEnum(encoded, context) != expected)
        {
            throw Invalid($"{context} has an unexpected discriminant.");
        }
    }

    private static uint ReadUInt32Field(ref FrameReader reader, string context)
    {
        var field = reader.ReadField(context, out _);
        if (field.Length != sizeof(uint))
        {
            throw Invalid($"{context} must contain exactly four bytes.");
        }
        return BinaryPrimitives.ReadUInt32LittleEndian(field);
    }

    private static ulong ReadUInt64Field(ref FrameReader reader, string context)
    {
        var field = reader.ReadField(context, out _);
        if (field.Length != sizeof(ulong))
        {
            throw Invalid($"{context} must contain exactly eight bytes.");
        }
        return BinaryPrimitives.ReadUInt64LittleEndian(field);
    }

    private static ulong? ReadOptionalUInt64Field(ref FrameReader reader, string context)
    {
        var encoded = reader.ReadField(context, out _);
        var option = new FrameReader(encoded);
        var tag = option.ReadByteRaw($"{context}.option");
        if (tag == 0)
        {
            option.RequireEnd(context);
            return null;
        }
        if (tag != 1)
        {
            throw Invalid($"{context} has an unknown option tag.");
        }
        var value = option.ReadField($"{context}.value", out _);
        option.RequireEnd(context);
        if (value.Length != sizeof(ulong))
        {
            throw Invalid($"{context} must contain one UInt64.");
        }
        return BinaryPrimitives.ReadUInt64LittleEndian(value);
    }

    private static byte[] DecodeExactArchive(string schemaName, byte[] archive)
    {
        byte[] payload;
        byte flags;
        try
        {
            (payload, flags) = NoritoCodec.Decode(schemaName, archive);
        }
        catch (ArgumentException error)
        {
            throw Invalid($"{schemaName} is not a valid Norito archive: {error.Message}");
        }
        if (flags != CanonicalFlags
            || archive.Length != NoritoHeader.EncodedLength + payload.Length
            || !NoritoCodec.Encode(schemaName, payload, CanonicalFlags).AsSpan().SequenceEqual(archive))
        {
            throw Invalid($"{schemaName} is not one canonical unpadded archive.");
        }
        return payload;
    }

    private static PrivacyExact12CapabilityManifestException Invalid(string message) => new(message);

    internal sealed record DecodedManifest(
        uint Version,
        ulong CommittedHeight,
        byte[] ConsensusPolicy,
        IReadOnlyList<PrivacyExact12CapabilityRowV1> Protocols,
        byte[] ManifestDigest,
        byte[] CanonicalArchive);

    private sealed record CompiledResult(
        bool IsAvailable,
        byte[]? UnavailableReason,
        byte[][]? AvailableFields);

    private sealed record LimitSet(uint ProtocolTag, uint[] Values);

    internal sealed class LocalTupleMismatchException : PrivacyExact12CapabilityManifestException
    {
        internal LocalTupleMismatchException(string message)
            : base(message)
        {
        }
    }

    private ref struct FrameReader
    {
        private readonly ReadOnlySpan<byte> input;
        private int offset;

        internal FrameReader(ReadOnlySpan<byte> input)
        {
            this.input = input;
            offset = 0;
        }

        internal ReadOnlySpan<byte> ReadField(string context, out int payloadOffset)
        {
            var length = ReadCompactLength($"{context}.length");
            if (length > int.MaxValue)
            {
                throw Invalid($"{context} exceeds the managed runtime bound.");
            }
            payloadOffset = offset;
            return ReadExact(checked((int)length), context);
        }

        internal byte ReadByteRaw(string context) => ReadExact(1, context)[0];

        internal uint ReadUInt32Raw(string context) =>
            BinaryPrimitives.ReadUInt32LittleEndian(ReadExact(sizeof(uint), context));

        internal ulong ReadUInt64Raw(string context) =>
            BinaryPrimitives.ReadUInt64LittleEndian(ReadExact(sizeof(ulong), context));

        internal void RequireEnd(string context)
        {
            if (offset != input.Length)
            {
                throw Invalid($"{context} contains trailing, duplicate, or unknown bytes.");
            }
        }

        private ulong ReadCompactLength(string context)
        {
            ulong value = 0;
            var shift = 0;
            for (var index = 0; index < 10; index++)
            {
                var current = ReadByteRaw(context);
                var chunk = (ulong)(current & 0x7f);
                if (shift == 63 && chunk > 1)
                {
                    throw Invalid($"{context} overflows UInt64.");
                }
                value |= chunk << shift;
                if ((current & 0x80) == 0)
                {
                    if (index > 0 && chunk == 0)
                    {
                        throw Invalid($"{context} is overlong.");
                    }
                    return value;
                }
                shift += 7;
            }
            throw Invalid($"{context} overflows UInt64.");
        }

        private ReadOnlySpan<byte> ReadExact(int count, string context)
        {
            if (count < 0 || offset > input.Length - count)
            {
                throw Invalid($"{context} is truncated.");
            }
            var result = input.Slice(offset, count);
            offset += count;
            return result;
        }
    }
}
