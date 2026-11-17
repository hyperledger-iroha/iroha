using System.Text.Json;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// The Norito RPC fixture transactions in the dense layout
/// (<c>fixtures/norito_rpc/transaction_fixtures.dense.json</c>) and their current-layout twins
/// (<c>transaction_fixtures.manifest.json</c>): the typed transaction decoder reads both layouts
/// and re-encodes each byte-identically. Needs no native artifact.
/// </summary>
public sealed class NoritoDenseTransactionFixtureTests
{
    private const int CanonicalFixtureCount = 27;
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const byte Ed25519AlgorithmTag = 0;

    private static readonly IReadOnlySet<string> DenseManifestFields =
        new HashSet<string>(StringComparer.Ordinal) { "fixtures", "layout" };

    private static readonly IReadOnlySet<string> DenseFixtureFields =
        new HashSet<string>(StringComparer.Ordinal)
        {
            "name",
            "payload_base64",
            "signed_base64",
            "signing_hash",
        };

    [Fact]
    public void EveryDenseFixtureTransactionDecodesAndReEncodesByteIdentically()
    {
        var fixtures = LoadFixtures();
        Assert.Equal(CanonicalFixtureCount, fixtures.Count);

        foreach (var fixture in fixtures)
        {
            var transaction = BareSignedTransaction.Decode(fixture.DenseSigned, NoritoWireLayout.Dense);
            Assert.Equal(NoritoWireLayout.Dense, transaction.Layout);
            Assert.Equal(fixture.DenseSigned, transaction.SignedTransactionBytes);
            Assert.Equal(fixture.DenseSigned, TransactionWireCodec.EncodeSignedTransaction(transaction.Wire));
            Assert.Equal(fixture.DensePayload, transaction.PayloadBytes);

            var payload = TransactionWireCodec.DecodePayload(
                fixture.DensePayload, NoritoWireLayout.Dense, nameof(fixture));
            Assert.Equal(NoritoWireLayout.Dense, payload.Layout);
            Assert.Equal(fixture.DensePayload, TransactionWireCodec.EncodePayload(payload));

            // No nested instruction frame survives in the dense layout.
            Assert.True(
                fixture.DenseSigned.AsSpan().IndexOf("NRT0"u8) < 0,
                $"{fixture.Name}: dense transaction carries a nested Norito header");
        }
    }

    [Fact]
    public void DenseFixturesAreSignedOverTheDensePayloadHash()
    {
        var seed = Convert.FromHexString(FixtureSeedHex);
        var publicKey = Ed25519Signer.GetPublicKey(seed);

        foreach (var fixture in LoadFixtures())
        {
            var dense = BareSignedTransaction.Decode(fixture.DenseSigned, NoritoWireLayout.Dense);

            // The SDK's hash of the dense payload is the fixture's signing hash.
            Assert.Equal(fixture.DenseSigningHash, Convert.ToHexStringLower(dense.SigningHash));
            Assert.Equal(fixture.DenseSigningHash, Convert.ToHexStringLower(IrohaHash.Hash(fixture.DensePayload)));

            // The authority key verifies the signature over that hash and no other layout's hash.
            var authority = Assert.IsType<WireSingleKeyController>(dense.Wire.Payload.Authority);
            Assert.Equal(Ed25519AlgorithmTag, authority.TaggedPublicKey[0]);
            Assert.Equal(publicKey, authority.TaggedPublicKey[1..]);
            Assert.Equal(Ed25519Signer.SignatureLength, dense.Signature.Length);
            Assert.True(Ed25519Signer.Verify(dense.SigningHash, dense.Signature, publicKey), fixture.Name);

            var compact = BareSignedTransaction.DecodeFrame(fixture.CompactSignedFrame);
            Assert.NotEqual(Convert.ToHexStringLower(compact.SigningHash), fixture.DenseSigningHash);
            Assert.True(Ed25519Signer.Verify(compact.SigningHash, compact.Signature, publicKey), fixture.Name);
            Assert.False(Ed25519Signer.Verify(compact.SigningHash, dense.Signature, publicKey), fixture.Name);
            Assert.False(Ed25519Signer.Verify(dense.SigningHash, compact.Signature, publicKey), fixture.Name);

            // Signing the same payload with the fixture key reproduces the fixture bytes.
            var signature = Ed25519Signer.Sign(IrohaHash.Hash(fixture.DensePayload), seed);
            Assert.Equal(dense.Signature, signature);
            Assert.Equal(
                fixture.DenseSigned,
                TransactionWireCodec.EncodeSignedTransaction(NoritoWireLayout.Dense, signature, fixture.DensePayload));
        }
    }

