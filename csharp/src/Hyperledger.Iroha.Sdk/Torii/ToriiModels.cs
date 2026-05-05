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

public sealed record class ToriiVpnProfile
{
    [JsonPropertyName("available")]
    public bool Available { get; init; }

    [JsonPropertyName("relay_endpoint")]
    public string RelayEndpoint { get; init; } = string.Empty;

    [JsonPropertyName("supported_exit_classes")]
    public IReadOnlyList<string> SupportedExitClasses { get; init; } = Array.Empty<string>();

    [JsonPropertyName("default_exit_class")]
    public string DefaultExitClass { get; init; } = string.Empty;

    [JsonPropertyName("lease_secs")]
    public ulong LeaseSeconds { get; init; }

    [JsonPropertyName("dns_push_interval_secs")]
    public ulong DnsPushIntervalSeconds { get; init; }

    [JsonPropertyName("meter_family")]
    public string MeterFamily { get; init; } = string.Empty;

    [JsonPropertyName("route_pushes")]
    public IReadOnlyList<string> RoutePushes { get; init; } = Array.Empty<string>();

    [JsonPropertyName("dns_servers")]
    public IReadOnlyList<string> DnsServers { get; init; } = Array.Empty<string>();

    [JsonPropertyName("display_billing_label")]
    public string DisplayBillingLabel { get; init; } = string.Empty;
}

public sealed record class ToriiVpnSessionCreateRequest
{
    [JsonPropertyName("exit_class")]
    public string ExitClass { get; init; } = string.Empty;
}

public sealed record class ToriiVpnSession
{
    [JsonPropertyName("session_id")]
    public string SessionId { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("exit_class")]
    public string ExitClass { get; init; } = string.Empty;

    [JsonPropertyName("relay_endpoint")]
    public string RelayEndpoint { get; init; } = string.Empty;

    [JsonPropertyName("lease_secs")]
    public ulong LeaseSeconds { get; init; }

    [JsonPropertyName("expires_at_ms")]
    public ulong ExpiresAtMilliseconds { get; init; }

    [JsonPropertyName("meter_family")]
    public string MeterFamily { get; init; } = string.Empty;

    [JsonPropertyName("route_pushes")]
    public IReadOnlyList<string> RoutePushes { get; init; } = Array.Empty<string>();

    [JsonPropertyName("dns_servers")]
    public IReadOnlyList<string> DnsServers { get; init; } = Array.Empty<string>();

    [JsonPropertyName("helper_ticket_hex")]
    public string HelperTicketHex { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;
}

public sealed record class ToriiVpnSessionDeleteResponse
{
    [JsonPropertyName("session_id")]
    public string SessionId { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("disconnected_at_ms")]
    public ulong DisconnectedAtMilliseconds { get; init; }
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

public record class ToriiExplorerPaginationQuery
{
    public ulong? Page { get; init; }

    public ulong? PerPage { get; init; }
}

public sealed record class ToriiContractInstancesQuery
{
    public string? Contains { get; init; }

    public string? HashPrefix { get; init; }

    public ulong? Offset { get; init; }

    public ulong? Limit { get; init; }

    public string? Order { get; init; }
}

public sealed record class ToriiContractStateQuery
{
    public string? Path { get; init; }

    public IReadOnlyList<string>? Paths { get; init; }

    public string? Prefix { get; init; }

    public bool? IncludeValue { get; init; }

    public ulong? Offset { get; init; }

    public ulong? Limit { get; init; }

    public string? Decode { get; init; }
}

public sealed record class ToriiExplorerAccountsQuery : ToriiExplorerPaginationQuery
{
    public string? Domain { get; init; }

    public string? WithAsset { get; init; }
}

public sealed record class ToriiExplorerDomainsQuery : ToriiExplorerPaginationQuery
{
    public string? OwnedBy { get; init; }
}

public sealed record class ToriiExplorerAssetDefinitionsQuery : ToriiExplorerPaginationQuery
{
    public string? Domain { get; init; }

    public string? OwnedBy { get; init; }
}

public sealed record class ToriiExplorerAssetsQuery : ToriiExplorerPaginationQuery
{
    public string? OwnedBy { get; init; }

    public string? Definition { get; init; }

    public string? AssetId { get; init; }
}

public sealed record class ToriiExplorerNftsQuery : ToriiExplorerPaginationQuery
{
    public string? OwnedBy { get; init; }

    public string? Domain { get; init; }
}

public sealed record class ToriiExplorerRwasQuery : ToriiExplorerPaginationQuery
{
    public string? OwnedBy { get; init; }

    public string? Domain { get; init; }
}

public enum ToriiExplorerTransactionStatusFilter
{
    Committed,
    Rejected,
}

public sealed record class ToriiExplorerTransactionsQuery : ToriiExplorerPaginationQuery
{
    public string? Authority { get; init; }

    public ulong? Block { get; init; }

    public ToriiExplorerTransactionStatusFilter? Status { get; init; }

    public string? AssetId { get; init; }
}

public sealed record class ToriiExplorerInstructionsQuery : ToriiExplorerPaginationQuery
{
    public string? Authority { get; init; }

    public string? Account { get; init; }

    public string? TransactionHash { get; init; }

    public ToriiExplorerTransactionStatusFilter? TransactionStatus { get; init; }

    public ulong? Block { get; init; }

    public string? Kind { get; init; }

    public string? AssetId { get; init; }
}

public sealed record class ToriiExplorerPaginationMeta
{
    [JsonPropertyName("page")]
    public ulong Page { get; init; }

