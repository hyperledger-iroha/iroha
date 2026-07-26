using System.Net;
using System.Buffers.Binary;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class TransactionBuilderTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureChainId = "00000042";
    private const string FixtureAccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private static FeePaymentIntent EmptyAuthorityFeePayment =>
        FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>());

    [Fact]
    public void TransactionBuilderRejectsPaddedTopLevelFields()
    {
        Assert.Throws<ArgumentException>(() =>
            new TransactionBuilder(" 00000042", FixtureAccountId, EmptyAuthorityFeePayment));
        Assert.Throws<ArgumentException>(() =>
            new TransactionBuilder("00000042", $" {FixtureAccountId}", EmptyAuthorityFeePayment));

        var builder = new TransactionBuilder("00000042", FixtureAccountId, EmptyAuthorityFeePayment);
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
        Assert.Throws<ArgumentNullException>(() => context.EncodeQuantity(null!));
        Assert.Throws<ArgumentException>(() => context.EncodeAssetDefinitionId(" 62Fk4FPcMuLvW5QjDGNF2a4jAmjM"));
        Assert.Throws<ArgumentException>(() => context.EncodeNftId(" dragon$wonderland"));
        Assert.Throws<ArgumentException>(() => context.EncodeHashLiteral(" " + new string('a', 64)));
        Assert.Throws<ArgumentException>(() => context.EncodeFixedBytesLiteral(" 0x0102", expectedLength: 2));
    }

    [Fact]
    public void ExecutableBatchEncodingPreservesInstructionCallInstructionOrder()
    {
        var context = new TransactionEncodingContext(FixtureAccountId);
        var instruction = TransactionInstruction.TransferAsset(
            FixtureAssetDefinitionId,
            "1",
            FixtureAccountId);
        var hash = Enumerable.Repeat((byte)0xA5, 32).ToArray();
        var invocation = new TransactionContractInvocation(
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
            hash,
            "run",
            [1, 2, 3]);
        var encoded = context.EncodeExecutableBatch([
            TransactionBatchEntry.Instruction(instruction),
            TransactionBatchEntry.ContractCall(invocation),
            TransactionBatchEntry.Instruction(instruction),
        ]);

        Assert.Equal(4U, BinaryPrimitives.ReadUInt32LittleEndian(encoded));
        var sequenceOffset = 12;
        Assert.Equal(3UL, BinaryPrimitives.ReadUInt64LittleEndian(encoded.AsSpan(sequenceOffset)));
        var cursor = sequenceOffset + 8;
        var tags = new List<uint>();
        for (var index = 0; index < 3; index++)
        {
            var itemLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(encoded.AsSpan(cursor)));
            cursor += 8;
            tags.Add(BinaryPrimitives.ReadUInt32LittleEndian(encoded.AsSpan(cursor)));
            cursor += itemLength;
        }
        Assert.Equal([0U, 1U, 0U], tags);
        Assert.Equal(encoded.Length, cursor);

        hash[0] = 0;
        Assert.Equal(0xA5, invocation.ExpectedCodeHash[0]);

        var emptyArguments = new TransactionContractInvocation(
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
            Enumerable.Repeat((byte)0xA5, 32).ToArray(),
            "run",
            Array.Empty<byte>());
        Assert.NotNull(emptyArguments.Arguments);
        Assert.Empty(emptyArguments.Arguments!);
    }

    [Fact]
    public void ExecutableBatchBuilderRequiresGasAndEmitsCanonicalJsonShape()
    {
        var invocation = new TransactionContractInvocation(
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
            Enumerable.Repeat((byte)0x11, 32).ToArray(),
            "run");
        var missingGas = new TransactionBuilder(
            FixtureChainId,
            FixtureAccountId,
            EmptyAuthorityFeePayment)
            .AddContractCall(invocation);
        Assert.Throws<InvalidOperationException>(() => missingGas.BuildUnsignedPayload());

        var instruction = TransactionInstruction.TransferAsset(
            FixtureAssetDefinitionId,
            "1",
            FixtureAccountId);
        var payload = new TransactionBuilder(
            FixtureChainId,
            FixtureAccountId,
            FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>(), gasLimit: 100_000))
            .WithExecutableBatch([
                TransactionBatchEntry.Instruction(instruction),
                TransactionBatchEntry.ContractCall(invocation),
                TransactionBatchEntry.Instruction(instruction),
            ])
            .SetCreationTimeMilliseconds(1_736_000_000_000)
            .BuildUnsignedPayload();

        var batch = Assert.IsType<JsonArray>(payload.Executable["Batch"]);
        Assert.Equal(3, batch.Count);
        Assert.NotNull(batch[0]!["Instruction"]);
        Assert.NotNull(batch[1]!["ContractCall"]);
        Assert.NotNull(batch[2]!["Instruction"]);
        Assert.Matches(
            "^hash:[0-9A-F]{64}#[0-9A-F]{4}$",
            batch[1]!["ContractCall"]!["expected_code_hash"]!.GetValue<string>());
        var json = JsonSerializer.Serialize(payload);
        Assert.Contains("\"instructions\":{\"Batch\"", json);
        Assert.DoesNotContain("\"Executable\"", json);
    }

    [Fact]
    public void ContractInvocationRequiresCanonicalV1Bech32mAddress()
    {
        var hash = Enumerable.Repeat((byte)0x11, 32).ToArray();
        var validAddresses = new[]
        {
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
            "tairac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8qtydf6x",
        };
        foreach (var address in validAddresses)
        {
            _ = new TransactionContractInvocation(address, hash, "run");
        }

        var invalidAddresses = new[]
        {
            "abc",
            " tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
            "TAIRAC1QYQQQQQQQQQQQQPUTUV64ZHF0A0A4HHLQDJ2LHNWUZQ4XJQDDCYQ8",
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyqp",
            "tairac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8q7ca9ly",
            "tairac1qgqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8qtm5n60",
            "tairac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpkqaty5s",
            "tairac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8pkjeu85",
        };
        foreach (var address in invalidAddresses)
        {
            Assert.Throws<ArgumentException>(
                () => new TransactionContractInvocation(address, hash, "run"));
        }
    }
    private const string FixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

    [Theory]
    [InlineData("swift_transfer_asset_basic", 805, 1423, "aaf57e9f247a5d92ba3c4c7d5076bd4aacf9b24b7de3b11e511b026784dec38b")]
    [InlineData("swift_mint_asset_basic", 695, 1313, "e38b26f35c901bf2099facd691511ea1ad1d1ebc7c43d98a9f473731fdeecd09")]
    [InlineData("swift_burn_asset_basic", 695, 1313, "8e2d28a9da78b1db4800d1e2e86be623a7d566235616c08daf813d9dcdd9a907")]
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
            FixtureAccountId,
            EmptyAuthorityFeePayment)
            .SetCreationTimeMilliseconds((ulong)payload.GetProperty("creation_time_ms").GetInt64())
            .SetTimeToLiveMilliseconds((ulong)payload.GetProperty("time_to_live_ms").GetInt64())
            .SetNonce((uint)payload.GetProperty("nonce").GetInt32());

        var instruction = payload.GetProperty("executable").GetProperty("Instructions")[0];
        var arguments = instruction.GetProperty("arguments");
        var action = arguments.GetProperty("action").GetString();
        var assetDefinitionId = arguments.GetProperty("asset_definition_id").GetString()!;
        var quantity = arguments.GetProperty("quantity").GetString()!;
        var destination = FixtureAccountId;

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

    [Fact]
    public void SignedTransactionEnvelopeDefensivelyCopiesBytes()
    {
        var envelope = NewTransactionBuilder()
            .TransferAsset(FixtureAssetDefinitionId, "1", FixtureAccountId)
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var expectedNoritoBytes = envelope.NoritoBytes;
        var expectedSignedTransactionBytes = envelope.SignedTransactionBytes;
        var expectedPayloadBytes = envelope.PayloadBytes;
        var expectedTransactionHash = envelope.TransactionHash;
        var expectedTransactionHashHex = envelope.TransactionHashHex;

        MutateFirstByte(envelope.NoritoBytes);
        MutateFirstByte(envelope.SignedTransactionBytes);
        MutateFirstByte(envelope.PayloadBytes);
        MutateFirstByte(envelope.TransactionHash);

        Assert.Equal(expectedNoritoBytes, envelope.NoritoBytes);
        Assert.Equal(expectedSignedTransactionBytes, envelope.SignedTransactionBytes);
        Assert.Equal(expectedPayloadBytes, envelope.PayloadBytes);
        Assert.Equal(expectedTransactionHash, envelope.TransactionHash);
        Assert.Equal(expectedTransactionHashHex, envelope.TransactionHashHex);
        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));

        var constructorSignatureBytes = Enumerable
            .Range(0, Ed25519Signer.SignatureLength)
            .Select(index => (byte)(0x0a + index))
            .ToArray();
        var constructorPayloadBytes = new byte[] { 0x07, 0x08, 0x09 };
        var constructorSignedTransactionBytes = BuildSignedTransactionBytes(
            constructorSignatureBytes,
            constructorPayloadBytes);
        var constructorNoritoBytes = constructorSignedTransactionBytes.ToArray();
        var constructorTransactionHash = ComputeTransactionHash(constructorSignedTransactionBytes);
        var expectedConstructorSignedTransactionBytes = constructorSignedTransactionBytes.ToArray();
        var expectedConstructorPayloadBytes = constructorPayloadBytes.ToArray();
        var expectedConstructorTransactionHash = constructorTransactionHash.ToArray();
        var direct = new SignedTransactionEnvelope(
            constructorNoritoBytes,
            constructorSignedTransactionBytes,
            constructorPayloadBytes,
            constructorTransactionHash);

        MutateFirstByte(constructorNoritoBytes);
        MutateFirstByte(constructorSignedTransactionBytes);
        MutateFirstByte(constructorPayloadBytes);
        MutateFirstByte(constructorTransactionHash);
        MutateFirstByte(direct.NoritoBytes);
        MutateFirstByte(direct.SignedTransactionBytes);
        MutateFirstByte(direct.PayloadBytes);
        MutateFirstByte(direct.TransactionHash);

        Assert.Equal(expectedConstructorSignedTransactionBytes, direct.NoritoBytes);
        Assert.Equal(expectedConstructorSignedTransactionBytes, direct.SignedTransactionBytes);
        Assert.Equal(expectedConstructorPayloadBytes, direct.PayloadBytes);
        Assert.Equal(expectedConstructorTransactionHash, direct.TransactionHash);
        Assert.Equal(Convert.ToHexString(expectedConstructorTransactionHash).ToLowerInvariant(), direct.TransactionHashHex);

        static void MutateFirstByte(byte[] value)
        {
            Assert.NotEmpty(value);
            value[0] ^= 0xff;
        }
    }

    [Fact]
    public void SignedTransactionEnvelopeRejectsMalformedConstructorBytes()
    {
        var signatureBytes = Enumerable.Repeat((byte)0x08, Ed25519Signer.SignatureLength).ToArray();
        var payloadBytes = new byte[] { 0x07 };
        var signedTransactionBytes = BuildSignedTransactionBytes(signatureBytes, payloadBytes);
        var noritoBytes = signedTransactionBytes.ToArray();
        var transactionHash = ComputeTransactionHash(signedTransactionBytes);
        var malformedSignedTransactionBytes = new byte[] { 0x04, 0x05, 0x06 };

        AssertArgumentException(
            "noritoBytes",
            () => new SignedTransactionEnvelope([], signedTransactionBytes, payloadBytes, transactionHash));
        AssertArgumentException(
            "signedTransactionBytes",
            () => new SignedTransactionEnvelope([0x01], [], payloadBytes, transactionHash));
        AssertArgumentException(
            "payloadBytes",
            () => new SignedTransactionEnvelope(noritoBytes, signedTransactionBytes, [], transactionHash));
        AssertArgumentException(
            "transactionHash",
            () => new SignedTransactionEnvelope(noritoBytes, signedTransactionBytes, payloadBytes, transactionHash[..^1]));
        AssertArgumentException(
            "noritoBytes",
            () => new SignedTransactionEnvelope([0x04, 0xff, 0x06], signedTransactionBytes, payloadBytes, transactionHash));
        AssertArgumentException(
            "signedTransactionBytes",
            () => new SignedTransactionEnvelope(
                malformedSignedTransactionBytes,
                malformedSignedTransactionBytes,
                payloadBytes,
                ComputeTransactionHash(malformedSignedTransactionBytes)));
        AssertArgumentException(
            "payloadBytes",
            () => new SignedTransactionEnvelope(noritoBytes, signedTransactionBytes, [0x08], transactionHash));
        AssertArgumentException(
            "signedTransactionBytes",
            () =>
            {
                var withNonEmptyAttachments = BuildSignedTransactionBytes(signatureBytes, payloadBytes, attachments: [1]);
                _ = new SignedTransactionEnvelope(
                    withNonEmptyAttachments,
                    withNonEmptyAttachments,
                    payloadBytes,
                    ComputeTransactionHash(withNonEmptyAttachments));
            });
        AssertArgumentException(
            "signedTransactionBytes",
            () =>
            {
                var withTrailingField = signedTransactionBytes.Concat(new byte[8]).ToArray();
                _ = new SignedTransactionEnvelope(
                    withTrailingField,
                    withTrailingField,
                    payloadBytes,
                    ComputeTransactionHash(withTrailingField));
            });
        AssertArgumentException(
            "transactionHash",
            () =>
            {
                var mismatchedHash = transactionHash.ToArray();
                mismatchedHash[0] ^= 0xff;
                _ = new SignedTransactionEnvelope(noritoBytes, signedTransactionBytes, payloadBytes, mismatchedHash);
            });
    }

    [Theory]
    [InlineData("", FixtureAccountId)]
    [InlineData(" 00000042", FixtureAccountId)]
    [InlineData("00000042 ", FixtureAccountId)]
    [InlineData("0000 0042", FixtureAccountId)]
    [InlineData("0000\u000042", FixtureAccountId)]
    [InlineData(FixtureChainId, "")]
    [InlineData(FixtureChainId, " ")]
    [InlineData(FixtureChainId, " sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData(FixtureChainId, "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData(FixtureChainId, "sorauﾛ1N ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData(FixtureChainId, "sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData(FixtureChainId, "merchant@sora")]
    [InlineData(FixtureChainId, "0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")]
    [InlineData(FixtureChainId, "n753Xnﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛ")]
    public void ConstructorRejectsNonExactRequiredFields(string chainId, string authorityAccountId)
    {
        Assert.Throws<ArgumentException>(() =>
            new TransactionBuilder(chainId, authorityAccountId, EmptyAuthorityFeePayment));
    }

    [Fact]
    public void SetCreationTimeRejectsZeroAndPreEpochValues()
    {
        var builder = NewTransactionBuilder();

        Assert.Throws<ArgumentOutOfRangeException>(() => builder.SetCreationTimeMilliseconds(0));
        Assert.Throws<ArgumentOutOfRangeException>(() => builder.SetCreationTime(DateTimeOffset.UnixEpoch));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            builder.SetCreationTime(DateTimeOffset.UnixEpoch.AddMilliseconds(-1));
        });
    }

    [Fact]
    public void SetCreationTimeAcceptsPositiveUnixMilliseconds()
    {
        var builder = NewTransactionBuilder();

        var result = builder.SetCreationTime(DateTimeOffset.UnixEpoch.AddMilliseconds(1));

        Assert.Same(builder, result);
        Assert.Equal(1UL, builder.CreationTimeMilliseconds);
    }

    [Fact]
    public void TriggerRepetitionInstructionsRejectZeroBeforeSigning()
    {
        var builder = NewTransactionBuilder();

        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            builder.MintTriggerRepetitions(0, "settlement_window");
        });
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            builder.BurnTriggerRepetitions(0, "settlement_window");
        });
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            TransactionInstruction.MintTriggerRepetitions(0, "settlement_window");
        });
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            TransactionInstruction.BurnTriggerRepetitions(0, "settlement_window");
        });
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            _ = new MintTriggerRepetitionsInstruction(0, "settlement_window");
        });
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            _ = new BurnTriggerRepetitionsInstruction(0, "settlement_window");
        });

        var mint = TransactionInstruction.MintTriggerRepetitions(1, "settlement_window");
        var burn = TransactionInstruction.BurnTriggerRepetitions(1, "settlement_window");
        Assert.Throws<ArgumentOutOfRangeException>(() => mint with { Repetitions = 0 });
        Assert.Throws<ArgumentOutOfRangeException>(() => burn with { Repetitions = 0 });
        Assert.Empty(builder.Instructions);
    }

    [Theory]
    [InlineData("0.0")]
    [InlineData("0.0000")]
    [InlineData("-1")]
    [InlineData("-1.25")]
    public void AssetQuantityInstructionsRejectNoncanonicalOrNegativeValuesBeforeSigning(string quantity)
    {
        var builder = NewTransactionBuilder();

        Assert.Throws<ArgumentException>(() =>
        {
            builder.TransferAsset(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            builder.MintAsset(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            builder.BurnAsset(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            TransactionInstruction.TransferAsset(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            TransactionInstruction.MintAsset(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            TransactionInstruction.BurnAsset(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            _ = new TransferAssetInstruction(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            _ = new MintAssetInstruction(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            _ = new BurnAssetInstruction(FixtureAssetDefinitionId, quantity, FixtureAccountId);
        });

        var transfer = TransactionInstruction.TransferAsset(FixtureAssetDefinitionId, "1", FixtureAccountId);
        var mint = TransactionInstruction.MintAsset(FixtureAssetDefinitionId, "1", FixtureAccountId);
        var burn = TransactionInstruction.BurnAsset(FixtureAssetDefinitionId, "1", FixtureAccountId);
        Assert.Throws<ArgumentException>(() => transfer with { Quantity = quantity });
        Assert.Throws<ArgumentException>(() => mint with { Quantity = quantity });
        Assert.Throws<ArgumentException>(() => burn with { Quantity = quantity });
        Assert.Empty(builder.Instructions);
    }

    [Theory]
    [InlineData("merchant@sora")]
    [InlineData("0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")]
    [InlineData("n753Xnﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛ")]
    public void AccountInstructionFactoriesRejectNonCanonicalAccountIdsBeforeSigning(string accountId)
    {
        var builder = NewTransactionBuilder();

        Assert.Throws<ArgumentException>(() => builder.TransferAsset(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(() => builder.TransferDomain("wonderland", accountId));
        Assert.Throws<ArgumentException>(() => builder.TransferAssetDefinition(FixtureAssetDefinitionId, accountId));
        Assert.Throws<ArgumentException>(() => builder.TransferNft("dragon$wonderland", accountId));
        Assert.Throws<ArgumentException>(() => builder.MintAsset(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(() => builder.BurnAsset(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(
            () => builder.SetAssetKeyValue(FixtureAssetDefinitionId, accountId, "display_name", JsonValue.Create("Treasury buffer")));
        Assert.Throws<ArgumentException>(() => builder.RemoveAssetKeyValue(FixtureAssetDefinitionId, accountId, "display_name"));
        Assert.Throws<ArgumentException>(
            () => builder.SetAccountKeyValue(accountId, "display_name", JsonValue.Create("Treasury buffer")));
        Assert.Throws<ArgumentException>(() => builder.RemoveAccountKeyValue(accountId, "display_name"));

        Assert.Throws<ArgumentException>(() => TransactionInstruction.TransferAsset(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.TransferDomain("wonderland", accountId));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.TransferAssetDefinition(FixtureAssetDefinitionId, accountId));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.TransferNft("dragon$wonderland", accountId));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.MintAsset(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.BurnAsset(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(
            () => TransactionInstruction.SetAssetKeyValue(
                FixtureAssetDefinitionId,
                accountId,
                "display_name",
                JsonValue.Create("Treasury buffer")));
        Assert.Throws<ArgumentException>(
            () => TransactionInstruction.RemoveAssetKeyValue(FixtureAssetDefinitionId, accountId, "display_name"));
        Assert.Throws<ArgumentException>(
            () => TransactionInstruction.SetAccountKeyValue(accountId, "display_name", JsonValue.Create("Treasury buffer")));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.RemoveAccountKeyValue(accountId, "display_name"));

        Assert.Throws<ArgumentException>(() => new TransferAssetInstruction(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(() => new TransferDomainInstruction("wonderland", accountId));
        Assert.Throws<ArgumentException>(() => new TransferAssetDefinitionInstruction(FixtureAssetDefinitionId, accountId));
        Assert.Throws<ArgumentException>(() => new TransferNftInstruction("dragon$wonderland", accountId));
        Assert.Throws<ArgumentException>(() => new MintAssetInstruction(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(() => new BurnAssetInstruction(FixtureAssetDefinitionId, "1", accountId));
        Assert.Throws<ArgumentException>(
            () => new SetAssetKeyValueInstruction(
                FixtureAssetDefinitionId,
                accountId,
                "display_name",
                JsonValue.Create("Treasury buffer")));
        Assert.Throws<ArgumentException>(
            () => new RemoveAssetKeyValueInstruction(FixtureAssetDefinitionId, accountId, "display_name"));
        Assert.Throws<ArgumentException>(
            () => new SetAccountKeyValueInstruction(accountId, "display_name", JsonValue.Create("Treasury buffer")));
        Assert.Throws<ArgumentException>(() => new RemoveAccountKeyValueInstruction(accountId, "display_name"));

        var transferAsset = TransactionInstruction.TransferAsset(FixtureAssetDefinitionId, "1", FixtureAccountId);
        var transferDomain = TransactionInstruction.TransferDomain("wonderland", FixtureAccountId);
        var transferDefinition = TransactionInstruction.TransferAssetDefinition(FixtureAssetDefinitionId, FixtureAccountId);
        var transferNft = TransactionInstruction.TransferNft("dragon$wonderland", FixtureAccountId);
        var mint = TransactionInstruction.MintAsset(FixtureAssetDefinitionId, "1", FixtureAccountId);
        var burn = TransactionInstruction.BurnAsset(FixtureAssetDefinitionId, "1", FixtureAccountId);
        var setAsset = TransactionInstruction.SetAssetKeyValue(
            FixtureAssetDefinitionId,
            FixtureAccountId,
            "display_name",
            JsonValue.Create("Treasury buffer"));
        var removeAsset = TransactionInstruction.RemoveAssetKeyValue(
            FixtureAssetDefinitionId,
            FixtureAccountId,
            "display_name");
        var setAccount = TransactionInstruction.SetAccountKeyValue(
            FixtureAccountId,
            "display_name",
            JsonValue.Create("Treasury buffer"));
        var removeAccount = TransactionInstruction.RemoveAccountKeyValue(FixtureAccountId, "display_name");

        Assert.Throws<ArgumentException>(() => transferAsset with { DestinationAccountId = accountId });
        Assert.Throws<ArgumentException>(() => transferDomain with { DestinationAccountId = accountId });
        Assert.Throws<ArgumentException>(() => transferDefinition with { DestinationAccountId = accountId });
        Assert.Throws<ArgumentException>(() => transferNft with { DestinationAccountId = accountId });
        Assert.Throws<ArgumentException>(() => mint with { DestinationAccountId = accountId });
        Assert.Throws<ArgumentException>(() => burn with { DestinationAccountId = accountId });
        Assert.Throws<ArgumentException>(() => setAsset with { AccountId = accountId });
        Assert.Throws<ArgumentException>(() => removeAsset with { AccountId = accountId });
        Assert.Throws<ArgumentException>(() => setAccount with { AccountId = accountId });
        Assert.Throws<ArgumentException>(() => removeAccount with { AccountId = accountId });
        Assert.Empty(builder.Instructions);
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" memo")]
    [InlineData("memo ")]
    [InlineData("\u00A0memo")]
    [InlineData("memo\u00A0")]
    [InlineData("me mo")]
    [InlineData("me\u00A0mo")]
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
    [InlineData("me mo")]
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
    [InlineData("0000 0042")]
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
    [InlineData("ali ce")]
    [InlineData("ali\u00A0ce")]
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
    [InlineData("+1")]
    [InlineData("+0.1")]
    [InlineData("-0")]
    [InlineData("-0.0")]
    [InlineData("01")]
    [InlineData("01.0")]
    [InlineData(".1")]
    [InlineData("1.")]
    [InlineData("-")]
    [InlineData("1 0")]
    [InlineData("1.\u00A00")]
    [InlineData("1\u0000")]
    public void CanonicalQuantityParserRejectsNonExactNumerics(string numeric)
    {
        Assert.Throws<NumericV1.NumericException>(() => NumericV1.QuantityValue.ParseCanonical(numeric));
    }

    [Theory]
    [InlineData("0")]
    [InlineData("1")]
    [InlineData("1.23")]
    [InlineData("0.0000000000000000000000000001")]
    public void TransactionEncodingContextAcceptsCanonicalQuantities(string numeric)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.NotEmpty(context.EncodeQuantity(NumericV1.QuantityValue.ParseCanonical(numeric)));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" dragon$wonderland")]
    [InlineData("dragon$wonderland ")]
    [InlineData("dragon$ wonderland")]
    [InlineData("dra gon$wonderland")]
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
    [InlineData("62Fk4FPcMuLvW5QjDGNF2 a4jAmjM")]
    [InlineData("62Fk4FPcMuLvW5QjDGNF2a4jAmjM\u0000")]
    public void TransactionEncodingContextRejectsNonExactAssetDefinitionIds(string assetDefinitionId)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeAssetDefinitionId(assetDefinitionId));
    }

    [Fact]
    public void TransactionEncodingContextRejectsNonExactHashAndFixedByteLiterals()
    {
        var context = new TransactionEncodingContext(FixtureAccountId);
        var hash = new string('a', 64);

        foreach (var literal in new[] { "", " ", " " + hash, hash + " ", "\u00A0" + hash, hash[..32] + " " + hash[32..], hash + "\u0000" })
        {
            Assert.ThrowsAny<ArgumentException>(() => context.EncodeHashLiteral(literal));
        }

        foreach (var literal in new[] { "", " ", " 0x0102", "0x0102 ", "\u00A00x0102", "0x01 02", "0x0102\u0000" })
        {
            Assert.ThrowsAny<ArgumentException>(() => context.EncodeFixedBytesLiteral(literal, expectedLength: 2));
        }
    }

    [Theory]
    [InlineData("0x010203", 2)]
    [InlineData("0x010", 2)]
    [InlineData("0xzz", 1)]
    [InlineData("zz", 1)]
    public void TransactionEncodingContextRejectsMalformedFixedByteLiterals(
        string literal,
        int expectedLength)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeFixedBytesLiteral(literal, expectedLength));
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
    [InlineData("sort key")]
    [InlineData("sort\u00A0key")]
    [InlineData("sort\u0000key")]
    public void TransactionEncodingContextRejectsNonExactOptionalStrings(string value)
    {
        var context = new TransactionEncodingContext(FixtureAccountId);

        Assert.Throws<ArgumentException>(() => context.EncodeOptionalString(value));
    }

    [Theory]
    [InlineData(" sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData("sorauﾛ1N ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("merchant@sora")]
    [InlineData("0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")]
    [InlineData("n753Xnﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛﾛ")]
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
        var configureException = Record.Exception(() => configure(builder));
        if (configureException is not null)
        {
            var argumentException = Assert.IsAssignableFrom<ArgumentException>(configureException);
            Assert.NotEmpty(label);
            Assert.NotEmpty(argumentException.Message);
            return;
        }

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
        Assert.Throws<ArgumentException>(() => instruction.EncodeInstructionBox(FixtureAccountId.Insert(8, " ")));
        Assert.Throws<ArgumentException>(() => instruction.EncodeInstructionBox("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"));
    }

    [Fact]
    public async Task LedgerClientSubmitAndWaitPollsUntilTerminalState()
    {
        var transaction = new TransactionBuilder(
            "00000042",
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
            EmptyAuthorityFeePayment)
            .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "15.75", "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
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
            }, cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal(PipelineTransactionState.Applied, status.State);
        Assert.Equal((ulong)11, status.BlockHeight);
        Assert.True(status.IsTerminal);
    }

    [Fact]
    public void BuildSignedEncodesAssetMetadataInstructions()
    {
        var envelope = new TransactionBuilder(
                "00000042",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                EmptyAuthorityFeePayment)
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
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
            EmptyAuthorityFeePayment)
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
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                EmptyAuthorityFeePayment)
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
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
            EmptyAuthorityFeePayment)
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
    public void BuildSignedEncodesNftAndTriggerInstructions()
    {
        var envelope = new TransactionBuilder(
                "00000042",
                "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
                EmptyAuthorityFeePayment)
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

    [Fact]
    public void BuildSignedDefensivelyCopiesJsonInstructionPayloads()
    {
        var assetValue = MutableInstructionJsonPayload("asset");
        var domainValue = MutableInstructionJsonPayload("domain");
        var accountValue = MutableInstructionJsonPayload("account");
        var assetDefinitionValue = MutableInstructionJsonPayload("asset-definition");
        var nftValue = MutableInstructionJsonPayload("nft");
        var triggerReplacementValue = MutableInstructionJsonPayload("trigger-init");
        var executeReplacementArgs = MutableInstructionJsonPayload("execute-init");

        var setAsset = TransactionInstruction.SetAssetKeyValue(
            FixtureAssetDefinitionId,
            FixtureAccountId,
            "display_name",
            assetValue);
        var setDomain = TransactionInstruction.SetDomainKeyValue("wonderland", "display_name", domainValue);
        var setAccount = TransactionInstruction.SetAccountKeyValue(FixtureAccountId, "display_name", accountValue);
        var setAssetDefinition = TransactionInstruction.SetAssetDefinitionKeyValue(
            FixtureAssetDefinitionId,
            "ticker",
            assetDefinitionValue);
        var setNft = TransactionInstruction.SetNftKeyValue("dragon$wonderland", "rarity", nftValue);
        var setTrigger = TransactionInstruction.SetTriggerKeyValue(
            "settlement_window",
            "mode",
            MutableInstructionJsonPayload("trigger-placeholder")) with
        {
            Value = triggerReplacementValue,
        };
        var executeTrigger = TransactionInstruction.ExecuteTrigger(
            "settlement_window",
            MutableInstructionJsonPayload("execute-placeholder")) with
        {
            Args = executeReplacementArgs,
        };

        var builder = NewTransactionBuilder()
            .AddInstruction(setAsset)
            .AddInstruction(setDomain)
            .AddInstruction(setAccount)
            .AddInstruction(setAssetDefinition)
            .AddInstruction(setNft)
            .AddInstruction(setTrigger)
            .AddInstruction(executeTrigger)
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17);

        MutateInstructionJsonPayload(assetValue, "asset-input-mutated");
        MutateInstructionJsonPayload(domainValue, "domain-input-mutated");
        MutateInstructionJsonPayload(accountValue, "account-input-mutated");
        MutateInstructionJsonPayload(assetDefinitionValue, "asset-definition-input-mutated");
        MutateInstructionJsonPayload(nftValue, "nft-input-mutated");
        MutateInstructionJsonPayload(triggerReplacementValue, "trigger-input-mutated");
        MutateInstructionJsonPayload(executeReplacementArgs, "execute-input-mutated");

        MutateInstructionJsonPayload(setAsset.Value!.AsObject(), "asset-getter-mutated");
        MutateInstructionJsonPayload(setDomain.Value!.AsObject(), "domain-getter-mutated");
        MutateInstructionJsonPayload(setAccount.Value!.AsObject(), "account-getter-mutated");
        MutateInstructionJsonPayload(setAssetDefinition.Value!.AsObject(), "asset-definition-getter-mutated");
        MutateInstructionJsonPayload(setNft.Value!.AsObject(), "nft-getter-mutated");
        MutateInstructionJsonPayload(setTrigger.Value!.AsObject(), "trigger-getter-mutated");
        MutateInstructionJsonPayload(executeTrigger.Args!.AsObject(), "execute-getter-mutated");

        var envelope = builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var instructions = ReadEncodedInstructions(envelope.PayloadBytes);
        Assert.Equal(7, instructions.Count);
        Assert.Equal(ExpectedInstructionJsonPayload("asset"), ReadSetJsonPayload(instructions[0].Payload, prefixLength: 0));
        Assert.Equal(ExpectedInstructionJsonPayload("domain"), ReadSetJsonPayload(instructions[1].Payload, prefixLength: 4));
        Assert.Equal(ExpectedInstructionJsonPayload("account"), ReadSetJsonPayload(instructions[2].Payload, prefixLength: 4));
        Assert.Equal(
            ExpectedInstructionJsonPayload("asset-definition"),
            ReadSetJsonPayload(instructions[3].Payload, prefixLength: 4));
        Assert.Equal(ExpectedInstructionJsonPayload("nft"), ReadSetJsonPayload(instructions[4].Payload, prefixLength: 4));
        Assert.Equal(
            ExpectedInstructionJsonPayload("trigger-init"),
            ReadSetJsonPayload(instructions[5].Payload, prefixLength: 4));
        Assert.Equal(ExpectedInstructionJsonPayload("execute-init"), ReadExecuteTriggerJsonPayload(instructions[6].Payload));

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));

        static JsonObject MutableInstructionJsonPayload(string label)
        {
            return new JsonObject
            {
                ["tags"] = new JsonArray("stable"),
                ["nested"] = new JsonObject
                {
                    ["enabled"] = true,
                },
                ["label"] = label,
            };
        }

        static string ExpectedInstructionJsonPayload(string label)
        {
            return "{\"label\":\"" + label + "\",\"nested\":{\"enabled\":true},\"tags\":[\"stable\"]}";
        }

        static void MutateInstructionJsonPayload(JsonObject payload, string label)
        {
            payload["label"] = label;
            payload["extra"] = true;
            payload["nested"]!.AsObject()["enabled"] = false;
            payload["tags"]!.AsArray()[0] = "mutated";
        }

        static string ReadSetJsonPayload(byte[] framedPayload, int prefixLength)
        {
            var payload = SkipNoritoHeader(framedPayload);
            _ = ReadField(payload[prefixLength..], out var offsetAfterObject);
            _ = ReadField(payload[(prefixLength + offsetAfterObject)..], out var offsetAfterKey);
            return ReadNoritoString(ReadField(payload[(prefixLength + offsetAfterObject + offsetAfterKey)..], out _));
        }

        static string ReadExecuteTriggerJsonPayload(byte[] framedPayload)
        {
            var payload = SkipNoritoHeader(framedPayload);
            _ = ReadField(payload, out var offsetAfterTriggerId);
            return ReadNoritoString(ReadField(payload[offsetAfterTriggerId..], out _));
        }
    }

    [Fact]
    public void BuilderPublicSnapshotsDoNotMutateSignedState()
    {
        var metadataValue = new JsonObject
        {
            ["label"] = "initial",
            ["nested"] = new JsonObject
            {
                ["enabled"] = true,
            },
        };
        var builder = NewTransactionBuilder()
            .TransferAsset(FixtureAssetDefinitionId, "1", FixtureAccountId)
            .SetMetadata("trace", metadataValue)
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17);

        metadataValue["label"] = "input-mutated";
        metadataValue["nested"]!.AsObject()["enabled"] = false;

        var instructionsSnapshot = Assert.IsAssignableFrom<IList<TransactionInstruction>>(builder.Instructions);
        instructionsSnapshot[0] = TransactionInstruction.MintAsset(FixtureAssetDefinitionId, "99", FixtureAccountId);

        var metadataSnapshot = builder.Metadata;
        metadataSnapshot["trace"]!.AsObject()["label"] = "snapshot-mutated";
        metadataSnapshot["trace"]!.AsObject()["nested"]!.AsObject()["enabled"] = false;
        if (metadataSnapshot is IDictionary<string, JsonNode?> writableMetadata)
        {
            var mutationException = Record.Exception(() =>
            {
                writableMetadata["injected"] = JsonValue.Create("bad");
            });
            Assert.True(
                mutationException is null or NotSupportedException,
                $"Unexpected metadata snapshot mutation exception: {mutationException}");
        }

        var envelope = builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var instructions = ReadEncodedInstructions(envelope.PayloadBytes);
        var instruction = Assert.Single(instructions);
        Assert.Equal("iroha.transfer", instruction.WireId);

        var metadata = ReadEncodedMetadata(envelope.PayloadBytes);
        var entry = Assert.Single(metadata);
        Assert.Equal("trace", entry.Key);
        Assert.Equal("{\"label\":\"initial\",\"nested\":{\"enabled\":true}}", entry.Value);
        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    private static void AssertSignedEnvelopeStructure(SignedTransactionEnvelope envelope, byte[] privateKeySeed)
    {
        Assert.Equal(envelope.SignedTransactionBytes, envelope.NoritoBytes);
        Assert.Equal(ComputeTransactionHash(envelope.SignedTransactionBytes), envelope.TransactionHash);

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

    private static void AssertArgumentException(string paramName, Action action)
    {
        var exception = Assert.Throws<ArgumentException>(action);
        Assert.Equal(paramName, exception.ParamName);
    }

    private static byte[] ComputeTransactionHash(ReadOnlySpan<byte> signedTransactionBytes)
    {
        var entrypoint = new OfflineNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(signedTransactionBytes);
        return IrohaHash.Hash(entrypoint.ToArray());
    }

    private static byte[] BuildSignedTransactionBytes(
        byte[] signatureBytes,
        byte[] payloadBytes,
        byte[]? attachments = null,
        byte[]? multisig = null)
    {
        var signedTransaction = new OfflineNoritoWriter();
        signedTransaction.WriteField(EncodeConstVec(signatureBytes));
        signedTransaction.WriteField(payloadBytes);
        signedTransaction.WriteField(attachments ?? [0]);
        signedTransaction.WriteField(multisig ?? [0]);
        return signedTransaction.ToArray();
    }

    private static byte[] EncodeConstVec(byte[] value)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt64LittleEndian((ulong)value.Length);
        foreach (var item in value)
        {
            writer.WriteField([item]);
        }

        return writer.ToArray();
    }

    private static TransactionBuilder NewTransactionBuilder()
    {
        return new TransactionBuilder(FixtureChainId, FixtureAccountId, EmptyAuthorityFeePayment);
    }

    private static void AssertArgumentDiagnostic(
        string expectedMessage,
        string expectedParameterName,
        Action action)
    {
        var error = Assert.ThrowsAny<ArgumentException>(action);
        Assert.Equal(expectedParameterName, error.ParamName);
        Assert.True(
            error.Message.Length >= expectedMessage.Length
            && (error.Message.Length == expectedMessage.Length
                || error.Message[expectedMessage.Length] == ' '
                || error.Message[expectedMessage.Length] == '.'
                || error.Message[expectedMessage.Length] == ':'),
            $"unexpected diagnostic suffix: {error.Message}");
        Assert.Equal(expectedMessage, error.Message[..expectedMessage.Length]);
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
            "mint asset definition",
            (Action<TransactionBuilder>)(builder => builder.MintAsset(
                FixtureAssetDefinitionId + "\u0000",
                "1",
                FixtureAccountId)),
        ];
        yield return
        [
            "mint quantity",
            (Action<TransactionBuilder>)(builder => builder.MintAsset(
                FixtureAssetDefinitionId,
                "\u00A01",
                FixtureAccountId)),
        ];
        yield return
        [
            "mint destination",
            (Action<TransactionBuilder>)(builder => builder.MintAsset(
                FixtureAssetDefinitionId,
                "1",
                FixtureAccountId + "\u0000")),
        ];
        yield return
        [
            "burn asset definition",
            (Action<TransactionBuilder>)(builder => builder.BurnAsset(
                " " + FixtureAssetDefinitionId,
                "1",
                FixtureAccountId)),
        ];
        yield return
        [
            "burn quantity",
            (Action<TransactionBuilder>)(builder => builder.BurnAsset(
                FixtureAssetDefinitionId,
                "1\u0000",
                FixtureAccountId)),
        ];
        yield return
        [
            "burn destination",
            (Action<TransactionBuilder>)(builder => builder.BurnAsset(
                FixtureAssetDefinitionId,
                "1",
                " " + FixtureAccountId)),
        ];
        yield return
        [
            "transfer quantity internal whitespace",
            (Action<TransactionBuilder>)(builder => builder.TransferAsset(
                FixtureAssetDefinitionId,
                "1 0",
                FixtureAccountId)),
        ];
        yield return
        [
            "domain id",
            (Action<TransactionBuilder>)(builder => builder.TransferDomain(" wonderland", FixtureAccountId)),
        ];
        yield return
        [
            "domain id internal whitespace",
            (Action<TransactionBuilder>)(builder => builder.TransferDomain("wonder land", FixtureAccountId)),
        ];
        yield return
        [
            "domain transfer destination",
            (Action<TransactionBuilder>)(builder => builder.TransferDomain("wonderland", FixtureAccountId + " ")),
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
            "asset-definition transfer destination",
            (Action<TransactionBuilder>)(builder => builder.TransferAssetDefinition(
                FixtureAssetDefinitionId,
                "\u00A0" + FixtureAccountId)),
        ];
        yield return
        [
            "nft id",
            (Action<TransactionBuilder>)(builder => builder.TransferNft("dragon$wonderland ", FixtureAccountId)),
        ];
        yield return
        [
            "nft transfer destination",
            (Action<TransactionBuilder>)(builder => builder.TransferNft("dragon$wonderland", FixtureAccountId + "\u0000")),
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
            "asset metadata key internal whitespace",
            (Action<TransactionBuilder>)(builder => builder.SetAssetKeyValue(
                FixtureAssetDefinitionId,
                FixtureAccountId,
                "display name",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "asset metadata asset definition",
            (Action<TransactionBuilder>)(builder => builder.SetAssetKeyValue(
                FixtureAssetDefinitionId + " ",
                FixtureAccountId,
                "display_name",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "asset metadata account",
            (Action<TransactionBuilder>)(builder => builder.SetAssetKeyValue(
                FixtureAssetDefinitionId,
                FixtureAccountId + "\u0000",
                "display_name",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "remove asset metadata key",
            (Action<TransactionBuilder>)(builder => builder.RemoveAssetKeyValue(
                FixtureAssetDefinitionId,
                FixtureAccountId,
                "display_name\u0000")),
        ];
        yield return
        [
            "remove asset metadata asset definition",
            (Action<TransactionBuilder>)(builder => builder.RemoveAssetKeyValue(
                " " + FixtureAssetDefinitionId,
                FixtureAccountId,
                "display_name")),
        ];
        yield return
        [
            "remove asset metadata account",
            (Action<TransactionBuilder>)(builder => builder.RemoveAssetKeyValue(
                FixtureAssetDefinitionId,
                "\u00A0" + FixtureAccountId,
                "display_name")),
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
            "domain metadata id",
            (Action<TransactionBuilder>)(builder => builder.SetDomainKeyValue(
                "wonderland\u0000",
                "display_name",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "remove domain metadata key",
            (Action<TransactionBuilder>)(builder => builder.RemoveDomainKeyValue(
                "wonderland",
                "\u00A0display_name")),
        ];
        yield return
        [
            "remove domain metadata id",
            (Action<TransactionBuilder>)(builder => builder.RemoveDomainKeyValue(
                " wonderland",
                "display_name")),
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
            "account metadata key",
            (Action<TransactionBuilder>)(builder => builder.SetAccountKeyValue(
                FixtureAccountId,
                "display_name ",
                JsonValue.Create("Treasury buffer"))),
        ];
        yield return
        [
            "remove account metadata account",
            (Action<TransactionBuilder>)(builder => builder.RemoveAccountKeyValue(
                FixtureAccountId + "\u0000",
                "display_name")),
        ];
        yield return
        [
            "remove account metadata key",
            (Action<TransactionBuilder>)(builder => builder.RemoveAccountKeyValue(
                FixtureAccountId,
                " display_name")),
        ];
        yield return
        [
            "asset-definition metadata id",
            (Action<TransactionBuilder>)(builder => builder.SetAssetDefinitionKeyValue(
                "\u00A0" + FixtureAssetDefinitionId,
                "ticker",
                JsonValue.Create("XOR"))),
        ];
        yield return
        [
            "asset-definition metadata key",
            (Action<TransactionBuilder>)(builder => builder.SetAssetDefinitionKeyValue(
                FixtureAssetDefinitionId,
                "ticker\u0000",
                JsonValue.Create("XOR"))),
        ];
        yield return
        [
            "remove asset-definition metadata id",
            (Action<TransactionBuilder>)(builder => builder.RemoveAssetDefinitionKeyValue(
                FixtureAssetDefinitionId + " ",
                "ticker")),
        ];
        yield return
        [
            "remove asset-definition metadata key",
            (Action<TransactionBuilder>)(builder => builder.RemoveAssetDefinitionKeyValue(
                FixtureAssetDefinitionId,
                " ticker")),
        ];
        yield return
        [
            "nft metadata id",
            (Action<TransactionBuilder>)(builder => builder.SetNftKeyValue(
                " dragon$wonderland",
                "rarity",
                JsonValue.Create("legendary"))),
        ];
        yield return
        [
            "nft metadata key",
            (Action<TransactionBuilder>)(builder => builder.SetNftKeyValue(
                "dragon$wonderland",
                "rarity ",
                JsonValue.Create("legendary"))),
        ];
        yield return
        [
            "remove nft metadata id",
            (Action<TransactionBuilder>)(builder => builder.RemoveNftKeyValue(
                "dragon$wonderland\u0000",
                "rarity")),
        ];
        yield return
        [
            "remove nft metadata key",
            (Action<TransactionBuilder>)(builder => builder.RemoveNftKeyValue(
                "dragon$wonderland",
                "\u00A0rarity")),
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
            "trigger id internal whitespace",
            (Action<TransactionBuilder>)(builder => builder.SetTriggerKeyValue(
                "settlement window",
                "mode",
                JsonValue.Create("strict"))),
        ];
        yield return
        [
            "trigger key",
            (Action<TransactionBuilder>)(builder => builder.SetTriggerKeyValue(
                "settlement_window",
                "mode\u0000",
                JsonValue.Create("strict"))),
        ];
        yield return
        [
            "remove trigger id",
            (Action<TransactionBuilder>)(builder => builder.RemoveTriggerKeyValue(
                "settlement_window ",
                "mode")),
        ];
        yield return
        [
            "remove trigger key",
            (Action<TransactionBuilder>)(builder => builder.RemoveTriggerKeyValue(
                "settlement_window",
                " mode")),
        ];
        yield return
        [
            "mint trigger id",
            (Action<TransactionBuilder>)(builder => builder.MintTriggerRepetitions(
                1,
                "\u00A0settlement_window")),
        ];
        yield return
        [
            "burn trigger id",
            (Action<TransactionBuilder>)(builder => builder.BurnTriggerRepetitions(
                1,
                "settlement_window\u0000")),
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
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
            EmptyAuthorityFeePayment)
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

        var envelope = new TransactionBuilder("00000042", authority, EmptyAuthorityFeePayment)
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

    private static List<(string Key, string Value)> ReadEncodedMetadata(ReadOnlySpan<byte> payloadBytes)
    {
        _ = ReadField(payloadBytes, out var offsetAfterChainId);
        _ = ReadField(payloadBytes[offsetAfterChainId..], out var offsetAfterAuthority);
        _ = ReadField(payloadBytes[(offsetAfterChainId + offsetAfterAuthority)..], out var offsetAfterCreationTime);
        _ = ReadField(payloadBytes[(offsetAfterChainId + offsetAfterAuthority + offsetAfterCreationTime)..], out var offsetAfterExecutable);
        _ = ReadField(
            payloadBytes[(offsetAfterChainId + offsetAfterAuthority + offsetAfterCreationTime + offsetAfterExecutable)..],
            out var offsetAfterTimeToLive);
        _ = ReadField(
            payloadBytes[
                (offsetAfterChainId + offsetAfterAuthority + offsetAfterCreationTime + offsetAfterExecutable
                    + offsetAfterTimeToLive)..],
            out var offsetAfterNonce);
        _ = ReadField(
            payloadBytes[
                (offsetAfterChainId + offsetAfterAuthority + offsetAfterCreationTime + offsetAfterExecutable
                    + offsetAfterTimeToLive + offsetAfterNonce)..],
            out var offsetAfterFeePayment);
        var metadataBytes = ReadField(
            payloadBytes[
                (offsetAfterChainId + offsetAfterAuthority + offsetAfterCreationTime + offsetAfterExecutable
                    + offsetAfterTimeToLive + offsetAfterNonce + offsetAfterFeePayment)..],
            out _);

        var count = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(metadataBytes[..8]));
        var offset = 8;
        var metadata = new List<(string Key, string Value)>(count);
        for (var index = 0; index < count; index++)
        {
            var entryBytes = ReadField(metadataBytes[offset..], out var consumed);
            offset += consumed;

            var keyBytes = ReadField(entryBytes, out var offsetAfterKey);
            var valueField = ReadField(entryBytes[offsetAfterKey..], out _);
            var encodedJson = ReadField(valueField, out _);
            metadata.Add((ReadNoritoString(keyBytes), ReadNoritoString(encodedJson)));
        }

        Assert.Equal(metadataBytes.Length, offset);
        return metadata;
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
