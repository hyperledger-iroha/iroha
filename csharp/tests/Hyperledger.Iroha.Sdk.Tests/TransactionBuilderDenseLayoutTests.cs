using System.Numerics;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Typed transaction builds in an explicit Norito layout. Building admits account identities and
/// the dense layout transcodes instructions without a managed dense encoder, so these tests need
/// the ABI-33 native bridge like every other builder test; only the vector suite
/// (<see cref="NoritoDenseLayoutVectorTests"/>) does not.
/// </summary>
public sealed class TransactionBuilderDenseLayoutTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureAssetDefinitionUuidHex = "a1586b9be1644b04889a88c08e401544";
    private const string FixtureDestinationPublicKeyHex =
        "6355691c178a8ff91007a7478afb955ef7352c63e7b25703984cf78b26e21a56";

    [Theory]
    [InlineData("transfer_asset")]
    [InlineData("transfer_domain")]
    [InlineData("transfer_asset_definition")]
    [InlineData("transfer_nft_demo")]
    [InlineData("mint_asset")]
    [InlineData("burn_asset")]
    [InlineData("asset_metadata_parity")]
    [InlineData("trigger_repetitions_demo")]
    public void TypedBuildsReproduceTheCurrentAndTheDenseFixtureBytes(string name)
    {
        var fixture = NoritoDenseTransactionFixtureTests.LoadFixtures().Single(item => item.Name == name);
        var seed = Convert.FromHexString(FixtureSeedHex);
        var builder = NewFixtureBuilder(fixture);
        AddFixtureInstructions(builder, name, fixture.Authority);

        // The typed inputs are the committed fixture's: the current layout reproduces it.
        var envelope = builder.BuildSigned(seed);
        Assert.Equal(fixture.CompactSignedFrame[NoritoHeader.EncodedLength..], envelope.SignedTransactionBytes);
        Assert.Equal(fixture.CompactPayloadFrame[NoritoHeader.EncodedLength..], envelope.PayloadBytes);

        // The same inputs built and signed in the dense layout reproduce the dense fixture.
        var dense = builder.BuildSignedBare(seed, NoritoWireLayout.Dense);
        Assert.Equal(NoritoWireLayout.Dense, dense.Layout);
        Assert.Equal(Convert.ToHexStringLower(fixture.DensePayload), Convert.ToHexStringLower(dense.PayloadBytes));
        Assert.Equal(Convert.ToHexStringLower(fixture.DenseSigned), Convert.ToHexStringLower(dense.SignedTransactionBytes));
        Assert.Equal(fixture.DenseSigningHash, Convert.ToHexStringLower(dense.SigningHash));

        // The compact layout through the same entry point is exactly `BuildSigned`.
        var compact = builder.BuildSignedBare(seed, NoritoWireLayout.Compact);
        Assert.Equal(NoritoWireLayout.Compact, compact.Layout);
        Assert.Equal(envelope.SignedTransactionBytes, compact.SignedTransactionBytes);
        Assert.Equal(envelope.PayloadBytes, compact.PayloadBytes);
        Assert.Equal(IrohaHash.Hash(envelope.PayloadBytes), compact.SigningHash);
        Assert.Equal(fixture.CompactSignedFrame, compact.EncodeFrame());
    }

    [Fact]
    public void ContractCallBatchWithMetadataBuildsTheSameTransactionInBothLayouts()
    {
        var fixture = NoritoDenseTransactionFixtureTests.LoadFixtures()
            .Single(static item => item.Name == "mixed_executable_batch");
        var seed = Convert.FromHexString(FixtureSeedHex);

        // The fixture's contract call, between two typed instructions, with its metadata and
        // gas-bound fee payment.
        var expected = Assert.IsType<WireBatchExecutable>(
            BareSignedTransaction.Decode(fixture.DenseSigned, NoritoWireLayout.Dense).Wire.Payload.Executable);
        var call = Assert.IsType<WireBatchContractCall>(expected.Items[1]).Invocation;
        var invocation = new TransactionContractInvocation(
            call.ContractAddress,
            call.ExpectedCodeHash,
            call.Entrypoint,
            call.Arguments);

        var builder = new TransactionBuilder(
                NetworkId.Parse(fixture.NetworkId),
                fixture.Authority,
                FeePaymentIntent.Authority(
                    [new FeeChargeLimit(FeeChargeKind.PipelineGas, FixtureAssetDefinitionId(), "1000")],
                    gasLimit: 100_000))
            .SetCreationTimeMilliseconds(fixture.CreationTimeMilliseconds)
            .SetTimeToLiveMilliseconds(fixture.TimeToLiveMilliseconds)
            .SetNonce(fixture.Nonce)
            .SetMetadata("fixture", JsonValue.Create("mixed-batch"))
            .SetMetadata("another", new JsonObject { ["b"] = 2, ["a"] = JsonValue.Create((string?)null) })
            .WithExecutableBatch(
            [
                TransactionBatchEntry.Instruction(
                    TransactionInstruction.MintAsset(FixtureAssetDefinitionId(), "7", fixture.Authority)),
                TransactionBatchEntry.ContractCall(invocation),
                TransactionBatchEntry.Instruction(
                    TransactionInstruction.MintTriggerRepetitions(2, "daily_report")),
            ]);

        var compact = builder.BuildSignedBare(seed, NoritoWireLayout.Compact);
        var dense = builder.BuildSignedBare(seed, NoritoWireLayout.Dense);
        Assert.Equal(builder.BuildSigned(seed).SignedTransactionBytes, compact.SignedTransactionBytes);

        // Both layouts carry the same transaction; only instruction payload bytes are layout-bound.
        var compactPayload = compact.Wire.Payload;
        var densePayload = dense.Wire.Payload;
        Assert.Equal(compactPayload.NetworkId, densePayload.NetworkId);
        Assert.Equal(compactPayload.CreationTimeMilliseconds, densePayload.CreationTimeMilliseconds);
        Assert.Equal(compactPayload.TimeToLiveMilliseconds, densePayload.TimeToLiveMilliseconds);
        Assert.Equal(compactPayload.Nonce, densePayload.Nonce);
        Assert.Equal(
            compactPayload.Metadata.Select(static entry => (entry.Key, entry.JsonText)),
            densePayload.Metadata.Select(static entry => (entry.Key, entry.JsonText)));
        Assert.Equal(
            new[] { ("another", "{\"a\":null,\"b\":2}"), ("fixture", "\"mixed-batch\"") },
            densePayload.Metadata.Select(static entry => (entry.Key, entry.JsonText)));
        var fee = Assert.IsType<WireAuthorityFeePayment>(densePayload.FeePayment);
        Assert.Equal(100_000UL, fee.GasLimit);
        var limit = Assert.Single(fee.ChargeLimits);
        Assert.Equal(1U, limit.Kind);
        Assert.Equal(Convert.FromHexString(FixtureAssetDefinitionUuidHex), limit.AssetDefinitionId);
        Assert.Equal(new WireNumeric(1000, 0), limit.MaxAmount);

        var compactItems = Assert.IsType<WireBatchExecutable>(compactPayload.Executable).Items;
        var denseItems = Assert.IsType<WireBatchExecutable>(densePayload.Executable).Items;
        Assert.Equal(3, denseItems.Count);
        // The dense contract call is the fixture's, byte for byte.
        Assert.Equal(
            TransactionWireCodec.EncodeContractInvocation(NoritoWireLayout.Dense, call),
            TransactionWireCodec.EncodeContractInvocation(
                NoritoWireLayout.Dense,
                Assert.IsType<WireBatchContractCall>(denseItems[1]).Invocation));
        foreach (var index in new[] { 0, 2 })
        {
            var framed = Assert.IsType<WireBatchInstruction>(compactItems[index]).Instruction;
            var bare = Assert.IsType<WireBatchInstruction>(denseItems[index]).Instruction;
            Assert.Equal(framed.WireId, bare.WireId);
            Assert.Equal(NoritoWireLayout.Compact, NoritoCodec.ReadLayout(framed.Payload));
            Assert.True(bare.Payload.AsSpan().IndexOf("NRT0"u8) < 0);
        }

        // Each layout is signed over its own payload hash.
        Assert.NotEqual(compact.SigningHash, dense.SigningHash);
        Assert.NotEqual(compact.Signature, dense.Signature);
        Assert.True(dense.SignedTransactionBytes.Length < compact.SignedTransactionBytes.Length);
    }

    [Fact]
    public void EveryDenseCapableInstructionBuildsADenseEnvelopeTheStrictDecoderAccepts()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "typed_transaction_v1.json")));
        var root = fixture.RootElement;
        string Text(string name) => root.GetProperty(name).GetString()!;
        var authority = Text("authority");
        var destination = Text("destination");
        var definition = Text("asset_definition_id");
        var nft = Text("nft_id");
        var trigger = Text("trigger_id");
        var key = Text("metadata_key");
        var value = JsonValue.Create(Text("metadata_value"));
        const string domain = "wonderland.universal";

        var instructions = new TransactionInstruction[]
        {
            TransactionInstruction.TransferAsset(definition, "10", destination),
            TransactionInstruction.TransferDomain(domain, destination),
            TransactionInstruction.TransferAssetDefinition(definition, destination),
            TransactionInstruction.TransferNft(nft, destination),
            TransactionInstruction.MintAsset(definition, "1.5", destination),
            TransactionInstruction.BurnAsset(definition, "2", destination),
            TransactionInstruction.SetAssetKeyValue(definition, authority, key, value),
            TransactionInstruction.RemoveAssetKeyValue(definition, authority, key),
            TransactionInstruction.SetAccountKeyValue(authority, key, value),
            TransactionInstruction.RemoveAccountKeyValue(authority, key),
            TransactionInstruction.SetDomainKeyValue(domain, key, value),
            TransactionInstruction.RemoveDomainKeyValue(domain, key),
            TransactionInstruction.SetAssetDefinitionKeyValue(definition, key, value),
            TransactionInstruction.RemoveAssetDefinitionKeyValue(definition, key),
            TransactionInstruction.SetNftKeyValue(nft, key, value),
            TransactionInstruction.RemoveNftKeyValue(nft, key),
            TransactionInstruction.SetTriggerKeyValue(trigger, key, value),
            TransactionInstruction.RemoveTriggerKeyValue(trigger, key),
            TransactionInstruction.MintTriggerRepetitions(7, trigger),
            TransactionInstruction.BurnTriggerRepetitions(3, trigger),
            TransactionInstruction.ExecuteTrigger(trigger, new JsonObject { ["force"] = true }),
        };
        Assert.All(instructions, static instruction => Assert.True(instruction.SupportsDenseLayout));
        Assert.Equal(instructions.Length, instructions.Select(static instruction => instruction.GetType()).Distinct().Count());

        var compactContext = new TransactionEncodingContext(authority);
        var denseContext = new TransactionEncodingContext(authority, NoritoWireLayout.Dense);
        Assert.Equal(NoritoWireLayout.Compact, compactContext.Layout);
        Assert.Equal(NoritoWireLayout.Dense, denseContext.NewWriter().Layout);
        foreach (var instruction in instructions)
        {
            // The current-layout envelope is the framed payload; the dense one is the bare payload.
            var framed = instruction.EncodeEnvelopePayload(compactContext);
            Assert.Equal(instruction.EncodeFramedPayload(compactContext), framed);
            Assert.Equal(NoritoWireLayout.Compact, NoritoCodec.ReadLayout(framed));
            var bare = instruction.EncodeEnvelopePayload(denseContext);
            Assert.Equal(instruction.EncodePayload(denseContext), bare);
            Assert.True(bare.AsSpan().IndexOf("NRT0"u8) < 0);
            Assert.True(bare.Length <= framed.Length - NoritoHeader.EncodedLength);
            Assert.Throws<InvalidOperationException>(() => instruction.EncodeFramedPayload(denseContext));
            // The managed dense encoder writes exactly what the data model writes for the frame.
            Assert.Equal(
                bare,
                NoritoInstructionLayoutNative.Transcode(
                    instruction.WireId, framed, NoritoWireLayout.Compact, NoritoWireLayout.Dense, "framed"));
            Assert.Equal(
                framed,
                NoritoInstructionLayoutNative.Transcode(
                    instruction.WireId, bare, NoritoWireLayout.Dense, NoritoWireLayout.Compact, "bare"));

            var envelope = denseContext.ToWireInstruction(instruction);
            Assert.Equal(instruction.WireId, envelope.WireId);
            Assert.True(TransactionWireCodec.IsWellFormedWireId(envelope.WireId));
            Assert.Equal(bare, envelope.Payload);
            Assert.Equal(
                TransactionWireCodec.EncodeInstruction(NoritoWireLayout.Dense, envelope),
                denseContext.EncodeInstruction(instruction));
        }

        // The builder signs all of them in one dense transaction and the strict decoder accepts it.
        var seed = Enumerable.Repeat((byte)0x41, 32).ToArray();
        var signer = AccountAddress.FromPublicKey(Ed25519Signer.GetPublicKey(seed)).ToI105();
        var builder = new TransactionBuilder(
                NetworkId.Parse(NoritoDenseTransactionFixtureTests.LoadFixtures()[0].NetworkId),
                signer,
                FeePaymentIntent.Authority([]))
            .SetCreationTimeMilliseconds(1_736_000_000_000);
        foreach (var instruction in instructions)
        {
            builder.AddInstruction(instruction);
        }

        var dense = builder.BuildSignedBare(seed, NoritoWireLayout.Dense);
        var decoded = Assert.IsType<WireInstructionsExecutable>(dense.Wire.Payload.Executable);
        Assert.Equal(
            instructions.Select(static instruction => instruction.WireId),
            decoded.Instructions.Select(static instruction => instruction.WireId));
        Assert.Equal(dense.SignedTransactionBytes, BareSignedTransaction.DecodeFrame(dense.EncodeFrame()).SignedTransactionBytes);
        Assert.True(Ed25519Signer.Verify(dense.SigningHash, dense.Signature, Ed25519Signer.GetPublicKey(seed)));
    }

    [Fact]
    public void InstructionsWithoutAManagedDenseEncoderAreTranscodedByTheNativeBridge()
    {
        var fixture = NoritoDenseTransactionFixtureTests.LoadFixtures()[0];
        var seed = Convert.FromHexString(FixtureSeedHex);
        var instruction = TransactionInstruction.CancelAssetLock("escrow-lock-1", "5");
        Assert.False(instruction.SupportsDenseLayout);

        var builder = NewFixtureBuilder(fixture).AddInstruction(instruction);
        var compact = builder.BuildSignedBare(seed, NoritoWireLayout.Compact);
        Assert.Equal(builder.BuildSigned(seed).SignedTransactionBytes, compact.SignedTransactionBytes);

        // The dense envelope is the bridge's transcoding of the compact frame, and back.
        var compactContext = new TransactionEncodingContext(fixture.Authority);
        var denseContext = new TransactionEncodingContext(fixture.Authority, NoritoWireLayout.Dense);
        var framed = instruction.EncodeEnvelopePayload(compactContext);
        var bare = instruction.EncodeEnvelopePayload(denseContext);
        Assert.True(bare.AsSpan().IndexOf("NRT0"u8) < 0);
        Assert.True(bare.Length <= framed.Length - NoritoHeader.EncodedLength);
        Assert.Equal(
            framed,
            NoritoInstructionLayoutNative.Transcode(
                instruction.WireId, bare, NoritoWireLayout.Dense, NoritoWireLayout.Compact, "bare"));

        var dense = builder.BuildSignedBare(seed, NoritoWireLayout.Dense);
        var decoded = Assert.IsType<WireInstructionsExecutable>(dense.Wire.Payload.Executable);
        var envelope = Assert.Single(decoded.Instructions);
        Assert.Equal(instruction.WireId, envelope.WireId);
        Assert.Equal(bare, envelope.Payload);
        Assert.Equal(dense.SignedTransactionBytes, BareSignedTransaction.Decode(dense.SignedTransactionBytes, NoritoWireLayout.Dense).SignedTransactionBytes);
        Assert.True(Ed25519Signer.Verify(dense.SigningHash, dense.Signature, Ed25519Signer.GetPublicKey(seed)));
        Assert.NotEqual(compact.SigningHash, dense.SigningHash);

        // A body that is not the instruction's canonical dense payload is refused on decode.
        var extended = dense.Wire.Payload with
        {
            Executable = new WireInstructionsExecutable([new WireInstruction(instruction.WireId, [.. bare, 0])]),
        };
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                TransactionWireCodec.EncodePayload(extended), NoritoWireLayout.Dense, "payload"));

        // The typed codecs do not speak fixed-width lengths.
        Assert.Throws<ArgumentOutOfRangeException>(() => builder.BuildSignedBare(seed, NoritoWireLayout.Fixed));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => new TransactionEncodingContext(fixture.Authority, NoritoWireLayout.Fixed));
    }

    private static TransactionBuilder NewFixtureBuilder(NoritoDenseTransactionFixtureTests.Fixture fixture) =>
        new TransactionBuilder(
                NetworkId.Parse(fixture.NetworkId),
                fixture.Authority,
                FeePaymentIntent.Authority([]))
            .SetCreationTimeMilliseconds(fixture.CreationTimeMilliseconds)
            .SetTimeToLiveMilliseconds(fixture.TimeToLiveMilliseconds)
            .SetNonce(fixture.Nonce);

    private static void AddFixtureInstructions(TransactionBuilder builder, string name, string authority)
    {
        var definition = FixtureAssetDefinitionId();
        var destination = AccountAddress.FromPublicKey(Convert.FromHexString(FixtureDestinationPublicKeyHex)).ToI105();
        _ = name switch
        {
            "transfer_asset" => builder.TransferAsset(definition, "10", destination),
            "transfer_domain" => builder.TransferDomain("wonderland.universal", destination),
            "transfer_asset_definition" => builder.TransferAssetDefinition(definition, destination),
            "transfer_nft_demo" => builder.TransferNft("rare_card$wonderland.universal", destination),
            "mint_asset" => builder.MintAsset(definition, "100", authority),
            "burn_asset" => builder.BurnAsset(definition, "5", authority),
            "asset_metadata_parity" => builder
                .SetAssetKeyValue(definition, authority, "note", JsonValue.Create("parity"))
                .RemoveAssetKeyValue(definition, authority, "note"),
            "trigger_repetitions_demo" => builder
                .MintTriggerRepetitions(2, "daily_report")
                .BurnTriggerRepetitions(1, "daily_report"),
            _ => throw new InvalidOperationException($"No typed inputs for fixture `{name}`."),
        };
    }

    // The canonical literal of the fixtures' asset definition: Base58 of the version byte, the
    // UUID and the first four BLAKE3 bytes of both.
    private static string FixtureAssetDefinitionId()
    {
        byte[] body = [1, .. Convert.FromHexString(FixtureAssetDefinitionUuidHex)];
        byte[] payload = [.. body, .. Blake3.Hash(body)[..4]];

        const string alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        var number = new BigInteger(payload, isUnsigned: true, isBigEndian: true);
        var text = new List<char>();
        while (number > 0)
        {
            number = BigInteger.DivRem(number, 58, out var remainder);
            text.Add(alphabet[(int)remainder]);
        }

        text.AddRange(payload.TakeWhile(static value => value == 0).Select(static _ => '1'));
        text.Reverse();
        return new string([.. text]);
    }
}