    [JsonPropertyName("per_page")]
    public ulong PerPage { get; init; }

    [JsonPropertyName("total_pages")]
    public ulong TotalPages { get; init; }

    [JsonPropertyName("total_items")]
    public ulong TotalItems { get; init; }
}

public sealed record class ToriiExplorerAccount
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("i105_address")]
    public string I105Address { get; init; } = string.Empty;

    [JsonPropertyName("network_prefix")]
    public ushort NetworkPrefix { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata { get; init; } = new JsonObject();

    [JsonPropertyName("owned_domains")]
    public uint OwnedDomains { get; init; }

    [JsonPropertyName("owned_assets")]
    public uint OwnedAssets { get; init; }

    [JsonPropertyName("owned_nfts")]
    public uint OwnedNfts { get; init; }
}

public sealed record class ToriiExplorerAccountsPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerAccount> Items { get; init; } = Array.Empty<ToriiExplorerAccount>();
}

public sealed record class ToriiExplorerDomain
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("logo")]
    public string? Logo { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata { get; init; } = new JsonObject();

    [JsonPropertyName("owned_by")]
    public string OwnedBy { get; init; } = string.Empty;

    [JsonPropertyName("accounts")]
    public uint Accounts { get; init; }

    [JsonPropertyName("assets")]
    public uint Assets { get; init; }

    [JsonPropertyName("nfts")]
    public uint Nfts { get; init; }
}

public sealed record class ToriiExplorerDomainsPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerDomain> Items { get; init; } = Array.Empty<ToriiExplorerDomain>();
}

public sealed record class ToriiExplorerAssetDefinition
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("mintable")]
    public string Mintable { get; init; } = string.Empty;

    [JsonPropertyName("logo")]
    public string? Logo { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata { get; init; } = new JsonObject();

    [JsonPropertyName("owned_by")]
    public string OwnedBy { get; init; } = string.Empty;

    [JsonPropertyName("assets")]
    public uint Assets { get; init; }

    [JsonPropertyName("total_quantity")]
    public string TotalQuantity { get; init; } = string.Empty;

    [JsonPropertyName("locked_quantity")]
    public string? LockedQuantity { get; init; }

    [JsonPropertyName("circulating_quantity")]
    public string? CirculatingQuantity { get; init; }
}

public sealed record class ToriiExplorerAssetDefinitionsPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerAssetDefinition> Items { get; init; } = Array.Empty<ToriiExplorerAssetDefinition>();
}

public sealed record class ToriiExplorerEconometricsVelocityWindow
{
    [JsonPropertyName("key")]
    public string Key { get; init; } = string.Empty;

    [JsonPropertyName("start_ms")]
    public ulong StartMilliseconds { get; init; }

    [JsonPropertyName("end_ms")]
    public ulong EndMilliseconds { get; init; }

    [JsonPropertyName("transfers")]
    public ulong Transfers { get; init; }

    [JsonPropertyName("unique_senders")]
    public ulong UniqueSenders { get; init; }

    [JsonPropertyName("unique_receivers")]
    public ulong UniqueReceivers { get; init; }

