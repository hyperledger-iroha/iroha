using Hyperledger.Iroha.Offline;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class KagemushaRecursiveSpendNativeTests
{
    [Fact]
    public void RecursiveSpendNativeAvailabilityProbeDoesNotThrow()
    {
        _ = KagemushaRecursiveSpendNative.IsAvailable();
    }

    [Fact]
    public void RecursiveSpendNativePreferredModeDefaultsToRecursiveWhenAvailable()
    {
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveSpendV1,
            KagemushaRecursiveSpendNative.PreferredMode(true));
        Assert.Equal(
            KagemushaOfflineSpendMode.CheckedPrefoldV1,
            KagemushaRecursiveSpendNative.PreferredMode(false));
        Assert.Equal(
            "recursive_spend_v1",
            KagemushaOfflineSpendMode.RecursiveSpendV1.WireName());
        Assert.Equal(
            "checked_prefold_v1",
            KagemushaOfflineSpendMode.CheckedPrefoldV1.WireName());
        Assert.Equal(6u, KagemushaRecursiveSpendNative.RequiredBridgeAbiVersion);
        Assert.Equal(
            "kagemusha-recursive-aggregation-v1",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1);

        _ = KagemushaRecursiveSpendNative.PreferredMode();
    }

    [Fact]
    public void RecursiveSpendNativeRejectsEmptyArchivesBeforeLoadingNativeBridge()
    {
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Init(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Append(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            Array.Empty<byte>(),
            new byte[] { 0x01 }));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            new byte[] { 0x01 },
            Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            Array.Empty<byte>(),
            new byte[] { 0x01 },
            new byte[] { 0x02 }));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            new byte[] { 0x01 },
            Array.Empty<byte>(),
            new byte[] { 0x02 }));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            new byte[] { 0x01 },
            new byte[] { 0x02 },
            Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Verify(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Redeem(Array.Empty<byte>()));
    }

    [Fact]
    public void RecursiveSpendNativeRejectsMalformedArchivesWhenBridgeIsAvailable()
    {
        if (!KagemushaRecursiveSpendNative.IsAvailable())
        {
            return;
        }

        var malformed = new byte[] { 0x01, 0x02 };
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Init(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Append(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            malformed,
            new byte[] { 0x03, 0x04 }));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            malformed,
            new byte[] { 0x03, 0x04 },
            new byte[] { 0x05, 0x06 }));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Verify(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Redeem(malformed));
    }
}
