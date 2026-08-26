using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiListSnapshots
{
    internal static T[]? Copy<T>(IReadOnlyList<T>? values) =>
        values is null ? null : CopyNonNullItemsCore(values, nameof(values));

    internal static T[] CopyRequired<T>(IReadOnlyList<T>? values) =>
        values is null ? Array.Empty<T>() : CopyNonNullItemsCore(values, nameof(values));

    internal static T[]? CopyNonNullItems<T>(IReadOnlyList<T>? values, string parameterName)
    {
        if (values is null)
        {
            return null;
        }

        return CopyNonNullItemsCore(values, parameterName);
    }

    private static T[] CopyNonNullItemsCore<T>(IReadOnlyList<T> values, string parameterName)
    {
        var copy = new T[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            var item = values[index];
            if (item is null)
            {
                throw new ArgumentException("List elements must not be null.", $"{parameterName}[{index}]");
            }

            copy[index] = item;
        }

        return copy;
    }
}

internal static class ToriiJsonSnapshots
{
    internal static JsonNode? Copy(JsonNode? value) => value?.DeepClone();

    internal static JsonObject? CopyObject(JsonObject? value) =>
        value is null ? null : (JsonObject)value.DeepClone();
}

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

/// <summary>Secret-free request accepted by the sponsored onboarding planner.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiAccountOnboardingPlanRequest
{
    private string[] permissions = Array.Empty<string>();

    [JsonPropertyName("version")]
    public byte Version { get; init; } = 1;

    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("permissions")]
    public IReadOnlyList<string> Permissions
    {
        get => permissions.ToArray();
        init => permissions = ToriiListSnapshots.CopyNonNullItems(value, nameof(Permissions))
            ?? throw new ArgumentNullException(nameof(Permissions));
    }
}

/// <summary>Canonical server-signed sponsored onboarding plan body.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiAccountOnboardingPlanBody
{
    [JsonPropertyName("version")]
    public byte Version { get; init; }

    [JsonPropertyName("request")]
    public ToriiAccountOnboardingPlanRequest Request { get; init; } = new();

    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("network_id")]
    public NetworkId NetworkId { get; init; } = null!;

    [JsonPropertyName("anchor")]
    public JsonElement Anchor { get; init; }

    [JsonPropertyName("resource")]
    public JsonElement Resource { get; init; }

    [JsonPropertyName("acquisition")]
    public JsonElement Acquisition { get; init; }

    [JsonPropertyName("quote_guard")]
    public JsonElement QuoteGuard { get; init; }

    [JsonPropertyName("instructions")]
    public JsonElement Instructions { get; init; }

    [JsonPropertyName("owner_auto_renew_instruction")]
    public JsonElement OwnerAutoRenewInstruction { get; init; }

    [JsonPropertyName("valid_until_ms")]
    public ulong ValidUntilMilliseconds { get; init; }
}

/// <summary>Encodes an onboarding plan body into its exact canonical Norito bytes.</summary>
public delegate byte[] ToriiAccountOnboardingPlanBodyEncoder(ToriiAccountOnboardingPlanBody body);

/// <summary>Stateless signer-authenticated sponsored onboarding receipt.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiAccountOnboardingPlanReceipt
{
    [JsonPropertyName("body")]
    public ToriiAccountOnboardingPlanBody Body { get; init; } = new();

    [JsonPropertyName("plan_hash")]
    public string PlanHash { get; init; } = string.Empty;

    [JsonPropertyName("signature")]
    public string Signature { get; init; } = string.Empty;
}

/// <summary>Apply request containing exactly one previously issued receipt.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiAccountOnboardingApplyRequest
{
    [JsonPropertyName("receipt")]
    public ToriiAccountOnboardingPlanReceipt Receipt { get; init; } = new();
}

/// <summary>Live disposition returned by sponsored onboarding apply.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiAccountOnboardingDisposition
{
    [JsonPropertyName("kind")]
    public string Kind { get; init; } = string.Empty;

    [JsonPropertyName("value")]
    public JsonElement Value { get; init; }
}

/// <summary>Queued, repaired, or unchanged sponsored onboarding result.</summary>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiAccountOnboardingResponse
{
    [JsonPropertyName("account_id")]
    public string AccountId { get; init; } = string.Empty;

    [JsonPropertyName("alias")]
    public string Alias { get; init; } = string.Empty;

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex { get; init; }

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("disposition")]
    public ToriiAccountOnboardingDisposition Disposition { get; init; } = new();
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

[JsonConverter(typeof(ToriiAccountFaucetResponseJsonConverter))]
public sealed record class ToriiAccountFaucetResponse
{
    private string accountId = string.Empty;
    private string assetDefinitionId = string.Empty;
    private string assetId = string.Empty;
    private string amount = string.Empty;
    private string transactionHashHex = string.Empty;
    private string status = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiAccountOnboardingReceiptVerifier.RequireCanonicalAccountId(
            value,
            nameof(AccountId));
    }

    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId
    {
        get => assetDefinitionId;
        init => assetDefinitionId = ToriiOnboardingDirectMetadata.RequireExactTokenText(
            value,
            nameof(AssetDefinitionId));
    }

    [JsonPropertyName("asset_id")]
    public string AssetId
    {
        get => assetId;
        init => assetId = ToriiOnboardingDirectMetadata.RequireExactTokenText(value, nameof(AssetId));
    }

    [JsonPropertyName("amount")]
    public string Amount
    {
        get => amount;
        init => amount = ToriiOnboardingDirectMetadata.RequireCanonicalQuantityText(
            value,
            nameof(Amount));
    }

    [JsonPropertyName("tx_hash_hex")]
    public string TransactionHashHex
    {
        get => transactionHashHex;
        init => transactionHashHex = ToriiOnboardingDirectMetadata.RequireOptionalTransactionHashHex(
            value,
            nameof(TransactionHashHex));
    }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiOnboardingDirectMetadata.RequireExactTokenText(value, nameof(Status));
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiVerifyingKeyRegisterRequest
{
    private byte[]? verifyingKeyBytes;

    [JsonPropertyName("authority")]
    public string? Authority { get; init; }

    [JsonPropertyName("backend")]
    public string? Backend { get; init; }

    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("version")]
    public uint? Version { get; init; }

    [JsonPropertyName("circuit_id")]
    public string? CircuitId { get; init; }

    [JsonPropertyName("public_inputs_schema_hash_hex")]
    public string? PublicInputsSchemaHashHex { get; init; }

    [JsonPropertyName("curve")]
    public string? Curve { get; init; }

    [JsonPropertyName("gas_schedule_id")]
    public string? GasScheduleId { get; init; }

    [JsonPropertyName("vk_len")]
    public uint? VerifyingKeyLength { get; init; }

    [JsonPropertyName("max_proof_bytes")]
    public uint? MaxProofBytes { get; init; }

    [JsonPropertyName("metadata_uri_cid")]
    public string? MetadataUriCid { get; init; }

    [JsonPropertyName("vk_bytes_cid")]
    public string? VerifyingKeyBytesCid { get; init; }

    [JsonPropertyName("activation_height")]
    public ulong? ActivationHeight { get; init; }

    [JsonPropertyName("withdraw_height")]
    public ulong? WithdrawHeight { get; init; }

    [JsonPropertyName("commitment_hex")]
    public string? CommitmentHex { get; init; }

    [JsonPropertyName("vk_bytes")]
    public byte[]? VerifyingKeyBytes
    {
        get => verifyingKeyBytes?.ToArray();
        init => verifyingKeyBytes = value?.ToArray();
    }

    [JsonPropertyName("status")]
    public string? Status { get; init; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiVerifyingKeyUpdateRequest
{
    private byte[]? verifyingKeyBytes;

    [JsonPropertyName("authority")]
    public string? Authority { get; init; }

    [JsonPropertyName("backend")]
    public string? Backend { get; init; }

    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("version")]
    public uint? Version { get; init; }

    [JsonPropertyName("circuit_id")]
    public string? CircuitId { get; init; }

    [JsonPropertyName("public_inputs_schema_hash_hex")]
    public string? PublicInputsSchemaHashHex { get; init; }

    [JsonPropertyName("curve")]
    public string? Curve { get; init; }

    [JsonPropertyName("gas_schedule_id")]
    public string? GasScheduleId { get; init; }

    [JsonPropertyName("commitment_hex")]
    public string? CommitmentHex { get; init; }

    [JsonPropertyName("vk_len")]
    public uint? VerifyingKeyLength { get; init; }

    [JsonPropertyName("max_proof_bytes")]
    public uint? MaxProofBytes { get; init; }

    [JsonPropertyName("metadata_uri_cid")]
    public string? MetadataUriCid { get; init; }

    [JsonPropertyName("vk_bytes_cid")]
    public string? VerifyingKeyBytesCid { get; init; }

    [JsonPropertyName("activation_height")]
    public ulong? ActivationHeight { get; init; }

    [JsonPropertyName("withdraw_height")]
    public ulong? WithdrawHeight { get; init; }

    [JsonPropertyName("vk_bytes")]
    public byte[]? VerifyingKeyBytes
    {
        get => verifyingKeyBytes?.ToArray();
        init => verifyingKeyBytes = value?.ToArray();
    }

    [JsonPropertyName("status")]
    public string? Status { get; init; }
}

/// <summary>
/// Exact unsigned transaction draft returned by verifying-key register/update routes.
/// </summary>
public sealed class ToriiVerifyingKeyTransactionDraft
{
    private readonly byte[] transactionPayload;
    private readonly byte[] signingMessage;

    internal ToriiVerifyingKeyTransactionDraft(
        string transactionPayloadBase64,
        string signingMessageBase64,
        byte[] transactionPayload,
        byte[] signingMessage)
    {
        Submitted = false;
        TransactionPayloadBase64 = transactionPayloadBase64;
        SigningMessageBase64 = signingMessageBase64;
        this.transactionPayload = transactionPayload.ToArray();
        this.signingMessage = signingMessage.ToArray();
    }

    /// <summary>
    /// Always <see langword="false"/> because the server never signs or submits the transaction.
    /// </summary>
    [JsonPropertyName("submitted")]
    public bool Submitted { get; }

    /// <summary>Canonical padded base64 containing the Norito <c>TransactionPayload</c>.</summary>
    [JsonPropertyName("transaction_payload_b64")]
    public string TransactionPayloadBase64 { get; }

    /// <summary>Canonical padded base64 containing the 32-byte Iroha payload prehash.</summary>
    [JsonPropertyName("signing_message_b64")]
    public string SigningMessageBase64 { get; }

    /// <summary>
    /// Canonical Norito payload bytes for SDK signers that apply the Iroha prehash themselves.
    /// </summary>
    [JsonIgnore]
    public byte[] TransactionPayload => transactionPayload.ToArray();

    /// <summary>
    /// Already-prehashed bytes for raw signature primitives and HSM integrations.
    /// </summary>
    [JsonIgnore]
    public byte[] SigningMessage => signingMessage.ToArray();
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiVpnQuoteCreateRequest
{
    [JsonPropertyName("exit_class")]
    public string ExitClass { get; init; } = string.Empty;

    [JsonPropertyName("metering_public_key_hex")]
    public string MeteringPublicKeyHex { get; init; } = string.Empty;
}

[JsonConverter(typeof(ToriiVpnQuoteJsonConverter))]
public sealed record class ToriiVpnQuote
{
    private string quoteId = string.Empty;
    private string leaseIdHex = string.Empty;
    private string sessionIdHex = string.Empty;
    private string paymentReference = string.Empty;
    private string accountId = string.Empty;
    private string exitClass = string.Empty;
    private string relayEndpoint = string.Empty;
    private ulong leaseSeconds;
    private ulong quoteExpiresAtMilliseconds;
    private string feeAssetId = string.Empty;
    private string escrowAccountId = string.Empty;
    private string operatorAccountId = string.Empty;
    private string leaseFee = string.Empty;
    private string[]? routePushes = Array.Empty<string>();
    private string[]? excludedRoutes = Array.Empty<string>();
    private string[]? dnsServers = Array.Empty<string>();
    private string[]? tunnelAddresses = Array.Empty<string>();
    private ulong mtuBytes;
    private string meterFamily = string.Empty;
    private string relayIdHex = string.Empty;
    private string descriptorCommitHex = string.Empty;
    private string tlsServerName = string.Empty;
    private string relayTlsSpkiSha256Hex = string.Empty;
    private string relayCertificateSha256Hex = string.Empty;
    private string directorySnapshotDigestHex = string.Empty;
    private string meteringPublicKeyHex = string.Empty;
    private ToriiVpnTxInstruction openLeaseInstruction = null!;

    [JsonPropertyName("quote_id")]
    public string QuoteId
    {
        get => quoteId;
        init => quoteId = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(QuoteId), 32);
    }

    [JsonPropertyName("lease_id_hex")]
    public string LeaseIdHex
    {
        get => leaseIdHex;
        init => leaseIdHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(LeaseIdHex), 32);
    }

    [JsonPropertyName("session_id_hex")]
    public string SessionIdHex
    {
        get => sessionIdHex;
        init => sessionIdHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(SessionIdHex), 16);
    }

    [JsonPropertyName("payment_reference")]
    public string PaymentReference
    {
        get => paymentReference;
        init => paymentReference = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(PaymentReference));
    }

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("exit_class")]
    public string ExitClass
    {
        get => exitClass;
        init => exitClass = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(ExitClass));
    }

    [JsonPropertyName("relay_endpoint")]
    public string RelayEndpoint
    {
        get => relayEndpoint;
        init => relayEndpoint = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(RelayEndpoint));
    }

    [JsonPropertyName("lease_secs")]
    public ulong LeaseSeconds
    {
        get => leaseSeconds;
        init => leaseSeconds = ToriiVpnDirectMetadata.RequirePositive(value, nameof(LeaseSeconds));
    }

    [JsonPropertyName("quote_expires_at_ms")]
    public ulong QuoteExpiresAtMilliseconds
    {
        get => quoteExpiresAtMilliseconds;
        init => quoteExpiresAtMilliseconds = ToriiVpnDirectMetadata.RequirePositive(
            value,
            nameof(QuoteExpiresAtMilliseconds));
    }

    [JsonPropertyName("fee_asset_id")]
    public string FeeAssetId
    {
        get => feeAssetId;
        init => feeAssetId = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(FeeAssetId));
    }

    [JsonPropertyName("escrow_account_id")]
    public string EscrowAccountId
    {
        get => escrowAccountId;
        init => escrowAccountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(EscrowAccountId));
    }

    [JsonPropertyName("operator_account_id")]
    public string OperatorAccountId
    {
        get => operatorAccountId;
        init => operatorAccountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(OperatorAccountId));
    }

    [JsonPropertyName("lease_fee")]
    public string LeaseFee
    {
        get => leaseFee;
        init => leaseFee = ToriiQuantityJson.RequireCanonicalQuantity(value, nameof(LeaseFee));
    }

    [JsonPropertyName("route_pushes")]
    public IReadOnlyList<string>? RoutePushes
    {
        get => ToriiListSnapshots.Copy(routePushes);
        init => routePushes = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(RoutePushes));
    }

    [JsonPropertyName("excluded_routes")]
    public IReadOnlyList<string>? ExcludedRoutes
    {
        get => ToriiListSnapshots.Copy(excludedRoutes);
        init => excludedRoutes = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(ExcludedRoutes));
    }

    [JsonPropertyName("dns_servers")]
    public IReadOnlyList<string>? DnsServers
    {
        get => ToriiListSnapshots.Copy(dnsServers);
        init => dnsServers = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(DnsServers));
    }

    [JsonPropertyName("tunnel_addresses")]
    public IReadOnlyList<string>? TunnelAddresses
    {
        get => ToriiListSnapshots.Copy(tunnelAddresses);
        init => tunnelAddresses = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(TunnelAddresses));
    }

    [JsonPropertyName("mtu_bytes")]
    public ulong MtuBytes
    {
        get => mtuBytes;
        init => mtuBytes = ToriiVpnDirectMetadata.RequirePositive(value, nameof(MtuBytes));
    }

    [JsonPropertyName("meter_family")]
    public string MeterFamily
    {
        get => meterFamily;
        init => meterFamily = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(MeterFamily));
    }

    [JsonPropertyName("flow_label_bits")]
    public byte FlowLabelBits { get; init; }

    [JsonPropertyName("padding_budget_ms")]
    public ushort PaddingBudgetMilliseconds { get; init; }

    [JsonPropertyName("relay_id_hex")]
    public string RelayIdHex
    {
        get => relayIdHex;
        init => relayIdHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(RelayIdHex), 32);
    }

    [JsonPropertyName("descriptor_commit_hex")]
    public string DescriptorCommitHex
    {
        get => descriptorCommitHex;
        init => descriptorCommitHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(DescriptorCommitHex), 32);
    }

    [JsonPropertyName("tls_server_name")]
    public string TlsServerName
    {
        get => tlsServerName;
        init => tlsServerName = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(TlsServerName));
    }

    [JsonPropertyName("relay_tls_spki_sha256_hex")]
    public string RelayTlsSpkiSha256Hex
    {
        get => relayTlsSpkiSha256Hex;
        init => relayTlsSpkiSha256Hex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(RelayTlsSpkiSha256Hex), 32);
    }

    [JsonPropertyName("relay_certificate_sha256_hex")]
    public string RelayCertificateSha256Hex
    {
        get => relayCertificateSha256Hex;
        init => relayCertificateSha256Hex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(RelayCertificateSha256Hex), 32);
    }

    [JsonPropertyName("directory_snapshot_digest_hex")]
    public string DirectorySnapshotDigestHex
    {
        get => directorySnapshotDigestHex;
        init => directorySnapshotDigestHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(DirectorySnapshotDigestHex), 32);
    }

    [JsonPropertyName("metering_public_key_hex")]
    public string MeteringPublicKeyHex
    {
        get => meteringPublicKeyHex;
        init => meteringPublicKeyHex = ToriiVpnDirectMetadata.RequireExactSizedHex(
            value,
            nameof(MeteringPublicKeyHex),
            32);
    }

    [JsonPropertyName("open_lease_instruction")]
    public ToriiVpnTxInstruction OpenLeaseInstruction
    {
        get => openLeaseInstruction;
        init => openLeaseInstruction = ToriiVpnDirectMetadata.RequireVpnTxInstruction(
            value,
            nameof(OpenLeaseInstruction));
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiVpnSessionCreateRequest
{
    [JsonPropertyName("exit_class")]
    public string ExitClass { get; init; } = string.Empty;

    [JsonPropertyName("quote_id")]
    public string QuoteId { get; init; } = string.Empty;

    [JsonPropertyName("payment_tx_hash")]
    public string PaymentTransactionHash { get; init; } = string.Empty;

    [JsonPropertyName("metering_public_key_hex")]
    public string MeteringPublicKeyHex { get; init; } = string.Empty;
}

[JsonConverter(typeof(ToriiVpnSessionJsonConverter))]
public sealed record class ToriiVpnSession
{
    private string sessionId = string.Empty;
    private string accountId = string.Empty;
    private string exitClass = string.Empty;
    private string relayEndpoint = string.Empty;
    private ulong leaseSeconds;
    private ulong expiresAtMilliseconds;
    private ulong connectedAtMilliseconds;
    private string meterFamily = string.Empty;
    private string quoteId = string.Empty;
    private string paymentReference = string.Empty;
    private string paymentTransactionHash = string.Empty;
    private string feeAssetId = string.Empty;
    private string escrowAccountId = string.Empty;
    private string operatorAccountId = string.Empty;
    private string leaseFee = string.Empty;
    private string relayIdHex = string.Empty;
    private string descriptorCommitHex = string.Empty;
    private string tlsServerName = string.Empty;
    private string relayTlsSpkiSha256Hex = string.Empty;
    private string relayCertificateSha256Hex = string.Empty;
    private string directorySnapshotDigestHex = string.Empty;
    private string[]? routePushes = Array.Empty<string>();
    private string[]? excludedRoutes = Array.Empty<string>();
    private string[]? dnsServers = Array.Empty<string>();
    private string[]? tunnelAddresses = Array.Empty<string>();
    private ulong mtuBytes;
    private string helperTicketHex = string.Empty;
    private string status = string.Empty;

    [JsonPropertyName("session_id")]
    public string SessionId
    {
        get => sessionId;
        init => sessionId = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(SessionId), 32);
    }

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("exit_class")]
    public string ExitClass
    {
        get => exitClass;
        init => exitClass = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(ExitClass));
    }

    [JsonPropertyName("relay_endpoint")]
    public string RelayEndpoint
    {
        get => relayEndpoint;
        init => relayEndpoint = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(RelayEndpoint));
    }

    [JsonPropertyName("lease_secs")]
    public ulong LeaseSeconds
    {
        get => leaseSeconds;
        init => leaseSeconds = ToriiVpnDirectMetadata.RequirePositive(value, nameof(LeaseSeconds));
    }

    [JsonPropertyName("expires_at_ms")]
    public ulong ExpiresAtMilliseconds
    {
        get => expiresAtMilliseconds;
        init => expiresAtMilliseconds = ToriiVpnDirectMetadata.RequirePositive(value, nameof(ExpiresAtMilliseconds));
    }

    [JsonPropertyName("connected_at_ms")]
    public ulong ConnectedAtMilliseconds
    {
        get => connectedAtMilliseconds;
        init => connectedAtMilliseconds = ToriiVpnDirectMetadata.RequirePositive(
            value,
            nameof(ConnectedAtMilliseconds));
    }

    [JsonPropertyName("meter_family")]
    public string MeterFamily
    {
        get => meterFamily;
        init => meterFamily = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(MeterFamily));
    }

    [JsonPropertyName("quote_id")]
    public string QuoteId
    {
        get => quoteId;
        init => quoteId = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(QuoteId), 32);
    }

    [JsonPropertyName("payment_reference")]
    public string PaymentReference
    {
        get => paymentReference;
        init => paymentReference = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(PaymentReference));
    }

    [JsonPropertyName("payment_tx_hash")]
    public string PaymentTransactionHash
    {
        get => paymentTransactionHash;
        init => paymentTransactionHash = ToriiVpnDirectMetadata.RequireExactSizedHex(
            value,
            nameof(PaymentTransactionHash),
            32);
    }

    [JsonPropertyName("fee_asset_id")]
    public string FeeAssetId
    {
        get => feeAssetId;
        init => feeAssetId = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(FeeAssetId));
    }

    [JsonPropertyName("escrow_account_id")]
    public string EscrowAccountId
    {
        get => escrowAccountId;
        init => escrowAccountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(EscrowAccountId));
    }

    [JsonPropertyName("operator_account_id")]
    public string OperatorAccountId
    {
        get => operatorAccountId;
        init => operatorAccountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(OperatorAccountId));
    }

    [JsonPropertyName("lease_fee")]
    public string LeaseFee
    {
        get => leaseFee;
        init => leaseFee = ToriiQuantityJson.RequireCanonicalQuantity(value, nameof(LeaseFee));
    }

    [JsonPropertyName("flow_label_bits")]
    public byte FlowLabelBits { get; init; }

    [JsonPropertyName("padding_budget_ms")]
    public ushort PaddingBudgetMilliseconds { get; init; }

    [JsonPropertyName("relay_id_hex")]
    public string RelayIdHex
    {
        get => relayIdHex;
        init => relayIdHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(RelayIdHex), 32);
    }

    [JsonPropertyName("descriptor_commit_hex")]
    public string DescriptorCommitHex
    {
        get => descriptorCommitHex;
        init => descriptorCommitHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(DescriptorCommitHex), 32);
    }

    [JsonPropertyName("tls_server_name")]
    public string TlsServerName
    {
        get => tlsServerName;
        init => tlsServerName = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(TlsServerName));
    }

    [JsonPropertyName("relay_tls_spki_sha256_hex")]
    public string RelayTlsSpkiSha256Hex
    {
        get => relayTlsSpkiSha256Hex;
        init => relayTlsSpkiSha256Hex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(RelayTlsSpkiSha256Hex), 32);
    }

    [JsonPropertyName("relay_certificate_sha256_hex")]
    public string RelayCertificateSha256Hex
    {
        get => relayCertificateSha256Hex;
        init => relayCertificateSha256Hex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(RelayCertificateSha256Hex), 32);
    }

    [JsonPropertyName("directory_snapshot_digest_hex")]
    public string DirectorySnapshotDigestHex
    {
        get => directorySnapshotDigestHex;
        init => directorySnapshotDigestHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(DirectorySnapshotDigestHex), 32);
    }

    [JsonPropertyName("route_pushes")]
    public IReadOnlyList<string>? RoutePushes
    {
        get => ToriiListSnapshots.Copy(routePushes);
        init => routePushes = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(RoutePushes));
    }

    [JsonPropertyName("excluded_routes")]
    public IReadOnlyList<string>? ExcludedRoutes
    {
        get => ToriiListSnapshots.Copy(excludedRoutes);
        init => excludedRoutes = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(ExcludedRoutes));
    }

    [JsonPropertyName("dns_servers")]
    public IReadOnlyList<string>? DnsServers
    {
        get => ToriiListSnapshots.Copy(dnsServers);
        init => dnsServers = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(DnsServers));
    }

    [JsonPropertyName("tunnel_addresses")]
    public IReadOnlyList<string>? TunnelAddresses
    {
        get => ToriiListSnapshots.Copy(tunnelAddresses);
        init => tunnelAddresses = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(TunnelAddresses));
    }

    [JsonPropertyName("mtu_bytes")]
    public ulong MtuBytes
    {
        get => mtuBytes;
        init => mtuBytes = ToriiVpnDirectMetadata.RequirePositive(value, nameof(MtuBytes));
    }

    [JsonPropertyName("helper_ticket_hex")]
    public string HelperTicketHex
    {
        get => helperTicketHex;
        init => helperTicketHex = ToriiVpnDirectMetadata.RequireExactSizedHex(
            value,
            nameof(HelperTicketHex),
            ToriiVpnDirectMetadata.HelperTicketByteLength);
    }

    [JsonPropertyName("bytes_in")]
    public ulong BytesIn { get; init; }

    [JsonPropertyName("bytes_out")]
    public ulong BytesOut { get; init; }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(Status));
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record class ToriiVpnReceiptSubmitRequest
{
    [JsonPropertyName("relay_receipt_hex")]
    public string RelayReceiptHex { get; init; } = string.Empty;

    [JsonPropertyName("client_voucher_hex")]
    public string ClientVoucherHex { get; init; } = string.Empty;

    [JsonPropertyName("lease_id_hex")]
    public string LeaseIdHex { get; init; } = string.Empty;
}