    [JsonPropertyName("amount")]
    public string Amount { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerEconometricsIssuanceWindow
{
    [JsonPropertyName("key")]
    public string Key { get; init; } = string.Empty;

    [JsonPropertyName("start_ms")]
    public ulong StartMilliseconds { get; init; }

    [JsonPropertyName("end_ms")]
    public ulong EndMilliseconds { get; init; }

    [JsonPropertyName("mint_count")]
    public ulong MintCount { get; init; }

    [JsonPropertyName("burn_count")]
    public ulong BurnCount { get; init; }

    [JsonPropertyName("minted")]
    public string Minted { get; init; } = string.Empty;

    [JsonPropertyName("burned")]
    public string Burned { get; init; } = string.Empty;

    [JsonPropertyName("net")]
    public string Net { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerEconometricsIssuanceSeriesPoint
{
    [JsonPropertyName("bucket_start_ms")]
    public ulong BucketStartMilliseconds { get; init; }

    [JsonPropertyName("minted")]
    public string Minted { get; init; } = string.Empty;

    [JsonPropertyName("burned")]
    public string Burned { get; init; } = string.Empty;

    [JsonPropertyName("net")]
    public string Net { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerAssetDefinitionEconometrics
{
    [JsonPropertyName("definition_id")]
    public string DefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("computed_at_ms")]
    public ulong ComputedAtMilliseconds { get; init; }

    [JsonPropertyName("velocity_windows")]
    public IReadOnlyList<ToriiExplorerEconometricsVelocityWindow> VelocityWindows { get; init; } = Array.Empty<ToriiExplorerEconometricsVelocityWindow>();

    [JsonPropertyName("issuance_windows")]
    public IReadOnlyList<ToriiExplorerEconometricsIssuanceWindow> IssuanceWindows { get; init; } = Array.Empty<ToriiExplorerEconometricsIssuanceWindow>();

    [JsonPropertyName("issuance_series")]
    public IReadOnlyList<ToriiExplorerEconometricsIssuanceSeriesPoint> IssuanceSeries { get; init; } = Array.Empty<ToriiExplorerEconometricsIssuanceSeriesPoint>();
}

public sealed record class ToriiExplorerEconometricsLorenzPoint
{
    [JsonPropertyName("population")]
    public double Population { get; init; }

    [JsonPropertyName("share")]
    public double Share { get; init; }
}

public sealed record class ToriiExplorerEconometricsDistributionSnapshot
{
    [JsonPropertyName("gini")]
    public double Gini { get; init; }

    [JsonPropertyName("hhi")]
    public double Hhi { get; init; }

    [JsonPropertyName("theil")]
    public double Theil { get; init; }

    [JsonPropertyName("entropy")]
    public double Entropy { get; init; }

    [JsonPropertyName("entropy_normalized")]
    public double EntropyNormalized { get; init; }

    [JsonPropertyName("nakamoto_33")]
    public ulong Nakamoto33 { get; init; }

    [JsonPropertyName("nakamoto_51")]
    public ulong Nakamoto51 { get; init; }

    [JsonPropertyName("nakamoto_67")]
    public ulong Nakamoto67 { get; init; }

    [JsonPropertyName("top1")]
    public double Top1 { get; init; }

    [JsonPropertyName("top5")]
    public double Top5 { get; init; }

    [JsonPropertyName("top10")]
    public double Top10 { get; init; }

    [JsonPropertyName("median")]
    public string? Median { get; init; }

    [JsonPropertyName("p90")]
    public string? P90 { get; init; }

    [JsonPropertyName("p99")]
    public string? P99 { get; init; }

    [JsonPropertyName("lorenz")]
    public IReadOnlyList<ToriiExplorerEconometricsLorenzPoint> Lorenz { get; init; } = Array.Empty<ToriiExplorerEconometricsLorenzPoint>();
}

public sealed record class ToriiExplorerEconometricsTopHolder
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("balance")]
    public string Balance { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerAssetDefinitionSnapshot
{
    [JsonPropertyName("definition_id")]
    public string DefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("computed_at_ms")]
    public ulong ComputedAtMilliseconds { get; init; }

    [JsonPropertyName("holders_total")]
    public ulong HoldersTotal { get; init; }

    [JsonPropertyName("total_supply")]
    public string TotalSupply { get; init; } = string.Empty;

    [JsonPropertyName("top_holders")]
    public IReadOnlyList<ToriiExplorerEconometricsTopHolder> TopHolders { get; init; } = Array.Empty<ToriiExplorerEconometricsTopHolder>();

    [JsonPropertyName("distribution")]
    public ToriiExplorerEconometricsDistributionSnapshot Distribution { get; init; } = new();
}

public sealed record class ToriiExplorerAsset
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("definition_id")]
    public string DefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("value")]
    public string Value { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerAssetsPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerAsset> Items { get; init; } = Array.Empty<ToriiExplorerAsset>();
}

public sealed record class ToriiExplorerNft
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("owned_by")]
    public string OwnedBy { get; init; } = string.Empty;

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata { get; init; } = new JsonObject();
}

public sealed record class ToriiExplorerNftsPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerNft> Items { get; init; } = Array.Empty<ToriiExplorerNft>();
}

public sealed record class ToriiExplorerRwaParent
{
    [JsonPropertyName("rwa")]
    public string Rwa { get; init; } = string.Empty;

    [JsonPropertyName("quantity")]
    public string Quantity { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerRwa
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("owned_by")]
    public string OwnedBy { get; init; } = string.Empty;

    [JsonPropertyName("quantity")]
    public string Quantity { get; init; } = string.Empty;

    [JsonPropertyName("held_quantity")]
    public string HeldQuantity { get; init; } = string.Empty;

    [JsonPropertyName("primary_reference")]
    public string PrimaryReference { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string? Status { get; init; }

    [JsonPropertyName("is_frozen")]
    public bool IsFrozen { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata { get; init; } = new JsonObject();

    [JsonPropertyName("parents")]
    public IReadOnlyList<ToriiExplorerRwaParent> Parents { get; init; } = Array.Empty<ToriiExplorerRwaParent>();
}

public sealed record class ToriiExplorerRwasPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerRwa> Items { get; init; } = Array.Empty<ToriiExplorerRwa>();
}

public sealed record class ToriiExplorerBlock
{
    [JsonPropertyName("hash")]
    public string Hash { get; init; } = string.Empty;

    [JsonPropertyName("height")]
    public ulong Height { get; init; }

    [JsonPropertyName("created_at")]
    public string CreatedAt { get; init; } = string.Empty;

    [JsonPropertyName("prev_block_hash")]
    public string? PreviousBlockHash { get; init; }

    [JsonPropertyName("transactions_hash")]
    public string? TransactionsHash { get; init; }

    [JsonPropertyName("transactions_rejected")]
    public uint TransactionsRejected { get; init; }

    [JsonPropertyName("transactions_total")]
    public uint TransactionsTotal { get; init; }
}

public sealed record class ToriiExplorerBlocksPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerBlock> Items { get; init; } = Array.Empty<ToriiExplorerBlock>();
}

