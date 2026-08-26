using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

[JsonConverter(typeof(ToriiVpnProfileJsonConverter))]
public sealed record class ToriiVpnProfile
{
    private string relayEndpoint = string.Empty;
    private string[]? supportedExitClasses = Array.Empty<string>();
    private string defaultExitClass = string.Empty;
    private ulong leaseSeconds;
    private ulong dnsPushIntervalSeconds;
    private string meterFamily = string.Empty;
    private string[]? routePushes = Array.Empty<string>();
    private string[]? excludedRoutes = Array.Empty<string>();
    private string[]? dnsServers = Array.Empty<string>();
    private string[]? tunnelAddresses = Array.Empty<string>();
    private ulong mtuBytes;
    private string displayBillingLabel = string.Empty;
    private string operatorAccountId = string.Empty;
    private string leaseFee = string.Empty;
    private string relayIdHex = string.Empty;
    private string relayMldsa65PublicKeyHex = string.Empty;
    private string descriptorCommitHex = string.Empty;
    private string tlsServerName = string.Empty;
    private string relayTlsSpkiSha256Hex = string.Empty;
    private string relayCertificateSha256Hex = string.Empty;
    private string directorySnapshotDigestHex = string.Empty;

    [JsonPropertyName("available")]
    public bool Available { get; init; }

    [JsonPropertyName("relay_endpoint")]
    public string RelayEndpoint
    {
        get => relayEndpoint;
        init => relayEndpoint = ToriiVpnDirectMetadata.RequireEmptyOrExactTokenText(value, nameof(RelayEndpoint));
    }

    [JsonPropertyName("supported_exit_classes")]
    public IReadOnlyList<string>? SupportedExitClasses
    {
        get => ToriiListSnapshots.Copy(supportedExitClasses);
        init => supportedExitClasses = ToriiVpnDirectMetadata.CopyOptionalExactTokenTextList(
            value,
            nameof(SupportedExitClasses));
    }

    [JsonPropertyName("default_exit_class")]
    public string DefaultExitClass
    {
        get => defaultExitClass;
        init => defaultExitClass = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(DefaultExitClass));
    }

    [JsonPropertyName("lease_secs")]
    public ulong LeaseSeconds
    {
        get => leaseSeconds;
        init => leaseSeconds = ToriiVpnDirectMetadata.RequirePositive(value, nameof(LeaseSeconds));
    }

    [JsonPropertyName("dns_push_interval_secs")]
    public ulong DnsPushIntervalSeconds
    {
        get => dnsPushIntervalSeconds;
        init => dnsPushIntervalSeconds = ToriiVpnDirectMetadata.RequireAtLeast(
            value,
            30,
            nameof(DnsPushIntervalSeconds));
    }

    [JsonPropertyName("meter_family")]
    public string MeterFamily
    {
        get => meterFamily;
        init => meterFamily = ToriiVpnDirectMetadata.RequireExactTokenText(value, nameof(MeterFamily));
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

    [JsonPropertyName("display_billing_label")]
    public string DisplayBillingLabel
    {
        get => displayBillingLabel;
        init => displayBillingLabel = ToriiVpnDirectMetadata.RequireExactNonEmptyText(
            value,
            nameof(DisplayBillingLabel));
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

    [JsonPropertyName("settlement_grace_secs")]
    public ulong SettlementGraceSeconds { get; init; }

    [JsonPropertyName("flow_label_bits")]
    public byte FlowLabelBits { get; init; }

    [JsonPropertyName("padding_budget_ms")]
    public ushort PaddingBudgetMilliseconds { get; init; }

    [JsonPropertyName("relay_id_hex")]
    public string RelayIdHex
    {
        get => relayIdHex;
        init => relayIdHex = ToriiVpnDirectMetadata.RequireEmptyOrExactSizedHex(value, nameof(RelayIdHex), 32);
    }

    [JsonPropertyName("relay_mldsa65_public_key_hex")]
    public string RelayMldsa65PublicKeyHex
    {
        get => relayMldsa65PublicKeyHex;
        init => relayMldsa65PublicKeyHex = ToriiVpnDirectMetadata.RequireEmptyOrNonZeroExactSizedHex(
            value,
            nameof(RelayMldsa65PublicKeyHex),
            ToriiVpnDirectMetadata.RelayMldsa65PublicKeyByteLength);
    }

    [JsonPropertyName("descriptor_commit_hex")]
    public string DescriptorCommitHex
    {
        get => descriptorCommitHex;
        init => descriptorCommitHex = ToriiVpnDirectMetadata.RequireEmptyOrExactSizedHex(value, nameof(DescriptorCommitHex), 32);
    }

    [JsonPropertyName("tls_server_name")]
    public string TlsServerName
    {
        get => tlsServerName;
        init => tlsServerName = ToriiVpnDirectMetadata.RequireEmptyOrExactTokenText(value, nameof(TlsServerName));
    }

    [JsonPropertyName("relay_tls_spki_sha256_hex")]
    public string RelayTlsSpkiSha256Hex
    {
        get => relayTlsSpkiSha256Hex;
        init => relayTlsSpkiSha256Hex = ToriiVpnDirectMetadata.RequireEmptyOrExactSizedHex(value, nameof(RelayTlsSpkiSha256Hex), 32);
    }

    [JsonPropertyName("relay_certificate_sha256_hex")]
    public string RelayCertificateSha256Hex
    {
        get => relayCertificateSha256Hex;
        init => relayCertificateSha256Hex = ToriiVpnDirectMetadata.RequireEmptyOrExactSizedHex(value, nameof(RelayCertificateSha256Hex), 32);
    }

    [JsonPropertyName("directory_snapshot_digest_hex")]
    public string DirectorySnapshotDigestHex
    {
        get => directorySnapshotDigestHex;
        init => directorySnapshotDigestHex = ToriiVpnDirectMetadata.RequireEmptyOrExactSizedHex(value, nameof(DirectorySnapshotDigestHex), 32);
    }
}

[JsonConverter(typeof(ToriiVpnTxInstructionJsonConverter))]
public sealed record class ToriiVpnTxInstruction
{
    private string wireId = string.Empty;
    private string payloadHex = string.Empty;

    [JsonPropertyName("wire_id")]
    public string WireId
    {
        get => wireId;
        init => wireId = ToriiVpnDirectMetadata.RequireExactNonEmptyText(value, nameof(WireId));
    }

    [JsonPropertyName("payload_hex")]
    public string PayloadHex
    {
        get => payloadHex;
        init => payloadHex = ToriiVpnDirectMetadata.RequireExactEvenLengthHex(value, nameof(PayloadHex));
    }
}
