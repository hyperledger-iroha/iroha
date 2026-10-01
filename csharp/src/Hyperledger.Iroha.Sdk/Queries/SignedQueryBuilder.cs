using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;
using Hyperledger.Iroha.Zk;

namespace Hyperledger.Iroha.Queries;

public sealed class SignedQueryBuilder
{
    private const byte SignedQueryVersion = 1;

    private readonly byte[] networkIdBytes;
    private ManagedSingularQueryKind? singularQueryKind;
    private string? subjectAccountId;
    private string? dataspaceAlias;
    private string? domain;
    private string? assetDefinitionId;
    private ContractArtifactId? contractArtifactId;
    private string? committeeId;
    private string? proofBackend;
    private string? proofHash;
    private string? twitterPepperId;
    private string? twitterBindingDigest;
    private string? storageTicket;
    private string? manifestDigest;
    private string? pinAlias;
    private string? providerId;
    private uint? laneId;
    private ulong? pinEpoch;
    private ulong? pinSequence;
    private ulong? dataspaceId;
    private ulong? dataspaceOwnerId;

    public SignedQueryBuilder(string authorityAccountId, NetworkId networkId)
    {
        AuthorityAccountId = NormalizeAccountId(authorityAccountId, nameof(authorityAccountId));
        NetworkId = networkId ?? throw new ArgumentNullException(nameof(networkId));
        networkIdBytes = NetworkId.ToBytes();
    }

    public string AuthorityAccountId { get; }

    public NetworkId NetworkId { get; }