public sealed record class ToriiExplorerTransaction
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("hash")]
    public string Hash { get; init; } = string.Empty;

    [JsonPropertyName("block")]
    public ulong Block { get; init; }

    [JsonPropertyName("created_at")]
    public string CreatedAt { get; init; } = string.Empty;

    [JsonPropertyName("executable")]
    public string Executable { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerDuration
{
    [JsonPropertyName("ms")]
    public ulong Milliseconds { get; init; }
}

public sealed record class ToriiExplorerTransactionRejection
{
    [JsonPropertyName("encoded")]
    public string Encoded { get; init; } = string.Empty;

    [JsonPropertyName("json")]
    public JsonNode? Json { get; init; }

    [JsonPropertyName("message")]
    public string Message { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerTransactionDetail
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("hash")]
    public string Hash { get; init; } = string.Empty;

    [JsonPropertyName("block")]
    public ulong Block { get; init; }

    [JsonPropertyName("created_at")]
    public string CreatedAt { get; init; } = string.Empty;

    [JsonPropertyName("executable")]
    public string Executable { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("rejection_reason")]
    public ToriiExplorerTransactionRejection? RejectionReason { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata { get; init; }

    [JsonPropertyName("nonce")]
    public ulong? Nonce { get; init; }

    [JsonPropertyName("signature")]
    public string Signature { get; init; } = string.Empty;

    [JsonPropertyName("time_to_live")]
    public ToriiExplorerDuration? TimeToLive { get; init; }
}

public sealed record class ToriiExplorerTransactionsPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerTransaction> Items { get; init; } = Array.Empty<ToriiExplorerTransaction>();
}

public sealed record class ToriiExplorerLatestTransactionsResponse
{
    [JsonPropertyName("sampled_at")]
    public string SampledAt { get; init; } = string.Empty;

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerTransaction> Items { get; init; } = Array.Empty<ToriiExplorerTransaction>();
}

public sealed record class ToriiExplorerInstructionJson
{
    [JsonPropertyName("kind")]
    public string Kind { get; init; } = string.Empty;

    [JsonPropertyName("payload")]
    public JsonNode? Payload { get; init; }

    [JsonPropertyName("wire_id")]
    public string WireId { get; init; } = string.Empty;

    [JsonPropertyName("encoded")]
    public string Encoded { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerInstructionBox
{
    [JsonPropertyName("encoded")]
    public string Encoded { get; init; } = string.Empty;

    [JsonPropertyName("json")]
    public ToriiExplorerInstructionJson? Json { get; init; }
}

public sealed record class ToriiExplorerInstruction
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("created_at")]
    public string CreatedAt { get; init; } = string.Empty;

    [JsonPropertyName("kind")]
    public string Kind { get; init; } = string.Empty;

    [JsonPropertyName("box")]
    public ToriiExplorerInstructionBox InstructionBox { get; init; } = new();

    [JsonPropertyName("transaction_hash")]
    public string TransactionHash { get; init; } = string.Empty;

    [JsonPropertyName("transaction_status")]
    public string TransactionStatus { get; init; } = string.Empty;

    [JsonPropertyName("block")]
    public ulong Block { get; init; }

    [JsonPropertyName("index")]
    public uint Index { get; init; }
}

public sealed record class ToriiExplorerInstructionsPage
{
    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerInstruction> Items { get; init; } = Array.Empty<ToriiExplorerInstruction>();
}

public sealed record class ToriiExplorerLatestInstructionsResponse
{
    [JsonPropertyName("sampled_at")]
    public string SampledAt { get; init; } = string.Empty;

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerInstruction> Items { get; init; } = Array.Empty<ToriiExplorerInstruction>();
}

public sealed record class ToriiExplorerHealthSnapshot
{
    [JsonPropertyName("head_height")]
    public ulong HeadHeight { get; init; }

    [JsonPropertyName("head_created_at")]
    public string? HeadCreatedAt { get; init; }

    [JsonPropertyName("sampled_at")]
    public string SampledAt { get; init; } = string.Empty;
}

public sealed record class ToriiExplorerMetricsSnapshot
{
    [JsonPropertyName("peers")]
    public ulong Peers { get; init; }

    [JsonPropertyName("domains")]
    public ulong Domains { get; init; }

    [JsonPropertyName("accounts")]
    public ulong Accounts { get; init; }

    [JsonPropertyName("assets")]
    public ulong Assets { get; init; }

    [JsonPropertyName("transactions_accepted")]
    public ulong TransactionsAccepted { get; init; }

    [JsonPropertyName("transactions_rejected")]
    public ulong TransactionsRejected { get; init; }

    [JsonPropertyName("block")]
    public ulong Block { get; init; }

    [JsonPropertyName("block_created_at")]
    public string? BlockCreatedAt { get; init; }

    [JsonPropertyName("finalized_block")]
    public ulong FinalizedBlock { get; init; }

    [JsonPropertyName("avg_commit_time")]
    public ToriiExplorerDuration? AverageCommitTime { get; init; }

    [JsonPropertyName("avg_block_time")]
    public ToriiExplorerDuration? AverageBlockTime { get; init; }
}

public sealed record class ToriiTransactionsPage
{
    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiTransactionSummary> Items { get; init; } = Array.Empty<ToriiTransactionSummary>();

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

public sealed record class ToriiSoraFsFileEntry
{
    [JsonPropertyName("path")]
    public IReadOnlyList<string> Path { get; init; } = Array.Empty<string>();

    [JsonPropertyName("offset")]
    public long Offset { get; init; }

    [JsonPropertyName("size")]
    public long Size { get; init; }

    [JsonPropertyName("first_chunk")]
    public long FirstChunk { get; init; }

    [JsonPropertyName("chunk_count")]
    public long ChunkCount { get; init; }
}

public sealed record class ToriiSoraFsCidLookupResponse
{
    [JsonPropertyName("content_cid")]
    public string ContentCid { get; init; } = string.Empty;

    [JsonPropertyName("manifest_digest_hex")]
    public string ManifestDigestHex { get; init; } = string.Empty;

    [JsonPropertyName("manifest_id_hex")]
    public string ManifestIdHex { get; init; } = string.Empty;

    [JsonPropertyName("index_document")]
    public string? IndexDocument { get; init; }

    [JsonPropertyName("files")]
    public IReadOnlyList<ToriiSoraFsFileEntry> Files { get; init; } = Array.Empty<ToriiSoraFsFileEntry>();
}

public sealed record class ToriiSoraFsDenylistPackSummary
{
    [JsonPropertyName("pack_id")]
    public string PackId { get; init; } = string.Empty;

    [JsonPropertyName("version")]
    public string? Version { get; init; }

    [JsonPropertyName("default_enabled")]
    public bool DefaultEnabled { get; init; }

    [JsonPropertyName("active")]
    public bool Active { get; init; }

    [JsonPropertyName("policy_tier")]
    public string? PolicyTier { get; init; }

    [JsonPropertyName("manifest_cid")]
    public string? ManifestCid { get; init; }

    [JsonPropertyName("merkle_root")]
    public string? MerkleRoot { get; init; }

    [JsonPropertyName("issued_by_proposal_id")]
    public string? IssuedByProposalId { get; init; }

    [JsonPropertyName("review_reference")]
    public string? ReviewReference { get; init; }

    [JsonPropertyName("jurisdiction")]
    public string? Jurisdiction { get; init; }

    [JsonPropertyName("issued_at")]
    public string? IssuedAt { get; init; }

    [JsonPropertyName("expires_at")]
    public string? ExpiresAt { get; init; }

    [JsonPropertyName("entry_count")]
    public long EntryCount { get; init; }
}

public sealed record class ToriiSoraFsDenylistCatalogResponse
{
    [JsonPropertyName("version")]
    public long Version { get; init; }

    [JsonPropertyName("jurisdiction")]
    public string? Jurisdiction { get; init; }

    [JsonPropertyName("opt_out_packs")]
    public IReadOnlyList<string> OptOutPacks { get; init; } = Array.Empty<string>();

    [JsonPropertyName("extra_packs")]
    public IReadOnlyList<string> ExtraPacks { get; init; } = Array.Empty<string>();

    [JsonPropertyName("packs")]
    public IReadOnlyList<ToriiSoraFsDenylistPackSummary> Packs { get; init; } = Array.Empty<ToriiSoraFsDenylistPackSummary>();
}

public sealed record class ToriiSoraFsDenylistPackResponse
{
    [JsonPropertyName("pack_id")]
    public string PackId { get; init; } = string.Empty;

    [JsonPropertyName("version")]
    public string? Version { get; init; }

    [JsonPropertyName("default_enabled")]
    public bool DefaultEnabled { get; init; }

    [JsonPropertyName("active")]
    public bool Active { get; init; }

    [JsonPropertyName("policy_tier")]
    public string? PolicyTier { get; init; }

    [JsonPropertyName("manifest_cid")]
    public string? ManifestCid { get; init; }

    [JsonPropertyName("merkle_root")]
    public string? MerkleRoot { get; init; }

    [JsonPropertyName("issued_by_proposal_id")]
    public string? IssuedByProposalId { get; init; }

    [JsonPropertyName("review_reference")]
    public string? ReviewReference { get; init; }

    [JsonPropertyName("jurisdiction")]
    public string? Jurisdiction { get; init; }

    [JsonPropertyName("issued_at")]
    public string? IssuedAt { get; init; }

    [JsonPropertyName("expires_at")]
    public string? ExpiresAt { get; init; }

    [JsonPropertyName("entry_count")]
    public long EntryCount { get; init; }

    [JsonPropertyName("source_path")]
    public string SourcePath { get; init; } = string.Empty;
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

public sealed record class ToriiContractManifestSummary
{
    [JsonPropertyName("code_hash")]
    public string? CodeHash { get; init; }

    [JsonPropertyName("abi_hash")]
    public string? AbiHash { get; init; }
}

public sealed record class ToriiContractCodeRecord
{
    [JsonPropertyName("manifest")]
    public ToriiContractManifestSummary Manifest { get; init; } = new();
}

public sealed record class ToriiDeployContractRequest
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("private_key")]
    public string PrivateKey { get; init; } = string.Empty;

    [JsonPropertyName("code_b64")]
    public string CodeBase64 { get; init; } = string.Empty;

    [JsonPropertyName("dataspace")]
    public string? Dataspace { get; init; }
}

public sealed record class ToriiDeployContractResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("contract_address")]
    public string ContractAddress { get; init; } = string.Empty;

    [JsonPropertyName("dataspace")]
    public string Dataspace { get; init; } = string.Empty;

    [JsonPropertyName("deploy_nonce")]
    public ulong DeployNonce { get; init; }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex { get; init; } = string.Empty;

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex { get; init; } = string.Empty;
}