[JsonConverter(typeof(ToriiVpnReceiptJsonConverter))]
public sealed record class ToriiVpnReceipt
{
    private string sessionId = string.Empty;
    private string accountId = string.Empty;
    private string exitClass = string.Empty;
    private string relayEndpoint = string.Empty;
    private string meterFamily = string.Empty;
    private ulong connectedAtMilliseconds;
    private ulong disconnectedAtMilliseconds;
    private string status = string.Empty;
    private string receiptSource = string.Empty;
    private string quoteId = string.Empty;
    private string paymentTransactionHash = string.Empty;
    private string feeAssetId = string.Empty;
    private string escrowAccountId = string.Empty;
    private string operatorAccountId = string.Empty;
    private string leaseFee = string.Empty;
    private string earnedFee = string.Empty;
    private string refundedFee = string.Empty;
    private string leaseIdHex = string.Empty;
    private ToriiVpnTxInstruction? settleLeaseInstruction;

    [JsonPropertyName("session_id")]
    public string SessionId
    {
        get => sessionId;
        init => sessionId = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(SessionId), 32);
    }

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("exit_class")]
    public string ExitClass
    {
        get => exitClass;
        init => exitClass = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(ExitClass));
    }

    [JsonPropertyName("relay_endpoint")]
    public string RelayEndpoint
    {
        get => relayEndpoint;
        init => relayEndpoint = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(RelayEndpoint));
    }

    [JsonPropertyName("meter_family")]
    public string MeterFamily
    {
        get => meterFamily;
        init => meterFamily = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(MeterFamily));
    }

    [JsonPropertyName("connected_at_ms")]
    public ulong ConnectedAtMilliseconds
    {
        get => connectedAtMilliseconds;
        init => connectedAtMilliseconds = ToriiVpnDirectMetadata.RequirePositive(
            value,
            nameof(ConnectedAtMilliseconds));
    }

    [JsonPropertyName("disconnected_at_ms")]
    public ulong DisconnectedAtMilliseconds
    {
        get => disconnectedAtMilliseconds;
        init => disconnectedAtMilliseconds = ToriiVpnDirectMetadata.RequirePositive(
            value,
            nameof(DisconnectedAtMilliseconds));
    }

    [JsonPropertyName("duration_ms")]
    public ulong DurationMilliseconds { get; init; }

    [JsonPropertyName("bytes_in")]
    public ulong BytesIn { get; init; }

    [JsonPropertyName("bytes_out")]
    public ulong BytesOut { get; init; }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(Status));
    }

    [JsonPropertyName("receipt_source")]
    public string ReceiptSource
    {
        get => receiptSource;
        init => receiptSource = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(ReceiptSource));
    }

    [JsonPropertyName("quote_id")]
    public string QuoteId
    {
        get => quoteId;
        init => quoteId = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(QuoteId), 32);
    }

    [JsonPropertyName("payment_tx_hash")]
    public string PaymentTransactionHash
    {
        get => paymentTransactionHash;
        init => paymentTransactionHash = ToriiVpnDirectMetadata.RequireExactSizedHex(
            value,
            nameof(PaymentTransactionHash),
            32);
    }

    [JsonPropertyName("fee_asset_id")]
    public string FeeAssetId
    {
        get => feeAssetId;
        init => feeAssetId = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(FeeAssetId));
    }

    [JsonPropertyName("escrow_account_id")]
    public string EscrowAccountId
    {
        get => escrowAccountId;
        init => escrowAccountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(EscrowAccountId));
    }

    [JsonPropertyName("operator_account_id")]
    public string OperatorAccountId
    {
        get => operatorAccountId;
        init => operatorAccountId = ToriiVpnDirectMetadata.RequireCanonicalAccountId(value, nameof(OperatorAccountId));
    }

    [JsonPropertyName("lease_fee")]
    public string LeaseFee
    {
        get => leaseFee;
        init => leaseFee = ToriiQuantityJson.RequireCanonicalQuantity(value, nameof(LeaseFee));
    }

    [JsonPropertyName("earned_fee")]
    public string EarnedFee
    {
        get => earnedFee;
        init => earnedFee = ToriiQuantityJson.RequireCanonicalQuantity(value, nameof(EarnedFee));
    }

    [JsonPropertyName("refunded_fee")]
    public string RefundedFee
    {
        get => refundedFee;
        init => refundedFee = ToriiQuantityJson.RequireCanonicalQuantity(value, nameof(RefundedFee));
    }

    [JsonPropertyName("lease_id_hex")]
    public string LeaseIdHex
    {
        get => leaseIdHex;
        init => leaseIdHex = ToriiVpnDirectMetadata.RequireExactSizedHex(value, nameof(LeaseIdHex), 32);
    }

    [JsonPropertyName("settle_lease_instruction")]
    public ToriiVpnTxInstruction? SettleLeaseInstruction
    {
        get => settleLeaseInstruction;
        init => settleLeaseInstruction = ToriiVpnDirectMetadata.RequireOptionalVpnTxInstruction(
            value,
            nameof(SettleLeaseInstruction));
    }
}

[JsonConverter(typeof(ToriiVpnReceiptListResponseJsonConverter))]
public sealed record class ToriiVpnReceiptListResponse
{
    private ToriiVpnReceipt[] items = Array.Empty<ToriiVpnReceipt>();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiVpnReceipt> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiVpnDirectMetadata.CopyRequiredVpnReceipts(value, nameof(Items));
    }

    [JsonPropertyName("total")]
    public ulong Total { get; init; }
}

internal static class ToriiVpnDirectMetadata
{
    internal const int HelperTicketByteLength = 696;

    internal static ulong RequirePositive(ulong value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequirePositive(value, paramName);
    }

    internal static ulong RequireAtLeast(ulong value, ulong minimum, string paramName)
    {
        if (value < minimum)
        {
            throw new ArgumentOutOfRangeException(
                paramName,
                value,
                $"Value must be at least {minimum}.");
        }

        return value;
    }

    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string RequireEmptyOrExactTokenText(string? value, string paramName)
    {
        if (value is null)
        {
            throw new ArgumentNullException(paramName);
        }

        return value.Length == 0 ? value : RequireExactTokenText(value, paramName);
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, paramName);
    }

    internal static string RequireExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, expectedBytes);
    }

    internal static string RequireEmptyOrExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        if (value is null)
        {
            throw new ArgumentNullException(paramName);
        }

        return value.Length == 0 ? value : RequireExactSizedHex(value, paramName, expectedBytes);
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(value, paramName, expectedBytes);
    }

    internal static string RequireExactEvenLengthHex(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value must be a non-empty even-length hex string.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new ArgumentException("Value must not contain whitespace.", paramName);
            }

            if (char.IsControl(character))
            {
                throw new ArgumentException("Value must not contain control characters.", paramName);
            }
        }

        if (value.Length % 2 != 0 || !value.All(static character => character is (>= '0' and <= '9') or (>= 'a' and <= 'f')))
        {
            throw new ArgumentException("Value must be an exact lowercase even-length hex string.", paramName);
        }

        return value;
    }

    internal static string[]? CopyOptionalExactTokenTextList(IReadOnlyList<string>? values, string paramName)
    {
        if (values is null)
        {
            return null;
        }

        var copy = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            var value = values[index];
            if (value is null)
            {
                throw new ArgumentException("List elements must not be null.", $"{paramName}[{index}]");
            }

            copy[index] = RequireExactTokenText(value, $"{paramName}[{index}]");
        }

        return copy;
    }

    internal static ToriiVpnTxInstruction? RequireOptionalVpnTxInstruction(
        ToriiVpnTxInstruction? value,
        string paramName)
    {
        return value is null ? null : RequireVpnTxInstruction(value, paramName);
    }

    internal static ToriiVpnReceipt[] CopyRequiredVpnReceipts(
        IReadOnlyList<ToriiVpnReceipt>? values,
        string paramName)
    {
        if (values is null)
        {
            return Array.Empty<ToriiVpnReceipt>();
        }

        var copy = new ToriiVpnReceipt[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            var value = values[index];
            if (value is null)
            {
                throw new ArgumentException("List elements must not be null.", $"{paramName}[{index}]");
            }

            copy[index] = RequireVpnReceipt(value, $"{paramName}[{index}]");
        }

        return copy;
    }

    internal static ToriiVpnTxInstruction RequireVpnTxInstruction(ToriiVpnTxInstruction value, string paramName)
    {
        ArgumentNullException.ThrowIfNull(value, paramName);
        RequireExactNonEmptyText(value.WireId, $"{paramName}.{nameof(ToriiVpnTxInstruction.WireId)}");
        RequireExactEvenLengthHex(value.PayloadHex, $"{paramName}.{nameof(ToriiVpnTxInstruction.PayloadHex)}");
        return value;
    }

    private static ToriiVpnReceipt RequireVpnReceipt(ToriiVpnReceipt value, string paramName)
    {
        RequireExactSizedHex(value.SessionId, $"{paramName}.{nameof(ToriiVpnReceipt.SessionId)}", 32);
        RequireCanonicalAccountId(value.AccountId, $"{paramName}.{nameof(ToriiVpnReceipt.AccountId)}");
        RequireExactTokenText(value.ExitClass, $"{paramName}.{nameof(ToriiVpnReceipt.ExitClass)}");
        RequireExactTokenText(value.RelayEndpoint, $"{paramName}.{nameof(ToriiVpnReceipt.RelayEndpoint)}");
        RequireExactTokenText(value.MeterFamily, $"{paramName}.{nameof(ToriiVpnReceipt.MeterFamily)}");
        RequirePositive(value.ConnectedAtMilliseconds, $"{paramName}.{nameof(ToriiVpnReceipt.ConnectedAtMilliseconds)}");
        RequirePositive(value.DisconnectedAtMilliseconds, $"{paramName}.{nameof(ToriiVpnReceipt.DisconnectedAtMilliseconds)}");
        RequireExactTokenText(value.Status, $"{paramName}.{nameof(ToriiVpnReceipt.Status)}");
        RequireExactTokenText(value.ReceiptSource, $"{paramName}.{nameof(ToriiVpnReceipt.ReceiptSource)}");
        RequireExactSizedHex(value.QuoteId, $"{paramName}.{nameof(ToriiVpnReceipt.QuoteId)}", 32);
        RequireExactSizedHex(value.PaymentTransactionHash, $"{paramName}.{nameof(ToriiVpnReceipt.PaymentTransactionHash)}", 32);
        RequireExactTokenText(value.FeeAssetId, $"{paramName}.{nameof(ToriiVpnReceipt.FeeAssetId)}");
        RequireCanonicalAccountId(value.EscrowAccountId, $"{paramName}.{nameof(ToriiVpnReceipt.EscrowAccountId)}");
        RequireCanonicalAccountId(value.OperatorAccountId, $"{paramName}.{nameof(ToriiVpnReceipt.OperatorAccountId)}");
        RequireExactSizedHex(value.LeaseIdHex, $"{paramName}.{nameof(ToriiVpnReceipt.LeaseIdHex)}", 32);
        RequireOptionalVpnTxInstruction(value.SettleLeaseInstruction, $"{paramName}.{nameof(ToriiVpnReceipt.SettleLeaseInstruction)}");
        return value;
    }
}

[JsonConverter(typeof(ToriiAccountSummaryJsonConverter))]
public sealed record class ToriiAccountSummary
{
    private string id = string.Empty;

    [JsonPropertyName("id")]
    public string Id
    {
        get => id;
        init => id = ToriiAccountQueryDirectMetadata.RequireCanonicalAccountId(value, nameof(Id));
    }
}

[JsonConverter(typeof(ToriiAccountsPageJsonConverter))]
public sealed record class ToriiAccountsPage
{
    private ToriiAccountSummary[] items = Array.Empty<ToriiAccountSummary>();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAccountSummary> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

[JsonConverter(typeof(ToriiAssetBalanceJsonConverter))]
public sealed record class ToriiAssetBalance
{
    private string asset = string.Empty;
    private string accountId = string.Empty;
    private string scope = string.Empty;
    private string assetName = string.Empty;
    private string? assetAlias;
    private string quantity = string.Empty;

    [JsonPropertyName("asset")]
    public string Asset
    {
        get => asset;
        init => asset = ToriiAccountQueryDirectMetadata.RequireExactNonEmptyText(value, nameof(Asset));
    }

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiAccountQueryDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("scope")]
    public string Scope
    {
        get => scope;
        init => scope = ToriiAccountQueryDirectMetadata.RequireExactNonEmptyText(value, nameof(Scope));
    }

    [JsonPropertyName("asset_name")]
    public string AssetName
    {
        get => assetName;
        init => assetName = ToriiAccountQueryDirectMetadata.RequireExactNonEmptyText(value, nameof(AssetName));
    }

    [JsonPropertyName("asset_alias")]
    public string? AssetAlias
    {
        get => assetAlias;
        init => assetAlias = ToriiAccountQueryDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(AssetAlias));
    }

    [JsonPropertyName("quantity")]
    public string Quantity
    {
        get => quantity;
        init => quantity = ToriiAccountQueryDirectMetadata.RequireCanonicalQuantityText(
            value,
            nameof(Quantity));
    }
}

[JsonConverter(typeof(ToriiAssetAliasBindingJsonConverter))]
public sealed record class ToriiAssetAliasBinding
{
    private string alias = string.Empty;
    private string status = string.Empty;
    private long? leaseExpiryMilliseconds;
    private long? graceUntilMilliseconds;
    private long boundAtMilliseconds;

    [JsonPropertyName("alias")]
    public string Alias
    {
        get => alias;
        init => alias = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Alias));
    }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Status));
    }

    [JsonPropertyName("lease_expiry_ms")]
    public long? LeaseExpiryMilliseconds
    {
        get => leaseExpiryMilliseconds;
        init => leaseExpiryMilliseconds = ToriiAliasDirectMetadata.RequireOptionalPositive(
            value,
            nameof(LeaseExpiryMilliseconds));
    }

    [JsonPropertyName("grace_until_ms")]
    public long? GraceUntilMilliseconds
    {
        get => graceUntilMilliseconds;
        init => graceUntilMilliseconds = ToriiAliasDirectMetadata.RequireOptionalPositive(
            value,
            nameof(GraceUntilMilliseconds));
    }

    [JsonPropertyName("bound_at_ms")]
    public long BoundAtMilliseconds
    {
        get => boundAtMilliseconds;
        init => boundAtMilliseconds = ToriiAliasDirectMetadata.RequirePositive(value, nameof(BoundAtMilliseconds));
    }
}

[JsonConverter(typeof(ToriiAssetAliasResolutionJsonConverter))]
public sealed record class ToriiAssetAliasResolution
{
    private string alias = string.Empty;
    private string assetDefinitionId = string.Empty;
    private string assetName = string.Empty;
    private string? description;
    private string? logo;
    private string? source;

    [JsonPropertyName("alias")]
    public string Alias
    {
        get => alias;
        init => alias = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Alias));
    }

    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId
    {
        get => assetDefinitionId;
        init => assetDefinitionId = ToriiAliasDirectMetadata.RequireExactTokenText(
            value,
            nameof(AssetDefinitionId));
    }

    [JsonPropertyName("asset_name")]
    public string AssetName
    {
        get => assetName;
        init => assetName = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(AssetName));
    }

    [JsonPropertyName("alias_binding")]
    public ToriiAssetAliasBinding? AliasBinding { get; init; }

    [JsonPropertyName("description")]
    public string? Description
    {
        get => description;
        init => description = ToriiAliasDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(Description));
    }

    [JsonPropertyName("logo")]
    public string? Logo
    {
        get => logo;
        init => logo = ToriiAliasDirectMetadata.RequireOptionalExactTokenText(value, nameof(Logo));
    }

    [JsonPropertyName("source")]
    public string? Source
    {
        get => source;
        init => source = ToriiAliasDirectMetadata.RequireOptionalExactTokenText(value, nameof(Source));
    }
}

[JsonConverter(typeof(ToriiAssetBalancesPageJsonConverter))]
public sealed record class ToriiAssetBalancesPage
{
    private ToriiAssetBalance[] items = Array.Empty<ToriiAssetBalance>();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAssetBalance> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

[JsonConverter(typeof(ToriiAccountAliasResolutionJsonConverter))]
public sealed record class ToriiAccountAliasResolution
{
    private string alias = string.Empty;
    private string accountId = string.Empty;
    private long? index;
    private string? source;

    [JsonPropertyName("alias")]
    public string Alias
    {
        get => alias;
        init => alias = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Alias));
    }

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiAliasDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("index")]
    public long? Index
    {
        get => index;
        init => index = ToriiAliasDirectMetadata.RequireOptionalNonNegative(value, nameof(Index));
    }

    [JsonPropertyName("source")]
    public string? Source
    {
        get => source;
        init => source = ToriiAliasDirectMetadata.RequireOptionalExactTokenText(value, nameof(Source));
    }
}

[JsonConverter(typeof(ToriiAccountAliasIndexResolutionJsonConverter))]
public sealed record class ToriiAccountAliasIndexResolution
{
    private string alias = string.Empty;
    private string accountId = string.Empty;
    private string? source;

    [JsonPropertyName("index")]
    public ulong Index { get; init; }

    [JsonPropertyName("alias")]
    public string Alias
    {
        get => alias;
        init => alias = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Alias));
    }

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiAliasDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("source")]
    public string? Source
    {
        get => source;
        init => source = ToriiAliasDirectMetadata.RequireOptionalExactTokenText(value, nameof(Source));
    }
}

[JsonConverter(typeof(ToriiAccountAliasLookupItemJsonConverter))]
public sealed record class ToriiAccountAliasLookupItem
{
    private string alias = string.Empty;
    private string dataspace = string.Empty;
    private string? domain;