    [Fact]
    public void DenseFixturesDecodeToTheSameTransactionsAsTheirCurrentLayoutTwins()
    {
        var compactTotal = 0;
        var denseTotal = 0;
        foreach (var fixture in LoadFixtures())
        {
            // The frame's header selects the layout: minor 0 with COMPACT_LEN.
            Assert.Equal(NoritoWireLayout.Compact, NoritoCodec.ReadLayout(fixture.CompactSignedFrame));
            var compact = BareSignedTransaction.DecodeFrame(fixture.CompactSignedFrame);
            Assert.Equal(NoritoWireLayout.Compact, compact.Layout);
            Assert.Equal(fixture.CompactSignedFrame, compact.EncodeFrame());
            Assert.Equal(
                fixture.CompactSignedFrame[NoritoHeader.EncodedLength..],
                compact.SignedTransactionBytes);
            Assert.Equal(
                NoritoCodec.DecodeWithLayout(
                    "iroha_data_model::transaction::signed::model::TransactionPayload",
                    fixture.CompactPayloadFrame,
                    NoritoWireLayout.Compact),
                compact.PayloadBytes);

            var dense = BareSignedTransaction.Decode(fixture.DenseSigned, NoritoWireLayout.Dense);
            var compactPayload = compact.Wire.Payload;
            var densePayload = dense.Wire.Payload;

            Assert.Equal(compactPayload.NetworkId, densePayload.NetworkId);
            Assert.Equal(NetworkId.Parse(fixture.NetworkId).ToBytes(), densePayload.NetworkId);
            Assert.Equal(Describe(compactPayload.Authority), Describe(densePayload.Authority));
            Assert.Equal(compactPayload.CreationTimeMilliseconds, densePayload.CreationTimeMilliseconds);
            Assert.Equal(fixture.CreationTimeMilliseconds, densePayload.CreationTimeMilliseconds);
            Assert.Equal(compactPayload.TimeToLiveMilliseconds, densePayload.TimeToLiveMilliseconds);
            Assert.Equal(fixture.TimeToLiveMilliseconds, densePayload.TimeToLiveMilliseconds);
            Assert.Equal(compactPayload.Nonce, densePayload.Nonce);
            Assert.Equal(fixture.Nonce, densePayload.Nonce);
            Assert.Equal(Describe(compactPayload.FeePayment), Describe(densePayload.FeePayment));
            Assert.Equal(
                compactPayload.Metadata.Select(static entry => (entry.Key, entry.JsonText)),
                densePayload.Metadata.Select(static entry => (entry.Key, entry.JsonText)));
            Assert.Equal(
                DescribeExecutable(compactPayload.Executable, out var compactInstructions),
                DescribeExecutable(densePayload.Executable, out var denseInstructions));

            // The same instructions travel in both layouts: a complete Norito frame per
            // instruction in the current layout, the bare dense payload in the dense one.
            Assert.Equal(compactInstructions.Count, denseInstructions.Count);
            foreach (var (framed, bare) in compactInstructions.Zip(denseInstructions))
            {
                Assert.Equal(framed.WireId, bare.WireId);
                Assert.Equal(NoritoWireLayout.Compact, NoritoCodec.ReadLayout(framed.Payload));
                Assert.True(bare.Payload.AsSpan().IndexOf("NRT0"u8) < 0);
                Assert.True(bare.Payload.Length + NoritoHeader.EncodedLength <= framed.Payload.Length);
            }

            compactTotal += compact.SignedTransactionBytes.Length;
            denseTotal += dense.SignedTransactionBytes.Length;
        }

        // Measured on the committed fixtures by the Rust codec: about 47% smaller.
        Assert.True(denseTotal * 100 < compactTotal * 56, $"dense {denseTotal} bytes vs compact {compactTotal} bytes");
    }