public sealed record class ToriiContractCodeBytesResponse
{
    [JsonPropertyName("code_b64")]
    public string CodeBase64 { get; init; } = string.Empty;

    public byte[] DecodeBytes()
    {
        if (string.IsNullOrWhiteSpace(CodeBase64))
        {
            return Array.Empty<byte>();
        }

        return Convert.FromBase64String(CodeBase64);
    }
}

public sealed record class ToriiActivateContractInstanceRequest
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("private_key")]
    public string PrivateKey { get; init; } = string.Empty;

    [JsonPropertyName("namespace")]
    public string Namespace { get; init; } = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("code_hash")]
    public string CodeHash { get; init; } = string.Empty;
}

public sealed record class ToriiActivateContractInstanceResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }
}

public sealed record class ToriiContractInstance
{
    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex { get; init; } = string.Empty;
}

public sealed record class ToriiContractInstancesResponse
{
    [JsonPropertyName("namespace")]
    public string Namespace { get; init; } = string.Empty;

    [JsonPropertyName("instances")]
    public IReadOnlyList<ToriiContractInstance> Instances { get; init; } = Array.Empty<ToriiContractInstance>();

    [JsonPropertyName("total")]
    public ulong Total { get; init; }

    [JsonPropertyName("offset")]
    public ulong Offset { get; init; }