    [JsonPropertyName("alias")]
    public string Alias
    {
        get => alias;
        init => alias = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Alias));
    }

    [JsonPropertyName("dataspace")]
    public string Dataspace
    {
        get => dataspace;
        init => dataspace = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Dataspace));
    }

    [JsonPropertyName("domain")]
    public string? Domain
    {
        get => domain;
        init => domain = ToriiAliasDirectMetadata.RequireOptionalExactTokenText(value, nameof(Domain));
    }

    [JsonPropertyName("is_primary")]
    public bool IsPrimary { get; init; }
}

[JsonConverter(typeof(ToriiAccountAliasLookupResponseJsonConverter))]
public sealed record class ToriiAccountAliasLookupResponse
{
    private ToriiAccountAliasLookupItem[] items = Array.Empty<ToriiAccountAliasLookupItem>();
    private string accountId = string.Empty;
    private string? source;

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiAliasDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAccountAliasLookupItem> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("total")]
    public long Total { get; init; }

    [JsonPropertyName("source")]
    public string? Source
    {
        get => source;
        init => source = ToriiAliasDirectMetadata.RequireOptionalExactTokenText(value, nameof(Source));
    }
}

[JsonConverter(typeof(ToriiAccountPermissionJsonConverter))]
public sealed record class ToriiAccountPermission
{
    private string name = string.Empty;
    private JsonNode? payload;

    [JsonPropertyName("name")]
    public string Name
    {
        get => name;
        init => name = ToriiAccountQueryDirectMetadata.RequireExactNonEmptyText(value, nameof(Name));
    }

    [JsonPropertyName("payload")]
    public JsonNode? Payload
    {
        get => ToriiJsonSnapshots.Copy(payload);
        init => payload = ToriiJsonSnapshots.Copy(value);
    }
}

[JsonConverter(typeof(ToriiAccountPermissionsPageJsonConverter))]
public sealed record class ToriiAccountPermissionsPage
{
    private ToriiAccountPermission[] items = Array.Empty<ToriiAccountPermission>();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiAccountPermission> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

[JsonConverter(typeof(ToriiTransactionSummaryJsonConverter))]
public sealed record class ToriiTransactionSummary
{
    private string? authority;
    private long? timestampMilliseconds;
    private string entrypointHash = string.Empty;

    [JsonPropertyName("authority")]
    public string? Authority
    {
        get => authority;
        init => authority = ToriiAccountQueryDirectMetadata.RequireOptionalCanonicalAccountId(
            value,
            nameof(Authority));
    }

    [JsonPropertyName("timestamp_ms")]
    public long? TimestampMilliseconds
    {
        get => timestampMilliseconds;
        init => timestampMilliseconds = ToriiAccountQueryDirectMetadata.RequireOptionalPositive(
            value,
            nameof(TimestampMilliseconds));
    }

    [JsonPropertyName("entrypoint_hash")]
    public string EntrypointHash
    {
        get => entrypointHash;
        init => entrypointHash = ToriiAccountQueryDirectMetadata.RequireExactSizedHex(
            value,
            nameof(EntrypointHash),
            32);
    }

    [JsonPropertyName("result_ok")]
    public bool ResultOk { get; init; }
}

internal static class ToriiAccountQueryDirectMetadata
{
    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static string? RequireOptionalCanonicalAccountId(string? value, string paramName)
    {
        return value is null ? null : RequireCanonicalAccountId(value, paramName);
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, paramName);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactNonEmptyText(value, paramName);
    }

    internal static string RequireCanonicalQuantityText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, paramName);
    }

    internal static string RequireExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, expectedBytes);
    }

    internal static long? RequireOptionalPositive(long? value, string paramName)
    {
        if (value is long integer && integer <= 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value;
    }
}

internal static class ToriiAliasDirectMetadata
{
    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string? RequireOptionalExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, paramName);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactNonEmptyText(value, paramName);
    }

    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static long RequirePositive(long value, string paramName)
    {
        if (value <= 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value;
    }

    internal static long? RequireOptionalPositive(long? value, string paramName)
    {
        return value is null ? null : RequirePositive(value.Value, paramName);
    }

    internal static long? RequireOptionalNonNegative(long? value, string paramName)
    {
        if (value is long integer && integer < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be non-negative.");
        }

        return value;
    }
}

internal static class ToriiOnboardingDirectMetadata
{
    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string RequireCanonicalQuantityText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, paramName);
    }

    internal static string RequireOptionalTransactionHashHex(string? value, string paramName)
    {
        if (string.IsNullOrEmpty(value))
        {
            return string.Empty;
        }

        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }

    internal static string RequireFaucetAlgorithm(string? value, string paramName)
    {
        var exact = RequireExactTokenText(value, paramName);
        if (!string.Equals(exact, ToriiAccountFaucetPow.Algorithm, StringComparison.Ordinal))
        {
            throw new ArgumentException($"Value must be {ToriiAccountFaucetPow.Algorithm}.", paramName);
        }

        return exact;
    }

    internal static ulong RequirePositive(ulong value, string paramName)
    {
        if (value == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value;
    }

    internal static string? RequireOptionalExactEvenLengthHex(string? value, string paramName)
    {
        if (value is null)
        {
            return null;
        }

        if (value.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty even-length hex string.", paramName);
        }

        var exact = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (exact.Length % 2 != 0 || !exact.All(IsLowercaseHexCharacter))
        {
            throw new ArgumentException("Value must be an exact lowercase even-length hex string.", paramName);
        }

        return exact;
    }

    internal static void RequireCheckedScryptParameters(
        byte scryptLogN,
        uint scryptR,
        uint scryptP,
        string paramName)
    {
        try
        {
            _ = ToriiAccountFaucetPow.CheckedScryptParameters(scryptLogN, scryptR, scryptP);
        }
        catch (ArgumentOutOfRangeException exception)
        {
            var directParamName = exception.ParamName switch
            {
                "scryptR" => nameof(ToriiAccountFaucetPuzzle.ScryptR),
                "scryptP" => nameof(ToriiAccountFaucetPuzzle.ScryptP),
                _ => paramName,
            };
            throw new ArgumentOutOfRangeException(directParamName, exception.Message);
        }
    }

    private static bool IsLowercaseHexCharacter(char value)
    {
        return value is (>= '0' and <= '9') or (>= 'a' and <= 'f');
    }
}

public record class ToriiExplorerPaginationQuery
{
    public ulong? Page { get; init; }

    public ulong? PerPage { get; init; }
}

public record class ToriiExplorerCursorQuery
{
    public string? Cursor { get; init; }

    public uint? Limit { get; init; }
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
    private string[]? paths;

    public string? Path { get; init; }

    public IReadOnlyList<string>? Paths
    {
        get => ToriiListSnapshots.Copy(paths);
        init => paths = ToriiListSnapshots.Copy(value);
    }

    public string? Prefix { get; init; }

    public bool? IncludeValue { get; init; }

    public ulong? Offset { get; init; }

    public ulong? Limit { get; init; }

    public string? Decode { get; init; }
}

public sealed record class ToriiExplorerAccountsQuery : ToriiExplorerCursorQuery
{
    public string? Domain { get; init; }

    public string? WithAsset { get; init; }
}

public sealed record class ToriiExplorerDomainsQuery : ToriiExplorerCursorQuery
{
    public string? OwnedBy { get; init; }
}

public sealed record class ToriiExplorerAssetDefinitionsQuery : ToriiExplorerCursorQuery
{
    public string? OwningDomain { get; init; }

    public string? OwnedBy { get; init; }
}

public sealed record class ToriiExplorerAssetsQuery : ToriiExplorerCursorQuery
{
    public string? OwnedBy { get; init; }

    public string? Definition { get; init; }

    public string? AssetId { get; init; }
}

public sealed record class ToriiExplorerNftsQuery : ToriiExplorerCursorQuery
{
    public string? OwnedBy { get; init; }

    public string? Domain { get; init; }
}

public sealed record class ToriiExplorerRwasQuery : ToriiExplorerCursorQuery
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

[JsonConverter(typeof(ToriiExplorerPaginationMetaJsonConverter))]
public sealed record class ToriiExplorerPaginationMeta
{
    private ulong page;
    private ulong perPage;

    [JsonPropertyName("page")]
    public ulong Page
    {
        get => page;
        init => page = ToriiExplorerDirectMetadata.RequirePositive(value, nameof(Page));
    }

    [JsonPropertyName("per_page")]
    public ulong PerPage
    {
        get => perPage;
        init => perPage = ToriiExplorerDirectMetadata.RequirePositive(value, nameof(PerPage));
    }

    [JsonPropertyName("total_pages")]
    public ulong TotalPages { get; init; }

    [JsonPropertyName("total_items")]
    public ulong TotalItems { get; init; }
}

[JsonConverter(typeof(ToriiExplorerCursorMetaJsonConverter))]
public sealed record class ToriiExplorerCursorMeta
{
    private uint limit;
    private string? nextCursor;

    [JsonPropertyName("limit")]
    public uint Limit
    {
        get => limit;
        init => limit = ToriiExplorerDirectMetadata.RequireExplorerCursorLimit(value, nameof(Limit));
    }

    [JsonPropertyName("next_cursor")]
    public string? NextCursor
    {
        get => nextCursor;
        init => nextCursor = ToriiExplorerDirectMetadata.RequireOptionalCanonicalExplorerCursor(
            value,
            nameof(NextCursor));
    }

    [JsonPropertyName("has_more")]
    public bool HasMore { get; init; }
}

[JsonConverter(typeof(ToriiExplorerAccountJsonConverter))]
public sealed record class ToriiExplorerAccount
{
    private string id = string.Empty;
    private string i105Address = string.Empty;
    private JsonNode? metadata = new JsonObject();

    [JsonPropertyName("id")]
    public string Id
    {
        get => id;
        init => id = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(Id));
    }

    [JsonPropertyName("i105_address")]
    public string I105Address
    {
        get => i105Address;
        init => i105Address = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(I105Address));
    }

    [JsonPropertyName("network_prefix")]
    public ushort NetworkPrefix { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata
    {
        get => ToriiJsonSnapshots.Copy(metadata);
        init => metadata = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("owned_domains")]
    public uint OwnedDomains { get; init; }

    [JsonPropertyName("owned_assets")]
    public uint OwnedAssets { get; init; }

    [JsonPropertyName("owned_nfts")]
    public uint OwnedNfts { get; init; }
}

[JsonConverter(typeof(ToriiExplorerAccountsPageJsonConverter))]
public sealed record class ToriiExplorerAccountsPage
{
    private ToriiExplorerAccount[] items = Array.Empty<ToriiExplorerAccount>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerCursorMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerAccount> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerDomainJsonConverter))]
public sealed record class ToriiExplorerDomain
{
    private string id = string.Empty;
    private string? logo;
    private string ownedBy = string.Empty;
    private JsonNode? metadata = new JsonObject();

    [JsonPropertyName("id")]
    public string Id
    {
        get => id;
        init => id = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Id));
    }

    [JsonPropertyName("logo")]
    public string? Logo
    {
        get => logo;
        init => logo = ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, nameof(Logo));
    }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata
    {
        get => ToriiJsonSnapshots.Copy(metadata);
        init => metadata = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("owned_by")]
    public string OwnedBy
    {
        get => ownedBy;
        init => ownedBy = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(OwnedBy));
    }

    [JsonPropertyName("accounts")]
    public uint Accounts { get; init; }

    [JsonPropertyName("assets")]
    public uint Assets { get; init; }

    [JsonPropertyName("nfts")]
    public uint Nfts { get; init; }
}

[JsonConverter(typeof(ToriiExplorerDomainsPageJsonConverter))]
public sealed record class ToriiExplorerDomainsPage
{
    private ToriiExplorerDomain[] items = Array.Empty<ToriiExplorerDomain>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerCursorMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerDomain> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerAssetDefinitionJsonConverter))]
public sealed record class ToriiExplorerAssetDefinition
{
    private string id = string.Empty;
    private string? owningDomain;
    private string mintable = string.Empty;
    private string? logo;
    private string ownedBy = string.Empty;
    private string totalQuantity = string.Empty;
    private string? lockedQuantity;
    private string? circulatingQuantity;
    private JsonNode? metadata = new JsonObject();

    [JsonPropertyName("id")]
    public string Id
    {
        get => id;
        init => id = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Id));
    }

    [JsonPropertyName("owning_domain")]
    public string? OwningDomain
    {
        get => owningDomain;
        init => owningDomain = ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(
            value,
            nameof(OwningDomain));
    }

    [JsonPropertyName("mintable")]
    public string Mintable
    {
        get => mintable;
        init => mintable = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Mintable));
    }

    [JsonPropertyName("logo")]
    public string? Logo
    {
        get => logo;
        init => logo = ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, nameof(Logo));
    }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata
    {
        get => ToriiJsonSnapshots.Copy(metadata);
        init => metadata = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("owned_by")]
    public string OwnedBy
    {
        get => ownedBy;
        init => ownedBy = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(OwnedBy));
    }

    [JsonPropertyName("assets")]
    public uint Assets { get; init; }

    [JsonPropertyName("total_quantity")]
    public string TotalQuantity
    {
        get => totalQuantity;
        init => totalQuantity = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(
            value,
            nameof(TotalQuantity));
    }

    [JsonPropertyName("locked_quantity")]
    public string? LockedQuantity
    {
        get => lockedQuantity;
        init => lockedQuantity = ToriiExplorerDirectMetadata.RequireOptionalCanonicalQuantityText(
            value,
            nameof(LockedQuantity));
    }

    [JsonPropertyName("circulating_quantity")]
    public string? CirculatingQuantity
    {
        get => circulatingQuantity;
        init => circulatingQuantity = ToriiExplorerDirectMetadata.RequireOptionalCanonicalQuantityText(
            value,
            nameof(CirculatingQuantity));
    }
}

[JsonConverter(typeof(ToriiExplorerAssetDefinitionsPageJsonConverter))]
public sealed record class ToriiExplorerAssetDefinitionsPage
{
    private ToriiExplorerAssetDefinition[] items = Array.Empty<ToriiExplorerAssetDefinition>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerCursorMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerAssetDefinition> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerEconometricsVelocityWindowJsonConverter))]
public sealed record class ToriiExplorerEconometricsVelocityWindow
{
    private string key = string.Empty;
    private string amount = string.Empty;

    [JsonPropertyName("key")]
    public string Key
    {
        get => key;
        init => key = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Key));
    }

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
    public string Amount
    {
        get => amount;
        init => amount = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Amount));
    }
}

[JsonConverter(typeof(ToriiExplorerEconometricsIssuanceWindowJsonConverter))]
public sealed record class ToriiExplorerEconometricsIssuanceWindow
{
    private string key = string.Empty;
    private string minted = string.Empty;
    private string burned = string.Empty;
    private string net = string.Empty;

    [JsonPropertyName("key")]
    public string Key
    {
        get => key;
        init => key = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Key));
    }

    [JsonPropertyName("start_ms")]
    public ulong StartMilliseconds { get; init; }

    [JsonPropertyName("end_ms")]
    public ulong EndMilliseconds { get; init; }

    [JsonPropertyName("mint_count")]
    public ulong MintCount { get; init; }

    [JsonPropertyName("burn_count")]
    public ulong BurnCount { get; init; }

    [JsonPropertyName("minted")]
    public string Minted
    {
        get => minted;
        init => minted = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Minted));
    }

    [JsonPropertyName("burned")]
    public string Burned
    {
        get => burned;
        init => burned = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Burned));
    }

    [JsonPropertyName("net")]
    public string Net
    {
        get => net;
        init => net = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Net));
    }
}

[JsonConverter(typeof(ToriiExplorerEconometricsIssuanceSeriesPointJsonConverter))]
public sealed record class ToriiExplorerEconometricsIssuanceSeriesPoint
{
    private string minted = string.Empty;
    private string burned = string.Empty;
    private string net = string.Empty;

    [JsonPropertyName("bucket_start_ms")]
    public ulong BucketStartMilliseconds { get; init; }

    [JsonPropertyName("minted")]
    public string Minted
    {
        get => minted;
        init => minted = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Minted));
    }

    [JsonPropertyName("burned")]
    public string Burned
    {
        get => burned;
        init => burned = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Burned));
    }

    [JsonPropertyName("net")]
    public string Net
    {
        get => net;
        init => net = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Net));
    }
}

[JsonConverter(typeof(ToriiExplorerAssetDefinitionEconometricsJsonConverter))]
public sealed record class ToriiExplorerAssetDefinitionEconometrics
{
    private ToriiExplorerEconometricsVelocityWindow[] velocityWindows = Array.Empty<ToriiExplorerEconometricsVelocityWindow>();
    private ToriiExplorerEconometricsIssuanceWindow[] issuanceWindows = Array.Empty<ToriiExplorerEconometricsIssuanceWindow>();
    private ToriiExplorerEconometricsIssuanceSeriesPoint[] issuanceSeries = Array.Empty<ToriiExplorerEconometricsIssuanceSeriesPoint>();
    private string definitionId = string.Empty;

    [JsonPropertyName("definition_id")]
    public string DefinitionId
    {
        get => definitionId;
        init => definitionId = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(DefinitionId));
    }

    [JsonPropertyName("computed_at_ms")]
    public ulong ComputedAtMilliseconds { get; init; }

    [JsonPropertyName("velocity_windows")]
    public IReadOnlyList<ToriiExplorerEconometricsVelocityWindow> VelocityWindows
    {
        get => ToriiListSnapshots.CopyRequired(velocityWindows);
        init => velocityWindows = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("issuance_windows")]
    public IReadOnlyList<ToriiExplorerEconometricsIssuanceWindow> IssuanceWindows
    {
        get => ToriiListSnapshots.CopyRequired(issuanceWindows);
        init => issuanceWindows = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("issuance_series")]
    public IReadOnlyList<ToriiExplorerEconometricsIssuanceSeriesPoint> IssuanceSeries
    {
        get => ToriiListSnapshots.CopyRequired(issuanceSeries);
        init => issuanceSeries = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerEconometricsLorenzPointJsonConverter))]
public sealed record class ToriiExplorerEconometricsLorenzPoint
{
    private double population;
    private double share;

    [JsonPropertyName("population")]
    public double Population
    {
        get => population;
        init => population = ToriiExplorerDirectMetadata.RequireFiniteUnitIntervalDouble(value, nameof(Population));
    }

    [JsonPropertyName("share")]
    public double Share
    {
        get => share;
        init => share = ToriiExplorerDirectMetadata.RequireFiniteUnitIntervalDouble(value, nameof(Share));
    }
}

[JsonConverter(typeof(ToriiExplorerEconometricsDistributionSnapshotJsonConverter))]
public sealed record class ToriiExplorerEconometricsDistributionSnapshot
{
    private ToriiExplorerEconometricsLorenzPoint[] lorenz = Array.Empty<ToriiExplorerEconometricsLorenzPoint>();
    private double gini;
    private double hhi;
    private double theil;
    private double entropy;
    private double entropyNormalized;
    private double top1;
    private double top5;
    private double top10;
    private string? median;
    private string? p90;
    private string? p99;

    [JsonPropertyName("gini")]
    public double Gini
    {
        get => gini;
        init => gini = ToriiExplorerDirectMetadata.RequireFiniteUnitIntervalDouble(value, nameof(Gini));
    }

    [JsonPropertyName("hhi")]
    public double Hhi
    {
        get => hhi;
        init => hhi = ToriiExplorerDirectMetadata.RequireFiniteNonNegativeDouble(value, nameof(Hhi));
    }

    [JsonPropertyName("theil")]
    public double Theil
    {
        get => theil;
        init => theil = ToriiExplorerDirectMetadata.RequireFiniteNonNegativeDouble(value, nameof(Theil));
    }

    [JsonPropertyName("entropy")]
    public double Entropy
    {
        get => entropy;
        init => entropy = ToriiExplorerDirectMetadata.RequireFiniteNonNegativeDouble(value, nameof(Entropy));
    }

    [JsonPropertyName("entropy_normalized")]
    public double EntropyNormalized
    {
        get => entropyNormalized;
        init => entropyNormalized = ToriiExplorerDirectMetadata.RequireFiniteUnitIntervalDouble(
            value,
            nameof(EntropyNormalized));
    }

    [JsonPropertyName("nakamoto_33")]
    public ulong Nakamoto33 { get; init; }

    [JsonPropertyName("nakamoto_51")]
    public ulong Nakamoto51 { get; init; }

    [JsonPropertyName("nakamoto_67")]
    public ulong Nakamoto67 { get; init; }

    [JsonPropertyName("top1")]
    public double Top1
    {
        get => top1;
        init => top1 = ToriiExplorerDirectMetadata.RequireFiniteUnitIntervalDouble(value, nameof(Top1));
    }

    [JsonPropertyName("top5")]
    public double Top5
    {
        get => top5;
        init => top5 = ToriiExplorerDirectMetadata.RequireFiniteUnitIntervalDouble(value, nameof(Top5));
    }

    [JsonPropertyName("top10")]
    public double Top10
    {
        get => top10;
        init => top10 = ToriiExplorerDirectMetadata.RequireFiniteUnitIntervalDouble(value, nameof(Top10));
    }

    [JsonPropertyName("median")]
    public string? Median
    {
        get => median;
        init => median = ToriiExplorerDirectMetadata.RequireOptionalCanonicalQuantityText(
            value,
            nameof(Median));
    }

    [JsonPropertyName("p90")]
    public string? P90
    {
        get => p90;
        init => p90 = ToriiExplorerDirectMetadata.RequireOptionalCanonicalQuantityText(value, nameof(P90));
    }

    [JsonPropertyName("p99")]
    public string? P99
    {
        get => p99;
        init => p99 = ToriiExplorerDirectMetadata.RequireOptionalCanonicalQuantityText(value, nameof(P99));
    }

    [JsonPropertyName("lorenz")]
    public IReadOnlyList<ToriiExplorerEconometricsLorenzPoint> Lorenz
    {
        get => ToriiListSnapshots.CopyRequired(lorenz);
        init => lorenz = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerEconometricsTopHolderJsonConverter))]
