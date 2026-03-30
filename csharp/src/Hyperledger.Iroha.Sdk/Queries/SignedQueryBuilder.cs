using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Queries;

public sealed class SignedQueryBuilder
{
    private const byte SignedQueryVersion = 1;

    private ManagedSingularQueryKind? singularQueryKind;
    private string? subjectAccountId;
    private string? dataspaceAlias;
    private string? domain;
    private string? assetDefinitionId;
    private string? codeHash;
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

    public SignedQueryBuilder(string authorityAccountId)
    {
        AuthorityAccountId = NormalizeRequiredValue(authorityAccountId, nameof(authorityAccountId));
    }

    public string AuthorityAccountId { get; }

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
        subjectAccountId = NormalizeRequiredValue(accountId, nameof(accountId));
        dataspaceAlias = NormalizeOptionalValue(dataspace);
        this.domain = NormalizeOptionalValue(domain);
        return this;
    }

    public SignedQueryBuilder FindAssetById(string assetDefinitionId, string accountId, ulong? dataspaceId = null)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindAssetById;
        this.assetDefinitionId = NormalizeRequiredValue(assetDefinitionId, nameof(assetDefinitionId));
        subjectAccountId = NormalizeRequiredValue(accountId, nameof(accountId));
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

    public SignedQueryBuilder FindContractManifestByCodeHash(string codeHash)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindContractManifestByCodeHash;
        this.codeHash = NormalizeRequiredValue(codeHash, nameof(codeHash));
        return this;
    }

    public SignedQueryBuilder FindProofRecordById(string backend, string proofHash)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindProofRecordById;
        proofBackend = NormalizeRequiredValue(backend, nameof(backend));
        this.proofHash = NormalizeRequiredValue(proofHash, nameof(proofHash));
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
        domain = NormalizeRequiredValue(domainId, nameof(domainId));
        return this;
    }

    public SignedQueryBuilder FindDomainEndorsementPolicy(string domainId)
    {
        ResetArguments();
        singularQueryKind = ManagedSingularQueryKind.FindDomainEndorsementPolicy;
        domain = NormalizeRequiredValue(domainId, nameof(domainId));
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
        if (!singularQueryKind.HasValue)
        {
            throw new InvalidOperationException("Queries must select a singular request before signing.");
        }

        var context = new TransactionEncodingContext(AuthorityAccountId);
        context.EnsureAuthorityMatchesPrivateKey(privateKeySeed);

        var payloadBytes = EncodeQueryRequestWithAuthority(context);
        var payloadHash = IrohaHash.Hash(payloadBytes);
        var signatureBytes = Ed25519Signer.Sign(payloadHash, privateKeySeed);

        var signedQuery = new OfflineNoritoWriter();
        signedQuery.WriteField(context.EncodeConstVec(signatureBytes));
        signedQuery.WriteField(payloadBytes);
        var signedQueryBytes = signedQuery.ToArray();

        var versionedNoritoBytes = new byte[signedQueryBytes.Length + 1];
        versionedNoritoBytes[0] = SignedQueryVersion;
        signedQueryBytes.CopyTo(versionedNoritoBytes.AsSpan(1));

        return new SignedQueryEnvelope(versionedNoritoBytes, signedQueryBytes, payloadBytes, signatureBytes);
    }

    private byte[] EncodeQueryRequestWithAuthority(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeAccountId(AuthorityAccountId));
        writer.WriteField(EncodeQueryRequest(context));
        return writer.ToArray();
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
            ManagedSingularQueryKind.FindAliasesByAccountId => EncodeEnumVariant(2, EncodeFindAliasesByAccountId(context)),
            ManagedSingularQueryKind.FindProofRecordById => EncodeEnumVariant(3, EncodeFindProofRecordById(context)),
            ManagedSingularQueryKind.FindContractManifestByCodeHash => EncodeEnumVariant(4, EncodeFindContractManifestByCodeHash(context)),
            ManagedSingularQueryKind.FindAbiVersion => EncodeEnumVariant(5, Array.Empty<byte>()),
            ManagedSingularQueryKind.FindAssetById => EncodeEnumVariant(6, EncodeFindAssetById(context)),
            ManagedSingularQueryKind.FindAssetDefinitionById => EncodeEnumVariant(7, EncodeFindAssetDefinitionById(context)),
            ManagedSingularQueryKind.FindTwitterBindingByHash => EncodeEnumVariant(8, EncodeFindTwitterBindingByHash(context)),
            ManagedSingularQueryKind.FindDomainEndorsements => EncodeEnumVariant(9, EncodeFindDomainId(context)),
            ManagedSingularQueryKind.FindDomainEndorsementPolicy => EncodeEnumVariant(10, EncodeFindDomainId(context)),
            ManagedSingularQueryKind.FindDomainCommittee => EncodeEnumVariant(11, EncodeFindDomainCommittee(context)),
            ManagedSingularQueryKind.FindDaPinIntentByTicket => EncodeEnumVariant(12, EncodeFindDaPinIntentByTicket(context)),
            ManagedSingularQueryKind.FindDaPinIntentByManifest => EncodeEnumVariant(13, EncodeFindDaPinIntentByManifest(context)),
            ManagedSingularQueryKind.FindDaPinIntentByAlias => EncodeEnumVariant(14, EncodeFindDaPinIntentByAlias(context)),
            ManagedSingularQueryKind.FindDaPinIntentByLaneEpochSequence => EncodeEnumVariant(15, EncodeFindDaPinIntentByLaneEpochSequence(context)),
            ManagedSingularQueryKind.FindSorafsProviderOwner => EncodeEnumVariant(16, EncodeFindSorafsProviderOwner(context)),
            ManagedSingularQueryKind.FindDataspaceNameOwnerById => EncodeEnumVariant(17, EncodeFindDataspaceNameOwnerById(context)),
            _ => throw new InvalidOperationException("Unsupported managed singular query kind."),
        };
    }

    private byte[] EncodeFindAliasesByAccountId(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeAccountId(subjectAccountId!));
        writer.WriteField(context.EncodeOptionalString(dataspaceAlias));
        writer.WriteField(context.EncodeOptionalString(domain));
        return writer.ToArray();
    }

    private byte[] EncodeFindAssetById(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeAssetId(assetDefinitionId!, subjectAccountId!, dataspaceId));
        return writer.ToArray();
    }

    private byte[] EncodeFindAssetDefinitionById(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeAssetDefinitionId(assetDefinitionId!));
        return writer.ToArray();
    }

    private byte[] EncodeFindContractManifestByCodeHash(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeHashLiteral(codeHash!));
        return writer.ToArray();
    }

    private byte[] EncodeFindProofRecordById(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeString(proofBackend!));
        writer.WriteField(context.EncodeFixedBytesLiteral(proofHash!, expectedLength: 32));
        return writer.ToArray();
    }

    private byte[] EncodeFindTwitterBindingByHash(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeString(twitterPepperId!));
        writer.WriteField(context.EncodeHashLiteral(twitterBindingDigest!));
        return writer.ToArray();
    }

    private byte[] EncodeFindDomainId(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeName(domain!));
        return writer.ToArray();
    }

    private byte[] EncodeFindDomainCommittee(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeString(committeeId!));
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByTicket(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeFixedBytesLiteral(storageTicket!, expectedLength: 32));
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByManifest(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeFixedBytesLiteral(manifestDigest!, expectedLength: 32));
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByAlias(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeString(pinAlias!));
        return writer.ToArray();
    }

    private byte[] EncodeFindDaPinIntentByLaneEpochSequence(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeUInt32(laneId!.Value));
        writer.WriteField(context.EncodeUInt64(pinEpoch!.Value));
        writer.WriteField(context.EncodeUInt64(pinSequence!.Value));
        return writer.ToArray();
    }

    private byte[] EncodeFindSorafsProviderOwner(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeFixedBytesLiteral(providerId!, expectedLength: 32));
        return writer.ToArray();
    }

    private byte[] EncodeFindDataspaceNameOwnerById(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeUInt64(dataspaceOwnerId!.Value));
        return writer.ToArray();
    }

    private static byte[] EncodeEnumVariant(uint discriminant, params byte[][] fields)
    {
        var writer = new OfflineNoritoWriter();
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
        codeHash = null;
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
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        return value.Trim();
    }

    private static string? NormalizeOptionalValue(string? value)
    {
        return string.IsNullOrWhiteSpace(value) ? null : value.Trim();
    }

    private enum ManagedSingularQueryKind
    {
        FindExecutorDataModel,
        FindParameters,
        FindAliasesByAccountId,
        FindProofRecordById,
        FindContractManifestByCodeHash,
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
