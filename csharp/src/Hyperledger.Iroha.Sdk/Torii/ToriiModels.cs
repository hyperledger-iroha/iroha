using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

public sealed record class ToriiAccountAliasLookupRequest
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("dataspace")]
    public string? Dataspace { get; init; }

    [JsonPropertyName("domain")]
    public string? Domain { get; init; }
}

public sealed record class ToriiAliasResolutionRequest
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;
}

public sealed record class ToriiIdentifierResolveRequest
{
    [JsonPropertyName("policy_id")]
    public string PolicyId { get; init; } = string.Empty;

    [JsonPropertyName("input")]
    public string? Input { get; init; }

    [JsonPropertyName("encrypted_input")]
    public string? EncryptedInput { get; init; }
}

public sealed record class ToriiAliasResolveIndexRequest
{
    [JsonPropertyName("index")]
    public ulong Index { get; init; }
}

public sealed record class ToriiAccountOnboardingRequest
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string? AccountId { get; init; }

    [JsonPropertyName("public_key_hex")]
    public string? PublicKeyHex { get; init; }

    [JsonPropertyName("identity")]
    public JsonObject? Identity { get; init; }

    [JsonPropertyName("uaid")]
    public string? Uaid { get; init; }

    [JsonPropertyName("permissions")]
    public IReadOnlyList<string> Permissions { get; init; } = Array.Empty<string>();
}

public sealed record class ToriiAccountOnboardingResponse
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("uaid")]
    public string Uaid { get; init; } = string.Empty;

    [JsonPropertyName("tx_hash_hex")]
    public string TransactionHashHex { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;
}

public sealed record class ToriiAccountFaucetRequest
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("pow_anchor_height")]
    public ulong? PowAnchorHeight { get; init; }

    [JsonPropertyName("pow_nonce_hex")]
    public string? PowNonceHex { get; init; }
}

public sealed record class ToriiAccountFaucetResponse
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("asset_id")]
    public string AssetId { get; init; } = string.Empty;

    [JsonPropertyName("amount")]
    public string Amount { get; init; } = string.Empty;

    [JsonPropertyName("tx_hash_hex")]
    public string TransactionHashHex { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;
}

public sealed record class ToriiAccountFaucetPuzzle
{
    [JsonPropertyName("algorithm")]
    public string Algorithm { get; init; } = string.Empty;

    [JsonPropertyName("difficulty_bits")]
    public byte DifficultyBits { get; init; }

    [JsonPropertyName("anchor_height")]
    public ulong AnchorHeight { get; init; }

    [JsonPropertyName("anchor_block_hash_hex")]
    public string AnchorBlockHashHex { get; init; } = string.Empty;

    [JsonPropertyName("challenge_salt_hex")]
    public string? ChallengeSaltHex { get; init; }

    [JsonPropertyName("scrypt_log_n")]
    public byte ScryptLogN { get; init; }

    [JsonPropertyName("scrypt_r")]
    public uint ScryptR { get; init; }

    [JsonPropertyName("scrypt_p")]
    public uint ScryptP { get; init; }

    [JsonPropertyName("max_anchor_age_blocks")]
    public ulong MaxAnchorAgeBlocks { get; init; }
}

public sealed record class ToriiMultisigAccountOnboardingRequest
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("required_signers")]
    public byte RequiredSigners { get; init; }

    [JsonPropertyName("member_account_ids")]
    public IReadOnlyList<string> MemberAccountIds { get; init; } = Array.Empty<string>();

    [JsonPropertyName("member_weights")]
    public IReadOnlyList<byte> MemberWeights { get; init; } = Array.Empty<byte>();

    [JsonPropertyName("transaction_ttl_ms")]
    public ulong? TransactionTtlMilliseconds { get; init; }
}

public sealed record class ToriiMultisigAccountOnboardingResponse
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("tx_hash_hex")]
    public string TransactionHashHex { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;
}

