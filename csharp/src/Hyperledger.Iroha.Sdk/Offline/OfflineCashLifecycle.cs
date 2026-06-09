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
    uint? BridgeAbiVersion = null,
    string? ArtifactSetId = null,
    string? CircuitId = null,
    ulong CreatedAtMs = 0,
    ulong? ExpiresAtMs = null)
{
    public void RequireUsableForOfflineExchange(
        ulong nowMs,
        uint? requiredBridgeAbiVersion = null)
    {
        if (!OfflinePaymentsEnabled)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "offline_payments_disabled",
                "Offline cash is disabled in the cached configuration snapshot.");
        }

        if (string.IsNullOrWhiteSpace(IssuerPublicKeyBase64))
        {
            throw new OfflineCashConfigurationSnapshotException(
                "missing_issuer_public_key",
                "Offline cash requires a cached issuer public key before offline exchange.");
        }

        if (ExpiresAtMs is { } expiresAtMs && expiresAtMs <= nowMs)
        {
            throw new OfflineCashConfigurationSnapshotException(
                "expired",
                $"Offline cash configuration snapshot expired at {expiresAtMs}.");
        }

        if (requiredBridgeAbiVersion is { } required
            && (!BridgeAbiVersion.HasValue || BridgeAbiVersion.Value < required))
        {
            throw new OfflineCashConfigurationSnapshotException(
                "unsupported_bridge_abi",
                $"Offline cash requires bridge ABI {required}.");
        }
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
