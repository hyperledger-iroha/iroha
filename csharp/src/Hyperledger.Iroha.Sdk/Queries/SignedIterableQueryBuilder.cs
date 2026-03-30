using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Queries;

public sealed class SignedIterableQueryBuilder
{
    private const byte SignedQueryVersion = 1;
    private const ulong MaxFetchSize = 10_000;

    private RequestMode? requestMode;
    private ManagedIterableQueryKind? iterableQueryKind;
    private string? accountId;
    private string? assetDefinitionId;
    private string? certificateId;
    private string? offlineTransferId;
    private string? continueQueryId;
    private ulong continueCursor;
    private ulong? continueGasBudget;
    private ulong? limit;
    private ulong offset;
    private ulong? fetchSize;
    private string? sortByMetadataKey;
    private bool descendingSort;

    public SignedIterableQueryBuilder(string authorityAccountId)
    {
        AuthorityAccountId = NormalizeRequiredValue(authorityAccountId, nameof(authorityAccountId));
    }

    public string AuthorityAccountId { get; }

    public SignedIterableQueryBuilder FindDomains()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindDomains;
        return this;
    }

    public SignedIterableQueryBuilder FindAccounts()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindAccounts;
        return this;
    }

    public SignedIterableQueryBuilder FindAssets()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindAssets;
        return this;
    }

    public SignedIterableQueryBuilder FindAssetDefinitions()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindAssetDefinitions;
        return this;
    }

    public SignedIterableQueryBuilder FindRepoAgreements()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindRepoAgreements;
        return this;
    }

    public SignedIterableQueryBuilder FindNfts()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindNfts;
        return this;
    }

    public SignedIterableQueryBuilder FindRwas()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindRwas;
        return this;
    }

    public SignedIterableQueryBuilder FindTransactions()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindTransactions;
        return this;
    }

    public SignedIterableQueryBuilder FindRoles()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindRoles;
        return this;
    }

    public SignedIterableQueryBuilder FindRoleIds()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindRoleIds;
        return this;
    }

    public SignedIterableQueryBuilder FindPeers()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindPeers;
        return this;
    }

    public SignedIterableQueryBuilder FindActiveTriggerIds()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindActiveTriggerIds;
        return this;
    }

    public SignedIterableQueryBuilder FindTriggers()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindTriggers;
        return this;
    }

    public SignedIterableQueryBuilder FindAccountsWithAsset(string assetDefinitionId)
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindAccountsWithAsset;
        this.assetDefinitionId = NormalizeRequiredValue(assetDefinitionId, nameof(assetDefinitionId));
        return this;
    }

    public SignedIterableQueryBuilder FindPermissionsByAccountId(string accountId)
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindPermissionsByAccountId;
        this.accountId = NormalizeRequiredValue(accountId, nameof(accountId));
        return this;
    }

    public SignedIterableQueryBuilder FindRolesByAccountId(string accountId)
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindRolesByAccountId;
        this.accountId = NormalizeRequiredValue(accountId, nameof(accountId));
        return this;
    }

    public SignedIterableQueryBuilder FindBlocks()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindBlocks;
        return this;
    }

    public SignedIterableQueryBuilder FindBlockHeaders()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindBlockHeaders;
        return this;
    }

    public SignedIterableQueryBuilder FindProofRecords()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindProofRecords;
        return this;
    }

    public SignedIterableQueryBuilder FindOfflineAllowances()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindOfflineAllowances;
        return this;
    }

    public SignedIterableQueryBuilder FindOfflineAllowanceByCertificateId(string certificateId)
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindOfflineAllowanceByCertificateId;
        this.certificateId = NormalizeRequiredValue(certificateId, nameof(certificateId));
        return this;
    }

    public SignedIterableQueryBuilder FindOfflineToOnlineTransfers()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindOfflineToOnlineTransfers;
        return this;
    }

    public SignedIterableQueryBuilder FindOfflineToOnlineTransferById(string bundleId)
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindOfflineToOnlineTransferById;
        offlineTransferId = NormalizeRequiredValue(bundleId, nameof(bundleId));
        return this;
    }

    public SignedIterableQueryBuilder FindOfflineCounterSummaries()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindOfflineCounterSummaries;
        return this;
    }

    public SignedIterableQueryBuilder FindOfflineVerdictRevocations()
    {
        Reset();
        requestMode = RequestMode.Start;
        iterableQueryKind = ManagedIterableQueryKind.FindOfflineVerdictRevocations;
        return this;
    }

    public SignedIterableQueryBuilder Continue(string queryId, ulong cursor, ulong? gasBudget = null)
    {
        if (cursor == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(cursor), "Query cursor must be non-zero.");
        }

        Reset();
        requestMode = RequestMode.Continue;
        continueQueryId = NormalizeRequiredValue(queryId, nameof(queryId));
        continueCursor = cursor;
        continueGasBudget = gasBudget;
        return this;
    }

    public SignedIterableQueryBuilder SetLimit(ulong? limit)
    {
        if (limit == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(limit), "Query limit must be positive when provided.");
        }

        this.limit = limit;
        return this;
    }

    public SignedIterableQueryBuilder SetOffset(ulong offset)
    {
        this.offset = offset;
        return this;
    }

    public SignedIterableQueryBuilder SetFetchSize(ulong? fetchSize)
    {
        if (fetchSize == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(fetchSize), "Query fetch size must be positive when provided.");
        }

        if (fetchSize > MaxFetchSize)
        {
            throw new ArgumentOutOfRangeException(nameof(fetchSize), $"Query fetch size cannot exceed {MaxFetchSize}.");
        }

        this.fetchSize = fetchSize;
        return this;
    }

    public SignedIterableQueryBuilder SortByMetadata(string key, bool descending = false)
    {
        sortByMetadataKey = NormalizeRequiredValue(key, nameof(key));
        descendingSort = descending;
        return this;
    }

    public SignedIterableQueryBuilder ClearSorting()
    {
        sortByMetadataKey = null;
        descendingSort = false;
        return this;
    }

    public SignedQueryEnvelope BuildSigned(ReadOnlySpan<byte> privateKeySeed)
    {
        if (!requestMode.HasValue)
        {
            throw new InvalidOperationException("Iterable queries must select a start or continue request before signing.");
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
        return requestMode switch
        {
            RequestMode.Start => EncodeEnumVariant(1, EncodeQueryWithParams(context)),
            RequestMode.Continue => EncodeEnumVariant(2, EncodeForwardCursor(context)),
            _ => throw new InvalidOperationException("Unsupported iterable query request mode."),
        };
    }

    private byte[] EncodeQueryWithParams(TransactionEncodingContext context)
    {
        if (!iterableQueryKind.HasValue)
        {
            throw new InvalidOperationException("Iterable start requests must select a query before signing.");
        }

        var writer = new OfflineNoritoWriter();
        writer.WriteField(Array.Empty<byte>());
        writer.WriteField(EncodeByteVector(EncodeIterableQueryPayload(context)));
        writer.WriteField(EncodeFieldlessEnumVariant(GetQueryItemKindDiscriminant(iterableQueryKind.Value)));
        writer.WriteField(EncodeByteVector(EncodeFieldlessEnumVariant(0)));
        writer.WriteField(EncodeByteVector(Array.Empty<byte>()));
        writer.WriteField(EncodeQueryParams(context));
        return writer.ToArray();
    }

    private byte[] EncodeIterableQueryPayload(TransactionEncodingContext context)
    {
        return iterableQueryKind switch
        {
            ManagedIterableQueryKind.FindDomains => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindAccounts => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindAssets => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindAssetDefinitions => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindRepoAgreements => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindNfts => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindRwas => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindTransactions => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindRoles => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindRoleIds => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindPeers => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindActiveTriggerIds => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindTriggers => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindAccountsWithAsset => EncodeSingleFieldStruct(context.EncodeAssetDefinitionId(assetDefinitionId!)),
            ManagedIterableQueryKind.FindPermissionsByAccountId => EncodeSingleFieldStruct(context.EncodeAccountId(accountId!)),
            ManagedIterableQueryKind.FindRolesByAccountId => EncodeSingleFieldStruct(context.EncodeAccountId(accountId!)),
            ManagedIterableQueryKind.FindBlocks => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindBlockHeaders => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindProofRecords => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindOfflineAllowances => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindOfflineAllowanceByCertificateId => EncodeSingleFieldStruct(context.EncodeHashLiteral(certificateId!)),
            ManagedIterableQueryKind.FindOfflineToOnlineTransfers => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindOfflineToOnlineTransferById => EncodeSingleFieldStruct(context.EncodeHashLiteral(offlineTransferId!)),
            ManagedIterableQueryKind.FindOfflineCounterSummaries => Array.Empty<byte>(),
            ManagedIterableQueryKind.FindOfflineVerdictRevocations => Array.Empty<byte>(),
            _ => throw new InvalidOperationException("Unsupported iterable query kind."),
        };
    }

    private byte[] EncodeQueryParams(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(EncodePagination(context));
        writer.WriteField(EncodeSorting(context));
        writer.WriteField(EncodeFetchSize(context));
        return writer.ToArray();
    }

    private byte[] EncodePagination(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeOption(limit, context.EncodeUInt64));
        writer.WriteField(context.EncodeUInt64(offset));
        return writer.ToArray();
    }

    private byte[] EncodeSorting(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeOptionalString(sortByMetadataKey));
        writer.WriteField(sortByMetadataKey is null || !descendingSort
            ? new byte[] { 0 }
            : context.EncodeOption<bool>(true, static _ => EncodeFieldlessEnumVariant(1)));
        return writer.ToArray();
    }

    private byte[] EncodeFetchSize(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeOption(fetchSize, context.EncodeUInt64));
        return writer.ToArray();
    }

    private byte[] EncodeForwardCursor(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeString(continueQueryId!));
        writer.WriteField(context.EncodeUInt64(continueCursor));
        writer.WriteField(context.EncodeOption(continueGasBudget, context.EncodeUInt64));
        return writer.ToArray();
    }

    private void Reset()
    {
        requestMode = null;
        iterableQueryKind = null;
        accountId = null;
        assetDefinitionId = null;
        certificateId = null;
        offlineTransferId = null;
        continueQueryId = null;
        continueCursor = 0;
        continueGasBudget = null;
        limit = null;
        offset = 0;
        fetchSize = null;
        sortByMetadataKey = null;
        descendingSort = false;
    }

    private static string NormalizeRequiredValue(string value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        return value.Trim();
    }

    private static byte[] EncodeSingleFieldStruct(byte[] field)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(field);
        return writer.ToArray();
    }

    private static byte[] EncodeFieldlessEnumVariant(uint discriminant)
    {
        return EncodeEnumVariant(discriminant, Array.Empty<byte>());
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

    private static byte[] EncodeByteVector(ReadOnlySpan<byte> bytes)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteLength((ulong)bytes.Length);
        writer.WriteBytes(bytes);
        return writer.ToArray();
    }

    private static uint GetQueryItemKindDiscriminant(ManagedIterableQueryKind kind)
    {
        return kind switch
        {
            ManagedIterableQueryKind.FindDomains => 0,
            ManagedIterableQueryKind.FindAccounts => 1,
            ManagedIterableQueryKind.FindAssets => 2,
            ManagedIterableQueryKind.FindAssetDefinitions => 3,
            ManagedIterableQueryKind.FindRepoAgreements => 4,
            ManagedIterableQueryKind.FindNfts => 5,
            ManagedIterableQueryKind.FindRwas => 6,
            ManagedIterableQueryKind.FindRoles => 7,
            ManagedIterableQueryKind.FindRoleIds => 8,
            ManagedIterableQueryKind.FindPeers => 9,
            ManagedIterableQueryKind.FindActiveTriggerIds => 10,
            ManagedIterableQueryKind.FindTriggers => 11,
            ManagedIterableQueryKind.FindTransactions => 12,
            ManagedIterableQueryKind.FindBlocks => 13,
            ManagedIterableQueryKind.FindBlockHeaders => 14,
            ManagedIterableQueryKind.FindProofRecords => 15,
            ManagedIterableQueryKind.FindPermissionsByAccountId => 16,
            ManagedIterableQueryKind.FindOfflineAllowances => 17,
            ManagedIterableQueryKind.FindOfflineAllowanceByCertificateId => 17,
            ManagedIterableQueryKind.FindOfflineToOnlineTransfers => 18,
            ManagedIterableQueryKind.FindOfflineToOnlineTransferById => 18,
            ManagedIterableQueryKind.FindOfflineCounterSummaries => 19,
            ManagedIterableQueryKind.FindOfflineVerdictRevocations => 20,
            ManagedIterableQueryKind.FindAccountsWithAsset => 1,
            ManagedIterableQueryKind.FindRolesByAccountId => 8,
            _ => throw new InvalidOperationException("Unsupported iterable query item kind."),
        };
    }

    private enum RequestMode
    {
        Start,
        Continue,
    }

    private enum ManagedIterableQueryKind
    {
        FindDomains,
        FindAccounts,
        FindAssets,
        FindAssetDefinitions,
        FindRepoAgreements,
        FindNfts,
        FindRwas,
        FindTransactions,
        FindRoles,
        FindRoleIds,
        FindPeers,
        FindActiveTriggerIds,
        FindTriggers,
        FindAccountsWithAsset,
        FindPermissionsByAccountId,
        FindRolesByAccountId,
        FindBlocks,
        FindBlockHeaders,
        FindProofRecords,
        FindOfflineAllowances,
        FindOfflineAllowanceByCertificateId,
        FindOfflineToOnlineTransfers,
        FindOfflineToOnlineTransferById,
        FindOfflineCounterSummaries,
        FindOfflineVerdictRevocations,
    }
}