public sealed record class ToriiAccountSummary
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;
}

public sealed record class ToriiAccountsPage
{
    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAccountSummary> Items { get; init; } = Array.Empty<ToriiAccountSummary>();

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

public sealed record class ToriiAssetBalance
{
    [JsonPropertyName("asset")]
    public string Asset { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("scope")]
    public string Scope { get; init; } = string.Empty;

    [JsonPropertyName("asset_name")]
    public string AssetName { get; init; } = string.Empty;

    [JsonPropertyName("asset_alias")]
    public string? AssetAlias { get; init; }

    [JsonPropertyName("quantity")]
    public string Quantity { get; init; } = string.Empty;
}

public sealed record class ToriiAssetAliasBinding
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("lease_expiry_ms")]
    public long? LeaseExpiryMilliseconds { get; init; }

    [JsonPropertyName("grace_until_ms")]
    public long? GraceUntilMilliseconds { get; init; }

    [JsonPropertyName("bound_at_ms")]
    public long BoundAtMilliseconds { get; init; }
}

public sealed record class ToriiAssetAliasResolution
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("asset_name")]
    public string AssetName { get; init; } = string.Empty;

    [JsonPropertyName("alias_binding")]
    public ToriiAssetAliasBinding? AliasBinding { get; init; }

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("logo")]
    public string? Logo { get; init; }

    [JsonPropertyName("source")]
    public string? Source { get; init; }
}

public sealed record class ToriiAssetBalancesPage
{
    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAssetBalance> Items { get; init; } = Array.Empty<ToriiAssetBalance>();

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

public sealed record class ToriiAccountAliasResolution
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("index")]
    public long? Index { get; init; }

    [JsonPropertyName("source")]
    public string? Source { get; init; }
}

public sealed record class ToriiAccountAliasIndexResolution
{
    [JsonPropertyName("index")]
    public ulong Index { get; init; }

    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("source")]
    public string? Source { get; init; }
}

public sealed record class ToriiAccountAliasLookupItem
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("dataspace")]
    public string Dataspace { get; init; } = string.Empty;

    [JsonPropertyName("domain")]
    public string? Domain { get; init; }

    [JsonPropertyName("is_primary")]
    public bool IsPrimary { get; init; }
}

public sealed record class ToriiAccountAliasLookupResponse
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAccountAliasLookupItem> Items { get; init; } = Array.Empty<ToriiAccountAliasLookupItem>();

    [JsonPropertyName("total")]
    public long Total { get; init; }

    [JsonPropertyName("source")]
    public string? Source { get; init; }
}

public sealed record class ToriiAccountPermission
{
    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("payload")]
    public JsonNode? Payload { get; init; }
}

public sealed record class ToriiAccountPermissionsPage
{
    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAccountPermission> Items { get; init; } = Array.Empty<ToriiAccountPermission>();

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

public sealed record class ToriiTransactionSummary
{
    [JsonPropertyName("authority")]
    public string? Authority { get; init; }

    [JsonPropertyName("timestamp_ms")]
    public long? TimestampMilliseconds { get; init; }

    [JsonPropertyName("entrypoint_hash")]
    public string EntrypointHash { get; init; } = string.Empty;

    [JsonPropertyName("result_ok")]
    public bool ResultOk { get; init; }
}

public sealed record class ToriiTransactionsPage
{
    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiTransactionSummary> Items { get; init; } = Array.Empty<ToriiTransactionSummary>();

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

public sealed record class ToriiUaidPortfolioQuery
{
    public string? AssetId { get; init; }
}

public enum ToriiUaidManifestStatusFilter
{
    Active,
    Inactive,
    All,
}

public sealed record class ToriiUaidManifestQuery
{
    public long? DataspaceId { get; init; }

    public ToriiUaidManifestStatusFilter? Status { get; init; }

    public int? Limit { get; init; }

