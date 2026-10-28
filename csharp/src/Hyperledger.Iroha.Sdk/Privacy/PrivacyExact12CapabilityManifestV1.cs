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

/// <summary>Runtime availability projected from the actual compiled implementation.</summary>
public enum PrivacyCapabilityReadinessV1
{
    Available = 0,
    Unavailable = 1,
}

/// <summary>Typed runtime reason why an Exact12 protocol is unavailable.</summary>
public enum PrivacyCapabilityUnavailableReasonV1
{
    CompiledProfile,
}

/// <summary>Typed reason why this binary has no complete executable profile.</summary>
public enum PrivacyCompiledProfileUnavailableReasonV1
{
    EngineUnavailable,
    ProfileInitializationFailed,
    StatementSchemaInvalid,
    JointSecurityReductionIncomplete,
}

/// <summary>Typed failure while canonicalizing a compiled public-statement schema.</summary>
public enum PrivacyCompiledStatementSchemaErrorV1
{
    ConflictingStableTypeId,
    MissingTypeReference,
}

/// <summary>Bounded manifest-validation result. It is not native release authority.</summary>
public enum PrivacyExact12CapabilityManifestValidationStatusV1
{
    Valid = 0,
    Empty = 1,
    ArchiveTooLarge = 2,
    NativeUnavailable = 3,
    InvalidManifest = 4,
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
    private readonly byte[] policy;

    internal PrivacyExact12CapabilityRowV1(
        PrivacyProtocolIdV1 protocolId,
        PrivacyOperationSchemaV1 operationSchema,
        PrivacyExecutionModeV1 executionMode,
        byte privacyFeatureMask,
        PrivacyCapabilityReadinessV1 readiness,
        PrivacyCapabilityUnavailableReasonV1? unavailableReason,
        PrivacyCompiledProfileUnavailableReasonV1? compiledProfileUnavailableReason,
        PrivacyCompiledStatementSchemaErrorV1? compiledStatementSchemaError,
        byte[] compiledProfile,
        byte[] policy,
        bool localCompiledTupleMatches)
    {
        ProtocolId = protocolId;
        OperationSchema = operationSchema;
        ExecutionMode = executionMode;
        PrivacyFeatureMask = privacyFeatureMask;
        Readiness = readiness;
        UnavailableReason = unavailableReason;
        CompiledProfileUnavailableReason = compiledProfileUnavailableReason;
        CompiledStatementSchemaError = compiledStatementSchemaError;
        this.compiledProfile = (byte[])compiledProfile.Clone();
        this.policy = (byte[])policy.Clone();
        LocalCompiledTupleMatches = localCompiledTupleMatches;
    }

    public PrivacyProtocolIdV1 ProtocolId { get; }

    public PrivacyOperationSchemaV1 OperationSchema { get; }

    public PrivacyExecutionModeV1 ExecutionMode { get; }

    public byte PrivacyFeatureMask { get; }

    public PrivacyCapabilityReadinessV1 Readiness { get; }

    /// <summary>Exact typed reason when <see cref="Readiness"/> is unavailable.</summary>
    public PrivacyCapabilityUnavailableReasonV1? UnavailableReason { get; }

    /// <summary>Nested compiled-profile reason when <see cref="UnavailableReason"/> is compiled profile.</summary>
    public PrivacyCompiledProfileUnavailableReasonV1? CompiledProfileUnavailableReason { get; }

    /// <summary>Nested schema error when the compiled profile's statement schema is invalid.</summary>
    public PrivacyCompiledStatementSchemaErrorV1? CompiledStatementSchemaError { get; }

    /// <summary>
    /// True only after byte-exact comparison with the native-validated local compiled catalog.
    /// </summary>
    public bool LocalCompiledTupleMatches { get; }

    /// <summary>Defensive copy of the canonical committed compiled-profile result field.</summary>
    public byte[] CompiledProfileCanonicalBytes => (byte[])compiledProfile.Clone();

    /// <summary>
    /// Defensive copy of the optional ordinary protocol policy with current limits and pending update.
    /// </summary>
    public byte[] PolicyCanonicalBytes => (byte[])policy.Clone();

