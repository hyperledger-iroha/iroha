namespace Hyperledger.Iroha.Offline;

public static class OfflineCashTransportKinds
{
    public const string Qr = "qr";
    public const string Nfc = "nfc";
    public const string Nearby = "nearby";
}

public sealed record class OfflineCashNfcCapability(bool Supported, string? Reason = null)
{
    public static OfflineCashNfcCapability Available { get; } = new(true);

    public static OfflineCashNfcCapability Unavailable(string reason)
    {
        return new OfflineCashNfcCapability(false, reason);
    }
}

public sealed record class OfflineCashTransportCapabilities(
    bool QrStreaming,
    OfflineCashNfcCapability? Nfc,
    bool Nearby)
{
    public IReadOnlyList<string> SupportedTransportKinds()
    {
        var kinds = new List<string>();
        if (QrStreaming)
        {
            kinds.Add(OfflineCashTransportKinds.Qr);
        }

        if (Nfc?.Supported == true)
        {
            kinds.Add(OfflineCashTransportKinds.Nfc);
        }

        if (Nearby)
        {
            kinds.Add(OfflineCashTransportKinds.Nearby);
        }

        return kinds;
    }
}

public sealed record class OfflineCashConfigurationSnapshot(
    string ChainId,
    string AssetDefinitionId,
    bool OfflinePaymentsEnabled,
    string? IssuerPublicKeyBase64,
    uint? NativeBridgeAbiVersion = null,
    string? ArtifactSetId = null,
    string? CircuitId = null,
    ulong CreatedAtMs = 0,
    ulong? ExpiresAtMs = null)
{
    public void RequireUsableForOfflineExchange(
        ulong nowMs,
        uint? requiredNativeBridgeAbiVersion = null)
    {
        if (!OfflinePaymentsEnabled)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "offline_payments_disabled",
                "Offline cash is disabled in the cached configuration snapshot.");
        }

        RequireCanonicalSnapshotField(ChainId, "chain_id", "invalid_chain_id");
        RequireCanonicalSnapshotField(AssetDefinitionId, "asset_definition_id", "invalid_asset_definition_id");
        RequireOptionalCanonicalSnapshotField(ArtifactSetId, "artifact_set_id", "invalid_artifact_set_id");
        RequireOptionalCanonicalSnapshotField(CircuitId, "circuit_id", "invalid_circuit_id");

        if (!IsCanonicalBase64SnapshotText(IssuerPublicKeyBase64))
        {
            throw new OfflineCashConfigurationSnapshotException(
                "missing_issuer_public_key",
                "Offline cash requires a cached issuer public key before offline exchange.");
        }

        if (ExpiresAtMs is { } invalidExpiresAtMs
            && CreatedAtMs != 0
            && invalidExpiresAtMs <= CreatedAtMs)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "invalid_snapshot_timestamps",
                "Offline cash configuration snapshot expiry must be after its creation time.");
        }

        if (CreatedAtMs != 0 && CreatedAtMs > nowMs)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "snapshot_created_in_future",
                $"Offline cash configuration snapshot was created at {CreatedAtMs}, after the current time.");
        }

        if (ExpiresAtMs is { } expiresAtMs && expiresAtMs <= nowMs)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "expired",
                $"Offline cash configuration snapshot expired at {expiresAtMs}.");
        }

        if (NativeBridgeAbiVersion is 0)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "invalid_native_bridge_abi",
                "Offline cash cached native bridge ABI version must be positive when present.");
        }

        if (requiredNativeBridgeAbiVersion is 0)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "invalid_required_native_bridge_abi",
                "Offline cash required native bridge ABI version must be positive when present.");
        }

        if (requiredNativeBridgeAbiVersion is { } required
            && (!NativeBridgeAbiVersion.HasValue || NativeBridgeAbiVersion.Value < required))
        {
            throw new OfflineCashConfigurationSnapshotException(
                "unsupported_native_bridge_abi",
                $"Offline cash requires native bridge ABI {required}.");
        }
    }

    private static void RequireCanonicalSnapshotField(string? value, string field, string code)
    {
        if (!IsCanonicalSnapshotText(value))
        {
            throw new OfflineCashConfigurationSnapshotException(
                code,
                $"Offline cash configuration snapshot field {field} is invalid.");
        }
    }

    private static void RequireOptionalCanonicalSnapshotField(string? value, string field, string code)
    {
        if (value is not null)
        {
            RequireCanonicalSnapshotField(value, field, code);
        }
    }

    private static bool IsCanonicalSnapshotText(string? value)
    {
        if (string.IsNullOrEmpty(value))
        {
            return false;
        }

        foreach (var ch in value)
        {
            if (ch <= 0x20 || ch > 0x7E)
            {
                return false;
            }
        }

        return true;
    }

    private static bool IsCanonicalBase64SnapshotText(string? value)
    {
        if (string.IsNullOrEmpty(value))
        {
            return false;
        }

        var text = value;
        if (!IsCanonicalSnapshotText(text))
        {
            return false;
        }

        byte[] decoded;
        try
        {
            decoded = Convert.FromBase64String(text);
        }
        catch (FormatException)
        {
            return false;
        }

        return decoded.Length > 0
            && string.Equals(Convert.ToBase64String(decoded), text, StringComparison.Ordinal);
    }
}