    public long Offset { get; init; }
}

public sealed record class ToriiUaidPortfolioTotals
{
    [JsonPropertyName("accounts")]
    public long Accounts { get; init; }

    [JsonPropertyName("positions")]
    public long Positions { get; init; }
}

public sealed record class ToriiUaidPortfolioAsset
{
    [JsonPropertyName("asset_id")]
    public string AssetId { get; init; } = string.Empty;

    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("quantity")]
    public string Quantity { get; init; } = string.Empty;
}

public sealed record class ToriiUaidPortfolioAccount
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("label")]
    public string? Label { get; init; }

    [JsonPropertyName("assets")]
    public IReadOnlyList<ToriiUaidPortfolioAsset> Assets { get; init; } = Array.Empty<ToriiUaidPortfolioAsset>();
}

public sealed record class ToriiUaidPortfolioDataspace
{
    [JsonPropertyName("dataspace_id")]
    public long DataspaceId { get; init; }

    [JsonPropertyName("dataspace_alias")]
    public string? DataspaceAlias { get; init; }

    [JsonPropertyName("accounts")]
    public IReadOnlyList<ToriiUaidPortfolioAccount> Accounts { get; init; } = Array.Empty<ToriiUaidPortfolioAccount>();
}

public sealed record class ToriiUaidPortfolioResponse
{
    [JsonPropertyName("uaid")]
    public string Uaid { get; init; } = string.Empty;

    [JsonPropertyName("totals")]
    public ToriiUaidPortfolioTotals Totals { get; init; } = new();

    [JsonPropertyName("dataspaces")]
    public IReadOnlyList<ToriiUaidPortfolioDataspace> Dataspaces { get; init; } = Array.Empty<ToriiUaidPortfolioDataspace>();
}

public sealed record class ToriiUaidBindingsDataspace
{
    [JsonPropertyName("dataspace_id")]
    public long DataspaceId { get; init; }

    [JsonPropertyName("dataspace_alias")]
    public string? DataspaceAlias { get; init; }

    [JsonPropertyName("accounts")]
    public IReadOnlyList<string> Accounts { get; init; } = Array.Empty<string>();
}

public sealed record class ToriiUaidBindingsResponse
{
    [JsonPropertyName("uaid")]
    public string Uaid { get; init; } = string.Empty;

    [JsonPropertyName("dataspaces")]
    public IReadOnlyList<ToriiUaidBindingsDataspace> Dataspaces { get; init; } = Array.Empty<ToriiUaidBindingsDataspace>();
}

public sealed record class ToriiUaidManifestRevocation
{
    [JsonPropertyName("epoch")]
    public long Epoch { get; init; }

    [JsonPropertyName("reason")]
    public string? Reason { get; init; }
}

public sealed record class ToriiUaidManifestLifecycle
{
    [JsonPropertyName("activated_epoch")]
    public long? ActivatedEpoch { get; init; }

    [JsonPropertyName("expired_epoch")]
    public long? ExpiredEpoch { get; init; }

    [JsonPropertyName("revocation")]
    public ToriiUaidManifestRevocation? Revocation { get; init; }
}

public sealed record class ToriiUaidManifestRecord
{
    [JsonPropertyName("dataspace_id")]
    public long DataspaceId { get; init; }

    [JsonPropertyName("dataspace_alias")]
    public string? DataspaceAlias { get; init; }

    [JsonPropertyName("manifest_hash")]
    public string ManifestHash { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("lifecycle")]
    public ToriiUaidManifestLifecycle Lifecycle { get; init; } = new();

    [JsonPropertyName("accounts")]
    public IReadOnlyList<string> Accounts { get; init; } = Array.Empty<string>();

    [JsonPropertyName("manifest")]
    public JsonNode? Manifest { get; init; }
}

public sealed record class ToriiUaidManifestsResponse
{
    [JsonPropertyName("uaid")]
    public string Uaid { get; init; } = string.Empty;

    [JsonPropertyName("total")]
    public long Total { get; init; }