public sealed record class ToriiExplorerEconometricsTopHolder
{
    private string accountId = string.Empty;
    private string balance = string.Empty;

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("balance")]
    public string Balance
    {
        get => balance;
        init => balance = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Balance));
    }
}

[JsonConverter(typeof(ToriiExplorerAssetDefinitionSnapshotJsonConverter))]
public sealed record class ToriiExplorerAssetDefinitionSnapshot
{
    private ToriiExplorerEconometricsTopHolder[] topHolders = Array.Empty<ToriiExplorerEconometricsTopHolder>();
    private string definitionId = string.Empty;
    private string totalSupply = string.Empty;
    private ToriiExplorerEconometricsDistributionSnapshot distribution = new();

    [JsonPropertyName("definition_id")]
    public string DefinitionId
    {
        get => definitionId;
        init => definitionId = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(DefinitionId));
    }

    [JsonPropertyName("computed_at_ms")]
    public ulong ComputedAtMilliseconds { get; init; }

    [JsonPropertyName("holders_total")]
    public ulong HoldersTotal { get; init; }

    [JsonPropertyName("total_supply")]
    public string TotalSupply
    {
        get => totalSupply;
        init => totalSupply = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(
            value,
            nameof(TotalSupply));
    }

    [JsonPropertyName("top_holders")]
    public IReadOnlyList<ToriiExplorerEconometricsTopHolder> TopHolders
    {
        get => ToriiListSnapshots.CopyRequired(topHolders);
        init => topHolders = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("distribution")]
    public ToriiExplorerEconometricsDistributionSnapshot Distribution
    {
        get => distribution;
        init => distribution = value ?? throw new ArgumentNullException(nameof(Distribution));
    }
}

[JsonConverter(typeof(ToriiExplorerAssetJsonConverter))]
public sealed record class ToriiExplorerAsset
{
    private string id = string.Empty;
    private string definitionId = string.Empty;
    private string accountId = string.Empty;
    private string value = string.Empty;

    [JsonPropertyName("id")]
    public string Id
    {
        get => id;
        init => id = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Id));
    }

    [JsonPropertyName("definition_id")]
    public string DefinitionId
    {
        get => definitionId;
        init => definitionId = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(DefinitionId));
    }

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("value")]
    public string Value
    {
        get => value;
        init => this.value = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Value));
    }
}

[JsonConverter(typeof(ToriiExplorerAssetsPageJsonConverter))]
public sealed record class ToriiExplorerAssetsPage
{
    private ToriiExplorerAsset[] items = Array.Empty<ToriiExplorerAsset>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerCursorMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerAsset> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerNftJsonConverter))]
public sealed record class ToriiExplorerNft
{
    private string id = string.Empty;
    private string ownedBy = string.Empty;
    private JsonNode? metadata = new JsonObject();

    [JsonPropertyName("id")]
    public string Id
    {
        get => id;
        init => id = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Id));
    }

    [JsonPropertyName("owned_by")]
    public string OwnedBy
    {
        get => ownedBy;
        init => ownedBy = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(OwnedBy));
    }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata
    {
        get => ToriiJsonSnapshots.Copy(metadata);
        init => metadata = ToriiJsonSnapshots.Copy(value);
    }
}

[JsonConverter(typeof(ToriiExplorerNftsPageJsonConverter))]
public sealed record class ToriiExplorerNftsPage
{
    private ToriiExplorerNft[] items = Array.Empty<ToriiExplorerNft>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerCursorMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerNft> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerRwaParentJsonConverter))]
public sealed record class ToriiExplorerRwaParent
{
    private string rwa = string.Empty;
    private string quantity = string.Empty;

    [JsonPropertyName("rwa")]
    public string Rwa
    {
        get => rwa;
        init => rwa = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Rwa));
    }

    [JsonPropertyName("quantity")]
    public string Quantity
    {
        get => quantity;
        init => quantity = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Quantity));
    }
}

[JsonConverter(typeof(ToriiExplorerRwaJsonConverter))]
public sealed record class ToriiExplorerRwa
{
    private ToriiExplorerRwaParent[] parents = Array.Empty<ToriiExplorerRwaParent>();
    private string id = string.Empty;
    private string ownedBy = string.Empty;
    private string quantity = string.Empty;
    private string heldQuantity = string.Empty;
    private string primaryReference = string.Empty;
    private string? status;
    private JsonNode? metadata = new JsonObject();

    [JsonPropertyName("id")]
    public string Id
    {
        get => id;
        init => id = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Id));
    }

    [JsonPropertyName("owned_by")]
    public string OwnedBy
    {
        get => ownedBy;
        init => ownedBy = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(OwnedBy));
    }

    [JsonPropertyName("quantity")]
    public string Quantity
    {
        get => quantity;
        init => quantity = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, nameof(Quantity));
    }

    [JsonPropertyName("held_quantity")]
    public string HeldQuantity
    {
        get => heldQuantity;
        init => heldQuantity = ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(
            value,
            nameof(HeldQuantity));
    }

    [JsonPropertyName("primary_reference")]
    public string PrimaryReference
    {
        get => primaryReference;
        init => primaryReference = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(
            value,
            nameof(PrimaryReference));
    }

    [JsonPropertyName("status")]
    public string? Status
    {
        get => status;
        init => status = ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, nameof(Status));
    }

    [JsonPropertyName("is_frozen")]
    public bool IsFrozen { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata
    {
        get => ToriiJsonSnapshots.Copy(metadata);
        init => metadata = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("parents")]
    public IReadOnlyList<ToriiExplorerRwaParent> Parents
    {
        get => ToriiListSnapshots.CopyRequired(parents);
        init => parents = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerRwasPageJsonConverter))]
public sealed record class ToriiExplorerRwasPage
{
    private ToriiExplorerRwa[] items = Array.Empty<ToriiExplorerRwa>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerCursorMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerRwa> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerBlockJsonConverter))]
public sealed record class ToriiExplorerBlock
{
    private string hash = string.Empty;
    private string createdAt = string.Empty;
    private string? previousBlockHash;
    private string? transactionsHash;

    [JsonPropertyName("hash")]
    public string Hash
    {
        get => hash;
        init => hash = ToriiExplorerDirectMetadata.RequireExactSizedHex(value, nameof(Hash), 32);
    }

    [JsonPropertyName("height")]
    public ulong Height { get; init; }

    [JsonPropertyName("created_at")]
    public string CreatedAt
    {
        get => createdAt;
        init => createdAt = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(CreatedAt));
    }

    [JsonPropertyName("prev_block_hash")]
    public string? PreviousBlockHash
    {
        get => previousBlockHash;
        init => previousBlockHash = ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(PreviousBlockHash),
            32);
    }

    [JsonPropertyName("transactions_hash")]
    public string? TransactionsHash
    {
        get => transactionsHash;
        init => transactionsHash = ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(TransactionsHash),
            32);
    }

    [JsonPropertyName("transactions_rejected")]
    public uint TransactionsRejected { get; init; }

    [JsonPropertyName("transactions_total")]
    public uint TransactionsTotal { get; init; }
}

[JsonConverter(typeof(ToriiExplorerBlocksPageJsonConverter))]
public sealed record class ToriiExplorerBlocksPage
{
    private ToriiExplorerBlock[] items = Array.Empty<ToriiExplorerBlock>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerBlock> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerTransactionJsonConverter))]
public sealed record class ToriiExplorerTransaction
{
    private string authority = string.Empty;
    private string hash = string.Empty;
    private string createdAt = string.Empty;
    private string executable = string.Empty;
    private string status = string.Empty;

    [JsonPropertyName("authority")]
    public string Authority
    {
        get => authority;
        init => authority = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(Authority));
    }

    [JsonPropertyName("hash")]
    public string Hash
    {
        get => hash;
        init => hash = ToriiExplorerDirectMetadata.RequireExactSizedHex(value, nameof(Hash), 32);
    }

    [JsonPropertyName("block")]
    public ulong Block { get; init; }

    [JsonPropertyName("created_at")]
    public string CreatedAt
    {
        get => createdAt;
        init => createdAt = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(CreatedAt));
    }

    [JsonPropertyName("executable")]
    public string Executable
    {
        get => executable;
        init => executable = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Executable));
    }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Status));
    }
}

[JsonConverter(typeof(ToriiExplorerDurationJsonConverter))]
public sealed record class ToriiExplorerDuration
{
    [JsonPropertyName("ms")]
    public ulong Milliseconds { get; init; }
}

[JsonConverter(typeof(ToriiExplorerTransactionRejectionJsonConverter))]
public sealed record class ToriiExplorerTransactionRejection
{
    private string encoded = string.Empty;
    private JsonNode? json;
    private string message = string.Empty;

    [JsonPropertyName("encoded")]
    public string Encoded
    {
        get => encoded;
        init => encoded = ToriiExplorerDirectMetadata.RequireExactHex(value, nameof(Encoded));
    }

    [JsonPropertyName("json")]
    public JsonNode? Json
    {
        get => ToriiJsonSnapshots.Copy(json);
        init => json = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("message")]
    public string Message
    {
        get => message;
        init => message = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Message));
    }
}

[JsonConverter(typeof(ToriiExplorerTransactionDetailJsonConverter))]
public sealed record class ToriiExplorerTransactionDetail
{
    private string authority = string.Empty;
    private string hash = string.Empty;
    private string createdAt = string.Empty;
    private string executable = string.Empty;
    private string status = string.Empty;
    private JsonNode? metadata;
    private string signature = string.Empty;

    [JsonPropertyName("authority")]
    public string Authority
    {
        get => authority;
        init => authority = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(Authority));
    }

    [JsonPropertyName("hash")]
    public string Hash
    {
        get => hash;
        init => hash = ToriiExplorerDirectMetadata.RequireExactSizedHex(value, nameof(Hash), 32);
    }

    [JsonPropertyName("block")]
    public ulong Block { get; init; }

    [JsonPropertyName("created_at")]
    public string CreatedAt
    {
        get => createdAt;
        init => createdAt = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(CreatedAt));
    }

    [JsonPropertyName("executable")]
    public string Executable
    {
        get => executable;
        init => executable = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Executable));
    }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Status));
    }

    [JsonPropertyName("rejection_reason")]
    public ToriiExplorerTransactionRejection? RejectionReason { get; init; }

    [JsonPropertyName("metadata")]
    public JsonNode? Metadata
    {
        get => ToriiJsonSnapshots.Copy(metadata);
        init => metadata = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("nonce")]
    public ulong? Nonce { get; init; }

    [JsonPropertyName("signature")]
    public string Signature
    {
        get => signature;
        init => signature = ToriiExplorerDirectMetadata.RequireExactEvenLengthHex(value, nameof(Signature));
    }

    [JsonPropertyName("time_to_live")]
    public ToriiExplorerDuration? TimeToLive { get; init; }
}

[JsonConverter(typeof(ToriiExplorerTransactionsPageJsonConverter))]
public sealed record class ToriiExplorerTransactionsPage
{
    private ToriiExplorerTransaction[] items = Array.Empty<ToriiExplorerTransaction>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerTransaction> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerLatestTransactionsResponseJsonConverter))]
public sealed record class ToriiExplorerLatestTransactionsResponse
{
    private ToriiExplorerTransaction[] items = Array.Empty<ToriiExplorerTransaction>();
    private string sampledAt = string.Empty;

    [JsonPropertyName("sampled_at")]
    public string SampledAt
    {
        get => sampledAt;
        init => sampledAt = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(SampledAt));
    }

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerTransaction> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerInstructionJsonJsonConverter))]
public sealed record class ToriiExplorerInstructionJson
{
    private JsonNode? payload;
    private string kind = string.Empty;
    private string wireId = string.Empty;
    private string encoded = string.Empty;

    [JsonPropertyName("kind")]
    public string Kind
    {
        get => kind;
        init => kind = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Kind));
    }

    [JsonPropertyName("payload")]
    public JsonNode? Payload
    {
        get => ToriiJsonSnapshots.Copy(payload);
        init => payload = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("wire_id")]
    public string WireId
    {
        get => wireId;
        init => wireId = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(WireId));
    }

    [JsonPropertyName("encoded")]
    public string Encoded
    {
        get => encoded;
        init => encoded = ToriiExplorerDirectMetadata.RequireExactEvenLengthHex(value, nameof(Encoded));
    }
}

[JsonConverter(typeof(ToriiExplorerInstructionBoxJsonConverter))]
public sealed record class ToriiExplorerInstructionBox
{
    private string encoded = string.Empty;
    private ToriiExplorerInstructionJson? json;

    [JsonPropertyName("encoded")]
    public string Encoded
    {
        get => encoded;
        init => encoded = ToriiExplorerDirectMetadata.RequireExactHex(value, nameof(Encoded));
    }

    [JsonPropertyName("json")]
    public ToriiExplorerInstructionJson? Json
    {
        get => json;
        init => json = value ?? throw new ArgumentNullException(nameof(Json), "Instruction JSON must not be null.");
    }
}

[JsonConverter(typeof(ToriiExplorerInstructionJsonConverter))]
public sealed record class ToriiExplorerInstruction
{
    private string authority = string.Empty;
    private string createdAt = string.Empty;
    private string kind = string.Empty;
    private ToriiExplorerInstructionBox instructionBox = new()
    {
        Encoded = "00",
        Json = new ToriiExplorerInstructionJson
        {
            Kind = "Unknown",
            WireId = "unknown",
            Encoded = "00",
        },
    };
    private string transactionHash = string.Empty;
    private string transactionStatus = string.Empty;

    [JsonPropertyName("authority")]
    public string Authority
    {
        get => authority;
        init => authority = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(Authority));
    }

    [JsonPropertyName("created_at")]
    public string CreatedAt
    {
        get => createdAt;
        init => createdAt = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(CreatedAt));
    }

    [JsonPropertyName("kind")]
    public string Kind
    {
        get => kind;
        init => kind = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Kind));
    }

    [JsonPropertyName("box")]
    public ToriiExplorerInstructionBox InstructionBox
    {
        get => instructionBox;
        init => instructionBox = value ?? throw new ArgumentNullException(
            nameof(InstructionBox),
            "Instruction box must not be null.");
    }

    [JsonPropertyName("transaction_hash")]
    public string TransactionHash
    {
        get => transactionHash;
        init => transactionHash = ToriiExplorerDirectMetadata.RequireExactSizedHex(
            value,
            nameof(TransactionHash),
            32);
    }

    [JsonPropertyName("transaction_status")]
    public string TransactionStatus
    {
        get => transactionStatus;
        init => transactionStatus = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(
            value,
            nameof(TransactionStatus));
    }

    [JsonPropertyName("block")]
    public ulong Block { get; init; }

    [JsonPropertyName("index")]
    public uint Index { get; init; }
}

internal static class ToriiExplorerDirectMetadata
{
    internal const uint ExplorerCursorLimitMaximum = 100;
    internal const int ExplorerCursorMaximumLength = 1424;

    internal static ulong RequirePositive(ulong value, string paramName)
    {
        if (value == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value;
    }

    internal static int RequireNonNegative(int value, string paramName)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be non-negative.");
        }

        return value;
    }

    internal static int RequirePositive(int value, string paramName)
    {
        if (value <= 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value;
    }

    internal static uint RequireExplorerCursorLimit(uint value, string paramName)
    {
        if (value is 0 or > ExplorerCursorLimitMaximum)
        {
            throw new ArgumentOutOfRangeException(
                paramName,
                $"Value must be between 1 and {ExplorerCursorLimitMaximum}.");
        }

        return value;
    }

    internal static string? RequireOptionalCanonicalExplorerCursor(string? value, string paramName)
    {
        return value is null ? null : RequireCanonicalExplorerCursor(value, paramName);
    }

    internal static string RequireCanonicalExplorerCursor(string? value, string paramName)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Length > ExplorerCursorMaximumLength)
        {
            throw new ArgumentException(
                $"Value must be at most {ExplorerCursorMaximumLength} characters.",
                paramName);
        }

        if (!exact.All(character =>
                character is (>= 'A' and <= 'Z')
                    or (>= 'a' and <= 'z')
                    or (>= '0' and <= '9')
                    or '-'
                    or '_'))
        {
            throw new ArgumentException(
                "Value must use the canonical unpadded base64url alphabet.",
                paramName);
        }

        byte[] decoded;
        try
        {
            var paddingLength = (4 - exact.Length % 4) % 4;
            var base64 = exact.Replace('-', '+').Replace('_', '/') + new string('=', paddingLength);
            decoded = Convert.FromBase64String(base64);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException(
                "Value must be canonical unpadded base64url.",
                paramName,
                exception);
        }

        var canonical = Convert.ToBase64String(decoded)
            .TrimEnd('=')
            .Replace('+', '-')
            .Replace('/', '_');
        if (!string.Equals(canonical, exact, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Value must be canonical unpadded base64url.",
                paramName);
        }

        return exact;
    }

    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        try
        {
            _ = global::Hyperledger.Iroha.Address.AccountAddress.Parse(exact);
            return exact;
        }
        catch (global::Hyperledger.Iroha.AccountAddressException exception)
        {
            throw new ArgumentException("Value must be a canonical I105 account id.", paramName, exception);
        }
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value must be a non-empty string.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return value is null ? null : RequireExactNonEmptyText(value, paramName);
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        return exact;
    }

    internal static string? RequireOptionalExactTokenText(string? value, string paramName)
    {
        return value is null ? null : RequireExactTokenText(value, paramName);
    }

    internal static string RequireCanonicalQuantityText(string? value, string paramName)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        try
        {
            return Hyperledger.Iroha.Numeric.NumericV1.QuantityValue.ParseCanonical(exact).ToString();
        }
        catch (Hyperledger.Iroha.Numeric.NumericV1.NumericException exception)
        {
            throw new ArgumentException(
                "Value must be a canonical non-negative numeric Kotodama V1 Quantity string.",
                paramName,
                exception);
        }
    }

    internal static string? RequireOptionalCanonicalQuantityText(string? value, string paramName)
    {
        return value is null ? null : RequireCanonicalQuantityText(value, paramName);
    }

    internal static double RequireFiniteNonNegativeDouble(double value, string paramName)
    {
        if (!double.IsFinite(value) || value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be a finite non-negative number.");
        }

        return value;
    }

    internal static double RequireFiniteUnitIntervalDouble(double value, string paramName)
    {
        if (!double.IsFinite(value) || value < 0 || value > 1)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be a finite number from 0 to 1.");
        }

        return value;
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        return value is null ? null : RequireExactSizedHex(value, paramName, expectedBytes);
    }

    internal static string RequireExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (exact.Length != expectedBytes * 2 || !exact.All(IsLowercaseHexCharacter))
        {
            throw new ArgumentException(
                $"Value must be an exact lowercase {expectedBytes}-byte hex string.",
                paramName);
        }

        return exact;
    }

    internal static string RequireExactHex(string? value, string paramName)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        var body = exact.StartsWith("0x", StringComparison.Ordinal) ? exact[2..] : exact;
        if (body.Length == 0 || body.Length % 2 != 0 || !body.All(IsHexCharacter))
        {
            throw new ArgumentException("Value must be an exact hex string.", paramName);
        }

        return exact;
    }

    internal static string RequireExactEvenLengthHex(string? value, string paramName)
    {
        var exact = RequireExactNonEmptyText(value, paramName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (exact.Length % 2 != 0 || !exact.All(IsLowercaseHexCharacter))
        {
            throw new ArgumentException("Value must be an exact lowercase even-length hex string.", paramName);
        }

        return exact;
    }

    private static bool IsHexCharacter(char value)
    {
        return value is (>= '0' and <= '9') or (>= 'a' and <= 'f') or (>= 'A' and <= 'F');
    }

    private static bool IsLowercaseHexCharacter(char value)
    {
        return value is (>= '0' and <= '9') or (>= 'a' and <= 'f');
    }
}

[JsonConverter(typeof(ToriiExplorerInstructionsPageJsonConverter))]
public sealed record class ToriiExplorerInstructionsPage
{
    private ToriiExplorerInstruction[] items = Array.Empty<ToriiExplorerInstruction>();

    [JsonPropertyName("pagination")]
    public ToriiExplorerPaginationMeta Pagination { get; init; } = new();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerInstruction> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerLatestInstructionsResponseJsonConverter))]
public sealed record class ToriiExplorerLatestInstructionsResponse
{
    private ToriiExplorerInstruction[] items = Array.Empty<ToriiExplorerInstruction>();
    private string sampledAt = string.Empty;

    [JsonPropertyName("sampled_at")]
    public string SampledAt
    {
        get => sampledAt;
        init => sampledAt = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(SampledAt));
    }

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiExplorerInstruction> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiExplorerHealthSnapshotJsonConverter))]
public sealed record class ToriiExplorerHealthSnapshot
{
    private string? headCreatedAt;
    private string sampledAt = string.Empty;

    [JsonPropertyName("head_height")]
    public ulong HeadHeight { get; init; }

    [JsonPropertyName("head_created_at")]
    public string? HeadCreatedAt
    {
        get => headCreatedAt;
        init => headCreatedAt = ToriiExplorerDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(HeadCreatedAt));
    }

    [JsonPropertyName("sampled_at")]
    public string SampledAt
    {
        get => sampledAt;
        init => sampledAt = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(SampledAt));
    }
}

[JsonConverter(typeof(ToriiExplorerMetricsSnapshotJsonConverter))]
public sealed record class ToriiExplorerMetricsSnapshot
{
    private string? blockCreatedAt;

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
    public string? BlockCreatedAt
    {
        get => blockCreatedAt;
        init => blockCreatedAt = ToriiExplorerDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(BlockCreatedAt));
    }

    [JsonPropertyName("finalized_block")]
    public ulong FinalizedBlock { get; init; }

    [JsonPropertyName("avg_commit_time")]
    public ToriiExplorerDuration? AverageCommitTime { get; init; }

    [JsonPropertyName("avg_block_time")]
    public ToriiExplorerDuration? AverageBlockTime { get; init; }
}