    /// <summary>Availability is derived from the actual compiled implementation; policy registration is optional.</summary>
    public bool IsNetworkAvailable =>
        Readiness == PrivacyCapabilityReadinessV1.Available;
}

/// <summary>
/// Canonical manifest obtained from authenticated Torii transport with local tuple comparisons.
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
        byte[] canonicalArchive,
        NetworkId networkId)
    {
        NetworkId = networkId;
        Version = version;
        CommittedHeight = committedHeight;
        this.consensusPolicy = (byte[])consensusPolicy.Clone();
        Protocols = new ReadOnlyCollection<PrivacyExact12CapabilityRowV1>(protocols.ToArray());
        this.manifestDigest = (byte[])manifestDigest.Clone();
        this.canonicalArchive = (byte[])canonicalArchive.Clone();
    }

    public uint Version { get; }

    /// <summary>Exact manifest network, checked against the authenticated Torii client.</summary>
    public NetworkId NetworkId { get; }

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

        var expectedNetworkId = client.Options.NetworkId ?? throw new InvalidOperationException(
            "Exact12 privacy capabilities require the configured exact network identity.");

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
        PrivacyNative.RequireValidCapabilityArchive(archive);
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
            archive,
            localCatalog);
        PrivacyExact12CapabilityManifestCodecV1.RequireManifestNetwork(
            decoded.NetworkId, expectedNetworkId);
        return new PrivacyExact12CapabilityManifestV1(
            decoded.Version,
            decoded.CommittedHeight,
            decoded.ConsensusPolicy,
            decoded.Protocols,
            decoded.ManifestDigest,
            decoded.CanonicalArchive,
            expectedNetworkId);
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
    private readonly byte[] manifestArchive;
    private readonly object seal;

    private PrivacyExact12CapabilityTupleAdmissionV1(
        PrivacyProtocolIdV1 protocolId,
        ulong committedHeight,
        byte[] manifestDigest,
        PrivacyOperationSchemaV1 operationSchema,
        NetworkId networkId,
        byte[] manifestArchive)
    {
        NetworkId = networkId;
        this.manifestArchive = (byte[])manifestArchive.Clone();
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

    /// <summary>Exact network this token may authorize at construction time.</summary>
    public NetworkId NetworkId { get; }

    internal static PrivacyExact12CapabilityTupleAdmissionV1 IssueValidated(
        PrivacyExact12CapabilityManifestV1 manifest,
        PrivacyProtocolIdV1 protocol)
    {
        ArgumentNullException.ThrowIfNull(manifest);
        _ = manifest.RowFor(protocol);
        PrivacyNative.RequireValidCapabilityArchive(manifest.CanonicalBytes);
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
            manifest.CanonicalBytes, PrivacyNative.CompiledProfileCatalogV1().NoritoBytes);
        PrivacyExact12CapabilityManifestCodecV1.RequireManifestNetwork(
            decoded.NetworkId, manifest.NetworkId);
        var row = decoded.Protocols[checked((int)protocol)];
        PrivacyExact12CapabilityManifestCodecV1.RequireSelectedRuntimeProfile(row);
        return new PrivacyExact12CapabilityTupleAdmissionV1(
            row.ProtocolId,
            manifest.CommittedHeight,
            manifest.ManifestDigest,
            row.OperationSchema,
            manifest.NetworkId,
            manifest.CanonicalBytes);
    }

    internal byte[] CopyAuthenticatedArchive(PrivacyProtocolIdV1 protocol, NetworkId expectedNetworkId)
    {
        RequireAuthentic(protocol, expectedNetworkId);
        return (byte[])manifestArchive.Clone();
    }

    internal void RequireAuthentic(PrivacyProtocolIdV1 protocol, NetworkId expectedNetworkId)
    {
        if (!ReferenceEquals(seal, AdmissionSeal) || ProtocolId != protocol
            || !NetworkId.Equals(expectedNetworkId))
        {
            throw new InvalidOperationException(
                "Exact12 capability admission is absent, invalid, or protocol/network-substituted.");
        }
        PrivacyNative.RequireValidCapabilityArchive(manifestArchive);
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
            manifestArchive, PrivacyNative.CompiledProfileCatalogV1().NoritoBytes);
        PrivacyExact12CapabilityManifestCodecV1.RequireManifestNetwork(
            decoded.NetworkId, expectedNetworkId);
        var row = decoded.Protocols[checked((int)protocol)];
        PrivacyExact12CapabilityManifestCodecV1.RequireSelectedRuntimeProfile(row);
        if (decoded.CommittedHeight != CommittedHeight
            || !decoded.ManifestDigest.AsSpan().SequenceEqual(manifestDigest))
        {
            throw new InvalidOperationException("Exact12 capability admission no longer validates.");
        }
    }
}