    [Fact]
    public void ATransactionNeverDecodesAsTheOtherLayout()
    {
        foreach (var fixture in LoadFixtures())
        {
            var compactBare = fixture.CompactSignedFrame[NoritoHeader.EncodedLength..];
            _ = BareSignedTransaction.Decode(compactBare, NoritoWireLayout.Compact);

            // Bare bytes of one layout are refused under the other, not silently misread.
            Assert.Throws<ArgumentException>(
                () => BareSignedTransaction.Decode(fixture.DenseSigned, NoritoWireLayout.Compact));
            Assert.Throws<ArgumentException>(
                () => BareSignedTransaction.Decode(compactBare, NoritoWireLayout.Dense));
            Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload(fixture.DensePayload, NoritoWireLayout.Compact, "payload"));
            Assert.Throws<ArgumentOutOfRangeException>(
                () => BareSignedTransaction.Decode(fixture.DenseSigned, NoritoWireLayout.Fixed));

            // A dense frame declares minor 1 and no flags, and round-trips through its header.
            var denseFrame = BareSignedTransaction.Decode(fixture.DenseSigned, NoritoWireLayout.Dense).EncodeFrame();
            Assert.Equal(1, denseFrame[5]);
            Assert.Equal(0, denseFrame[NoritoHeader.EncodedLength - 1]);
            Assert.Equal(fixture.DenseSigned, denseFrame[NoritoHeader.EncodedLength..]);
            Assert.Equal(NoritoWireLayout.Dense, NoritoCodec.ReadLayout(denseFrame));
            var fromFrame = BareSignedTransaction.DecodeFrame(denseFrame);
            Assert.Equal(NoritoWireLayout.Dense, fromFrame.Layout);
            Assert.Equal(fixture.DenseSigned, fromFrame.SignedTransactionBytes);
            Assert.Equal(denseFrame, fromFrame.EncodeFrame());

            // The current-layout frame reader refuses the dense frame.
            Assert.Throws<ArgumentException>(
                () => NoritoCodec.Decode(BareSignedTransaction.SignedTransactionSchemaName, denseFrame));

            // A header that declares the other layout over the same payload is refused: the
            // decoder follows the header and the payload does not parse in that layout.
            var denseRelabelledCurrent = denseFrame.ToArray();
            denseRelabelledCurrent[5] = 0;
            denseRelabelledCurrent[NoritoHeader.EncodedLength - 1] = 0x02;
            Assert.Equal(NoritoWireLayout.Compact, NoritoCodec.ReadLayout(denseRelabelledCurrent));
            Assert.Throws<ArgumentException>(() => BareSignedTransaction.DecodeFrame(denseRelabelledCurrent));

            var currentRelabelledDense = fixture.CompactSignedFrame.ToArray();
            currentRelabelledDense[5] = 1;
            currentRelabelledDense[NoritoHeader.EncodedLength - 1] = 0;
            Assert.Equal(NoritoWireLayout.Dense, NoritoCodec.ReadLayout(currentRelabelledDense));
            Assert.Throws<ArgumentException>(() => BareSignedTransaction.DecodeFrame(currentRelabelledDense));

            // Fixed-width frames are refused by the typed decoder rather than read as compact.
            var fixedWidth = fixture.CompactSignedFrame.ToArray();
            fixedWidth[NoritoHeader.EncodedLength - 1] = 0;
            Assert.Equal(NoritoWireLayout.Fixed, NoritoCodec.ReadLayout(fixedWidth));
            Assert.Throws<ArgumentOutOfRangeException>(() => BareSignedTransaction.DecodeFrame(fixedWidth));
        }
    }

    [Fact]
    public void DenseTransactionsRejectUnknownMinorsFlagsPaddingTruncationAndTrailingBytes()
    {
        foreach (var fixture in LoadFixtures())
        {
            var signed = fixture.DenseSigned;
            var payload = fixture.DensePayload;

            // Truncated input at every length, and trailing bytes.
            for (var length = 0; length < signed.Length; length++)
            {
                var truncated = signed[..length];
                Assert.Throws<ArgumentException>(
                    () => BareSignedTransaction.Decode(truncated, NoritoWireLayout.Dense));
            }

            for (var length = 0; length < payload.Length; length++)
            {
                var truncated = payload[..length];
                Assert.Throws<ArgumentException>(
                    () => TransactionWireCodec.DecodePayload(truncated, NoritoWireLayout.Dense, "payload"));
            }

            Assert.Throws<ArgumentException>(
                () => BareSignedTransaction.Decode([.. signed, 0], NoritoWireLayout.Dense));
            Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload([.. payload, 0], NoritoWireLayout.Dense, "payload"));

            var frame = BareSignedTransaction.Decode(signed, NoritoWireLayout.Dense).EncodeFrame();

            // An unknown header minor is refused.
            foreach (var minor in new byte[] { 2, 3, 0x7f, 0xff })
            {
                var future = frame.ToArray();
                future[5] = minor;
                Assert.Throws<ArgumentException>(() => NoritoCodec.ReadLayout(future));
                Assert.Throws<ArgumentException>(() => BareSignedTransaction.DecodeFrame(future));
            }

            // Dense has no layout flags.
            foreach (var flags in new byte[] { 0x01, 0x02, 0x04, 0x80, 0x82 })
            {
                var flagged = frame.ToArray();
                flagged[NoritoHeader.EncodedLength - 1] = flags;
                Assert.Throws<ArgumentException>(() => BareSignedTransaction.DecodeFrame(flagged));
            }

            // Dense has no header padding, and a frame ends with its payload.
            byte[] padded =
            [
                .. frame[..NoritoHeader.EncodedLength],
                .. new byte[8],
                .. frame[NoritoHeader.EncodedLength..],
            ];
            Assert.Throws<ArgumentException>(() => BareSignedTransaction.DecodeFrame(padded));
            Assert.Throws<ArgumentException>(() => BareSignedTransaction.DecodeFrame([.. frame, 0]));
            Assert.Throws<ArgumentException>(() => BareSignedTransaction.DecodeFrame(frame[..^1]));
        }
    }

    [Fact]
    public void DenseTransactionsRejectNonCanonicalCountsTagsAndUnmodeledValues()
    {
        var fixture = LoadFixtures().Single(static item => item.Name == "transfer_asset");
        var payload = fixture.DensePayload;

        // The payload ends with empty metadata `[len = 1][count = 0]` and absent attachments
        // `[len = 1][None]`.
        Assert.Equal(new byte[] { 0x01, 0x00, 0x01, 0x00 }, payload[^4..]);
        var prefix = payload[..^4];
        _ = TransactionWireCodec.DecodePayload([.. prefix, 0x01, 0x00, 0x01, 0x00], NoritoWireLayout.Dense, "payload");

        // An overlong metadata count.
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [.. prefix, 0x02, 0x80, 0x00, 0x01, 0x00], NoritoWireLayout.Dense, "payload"));
        // The current layout's fixed-width count where dense requires a varint.
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [.. prefix, 0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x00], NoritoWireLayout.Dense, "payload"));
        // A metadata count the remaining bytes cannot hold.
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [.. prefix, 0x01, 0x7f, 0x01, 0x00], NoritoWireLayout.Dense, "payload"));
        // An invalid Option tag and a present, unmodeled attachment set.
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [.. prefix, 0x01, 0x00, 0x01, 0x02], NoritoWireLayout.Dense, "payload"));
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [.. prefix, 0x01, 0x00, 0x03, 0x01, 0x01, 0x00], NoritoWireLayout.Dense, "payload"));

        // The domain discriminant: an overlong varint, an unknown variant and the current
        // layout's four-byte word are refused.
        Assert.Equal(new byte[] { 0x22, 0x00, 0x20 }, payload[..3]);
        var afterDomain = payload[35..];
        var networkId = payload[3..35];
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [0x23, 0x80, 0x00, 0x20, .. networkId, .. afterDomain], NoritoWireLayout.Dense, "payload"));
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [0x22, 0x02, 0x20, .. networkId, .. afterDomain], NoritoWireLayout.Dense, "payload"));
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(
                [0x25, 0x00, 0x00, 0x00, 0x00, 0x20, .. networkId, .. afterDomain], NoritoWireLayout.Dense, "payload"));

        // A present multisig bundle is not modeled; an invalid Option tag is malformed.
        var signed = fixture.DenseSigned;
        Assert.Equal(new byte[] { 0x01, 0x00 }, signed[^2..]);
        Assert.Throws<ArgumentException>(
            () => BareSignedTransaction.Decode([.. signed[..^2], 0x03, 0x01, 0x01, 0x00], NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(
            () => BareSignedTransaction.Decode([.. signed[..^1], 0x02], NoritoWireLayout.Dense));

        // Empty and oversized inputs are refused before any parsing.
        Assert.Throws<ArgumentException>(() => BareSignedTransaction.Decode([], NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(
            () => BareSignedTransaction.Decode(
                new byte[TransactionWireCodec.MaximumTransactionBytes + 1], NoritoWireLayout.Dense));
    }

    [Fact]
    public void DenseManifestIsClosedSchemaAndNamesTheCurrentLayoutFixtures()
    {
        var directory = Path.Combine(AppContext.BaseDirectory, "Fixtures", "norito_rpc");
        using var dense = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(directory, "transaction_fixtures.dense.json")));
        using var manifest = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(directory, "transaction_fixtures.manifest.json")));

        RequireExactProperties(dense.RootElement, DenseManifestFields, "dense manifest");
        Assert.Equal("dense", dense.RootElement.GetProperty("layout").GetString());
        foreach (var fixture in dense.RootElement.GetProperty("fixtures").EnumerateArray())
        {
            RequireExactProperties(fixture, DenseFixtureFields, "dense fixture");
        }

        // The same transactions, in the same order, as the current-layout manifest.
        Assert.Equal(
            manifest.RootElement.GetProperty("fixtures").EnumerateArray()
                .Select(static fixture => fixture.GetProperty("name").GetString()),
            dense.RootElement.GetProperty("fixtures").EnumerateArray()
                .Select(static fixture => fixture.GetProperty("name").GetString()));
    }

    internal sealed record Fixture(
        string Name,
        byte[] DensePayload,
        byte[] DenseSigned,
        string DenseSigningHash,
        byte[] CompactPayloadFrame,
        byte[] CompactSignedFrame,
        string NetworkId,
        string Authority,
        ulong CreationTimeMilliseconds,
        ulong TimeToLiveMilliseconds,
        uint? Nonce);

    internal static IReadOnlyList<Fixture> LoadFixtures()
    {
        var directory = Path.Combine(AppContext.BaseDirectory, "Fixtures", "norito_rpc");
        using var dense = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(directory, "transaction_fixtures.dense.json")));
        using var manifest = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(directory, "transaction_fixtures.manifest.json")));

        var twins = manifest.RootElement.GetProperty("fixtures").EnumerateArray()
            .ToDictionary(static fixture => fixture.GetProperty("name").GetString()!, StringComparer.Ordinal);
        var fixtures = new List<Fixture>();
        foreach (var fixture in dense.RootElement.GetProperty("fixtures").EnumerateArray())
        {
            var name = fixture.GetProperty("name").GetString()!;
            Assert.True(twins.TryGetValue(name, out var twin), $"{name}: no current-layout twin");
            var nonce = twin.GetProperty("nonce");
            fixtures.Add(new Fixture(
                name,
                DecodeCanonicalBase64(fixture, "payload_base64", name),
                DecodeCanonicalBase64(fixture, "signed_base64", name),
                RequireLowerHex64(fixture, "signing_hash", name),
                DecodeCanonicalBase64(twin, "payload_base64", name),
                DecodeCanonicalBase64(twin, "signed_base64", name),
                twin.GetProperty("network_id").GetString()!,
                twin.GetProperty("authority").GetString()!,
                twin.GetProperty("creation_time_ms").GetUInt64(),
                twin.GetProperty("time_to_live_ms").GetUInt64(),
                nonce.ValueKind == JsonValueKind.Null ? null : nonce.GetUInt32()));
        }

        return fixtures;
    }

    private static string Describe(WireAccountController controller) => controller switch
    {
        WireSingleKeyController single => $"single:{Convert.ToHexStringLower(single.TaggedPublicKey)}",
        WireMultisigController multisig =>
            $"multisig:{multisig.Version}:{multisig.Threshold}:"
            + string.Join(
                ",",
                multisig.Members.Select(
                    static member => $"{Convert.ToHexStringLower(member.TaggedPublicKey)}*{member.Weight}")),
        _ => throw new InvalidOperationException("unknown controller"),
    };

    private static string Describe(WireFeePayment feePayment)
    {
        var limits = string.Join(
            ",",
            feePayment.ChargeLimits.Select(static limit =>
                $"{limit.Kind}:{Convert.ToHexStringLower(limit.AssetDefinitionId)}:{limit.MaxAmount.Mantissa}e-{limit.MaxAmount.Scale}"));
        var payer = feePayment switch
        {
            WireAuthorityFeePayment => "authority",
            WireSponsorFeePayment sponsor =>
                $"sponsor:{Describe(sponsor.Sponsor)}:{sponsor.ProgramName}:{sponsor.ProgramRevision}",
            _ => throw new InvalidOperationException("unknown fee payment"),
        };
        return $"{payer}|{limits}|gas={feePayment.GasLimit?.ToString() ?? "none"}";
    }

    // The executable's shape with every layout-independent value; instruction payloads are
    // returned separately because their bytes belong to the layout.
    private static string DescribeExecutable(WireExecutable executable, out List<WireInstruction> instructions)
    {
        instructions = [];
        switch (executable)
        {
            case WireInstructionsExecutable list:
                instructions.AddRange(list.Instructions);
                return $"instructions:{string.Join(",", list.Instructions.Select(static item => item.WireId))}";
            case WireContractCallExecutable call:
                return $"call:{Describe(call.Invocation)}";
            case WireIvmExecutable ivm:
                return $"ivm:{Convert.ToHexStringLower(ivm.Bytecode)}";
            case WireBatchExecutable batch:
                var parts = new List<string>();
                foreach (var item in batch.Items)
                {
                    switch (item)
                    {
                        case WireBatchInstruction instruction:
                            instructions.Add(instruction.Instruction);
                            parts.Add($"instruction:{instruction.Instruction.WireId}");
                            break;
                        case WireBatchContractCall call:
                            parts.Add($"call:{Describe(call.Invocation)}");
                            break;
                        default:
                            throw new InvalidOperationException("unknown batch item");
                    }
                }

                return $"batch:{string.Join(",", parts)}";
            default:
                throw new InvalidOperationException("unknown executable");
        }
    }

    private static string Describe(WireContractInvocation invocation) =>
        $"{invocation.ContractAddress}:{Convert.ToHexStringLower(invocation.ExpectedCodeHash)}:{invocation.Entrypoint}:"
        + (invocation.Arguments is { } arguments ? Convert.ToHexStringLower(arguments) : "none");

    private static void RequireExactProperties(JsonElement value, IReadOnlySet<string> expected, string context)
    {
        Assert.Equal(JsonValueKind.Object, value.ValueKind);
        var actual = value.EnumerateObject().Select(static property => property.Name).ToArray();
        Assert.Equal(actual.Length, actual.Distinct(StringComparer.Ordinal).Count());
        Assert.True(
            expected.SetEquals(actual),
            $"{context} has fields [{string.Join(',', actual.Order(StringComparer.Ordinal))}]");
    }

    private static byte[] DecodeCanonicalBase64(JsonElement value, string field, string name)
    {
        var text = value.GetProperty(field).GetString()
            ?? throw new InvalidDataException($"{name}.{field} must be a string");
        var decoded = Convert.FromBase64String(text);
        Assert.Equal(text, Convert.ToBase64String(decoded));
        Assert.NotEmpty(decoded);
        return decoded;
    }

    private static string RequireLowerHex64(JsonElement value, string field, string name)
    {
        var text = value.GetProperty(field).GetString()
            ?? throw new InvalidDataException($"{name}.{field} must be a string");
        Assert.True(
            text.Length == 64 && text.All(static character => character is >= '0' and <= '9' or >= 'a' and <= 'f'),
            $"{name}.{field} must be 32-byte lowercase hex");
        return text;
    }
}
