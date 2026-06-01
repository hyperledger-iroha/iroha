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

        _ = KagemushaRecursiveSpendNative.PreferredMode();
    }

    [Fact]
    public void RecursiveSpendNativeRejectsEmptyArchivesBeforeLoadingNativeBridge()
    {
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Init(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Append(Array.Empty<byte>()));
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
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Verify(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Redeem(malformed));
    }
}