/// <summary>Fail-closed bridge from authenticated committed state to privacy construction.</summary>
public static class PrivacyExact12CapabilityAdmissionV1
{
    /// <summary>Require actual implementation availability and the exact selected local native tuple.</summary>
    public static PrivacyExact12CapabilityTupleAdmissionV1 RequireExact12CapabilityTupleV1(
        PrivacyExact12CapabilityManifestV1 manifest,
        PrivacyProtocolIdV1 protocol)
    {
        return PrivacyExact12CapabilityTupleAdmissionV1.IssueValidated(manifest, protocol);
    }

    /// <summary>Verify a sealed token immediately before retained privacy construction.</summary>
    public static void RequireForConstruction(
        PrivacyExact12CapabilityTupleAdmissionV1 admission,
        PrivacyProtocolIdV1 protocol,
        NetworkId expectedNetworkId)
    {
        ArgumentNullException.ThrowIfNull(admission);
        ArgumentNullException.ThrowIfNull(expectedNetworkId);
        admission.RequireAuthentic(protocol, expectedNetworkId);
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
        0,
        6,
        1,
        2,
        2,
        2,
        0,
        2,
        7,
        2,
        7,
        31,
    ];

    private static readonly uint[] ExpectedProofSystems =
    [
        0,
        2,
        3,
        1,
        4,
        0,
        5,
        8,
        6,
        7,
        0,
        0,
    ];

    private static readonly uint[] ExpectedEngines =
    [
        0,
        2,
        3,
        1,
        4,
        0,
        5,
        8,
        6,
        7,
        0,
        0,
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
        NetworkId networkId;
        try { networkId = NetworkId.FromBytes(reader.ReadField("manifest.network_id", out _)); }
        catch (ArgumentException error) { throw Invalid($"Invalid Exact12 network identity: {error.Message}"); }
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
            rows[index] = DecodeRow(
                row,
                index,
                committedHeight,
                localProfiles[index]);
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
            networkId,
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
        var localCompiledTupleMatches = compiledProfile.AsSpan().SequenceEqual(localCompiledProfile);
        var compiled = ParseCompiledResult(compiledProfile, protocol, index);

        var readiness = ParseReadiness(
            reader.ReadField($"row[{index}].readiness", out _),
            index);

        var policy = reader.ReadField($"row[{index}].policy", out _).ToArray();
        ParsePolicy(
            policy,
            protocol,
            committedHeight,
            index);
        reader.RequireEnd($"row[{index}]");
        ValidateProjectedReadiness(
            readiness,
            compiled,
            index);

        return new PrivacyExact12CapabilityRowV1(
            protocol,
            operationSchema,
            executionMode,
            featureMask,
            readiness.Readiness,
            readiness.UnavailableReason,
            readiness.CompiledProfileUnavailableReason,
            readiness.CompiledStatementSchemaError,
            compiledProfile,
            policy,
            localCompiledTupleMatches);
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
            var unavailable = ParseCompiledProfileUnavailableReason(reason, index);
            reader.RequireEnd($"row[{index}].compiled_profile");
            return new CompiledResult(
                false,
                reason,
                unavailable.Reason,
                unavailable.StatementSchemaError,
                null);
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
        RequireUnitEnum(
            fields[1],
            ExpectedProofSystems[index],
            $"row[{index}].compiled_profile.proof_system_id");
        RequireUnitEnum(
            fields[2],
            ExpectedEngines[index],
            $"row[{index}].compiled_profile.engine_id");
        for (var field = 3; field <= 7; field++)
        {
            RequireNonzeroDigestNewtype(
                fields[field],
                $"row[{index}].compiled_profile.digest[{field - 3}]");
        }
        _ = ParseProtocolLimits(fields[8], protocol, $"row[{index}].compiled_profile.protocol_limits");
        return new CompiledResult(true, null, null, null, fields);
    }