    public SignedQueryBuilder FindExecutorDataModel()
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindExecutorDataModel;
        return this;
    }

    public SignedQueryBuilder FindParameters()
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindParameters;
        return this;
    }

    public SignedQueryBuilder FindAbiVersion()
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindAbiVersion;
        return this;
    }

    public SignedQueryBuilder FindAliasesByAccountId(string accountId, string? dataspace = null, string? domain = null)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindAliasesByAccountId;
        subjectAccountId = NormalizeAccountId(accountId, nameof(accountId));
        dataspaceAlias = NormalizeOptionalValue(dataspace, nameof(dataspace));
        this.domain = NormalizeOptionalValue(domain, nameof(domain));
        return this;
    }

    public SignedQueryBuilder FindAssetById(string assetDefinitionId, string accountId, ulong? dataspaceId = null)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindAssetById;
        this.assetDefinitionId = NormalizeRequiredValue(assetDefinitionId, nameof(assetDefinitionId));
        subjectAccountId = NormalizeAccountId(accountId, nameof(accountId));
        this.dataspaceId = dataspaceId;
        return this;
    }

    public SignedQueryBuilder FindAssetDefinitionById(string assetDefinitionId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindAssetDefinitionById;
        this.assetDefinitionId = NormalizeRequiredValue(assetDefinitionId, nameof(assetDefinitionId));
        return this;
    }

    public SignedQueryBuilder FindContractManifestByArtifactId(ContractArtifactId artifactId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindContractManifestByArtifactId;
        contractArtifactId = artifactId ?? throw new ArgumentNullException(nameof(artifactId));
        return this;
    }

    public SignedQueryBuilder FindProofRecordById(string backend, string proofHash)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindProofRecordById;
        proofBackend = VerifierBackendRegistryLabels.RequireSupportedLabel(
            backend,
            nameof(backend));
        this.proofHash = NormalizeProofHashHex(proofHash, nameof(proofHash));
        return this;
    }

    public SignedQueryBuilder FindTwitterBindingByHash(string pepperId, string digestHex)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindTwitterBindingByHash;
        twitterPepperId = NormalizeRequiredValue(pepperId, nameof(pepperId));
        twitterBindingDigest = NormalizeRequiredValue(digestHex, nameof(digestHex));
        return this;
    }

    public SignedQueryBuilder FindDomainEndorsements(string domainId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDomainEndorsements;
        domain = NormalizeRequiredDomainId(domainId, nameof(domainId));
        return this;
    }

    public SignedQueryBuilder FindDomainEndorsementPolicy(string domainId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDomainEndorsementPolicy;
        domain = NormalizeRequiredDomainId(domainId, nameof(domainId));
        return this;
    }

    public SignedQueryBuilder FindDomainCommittee(string committeeId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDomainCommittee;
        this.committeeId = NormalizeRequiredValue(committeeId, nameof(committeeId));
        return this;
    }

    public SignedQueryBuilder FindDataspaceNameOwnerById(ulong dataspaceId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDataspaceNameOwnerById;
        dataspaceOwnerId = dataspaceId;
        return this;
    }

    public SignedQueryBuilder FindDaPinIntentByTicket(string storageTicket)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDaPinIntentByTicket;
        this.storageTicket = NormalizeRequiredValue(storageTicket, nameof(storageTicket));
        return this;
    }

    public SignedQueryBuilder FindDaPinIntentByManifest(string manifestDigest)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDaPinIntentByManifest;
        this.manifestDigest = NormalizeRequiredValue(manifestDigest, nameof(manifestDigest));
        return this;
    }

    public SignedQueryBuilder FindDaPinIntentByAlias(string alias)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDaPinIntentByAlias;
        pinAlias = NormalizeRequiredValue(alias, nameof(alias));
        return this;
    }

    public SignedQueryBuilder FindDaPinIntentByLaneEpochSequence(uint laneId, ulong epoch, ulong sequence)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDaPinIntentByLaneEpochSequence;
        this.laneId = laneId;
        pinEpoch = epoch;
        pinSequence = sequence;
        return this;
    }

    public SignedQueryBuilder FindSorafsProviderOwner(string providerId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindSorafsProviderOwner;
        this.providerId = NormalizeRequiredValue(providerId, nameof(providerId));
        return this;
    }

    public SignedQueryEnvelope BuildSigned(ReadOnlySpan<byte> privateKeySeed)
    {
        var (creationTimeMilliseconds, nonce) = SignedQueryRequestContext.CreateFresh();
        return BuildSigned(
            privateKeySeed,
            creationTimeMilliseconds,
            SignedQueryRequestContext.DefaultTimeToLiveMilliseconds,
            nonce);
    }

    public SignedQueryEnvelope BuildSigned(
        ReadOnlySpan<byte> privateKeySeed,
        ulong creationTimeMilliseconds,
        ulong timeToLiveMilliseconds,
        ReadOnlySpan<byte> nonce)
    {
        if (!singularQueryKind.HasValue)
        {
            throw new InvalidOperationException("Queries must select a singular request before signing.");
        }

        var context = new TransactionEncodingContext(AuthorityAccountId);
        context.EnsureAuthorityMatchesPrivateKey(privateKeySeed);

        var payloadBytes = EncodeQueryRequestWithAuthority(
            context,
            creationTimeMilliseconds,
            timeToLiveMilliseconds,
            nonce);
        var payloadHash = IrohaHash.Hash(payloadBytes);
        var signatureBytes = Ed25519Signer.Sign(payloadHash, privateKeySeed);

        var signedQuery = new CanonicalNoritoWriter();
        signedQuery.WriteField(context.EncodeConstVec(signatureBytes));
        signedQuery.WriteField(payloadBytes);
        var signedQueryBytes = signedQuery.ToArray();

        var versionedNoritoBytes = new byte[signedQueryBytes.Length + 1];
        versionedNoritoBytes[0] = SignedQueryVersion;
        signedQueryBytes.CopyTo(versionedNoritoBytes.AsSpan(1));

        return new SignedQueryEnvelope(versionedNoritoBytes, signedQueryBytes, payloadBytes, signatureBytes);
    }

    private byte[] EncodeQueryRequestWithAuthority(
        TransactionEncodingContext context,
        ulong creationTimeMilliseconds,
        ulong timeToLiveMilliseconds,
        ReadOnlySpan<byte> nonce)
    {
        return SignedQueryRequestContext.EncodePayload(
            context,
            networkIdBytes,
            AuthorityAccountId,
            creationTimeMilliseconds,
            timeToLiveMilliseconds,
            nonce,
            EncodeQueryRequest(context));
    }

    private byte[] EncodeQueryRequest(TransactionEncodingContext context)
    {
        return EncodeEnumVariant(0, EncodeSingularQueryBox(context));
    }

    private byte[] EncodeSingularQueryBox(TransactionEncodingContext context)
    {
        return singularQueryKind switch
        {
            ManagedSingularQueryKind.FindExecutorDataModel => EncodeEnumVariant(0, Array.Empty<byte>()),
            ManagedSingularQueryKind.FindParameters => EncodeEnumVariant(1, Array.Empty<byte>()),
            ManagedSingularQueryKind.FindAliasesByAccountId => EncodeEnumVariant(3, EncodeFindAliasesByAccountId(context)),
            ManagedSingularQueryKind.FindProofRecordById => EncodeEnumVariant(6, EncodeFindProofRecordById(context)),
            ManagedSingularQueryKind.FindContractManifestByArtifactId => EncodeEnumVariant(7, EncodeFindContractManifestByArtifactId(context)),
            ManagedSingularQueryKind.FindAbiVersion => EncodeEnumVariant(8, Array.Empty<byte>()),
            ManagedSingularQueryKind.FindAssetById => EncodeEnumVariant(9, EncodeFindAssetById(context)),
            ManagedSingularQueryKind.FindAssetDefinitionById => EncodeEnumVariant(10, EncodeFindAssetDefinitionById(context)),
            ManagedSingularQueryKind.FindTwitterBindingByHash => EncodeEnumVariant(13, EncodeFindTwitterBindingByHash(context)),
            ManagedSingularQueryKind.FindDomainEndorsements => EncodeEnumVariant(19, EncodeFindDomainId(context)),
            ManagedSingularQueryKind.FindDomainEndorsementPolicy => EncodeEnumVariant(20, EncodeFindDomainId(context)),
            ManagedSingularQueryKind.FindDomainCommittee => EncodeEnumVariant(21, EncodeFindDomainCommittee(context)),
            ManagedSingularQueryKind.FindDaPinIntentByTicket => EncodeEnumVariant(22, EncodeFindDaPinIntentByTicket(context)),
            ManagedSingularQueryKind.FindDaPinIntentByManifest => EncodeEnumVariant(23, EncodeFindDaPinIntentByManifest(context)),
            ManagedSingularQueryKind.FindDaPinIntentByAlias => EncodeEnumVariant(24, EncodeFindDaPinIntentByAlias(context)),
            ManagedSingularQueryKind.FindDaPinIntentByLaneEpochSequence => EncodeEnumVariant(25, EncodeFindDaPinIntentByLaneEpochSequence(context)),
            ManagedSingularQueryKind.FindSorafsProviderOwner => EncodeEnumVariant(30, EncodeFindSorafsProviderOwner(context)),
            ManagedSingularQueryKind.FindDataspaceNameOwnerById => EncodeEnumVariant(83, EncodeFindDataspaceNameOwnerById(context)),
            _ => throw new InvalidOperationException("Unsupported managed singular query kind."),
        };
    }

    private byte[] EncodeFindAliasesByAccountId(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeAccountId(RequireSelected(subjectAccountId, nameof(subjectAccountId))));
        writer.WriteField(context.EncodeOptionalString(dataspaceAlias));
        writer.WriteField(context.EncodeOptionalString(domain));
        return writer.ToArray();
    }

    private byte[] EncodeFindAssetById(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeAssetId(
            RequireSelected(assetDefinitionId, nameof(assetDefinitionId)),
            RequireSelected(subjectAccountId, nameof(subjectAccountId)),
            dataspaceId));
        return writer.ToArray();
    }

    private byte[] EncodeFindAssetDefinitionById(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeAssetDefinitionId(RequireSelected(assetDefinitionId, nameof(assetDefinitionId))));
        return writer.ToArray();
    }

    private byte[] EncodeFindContractManifestByArtifactId(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var artifactId = contractArtifactId ?? throw new InvalidOperationException("Contract artifact id is required.");
        var identity = new CanonicalNoritoWriter();
        var dataspace = new CanonicalNoritoWriter();
        dataspace.WriteField(context.EncodeUInt64(artifactId.DataspaceId));
        identity.WriteField(dataspace.ToArray());
        identity.WriteField(Convert.FromHexString(artifactId.CodeHashHex));
        writer.WriteField(identity.ToArray());
        return writer.ToArray();
    }

    private byte[] EncodeFindProofRecordById(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var proof = new CanonicalNoritoWriter();
        proof.WriteField(context.EncodeString(RequireSelected(proofBackend, nameof(proofBackend))));
        proof.WriteField(context.EncodeFixedBytesLiteral(RequireSelected(proofHash, nameof(proofHash)), expectedLength: 32));
        writer.WriteField(proof.ToArray());
        return writer.ToArray();
    }

    private byte[] EncodeFindTwitterBindingByHash(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var keyed = new CanonicalNoritoWriter();
        keyed.WriteField(context.EncodeString(RequireSelected(twitterPepperId, nameof(twitterPepperId))));
        keyed.WriteField(context.EncodeHashLiteral(RequireSelected(twitterBindingDigest, nameof(twitterBindingDigest))));
        writer.WriteField(keyed.ToArray());
        return writer.ToArray();
    }

    private byte[] EncodeFindDomainId(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeDomainId(RequireSelected(domain, nameof(domain))));
        return writer.ToArray();
    }

    private byte[] EncodeFindDomainCommittee(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeString(RequireSelected(committeeId, nameof(committeeId))));
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByTicket(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var identity = new CanonicalNoritoWriter();
        identity.WriteField(context.EncodeFixedBytesLiteral(RequireSelected(storageTicket, nameof(storageTicket)), expectedLength: 32));
        writer.WriteField(identity.ToArray());
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByManifest(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var identity = new CanonicalNoritoWriter();
        identity.WriteField(context.EncodeFixedBytesLiteral(RequireSelected(manifestDigest, nameof(manifestDigest)), expectedLength: 32));
        writer.WriteField(identity.ToArray());
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByAlias(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeString(RequireSelected(pinAlias, nameof(pinAlias))));
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByLaneEpochSequence(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var lane = new CanonicalNoritoWriter();
        lane.WriteField(context.EncodeUInt32(RequireSelected(laneId, nameof(laneId))));
        writer.WriteField(lane.ToArray());
        writer.WriteField(context.EncodeUInt64(RequireSelected(pinEpoch, nameof(pinEpoch))));
        writer.WriteField(context.EncodeUInt64(RequireSelected(pinSequence, nameof(pinSequence))));
        return writer.ToArray();
    }

    private byte[] EncodeFindSorafsProviderOwner(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var identity = new CanonicalNoritoWriter();
        identity.WriteField(context.EncodeFixedBytesLiteral(RequireSelected(providerId, nameof(providerId)), expectedLength: 32));
        writer.WriteField(identity.ToArray());
        return writer.ToArray();
    }

    private byte[] EncodeFindDataspaceNameOwnerById(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        var identity = new CanonicalNoritoWriter();
        identity.WriteField(context.EncodeUInt64(RequireSelected(dataspaceOwnerId, nameof(dataspaceOwnerId))));
        writer.WriteField(identity.ToArray());
        return writer.ToArray();
    }

    private static string NormalizeRequiredDomainId(string value, string paramName)
    {
        return TransactionEncodingContext.CanonicalizeDomainId(value, paramName);
    }

    private static string RequireSelected(string? value, string field)
    {
        return value ?? throw new InvalidOperationException($"{field} must be selected before encoding.");
    }

    private static T RequireSelected<T>(T? value, string field)
        where T : struct
    {
        return value ?? throw new InvalidOperationException($"{field} must be selected before encoding.");
    }

    private static byte[] EncodeEnumVariant(uint discriminant, params byte[][] fields)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(discriminant);
        foreach (var field in fields)
        {
            writer.WriteField(field);
        }

        return writer.ToArray();
    }

    private void ResetArguments()
    {
        subjectAccountId = null;
        dataspaceAlias = null;
        domain = null;
        assetDefinitionId = null;
        contractArtifactId = null;
        committeeId = null;
        proofBackend = null;
        proofHash = null;
        twitterPepperId = null;
        twitterBindingDigest = null;
        storageTicket = null;
        manifestDigest = null;
        pinAlias = null;
        providerId = null;
        laneId = null;
        pinEpoch = null;
        pinSequence = null;
        dataspaceId = null;
        dataspaceOwnerId = null;
    }

    private static string NormalizeRequiredValue(string value, string paramName)
    {
        if (string.IsNullOrEmpty(value))
        {
            throw new ArgumentException("Value cannot be null or empty.", paramName);
        }
        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }
        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }

    private static string? NormalizeOptionalValue(string? value, string paramName)
    {
        return value is null ? null : NormalizeRequiredValue(value, paramName);
    }

    private static string NormalizeAccountId(string value, string paramName)
    {
        var exact = NormalizeRequiredValue(value, paramName);
        try
        {
            return AccountAddress.Parse(exact, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        catch (AccountAddressException exception)
        {
            throw new ArgumentException("Account id must be a canonical I105 account id.", paramName, exception);
        }
    }

    private static string NormalizeProofHashHex(string value, string paramName)
    {
        var normalized = NormalizeRequiredValue(value, paramName).ToLowerInvariant();
        if (normalized.StartsWith("0x", StringComparison.Ordinal))
        {
            normalized = normalized[2..];
        }

        if (normalized.Length != 64 || !IsLowerHex(normalized))
        {
            throw new ArgumentException("Value must be a 32-byte hex string.", paramName);
        }

        return normalized;
    }

    private static bool IsLowerHex(string value)
    {
        foreach (var c in value)
        {
            if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')))
            {
                return false;
            }
        }

        return true;
    }

    private enum ManagedSingularQueryKind
    {
        FindExecutorDataModel,
        FindParameters,
        FindAliasesByAccountId,
        FindProofRecordById,
        FindContractManifestByArtifactId,
        FindAbiVersion,
        FindAssetById,
        FindAssetDefinitionById,
        FindTwitterBindingByHash,
        FindDomainEndorsements,
        FindDomainEndorsementPolicy,
        FindDomainCommittee,
        FindDaPinIntentByTicket,
        FindDaPinIntentByManifest,
        FindDaPinIntentByAlias,
        FindDaPinIntentByLaneEpochSequence,
        FindSorafsProviderOwner,
        FindDataspaceNameOwnerById,
    }
}