    [JsonPropertyName("manifests")]
    public IReadOnlyList<ToriiUaidManifestRecord> Manifests { get; init; } = Array.Empty<ToriiUaidManifestRecord>();
}

public sealed record class ToriiExplorerAccountQrSnapshot
{
    [JsonPropertyName("canonical_id")]
    public string CanonicalId { get; init; } = string.Empty;

    [JsonPropertyName("literal")]
    public string Literal { get; init; } = string.Empty;

    [JsonPropertyName("network_prefix")]
    public int NetworkPrefix { get; init; }

    [JsonPropertyName("error_correction")]
    public string ErrorCorrection { get; init; } = string.Empty;

    [JsonPropertyName("modules")]
    public int Modules { get; init; }

    [JsonPropertyName("qr_version")]
    public int QrVersion { get; init; }

    [JsonPropertyName("svg")]
    public string Svg { get; init; } = string.Empty;
}

public sealed record class ToriiIdentifierPolicySummary
{
    [JsonPropertyName("policy_id")]
    public string PolicyId { get; init; } = string.Empty;

    [JsonPropertyName("owner")]
    public string Owner { get; init; } = string.Empty;

    [JsonPropertyName("active")]
    public bool Active { get; init; }

    [JsonPropertyName("normalization")]
    public string Normalization { get; init; } = string.Empty;

    [JsonPropertyName("resolver_public_key")]
    public string ResolverPublicKey { get; init; } = string.Empty;

    [JsonPropertyName("backend")]
    public string Backend { get; init; } = string.Empty;

    [JsonPropertyName("input_encryption")]
    public string? InputEncryption { get; init; }

    [JsonPropertyName("input_encryption_public_parameters")]
    public string? InputEncryptionPublicParameters { get; init; }

    [JsonPropertyName("input_encryption_public_parameters_decoded")]
    public JsonNode? InputEncryptionPublicParametersDecoded { get; init; }

    [JsonPropertyName("ram_fhe_profile")]
    public JsonNode? RamFheProfile { get; init; }

    [JsonPropertyName("note")]
    public string? Note { get; init; }
}

public sealed record class ToriiIdentifierPoliciesResponse
{
    [JsonPropertyName("total")]
    public long Total { get; init; }

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiIdentifierPolicySummary> Items { get; init; } = Array.Empty<ToriiIdentifierPolicySummary>();
}

public sealed record class ToriiIdentifierResolveResponse
{
    [JsonPropertyName("policy_id")]
    public string PolicyId { get; init; } = string.Empty;

    [JsonPropertyName("opaque_id")]
    public string OpaqueId { get; init; } = string.Empty;

    [JsonPropertyName("receipt_hash")]
    public string ReceiptHash { get; init; } = string.Empty;

    [JsonPropertyName("uaid")]
    public string Uaid { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("resolved_at_ms")]
    public long ResolvedAtMilliseconds { get; init; }

    [JsonPropertyName("expires_at_ms")]
    public long? ExpiresAtMilliseconds { get; init; }

    [JsonPropertyName("backend")]
    public string Backend { get; init; } = string.Empty;

    [JsonPropertyName("signature")]
    public string Signature { get; init; } = string.Empty;

    [JsonPropertyName("signature_payload_hex")]
    public string SignaturePayloadHex { get; init; } = string.Empty;

    [JsonPropertyName("signature_payload")]
    public JsonNode? SignaturePayload { get; init; }
}

public sealed record class ToriiContractAliasBinding
{
    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("lease_expiry_ms")]
    public long? LeaseExpiryMilliseconds { get; init; }

    [JsonPropertyName("grace_until_ms")]
    public long? GraceUntilMilliseconds { get; init; }

    [JsonPropertyName("bound_at_ms")]
    public long BoundAtMilliseconds { get; init; }
}

public sealed record class ToriiContractAliasResolutionRequest
{
    [JsonPropertyName("contract_alias")]
    public string ContractAlias { get; init; } = string.Empty;
}