    private static ReadinessProjection ParseReadiness(
        ReadOnlySpan<byte> encoded,
        int index)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"row[{index}].readiness.tag");
        if (tag == 0)
        {
            reader.RequireEnd($"row[{index}].readiness");
            return new ReadinessProjection(
                PrivacyCapabilityReadinessV1.Available,
                null,
                null,
                null,
                null);
        }
        if (tag != 1)
        {
            throw Invalid($"Exact12 row {index} readiness discriminant is unknown.");
        }

        var reasonBytes = reader.ReadField($"row[{index}].readiness.reason", out _).ToArray();
        reader.RequireEnd($"row[{index}].readiness");
        var reasonReader = new FrameReader(reasonBytes);
        var reasonTag = reasonReader.ReadUInt32Raw($"row[{index}].readiness.reason.tag");
        if (reasonTag == 0)
        {
            var compiledReasonBytes = reasonReader.ReadField(
                $"row[{index}].readiness.reason.compiled_profile",
                out _).ToArray();
            reasonReader.RequireEnd($"row[{index}].readiness.reason");
            var compiledReason = ParseCompiledProfileUnavailableReason(compiledReasonBytes, index);
            return new ReadinessProjection(
                PrivacyCapabilityReadinessV1.Unavailable,
                PrivacyCapabilityUnavailableReasonV1.CompiledProfile,
                compiledReason.Reason,
                compiledReason.StatementSchemaError,
                compiledReasonBytes);
        }

        throw Invalid($"Exact12 row {index} readiness reason is retired or unknown.");
    }

    private static CompiledUnavailableReason ParseCompiledProfileUnavailableReason(
        ReadOnlySpan<byte> encoded,
        int index)
    {
        var reader = new FrameReader(encoded);
        var tag = reader.ReadUInt32Raw($"row[{index}].compiled_profile.reason.tag");
        if (tag == 3)
        {
            reader.RequireEnd($"row[{index}].compiled_profile.reason");
            return new CompiledUnavailableReason(
                PrivacyCompiledProfileUnavailableReasonV1.JointSecurityReductionIncomplete, null);
        }
        if (tag <= 1)
        {
            reader.RequireEnd($"row[{index}].compiled_profile.reason");
            return new CompiledUnavailableReason(
                tag == 0
                    ? PrivacyCompiledProfileUnavailableReasonV1.EngineUnavailable
                    : PrivacyCompiledProfileUnavailableReasonV1.ProfileInitializationFailed,
                null);
        }
        if (tag != 2)
        {
            throw Invalid($"Exact12 row {index} compiled-profile unavailable reason is unknown.");
        }

        var schemaError = reader.ReadField(
            $"row[{index}].compiled_profile.reason.schema_error",
            out _);
        reader.RequireEnd($"row[{index}].compiled_profile.reason");
        var schemaTag = ReadUnitEnum(
            schemaError,
            $"row[{index}].compiled_profile.reason.schema_error");
        var parsedSchemaError = schemaTag switch
        {
            0 => PrivacyCompiledStatementSchemaErrorV1.ConflictingStableTypeId,
            1 => PrivacyCompiledStatementSchemaErrorV1.MissingTypeReference,
            _ => throw Invalid($"Exact12 row {index} statement-schema error is unknown."),
        };
        return new CompiledUnavailableReason(
            PrivacyCompiledProfileUnavailableReasonV1.StatementSchemaInvalid,
            parsedSchemaError);
    }

    private static void ValidateProjectedReadiness(ReadinessProjection readiness, CompiledResult compiled, int index)
    {
        var expected = compiled.IsAvailable ? PrivacyCapabilityReadinessV1.Available : PrivacyCapabilityReadinessV1.Unavailable;
        if (readiness.Readiness != expected || (!compiled.IsAvailable &&
            (readiness.UnavailableReason != PrivacyCapabilityUnavailableReasonV1.CompiledProfile
             || readiness.CompiledProfileReasonCanonical is null || compiled.UnavailableReason is null
             || !readiness.CompiledProfileReasonCanonical.AsSpan().SequenceEqual(compiled.UnavailableReason))))
            throw Invalid($"Exact12 row {index} readiness differs from its actual compiled result.");
    }

    private static void ParsePolicy(ReadOnlySpan<byte> encoded,
        PrivacyProtocolIdV1 protocol, ulong committedHeight, int index)
    {
        var option = new FrameReader(encoded);
        var tag = option.ReadByteRaw($"row[{index}].policy.option");
        if (tag == 0) { option.RequireEnd($"row[{index}].policy"); return; }
        if (tag != 1) throw Invalid($"Exact12 row {index} policy option tag is unknown.");
        var value = option.ReadField($"row[{index}].policy.value", out _);
        option.RequireEnd($"row[{index}].policy");
        var reader = new FrameReader(value);
        RequireUnitEnum(reader.ReadField($"row[{index}].policy.protocol_id", out _), (uint)protocol, $"row[{index}].policy.protocol_id");
        var current = ParseProtocolLimits(reader.ReadField($"row[{index}].policy.current_limits", out _), protocol, $"row[{index}].policy.current_limits");
        var pending = reader.ReadField($"row[{index}].policy.pending_update", out _);
        reader.RequireEnd($"row[{index}].policy");
        // Policies are ordinary bounded values, including for currently unavailable engines.
        // The selected native admission owner enforces this binary's supported limits.
        ValidatePendingProtocolLimits(pending, current, protocol, committedHeight, index);
    }

    // This checks one selected row only; it does not create authenticated origin or an admission token.
    internal static void RequireSelectedRuntimeProfile(PrivacyExact12CapabilityRowV1 row)
    {
        if (!row.IsNetworkAvailable)
            throw new PrivacyCapabilityUnavailableException(row);
        if (!row.LocalCompiledTupleMatches)
            throw new LocalTupleMismatchException("Exact12 selected profile differs from CompiledProfileCatalogV1.");
    }

    internal static void RequireManifestNetwork(NetworkId manifestNetworkId, NetworkId expectedNetworkId)
    {
        ArgumentNullException.ThrowIfNull(manifestNetworkId);
        ArgumentNullException.ThrowIfNull(expectedNetworkId);
        if (!manifestNetworkId.Equals(expectedNetworkId))
        {
            throw Invalid("Exact12 capability manifest differs from the configured network identity.");
        }
    }

    private static void ValidateConsensusPolicy(ReadOnlySpan<byte> encoded, ulong committedHeight)
    {
        var reader = new FrameReader(encoded);
        var current = ParseConsensusLimits(
            reader.ReadField("manifest.consensus_policy.current_limits", out _),
            "manifest.consensus_policy.current_limits");
        var pending = reader.ReadField("manifest.consensus_policy.pending_update", out _);
        reader.RequireEnd("manifest.consensus_policy");
        var option = new FrameReader(pending);
        var tag = option.ReadByteRaw("manifest.consensus_policy.pending_update.option");
        if (tag == 0)
        {
            option.RequireEnd("manifest.consensus_policy.pending_update");
            return;
        }
        if (tag != 1)
        {
            throw Invalid("Privacy consensus pending-update option tag is unknown.");
        }
        var value = option.ReadField("manifest.consensus_policy.pending_update.value", out _);
        option.RequireEnd("manifest.consensus_policy.pending_update");
        var update = new FrameReader(value);
        var scheduled = ReadUInt64Field(ref update, "privacy policy scheduled_at_height");
        var effective = ReadUInt64Field(ref update, "privacy policy effective_at_height");
        var next = ParseConsensusLimits(
            update.ReadField("privacy policy next_limits", out _),
            "privacy policy next_limits");
        update.RequireEnd("privacy policy update");
        RequireSchedule(scheduled, effective, committedHeight, "privacy policy update");
        if (next.Values.SequenceEqual(current.Values))
        {
            throw Invalid("Privacy policy update must not be a no-op.");
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
            9 * 1024 * 1024 + 256 * 1024 + 512,
            9 * 1024 * 1024 + 256 * 1024 + 512,
            16 * 1024 * 1024 - 64 * 1024,
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
            2 => [8],
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
        var tag = option.ReadByteRaw($"row[{index}].policy.pending_update.option");
        if (tag == 0)
        {
            option.RequireEnd($"row[{index}].policy.pending_update");
            return;
        }
        if (tag != 1)
        {
            throw Invalid($"Exact12 row {index} pending-limit option tag is unknown.");
        }
        var value = option.ReadField($"row[{index}].policy.pending_update.value", out _);
        option.RequireEnd($"row[{index}].policy.pending_update");
        var reader = new FrameReader(value);
        var scheduled = ReadUInt64Field(ref reader, $"row[{index}].pending_update.scheduled_at_height");
        var effective = ReadUInt64Field(ref reader, $"row[{index}].pending_update.effective_at_height");
        var next = ParseProtocolLimits(
            reader.ReadField($"row[{index}].pending_update.next_limits", out _),
            protocol,
            $"row[{index}].pending_update.next_limits");
        reader.RequireEnd($"row[{index}].pending_update");
        RequireSchedule(scheduled, effective, committedHeight, $"row[{index}].pending_update");
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
            || effective <= scheduled)
        {
            throw Invalid($"{context} has invalid future or committed-height binding.");
        }
    }

    private static void RequireNonzeroDigestNewtype(ReadOnlySpan<byte> encoded, string context)
    {
        _ = ReadNonzeroDigestNewtype(encoded, context);
    }

    private static byte[] ReadNonzeroDigestNewtype(ReadOnlySpan<byte> encoded, string context)
    {
        var reader = new FrameReader(encoded);
        var digest = reader.ReadField($"{context}.value", out _);
        reader.RequireEnd(context);
        if (digest.Length != 32 || digest.IndexOfAnyExcept((byte)0) < 0)
        {
            throw Invalid($"{context} must be one nonzero 32-byte value.");
        }
        return digest.ToArray();
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

    private static ushort ReadUInt16Field(ref FrameReader reader, string context)
    {
        var field = reader.ReadField(context, out _);
        if (field.Length != sizeof(ushort))
        {
            throw Invalid($"{context} must contain exactly two bytes.");
        }
        return BinaryPrimitives.ReadUInt16LittleEndian(field);
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
        NetworkId NetworkId,
        ulong CommittedHeight,
        byte[] ConsensusPolicy,
        IReadOnlyList<PrivacyExact12CapabilityRowV1> Protocols,
        byte[] ManifestDigest,
        byte[] CanonicalArchive);

    private sealed record CompiledResult(
        bool IsAvailable,
        byte[]? UnavailableReason,
        PrivacyCompiledProfileUnavailableReasonV1? UnavailableReasonKind,
        PrivacyCompiledStatementSchemaErrorV1? StatementSchemaError,
        byte[][]? AvailableFields);

    private sealed record CompiledUnavailableReason(
        PrivacyCompiledProfileUnavailableReasonV1 Reason,
        PrivacyCompiledStatementSchemaErrorV1? StatementSchemaError);

    private sealed record ReadinessProjection(
        PrivacyCapabilityReadinessV1 Readiness,
        PrivacyCapabilityUnavailableReasonV1? UnavailableReason,
        PrivacyCompiledProfileUnavailableReasonV1? CompiledProfileUnavailableReason,
        PrivacyCompiledStatementSchemaErrorV1? CompiledStatementSchemaError,
        byte[]? CompiledProfileReasonCanonical);

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

        internal string ReadCompactString(string context)
        {
            var length = ReadCompactLength($"{context}.length");
            if (length > int.MaxValue)
            {
                throw Invalid($"{context} exceeds the managed runtime bound.");
            }
            var value = ReadExact(checked((int)length), context);
            try
            {
                return new UTF8Encoding(false, true).GetString(value);
            }
            catch (DecoderFallbackException error)
            {
                throw Invalid($"{context} is not UTF-8: {error.Message}");
            }
        }

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