    [JsonPropertyName("limit")]
    public ulong Limit { get; init; }
}

public sealed record class ToriiDeployAndActivateContractInstanceRequest
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("private_key")]
    public string PrivateKey { get; init; } = string.Empty;

    [JsonPropertyName("namespace")]
    public string Namespace { get; init; } = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("code_b64")]
    public string CodeBase64 { get; init; } = string.Empty;
}

public sealed record class ToriiDeployAndActivateContractInstanceResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("namespace")]
    public string Namespace { get; init; } = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex { get; init; } = string.Empty;

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex { get; init; } = string.Empty;
}

public sealed record class ToriiContractStateEntry
{
    [JsonPropertyName("path")]
    public string Path { get; init; } = string.Empty;

    [JsonPropertyName("found")]
    public bool Found { get; init; }

    [JsonPropertyName("value_b64")]
    public string? ValueBase64 { get; init; }

    [JsonPropertyName("value_len")]
    public ulong? ValueLength { get; init; }

    [JsonPropertyName("value_json")]
    public JsonNode? ValueJson { get; init; }

    [JsonPropertyName("decode_error")]
    public string? DecodeError { get; init; }
}

public sealed record class ToriiContractStateResponse
{
    [JsonPropertyName("path")]
    public string? Path { get; init; }

    [JsonPropertyName("paths")]
    public IReadOnlyList<string>? Paths { get; init; }

    [JsonPropertyName("prefix")]
    public string? Prefix { get; init; }

    [JsonPropertyName("entries")]
    public IReadOnlyList<ToriiContractStateEntry> Entries { get; init; } = Array.Empty<ToriiContractStateEntry>();

    [JsonPropertyName("offset")]
    public ulong Offset { get; init; }

    [JsonPropertyName("limit")]
    public ulong Limit { get; init; }

    [JsonPropertyName("next_offset")]
    public ulong? NextOffset { get; init; }
}

public sealed record class ToriiContractViewRequest
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("contract_alias")]
    public string? ContractAlias { get; init; }

    [JsonPropertyName("entrypoint")]
    public string? Entrypoint { get; init; }

    [JsonPropertyName("payload")]
    public JsonNode? Payload { get; init; }

    [JsonPropertyName("gas_limit")]
    public ulong GasLimit { get; init; }
}

public sealed record class ToriiContractCallRequest
{
    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("private_key")]
    public string? PrivateKey { get; init; }

    [JsonPropertyName("public_key_hex")]
    public string? PublicKeyHex { get; init; }

    [JsonPropertyName("signature_b64")]
    public string? SignatureBase64 { get; init; }

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("contract_alias")]
    public string? ContractAlias { get; init; }

    [JsonPropertyName("entrypoint")]
    public string? Entrypoint { get; init; }

    [JsonPropertyName("payload")]
    public JsonNode? Payload { get; init; }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds { get; init; }

    [JsonPropertyName("gas_asset_id")]
    public string? GasAssetId { get; init; }

    [JsonPropertyName("fee_sponsor")]
    public string? FeeSponsor { get; init; }

    [JsonPropertyName("gas_limit")]
    public ulong GasLimit { get; init; }
}

public sealed record class ToriiContractViewAccessHints
{
    [JsonPropertyName("read_keys")]
    public IReadOnlyList<string> ReadKeys { get; init; } = Array.Empty<string>();

    [JsonPropertyName("write_keys")]
    public IReadOnlyList<string> WriteKeys { get; init; } = Array.Empty<string>();
}

public sealed record class ToriiContractViewEntrypointParam
{
    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("type_name")]
    public string TypeName { get; init; } = string.Empty;
}

public sealed record class ToriiContractViewEntrypoint
{
    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;

    [JsonPropertyName("kind")]
    public string Kind { get; init; } = string.Empty;

    [JsonPropertyName("params")]
    public IReadOnlyList<ToriiContractViewEntrypointParam> Parameters { get; init; } = Array.Empty<ToriiContractViewEntrypointParam>();

    [JsonPropertyName("return_type")]
    public string? ReturnType { get; init; }

    [JsonPropertyName("permission")]
    public string? Permission { get; init; }

    [JsonPropertyName("read_keys")]
    public IReadOnlyList<string> ReadKeys { get; init; } = Array.Empty<string>();

    [JsonPropertyName("write_keys")]
    public IReadOnlyList<string> WriteKeys { get; init; } = Array.Empty<string>();

    [JsonPropertyName("access_hints_complete")]
    public bool? AccessHintsComplete { get; init; }

    [JsonPropertyName("access_hints_skipped")]
    public IReadOnlyList<string> AccessHintsSkipped { get; init; } = Array.Empty<string>();

    [JsonPropertyName("triggers")]
    public IReadOnlyList<string> Triggers { get; init; } = Array.Empty<string>();
}

