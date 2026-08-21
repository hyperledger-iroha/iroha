using System.Buffers.Binary;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class TransactionTtlContractTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureNetworkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private const string FixtureAccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string FixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

    [Fact]
    public void TransactionBuilderAssignsRequiredSignatureBoundDefaultTtl()
    {
        var builder = NewBuilder()
            .TransferAsset(FixtureAssetDefinitionId, "1", FixtureAccountId)
            .SetCreationTimeMilliseconds(1_736_000_000_000);

        Assert.Equal(
            TransactionBuilder.DefaultTimeToLiveMilliseconds,
            builder.TimeToLiveMilliseconds);

        var unsigned = builder.BuildUnsignedPayload();
        Assert.Equal(
            TransactionBuilder.DefaultTimeToLiveMilliseconds,
            unsigned.TimeToLiveMilliseconds);
        Assert.Equal(TransactionAdmissionIntent.QueuePlanSynced, unsigned.AdmissionIntent);
        using var document = JsonDocument.Parse(JsonSerializer.Serialize(unsigned));
        Assert.Equal(
            TransactionBuilder.DefaultTimeToLiveMilliseconds,
            document.RootElement.GetProperty("time_to_live_ms").GetUInt64());
        var admissionIntent = document.RootElement.GetProperty("admission_intent");
        Assert.Equal("queue_plan_synced", admissionIntent.GetProperty("intent").GetString());
        Assert.Equal(JsonValueKind.Null, admissionIntent.GetProperty("value").ValueKind);

        var signed = builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));
        Assert.Equal(
            TransactionBuilder.DefaultTimeToLiveMilliseconds,
            ReadTimeToLiveMilliseconds(signed.PayloadBytes));
    }

    [Fact]
    public void TransactionBuilderRejectsZeroTtl()
    {
        var error = Assert.Throws<ArgumentOutOfRangeException>(
            () => NewBuilder().SetTimeToLiveMilliseconds(0));

        Assert.Equal("timeToLiveMilliseconds", error.ParamName);
        Assert.StartsWith("Transaction TTL must be positive", error.Message);
    }

    [Fact]
    public void CanonicalTransactionFixtureDescriptorsRequireExplicitPositiveTtlAndMatchPayloads()
    {
        var fixtureDirectory = Path.Combine(AppContext.BaseDirectory, "Fixtures", "norito_rpc");
        using var manifest = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(fixtureDirectory, "transaction_fixtures.manifest.json")));
        using var payloads = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(fixtureDirectory, "transaction_payloads.json")));
        var payloadsByName = payloads.RootElement
            .EnumerateArray()
            .ToDictionary(
                static entry => entry.GetProperty("name").GetString()!,
                static entry => entry,
                StringComparer.Ordinal);

        foreach (var descriptor in manifest.RootElement.GetProperty("fixtures").EnumerateArray())
        {
            var name = descriptor.GetProperty("name").GetString()!;
            var descriptorNetworkId = RequireCanonicalNetworkId(descriptor, $"{name}.descriptor");
            var descriptorTtl = RequirePositiveTimeToLive(descriptor, $"{name}.descriptor");
            Assert.True(payloadsByName.TryGetValue(name, out var payload), $"{name}: missing payload fixture");
            Assert.Equal(
                descriptorNetworkId,
                RequireCanonicalNetworkId(payload, $"{name}.payload entry"));
            Assert.Equal(
                descriptorNetworkId,
                RequireCanonicalNetworkId(payload.GetProperty("payload"), $"{name}.payload"));
            Assert.Equal(descriptorTtl, RequirePositiveTimeToLive(payload, $"{name}.payload entry"));
            Assert.Equal(
                descriptorTtl,
                RequirePositiveTimeToLive(payload.GetProperty("payload"), $"{name}.payload"));
            var payloadBytes = Convert.FromBase64String(
                descriptor.GetProperty("payload_base64").GetString()!);
            Assert.Equal(
                NetworkId.Parse(descriptorNetworkId).ToBytes(),
                ReadNetworkId(payloadBytes));
        }
    }

    [Theory]
    [InlineData("{}")]
    [InlineData("{\"time_to_live_ms\":null}")]
    [InlineData("{\"time_to_live_ms\":0}")]
    public void CanonicalTransactionFixtureDescriptorRejectsMissingNullOrZeroTtl(string json)
    {
        using var document = JsonDocument.Parse(json);
        Assert.Throws<InvalidDataException>(
            () => RequirePositiveTimeToLive(document.RootElement, "fixture"));
    }

    [Theory]
    [InlineData("{\"chain\":\"00000042\"}")]
    [InlineData("{\"network_id\":\"00000042\"}")]
    [InlineData("{\"network_id\":\"hash:32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149#a2f0\"}")]
    [InlineData("{\"network_id\":\"hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#0000\"}")]
    [InlineData("{\"network_id\":\"hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91148#B2D1\"}")]
    public void CanonicalTransactionFixtureDescriptorRejectsLegacyOrMalformedNetworkIdentity(string json)
    {
        using var document = JsonDocument.Parse(json);
        Assert.Throws<InvalidDataException>(
            () => RequireCanonicalNetworkId(document.RootElement, "fixture"));
    }

    private static TransactionBuilder NewBuilder() =>
        new(
            NetworkId.Parse(FixtureNetworkId),
            FixtureAccountId,
            FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()));

    private static string RequireCanonicalNetworkId(JsonElement value, string context)
    {
        if (value.TryGetProperty("chain", out _)
            || !value.TryGetProperty("network_id", out var encoded)
            || encoded.ValueKind != JsonValueKind.String
            || encoded.GetString() is not { } networkId)
        {
            throw new InvalidDataException(
                $"{context}.network_id must be an explicit canonical NetworkId");
        }

        try
        {
            _ = NetworkId.Parse(networkId);
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                $"{context}.network_id must be an explicit canonical NetworkId",
                error);
        }

        return networkId;
    }

    private static ulong RequirePositiveTimeToLive(JsonElement value, string context)
    {
        if (!value.TryGetProperty("time_to_live_ms", out var encoded)
            || encoded.ValueKind != JsonValueKind.Number
            || !encoded.TryGetUInt64(out var timeToLiveMilliseconds)
            || timeToLiveMilliseconds == 0)
        {
            throw new InvalidDataException(
                $"{context}.time_to_live_ms must be an explicit positive integer");
        }

        return timeToLiveMilliseconds;
    }

    private static ulong ReadTimeToLiveMilliseconds(ReadOnlySpan<byte> payload)
    {
        var networkDomain = ReadField(payload, out var networkDomainLength);
        AssertNetworkDomain(networkDomain);
        _ = ReadField(payload[networkDomainLength..], out var authorityLength);
        _ = ReadField(payload[(networkDomainLength + authorityLength)..], out var creationTimeLength);
        _ = ReadField(
            payload[(networkDomainLength + authorityLength + creationTimeLength)..],
            out var executableLength);
        var option = ReadField(
            payload[(networkDomainLength + authorityLength + creationTimeLength + executableLength)..],
            out _);

        Assert.Equal(1, option[0]);
        var encoded = ReadField(option.AsSpan(1), out var consumed);
        Assert.Equal(option.Length - 1, consumed);
        Assert.Equal(sizeof(ulong), encoded.Length);
        return BinaryPrimitives.ReadUInt64LittleEndian(encoded);
    }

    private static void AssertNetworkDomain(ReadOnlySpan<byte> encoded)
    {
        Assert.Equal(
            NetworkId.Parse(FixtureNetworkId).ToBytes(),
            ReadNetworkIdFromDomain(encoded));
    }

    private static byte[] ReadNetworkId(ReadOnlySpan<byte> payload)
    {
        var domain = ReadField(payload, out _);
        return ReadNetworkIdFromDomain(domain);
    }

    private static byte[] ReadNetworkIdFromDomain(ReadOnlySpan<byte> encoded)
    {
        Assert.True(encoded.Length >= sizeof(uint), "transaction domain must include its enum tag");
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(encoded[..sizeof(uint)]));
        var networkId = ReadField(encoded[sizeof(uint)..], out var consumed);
        Assert.Equal(encoded.Length - sizeof(uint), consumed);
        Assert.Equal(NetworkId.ByteLength, networkId.Length);
        return networkId;
    }

    private static byte[] ReadField(ReadOnlySpan<byte> bytes, out int consumed)
    {
        var reader = new CanonicalNoritoReader(
            bytes,
            "transaction TTL test payload",
            nameof(bytes));
        var field = reader.ReadField("field").ToArray();
        consumed = bytes.Length - reader.Remaining;
        return field;
    }
}