public sealed record class ToriiContractAliasResolution
{
    [JsonPropertyName("contract_alias")]
    public string ContractAlias { get; init; } = string.Empty;

    [JsonPropertyName("contract_address")]
    public string ContractAddress { get; init; } = string.Empty;

    [JsonPropertyName("dataspace")]
    public string Dataspace { get; init; } = string.Empty;

    [JsonPropertyName("contract_alias_binding")]
    public ToriiContractAliasBinding? ContractAliasBinding { get; init; }

    [JsonPropertyName("source")]
    public string? Source { get; init; }
}

public sealed record class ToriiNodeCapabilities
{
    [JsonPropertyName("abi_version")]
    public int AbiVersion { get; init; }

    [JsonPropertyName("data_model_version")]
    public int DataModelVersion { get; init; }

    [JsonPropertyName("crypto")]
    public ToriiNodeCryptoCapabilities Crypto { get; init; } = new();
}

public sealed record class ToriiNodeCryptoCapabilities
{
    [JsonPropertyName("sm")]
    public ToriiNodeSmCapabilities Sm { get; init; } = new();

    [JsonPropertyName("curves")]
    public ToriiNodeCurveCapabilities Curves { get; init; } = new();
}

public sealed record class ToriiNodeSmCapabilities
{
    [JsonPropertyName("enabled")]
    public bool Enabled { get; init; }

    [JsonPropertyName("default_hash")]
    public string DefaultHash { get; init; } = string.Empty;

    [JsonPropertyName("allowed_signing")]
    public IReadOnlyList<string> AllowedSigning { get; init; } = Array.Empty<string>();

    [JsonPropertyName("sm2_distid_default")]
    public string Sm2DistidDefault { get; init; } = string.Empty;

    [JsonPropertyName("openssl_preview")]
    public bool OpensslPreview { get; init; }

    [JsonPropertyName("acceleration")]
    public ToriiNodeSmAcceleration Acceleration { get; init; } = new();
}

public sealed record class ToriiNodeSmAcceleration
{
    [JsonPropertyName("scalar")]
    public bool Scalar { get; init; }

    [JsonPropertyName("neon_sm3")]
    public bool NeonSm3 { get; init; }

    [JsonPropertyName("neon_sm4")]
    public bool NeonSm4 { get; init; }

    [JsonPropertyName("policy")]
    public string Policy { get; init; } = string.Empty;
}

public sealed record class ToriiNodeCurveCapabilities
{
    [JsonPropertyName("registry_version")]
    public int RegistryVersion { get; init; }

    [JsonPropertyName("allowed_curve_ids")]
    public IReadOnlyList<int> AllowedCurveIds { get; init; } = Array.Empty<int>();

    [JsonPropertyName("allowed_curve_bitmap")]
    public IReadOnlyList<ulong> AllowedCurveBitmap { get; init; } = Array.Empty<ulong>();
}

public sealed record class ToriiRuntimeUpgradeCounters
{
    [JsonPropertyName("proposed")]
    public long Proposed { get; init; }

    [JsonPropertyName("activated")]
    public long Activated { get; init; }

    [JsonPropertyName("canceled")]
    public long Canceled { get; init; }
}

public sealed record class ToriiRuntimeMetrics
{
    [JsonPropertyName("abi_version")]
    public int AbiVersion { get; init; }

    [JsonPropertyName("upgrade_events_total")]
    public ToriiRuntimeUpgradeCounters UpgradeEventsTotal { get; init; } = new();
}

public sealed record class ToriiRuntimeAbiActive
{
    [JsonPropertyName("abi_version")]
    public int AbiVersion { get; init; }
}

public sealed record class ToriiRuntimeAbiHash
{
    [JsonPropertyName("policy")]
    public string Policy { get; init; } = string.Empty;

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex { get; init; } = string.Empty;
}
