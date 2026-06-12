using Hyperledger.Iroha.Offline;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineCashLifecycleTests
{
    [Fact]
    public void TransportCapabilitiesHideUnsupportedNfc()
    {
        var capabilities = new OfflineCashTransportCapabilities(
            QrStreaming: true,
            Nfc: OfflineCashNfcCapability.Unavailable("missing HCE"),
            Nearby: true);

        Assert.Equal(
            new[] { OfflineCashTransportKinds.Qr, OfflineCashTransportKinds.Nearby },
            capabilities.SupportedTransportKinds());
        Assert.Equal(
            new[] { OfflineCashTransportKinds.Qr, OfflineCashTransportKinds.Nearby },
            new OfflineCashTransportCapabilities(
                    QrStreaming: true,
                    Nfc: null!,
                    Nearby: true)
                .SupportedTransportKinds());
    }

    [Fact]
    public async Task LifecycleSyncsPendingAuditReceiptsBeforeLoad()
    {
        var events = new List<string>();
        var controller = new OfflineCashLifecycleController(
            new RecordingWallet(events),
            new RecordingSynchronizer(events, hasPending: true));

        var result = await controller.LoadAsync("pkr#sbp", "10");

        Assert.Equal("ok", result);
        Assert.Equal(new[] { "hasPending", "sync", "load:pkr#sbp:10" }, events);
    }

    [Fact]
    public async Task LifecycleDoesNotLoadWhenAuditReceiptSyncFails()
    {
        var events = new List<string>();
        var controller = new OfflineCashLifecycleController(
            new RecordingWallet(events),
            new RecordingSynchronizer(
                events,
                hasPending: true,
                syncFailure: new InvalidOperationException("audit sync failed")));

        var error = await Assert.ThrowsAsync<InvalidOperationException>(
            async () => await controller.LoadAsync("pkr#sbp", "10"));

        Assert.Equal("audit sync failed", error.Message);
        Assert.Equal(new[] { "hasPending", "sync" }, events);
    }

    [Fact]
    public void ConfigurationSnapshotRequiresCachedIssuerKey()
    {
        var snapshot = new OfflineCashConfigurationSnapshot(
            ChainId: "00000042",
            AssetDefinitionId: "pkr#sbp",
            OfflinePaymentsEnabled: true,
            IssuerPublicKeyBase64: "issuer-key",
            NativeBridgeAbiVersion: 7,
            CreatedAtMs: 100,
            ExpiresAtMs: 1_000);

        snapshot.RequireUsableForOfflineExchange(nowMs: 999, requiredNativeBridgeAbiVersion: 7);

        var missingKey = snapshot with { IssuerPublicKeyBase64 = " " };
        var error = Assert.Throws<OfflineCashConfigurationSnapshotException>(
            () => missingKey.RequireUsableForOfflineExchange(nowMs: 200));
        Assert.Equal("missing_issuer_public_key", error.Code);

        foreach (var issuerKey in new[]
        {
            "",
            " issuer-key",
            "issuer-key ",
            "issuer key",
            "issuer-key\n",
            "issuer-key\u2603",
        })
        {
            var noncanonical = snapshot with { IssuerPublicKeyBase64 = issuerKey };
            error = Assert.Throws<OfflineCashConfigurationSnapshotException>(
                () => noncanonical.RequireUsableForOfflineExchange(
                    nowMs: 200,
                    requiredNativeBridgeAbiVersion: 7));
            Assert.Equal("missing_issuer_public_key", error.Code);
        }

        var disabled = snapshot with { OfflinePaymentsEnabled = false };
        error = Assert.Throws<OfflineCashConfigurationSnapshotException>(
            () => disabled.RequireUsableForOfflineExchange(nowMs: 200, requiredNativeBridgeAbiVersion: 7));
        Assert.Equal("offline_payments_disabled", error.Code);

        var staleAbi = snapshot with { NativeBridgeAbiVersion = 6 };
        error = Assert.Throws<OfflineCashConfigurationSnapshotException>(
            () => staleAbi.RequireUsableForOfflineExchange(nowMs: 200, requiredNativeBridgeAbiVersion: 7));
        Assert.Equal("unsupported_native_bridge_abi", error.Code);

        error = Assert.Throws<OfflineCashConfigurationSnapshotException>(
            () => snapshot.RequireUsableForOfflineExchange(nowMs: 1_000, requiredNativeBridgeAbiVersion: 7));
        Assert.Equal("expired", error.Code);
    }

    [Fact]
    public void KagemushaWireNameConstantsAreCanonical()
    {
        Assert.Equal(
            "iroha_data_model::isi::offline::KagemushaTransfer",
            KagemushaWireNames.TransferInstruction);
        Assert.Equal(
            "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
            KagemushaWireNames.RedeemRecursiveInstruction);
        Assert.Equal(
            "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1",
            KagemushaWireNames.RecursiveRedeemRequest);
        Assert.Equal(KagemushaWireNames.TransferInstruction, KagemushaInstructionType.Transfer.WireName());
    }

    private sealed class RecordingSynchronizer : IOfflineCashAuditReceiptSynchronizer
    {
        private readonly List<string> events;
        private readonly bool hasPending;
        private readonly Exception? syncFailure;

        public RecordingSynchronizer(List<string> events, bool hasPending, Exception? syncFailure = null)
        {
            this.events = events;
            this.hasPending = hasPending;
            this.syncFailure = syncFailure;
        }

        public ValueTask<bool> HasPendingAuditReceiptsAsync(CancellationToken cancellationToken = default)
        {
            events.Add("hasPending");
            return ValueTask.FromResult(hasPending);
        }

        public ValueTask SyncPendingAuditReceiptsAsync(CancellationToken cancellationToken = default)
        {
            events.Add("sync");
            if (syncFailure is not null)
            {
                throw syncFailure;
            }

            return ValueTask.CompletedTask;
        }
    }

    private sealed class RecordingWallet : IOfflineCashLifecycleWallet
    {
        private readonly List<string> events;

        public RecordingWallet(List<string> events)
        {
            this.events = events;
        }

        public ValueTask<object?> LoadAsync(
            string assetDefinitionId,
            string amount,
            CancellationToken cancellationToken = default)
        {
            events.Add($"load:{assetDefinitionId}:{amount}");
            return ValueTask.FromResult<object?>("ok");
        }

        public object? PrepareReceive(string assetDefinitionId, string amount)
        {
            throw new NotSupportedException();
        }

        public object? CreatePayment(object receiveRequest)
        {
            throw new NotSupportedException();
        }

        public object? AcceptPayment(object paymentToken)
        {
            throw new NotSupportedException();
        }

        public ValueTask<object?> RedeemAsync(
            object note,
            string? recipient = null,
            CancellationToken cancellationToken = default)
        {
            throw new NotSupportedException();
        }
    }
}