[JsonConverter(typeof(ToriiTransactionsPageJsonConverter))]
public sealed record class ToriiTransactionsPage
{
    private ToriiTransactionSummary[] items = Array.Empty<ToriiTransactionSummary>();

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiTransactionSummary> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("total")]
    public long Total { get; init; }
}

[JsonConverter(typeof(ToriiSoraFsFileEntryJsonConverter))]
public sealed record class ToriiSoraFsFileEntry
{
    private string[] path = Array.Empty<string>();
    private long offset;
    private long size;
    private long firstChunk;
    private long chunkCount;

    [JsonPropertyName("path")]
    public IReadOnlyList<string> Path
    {
        get => ToriiListSnapshots.CopyRequired(path);
        init => path = ToriiSoraFsDirectMetadata.CopyRequiredPathComponents(value, nameof(Path));
    }

    [JsonPropertyName("offset")]
    public long Offset
    {
        get => offset;
        init => offset = ToriiSoraFsDirectMetadata.RequireNonNegative(value, nameof(Offset));
    }

    [JsonPropertyName("size")]
    public long Size
    {
        get => size;
        init => size = ToriiSoraFsDirectMetadata.RequireNonNegative(value, nameof(Size));
    }

    [JsonPropertyName("first_chunk")]
    public long FirstChunk
    {
        get => firstChunk;
        init => firstChunk = ToriiSoraFsDirectMetadata.RequireNonNegative(value, nameof(FirstChunk));
    }

    [JsonPropertyName("chunk_count")]
    public long ChunkCount
    {
        get => chunkCount;
        init => chunkCount = ToriiSoraFsDirectMetadata.RequireNonNegative(value, nameof(ChunkCount));
    }
}

[JsonConverter(typeof(ToriiSoraFsCidLookupResponseJsonConverter))]
public sealed record class ToriiSoraFsCidLookupResponse
{
    private string contentCid = string.Empty;
    private string manifestDigestHex = string.Empty;
    private string manifestIdHex = string.Empty;
    private string? indexDocument;
    private ToriiSoraFsFileEntry[] files = Array.Empty<ToriiSoraFsFileEntry>();

    [JsonPropertyName("content_cid")]
    public string ContentCid
    {
        get => contentCid;
        init => contentCid = ToriiSoraFsDirectMetadata.RequireContentCid(value, nameof(ContentCid));
    }

    [JsonPropertyName("manifest_digest_hex")]
    public string ManifestDigestHex
    {
        get => manifestDigestHex;
        init => manifestDigestHex = ToriiSoraFsDirectMetadata.RequireExactSizedHex(
            value,
            nameof(ManifestDigestHex),
            32);
    }

    [JsonPropertyName("manifest_id_hex")]
    public string ManifestIdHex
    {
        get => manifestIdHex;
        init => manifestIdHex = ToriiSoraFsDirectMetadata.RequireExactSizedHex(
            value,
            nameof(ManifestIdHex),
            32);
    }

    [JsonPropertyName("index_document")]
    public string? IndexDocument
    {
        get => indexDocument;
        init => indexDocument = ToriiSoraFsDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(IndexDocument));
    }

    [JsonPropertyName("files")]
    public IReadOnlyList<ToriiSoraFsFileEntry> Files
    {
        get => ToriiListSnapshots.CopyRequired(files);
        init => files = ToriiSoraFsDirectMetadata.CopyRequiredFileEntries(value, nameof(Files));
    }
}

[JsonConverter(typeof(ToriiSoraFsChunkerHandleJsonConverter))]
public sealed record class ToriiSoraFsChunkerHandle
{
    [JsonPropertyName("profile_id")]
    public uint? ProfileId { get; init; }

    [JsonPropertyName("namespace")]
    public string? Namespace { get; init; }

    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("semver")]
    public string? Semver { get; init; }

    [JsonPropertyName("multihash_code")]
    public uint? MultihashCode { get; init; }
}

[JsonConverter(typeof(ToriiSoraFsStorageClassJsonConverter))]
public sealed record class ToriiSoraFsStorageClass
{
    [JsonPropertyName("type")]
    public string? Type { get; init; }

    public static ToriiSoraFsStorageClass From(string type) => new() { Type = type };
}

[JsonConverter(typeof(ToriiSoraFsPinPolicyJsonConverter))]
public sealed record class ToriiSoraFsPinPolicy
{
    [JsonPropertyName("min_replicas")]
    public uint? MinReplicas { get; init; }

    [JsonPropertyName("storage_class")]
    public ToriiSoraFsStorageClass? StorageClass { get; init; }

    [JsonPropertyName("retention_epoch")]
    public ulong? RetentionEpoch { get; init; }
}

public sealed record class ToriiSoraFsPinRegisterResponse
{
    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("tx_hash_hex")]
    public string TxHashHex { get; init; } = string.Empty;

    [JsonPropertyName("manifest_digest_hex")]
    public string ManifestDigestHex { get; init; } = string.Empty;
}

internal static class ToriiSoraFsDirectMetadata
{
    internal static long RequireNonNegative(long value, string paramName)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, value, "Value must be non-negative.");
        }

        return value;
    }

    internal static string RequireContentCid(string? value, string paramName)
    {
        var text = RequireExactNonEmptyText(value, paramName);
        if (text.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (text[0] != 'b' || text.Length == 1)
        {
            throw new ArgumentException("Value must be lowercase multibase base32 CID text.", paramName);
        }

        for (var index = 1; index < text.Length; index++)
        {
            var character = text[index];
            if (character is not (>= 'a' and <= 'z') and not (>= '2' and <= '7'))
            {
                throw new ArgumentException("Value must be lowercase multibase base32 CID text.", paramName);
            }
        }

        return text;
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, paramName);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactNonEmptyText(value, paramName);
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static ulong RequireRequiredUInt64(ulong? value, string paramName)
    {
        return value ?? throw new ArgumentNullException(paramName, "Value must not be null.");
    }

    internal static string? RequireOptionalExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, paramName);
    }

    internal static string RequireExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, expectedBytes);
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string paramName, int expectedBytes)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(value, paramName, expectedBytes);
    }

    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static string? RequireOptionalCanonicalAccountId(string? value, string paramName)
    {
        return value is null ? null : ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static string[] CopyRequiredPathComponents(IReadOnlyList<string>? values, string paramName)
    {
        if (values is null)
        {
            throw new ArgumentNullException(paramName, "Value must not be null.");
        }
        if (values.Count == 0)
        {
            throw new ArgumentException("Value must not be empty.", paramName);
        }

        var copy = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            var value = values[index];
            if (value is null)
            {
                throw new ArgumentException("List elements must not be null.", $"{paramName}[{index}]");
            }

            copy[index] = RequirePathComponent(value, $"{paramName}[{index}]");
        }

        return copy;
    }

    internal static ToriiSoraFsFileEntry[] CopyRequiredFileEntries(
        IReadOnlyList<ToriiSoraFsFileEntry>? values,
        string paramName)
    {
        if (values is null)
        {
            return Array.Empty<ToriiSoraFsFileEntry>();
        }

        var copy = new ToriiSoraFsFileEntry[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            var value = values[index];
            if (value is null)
            {
                throw new ArgumentException("List elements must not be null.", $"{paramName}[{index}]");
            }

            copy[index] = RequireFileEntry(value, $"{paramName}[{index}]");
        }

        return copy;
    }


    private static string RequirePathComponent(string value, string paramName)
    {
        var text = RequireExactNonEmptyText(value, paramName);
        if (text == "." || text == ".." || text.Contains('/', StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be a relative path component.", paramName);
        }

        return text;
    }

    private static ToriiSoraFsFileEntry RequireFileEntry(ToriiSoraFsFileEntry value, string paramName)
    {
        var path = value.Path;
        if (path.Count == 0)
        {
            throw new ArgumentException("Value must not be empty.", $"{paramName}.{nameof(ToriiSoraFsFileEntry.Path)}");
        }

        for (var index = 0; index < path.Count; index++)
        {
            RequirePathComponent(path[index], $"{paramName}.{nameof(ToriiSoraFsFileEntry.Path)}[{index}]");
        }

        RequireNonNegative(value.Offset, $"{paramName}.{nameof(ToriiSoraFsFileEntry.Offset)}");
        RequireNonNegative(value.Size, $"{paramName}.{nameof(ToriiSoraFsFileEntry.Size)}");
        RequireNonNegative(value.FirstChunk, $"{paramName}.{nameof(ToriiSoraFsFileEntry.FirstChunk)}");
        RequireNonNegative(value.ChunkCount, $"{paramName}.{nameof(ToriiSoraFsFileEntry.ChunkCount)}");
        return value;
    }

    private static string RequireCanonicalBase64(string? value, string paramName)
    {
        var text = RequireExactTokenText(value, paramName);
        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(text);
        }
        catch (FormatException error)
        {
            throw new ArgumentException("Value must be base64 encoded.", paramName, error);
        }

        if (bytes.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty base64 payload.", paramName);
        }

        if (!string.Equals(Convert.ToBase64String(bytes), text, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be canonical base64 text.", paramName);
        }

        return text;
    }
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

[JsonConverter(typeof(ToriiUaidPortfolioTotalsJsonConverter))]
public sealed record class ToriiUaidPortfolioTotals
{
    private long accounts;
    private long positions;

    [JsonPropertyName("accounts")]
    public long Accounts
    {
        get => accounts;
        init => accounts = ToriiUaidDirectMetadata.RequireNonNegativeInt64(value, nameof(Accounts));
    }

    [JsonPropertyName("positions")]
    public long Positions
    {
        get => positions;
        init => positions = ToriiUaidDirectMetadata.RequireNonNegativeInt64(value, nameof(Positions));
    }
}

[JsonConverter(typeof(ToriiUaidPortfolioAssetJsonConverter))]
public sealed record class ToriiUaidPortfolioAsset
{
    private string assetId = string.Empty;
    private string assetDefinitionId = string.Empty;
    private string quantity = string.Empty;

    [JsonPropertyName("asset_id")]
    public string AssetId
    {
        get => assetId;
        init => assetId = ToriiUaidDirectMetadata.RequireExactNonEmptyText(value, nameof(AssetId));
    }

    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId
    {
        get => assetDefinitionId;
        init => assetDefinitionId = ToriiUaidDirectMetadata.RequireExactNonEmptyText(
            value,
            nameof(AssetDefinitionId));
    }

    [JsonPropertyName("quantity")]
    public string Quantity
    {
        get => quantity;
        init => quantity = ToriiUaidDirectMetadata.RequireCanonicalQuantityText(
            value,
            nameof(Quantity));
    }
}

[JsonConverter(typeof(ToriiUaidPortfolioAccountJsonConverter))]
public sealed record class ToriiUaidPortfolioAccount
{
    private string accountId = string.Empty;
    private string? label;
    private ToriiUaidPortfolioAsset[] assets = Array.Empty<ToriiUaidPortfolioAsset>();

    [JsonPropertyName("account_id")]
    public string AccountId
    {
        get => accountId;
        init => accountId = ToriiUaidDirectMetadata.RequireCanonicalAccountId(value, nameof(AccountId));
    }

    [JsonPropertyName("label")]
    public string? Label
    {
        get => label;
        init => label = ToriiUaidDirectMetadata.RequireOptionalExactNonEmptyText(value, nameof(Label));
    }

    [JsonPropertyName("assets")]
    public IReadOnlyList<ToriiUaidPortfolioAsset> Assets
    {
        get => ToriiListSnapshots.CopyRequired(assets);
        init => assets = ToriiUaidDirectMetadata.CopyRequiredPortfolioAssets(value, nameof(Assets));
    }
}

[JsonConverter(typeof(ToriiUaidPortfolioDataspaceJsonConverter))]
public sealed record class ToriiUaidPortfolioDataspace
{
    private long dataspaceId;
    private string? dataspaceAlias;
    private ToriiUaidPortfolioAccount[] accounts = Array.Empty<ToriiUaidPortfolioAccount>();

    [JsonPropertyName("dataspace_id")]
    public long DataspaceId
    {
        get => dataspaceId;
        init => dataspaceId = ToriiUaidDirectMetadata.RequireNonNegativeInt64(value, nameof(DataspaceId));
    }

    [JsonPropertyName("dataspace_alias")]
    public string? DataspaceAlias
    {
        get => dataspaceAlias;
        init => dataspaceAlias = ToriiUaidDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(DataspaceAlias));
    }

    [JsonPropertyName("accounts")]
    public IReadOnlyList<ToriiUaidPortfolioAccount> Accounts
    {
        get => ToriiListSnapshots.CopyRequired(accounts);
        init => accounts = ToriiUaidDirectMetadata.CopyRequiredPortfolioAccounts(value, nameof(Accounts));
    }
}

[JsonConverter(typeof(ToriiUaidPortfolioResponseJsonConverter))]
public sealed record class ToriiUaidPortfolioResponse
{
    private string uaid = string.Empty;
    private ToriiUaidPortfolioTotals totals = new();
    private ToriiUaidPortfolioDataspace[] dataspaces = Array.Empty<ToriiUaidPortfolioDataspace>();

    [JsonPropertyName("uaid")]
    public string Uaid
    {
        get => uaid;
        init => uaid = ToriiUaidDirectMetadata.RequireCanonicalUaidLiteral(value, nameof(Uaid));
    }

    [JsonPropertyName("totals")]
    public ToriiUaidPortfolioTotals Totals
    {
        get => totals;
        init => totals = ToriiUaidDirectMetadata.RequirePortfolioTotals(value, nameof(Totals));
    }

    [JsonPropertyName("dataspaces")]
    public IReadOnlyList<ToriiUaidPortfolioDataspace> Dataspaces
    {
        get => ToriiListSnapshots.CopyRequired(dataspaces);
        init => dataspaces = ToriiUaidDirectMetadata.CopyRequiredPortfolioDataspaces(value, nameof(Dataspaces));
    }
}

[JsonConverter(typeof(ToriiUaidBindingsDataspaceJsonConverter))]
public sealed record class ToriiUaidBindingsDataspace
{
    private long dataspaceId;
    private string? dataspaceAlias;
    private string[] accounts = Array.Empty<string>();

    [JsonPropertyName("dataspace_id")]
    public long DataspaceId
    {
        get => dataspaceId;
        init => dataspaceId = ToriiUaidDirectMetadata.RequireNonNegativeInt64(value, nameof(DataspaceId));
    }

    [JsonPropertyName("dataspace_alias")]
    public string? DataspaceAlias
    {
        get => dataspaceAlias;
        init => dataspaceAlias = ToriiUaidDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(DataspaceAlias));
    }

    [JsonPropertyName("accounts")]
    public IReadOnlyList<string> Accounts
    {
        get => ToriiListSnapshots.CopyRequired(accounts);
        init => accounts = ToriiUaidDirectMetadata.CopyRequiredCanonicalAccountIdList(value, nameof(Accounts));
    }
}

[JsonConverter(typeof(ToriiUaidBindingsResponseJsonConverter))]
public sealed record class ToriiUaidBindingsResponse
{
    private string uaid = string.Empty;
    private ToriiUaidBindingsDataspace[] dataspaces = Array.Empty<ToriiUaidBindingsDataspace>();

    [JsonPropertyName("uaid")]
    public string Uaid
    {
        get => uaid;
        init => uaid = ToriiUaidDirectMetadata.RequireCanonicalUaidLiteral(value, nameof(Uaid));
    }

    [JsonPropertyName("dataspaces")]
    public IReadOnlyList<ToriiUaidBindingsDataspace> Dataspaces
    {
        get => ToriiListSnapshots.CopyRequired(dataspaces);
        init => dataspaces = ToriiUaidDirectMetadata.CopyRequiredBindingsDataspaces(value, nameof(Dataspaces));
    }
}

[JsonConverter(typeof(ToriiUaidManifestRevocationJsonConverter))]
public sealed record class ToriiUaidManifestRevocation
{
    private long epoch;
    private string? reason;

    [JsonPropertyName("epoch")]
    public long Epoch
    {
        get => epoch;
        init => epoch = ToriiUaidDirectMetadata.RequireNonNegativeInt64(value, nameof(Epoch));
    }

    [JsonPropertyName("reason")]
    public string? Reason
    {
        get => reason;
        init => reason = ToriiUaidDirectMetadata.RequireOptionalExactNonEmptyText(value, nameof(Reason));
    }
}

[JsonConverter(typeof(ToriiUaidManifestLifecycleJsonConverter))]
public sealed record class ToriiUaidManifestLifecycle
{
    private long? activatedEpoch;
    private long? expiredEpoch;
    private ToriiUaidManifestRevocation? revocation;

    [JsonPropertyName("activated_epoch")]
    public long? ActivatedEpoch
    {
        get => activatedEpoch;
        init => activatedEpoch = ToriiUaidDirectMetadata.RequireOptionalNonNegativeInt64(
            value,
            nameof(ActivatedEpoch));
    }

    [JsonPropertyName("expired_epoch")]
    public long? ExpiredEpoch
    {
        get => expiredEpoch;
        init => expiredEpoch = ToriiUaidDirectMetadata.RequireOptionalNonNegativeInt64(value, nameof(ExpiredEpoch));
    }

    [JsonPropertyName("revocation")]
    public ToriiUaidManifestRevocation? Revocation
    {
        get => revocation;
        init => revocation = ToriiUaidDirectMetadata.RequireOptionalManifestRevocation(
            value,
            nameof(Revocation));
    }
}

[JsonConverter(typeof(ToriiUaidManifestRecordJsonConverter))]
public sealed record class ToriiUaidManifestRecord
{
    private long dataspaceId;
    private string? dataspaceAlias;
    private string manifestHash = string.Empty;
    private string status = string.Empty;
    private ToriiUaidManifestLifecycle lifecycle = new();
    private string[] accounts = Array.Empty<string>();
    private JsonNode? manifest;

    [JsonPropertyName("dataspace_id")]
    public long DataspaceId
    {
        get => dataspaceId;
        init => dataspaceId = ToriiUaidDirectMetadata.RequireNonNegativeInt64(value, nameof(DataspaceId));
    }

    [JsonPropertyName("dataspace_alias")]
    public string? DataspaceAlias
    {
        get => dataspaceAlias;
        init => dataspaceAlias = ToriiUaidDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(DataspaceAlias));
    }

    [JsonPropertyName("manifest_hash")]
    public string ManifestHash
    {
        get => manifestHash;
        init => manifestHash = ToriiUaidDirectMetadata.RequireExactSizedHex(value, nameof(ManifestHash));
    }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiUaidDirectMetadata.RequireExactNonEmptyText(value, nameof(Status));
    }

    [JsonPropertyName("lifecycle")]
    public ToriiUaidManifestLifecycle Lifecycle
    {
        get => lifecycle;
        init => lifecycle = ToriiUaidDirectMetadata.RequireManifestLifecycle(value, nameof(Lifecycle));
    }

    [JsonPropertyName("accounts")]
    public IReadOnlyList<string> Accounts
    {
        get => ToriiListSnapshots.CopyRequired(accounts);
        init => accounts = ToriiUaidDirectMetadata.CopyRequiredCanonicalAccountIdList(value, nameof(Accounts));
    }

    [JsonPropertyName("manifest")]
    public JsonNode? Manifest
    {
        get => ToriiJsonSnapshots.Copy(manifest);
        init => manifest = ToriiJsonSnapshots.Copy(value);
    }
}

[JsonConverter(typeof(ToriiUaidManifestsResponseJsonConverter))]
public sealed record class ToriiUaidManifestsResponse
{
    private string uaid = string.Empty;
    private long total;
    private ToriiUaidManifestRecord[] manifests = Array.Empty<ToriiUaidManifestRecord>();

    [JsonPropertyName("uaid")]
    public string Uaid
    {
        get => uaid;
        init => uaid = ToriiUaidDirectMetadata.RequireCanonicalUaidLiteral(value, nameof(Uaid));
    }

    [JsonPropertyName("total")]
    public long Total
    {
        get => total;
        init => total = ToriiUaidDirectMetadata.RequireNonNegativeInt64(value, nameof(Total));
    }

    [JsonPropertyName("manifests")]
    public IReadOnlyList<ToriiUaidManifestRecord> Manifests
    {
        get => ToriiListSnapshots.CopyRequired(manifests);
        init => manifests = ToriiUaidDirectMetadata.CopyRequiredManifestRecords(value, nameof(Manifests));
    }
}