public sealed class OfflineCashConfigurationSnapshotException : InvalidOperationException
{
    public OfflineCashConfigurationSnapshotException(string code, string message)
        : base(message)
    {
        Code = code;
    }

    public string Code { get; }
}

public interface IOfflineCashAuditReceiptSynchronizer
{
    ValueTask<bool> HasPendingAuditReceiptsAsync(CancellationToken cancellationToken = default);

    ValueTask SyncPendingAuditReceiptsAsync(CancellationToken cancellationToken = default);
}

public interface IOfflineCashLifecycleWallet
{
    ValueTask<object?> LoadAsync(
        string assetDefinitionId,
        string amount,
        CancellationToken cancellationToken = default);

    object? PrepareReceive(string assetDefinitionId, string amount);

    object? CreatePayment(object receiveRequest);

    object? AcceptPayment(object paymentToken);

    ValueTask<object?> RedeemAsync(
        object note,
        string? recipient = null,
        CancellationToken cancellationToken = default);
}

public sealed class OfflineCashLifecycleController
{
    private readonly IOfflineCashLifecycleWallet wallet;
    private readonly IOfflineCashAuditReceiptSynchronizer? auditReceiptSynchronizer;

    public OfflineCashLifecycleController(
        IOfflineCashLifecycleWallet wallet,
        IOfflineCashAuditReceiptSynchronizer? auditReceiptSynchronizer = null)
    {
        this.wallet = wallet ?? throw new ArgumentNullException(nameof(wallet));
        this.auditReceiptSynchronizer = auditReceiptSynchronizer;
    }

    public async ValueTask<bool> SyncPendingAuditReceiptsIfNeededAsync(
        CancellationToken cancellationToken = default)
    {
        if (auditReceiptSynchronizer is null)
        {
            return false;
        }

        if (!await auditReceiptSynchronizer.HasPendingAuditReceiptsAsync(cancellationToken)
                .ConfigureAwait(false))
        {
            return false;
        }

        await auditReceiptSynchronizer.SyncPendingAuditReceiptsAsync(cancellationToken)
            .ConfigureAwait(false);
        return true;
    }

    public async ValueTask<object?> LoadAsync(
        string assetDefinitionId,
        string amount,
        CancellationToken cancellationToken = default)
    {
        await SyncPendingAuditReceiptsIfNeededAsync(cancellationToken).ConfigureAwait(false);
        return await wallet.LoadAsync(assetDefinitionId, amount, cancellationToken).ConfigureAwait(false);
    }

    public object? PrepareReceive(string assetDefinitionId, string amount)
    {
        return wallet.PrepareReceive(assetDefinitionId, amount);
    }

    public object? CreatePayment(object receiveRequest)
    {
        return wallet.CreatePayment(receiveRequest);
    }

    public object? AcceptPayment(object paymentToken)
    {
        return wallet.AcceptPayment(paymentToken);
    }

    public ValueTask<object?> RedeemAsync(
        object note,
        string? recipient = null,
        CancellationToken cancellationToken = default)
    {
        return wallet.RedeemAsync(note, recipient, cancellationToken);
    }
}