public sealed record class ToriiContractViewSyscall
{
    [JsonPropertyName("number")]
    public byte Number { get; init; }

    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("count")]
    public ulong Count { get; init; }
}

public sealed record class ToriiContractViewMemory
{
    [JsonPropertyName("load64")]
    public ulong Load64 { get; init; }

    [JsonPropertyName("store64")]
    public ulong Store64 { get; init; }

    [JsonPropertyName("load128")]
    public ulong Load128 { get; init; }

    [JsonPropertyName("store128")]
    public ulong Store128 { get; init; }
}

public sealed record class ToriiContractViewAnalysis
{
    [JsonPropertyName("instruction_count")]
    public ulong InstructionCount { get; init; }

    [JsonPropertyName("memory")]
    public ToriiContractViewMemory Memory { get; init; } = new();

    [JsonPropertyName("syscalls")]
    public IReadOnlyList<ToriiContractViewSyscall> Syscalls { get; init; } = Array.Empty<ToriiContractViewSyscall>();
}

public sealed record class ToriiContractVerifiedSourceReference
{
    [JsonPropertyName("language")]
    public string Language { get; init; } = string.Empty;

    [JsonPropertyName("source_name")]
    public string? SourceName { get; init; }

    [JsonPropertyName("submitted_at")]
    public string SubmittedAt { get; init; } = string.Empty;

    [JsonPropertyName("manifest_id_hex")]
    public string? ManifestIdHex { get; init; }

    [JsonPropertyName("payload_digest_hex")]
    public string? PayloadDigestHex { get; init; }

    [JsonPropertyName("content_length")]
    public ulong? ContentLength { get; init; }
}

public sealed record class ToriiContractCodeView
{
    [JsonPropertyName("code_hash")]
    public string CodeHash { get; init; } = string.Empty;

    [JsonPropertyName("declared_code_hash")]
    public string? DeclaredCodeHash { get; init; }

    [JsonPropertyName("abi_hash")]
    public string? AbiHash { get; init; }

    [JsonPropertyName("compiler_fingerprint")]
    public string? CompilerFingerprint { get; init; }

    [JsonPropertyName("byte_len")]
    public ulong? ByteLength { get; init; }

    [JsonPropertyName("permissions")]
    public IReadOnlyList<string> Permissions { get; init; } = Array.Empty<string>();

    [JsonPropertyName("access_hints")]
    public ToriiContractViewAccessHints? AccessHints { get; init; }

    [JsonPropertyName("entrypoints")]
    public IReadOnlyList<ToriiContractViewEntrypoint> Entrypoints { get; init; } = Array.Empty<ToriiContractViewEntrypoint>();

    [JsonPropertyName("analysis")]
    public ToriiContractViewAnalysis? Analysis { get; init; }

    [JsonPropertyName("warnings")]
    public IReadOnlyList<string> Warnings { get; init; } = Array.Empty<string>();

    [JsonPropertyName("rendered_source_kind")]
    public string RenderedSourceKind { get; init; } = string.Empty;

    [JsonPropertyName("rendered_source_text")]
    public string RenderedSourceText { get; init; } = string.Empty;

    [JsonPropertyName("verified_source_ref")]
    public ToriiContractVerifiedSourceReference? VerifiedSourceReference { get; init; }
}

public sealed record class ToriiContractCallResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("submitted")]
    public bool Submitted { get; init; }

    [JsonPropertyName("dataspace")]
    public string Dataspace { get; init; } = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex { get; init; } = string.Empty;

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex { get; init; } = string.Empty;

    [JsonPropertyName("creation_time_ms")]
    public ulong CreationTimeMilliseconds { get; init; }

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex { get; init; }

    [JsonPropertyName("transaction_scaffold_b64")]
    public string? TransactionScaffoldBase64 { get; init; }

    [JsonPropertyName("signed_transaction_b64")]
    public string? SignedTransactionBase64 { get; init; }

    [JsonPropertyName("signing_message_b64")]
    public string? SigningMessageBase64 { get; init; }

    [JsonPropertyName("entrypoint")]
    public string? Entrypoint { get; init; }
}

public sealed record class ToriiContractViewResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("dataspace")]
    public string Dataspace { get; init; } = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex { get; init; } = string.Empty;

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex { get; init; } = string.Empty;

    [JsonPropertyName("entrypoint")]
    public string Entrypoint { get; init; } = string.Empty;

    [JsonPropertyName("result")]
    public JsonNode? Result { get; init; }
}

public sealed record class ToriiContractViewVmDiagnostic
{
    [JsonPropertyName("trap_kind")]
    public string TrapKind { get; init; } = string.Empty;

    [JsonPropertyName("message")]
    public string Message { get; init; } = string.Empty;

    [JsonPropertyName("pc")]
    public ulong ProgramCounter { get; init; }

    [JsonPropertyName("function")]
    public string? Function { get; init; }

    [JsonPropertyName("source_path")]
    public string? SourcePath { get; init; }

    [JsonPropertyName("line")]
    public uint? Line { get; init; }

    [JsonPropertyName("column")]
    public uint? Column { get; init; }

    [JsonPropertyName("gas_limit")]
    public ulong GasLimit { get; init; }

    [JsonPropertyName("gas_remaining")]
    public ulong GasRemaining { get; init; }

    [JsonPropertyName("gas_used")]
    public ulong GasUsed { get; init; }

    [JsonPropertyName("cycles")]
    public ulong Cycles { get; init; }

    [JsonPropertyName("max_cycles")]
    public ulong MaxCycles { get; init; }