internal static class ToriiUaidDirectMetadata
{
    internal static long RequireNonNegativeInt64(long value, string paramName)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, value, "Value must be non-negative.");
        }

        return value;
    }

    internal static long? RequireOptionalNonNegativeInt64(long? value, string paramName)
    {
        return value is long integer ? RequireNonNegativeInt64(integer, paramName) : null;
    }

    internal static string RequireCanonicalUaidLiteral(string? value, string paramName)
    {
        var text = RequireExactNonEmptyText(value, paramName);
        if (!text.StartsWith("uaid:", StringComparison.Ordinal)
            || text.Length != 69
            || !IsLowercaseHex(text.AsSpan(5))
            || (HexNibble(text[^1]) & 1) == 0)
        {
            throw new ArgumentException(
                "Value must be a canonical `uaid:<64 lowercase hex chars>` literal.",
                paramName);
        }

        return text;
    }

    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, paramName);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactNonEmptyText(value, paramName);
    }

    internal static string RequireCanonicalQuantityText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalQuantityText(value, paramName);
    }

    internal static string RequireExactSizedHex(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }

    internal static ToriiUaidPortfolioTotals RequirePortfolioTotals(
        ToriiUaidPortfolioTotals? value,
        string paramName)
    {
        if (value is null)
        {
            throw new ArgumentNullException(paramName, "Value must not be null.");
        }

        RequireNonNegativeInt64(value.Accounts, $"{paramName}.{nameof(ToriiUaidPortfolioTotals.Accounts)}");
        RequireNonNegativeInt64(value.Positions, $"{paramName}.{nameof(ToriiUaidPortfolioTotals.Positions)}");
        return value;
    }

    internal static ToriiUaidManifestLifecycle RequireManifestLifecycle(
        ToriiUaidManifestLifecycle? value,
        string paramName)
    {
        if (value is null)
        {
            throw new ArgumentNullException(paramName, "Value must not be null.");
        }

        RequireOptionalNonNegativeInt64(
            value.ActivatedEpoch,
            $"{paramName}.{nameof(ToriiUaidManifestLifecycle.ActivatedEpoch)}");
        RequireOptionalNonNegativeInt64(
            value.ExpiredEpoch,
            $"{paramName}.{nameof(ToriiUaidManifestLifecycle.ExpiredEpoch)}");
        RequireOptionalManifestRevocation(
            value.Revocation,
            $"{paramName}.{nameof(ToriiUaidManifestLifecycle.Revocation)}");
        return value;
    }

    internal static ToriiUaidManifestRevocation? RequireOptionalManifestRevocation(
        ToriiUaidManifestRevocation? value,
        string paramName)
    {
        if (value is null)
        {
            return null;
        }

        RequireNonNegativeInt64(value.Epoch, $"{paramName}.{nameof(ToriiUaidManifestRevocation.Epoch)}");
        RequireOptionalExactNonEmptyText(value.Reason, $"{paramName}.{nameof(ToriiUaidManifestRevocation.Reason)}");
        return value;
    }

    internal static ToriiUaidPortfolioAsset[] CopyRequiredPortfolioAssets(
        IReadOnlyList<ToriiUaidPortfolioAsset>? values,
        string paramName)
    {
        return CopyRequiredObjects(values, paramName, RequirePortfolioAsset);
    }

    internal static ToriiUaidPortfolioAccount[] CopyRequiredPortfolioAccounts(
        IReadOnlyList<ToriiUaidPortfolioAccount>? values,
        string paramName)
    {
        return CopyRequiredObjects(values, paramName, RequirePortfolioAccount);
    }

    internal static ToriiUaidPortfolioDataspace[] CopyRequiredPortfolioDataspaces(
        IReadOnlyList<ToriiUaidPortfolioDataspace>? values,
        string paramName)
    {
        return CopyRequiredObjects(values, paramName, RequirePortfolioDataspace);
    }

    internal static ToriiUaidBindingsDataspace[] CopyRequiredBindingsDataspaces(
        IReadOnlyList<ToriiUaidBindingsDataspace>? values,
        string paramName)
    {
        return CopyRequiredObjects(values, paramName, RequireBindingsDataspace);
    }

    internal static ToriiUaidManifestRecord[] CopyRequiredManifestRecords(
        IReadOnlyList<ToriiUaidManifestRecord>? values,
        string paramName)
    {
        return CopyRequiredObjects(values, paramName, RequireManifestRecord);
    }

    internal static string[] CopyRequiredCanonicalAccountIdList(IReadOnlyList<string>? values, string paramName)
    {
        if (values is null)
        {
            throw new ArgumentNullException(paramName, "Value must not be null.");
        }

        var copy = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            var value = values[index];
            if (value is null)
            {
                throw new ArgumentException("List elements must not be null.", $"{paramName}[{index}]");
            }

            copy[index] = RequireCanonicalAccountId(value, $"{paramName}[{index}]");
        }

        return copy;
    }

    private static T[] CopyRequiredObjects<T>(
        IReadOnlyList<T>? values,
        string paramName,
        Func<T, string, T> validate)
        where T : class
    {
        if (values is null)
        {
            throw new ArgumentNullException(paramName, "Value must not be null.");
        }

        var copy = new T[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            var value = values[index];
            if (value is null)
            {
                throw new ArgumentException("List elements must not be null.", $"{paramName}[{index}]");
            }

            copy[index] = validate(value, $"{paramName}[{index}]");
        }

        return copy;
    }

    private static ToriiUaidPortfolioAsset RequirePortfolioAsset(
        ToriiUaidPortfolioAsset value,
        string paramName)
    {
        RequireExactNonEmptyText(value.AssetId, $"{paramName}.{nameof(ToriiUaidPortfolioAsset.AssetId)}");
        RequireExactNonEmptyText(
            value.AssetDefinitionId,
            $"{paramName}.{nameof(ToriiUaidPortfolioAsset.AssetDefinitionId)}");
        RequireCanonicalQuantityText(
            value.Quantity,
            $"{paramName}.{nameof(ToriiUaidPortfolioAsset.Quantity)}");
        return value;
    }

    private static ToriiUaidPortfolioAccount RequirePortfolioAccount(
        ToriiUaidPortfolioAccount value,
        string paramName)
    {
        RequireCanonicalAccountId(value.AccountId, $"{paramName}.{nameof(ToriiUaidPortfolioAccount.AccountId)}");
        RequireOptionalExactNonEmptyText(value.Label, $"{paramName}.{nameof(ToriiUaidPortfolioAccount.Label)}");
        CopyRequiredPortfolioAssets(value.Assets, $"{paramName}.{nameof(ToriiUaidPortfolioAccount.Assets)}");
        return value;
    }

    private static ToriiUaidPortfolioDataspace RequirePortfolioDataspace(
        ToriiUaidPortfolioDataspace value,
        string paramName)
    {
        RequireNonNegativeInt64(value.DataspaceId, $"{paramName}.{nameof(ToriiUaidPortfolioDataspace.DataspaceId)}");
        RequireOptionalExactNonEmptyText(
            value.DataspaceAlias,
            $"{paramName}.{nameof(ToriiUaidPortfolioDataspace.DataspaceAlias)}");
        CopyRequiredPortfolioAccounts(value.Accounts, $"{paramName}.{nameof(ToriiUaidPortfolioDataspace.Accounts)}");
        return value;
    }

    private static ToriiUaidBindingsDataspace RequireBindingsDataspace(
        ToriiUaidBindingsDataspace value,
        string paramName)
    {
        RequireNonNegativeInt64(value.DataspaceId, $"{paramName}.{nameof(ToriiUaidBindingsDataspace.DataspaceId)}");
        RequireOptionalExactNonEmptyText(
            value.DataspaceAlias,
            $"{paramName}.{nameof(ToriiUaidBindingsDataspace.DataspaceAlias)}");
        CopyRequiredCanonicalAccountIdList(value.Accounts, $"{paramName}.{nameof(ToriiUaidBindingsDataspace.Accounts)}");
        return value;
    }

    private static ToriiUaidManifestRecord RequireManifestRecord(
        ToriiUaidManifestRecord value,
        string paramName)
    {
        RequireNonNegativeInt64(value.DataspaceId, $"{paramName}.{nameof(ToriiUaidManifestRecord.DataspaceId)}");
        RequireOptionalExactNonEmptyText(
            value.DataspaceAlias,
            $"{paramName}.{nameof(ToriiUaidManifestRecord.DataspaceAlias)}");
        RequireExactSizedHex(value.ManifestHash, $"{paramName}.{nameof(ToriiUaidManifestRecord.ManifestHash)}");
        RequireExactNonEmptyText(value.Status, $"{paramName}.{nameof(ToriiUaidManifestRecord.Status)}");
        RequireManifestLifecycle(value.Lifecycle, $"{paramName}.{nameof(ToriiUaidManifestRecord.Lifecycle)}");
        CopyRequiredCanonicalAccountIdList(value.Accounts, $"{paramName}.{nameof(ToriiUaidManifestRecord.Accounts)}");
        return value;
    }

    private static bool IsLowercaseHex(ReadOnlySpan<char> value)
    {
        foreach (var character in value)
        {
            if (character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f'))
            {
                return false;
            }
        }

        return true;
    }

    private static int HexNibble(char character)
    {
        if (character >= '0' && character <= '9')
        {
            return character - '0';
        }

        if (character >= 'a' && character <= 'f')
        {
            return character - 'a' + 10;
        }

        return 0;
    }
}

[JsonConverter(typeof(ToriiExplorerAccountQrSnapshotJsonConverter))]
public sealed record class ToriiExplorerAccountQrSnapshot
{
    private string canonicalId = string.Empty;
    private string literal = string.Empty;
    private int networkPrefix;
    private string errorCorrection = string.Empty;
    private int modules;
    private int qrVersion;
    private string svg = string.Empty;

    [JsonPropertyName("canonical_id")]
    public string CanonicalId
    {
        get => canonicalId;
        init => canonicalId = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, nameof(CanonicalId));
    }

    [JsonPropertyName("literal")]
    public string Literal
    {
        get => literal;
        init => literal = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(Literal));
    }

    [JsonPropertyName("network_prefix")]
    public int NetworkPrefix
    {
        get => networkPrefix;
        init => networkPrefix = ToriiExplorerDirectMetadata.RequireNonNegative(value, nameof(NetworkPrefix));
    }

    [JsonPropertyName("error_correction")]
    public string ErrorCorrection
    {
        get => errorCorrection;
        init => errorCorrection = ToriiExplorerDirectMetadata.RequireExactTokenText(value, nameof(ErrorCorrection));
    }

    [JsonPropertyName("modules")]
    public int Modules
    {
        get => modules;
        init => modules = ToriiExplorerDirectMetadata.RequirePositive(value, nameof(Modules));
    }

    [JsonPropertyName("qr_version")]
    public int QrVersion
    {
        get => qrVersion;
        init => qrVersion = ToriiExplorerDirectMetadata.RequirePositive(value, nameof(QrVersion));
    }

    [JsonPropertyName("svg")]
    public string Svg
    {
        get => svg;
        init => svg = ToriiExplorerDirectMetadata.RequireExactNonEmptyText(value, nameof(Svg));
    }
}

[JsonConverter(typeof(ToriiIdentifierPolicySummaryJsonConverter))]
public sealed record class ToriiIdentifierPolicySummary
{
    private JsonNode? inputEncryptionPublicParametersDecoded;
    private JsonNode? ramFheProfile;

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
    public JsonNode? InputEncryptionPublicParametersDecoded
    {
        get => ToriiJsonSnapshots.Copy(inputEncryptionPublicParametersDecoded);
        init => inputEncryptionPublicParametersDecoded = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("ram_fhe_profile")]
    public JsonNode? RamFheProfile
    {
        get => ToriiJsonSnapshots.Copy(ramFheProfile);
        init => ramFheProfile = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("note")]
    public string? Note { get; init; }
}

[JsonConverter(typeof(ToriiIdentifierPoliciesResponseJsonConverter))]
public sealed record class ToriiIdentifierPoliciesResponse
{
    private ToriiIdentifierPolicySummary[] items = Array.Empty<ToriiIdentifierPolicySummary>();

    [JsonPropertyName("total")]
    public long Total { get; init; }

    [JsonPropertyName("items")]
    public IReadOnlyList<ToriiIdentifierPolicySummary> Items
    {
        get => ToriiListSnapshots.CopyRequired(items);
        init => items = ToriiListSnapshots.CopyRequired(value);
    }
}

[JsonConverter(typeof(ToriiIdentifierResolveResponseJsonConverter))]
public sealed record class ToriiIdentifierResolveResponse
{
    private JsonNode? signaturePayload;

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
    public JsonNode? SignaturePayload
    {
        get => ToriiJsonSnapshots.Copy(signaturePayload);
        init => signaturePayload = ToriiJsonSnapshots.Copy(value);
    }
}

[JsonConverter(typeof(ToriiContractAliasBindingJsonConverter))]
public sealed record class ToriiContractAliasBinding
{
    private string alias = string.Empty;
    private string status = string.Empty;
    private long? leaseExpiryMilliseconds;
    private long? graceUntilMilliseconds;
    private long boundAtMilliseconds;

    [JsonPropertyName("alias")]
    public string Alias
    {
        get => alias;
        init => alias = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Alias));
    }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Status));
    }

    [JsonPropertyName("lease_expiry_ms")]
    public long? LeaseExpiryMilliseconds
    {
        get => leaseExpiryMilliseconds;
        init => leaseExpiryMilliseconds = ToriiAliasDirectMetadata.RequireOptionalPositive(
            value,
            nameof(LeaseExpiryMilliseconds));
    }

    [JsonPropertyName("grace_until_ms")]
    public long? GraceUntilMilliseconds
    {
        get => graceUntilMilliseconds;
        init => graceUntilMilliseconds = ToriiAliasDirectMetadata.RequireOptionalPositive(
            value,
            nameof(GraceUntilMilliseconds));
    }

    [JsonPropertyName("bound_at_ms")]
    public long BoundAtMilliseconds
    {
        get => boundAtMilliseconds;
        init => boundAtMilliseconds = ToriiAliasDirectMetadata.RequirePositive(value, nameof(BoundAtMilliseconds));
    }
}

public sealed record class ToriiContractAliasResolutionRequest
{
    [JsonPropertyName("contract_alias")]
    public string ContractAlias { get; init; } = string.Empty;
}

[JsonConverter(typeof(ToriiContractAliasResolutionJsonConverter))]
public sealed record class ToriiContractAliasResolution
{
    private string contractAlias = string.Empty;
    private string contractAddress = string.Empty;
    private string dataspace = string.Empty;
    private string? source;

    [JsonPropertyName("contract_alias")]
    public string ContractAlias
    {
        get => contractAlias;
        init => contractAlias = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(ContractAlias));
    }

    [JsonPropertyName("contract_address")]
    public string ContractAddress
    {
        get => contractAddress;
        init => contractAddress = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(ContractAddress));
    }

    [JsonPropertyName("dataspace")]
    public string Dataspace
    {
        get => dataspace;
        init => dataspace = ToriiAliasDirectMetadata.RequireExactTokenText(value, nameof(Dataspace));
    }

    [JsonPropertyName("contract_alias_binding")]
    public ToriiContractAliasBinding? ContractAliasBinding { get; init; }

    [JsonPropertyName("source")]
    public string? Source
    {
        get => source;
        init => source = ToriiAliasDirectMetadata.RequireOptionalExactTokenText(value, nameof(Source));
    }
}

[JsonConverter(typeof(ToriiContractCodeRecordJsonConverter))]
public sealed record class ToriiContractCodeRecord
{
    private ToriiContractManifest manifest = new();
    private string? codeHash;
    private string? abiHash;

    [JsonPropertyName("manifest")]
    public ToriiContractManifest Manifest
    {
        get => manifest;
        init => manifest = ToriiContractMetadataDirectMetadata.RequireObject(value, nameof(Manifest));
    }

    [JsonPropertyName("code_hash")]
    public string? CodeHash
    {
        get => codeHash;
        init => codeHash = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(value, nameof(CodeHash));
    }

    [JsonPropertyName("abi_hash")]
    public string? AbiHash
    {
        get => abiHash;
        init => abiHash = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(value, nameof(AbiHash));
    }
}

internal static class ToriiContractMetadataDirectMetadata
{
    internal static T RequireObject<T>(T? value, string paramName)
        where T : class
    {
        return value ?? throw new ArgumentNullException(paramName, "Value must not be null.");
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string? RequireOptionalExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, paramName);
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        return ToriiContractCallDirectMetadata.RequireExactNonEmptyText(value, paramName);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return ToriiContractCallDirectMetadata.RequireOptionalExactNonEmptyText(value, paramName);
    }

    internal static string RequireExactSizedHex(string? value, string paramName)
    {
        if (value is not null && value.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty 32-byte hex string.", paramName);
        }

        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(value, paramName, 32);
    }

    internal static string[] CopyRequiredExactTokenTextList(
        IReadOnlyList<string>? values,
        string paramName)
    {
        return CopyRequiredTextList(values, paramName, RequireExactTokenText);
    }

    internal static string[] CopyRequiredExactNonEmptyTextList(
        IReadOnlyList<string>? values,
        string paramName)
    {
        return CopyRequiredTextList(values, paramName, RequireExactNonEmptyText);
    }

    internal static string RequireRenderedSourceText(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value must be non-empty rendered source text.", paramName);
        }

        if (value.IndexOf('\0') >= 0)
        {
            throw new ArgumentException("Value must not contain NUL characters.", paramName);
        }

        return value;
    }

    private static string[] CopyRequiredTextList(
        IReadOnlyList<string>? values,
        string paramName,
        Func<string?, string, string> validate)
    {
        if (values is null)
        {
            return Array.Empty<string>();
        }

        var copy = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            copy[index] = validate(values[index], $"{paramName}[{index}]");
        }

        return copy;
    }
}


[JsonConverter(typeof(ToriiContractCodeBytesResponseJsonConverter))]
public sealed record class ToriiContractCodeBytesResponse
{
    private string codeBase64 = string.Empty;

    [JsonPropertyName("code_b64")]
    public string CodeBase64
    {
        get => codeBase64;
        init
        {
            codeBase64 = value ?? throw new ArgumentNullException(nameof(CodeBase64));
            _ = DecodeExactBase64(codeBase64);
        }
    }

    public byte[] DecodeBytes()
    {
        return DecodeExactBase64(CodeBase64);
    }

    private static byte[] DecodeExactBase64(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new FormatException("code_b64 must be a non-empty base64 payload.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new FormatException("code_b64 must not contain surrounding whitespace.");
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new FormatException("code_b64 must not contain whitespace.");
        }

        if (value.Any(char.IsControl))
        {
            throw new FormatException("code_b64 must not contain control characters.");
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new FormatException("code_b64 must be an exact base64 payload.", error);
        }

        if (bytes.Length == 0)
        {
            throw new FormatException("code_b64 must be a non-empty base64 payload.");
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new FormatException("code_b64 must be canonical base64 text.");
        }

        return bytes;
    }
}

[JsonConverter(typeof(ToriiContractInstanceJsonConverter))]
public sealed record class ToriiContractInstance
{
    private string contractId = string.Empty;
    private string codeHashHex = string.Empty;

    [JsonPropertyName("contract_id")]
    public string ContractId
    {
        get => contractId;
        init => contractId = ToriiContractInstancesDirectMetadata.RequireExactTokenText(value, nameof(ContractId));
    }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex
    {
        get => codeHashHex;
        init => codeHashHex = ToriiContractInstancesDirectMetadata.RequireExactSizedHex(value, nameof(CodeHashHex));
    }
}

[JsonConverter(typeof(ToriiContractInstancesResponseJsonConverter))]
public sealed record class ToriiContractInstancesResponse
{
    private string @namespace = string.Empty;
    private ToriiContractInstance[] instances = Array.Empty<ToriiContractInstance>();

    [JsonPropertyName("namespace")]
    public string Namespace
    {
        get => @namespace;
        init => @namespace = ToriiContractInstancesDirectMetadata.RequireExactTokenText(value, nameof(Namespace));
    }

    [JsonPropertyName("instances")]
    public IReadOnlyList<ToriiContractInstance> Instances
    {
        get => ToriiListSnapshots.CopyRequired(instances);
        init => instances = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("total")]
    public ulong Total { get; init; }

    [JsonPropertyName("offset")]
    public ulong Offset { get; init; }

    [JsonPropertyName("limit")]
    public ulong Limit { get; init; }
}

internal static class ToriiContractInstancesDirectMetadata
{
    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string RequireExactSizedHex(string? value, string paramName)
    {
        if (value is not null && value.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty 32-byte hex string.", paramName);
        }

        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }
}


[JsonConverter(typeof(ToriiContractStateEntryJsonConverter))]
public sealed record class ToriiContractStateEntry
{
    private string path = string.Empty;
    private string? valueBase64;
    private JsonNode? valueJson;
    private string? decodeError;

    [JsonPropertyName("path")]
    public string Path
    {
        get => path;
        init => path = ToriiContractStateDirectMetadata.RequireExactTokenText(value, nameof(Path));
    }

    [JsonPropertyName("found")]
    public bool Found { get; init; }

    [JsonPropertyName("value_b64")]
    public string? ValueBase64
    {
        get => valueBase64;
        init => valueBase64 = ToriiContractStateDirectMetadata.RequireOptionalExactBase64AllowEmpty(
            value,
            nameof(ValueBase64));
    }

    [JsonPropertyName("value_len")]
    public ulong? ValueLength { get; init; }

    [JsonPropertyName("value_json")]
    public JsonNode? ValueJson
    {
        get => ToriiJsonSnapshots.Copy(valueJson);
        init => valueJson = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("decode_error")]
    public string? DecodeError
    {
        get => decodeError;
        init => decodeError = ToriiContractStateDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(DecodeError));
    }
}

[JsonConverter(typeof(ToriiContractStateResponseJsonConverter))]
public sealed record class ToriiContractStateResponse
{
    private string? path;
    private string[]? paths;
    private string? prefix;
    private ToriiContractStateEntry[] entries = Array.Empty<ToriiContractStateEntry>();

    [JsonPropertyName("path")]
    public string? Path
    {
        get => path;
        init => path = ToriiContractStateDirectMetadata.RequireOptionalExactTokenText(value, nameof(Path));
    }

    [JsonPropertyName("paths")]
    public IReadOnlyList<string>? Paths
    {
        get => ToriiListSnapshots.Copy(paths);
        init => paths = ToriiContractStateDirectMetadata.CopyOptionalExactTokenTextList(value, nameof(Paths));
    }

    [JsonPropertyName("prefix")]
    public string? Prefix
    {
        get => prefix;
        init => prefix = ToriiContractStateDirectMetadata.RequireOptionalExactTokenText(value, nameof(Prefix));
    }

    [JsonPropertyName("entries")]
    public IReadOnlyList<ToriiContractStateEntry> Entries
    {
        get => ToriiListSnapshots.CopyRequired(entries);
        init => entries = ToriiListSnapshots.CopyRequired(value);
    }

    [JsonPropertyName("offset")]
    public ulong Offset { get; init; }

    [JsonPropertyName("limit")]
    public ulong Limit { get; init; }

    [JsonPropertyName("next_offset")]
    public ulong? NextOffset { get; init; }
}

internal static class ToriiContractStateDirectMetadata
{
    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string? RequireOptionalExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, paramName);
    }

    internal static string[]? CopyOptionalExactTokenTextList(
        IReadOnlyList<string>? values,
        string paramName)
    {
        if (values is null)
        {
            return null;
        }

        var copy = new string[values.Count];
        for (var index = 0; index < values.Count; index++)
        {
            copy[index] = RequireExactTokenText(values[index], $"{paramName}[{index}]");
        }

        return copy;
    }

    internal static string? RequireOptionalExactBase64AllowEmpty(string? value, string paramName)
    {
        if (value is null)
        {
            return null;
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new ArgumentException("Value must not contain whitespace.", paramName);
            }

            if (char.IsControl(character))
            {
                throw new ArgumentException("Value must not contain control characters.", paramName);
            }
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException("Value must be valid base64.", paramName, exception);
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be canonical base64 text.", paramName);
        }

        return value;
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return ToriiContractCallDirectMetadata.RequireOptionalExactNonEmptyText(value, paramName);
    }
}

