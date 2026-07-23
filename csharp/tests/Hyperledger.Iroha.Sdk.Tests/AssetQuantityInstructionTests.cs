using System.Buffers.Binary;
using System.Globalization;
using System.Numerics;
using System.Text.Json;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class AssetQuantityInstructionTests
{
    private const string ChainId = "00000042";
    private const string AccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private static FeePaymentIntent EmptyAuthorityFeePayment =>
        FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>());

    public static IEnumerable<object[]> InvalidQuantitySpellings()
    {
        var aboveMaximum = (NumericV1.IntMax + BigInteger.One).ToString(CultureInfo.InvariantCulture);
        var scaleTwentyNine = "0." + new string('0', NumericV1.MaxScale) + "1";

        foreach (var value in new[]
        {
            string.Empty,
            " ",
            "\t1",
            "1 ",
            "+1",
            "01",
            "1.",
            ".5",
            "1e0",
            "1E+3",
            "-1",
            "-0",
            "-0.0",
            "0.0",
            "1.0",
            "1.2300",
            scaleTwentyNine,
            aboveMaximum,
            new string('9', 155),
        })
        {
            yield return new object[] { value };
        }
    }

    [Theory]
    [MemberData(nameof(InvalidQuantitySpellings))]
    public void EveryAssetInstructionBoundaryRejectsInvalidQuantityText(string quantity)
    {
        var builder = new TransactionBuilder(ChainId, AccountId, EmptyAuthorityFeePayment);

        foreach (var construct in StringConstructionAttempts(builder, quantity))
        {
            var error = Assert.Throws<ArgumentException>(construct);
            Assert.Contains("quantity", error.Message, StringComparison.OrdinalIgnoreCase);
        }

        var transfer = TransactionInstruction.TransferAsset(AssetDefinitionId, "1", AccountId);
        var mint = TransactionInstruction.MintAsset(AssetDefinitionId, "1", AccountId);
        var burn = TransactionInstruction.BurnAsset(AssetDefinitionId, "1", AccountId);
        Assert.Throws<ArgumentException>(() => transfer with { Quantity = quantity });
        Assert.Throws<ArgumentException>(() => mint with { Quantity = quantity });
        Assert.Throws<ArgumentException>(() => burn with { Quantity = quantity });
        Assert.Empty(builder.Instructions);
    }

    [Theory]
    [InlineData("0")]
    [InlineData("1")]
    [InlineData("1.25")]
    [InlineData("0.0000000000000000000000000001")]
    public void AssetInstructionBoundariesAcceptCanonicalNonNegativeQuantities(string text)
    {
        var quantity = NumericV1.QuantityValue.ParseCanonical(text);
        var builder = new TransactionBuilder(ChainId, AccountId, EmptyAuthorityFeePayment)
            .TransferAsset(AssetDefinitionId, quantity, AccountId)
            .MintAsset(AssetDefinitionId, quantity, AccountId)
            .BurnAsset(AssetDefinitionId, quantity, AccountId);

        Assert.Collection(
            builder.Instructions,
            instruction => AssertQuantity(instruction, quantity),
            instruction => AssertQuantity(instruction, quantity),
            instruction => AssertQuantity(instruction, quantity));

        Assert.Equal(quantity, TransactionInstruction.TransferAsset(AssetDefinitionId, quantity, AccountId).QuantityValue);
        Assert.Equal(quantity, TransactionInstruction.MintAsset(AssetDefinitionId, quantity, AccountId).QuantityValue);
        Assert.Equal(quantity, TransactionInstruction.BurnAsset(AssetDefinitionId, quantity, AccountId).QuantityValue);
    }

    [Fact]
    public void AssetInstructionStringBoundaryAcceptsMaximumMantissa()
    {
        var maximum = NumericV1.IntMax.ToString(CultureInfo.InvariantCulture);
        var transfer = TransactionInstruction.TransferAsset(AssetDefinitionId, maximum, AccountId);

        Assert.Equal(NumericV1.IntMax, transfer.QuantityValue.Mantissa);
        Assert.Equal(0, transfer.QuantityValue.Scale);
        Assert.Equal(maximum, transfer.Quantity);
    }

    [Fact]
    public void QuantityWireEncodingUsesCanonicalMinimalMantissa()
    {
        var context = new TransactionEncodingContext(AccountId);

        AssertQuantityPayload(context.EncodeQuantity(NumericV1.QuantityValue.ParseCanonical("0")), 0, 0);
        AssertQuantityPayload(context.EncodeQuantity(NumericV1.QuantityValue.ParseCanonical("128")), 2, 0);
        AssertQuantityPayload(
            context.EncodeQuantity(NumericV1.QuantityValue.ParseCanonical("0.0000000000000000000000000001")),
            1,
            NumericV1.MaxScale);
        AssertQuantityPayload(
            context.EncodeQuantity(NumericV1.QuantityValue.FromMantissa(NumericV1.IntMax, 0)),
            64,
            0);
    }

    [Theory]
    [MemberData(nameof(InvalidQuantitySpellings))]
    public void ToriiAssetRwaAndUaidReadbacksRejectInvalidQuantityText(string quantity)
    {
        Assert.Throws<ArgumentException>(() => new ToriiAccountFaucetResponse { Amount = quantity });
        Assert.Throws<ArgumentException>(() => new ToriiAssetBalance { Quantity = quantity });
        Assert.Throws<ArgumentException>(() => new ToriiExplorerAsset { Value = quantity });
        Assert.Throws<ArgumentException>(() => new ToriiExplorerRwaParent { Quantity = quantity });
        Assert.Throws<ArgumentException>(() => new ToriiExplorerRwa { Quantity = quantity });
        Assert.Throws<ArgumentException>(() => new ToriiExplorerRwa { HeldQuantity = quantity });
        Assert.Throws<ArgumentException>(() => new ToriiUaidPortfolioAsset { Quantity = quantity });
    }

    [Theory]
    [MemberData(nameof(InvalidQuantitySpellings))]
    public void RawToriiAssetRwaAndUaidJsonRejectInvalidQuantityText(string quantity)
    {
        var encoded = JsonSerializer.Serialize(quantity);
        var assetBalance = "{\"asset\":\"asset\",\"account_id\":"
            + JsonSerializer.Serialize(AccountId)
            + ",\"scope\":\"global\",\"asset_name\":\"asset\",\"asset_alias\":null,\"quantity\":"
            + encoded
            + "}";
        var rwaParent = "{\"rwa\":\"gold-lot\",\"quantity\":" + encoded + "}";
        var uaidAsset = "{\"asset_id\":\"asset\",\"asset_definition_id\":\"definition\",\"quantity\":"
            + encoded
            + "}";

        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiAssetBalance>(assetBalance));
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiExplorerRwaParent>(rwaParent));
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiUaidPortfolioAsset>(uaidAsset));
    }

    private static IEnumerable<Action> StringConstructionAttempts(TransactionBuilder builder, string quantity)
    {
        yield return () => builder.TransferAsset(AssetDefinitionId, quantity, AccountId);
        yield return () => builder.MintAsset(AssetDefinitionId, quantity, AccountId);
        yield return () => builder.BurnAsset(AssetDefinitionId, quantity, AccountId);
        yield return () => TransactionInstruction.TransferAsset(AssetDefinitionId, quantity, AccountId);
        yield return () => TransactionInstruction.MintAsset(AssetDefinitionId, quantity, AccountId);
        yield return () => TransactionInstruction.BurnAsset(AssetDefinitionId, quantity, AccountId);
        yield return () => new TransferAssetInstruction(AssetDefinitionId, quantity, AccountId);
        yield return () => new MintAssetInstruction(AssetDefinitionId, quantity, AccountId);
        yield return () => new BurnAssetInstruction(AssetDefinitionId, quantity, AccountId);
    }

    private static void AssertQuantity(TransactionInstruction instruction, NumericV1.QuantityValue expected)
    {
        var actual = instruction switch
        {
            TransferAssetInstruction transfer => transfer.QuantityValue,
            MintAssetInstruction mint => mint.QuantityValue,
            BurnAssetInstruction burn => burn.QuantityValue,
            _ => throw new InvalidOperationException($"Unexpected instruction {instruction.GetType().Name}"),
        };
        Assert.Equal(expected, actual);
        Assert.Equal(expected.ToString(), actual.ToString());
    }

    private static void AssertQuantityPayload(byte[] payload, int expectedMantissaBytes, int expectedScale)
    {
        var mantissaFieldBytes = BinaryPrimitives.ReadUInt64LittleEndian(payload);
        Assert.Equal((ulong)(sizeof(uint) + expectedMantissaBytes), mantissaFieldBytes);
        Assert.Equal((uint)expectedMantissaBytes, BinaryPrimitives.ReadUInt32LittleEndian(payload.AsSpan(sizeof(ulong))));

        var scaleFieldOffset = sizeof(ulong) + checked((int)mantissaFieldBytes);
        Assert.Equal((ulong)sizeof(uint), BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(scaleFieldOffset)));
        Assert.Equal(
            (uint)expectedScale,
            BinaryPrimitives.ReadUInt32LittleEndian(payload.AsSpan(scaleFieldOffset + sizeof(ulong))));
        Assert.Equal(scaleFieldOffset + sizeof(ulong) + sizeof(uint), payload.Length);
    }
}