    [JsonPropertyName("stack_limit_bytes")]
    public ulong StackLimitBytes { get; init; }

    [JsonPropertyName("stack_bytes_used")]
    public ulong StackBytesUsed { get; init; }

    [JsonPropertyName("entrypoint_pc")]
    public ulong? EntrypointProgramCounter { get; init; }

    [JsonPropertyName("current_function")]
    public string? CurrentFunction { get; init; }

    [JsonPropertyName("opcode")]
    public ushort? Opcode { get; init; }

    [JsonPropertyName("syscall")]
    public uint? Syscall { get; init; }

    [JsonPropertyName("predecoded_loaded")]
    public bool PredecodedLoaded { get; init; }

    [JsonPropertyName("predecoded_hit")]
    public bool? PredecodedHit { get; init; }
}

public sealed record class ToriiContractViewErrorResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("dataspace")]
    public string Dataspace { get; init; } = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex { get; init; } = string.Empty;

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex { get; init; } = string.Empty;

    [JsonPropertyName("entrypoint")]
    public string Entrypoint { get; init; } = string.Empty;

    [JsonPropertyName("error")]
    public string Error { get; init; } = string.Empty;

    [JsonPropertyName("vm_diagnostic")]
    public ToriiContractViewVmDiagnostic? VmDiagnostic { get; init; }
}

public sealed record class ToriiContractViewExecutionResult
{
    public bool IsSuccess => Success is not null;

    public ToriiContractViewResponse? Success { get; init; }

    public ToriiContractViewErrorResponse? Error { get; init; }
}

public sealed record class ToriiContractVerifiedSourceSubmission
{
    [JsonPropertyName("language")]
    public string Language { get; init; } = string.Empty;

    [JsonPropertyName("source_name")]
    public string? SourceName { get; init; }

    [JsonPropertyName("source_text")]
    public string SourceText { get; init; } = string.Empty;
}

public sealed record class ToriiContractVerifiedSourceJob
{
    [JsonPropertyName("job_id")]
    public string JobId { get; init; } = string.Empty;

    [JsonPropertyName("code_hash")]
    public string CodeHash { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("submitted_at")]
    public string SubmittedAt { get; init; } = string.Empty;

    [JsonPropertyName("completed_at")]
    public string? CompletedAt { get; init; }

    [JsonPropertyName("message")]
    public string? Message { get; init; }

    [JsonPropertyName("actual_code_hash")]
    public string? ActualCodeHash { get; init; }

    [JsonPropertyName("verified_source_ref")]
    public ToriiContractVerifiedSourceReference? VerifiedSourceReference { get; init; }
}

public sealed record class ToriiMultisigContractCallProposeRequest
{
    [JsonPropertyName("multisig_account_id")]
    public string? MultisigAccountId { get; init; }

    [JsonPropertyName("multisig_account_alias")]
    public string? MultisigAccountAlias { get; init; }

    [JsonPropertyName("signer_account_id")]
    public string SignerAccountId { get; init; } = string.Empty;

    [JsonPropertyName("private_key")]
    public string? PrivateKey { get; init; }

    [JsonPropertyName("public_key_hex")]
    public string? PublicKeyHex { get; init; }

    [JsonPropertyName("signature_b64")]
    public string? SignatureBase64 { get; init; }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds { get; init; }

    [JsonPropertyName("namespace")]
    public string Namespace { get; init; } = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId { get; init; } = string.Empty;

    [JsonPropertyName("entrypoint")]
    public string Entrypoint { get; init; } = string.Empty;

    [JsonPropertyName("payload")]
    public JsonNode? Payload { get; init; }

    [JsonPropertyName("gas_asset_id")]
    public string? GasAssetId { get; init; }

    [JsonPropertyName("fee_sponsor")]
    public string? FeeSponsor { get; init; }

    [JsonPropertyName("gas_limit")]
    public ulong? GasLimit { get; init; }
}

public sealed record class ToriiMultisigContractCallApproveRequest
{
    [JsonPropertyName("multisig_account_id")]
    public string? MultisigAccountId { get; init; }

    [JsonPropertyName("multisig_account_alias")]
    public string? MultisigAccountAlias { get; init; }

    [JsonPropertyName("signer_account_id")]
    public string SignerAccountId { get; init; } = string.Empty;

    [JsonPropertyName("private_key")]
    public string? PrivateKey { get; init; }

    [JsonPropertyName("public_key_hex")]
    public string? PublicKeyHex { get; init; }

    [JsonPropertyName("signature_b64")]
    public string? SignatureBase64 { get; init; }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds { get; init; }

    [JsonPropertyName("proposal_id")]
    public string? ProposalId { get; init; }

    [JsonPropertyName("instructions_hash")]
    public string? InstructionsHash { get; init; }
}

public sealed record class ToriiMultisigContractCallResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("resolved_multisig_account_id")]
    public string ResolvedMultisigAccountId { get; init; } = string.Empty;

    [JsonPropertyName("submitted")]
    public bool? Submitted { get; init; }

    [JsonPropertyName("proposal_id")]
    public string? ProposalId { get; init; }

    [JsonPropertyName("instructions_hash")]
    public string? InstructionsHash { get; init; }

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex { get; init; }

    [JsonPropertyName("executed_tx_hash_hex")]
    public string? ExecutedTransactionHashHex { get; init; }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds { get; init; }

    [JsonPropertyName("signing_message_b64")]
    public string? SigningMessageBase64 { get; init; }
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