public sealed record class ToriiContractViewRequest
{
    private JsonNode? payload;

    [JsonPropertyName("authority")]
    public string Authority { get; init; } = string.Empty;

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("contract_alias")]
    public string? ContractAlias { get; init; }

    [JsonPropertyName("entrypoint")]
    public string? Entrypoint { get; init; }

    [JsonPropertyName("payload")]
    public JsonNode? Payload
    {
        get => ToriiJsonSnapshots.Copy(payload);
        init => payload = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("gas_limit")]
    public ulong GasLimit { get; init; }
}

public sealed record class ToriiContractCallRequest
{
    private JsonNode? payload;

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
    public JsonNode? Payload
    {
        get => ToriiJsonSnapshots.Copy(payload);
        init => payload = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds { get; init; }

    [JsonPropertyName("transaction_ttl_ms")]
    public ulong? TransactionTimeToLiveMilliseconds { get; init; }

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; init; } = null!;
}

public sealed record class ToriiFeeQuoteRequest
{
    [JsonPropertyName("payload")]
    public UnsignedTransactionPayload Payload { get; init; } = null!;
}

public sealed record class ToriiFeeQuoteObservation
{
    [JsonPropertyName("ledger_time_ms")]
    public ulong LedgerTimeMilliseconds { get; init; }

    [JsonPropertyName("next_block_height")]
    public ulong NextBlockHeight { get; init; }

    [JsonPropertyName("route_dataspace_id")]
    public ulong RouteDataspaceId { get; init; }
}

public sealed record class ToriiFeeQuoteCapacity
{
    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("vault_balance")]
    public string VaultBalance { get; init; } = string.Empty;

    [JsonPropertyName("reserve_floor")]
    public string ReserveFloor { get; init; } = string.Empty;

    [JsonPropertyName("block_remaining")]
    public string BlockRemaining { get; init; } = string.Empty;

    [JsonPropertyName("program_epoch_remaining")]
    public string ProgramEpochRemaining { get; init; } = string.Empty;

    [JsonPropertyName("beneficiary_epoch_remaining")]
    public string BeneficiaryEpochRemaining { get; init; } = string.Empty;
}

public sealed record class ToriiFeeDebitSource
{
    [JsonPropertyName("kind")]
    public string Kind { get; init; } = string.Empty;

    [JsonPropertyName("value")]
    public JsonElement Value { get; init; }
}

public sealed record class ToriiFeeQuoteDecisionValue
{
    [JsonPropertyName("debit_source")]
    public ToriiFeeDebitSource DebitSource { get; init; } = null!;

    [JsonPropertyName("program_revision")]
    public ulong? ProgramRevision { get; init; }
}

public sealed record class ToriiFeeQuoteDecision
{
    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("value")]
    public ToriiFeeQuoteDecisionValue Value { get; init; } = null!;
}

public sealed record class ToriiFeeQuoteResponse
{
    [JsonPropertyName("intent")]
    public FeePaymentIntent Intent { get; init; } = null!;

    [JsonPropertyName("observation")]
    public ToriiFeeQuoteObservation Observation { get; init; } = null!;

    [JsonPropertyName("components")]
    public IReadOnlyList<FeeChargeLimit> Components { get; init; } = Array.Empty<FeeChargeLimit>();

    [JsonPropertyName("capacities")]
    public IReadOnlyList<ToriiFeeQuoteCapacity> Capacities { get; init; } = Array.Empty<ToriiFeeQuoteCapacity>();

    [JsonPropertyName("decision")]
    public ToriiFeeQuoteDecision Decision { get; init; } = null!;
}

public sealed record class ToriiFeeSponsorProgramLookupRequest
{
    [JsonPropertyName("program_id")]
    public string ProgramId { get; init; } = string.Empty;
}

public sealed record class ToriiFeeSponsorProgramLifecycle
{
    [JsonPropertyName("state")]
    public string State { get; init; } = string.Empty;

    [JsonPropertyName("value")]
    public JsonNode? Value { get; init; }
}

public sealed record class ToriiFeeSponsorProgramActivation
{
    [JsonPropertyName("revision")]
    public ulong Revision { get; init; }

    [JsonPropertyName("activate_at_height")]
    public ulong ActivateAtHeight { get; init; }
}

public sealed record class ToriiFeeSponsorProgram
{
    [JsonPropertyName("id")]
    public FeeSponsorProgramId Id { get; init; } = null!;

    [JsonPropertyName("payout_account")]
    public string PayoutAccount { get; init; } = string.Empty;

    [JsonPropertyName("lifecycle")]
    public ToriiFeeSponsorProgramLifecycle Lifecycle { get; init; } = null!;

    [JsonPropertyName("active_revision")]
    public ulong? ActiveRevision { get; init; }

    [JsonPropertyName("staged_revision")]
    public ulong? StagedRevision { get; init; }

    [JsonPropertyName("scheduled_activation")]
    public ToriiFeeSponsorProgramActivation? ScheduledActivation { get; init; }
}

public sealed record class ToriiOperationReceipt
{
    [JsonPropertyName("operation_kind")]
    public string OperationKind { get; init; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; init; } = string.Empty;

    [JsonPropertyName("transport")]
    public string Transport { get; init; } = string.Empty;

    [JsonPropertyName("dataspace")]
    public string Dataspace { get; init; } = string.Empty;

    [JsonPropertyName("contract_alias")]
    public string? ContractAlias { get; init; }

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("code_hash_hex")]
    public string? CodeHashHex { get; init; }

    [JsonPropertyName("abi_hash_hex")]
    public string? AbiHashHex { get; init; }

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex { get; init; }

    [JsonPropertyName("entrypoint")]
    public string? Entrypoint { get; init; }

    [JsonPropertyName("entrypoint_hash_hex")]
    public string? EntrypointHashHex { get; init; }

    [JsonPropertyName("gas_limit")]
    public ulong? GasLimit { get; init; }

    [JsonPropertyName("gas_used")]
    public ulong? GasUsed { get; init; }

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent? FeePayment { get; init; }

    [JsonPropertyName("payload_digest_hex")]
    public string PayloadDigestHex { get; init; } = string.Empty;
}

[JsonConverter(typeof(ToriiContractViewAccessHintsJsonConverter))]
public sealed record class ToriiContractViewAccessHints
{
    private string[] readKeys = Array.Empty<string>();
    private string[] writeKeys = Array.Empty<string>();

    [JsonPropertyName("read_keys")]
    public IReadOnlyList<string> ReadKeys
    {
        get => ToriiListSnapshots.CopyRequired(readKeys);
        init => readKeys = ToriiContractMetadataDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(ReadKeys));
    }

    [JsonPropertyName("write_keys")]
    public IReadOnlyList<string> WriteKeys
    {
        get => ToriiListSnapshots.CopyRequired(writeKeys);
        init => writeKeys = ToriiContractMetadataDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(WriteKeys));
    }
}

[JsonConverter(typeof(ToriiContractViewEntrypointParamJsonConverter))]
public sealed record class ToriiContractViewEntrypointParam
{
    private string name = string.Empty;
    private string typeName = string.Empty;

    [JsonPropertyName("name")]
    public string Name
    {
        get => name;
        init => name = ToriiContractMetadataDirectMetadata.RequireExactTokenText(value, nameof(Name));
    }

    [JsonPropertyName("type_name")]
    public string TypeName
    {
        get => typeName;
        init => typeName = ToriiContractMetadataDirectMetadata.RequireExactNonEmptyText(value, nameof(TypeName));
    }
}

[JsonConverter(typeof(ToriiContractViewEntrypointJsonConverter))]
public sealed record class ToriiContractViewEntrypoint
{
    private string name = string.Empty;
    private string kind = string.Empty;
    private ToriiContractViewEntrypointParam[] parameters = Array.Empty<ToriiContractViewEntrypointParam>();
    private string? returnType;
    private string? permission;
    private string[] readKeys = Array.Empty<string>();
    private string[] writeKeys = Array.Empty<string>();
    private string[] accessHintsSkipped = Array.Empty<string>();
    private string[] triggers = Array.Empty<string>();

    [JsonPropertyName("name")]
    public string Name
    {
        get => name;
        init => name = ToriiContractMetadataDirectMetadata.RequireExactTokenText(value, nameof(Name));
    }

    [JsonPropertyName("kind")]
    public string Kind
    {
        get => kind;
        init => kind = ToriiContractMetadataDirectMetadata.RequireExactTokenText(value, nameof(Kind));
    }

    [JsonPropertyName("params")]
    public IReadOnlyList<ToriiContractViewEntrypointParam> Parameters
    {
        get => ToriiListSnapshots.CopyRequired(parameters);
        init => parameters = ToriiListSnapshots.CopyNonNullItems(value, nameof(Parameters)) ?? Array.Empty<ToriiContractViewEntrypointParam>();
    }

    [JsonPropertyName("return_type")]
    public string? ReturnType
    {
        get => returnType;
        init => returnType = ToriiContractMetadataDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(ReturnType));
    }

    [JsonPropertyName("permission")]
    public string? Permission
    {
        get => permission;
        init => permission = ToriiContractMetadataDirectMetadata.RequireOptionalExactTokenText(
            value,
            nameof(Permission));
    }

    [JsonPropertyName("read_keys")]
    public IReadOnlyList<string> ReadKeys
    {
        get => ToriiListSnapshots.CopyRequired(readKeys);
        init => readKeys = ToriiContractMetadataDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(ReadKeys));
    }

    [JsonPropertyName("write_keys")]
    public IReadOnlyList<string> WriteKeys
    {
        get => ToriiListSnapshots.CopyRequired(writeKeys);
        init => writeKeys = ToriiContractMetadataDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(WriteKeys));
    }

    [JsonPropertyName("access_hints_complete")]
    public bool? AccessHintsComplete { get; init; }

    [JsonPropertyName("access_hints_skipped")]
    public IReadOnlyList<string> AccessHintsSkipped
    {
        get => ToriiListSnapshots.CopyRequired(accessHintsSkipped);
        init => accessHintsSkipped = ToriiContractMetadataDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(AccessHintsSkipped));
    }

    [JsonPropertyName("triggers")]
    public IReadOnlyList<string> Triggers
    {
        get => ToriiListSnapshots.CopyRequired(triggers);
        init => triggers = ToriiContractMetadataDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(Triggers));
    }
}

[JsonConverter(typeof(ToriiContractViewSyscallJsonConverter))]
public sealed record class ToriiContractViewSyscall
{
    private string? name;

    [JsonPropertyName("number")]
    public byte Number { get; init; }

    [JsonPropertyName("name")]
    public string? Name
    {
        get => name;
        init => name = ToriiContractMetadataDirectMetadata.RequireOptionalExactTokenText(value, nameof(Name));
    }

    [JsonPropertyName("count")]
    public ulong Count { get; init; }
}

[JsonConverter(typeof(ToriiContractViewMemoryJsonConverter))]
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

[JsonConverter(typeof(ToriiContractViewAnalysisJsonConverter))]
public sealed record class ToriiContractViewAnalysis
{
    private ToriiContractViewMemory memory = new();
    private ToriiContractViewSyscall[] syscalls = Array.Empty<ToriiContractViewSyscall>();

    [JsonPropertyName("instruction_count")]
    public ulong InstructionCount { get; init; }

    [JsonPropertyName("memory")]
    public ToriiContractViewMemory Memory
    {
        get => memory;
        init => memory = ToriiContractMetadataDirectMetadata.RequireObject(value, nameof(Memory));
    }

    [JsonPropertyName("syscalls")]
    public IReadOnlyList<ToriiContractViewSyscall> Syscalls
    {
        get => ToriiListSnapshots.CopyRequired(syscalls);
        init => syscalls = ToriiListSnapshots.CopyNonNullItems(value, nameof(Syscalls)) ?? Array.Empty<ToriiContractViewSyscall>();
    }
}

[JsonConverter(typeof(ToriiContractVerifiedSourceReferenceJsonConverter))]
public sealed record class ToriiContractVerifiedSourceReference
{
    private string language = string.Empty;
    private string? sourceName;
    private string submittedAt = string.Empty;
    private string? manifestIdHex;
    private string? payloadDigestHex;

    [JsonPropertyName("language")]
    public string Language
    {
        get => language;
        init => language = ToriiContractMetadataDirectMetadata.RequireExactNonEmptyText(value, nameof(Language));
    }

    [JsonPropertyName("source_name")]
    public string? SourceName
    {
        get => sourceName;
        init => sourceName = ToriiContractMetadataDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(SourceName));
    }

    [JsonPropertyName("submitted_at")]
    public string SubmittedAt
    {
        get => submittedAt;
        init => submittedAt = ToriiContractMetadataDirectMetadata.RequireExactNonEmptyText(
            value,
            nameof(SubmittedAt));
    }

    [JsonPropertyName("manifest_id_hex")]
    public string? ManifestIdHex
    {
        get => manifestIdHex;
        init => manifestIdHex = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(ManifestIdHex));
    }

    [JsonPropertyName("payload_digest_hex")]
    public string? PayloadDigestHex
    {
        get => payloadDigestHex;
        init => payloadDigestHex = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(PayloadDigestHex));
    }

    [JsonPropertyName("content_length")]
    public ulong? ContentLength { get; init; }
}

[JsonConverter(typeof(ToriiContractCodeViewJsonConverter))]
public sealed record class ToriiContractCodeView
{
    private string codeHash = string.Empty;
    private string? declaredCodeHash;
    private string? abiHash;
    private string? compilerFingerprint;
    private string[] permissions = Array.Empty<string>();
    private ToriiContractViewEntrypoint[] entrypoints = Array.Empty<ToriiContractViewEntrypoint>();
    private string[] warnings = Array.Empty<string>();
    private string renderedSourceKind = string.Empty;
    private string renderedSourceText = string.Empty;

    [JsonPropertyName("code_hash")]
    public string CodeHash
    {
        get => codeHash;
        init => codeHash = ToriiContractMetadataDirectMetadata.RequireExactSizedHex(value, nameof(CodeHash));
    }

    [JsonPropertyName("declared_code_hash")]
    public string? DeclaredCodeHash
    {
        get => declaredCodeHash;
        init => declaredCodeHash = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(DeclaredCodeHash));
    }

    [JsonPropertyName("abi_hash")]
    public string? AbiHash
    {
        get => abiHash;
        init => abiHash = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(AbiHash));
    }

    [JsonPropertyName("compiler_fingerprint")]
    public string? CompilerFingerprint
    {
        get => compilerFingerprint;
        init => compilerFingerprint = ToriiContractMetadataDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(CompilerFingerprint));
    }

    [JsonPropertyName("byte_len")]
    public ulong? ByteLength { get; init; }

    [JsonPropertyName("permissions")]
    public IReadOnlyList<string> Permissions
    {
        get => ToriiListSnapshots.CopyRequired(permissions);
        init => permissions = ToriiContractMetadataDirectMetadata.CopyRequiredExactTokenTextList(
            value,
            nameof(Permissions));
    }

    [JsonPropertyName("access_hints")]
    public ToriiContractViewAccessHints? AccessHints { get; init; }

    [JsonPropertyName("entrypoints")]
    public IReadOnlyList<ToriiContractViewEntrypoint> Entrypoints
    {
        get => ToriiListSnapshots.CopyRequired(entrypoints);
        init => entrypoints = ToriiListSnapshots.CopyNonNullItems(value, nameof(Entrypoints)) ?? Array.Empty<ToriiContractViewEntrypoint>();
    }

    [JsonPropertyName("analysis")]
    public ToriiContractViewAnalysis? Analysis { get; init; }

    [JsonPropertyName("warnings")]
    public IReadOnlyList<string> Warnings
    {
        get => ToriiListSnapshots.CopyRequired(warnings);
        init => warnings = ToriiContractMetadataDirectMetadata.CopyRequiredExactNonEmptyTextList(
            value,
            nameof(Warnings));
    }

    [JsonPropertyName("rendered_source_kind")]
    public string RenderedSourceKind
    {
        get => renderedSourceKind;
        init => renderedSourceKind = ToriiContractMetadataDirectMetadata.RequireExactTokenText(
            value,
            nameof(RenderedSourceKind));
    }

    [JsonPropertyName("rendered_source_text")]
    public string RenderedSourceText
    {
        get => renderedSourceText;
        init => renderedSourceText = ToriiContractMetadataDirectMetadata.RequireRenderedSourceText(
            value,
            nameof(RenderedSourceText));
    }

    [JsonPropertyName("verified_source_ref")]
    public ToriiContractVerifiedSourceReference? VerifiedSourceReference { get; init; }
}

public sealed record class ToriiContractCallResponse
{
    private bool ok;
    private string dataspace = string.Empty;
    private string? contractAddress;
    private string codeHashHex = string.Empty;
    private string abiHashHex = string.Empty;
    private ulong creationTimeMilliseconds;
    private string? transactionHashHex;
    private string? transactionPayloadBase64, signingMessageBase64;
    private string? entrypoint;

    [JsonPropertyName("ok")]
    public bool Ok
    {
        get => ok;
        init => ok = ToriiContractCallDirectMetadata.RequireTrue(value, nameof(Ok));
    }

    [JsonPropertyName("submitted")]
    public bool Submitted { get; init; }

    [JsonPropertyName("dataspace")]
    public string Dataspace
    {
        get => dataspace;
        init => dataspace = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(Dataspace));
    }

    [JsonPropertyName("contract_address")]
    public string? ContractAddress
    {
        get => contractAddress;
        init => contractAddress = ToriiContractCallDirectMetadata.RequireOptionalExactTokenText(
            value,
            nameof(ContractAddress));
    }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex
    {
        get => codeHashHex;
        init => codeHashHex = ToriiContractCallDirectMetadata.RequireExactSizedHex(value, nameof(CodeHashHex));
    }

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex
    {
        get => abiHashHex;
        init => abiHashHex = ToriiContractCallDirectMetadata.RequireExactSizedHex(value, nameof(AbiHashHex));
    }

    [JsonPropertyName("creation_time_ms")]
    public ulong CreationTimeMilliseconds
    {
        get => creationTimeMilliseconds;
        init => creationTimeMilliseconds = ToriiContractCallDirectMetadata.RequirePositive(
            value,
            nameof(CreationTimeMilliseconds));
    }

    [JsonPropertyName("transaction_ttl_ms")]
    public ulong? TransactionTimeToLiveMilliseconds { get; init; }

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex
    {
        get => transactionHashHex;
        init => transactionHashHex = ToriiContractCallDirectMetadata.RequireOptionalTransactionHashHex(
            value,
            nameof(TransactionHashHex));
    }

    [JsonPropertyName("pipeline_status")]
    public JsonNode? PipelineStatus { get; init; }

    [JsonPropertyName("entrypoint_hash_hex")]
    public string? EntrypointHashHex { get; init; }

    [JsonPropertyName("transaction_payload_b64")]
    public string? TransactionPayloadBase64
    {
        get => transactionPayloadBase64;
        init => transactionPayloadBase64 = ToriiContractCallDirectMetadata.RequireOptionalBase64(value, nameof(TransactionPayloadBase64));
    }
    [JsonPropertyName("signing_message_b64")]
    public string? SigningMessageBase64
    {
        get => signingMessageBase64;
        init => signingMessageBase64 = ToriiContractCallDirectMetadata.RequireOptionalBase64(
            value,
            nameof(SigningMessageBase64));
    }

    [JsonPropertyName("entrypoint")]
    public string? Entrypoint
    {
        get => entrypoint;
        init => entrypoint = ToriiContractCallDirectMetadata.RequireOptionalExactTokenText(
            value,
            nameof(Entrypoint));
    }

    [JsonPropertyName("operation_receipt")]
    public ToriiOperationReceipt OperationReceipt { get; init; } = null!;
}

[JsonConverter(typeof(ToriiContractViewResponseJsonConverter))]
public sealed record class ToriiContractViewResponse
{
    private bool ok;
    private string dataspace = string.Empty;
    private string contractId = string.Empty;
    private string? contractAddress;
    private string codeHashHex = string.Empty;
    private string abiHashHex = string.Empty;
    private string entrypoint = string.Empty;
    private JsonNode? result;

    [JsonPropertyName("ok")]
    public bool Ok
    {
        get => ok;
        init => ok = ToriiContractCallDirectMetadata.RequireTrue(value, nameof(Ok));
    }

    [JsonPropertyName("dataspace")]
    public string Dataspace
    {
        get => dataspace;
        init => dataspace = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(Dataspace));
    }

    [JsonPropertyName("contract_id")]
    public string ContractId
    {
        get => contractId;
        init => contractId = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(ContractId));
    }

    [JsonPropertyName("contract_address")]
    public string? ContractAddress
    {
        get => contractAddress;
        init => contractAddress = ToriiContractCallDirectMetadata.RequireOptionalExactTokenText(
            value,
            nameof(ContractAddress));
    }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex
    {
        get => codeHashHex;
        init => codeHashHex = ToriiContractCallDirectMetadata.RequireExactSizedHex(value, nameof(CodeHashHex));
    }

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex
    {
        get => abiHashHex;
        init => abiHashHex = ToriiContractCallDirectMetadata.RequireExactSizedHex(value, nameof(AbiHashHex));
    }

    [JsonPropertyName("entrypoint")]
    public string Entrypoint
    {
        get => entrypoint;
        init => entrypoint = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(Entrypoint));
    }

    [JsonPropertyName("result")]
    public JsonNode? Result
    {
        get => ToriiJsonSnapshots.Copy(result);
        init => result = ToriiJsonSnapshots.Copy(value);
    }
}

