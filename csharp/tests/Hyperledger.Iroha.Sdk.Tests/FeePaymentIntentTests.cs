using System.Text.Json;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class FeePaymentIntentTests
{
    private const string NetworkIdLiteral = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private static NetworkId FixtureNetworkId => NetworkId.Parse(NetworkIdLiteral);
    private const string AccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

    [Fact]
    public void SponsorIntentRoundTripsThroughStrictNoritoJson()
    {
        var intent = FeePaymentIntent.Sponsor(
            new FeeSponsorProgramId(AccountId, "wallet_fx"),
            7,
            [
                new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionId, "2.5"),
                new FeeChargeLimit(FeeChargeKind.PipelineGas, AssetDefinitionId, "8"),
            ],
            500_000);

        var json = JsonSerializer.Serialize(intent);
        var decoded = JsonSerializer.Deserialize<FeePaymentIntent>(json);

        Assert.Equal(intent, decoded);
        Assert.Contains("\"payer\":\"sponsor\"", json, StringComparison.Ordinal);
        Assert.Contains("\"program_revision\":7", json, StringComparison.Ordinal);
        Assert.Contains("\"gas_limit\":500000", json, StringComparison.Ordinal);
    }

    [Fact]
    public void IntentRejectsDuplicateOrOutOfOrderComponents()
    {
        var nexus = new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionId, "1");
        var gas = new FeeChargeLimit(FeeChargeKind.PipelineGas, AssetDefinitionId, "2");

        Assert.Throws<ArgumentException>(() => FeePaymentIntent.Authority([nexus, nexus]));
        Assert.Throws<ArgumentException>(() => FeePaymentIntent.Authority([gas, nexus]));
        Assert.Throws<ArgumentOutOfRangeException>(() => FeePaymentIntent.Authority([], 0));
    }

    [Fact]
    public void QuoteCanOnlyReplaceComponentMaxima()
    {
        var programId = new FeeSponsorProgramId(AccountId, "wallet_fx");
        var requested = FeePaymentIntent.Sponsor(
            programId,
            7,
            [new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionId, "10")],
            500_000);
        var quoted = FeePaymentIntent.Sponsor(
            programId,
            7,
            [new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionId, "3")],
            500_000);
        var builder = new TransactionBuilder(FixtureNetworkId, AccountId, requested)
            .TransferAsset(AssetDefinitionId, "1", AccountId);

        Assert.Same(builder, builder.ApplyFeeQuote(quoted));
        Assert.Same(quoted, builder.FeePayment);
        Assert.Throws<InvalidOperationException>(() => builder.ApplyFeeQuote(
            FeePaymentIntent.Sponsor(programId, 8, quoted.ChargeLimits, 500_000)));
        Assert.Throws<InvalidOperationException>(() => builder.ApplyFeeQuote(
            FeePaymentIntent.Authority(quoted.ChargeLimits, 500_000)));
    }

    [Fact]
    public void UnsignedQuotePayloadCarriesFeeIntentAndRejectsRetiredMetadata()
    {
        var intent = FeePaymentIntent.Authority(
            [new FeeChargeLimit(FeeChargeKind.Nexus, AssetDefinitionId, "4")]);
        var builder = new TransactionBuilder(FixtureNetworkId, AccountId, intent)
            .TransferAsset(AssetDefinitionId, "1", AccountId)
            .SetCreationTimeMilliseconds(1_736_000_000_000);

        var payload = builder.BuildUnsignedPayload();
        using var payloadJson = JsonDocument.Parse(JsonSerializer.Serialize(payload));
        var payloadRoot = payloadJson.RootElement;
        var domain = payloadRoot.GetProperty("domain");

        Assert.Same(intent, payload.FeePayment);
        Assert.Equal(1_736_000_000_000UL, payload.CreationTimeMilliseconds);
        Assert.True(payload.Executable.ContainsKey("Instructions"));
        Assert.Equal("network", domain.GetProperty("kind").GetString());
        Assert.Equal(NetworkIdLiteral, domain.GetProperty("value").GetString());
        Assert.False(payloadRoot.TryGetProperty("network_id", out _));
        Assert.False(payloadRoot.TryGetProperty("chain", out _));
        Assert.Throws<ArgumentException>(() => builder.SetMetadata("fee_sponsor", null));
        Assert.Throws<ArgumentException>(() => builder.SetMetadata("gas_limit", null));
        Assert.Throws<ArgumentException>(() => builder.SetMetadata("gas_asset_id", null));
    }
}
