using System.Net;
using System.Buffers.Binary;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Offline;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class TransactionBuilderTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureChainId = "00000042";
    private const string FixtureAccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";

    [Fact]
    public void TransactionBuilderRejectsPaddedTopLevelFields()
    {
        Assert.Throws<ArgumentException>(() => new TransactionBuilder(" 00000042", FixtureAccountId));
        Assert.Throws<ArgumentException>(() => new TransactionBuilder("00000042", $" {FixtureAccountId}"));

        var builder = new TransactionBuilder("00000042", FixtureAccountId);
        Assert.Throws<ArgumentException>(() => builder.SetMetadata(" trace ", JsonValue.Create("abc")));
        Assert.Throws<ArgumentException>(
            () => builder.ReplaceMetadata(
                new Dictionary<string, JsonNode?>
                {
                    [" trace "] = JsonValue.Create("abc"),
                }));
    }

    [Fact]
    public void TransactionEncodingContextRejectsPaddedBoundaryFields()
    {
        Assert.Throws<ArgumentException>(() => new TransactionEncodingContext($" {FixtureAccountId}"));

        var context = new TransactionEncodingContext(FixtureAccountId);
        Assert.Throws<ArgumentException>(() => context.EncodeChainId(" 00000042"));
        Assert.Throws<ArgumentException>(() => context.EncodeAccountId($" {FixtureAccountId}"));
        Assert.Throws<ArgumentException>(() => context.EncodeName(" display_name"));
        Assert.Throws<ArgumentException>(() => context.EncodeOptionalString(" memo "));
        Assert.Throws<ArgumentException>(() => context.EncodeNumeric(" 15.7500"));
        Assert.Throws<ArgumentException>(() => context.EncodeAssetDefinitionId(" 62Fk4FPcMuLvW5QjDGNF2a4jAmjM"));
        Assert.Throws<ArgumentException>(() => context.EncodeNftId(" dragon$wonderland"));
        Assert.Throws<ArgumentException>(() => context.EncodeHashLiteral(" " + new string('a', 64)));
        Assert.Throws<ArgumentException>(() => context.EncodeFixedBytesLiteral(" 0x0102", expectedLength: 2));
    }
    private const string FixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

    [Theory]
    [InlineData("swift_transfer_asset_basic", 761, 1379, "670e42748e47402ec28a9090befd3bcbe0a2e82ef362405a0c237daac3111d65")]
    [InlineData("swift_mint_asset_basic", 651, 1269, "83de89f2a66af6f3c33658c26aa66d1c88f6903c0e278f11b4bd0b7f4db3d287")]
    [InlineData("swift_burn_asset_basic", 651, 1269, "fe23070b7c9a3ca13a624d4d9ff7f90c046626e62a07dea68c5137d2104faba7")]
    public void BuildSignedProducesDeterministicGoldenOutputs(
        string fixtureName,
        int expectedPayloadLength,
        int expectedSignedLength,
        string expectedHashHex)
    {
        using var payloadsDocument = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "swift_parity_payloads.json")));

        var payload = payloadsDocument.RootElement.EnumerateArray()
            .First(candidate => candidate.GetProperty("name").GetString() == fixtureName)
            .GetProperty("payload");

        var builder = new TransactionBuilder(
            payload.GetProperty("chain").GetString()!,
            payload.GetProperty("authority").GetString()!)
            .SetCreationTimeMilliseconds((ulong)payload.GetProperty("creation_time_ms").GetInt64())
            .SetTimeToLiveMilliseconds((ulong)payload.GetProperty("time_to_live_ms").GetInt64())
            .SetNonce((uint)payload.GetProperty("nonce").GetInt32());

        var instruction = payload.GetProperty("executable").GetProperty("Instructions")[0];
        var arguments = instruction.GetProperty("arguments");
        var action = arguments.GetProperty("action").GetString();
        var assetDefinitionId = arguments.GetProperty("asset_definition_id").GetString()!;
        var quantity = arguments.GetProperty("quantity").GetString()!;
        var destination = arguments.GetProperty("destination").GetString()!;

        _ = action switch
        {
            "TransferAsset" => builder.TransferAsset(assetDefinitionId, quantity, destination),
            "MintAsset" => builder.MintAsset(assetDefinitionId, quantity, destination),
            "BurnAsset" => builder.BurnAsset(assetDefinitionId, quantity, destination),
            _ => throw new InvalidOperationException($"Unsupported fixture action `{action}`."),
        };

        var envelope = builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));
        Assert.Equal(expectedPayloadLength, envelope.PayloadBytes.Length);
        Assert.Equal(expectedSignedLength, envelope.SignedTransactionBytes.Length);
        Assert.Equal(expectedHashHex, envelope.TransactionHashHex);

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    [Theory]
    [InlineData("", FixtureAccountId)]
    [InlineData(" 00000042", FixtureAccountId)]
    [InlineData("00000042 ", FixtureAccountId)]
    [InlineData("0000\u000042", FixtureAccountId)]
    [InlineData(FixtureChainId, "")]
    [InlineData(FixtureChainId, " ")]
    [InlineData(FixtureChainId, " sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData(FixtureChainId, "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData(FixtureChainId, "sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void ConstructorRejectsNonExactRequiredFields(string chainId, string authorityAccountId)
    {
        Assert.Throws<ArgumentException>(() => new TransactionBuilder(chainId, authorityAccountId));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" memo")]
    [InlineData("memo ")]
    [InlineData("\u00A0memo")]
    [InlineData("memo\u00A0")]
    [InlineData("me\u0000mo")]
    public void SetMetadataRejectsNonExactKeys(string key)
    {
        var builder = NewTransactionBuilder();

        Assert.Throws<ArgumentException>(() =>
        {
            builder.SetMetadata(key, JsonValue.Create("value"));
        });
        Assert.Empty(builder.Metadata);
    }

    [Theory]
    [InlineData("")]
    [InlineData(" memo")]
    [InlineData("memo ")]
    [InlineData("me\u001Fmo")]
    public void ReplaceMetadataRejectsNonExactKeysWithoutMutatingExistingMetadata(string key)
    {
        var builder = NewTransactionBuilder().SetMetadata("existing", JsonValue.Create("keep"));
        var replacement = new Dictionary<string, JsonNode?>
        {
            ["next"] = JsonValue.Create("value"),
            [key] = JsonValue.Create("bad"),
        };

        Assert.Throws<ArgumentException>(() =>
        {
            builder.ReplaceMetadata(replacement);
        });
        var existing = Assert.Single(builder.Metadata);
        Assert.Equal("existing", existing.Key);
        Assert.Equal("\"keep\"", existing.Value!.ToJsonString());
    }

    [Theory]
    [InlineData("")]
    [InlineData(" 00000042")]
    [InlineData("00000042 ")]
    [InlineData("0000\u000042")]
    public void TransactionEncodingContextRejectsNonExactChainIds(string chainId)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeChainId(chainId));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" alice")]
    [InlineData("alice ")]
    [InlineData("\u00A0alice")]
    [InlineData("ali\u0000ce")]
    public void TransactionEncodingContextRejectsNonExactNames(string name)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeName(name));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" 1")]
    [InlineData("1 ")]
    [InlineData("\u00A01")]
    [InlineData("1\u0000")]
    public void TransactionEncodingContextRejectsNonExactNumerics(string numeric)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeNumeric(numeric));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" dragon$wonderland")]
    [InlineData("dragon$wonderland ")]
    [InlineData("dragon$ wonderland")]
    [InlineData("dra\u0000gon$wonderland")]
    public void TransactionEncodingContextRejectsNonExactNftIds(string nftId)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeNftId(nftId));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" 62Fk4FPcMuLvW5QjDGNF2a4jAmjM")]
    [InlineData("62Fk4FPcMuLvW5QjDGNF2a4jAmjM ")]
    [InlineData("62Fk4FPcMuLvW5QjDGNF2a4jAmjM\u0000")]
    public void TransactionEncodingContextRejectsNonExactAssetDefinitionIds(string assetDefinitionId)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeAssetDefinitionId(assetDefinitionId));
    }

    [Fact]
    public void TransactionEncodingContextEncodesNullOptionalStringAsNone()
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Equal(new byte[] { 0 }, context.EncodeOptionalString(null));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" sort_key")]
    [InlineData("sort_key ")]
    [InlineData("sort\u0000key")]
    public void TransactionEncodingContextRejectsNonExactOptionalStrings(string value)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeOptionalString(value));
    }

    [Theory]
    [InlineData(" sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void TransactionEncodingContextRejectsNonExactAccountIds(string accountId)
    {
        Assert.Throws<ArgumentException>(() => new TransactionEncodingContext(accountId));

        var context = new TransactionEncodingContext(FixtureAccountId);
        Assert.Throws<ArgumentException>(() => context.EncodeAccountId(accountId));
    }

    [Theory]
    [MemberData(nameof(NonExactInstructionFieldCases))]
    public void BuildSignedRejectsNonExactInstructionFields(
        string label,
        Action<TransactionBuilder> configure)
    {
        var builder = NewTransactionBuilder()
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17);
        configure(builder);

        var exception = Assert.ThrowsAny<ArgumentException>(() =>
        {
            builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));
        });
        Assert.NotEmpty(label);
        Assert.NotEmpty(exception.Message);
    }

    [Fact]
    public void EncodeInstructionBoxRejectsNonExactAuthority()
    {
        var instruction = TransactionInstruction.TransferDomain("wonderland", FixtureAccountId);

        Assert.Throws<ArgumentException>(() => instruction.EncodeInstructionBox(" " + FixtureAccountId));
        Assert.Throws<ArgumentException>(() => instruction.EncodeInstructionBox(FixtureAccountId + " "));
        Assert.Throws<ArgumentException>(() => instruction.EncodeInstructionBox("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"));
    }

    [Fact]
    public async Task LedgerClientSubmitAndWaitPollsUntilTerminalState()
    {
        var transaction = new TransactionBuilder("00000042", "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "15.7500", "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var statusPollCount = 0;
        using var handler = new RecordingHandler(request =>
        {
            if (request.RequestUri!.AbsolutePath == "/transaction")
            {
                return new HttpResponseMessage(HttpStatusCode.Accepted)
                {
                    Content = new ByteArrayContent(Array.Empty<byte>()),
                };
            }

            statusPollCount++;
            var body = statusPollCount switch
            {
                1 => """
                    {
                      "kind": "Transaction",
                      "content": {
                        "hash": "da01f3a369d10e6ad78f241c86f4fe2d5481ff13ace97e6fb5db5c30240bdb3b",
                        "status": { "kind": "Queued", "content": null },
                        "scope": "auto",
                        "resolved_from": "queue"
                      }
                    }
                    """,
                _ => """
                    {
                      "kind": "Transaction",
                      "content": {
                        "hash": "da01f3a369d10e6ad78f241c86f4fe2d5481ff13ace97e6fb5db5c30240bdb3b",
                        "status": { "kind": "Applied", "block_height": 11, "content": null },
                        "scope": "auto",
                        "resolved_from": "state"
                      }
                    }
                    """,
            };

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(body),
            };
        });

        using var client = new IrohaClient(new Uri("https://torii.example"), new HttpClient(handler));
        var status = await client.Ledger.SubmitAndWaitAsync(
            transaction,
            new PipelineSubmitOptions
            {
                PollInterval = TimeSpan.FromMilliseconds(1),
                Timeout = TimeSpan.FromSeconds(1),
            });

        Assert.Equal(PipelineTransactionState.Applied, status.State);
        Assert.Equal((ulong)11, status.BlockHeight);
        Assert.True(status.IsTerminal);
    }

    [Fact]
    public void BuildSignedEncodesAssetMetadataInstructions()
    {
        var envelope = new TransactionBuilder(
                "00000042",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .SetAssetKeyValue(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                "display_name",
                JsonValue.Create("Treasury buffer"))
            .RemoveAssetKeyValue(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                "legacy_flag")
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var instructions = ReadEncodedInstructions(envelope.PayloadBytes);
        Assert.Equal(2, instructions.Count);

        Assert.Equal("iroha_data_model::isi::transparent::SetAssetKeyValue", instructions[0].WireId);
        var setPayload = SkipNoritoHeader(instructions[0].Payload);
        _ = ReadField(setPayload, out var setOffsetAfterAsset);
        var setKey = ReadNoritoString(ReadField(setPayload[setOffsetAfterAsset..], out var setOffsetAfterKey));
        var setValue = ReadNoritoString(ReadField(setPayload[(setOffsetAfterAsset + setOffsetAfterKey)..], out _));
        Assert.Equal("display_name", setKey);
        Assert.Equal("\"Treasury buffer\"", setValue);

        Assert.Equal("iroha_data_model::isi::transparent::RemoveAssetKeyValue", instructions[1].WireId);
        var removePayload = SkipNoritoHeader(instructions[1].Payload);
        _ = ReadField(removePayload, out var removeOffsetAfterAsset);
        var removeKey = ReadNoritoString(ReadField(removePayload[removeOffsetAfterAsset..], out _));
        Assert.Equal("legacy_flag", removeKey);

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    [Fact]
    public void AddInstructionAcceptsAccountAndAssetDefinitionMetadataFactories()
    {
        var builder = new TransactionBuilder(
            "00000042",
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .AddInstruction(TransactionInstruction.SetDomainKeyValue(
                "wonderland",
                "display_name",
                JsonValue.Create("Treasury buffer")))
            .AddInstruction(TransactionInstruction.RemoveDomainKeyValue(
                "wonderland",
                "legacy_flag"))
            .AddInstruction(TransactionInstruction.SetAccountKeyValue(
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                "display_name",
                JsonValue.Create("Treasury buffer")))
            .AddInstruction(TransactionInstruction.RemoveAccountKeyValue(
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                "legacy_flag"))
            .AddInstruction(TransactionInstruction.SetAssetDefinitionKeyValue(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "ticker",
                JsonValue.Create("XOR")))
            .AddInstruction(TransactionInstruction.RemoveAssetDefinitionKeyValue(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "deprecated_label"));

        Assert.Collection(
            builder.Instructions,
            instruction => Assert.IsType<SetDomainKeyValueInstruction>(instruction),
            instruction => Assert.IsType<RemoveDomainKeyValueInstruction>(instruction),
            instruction => Assert.IsType<SetAccountKeyValueInstruction>(instruction),
            instruction => Assert.IsType<RemoveAccountKeyValueInstruction>(instruction),
            instruction => Assert.IsType<SetAssetDefinitionKeyValueInstruction>(instruction),
            instruction => Assert.IsType<RemoveAssetDefinitionKeyValueInstruction>(instruction));
    }

    [Fact]
    public void BuildSignedEncodesAccountAndAssetDefinitionMetadataInstructions()
    {
        var envelope = new TransactionBuilder(
                "00000042",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .SetDomainKeyValue(
                "wonderland",
                "display_name",
                JsonValue.Create("Treasury buffer"))
            .RemoveDomainKeyValue(
                "wonderland",
                "legacy_flag")
            .SetAccountKeyValue(
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                "display_name",
                JsonValue.Create("Treasury buffer"))
            .RemoveAccountKeyValue(
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                "legacy_flag")
            .SetAssetDefinitionKeyValue(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "ticker",
                JsonValue.Create("XOR"))
            .RemoveAssetDefinitionKeyValue(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "deprecated_label")
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var instructions = ReadEncodedInstructions(envelope.PayloadBytes);
        Assert.Equal(6, instructions.Count);

        Assert.Equal("iroha.set_key_value", instructions[0].WireId);
        var setDomainPayload = SkipNoritoHeader(instructions[0].Payload);
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(setDomainPayload[..4]));
        _ = ReadField(setDomainPayload[4..], out var setDomainOffsetAfterObject);
        var setDomainKey = ReadNoritoString(ReadField(setDomainPayload[(4 + setDomainOffsetAfterObject)..], out var setDomainOffsetAfterKey));
        var setDomainValue = ReadNoritoString(ReadField(setDomainPayload[(4 + setDomainOffsetAfterObject + setDomainOffsetAfterKey)..], out _));
        Assert.Equal("display_name", setDomainKey);
        Assert.Equal("\"Treasury buffer\"", setDomainValue);

        Assert.Equal("iroha.remove_key_value", instructions[1].WireId);
        var removeDomainPayload = SkipNoritoHeader(instructions[1].Payload);
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(removeDomainPayload[..4]));
        _ = ReadField(removeDomainPayload[4..], out var removeDomainOffsetAfterObject);
        var removeDomainKey = ReadNoritoString(ReadField(removeDomainPayload[(4 + removeDomainOffsetAfterObject)..], out _));
        Assert.Equal("legacy_flag", removeDomainKey);

        Assert.Equal("iroha.set_key_value", instructions[2].WireId);
        var setAccountPayload = SkipNoritoHeader(instructions[2].Payload);
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(setAccountPayload[..4]));
        _ = ReadField(setAccountPayload[4..], out var setAccountOffsetAfterObject);
        var setAccountKey = ReadNoritoString(ReadField(setAccountPayload[(4 + setAccountOffsetAfterObject)..], out var setAccountOffsetAfterKey));
        var setAccountValue = ReadNoritoString(ReadField(setAccountPayload[(4 + setAccountOffsetAfterObject + setAccountOffsetAfterKey)..], out _));
        Assert.Equal("display_name", setAccountKey);
        Assert.Equal("\"Treasury buffer\"", setAccountValue);

        Assert.Equal("iroha.remove_key_value", instructions[3].WireId);
        var removeAccountPayload = SkipNoritoHeader(instructions[3].Payload);
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(removeAccountPayload[..4]));
        _ = ReadField(removeAccountPayload[4..], out var removeAccountOffsetAfterObject);
        var removeAccountKey = ReadNoritoString(ReadField(removeAccountPayload[(4 + removeAccountOffsetAfterObject)..], out _));
        Assert.Equal("legacy_flag", removeAccountKey);

        Assert.Equal("iroha.set_key_value", instructions[4].WireId);
        var setAssetDefinitionPayload = SkipNoritoHeader(instructions[4].Payload);
        Assert.Equal(2u, BinaryPrimitives.ReadUInt32LittleEndian(setAssetDefinitionPayload[..4]));
        _ = ReadField(setAssetDefinitionPayload[4..], out var setAssetDefinitionOffsetAfterObject);
        var setAssetDefinitionKey = ReadNoritoString(ReadField(setAssetDefinitionPayload[(4 + setAssetDefinitionOffsetAfterObject)..], out var setAssetDefinitionOffsetAfterKey));
        var setAssetDefinitionValue = ReadNoritoString(ReadField(setAssetDefinitionPayload[(4 + setAssetDefinitionOffsetAfterObject + setAssetDefinitionOffsetAfterKey)..], out _));
        Assert.Equal("ticker", setAssetDefinitionKey);
        Assert.Equal("\"XOR\"", setAssetDefinitionValue);

        Assert.Equal("iroha.remove_key_value", instructions[5].WireId);
        var removeAssetDefinitionPayload = SkipNoritoHeader(instructions[5].Payload);
        Assert.Equal(2u, BinaryPrimitives.ReadUInt32LittleEndian(removeAssetDefinitionPayload[..4]));
        _ = ReadField(removeAssetDefinitionPayload[4..], out var removeAssetDefinitionOffsetAfterObject);
        var removeAssetDefinitionKey = ReadNoritoString(ReadField(removeAssetDefinitionPayload[(4 + removeAssetDefinitionOffsetAfterObject)..], out _));
        Assert.Equal("deprecated_label", removeAssetDefinitionKey);

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    [Fact]
    public void AddInstructionAcceptsNftAndTriggerFactories()
    {
        var builder = new TransactionBuilder(
            "00000042",
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .AddInstruction(TransactionInstruction.SetNftKeyValue(
                "dragon$wonderland",
                "rarity",
                JsonValue.Create("legendary")))
            .AddInstruction(TransactionInstruction.RemoveNftKeyValue(
                "dragon$wonderland",
                "legacy_flag"))
            .AddInstruction(TransactionInstruction.SetTriggerKeyValue(
                "settlement_window",
                "mode",
                JsonValue.Create("strict")))
            .AddInstruction(TransactionInstruction.RemoveTriggerKeyValue(
                "settlement_window",
                "legacy_flag"))
            .AddInstruction(TransactionInstruction.MintTriggerRepetitions(3, "settlement_window"))
            .AddInstruction(TransactionInstruction.BurnTriggerRepetitions(1, "settlement_window"))
            .AddInstruction(TransactionInstruction.ExecuteTrigger(
                "settlement_window",
                JsonNode.Parse("""{ "force": true }""")));

        Assert.Collection(
            builder.Instructions,
            instruction => Assert.IsType<SetNftKeyValueInstruction>(instruction),
            instruction => Assert.IsType<RemoveNftKeyValueInstruction>(instruction),
            instruction => Assert.IsType<SetTriggerKeyValueInstruction>(instruction),
            instruction => Assert.IsType<RemoveTriggerKeyValueInstruction>(instruction),
            instruction => Assert.IsType<MintTriggerRepetitionsInstruction>(instruction),
            instruction => Assert.IsType<BurnTriggerRepetitionsInstruction>(instruction),
            instruction => Assert.IsType<ExecuteTriggerInstruction>(instruction));
    }

    [Fact]
    public void AddInstructionAcceptsKagemushaInstructionArchiveFactories()
    {
        var redeemArchive = KagemushaArchive(
            KagemushaInstructionType.RedeemRecursive,
            new byte[] { 1, 2, 3 });
        var transferArchive = KagemushaArchive(
            KagemushaInstructionType.Transfer,
            new byte[] { 4, 5, 6 });

        var builder = new TransactionBuilder(
                "00000042",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .AddInstruction(TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.Transfer,
                transferArchive))
            .KagemushaRecursiveRedeem(new KagemushaRecursiveSpendRedeemInstructionArchive(redeemArchive));

        Assert.Collection(
            builder.Instructions,
            instruction =>
            {
                var kagemusha = Assert.IsType<KagemushaInstructionArchiveInstruction>(instruction);
                Assert.Equal(KagemushaInstructionType.Transfer, kagemusha.InstructionType);
            },
            instruction =>
            {
                var kagemusha = Assert.IsType<KagemushaInstructionArchiveInstruction>(instruction);
                Assert.Equal(KagemushaInstructionType.RedeemRecursive, kagemusha.InstructionType);
            });
    }

    [Fact]
    public void KagemushaRecursiveRedeemMetadataOverloadRejectsInvalidChangeOutputBeforeNativeBridge()
    {
        var builder = NewTransactionBuilder();
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };

        var missingChangeOutput = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                publicAmount: "40",
                currentNoteAmount: "100",
                hasChangeOutput: false));
        Assert.Equal("hasChangeOutput", missingChangeOutput.ParamName);
        Assert.Contains(
            "changeOutput is required when publicAmount is less than current note amount",
            missingChangeOutput.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);

        var unexpectedChangeOutput = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                publicAmount: "100",
                currentNoteAmount: "100",
                hasChangeOutput: true));
        Assert.Equal("publicAmount", unexpectedChangeOutput.ParamName);
        Assert.Contains(
            "publicAmount must be less than current note amount when changeOutput is present",
            unexpectedChangeOutput.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);

        var overAmount = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                publicAmount: "101",
                currentNoteAmount: "100",
                hasChangeOutput: false));
        Assert.Equal("publicAmount", overAmount.ParamName);
        Assert.Contains(
            "publicAmount must not exceed current note amount",
            overAmount.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);
    }

    [Theory]
    [InlineData(" 40", "100", "publicAmount")]
    [InlineData("40 ", "100", "publicAmount")]
    [InlineData("+40", "100", "publicAmount")]
    [InlineData("-40", "100", "publicAmount")]
    [InlineData("40.0", "100", "publicAmount")]
    [InlineData("40", " 100", "currentNoteAmount")]
    [InlineData("40", "100 ", "currentNoteAmount")]
    [InlineData("40", "+100", "currentNoteAmount")]
    [InlineData("40", "-100", "currentNoteAmount")]
    [InlineData("40", "100.0", "currentNoteAmount")]
    public void KagemushaRecursiveRedeemMetadataOverloadRejectsMalformedAmountsBeforeNativeBridge(
        string publicAmount,
        string currentNoteAmount,
        string expectedParamName)
    {
        var builder = NewTransactionBuilder();
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };

        var invalid = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                publicAmount,
                currentNoteAmount,
                hasChangeOutput: true));

        Assert.Equal(expectedParamName, invalid.ParamName);
        Assert.Contains(
            $"{expectedParamName} must be a decimal integer",
            invalid.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);
    }

    [Fact]
    public void KagemushaRecursiveRedeemMetadataOverloadRejectsInvalidLineageBeforeNativeBridge()
    {
        var builder = NewTransactionBuilder();
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };

        var missingWitness = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                hopCount: 1u,
                hasLineageWitness: false,
                hasLineageVerifierRecord: false));
        Assert.Equal("hasLineageWitness", missingWitness.ParamName);
        Assert.Contains(
            "lineageWitness is required for this bundle",
            missingWitness.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);

        var missingVerifierRecord = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                hopCount: 1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false));
        Assert.Equal("hasLineageVerifierRecord", missingVerifierRecord.ParamName);
        Assert.Contains(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            missingVerifierRecord.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);
    }

    [Fact]
    public void KagemushaRecursiveRedeemMetadataOverloadRejectsLineageAndAmountDriftBeforeNativeBridge()
    {
        var builder = NewTransactionBuilder();
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };

        var missingWitness = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                hopCount: 1u,
                hasLineageWitness: false,
                hasLineageVerifierRecord: true,
                publicAmount: "100",
                currentNoteAmount: "100",
                hasChangeOutput: false));
        Assert.Equal("hasLineageWitness", missingWitness.ParamName);
        Assert.Contains(
            "lineageWitness is required for this bundle",
            missingWitness.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);

        var missingChangeOutput = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                hopCount: 1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                publicAmount: "40",
                currentNoteAmount: "100",
                hasChangeOutput: false));
        Assert.Equal("hasChangeOutput", missingChangeOutput.ParamName);
        Assert.Contains(
            "changeOutput is required when publicAmount is less than current note amount",
            missingChangeOutput.Message,
            StringComparison.Ordinal);
        Assert.Empty(builder.Instructions);
    }

    [Fact]
    public void KagemushaRecursiveRedeemMetadataOverloadsAllowValidRelationshipsBeforeNativeRequestValidation()
    {
        var builder = NewTransactionBuilder();
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };

        var exactAmount = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                publicAmount: "100",
                currentNoteAmount: "100",
                hasChangeOutput: false));
        Assert.Equal("requestArchive", exactAmount.ParamName);
        Assert.Empty(builder.Instructions);

        var partialWithChange = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                publicAmount: "40",
                currentNoteAmount: "100",
                hasChangeOutput: true));
        Assert.Equal("requestArchive", partialWithChange.ParamName);
        Assert.Empty(builder.Instructions);

        var lineageOnly = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                hopCount: 1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false));
        Assert.Equal("requestArchive", lineageOnly.ParamName);
        Assert.Empty(builder.Instructions);

        var lineageAndAmount = Assert.Throws<ArgumentException>(() =>
            builder.KagemushaRecursiveRedeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                hopCount: 1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                publicAmount: "40",
                currentNoteAmount: "100",
                hasChangeOutput: true));
        Assert.Equal("requestArchive", lineageAndAmount.ParamName);
        Assert.Empty(builder.Instructions);
    }

    [Fact]
    public void BuildSignedEmbedsKagemushaInstructionArchiveWithoutReframing()
    {
        var archive = KagemushaArchive(
            KagemushaInstructionType.RedeemRecursive,
            new byte[] { 9, 8, 7, 6 },
            flags: 0x26);

        var envelope = new TransactionBuilder(
                "00000042",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .KagemushaInstructionArchive(KagemushaInstructionType.RedeemRecursive, archive)
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var instructions = ReadEncodedInstructions(envelope.PayloadBytes);
        var instruction = Assert.Single(instructions);
        Assert.Equal("iroha_data_model::isi::offline::RedeemKagemushaRecursive", instruction.WireId);
        Assert.Equal(archive, instruction.Payload);
        Assert.Equal((byte)0x26, instruction.Payload[39]);

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    [Fact]
    public void KagemushaInstructionArchiveAcceptsNativeAbi7RedeemInstructionFixture()
    {
        var archive = SharedRecursiveSpendAbi7Archive("redeem_instruction");
        var instruction = TransactionInstruction.KagemushaInstructionArchive(
            KagemushaInstructionType.RedeemRecursive,
            archive);

        Assert.Equal(KagemushaInstructionType.RedeemRecursive, instruction.InstructionType);
        Assert.Equal(archive, instruction.InstructionArchive);
    }

    [Fact]
    public void KagemushaInstructionArchiveRejectsMalformedWrongTypeAndMismatchedType()
    {
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                new byte[] { 0 }));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                NoritoCodec.Encode("KagemushaRecursiveSpendRedeemRequestV1", new byte[] { 1, 2, 3 })));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                KagemushaArchive(KagemushaInstructionType.Transfer, new byte[] { 1, 2, 3 })));

        var compressed = KagemushaArchive(KagemushaInstructionType.RedeemRecursive, new byte[] { 1, 2, 3 });
        compressed[22] = 1;
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                compressed));

        var unsupportedFlags = KagemushaArchive(KagemushaInstructionType.RedeemRecursive, new byte[] { 1, 2, 3 });
        unsupportedFlags[39] = 0x08;
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                unsupportedFlags));

        var invalidFieldBitset = KagemushaArchive(KagemushaInstructionType.RedeemRecursive, new byte[] { 1, 2, 3 });
        invalidFieldBitset[39] = 0x20;
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                invalidFieldBitset));

        var nonZeroPadding = WithHeaderPadding(
            KagemushaArchive(KagemushaInstructionType.RedeemRecursive, new byte[] { 1, 2, 3 }),
            new byte[] { 0x7f });
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                nonZeroPadding));

        var excessivePadding = WithHeaderPadding(
            KagemushaArchive(KagemushaInstructionType.RedeemRecursive, new byte[] { 1, 2, 3 }),
            new byte[65]);
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.KagemushaInstructionArchive(
                KagemushaInstructionType.RedeemRecursive,
                excessivePadding));
    }

    [Fact]
    public void BuildSignedEncodesNftAndTriggerInstructions()
    {
        var envelope = new TransactionBuilder(
                "00000042",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .SetNftKeyValue(
                "dragon$wonderland",
                "rarity",
                JsonValue.Create("legendary"))
            .RemoveNftKeyValue(
                "dragon$wonderland",
                "legacy_flag")
            .SetTriggerKeyValue(
                "settlement_window",
                "mode",
                JsonValue.Create("strict"))
            .RemoveTriggerKeyValue(
                "settlement_window",
                "legacy_flag")
            .MintTriggerRepetitions(3, "settlement_window")
            .BurnTriggerRepetitions(1, "settlement_window")
            .ExecuteTrigger(
                "settlement_window",
                JsonNode.Parse("""{ "force": true }"""))
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var instructions = ReadEncodedInstructions(envelope.PayloadBytes);
        Assert.Equal(7, instructions.Count);

        Assert.Equal("iroha.set_key_value", instructions[0].WireId);
        var setNftPayload = SkipNoritoHeader(instructions[0].Payload);
        Assert.Equal(3u, BinaryPrimitives.ReadUInt32LittleEndian(setNftPayload[..4]));
        var setNftObject = ReadField(setNftPayload[4..], out var setNftOffsetAfterObject);
        var setNftDomain = ReadNoritoString(ReadField(setNftObject, out var setNftOffsetAfterDomain));
        var setNftName = ReadNoritoString(ReadField(setNftObject[setNftOffsetAfterDomain..], out _));
        var setNftKey = ReadNoritoString(ReadField(setNftPayload[(4 + setNftOffsetAfterObject)..], out var setNftOffsetAfterKey));
        var setNftValue = ReadNoritoString(ReadField(setNftPayload[(4 + setNftOffsetAfterObject + setNftOffsetAfterKey)..], out _));
        Assert.Equal("wonderland", setNftDomain);
        Assert.Equal("dragon", setNftName);
        Assert.Equal("rarity", setNftKey);
        Assert.Equal("\"legendary\"", setNftValue);

        Assert.Equal("iroha.remove_key_value", instructions[1].WireId);
        var removeNftPayload = SkipNoritoHeader(instructions[1].Payload);
        Assert.Equal(3u, BinaryPrimitives.ReadUInt32LittleEndian(removeNftPayload[..4]));
        var removeNftObject = ReadField(removeNftPayload[4..], out var removeNftOffsetAfterObject);
        var removeNftDomain = ReadNoritoString(ReadField(removeNftObject, out var removeNftOffsetAfterDomain));
        var removeNftName = ReadNoritoString(ReadField(removeNftObject[removeNftOffsetAfterDomain..], out _));
        var removeNftKey = ReadNoritoString(ReadField(removeNftPayload[(4 + removeNftOffsetAfterObject)..], out _));
        Assert.Equal("wonderland", removeNftDomain);
        Assert.Equal("dragon", removeNftName);
        Assert.Equal("legacy_flag", removeNftKey);

        Assert.Equal("iroha.set_key_value", instructions[2].WireId);
        var setTriggerPayload = SkipNoritoHeader(instructions[2].Payload);
        Assert.Equal(4u, BinaryPrimitives.ReadUInt32LittleEndian(setTriggerPayload[..4]));
        var setTriggerId = ReadNoritoString(ReadField(setTriggerPayload[4..], out var setTriggerOffsetAfterObject));
        var setTriggerKey = ReadNoritoString(ReadField(setTriggerPayload[(4 + setTriggerOffsetAfterObject)..], out var setTriggerOffsetAfterKey));
        var setTriggerValue = ReadNoritoString(ReadField(setTriggerPayload[(4 + setTriggerOffsetAfterObject + setTriggerOffsetAfterKey)..], out _));
        Assert.Equal("settlement_window", setTriggerId);
        Assert.Equal("mode", setTriggerKey);
        Assert.Equal("\"strict\"", setTriggerValue);

        Assert.Equal("iroha.remove_key_value", instructions[3].WireId);
        var removeTriggerPayload = SkipNoritoHeader(instructions[3].Payload);
        Assert.Equal(4u, BinaryPrimitives.ReadUInt32LittleEndian(removeTriggerPayload[..4]));
        var removeTriggerId = ReadNoritoString(ReadField(removeTriggerPayload[4..], out var removeTriggerOffsetAfterObject));
        var removeTriggerKey = ReadNoritoString(ReadField(removeTriggerPayload[(4 + removeTriggerOffsetAfterObject)..], out _));
        Assert.Equal("settlement_window", removeTriggerId);
        Assert.Equal("legacy_flag", removeTriggerKey);

        Assert.Equal("iroha.mint", instructions[4].WireId);
        var mintTriggerPayload = SkipNoritoHeader(instructions[4].Payload);
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(mintTriggerPayload[..4]));
        var mintTriggerRepetitions = BinaryPrimitives.ReadUInt32LittleEndian(ReadField(mintTriggerPayload[4..], out var mintTriggerOffsetAfterRepetitions));
        var mintTriggerId = ReadNoritoString(ReadField(mintTriggerPayload[(4 + mintTriggerOffsetAfterRepetitions)..], out _));
        Assert.Equal(3u, mintTriggerRepetitions);
        Assert.Equal("settlement_window", mintTriggerId);

        Assert.Equal("iroha.burn", instructions[5].WireId);
        var burnTriggerPayload = SkipNoritoHeader(instructions[5].Payload);
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(burnTriggerPayload[..4]));
        var burnTriggerRepetitions = BinaryPrimitives.ReadUInt32LittleEndian(ReadField(burnTriggerPayload[4..], out var burnTriggerOffsetAfterRepetitions));
        var burnTriggerId = ReadNoritoString(ReadField(burnTriggerPayload[(4 + burnTriggerOffsetAfterRepetitions)..], out _));
        Assert.Equal(1u, burnTriggerRepetitions);
        Assert.Equal("settlement_window", burnTriggerId);

        Assert.Equal("iroha.execute_trigger", instructions[6].WireId);
        var executeTriggerPayload = SkipNoritoHeader(instructions[6].Payload);
        var executeTriggerId = ReadNoritoString(ReadField(executeTriggerPayload, out var executeTriggerOffsetAfterId));
        var executeTriggerArgs = ReadNoritoString(ReadField(executeTriggerPayload[executeTriggerOffsetAfterId..], out _));
        Assert.Equal("settlement_window", executeTriggerId);
        Assert.Equal("{\"force\":true}", executeTriggerArgs);

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    private static void AssertSignedEnvelopeStructure(SignedTransactionEnvelope envelope, byte[] privateKeySeed)
    {
        var signatureField = ReadField(envelope.SignedTransactionBytes, out var offsetAfterSignature);
        var payloadField = ReadField(envelope.SignedTransactionBytes[offsetAfterSignature..], out var offsetAfterPayload);
        var attachmentsField = ReadField(envelope.SignedTransactionBytes[(offsetAfterSignature + offsetAfterPayload)..], out var offsetAfterAttachments);
        var multisigField = ReadField(envelope.SignedTransactionBytes[(offsetAfterSignature + offsetAfterPayload + offsetAfterAttachments)..], out _);

        Assert.Equal(envelope.PayloadBytes, payloadField);
        Assert.Equal(new byte[] { 0 }, attachmentsField);
        Assert.Equal(new byte[] { 0 }, multisigField);

        var signature = DecodeConstVec(signatureField);
        var payloadHash = IrohaHash.Hash(envelope.PayloadBytes);
        var publicKey = Ed25519Signer.GetPublicKey(privateKeySeed);
        Assert.True(Ed25519Signer.Verify(payloadHash, signature, publicKey));
    }

    private static TransactionBuilder NewTransactionBuilder()
    {
        return new TransactionBuilder(FixtureChainId, FixtureAccountId);
    }

    public static IEnumerable<object[]> NonExactInstructionFieldCases()
    {
        yield return
        [
            "transfer asset definition",
            (Action<TransactionBuilder>)(builder => builder.TransferAsset(
                " " + FixtureAssetDefinitionId,
                "1",
                FixtureAccountId)),
        ];
        yield return
        [
            "transfer quantity",
            (Action<TransactionBuilder>)(builder => builder.TransferAsset(
                FixtureAssetDefinitionId,
                "1 ",
                FixtureAccountId)),
        ];
        yield return
        [
            "transfer destination",
            (Action<TransactionBuilder>)(builder => builder.TransferAsset(
                FixtureAssetDefinitionId,
                "1",
                FixtureAccountId + " ")),
        ];
        yield return
        [
            "domain id",
            (Action<TransactionBuilder>)(builder => builder.TransferDomain(" wonderland", FixtureAccountId)),
        ];
        yield return
        [
            "asset-definition transfer id",
            (Action<TransactionBuilder>)(builder => builder.TransferAssetDefinition(
                FixtureAssetDefinitionId + " ",
                FixtureAccountId)),
        ];
        yield return
        [
            "nft id",
            (Action<TransactionBuilder>)(builder => builder.TransferNft("dragon$wonderland ", FixtureAccountId)),
        ];
        yield return
        [
            "asset metadata key",
            (Action<TransactionBuilder>)(builder => builder.SetAssetKeyValue(
                FixtureAssetDefinitionId,
                FixtureAccountId,
                " display_name",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "domain metadata key",
            (Action<TransactionBuilder>)(builder => builder.SetDomainKeyValue(
                "wonderland",
                "display_name ",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "account metadata account",
            (Action<TransactionBuilder>)(builder => builder.SetAccountKeyValue(
                " " + FixtureAccountId,
                "display_name",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "trigger id",
            (Action<TransactionBuilder>)(builder => builder.SetTriggerKeyValue(
                " settlement_window",
                "mode",
                JsonValue.Create("strict"))),
        ];
        yield return
        [
            "execute trigger id",
            (Action<TransactionBuilder>)(builder => builder.ExecuteTrigger("settlement_window ")),
        ];
    }

    [Fact]
    public void AddInstructionAcceptsTransferFactories()
    {
        var builder = new TransactionBuilder(
            "00000042",
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .AddInstruction(TransactionInstruction.TransferDomain(
                "wonderland",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"))
            .AddInstruction(TransactionInstruction.TransferAssetDefinition(
                "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"))
            .AddInstruction(TransactionInstruction.TransferNft(
                "dragon$wonderland",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"));

        Assert.Collection(
            builder.Instructions,
            instruction => Assert.IsType<TransferDomainInstruction>(instruction),
            instruction => Assert.IsType<TransferAssetDefinitionInstruction>(instruction),
            instruction => Assert.IsType<TransferNftInstruction>(instruction));
    }

    [Fact]
    public void BuildSignedEncodesDomainAssetDefinitionAndNftTransfers()
    {
        var authority = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
        var destination = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";

        var envelope = new TransactionBuilder("00000042", authority)
            .TransferDomain("wonderland", destination)
            .TransferAssetDefinition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", destination)
            .TransferNft("dragon$wonderland", destination)
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var instructions = ReadEncodedInstructions(envelope.PayloadBytes);
        Assert.Equal(3, instructions.Count);

        Assert.Equal("iroha.transfer", instructions[0].WireId);
        var domainTransferPayload = SkipNoritoHeader(instructions[0].Payload);
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(domainTransferPayload[..4]));
        _ = ReadField(domainTransferPayload[4..], out var domainTransferOffsetAfterSource);
        var transferredDomain = ReadNoritoString(ReadField(domainTransferPayload[(4 + domainTransferOffsetAfterSource)..], out var domainTransferOffsetAfterObject));
        var domainTransferDestination = ReadField(domainTransferPayload[(4 + domainTransferOffsetAfterSource + domainTransferOffsetAfterObject)..], out _);
        Assert.Equal("wonderland", transferredDomain);
        Assert.NotEmpty(domainTransferDestination);

        Assert.Equal("iroha.transfer", instructions[1].WireId);
        var assetDefinitionTransferPayload = SkipNoritoHeader(instructions[1].Payload);
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(assetDefinitionTransferPayload[..4]));
        _ = ReadField(assetDefinitionTransferPayload[4..], out var assetDefinitionTransferOffsetAfterSource);
        var transferredAssetDefinition = ReadField(assetDefinitionTransferPayload[(4 + assetDefinitionTransferOffsetAfterSource)..], out var assetDefinitionTransferOffsetAfterObject);
        var assetDefinitionTransferDestination = ReadField(assetDefinitionTransferPayload[(4 + assetDefinitionTransferOffsetAfterSource + assetDefinitionTransferOffsetAfterObject)..], out _);
        Assert.Equal(144, transferredAssetDefinition.Length);
        Assert.NotEmpty(assetDefinitionTransferDestination);

        Assert.Equal("iroha.transfer", instructions[2].WireId);
        var nftTransferPayload = SkipNoritoHeader(instructions[2].Payload);
        Assert.Equal(3u, BinaryPrimitives.ReadUInt32LittleEndian(nftTransferPayload[..4]));
        _ = ReadField(nftTransferPayload[4..], out var nftTransferOffsetAfterSource);
        var transferredNft = ReadField(nftTransferPayload[(4 + nftTransferOffsetAfterSource)..], out var nftTransferOffsetAfterObject);
        var transferredNftDomain = ReadNoritoString(ReadField(transferredNft, out var nftOffsetAfterDomain));
        var transferredNftName = ReadNoritoString(ReadField(transferredNft[nftOffsetAfterDomain..], out _));
        var nftTransferDestination = ReadField(nftTransferPayload[(4 + nftTransferOffsetAfterSource + nftTransferOffsetAfterObject)..], out _);
        Assert.Equal("wonderland", transferredNftDomain);
        Assert.Equal("dragon", transferredNftName);
        Assert.NotEmpty(nftTransferDestination);

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    private static byte[] ReadField(ReadOnlySpan<byte> bytes, out int consumed)
    {
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        consumed = 8 + length;
        return bytes.Slice(8, length).ToArray();
    }

    private static byte[] DecodeConstVec(ReadOnlySpan<byte> bytes)
    {
        var count = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        var output = new byte[count];
        var offset = 8;
        for (var index = 0; index < count; index++)
        {
            var fieldLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes.Slice(offset, 8)));
            Assert.Equal(1, fieldLength);
            output[index] = bytes[offset + 8];
            offset += 9;
        }

        Assert.Equal(bytes.Length, offset);
        return output;
    }

    private static List<EncodedInstruction> ReadEncodedInstructions(ReadOnlySpan<byte> payloadBytes)
    {
        _ = ReadField(payloadBytes, out var offsetAfterChainId);
        _ = ReadField(payloadBytes[offsetAfterChainId..], out var offsetAfterAuthority);
        _ = ReadField(payloadBytes[(offsetAfterChainId + offsetAfterAuthority)..], out var offsetAfterCreationTime);
        var executable = ReadField(payloadBytes[(offsetAfterChainId + offsetAfterAuthority + offsetAfterCreationTime)..], out _);

        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(executable[..4]));
        var instructionsBytes = ReadField(executable[4..], out _);
        var count = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(instructionsBytes[..8]));
        var offset = 8;
        var instructions = new List<EncodedInstruction>(count);
        for (var index = 0; index < count; index++)
        {
            var encodedInstruction = ReadField(instructionsBytes[offset..], out var consumed);
            offset += consumed;

            var wireIdBytes = ReadField(encodedInstruction, out var offsetAfterWireId);
            var payloadBytesVec = ReadField(encodedInstruction[offsetAfterWireId..], out _);
            var payload = ReadNoritoBytes(payloadBytesVec);
            instructions.Add(new EncodedInstruction(ReadNoritoString(wireIdBytes), payload));
        }

        Assert.Equal(instructionsBytes.Length, offset);
        return instructions;
    }

    private static string ReadNoritoString(ReadOnlySpan<byte> bytes)
    {
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        return Encoding.UTF8.GetString(bytes.Slice(8, length));
    }

    private static byte[] ReadNoritoBytes(ReadOnlySpan<byte> bytes)
    {
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        return bytes.Slice(8, length).ToArray();
    }

    private static byte[] SkipNoritoHeader(byte[] framedPayload)
    {
        Assert.True(framedPayload.Length >= NoritoHeader.EncodedLength);
        return framedPayload[NoritoHeader.EncodedLength..];
    }

    private static byte[] KagemushaArchive(
        KagemushaInstructionType instructionType,
        byte[] payload,
        byte flags = 0)
    {
        return NoritoCodec.Encode(instructionType.WireName(), payload, flags);
    }

    private static byte[] WithHeaderPadding(byte[] archive, byte[] padding)
    {
        var padded = new byte[archive.Length + padding.Length];
        Array.Copy(archive, 0, padded, 0, NoritoHeader.EncodedLength);
        Array.Copy(padding, 0, padded, NoritoHeader.EncodedLength, padding.Length);
        Array.Copy(
            archive,
            NoritoHeader.EncodedLength,
            padded,
            NoritoHeader.EncodedLength + padding.Length,
            archive.Length - NoritoHeader.EncodedLength);
        return padded;
    }

    private static byte[] SharedRecursiveSpendAbi7Archive(string archiveName)
    {
        using var document = LoadSharedRecursiveSpendAbi7Archives();
        var archive = document.RootElement
            .GetProperty("archives")
            .EnumerateArray()
            .First(candidate => candidate.GetProperty("name").GetString() == archiveName);
        return Convert.FromBase64String(archive.GetProperty("bytes_base64").GetString()!);
    }

    private static JsonDocument LoadSharedRecursiveSpendAbi7Archives()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory != null)
        {
            var candidate = Path.Combine(
                directory.FullName,
                "fixtures",
                "kagemusha_recursive_spend_abi7",
                "archives.json");
            if (File.Exists(candidate))
            {
                return JsonDocument.Parse(File.ReadAllText(candidate));
            }

            directory = directory.Parent;
        }

        throw new FileNotFoundException("missing shared recursive spend ABI-7 archives fixture");
    }

    private sealed record EncodedInstruction(string WireId, byte[] Payload);

    private sealed class RecordingHandler : HttpMessageHandler
    {
        private readonly Func<HttpRequestMessage, HttpResponseMessage> responder;

        public RecordingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        {
            this.responder = responder;
        }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            var response = responder(request);
            response.RequestMessage ??= request;
            return Task.FromResult(response);
        }
    }
}