[JsonConverter(typeof(ToriiContractViewVmDiagnosticJsonConverter))]
public sealed record class ToriiContractViewVmDiagnostic
{
    private string trapKind = string.Empty;
    private string message = string.Empty;
    private string? function;
    private string? sourcePath;
    private ulong gasLimit;
    private ulong maxCycles;
    private ulong stackLimitBytes;
    private string? currentFunction;

    [JsonPropertyName("trap_kind")]
    public string TrapKind
    {
        get => trapKind;
        init => trapKind = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(TrapKind));
    }

    [JsonPropertyName("message")]
    public string Message
    {
        get => message;
        init => message = ToriiContractCallDirectMetadata.RequireExactNonEmptyText(value, nameof(Message));
    }

    [JsonPropertyName("pc")]
    public ulong ProgramCounter { get; init; }

    [JsonPropertyName("function")]
    public string? Function
    {
        get => function;
        init => function = ToriiContractCallDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(Function));
    }

    [JsonPropertyName("source_path")]
    public string? SourcePath
    {
        get => sourcePath;
        init => sourcePath = ToriiContractCallDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(SourcePath));
    }

    [JsonPropertyName("line")]
    public uint? Line { get; init; }

    [JsonPropertyName("column")]
    public uint? Column { get; init; }

    [JsonPropertyName("gas_limit")]
    public ulong GasLimit
    {
        get => gasLimit;
        init => gasLimit = ToriiContractCallDirectMetadata.RequirePositive(value, nameof(GasLimit));
    }

    [JsonPropertyName("gas_remaining")]
    public ulong GasRemaining { get; init; }

    [JsonPropertyName("gas_used")]
    public ulong GasUsed { get; init; }

    [JsonPropertyName("cycles")]
    public ulong Cycles { get; init; }

    [JsonPropertyName("max_cycles")]
    public ulong MaxCycles
    {
        get => maxCycles;
        init => maxCycles = ToriiContractCallDirectMetadata.RequirePositive(value, nameof(MaxCycles));
    }

    [JsonPropertyName("stack_limit_bytes")]
    public ulong StackLimitBytes
    {
        get => stackLimitBytes;
        init => stackLimitBytes = ToriiContractCallDirectMetadata.RequirePositive(
            value,
            nameof(StackLimitBytes));
    }

    [JsonPropertyName("stack_bytes_used")]
    public ulong StackBytesUsed { get; init; }

    [JsonPropertyName("entrypoint_pc")]
    public ulong? EntrypointProgramCounter { get; init; }

    [JsonPropertyName("current_function")]
    public string? CurrentFunction
    {
        get => currentFunction;
        init => currentFunction = ToriiContractCallDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(CurrentFunction));
    }

    [JsonPropertyName("opcode")]
    public ushort? Opcode { get; init; }

    [JsonPropertyName("syscall")]
    public uint? Syscall { get; init; }

    [JsonPropertyName("predecoded_loaded")]
    public bool PredecodedLoaded { get; init; }

    [JsonPropertyName("predecoded_hit")]
    public bool? PredecodedHit { get; init; }
}

[JsonConverter(typeof(ToriiContractViewErrorResponseJsonConverter))]
public sealed record class ToriiContractViewErrorResponse
{
    private bool ok;
    private string dataspace = string.Empty;
    private string contractId = string.Empty;
    private string? contractAddress;
    private string codeHashHex = string.Empty;
    private string abiHashHex = string.Empty;
    private string entrypoint = string.Empty;
    private string error = string.Empty;

    [JsonPropertyName("ok")]
    public bool Ok
    {
        get => ok;
        init => ok = ToriiContractCallDirectMetadata.RequireFalse(value, nameof(Ok));
    }

    [JsonPropertyName("dataspace")]
    public string Dataspace
    {
        get => dataspace;
        init => dataspace = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(Dataspace));
    }

    [JsonPropertyName("contract_id")]
    public string ContractId
    {
        get => contractId;
        init => contractId = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(ContractId));
    }

    [JsonPropertyName("contract_address")]
    public string? ContractAddress
    {
        get => contractAddress;
        init => contractAddress = ToriiContractCallDirectMetadata.RequireOptionalExactTokenText(
            value,
            nameof(ContractAddress));
    }

    [JsonPropertyName("code_hash_hex")]
    public string CodeHashHex
    {
        get => codeHashHex;
        init => codeHashHex = ToriiContractCallDirectMetadata.RequireExactSizedHex(value, nameof(CodeHashHex));
    }

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex
    {
        get => abiHashHex;
        init => abiHashHex = ToriiContractCallDirectMetadata.RequireExactSizedHex(value, nameof(AbiHashHex));
    }

    [JsonPropertyName("entrypoint")]
    public string Entrypoint
    {
        get => entrypoint;
        init => entrypoint = ToriiContractCallDirectMetadata.RequireExactTokenText(value, nameof(Entrypoint));
    }

    [JsonPropertyName("error")]
    public string Error
    {
        get => error;
        init => error = ToriiContractCallDirectMetadata.RequireExactNonEmptyText(value, nameof(Error));
    }

    [JsonPropertyName("vm_diagnostic")]
    public ToriiContractViewVmDiagnostic? VmDiagnostic { get; init; }
}

internal static class ToriiContractCallDirectMetadata
{
    internal static bool RequireTrue(bool value, string paramName)
    {
        if (!value)
        {
            throw new ArgumentException("Value must be true.", paramName);
        }

        return value;
    }

    internal static bool RequireFalse(bool value, string paramName)
    {
        if (value)
        {
            throw new ArgumentException("Value must be false.", paramName);
        }

        return value;
    }

    internal static string RequireExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
    }

    internal static string? RequireOptionalExactTokenText(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactTokenText(value, paramName);
    }

    internal static string RequireExactSizedHex(string? value, string paramName)
    {
        if (value is not null && value.Length == 0)
        {
            throw new ArgumentException("Value must be a non-empty 32-byte hex string.", paramName);
        }

        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }

    internal static string? RequireOptionalTransactionHashHex(string? value, string paramName)
    {
        if (value is null || value.Length == 0)
        {
            return value;
        }

        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }

    internal static ulong RequirePositive(ulong value, string paramName)
    {
        if (value == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive.");
        }

        return value;
    }

    internal static string? RequireOptionalBase64(string? value, string paramName)
    {
        return ToriiMultisigDirectMetadata.RequireOptionalBase64(value, paramName);
    }

    internal static string RequireExactNonEmptyText(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value must be a non-empty string.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        foreach (var character in value)
        {
            if (char.IsControl(character))
            {
                throw new ArgumentException("Value must not contain control characters.", paramName);
            }
        }

        return value;
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string paramName)
    {
        return value is null ? null : RequireExactNonEmptyText(value, paramName);
    }
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

[JsonConverter(typeof(ToriiContractVerifiedSourceJobJsonConverter))]
public sealed record class ToriiContractVerifiedSourceJob
{
    private string jobId = string.Empty;
    private string codeHash = string.Empty;
    private string status = string.Empty;
    private string submittedAt = string.Empty;
    private string? completedAt;
    private string? message;
    private string? actualCodeHash;

    [JsonPropertyName("job_id")]
    public string JobId
    {
        get => jobId;
        init => jobId = ToriiContractMetadataDirectMetadata.RequireExactTokenText(value, nameof(JobId));
    }

    [JsonPropertyName("code_hash")]
    public string CodeHash
    {
        get => codeHash;
        init => codeHash = ToriiContractMetadataDirectMetadata.RequireExactSizedHex(value, nameof(CodeHash));
    }

    [JsonPropertyName("status")]
    public string Status
    {
        get => status;
        init => status = ToriiContractMetadataDirectMetadata.RequireExactTokenText(value, nameof(Status));
    }

    [JsonPropertyName("submitted_at")]
    public string SubmittedAt
    {
        get => submittedAt;
        init => submittedAt = ToriiContractMetadataDirectMetadata.RequireExactNonEmptyText(
            value,
            nameof(SubmittedAt));
    }

    [JsonPropertyName("completed_at")]
    public string? CompletedAt
    {
        get => completedAt;
        init => completedAt = ToriiContractMetadataDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(CompletedAt));
    }

    [JsonPropertyName("message")]
    public string? Message
    {
        get => message;
        init => message = ToriiContractMetadataDirectMetadata.RequireOptionalExactNonEmptyText(
            value,
            nameof(Message));
    }

    [JsonPropertyName("actual_code_hash")]
    public string? ActualCodeHash
    {
        get => actualCodeHash;
        init => actualCodeHash = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(ActualCodeHash));
    }

    [JsonPropertyName("verified_source_ref")]
    public ToriiContractVerifiedSourceReference? VerifiedSourceReference { get; init; }
}

public sealed record class ToriiMultisigProposeRequest
{
    private string[]? instructions = Array.Empty<string>();

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

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; init; } = null!;

    [JsonPropertyName("instructions")]
    public IReadOnlyList<string>? Instructions
    {
        get => ToriiListSnapshots.Copy(instructions);
        init => instructions = ToriiListSnapshots.CopyNonNullItems(value, nameof(Instructions));
    }
}

public sealed record class ToriiMultisigContractCallProposeRequest
{
    private JsonNode? payload;

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

    [JsonPropertyName("contract_address")]
    public string? ContractAddress { get; init; }

    [JsonPropertyName("contract_alias")]
    public string? ContractAlias { get; init; }

    [JsonPropertyName("entrypoint")]
    public string Entrypoint { get; init; } = string.Empty;

    [JsonPropertyName("payload")]
    public JsonNode? Payload
    {
        get => ToriiJsonSnapshots.Copy(payload);
        init => payload = ToriiJsonSnapshots.Copy(value);
    }

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; init; } = null!;
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

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; init; } = null!;

    [JsonPropertyName("proposal_id")]
    public string? ProposalId { get; init; }

    [JsonPropertyName("instructions_hash")]
    public string? InstructionsHash { get; init; }
}

public sealed record class ToriiMultisigApproveRequest
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

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; init; } = null!;

    [JsonPropertyName("proposal_id")]
    public string? ProposalId { get; init; }

    [JsonPropertyName("instructions_hash")]
    public string? InstructionsHash { get; init; }
}

public sealed record class ToriiMultisigCancelRequest
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

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; init; } = null!;

    [JsonPropertyName("proposal_id")]
    public string? ProposalId { get; init; }

    [JsonPropertyName("instructions_hash")]
    public string? InstructionsHash { get; init; }
}

[JsonConverter(typeof(ToriiMultisigResponseJsonConverter))]
public sealed record class ToriiMultisigResponse
{
    private bool ok;
    private string resolvedMultisigAccountId = string.Empty;
    private string? proposalId;
    private string? instructionsHash;
    private string? transactionHashHex;
    private string? executedTransactionHashHex;
    private ulong? creationTimeMilliseconds;
    private string? transactionPayloadBase64, signingMessageBase64;

    [JsonPropertyName("ok")]
    public bool Ok
    {
        get => ok;
        init => ok = ToriiMultisigDirectMetadata.RequireTrue(value, nameof(Ok));
    }

    [JsonPropertyName("resolved_multisig_account_id")]
    public string ResolvedMultisigAccountId
    {
        get => resolvedMultisigAccountId;
        init => resolvedMultisigAccountId = ToriiMultisigDirectMetadata.RequireCanonicalAccountId(
            value,
            nameof(ResolvedMultisigAccountId));
    }

    [JsonPropertyName("submitted")]
    public bool Submitted { get; init; }

    [JsonPropertyName("proposal_id")]
    public string? ProposalId
    {
        get => proposalId;
        init => proposalId = ToriiMultisigDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(ProposalId));
    }

    [JsonPropertyName("instructions_hash")]
    public string? InstructionsHash
    {
        get => instructionsHash;
        init => instructionsHash = ToriiMultisigDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(InstructionsHash));
    }

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex
    {
        get => transactionHashHex;
        init => transactionHashHex = ToriiMultisigDirectMetadata.RequireOptionalTransactionHashHex(
            value,
            nameof(TransactionHashHex));
    }

    [JsonPropertyName("executed_tx_hash_hex")]
    public string? ExecutedTransactionHashHex
    {
        get => executedTransactionHashHex;
        init => executedTransactionHashHex = ToriiMultisigDirectMetadata.RequireOptionalTransactionHashHex(
            value,
            nameof(ExecutedTransactionHashHex));
    }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds
    {
        get => creationTimeMilliseconds;
        init => creationTimeMilliseconds = ToriiMultisigDirectMetadata.RequireOptionalPositive(
            value,
            nameof(CreationTimeMilliseconds));
    }

    [JsonPropertyName("transaction_payload_b64")]
    public string? TransactionPayloadBase64
    {
        get => transactionPayloadBase64;
        init => transactionPayloadBase64 = ToriiMultisigDirectMetadata.RequireOptionalBase64(value, nameof(TransactionPayloadBase64));
    }
    [JsonPropertyName("signing_message_b64")]
    public string? SigningMessageBase64
    {
        get => signingMessageBase64;
        init => signingMessageBase64 = ToriiMultisigDirectMetadata.RequireOptionalBase64(
            value,
            nameof(SigningMessageBase64));
    }
}

public sealed record class ToriiMultisigCancelResponse
{
    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("resolved_multisig_account_id")]
    public string ResolvedMultisigAccountId { get; init; } = string.Empty;

    [JsonPropertyName("submitted")]
    public bool Submitted { get; init; }

    [JsonPropertyName("action")]
    public string Action { get; init; } = string.Empty;

    [JsonPropertyName("target_proposal_id")]
    public string TargetProposalId { get; init; } = string.Empty;

    [JsonPropertyName("target_instructions_hash")]
    public string TargetInstructionsHash { get; init; } = string.Empty;

    [JsonPropertyName("cancel_proposal_id")]
    public string CancelProposalId { get; init; } = string.Empty;

    [JsonPropertyName("cancel_instructions_hash")]
    public string CancelInstructionsHash { get; init; } = string.Empty;

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex { get; init; }

    [JsonPropertyName("executed_tx_hash_hex")]
    public string? ExecutedTransactionHashHex { get; init; }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds { get; init; }

    [JsonPropertyName("transaction_payload_b64")]
    public string? TransactionPayloadBase64 { get; init; }
    [JsonPropertyName("signing_message_b64")]
    public string? SigningMessageBase64 { get; init; }
}

[JsonConverter(typeof(ToriiMultisigContractCallResponseJsonConverter))]
public sealed record class ToriiMultisigContractCallResponse
{
    private bool ok;
    private string resolvedMultisigAccountId = string.Empty;
    private string? proposalId;
    private string? instructionsHash;
    private string? transactionHashHex;
    private string? executedTransactionHashHex;
    private ulong? creationTimeMilliseconds;
    private string? transactionPayloadBase64, signingMessageBase64;

    [JsonPropertyName("ok")]
    public bool Ok
    {
        get => ok;
        init => ok = ToriiMultisigDirectMetadata.RequireTrue(value, nameof(Ok));
    }

    [JsonPropertyName("resolved_multisig_account_id")]
    public string ResolvedMultisigAccountId
    {
        get => resolvedMultisigAccountId;
        init => resolvedMultisigAccountId = ToriiMultisigDirectMetadata.RequireCanonicalAccountId(
            value,
            nameof(ResolvedMultisigAccountId));
    }

    [JsonPropertyName("submitted")]
    public bool Submitted { get; init; }

    [JsonPropertyName("proposal_id")]
    public string? ProposalId
    {
        get => proposalId;
        init => proposalId = ToriiMultisigDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(ProposalId));
    }

    [JsonPropertyName("instructions_hash")]
    public string? InstructionsHash
    {
        get => instructionsHash;
        init => instructionsHash = ToriiMultisigDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(InstructionsHash));
    }

    [JsonPropertyName("tx_hash_hex")]
    public string? TransactionHashHex
    {
        get => transactionHashHex;
        init => transactionHashHex = ToriiMultisigDirectMetadata.RequireOptionalTransactionHashHex(
            value,
            nameof(TransactionHashHex));
    }

    [JsonPropertyName("executed_tx_hash_hex")]
    public string? ExecutedTransactionHashHex
    {
        get => executedTransactionHashHex;
        init => executedTransactionHashHex = ToriiMultisigDirectMetadata.RequireOptionalTransactionHashHex(
            value,
            nameof(ExecutedTransactionHashHex));
    }

    [JsonPropertyName("creation_time_ms")]
    public ulong? CreationTimeMilliseconds
    {
        get => creationTimeMilliseconds;
        init => creationTimeMilliseconds = ToriiMultisigDirectMetadata.RequireOptionalPositive(
            value,
            nameof(CreationTimeMilliseconds));
    }

    [JsonPropertyName("transaction_payload_b64")]
    public string? TransactionPayloadBase64
    {
        get => transactionPayloadBase64;
        init => transactionPayloadBase64 = ToriiMultisigDirectMetadata.RequireOptionalBase64(value, nameof(TransactionPayloadBase64));
    }
    [JsonPropertyName("signing_message_b64")]
    public string? SigningMessageBase64
    {
        get => signingMessageBase64;
        init => signingMessageBase64 = ToriiMultisigDirectMetadata.RequireOptionalBase64(
            value,
            nameof(SigningMessageBase64));
    }
}

internal static class ToriiMultisigDirectMetadata
{
    internal static bool RequireTrue(bool value, string paramName)
    {
        if (!value)
        {
            throw new ArgumentException("Value must be true.", paramName);
        }

        return value;
    }

    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value, paramName);
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(value, paramName, 32);
    }

    internal static string? RequireOptionalTransactionHashHex(string? value, string paramName)
    {
        if (value is null || value.Length == 0)
        {
            return value;
        }

        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }

    internal static ulong? RequireOptionalPositive(ulong? value, string paramName)
    {
        if (value == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Value must be positive when provided.");
        }

        return value;
    }

    internal static string? RequireOptionalBase64(string? value, string paramName)
    {
        if (value is null)
        {
            return null;
        }

        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value must be a non-empty base64 string.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new ArgumentException("Value must not contain whitespace.", paramName);
            }

            if (char.IsControl(character))
            {
                throw new ArgumentException("Value must not contain control characters.", paramName);
            }
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(value);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException("Value must be valid base64.", paramName, exception);
        }

        if (bytes.Length == 0)
        {
            throw new ArgumentException("Value must not decode to empty bytes.", paramName);
        }

        if (!string.Equals(Convert.ToBase64String(bytes), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be canonical base64 text.", paramName);
        }

        return value;
    }
}

[JsonConverter(typeof(ToriiRuntimeUpgradeCountersJsonConverter))]
public sealed record class ToriiRuntimeUpgradeCounters
{
    private long proposed;
    private long activated;
    private long canceled;

    [JsonPropertyName("proposed")]
    public long Proposed
    {
        get => proposed;
        init => proposed = ToriiRuntimeDirectMetadata.RequireNonNegativeInt64(value, nameof(Proposed));
    }

    [JsonPropertyName("activated")]
    public long Activated
    {
        get => activated;
        init => activated = ToriiRuntimeDirectMetadata.RequireNonNegativeInt64(value, nameof(Activated));
    }

    [JsonPropertyName("canceled")]
    public long Canceled
    {
        get => canceled;
        init => canceled = ToriiRuntimeDirectMetadata.RequireNonNegativeInt64(value, nameof(Canceled));
    }
}

[JsonConverter(typeof(ToriiRuntimeMetricsJsonConverter))]
public sealed record class ToriiRuntimeMetrics
{
    private int abiVersion = 1;
    private ToriiRuntimeUpgradeCounters upgradeEventsTotal = new();

    [JsonPropertyName("abi_version")]
    public int AbiVersion
    {
        get => abiVersion;
        init => abiVersion = ToriiRuntimeDirectMetadata.RequireAbiVersionV1(value, nameof(AbiVersion));
    }

    [JsonPropertyName("upgrade_events_total")]
    public ToriiRuntimeUpgradeCounters UpgradeEventsTotal
    {
        get => upgradeEventsTotal;
        init => upgradeEventsTotal = ToriiRuntimeDirectMetadata.RequireObject(value, nameof(UpgradeEventsTotal));
    }
}

[JsonConverter(typeof(ToriiRuntimeAbiActiveJsonConverter))]
public sealed record class ToriiRuntimeAbiActive
{
    private int abiVersion = 1;

    [JsonPropertyName("abi_version")]
    public int AbiVersion
    {
        get => abiVersion;
        init => abiVersion = ToriiRuntimeDirectMetadata.RequireAbiVersionV1(value, nameof(AbiVersion));
    }
}

[JsonConverter(typeof(ToriiRuntimeAbiHashJsonConverter))]
public sealed record class ToriiRuntimeAbiHash
{
    private string policy = "V1";
    private string abiHashHex = string.Empty;

    [JsonPropertyName("policy")]
    public string Policy
    {
        get => policy;
        init => policy = ToriiRuntimeDirectMetadata.RequirePolicyV1(value, nameof(Policy));
    }

    [JsonPropertyName("abi_hash_hex")]
    public string AbiHashHex
    {
        get => abiHashHex;
        init => abiHashHex = ToriiRuntimeDirectMetadata.RequireExactSizedHex(value, nameof(AbiHashHex));
    }
}

internal static class ToriiRuntimeDirectMetadata
{
    internal static T RequireObject<T>(T? value, string paramName)
        where T : class
    {
        return value ?? throw new ArgumentNullException(paramName, "Value must not be null.");
    }

    internal static int RequireAbiVersionV1(int value, string paramName)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, value, "Value must be non-negative.");
        }

        if (value != 1)
        {
            throw new ArgumentException("Value must be 1.", paramName);
        }

        return value;
    }

    internal static long RequireNonNegativeInt64(long value, string paramName)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, value, "Value must be non-negative.");
        }

        return value;
    }

    internal static string RequirePolicyV1(string? value, string paramName)
    {
        var exact = ToriiExplorerDirectMetadata.RequireExactTokenText(value, paramName);
        if (!string.Equals(exact, "V1", StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must be V1.", paramName);
        }

        return exact;
    }

    internal static string RequireExactSizedHex(string? value, string paramName)
    {
        return ToriiExplorerDirectMetadata.RequireExactSizedHex(value, paramName, 32);
    }
}
