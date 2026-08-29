using System.Buffers.Binary;
using System.Linq;
using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Sccp;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class SccpExactTests
{
    private const ulong DefaultTransactionTimeToLiveMilliseconds = 100_000;
    private static readonly string MessageId = SccpV1.LowerHex(SccpV1.MessageId(BundleLane(), ExactTransfer()));
    private static readonly FeePaymentIntent BridgeFeePayment = FeePaymentIntent.Authority([]);

    [Fact]
    public void ClosedInventoryContainsOnlyFirstReleaseNetworksAndCodecs()
    {
        Assert.Equal(
        new[]
        {
            SccpNetworkV1.SoraTaira,
            SccpNetworkV1.EthereumMainnet,
            SccpNetworkV1.BscMainnet,
            SccpNetworkV1.TronMainnet,
            SccpNetworkV1.TonMainnet,
        }, Enum.GetValues<SccpNetworkV1>());
        Assert.Equal(
            new[]
            {
                SccpCodecV1.CanonicalText, SccpCodecV1.EvmAddress20,
                SccpCodecV1.TronAddress21, SccpCodecV1.TonAccount36,
            },
            Enum.GetValues<SccpCodecV1>());
        Assert.Equal(new byte[] { 0x40, 0x41, 0x42, 0x43, 0x44 },
            Enum.GetValues<SccpNetworkV1>().Select(static value => (byte)value));
        for (var tag = 0; tag <= byte.MaxValue; tag++)
        {
            Assert.Equal(tag is >= 0x40 and <= 0x44, Enum.IsDefined((SccpNetworkV1)tag));
        }
        Assert.False(Enum.IsDefined((SccpCodecV1)3));
        Assert.False(Enum.IsDefined((SccpCodecV1)4));
        Assert.False(Enum.IsDefined((SccpCodecV1)6));
        Assert.Equal(new[] { SccpPayloadKindV1.Transfer }, Enum.GetValues<SccpPayloadKindV1>());
        foreach (var alias in new[]
        {
            "sora-nexus", "sora_nexus", "eth-mainnet", "Ethereum-mainnet", "ethereum_mainnet", " ethereum-mainnet",
            "solana-mainnet-beta", "ton_mainnet", "Ton-mainnet", "tron-testnet",
        })
        {
            Assert.Throws<ArgumentException>(() => SccpNetworkV1Extensions.ParseProfileKey(alias));
        }

        Assert.Throws<ArgumentException>(() => new SccpLaneIdV1(
            SccpNetworkV1.EthereumMainnet,
            SccpNetworkV1.BscMainnet));
        Assert.Throws<ArgumentException>(() => new SccpLaneIdV1(
            SccpNetworkV1.SoraTaira,
            SccpNetworkV1.SoraTaira));
    }

    [Fact]
    public void CodecsAndSourceRolesRejectAliasesAndCollisions()
    {
        Assert.Equal("merchant@taira"u8.ToArray(), SccpCodecV1.CanonicalText.Validate("merchant@taira"u8));
        var canonicalI105 = Ed25519KeyPair
            .FromSeed(Enumerable.Repeat((byte)0x91, 32).ToArray())
            .ToAccountAddress()
            .ToI105();
        Assert.False(canonicalI105.All(static ch => ch <= 0x7f));
        var canonicalI105Bytes = Encoding.UTF8.GetBytes(canonicalI105);
        Assert.Equal(canonicalI105Bytes, SccpCodecV1.CanonicalText.Validate(canonicalI105Bytes));
        var inbound = new SccpTransferPayloadV1(
            2, 0, 3, 1, 2,
            SccpCodecV1.EvmAddress20, Enumerable.Repeat((byte)0x21, 20).ToArray(), 1,
            SccpCodecV1.EvmAddress20, Enumerable.Repeat((byte)0x22, 20).ToArray(),
            SccpCodecV1.CanonicalText, canonicalI105Bytes,
            SccpCodecV1.CanonicalText, "bsc_taira_xor"u8.ToArray());
        Assert.Equal(canonicalI105Bytes, SccpV1.DecodeCanonicalPayload(inbound.CanonicalBytes()).Recipient);
        var checksumMutation = canonicalI105[..^1]
            + (canonicalI105[^1] == '1' ? "2" : "1");
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate([]));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate(" padded"u8));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate(Encoding.UTF8.GetBytes(checksumMutation)));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate(Encoding.UTF8.GetBytes("ｲ")));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate("two words"u8));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate("line\nbreak"u8));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate([0xff]));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.CanonicalText.Validate(Enumerable.Repeat((byte)'a', 257).ToArray()));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.EvmAddress20.Validate(new byte[20]));
        var tron = Enumerable.Repeat((byte)1, 21).ToArray();
        tron[0] = 0x41;
        Assert.Equal(21, SccpCodecV1.TronAddress21.Validate(tron).Length);
        tron[0] = 0x42;
        Assert.Throws<ArgumentException>(() => SccpCodecV1.TronAddress21.Validate(tron));
        var ton = Enumerable.Repeat((byte)0x33, 36).ToArray();
        ton.AsSpan(0, 4).Clear();
        Assert.Equal(36, SccpCodecV1.TonAccount36.Validate(ton).Length);
        ton[3] = 1;
        Assert.Throws<ArgumentException>(() => SccpCodecV1.TonAccount36.Validate(ton));
        Assert.Throws<ArgumentException>(() => SccpCodecV1.TonAccount36.Validate(new byte[36]));
        Assert.Throws<ArgumentException>(() => new SccpSourceEmitterV1.Evm(
            Enumerable.Repeat((byte)1, 20).ToArray(),
            Enumerable.Repeat((byte)2, 32).ToArray(),
            Enumerable.Repeat((byte)2, 32).ToArray()));
        Assert.Equal("ton_masterchain_v1", SccpNativeBackendV1.TonMasterchain.WireKey());
        Assert.True(SccpNativeBackendV1.TonMasterchain.Supports(SccpNetworkV1.TonMainnet));
    }

    [Fact]
    public void TonMainnetIdentityCommitsCanonicalGlobalAndZeroStateMaterial()
    {
        var mainnet = SccpV1.CanonicalNetworkBytes(SccpNetworkV1.TonMainnet);
        Assert.Equal(90, mainnet.Length);
        Assert.Equal(1, mainnet[0]);
        Assert.Equal(0x44, mainnet[1]);
        Assert.Equal(4U, BinaryPrimitives.ReadUInt32LittleEndian(mainnet.AsSpan(2, 4)));
        Assert.Equal(-239, BinaryPrimitives.ReadInt32LittleEndian(mainnet.AsSpan(6, 4)));
        Assert.Equal(-1, BinaryPrimitives.ReadInt32LittleEndian(mainnet.AsSpan(10, 4)));
        Assert.Equal(0x8000_0000_0000_0000UL, BinaryPrimitives.ReadUInt64LittleEndian(mainnet.AsSpan(14, 8)));
    }

    [Fact]
    public void CanonicalTransferCommitsRouteRevisionAndRejectsEveryNonCanonicalBoundary()
    {
        var payload = new SccpTransferPayloadV1(
            0, 2, 7, 0x0102_0304, 0,
            SccpCodecV1.CanonicalText, "xor"u8.ToArray(), 1000,
            SccpCodecV1.CanonicalText, "alice@taira"u8.ToArray(),
            SccpCodecV1.EvmAddress20, Enumerable.Repeat((byte)0x11, 20).ToArray(),
            SccpCodecV1.CanonicalText, "taira_bsc_xor"u8.ToArray());
        var bytes = payload.CanonicalBytes();
        Assert.Equal(0x0102_0304U, BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(18, 4)));
        var decoded = SccpV1.DecodeCanonicalPayload(bytes);
        Assert.Equal(payload.RouteRevision, decoded.RouteRevision);
        Assert.Equal(bytes, decoded.CanonicalBytes());

        var leaked = decoded.AssetId;
        leaked[0] ^= 0xff;
        Assert.Equal(bytes, decoded.CanonicalBytes());

        foreach (var length in new[] { 0, 1, 2, 17, 21, 30, bytes.Length - 1 })
        {
            Assert.ThrowsAny<ArgumentException>(() => SccpV1.DecodeCanonicalPayload(bytes.AsSpan(0, length)));
        }

        foreach (var mutation in new Action<byte[]>[]
        {
            value => value[0] = 1,
            value => value[1] = 2,
            value => BinaryPrimitives.WriteUInt32LittleEndian(value.AsSpan(2, 4), 3),
            value => BinaryPrimitives.WriteUInt32LittleEndian(value.AsSpan(18, 4), 0),
            value => value[26] = 3,
            value => BinaryPrimitives.WriteUInt32LittleEndian(value.AsSpan(27, 4), uint.MaxValue),
            value => value.AsSpan(34, 16).Clear(),
            value => value[50] = (byte)SccpCodecV1.EvmAddress20,
        })
        {
            var malformed = bytes.ToArray();
            mutation(malformed);
            Assert.ThrowsAny<ArgumentException>(() => SccpV1.DecodeCanonicalPayload(malformed));
        }

        Assert.Throws<ArgumentException>(() => SccpV1.DecodeCanonicalPayload(bytes.Concat([(byte)0]).ToArray()));
        Assert.Throws<ArgumentOutOfRangeException>(() => new SccpTransferPayloadV1(
            0, 2, 7, 0, 0,
            SccpCodecV1.CanonicalText, "xor"u8.ToArray(), 1000,
            SccpCodecV1.CanonicalText, "alice@taira"u8.ToArray(),
            SccpCodecV1.EvmAddress20, Enumerable.Repeat((byte)0x11, 20).ToArray(),
            SccpCodecV1.CanonicalText, "taira_bsc_xor"u8.ToArray()));
    }

    [Fact]
    public void CanonicalCommitmentRejectsReservedTagsTrailingBytesAndRoleAliases()
    {
        var binding = Enumerable.Repeat((byte)0x71, 32).ToArray();
        var configuration = Enumerable.Repeat((byte)0x72, 32).ToArray();
        var context = new SccpOutboundMessageContextV1(BundleLane(), binding, configuration);
        binding[0] ^= 0xff;
        configuration[0] ^= 0xff;
        var commitment = SccpV1.Commitment(context, ExactTransfer());
        var bytes = SccpV1.CanonicalCommitmentBytes(commitment);
        var decoded = SccpV1.DecodeCanonicalCommitment(bytes);
        Assert.Equal(bytes, SccpV1.CanonicalCommitmentBytes(decoded));
        Assert.Equal(SccpV1.CommitmentRoot(commitment), SccpV1.MerkleRootFromCommitment(commitment, []));

        foreach (var mutation in new Action<byte[]>[]
        {
            value => value[0] = 2,
            value => value[1] = 4,
            value => value[2] = 0,
            value => value[2] = 6,
            value => value[3] = 8,
            value => value.AsSpan(4, 32).CopyTo(value.AsSpan(36, 32)),
            value => value.AsSpan(68, 32).CopyTo(value.AsSpan(100, 32)),
        })
        {
            var malformed = bytes.ToArray();
            mutation(malformed);
            Assert.Throws<ArgumentException>(() => SccpV1.DecodeCanonicalCommitment(malformed));
        }

        Assert.Throws<ArgumentException>(() => SccpV1.DecodeCanonicalCommitment(bytes.Concat([(byte)0]).ToArray()));
        Assert.Throws<ArgumentException>(() => new SccpOutboundMessageContextV1(
            BundleLane(),
            Enumerable.Repeat((byte)1, 32).ToArray(),
            Enumerable.Repeat((byte)1, 32).ToArray()));
        Assert.Throws<ArgumentException>(() => SccpV1.MerkleRootFromCommitment(
            commitment,
            Enumerable.Range(0, 65)
                .Select(index => new SccpMerkleStepV1(Enumerable.Repeat((byte)(index + 1), 32).ToArray(), false))
                .ToArray()));
    }

    [Fact]
    public void CanonicalBundleBytesRejectNestedLengthDirectionPayloadAndTrailingTampering()
    {
        var context = new SccpOutboundMessageContextV1(
            BundleLane(),
            Enumerable.Repeat((byte)0x71, 32).ToArray(),
            Enumerable.Repeat((byte)0x72, 32).ToArray());
        var payload = ExactTransfer();
        var commitment = SccpV1.Commitment(context, payload);
        var step = new SccpMerkleStepV1(Enumerable.Repeat((byte)0x55, 32).ToArray(), true);
        var bytes = SccpV1.CanonicalMessageBundleBytes(commitment, payload, [step], FinalityProofBytes());
        var decoded = SccpV1.DecodeCanonicalMessageBundle(bytes);
        Assert.Equal(1U, decoded.Payload.RouteRevision);
        Assert.Single(decoded.MerkleProof);

        var commitmentLength = checked((int)BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(33, 4)));
        var proofLengthOffset = 37 + commitmentLength;
        var proofLength = checked((int)BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(proofLengthOffset, 4)));
        var proofOffset = proofLengthOffset + 4;
        var payloadLengthOffset = proofOffset + proofLength;
        var payloadOffset = payloadLengthOffset + 4;
        foreach (var mutation in new Action<byte[]>[]
        {
            value => value[0] = 2,
            value => value[1] ^= 1,
            value => BinaryPrimitives.WriteUInt32LittleEndian(value.AsSpan(33, 4), uint.MaxValue),
            value => BinaryPrimitives.WriteUInt32LittleEndian(value.AsSpan(proofOffset, 4), 65),
            value => value[proofOffset + 36] = 2,
            value => BinaryPrimitives.WriteUInt32LittleEndian(value.AsSpan(payloadOffset + 18, 4), 2),
        })
        {
            var malformed = bytes.ToArray();
            mutation(malformed);
            Assert.ThrowsAny<ArgumentException>(() => SccpV1.DecodeCanonicalMessageBundle(malformed));
        }

        Assert.Throws<ArgumentException>(() => SccpV1.DecodeCanonicalMessageBundle(bytes.Concat([(byte)0]).ToArray()));
    }

    [Fact]
    public void SubmitDtosContainOnlyClosedArtifactFields()
    {
        var pair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)0x57, 32).ToArray());
        var authority = pair.ToAccountAddress().ToI105(AccountAddress.TestChainDiscriminant);
        var artifact = Convert.ToBase64String(NoritoCodec.Encode(
            SccpSubmitValidation.DestinationArtifactSchemaName,
            [1],
            flags: 0x02));
        var nativeArtifact = Convert.ToBase64String(NoritoCodec.Encode(
            SccpSubmitValidation.NativeInboundProofSchemaName,
            [1]));
        var unrelatedArtifact = Convert.ToBase64String(NoritoCodec.Encode("iroha.test.Unrelated", [1]));
        var transaction = CanonicalTransactionPayload(7, destinationProof: true);
        var signature = Convert.ToBase64String(
            Ed25519Signer.Sign(IrohaHash.Hash(transaction), pair.PrivateKeySeed));
        var gasBoundIntent = FeePaymentIntent.Authority([], gasLimit: 9);
        var gasBoundTransaction = CanonicalTransactionPayload(
            7,
            destinationProof: true,
            feePayment: gasBoundIntent);
        var gasBoundSignature = Convert.ToBase64String(
            Ed25519Signer.Sign(IrohaHash.Hash(gasBoundTransaction), pair.PrivateKeySeed));
        _ = new SccpBridgeProofSubmitRequest(
            authority,
            artifact,
            gasBoundIntent,
            gasBoundSignature,
            Convert.ToBase64String(gasBoundTransaction),
            creationTimeMs: 7);
        Assert.Throws<ArgumentException>(() => new SccpBridgeProofSubmitRequest(
            authority,
            artifact,
            FeePaymentIntent.Authority([], gasLimit: 10),
            gasBoundSignature,
            Convert.ToBase64String(gasBoundTransaction),
            creationTimeMs: 7));
        var request = BridgeProofRequest(
            authority,
            artifact,
            signature,
            Convert.ToBase64String(transaction),
            creationTimeMs: 7);
        foreach (var invalidTimeToLiveMilliseconds in new ulong?[] { null, 99_999 })
        {
            var invalidTransaction = CanonicalTransactionPayload(
                7,
                destinationProof: true,
                timeToLiveMilliseconds: invalidTimeToLiveMilliseconds);
            Assert.Throws<ArgumentException>(() => BridgeProofRequest(
                authority,
                artifact,
                Convert.ToBase64String(Ed25519Signer.Sign(
                    IrohaHash.Hash(invalidTransaction),
                    pair.PrivateKeySeed)),
                Convert.ToBase64String(invalidTransaction),
                creationTimeMs: 7));
        }
        Assert.Equal(nativeArtifact, BridgeMessageRequest(authority, nativeArtifact).NativeProofBase64);
        Assert.Equal(
            ReplayWitnessArtifact(),
            BridgeMessageRequest(authority, nativeArtifact).ReplayWitnessBase64);
        var membershipWitness = Convert.ToBase64String(NoritoCodec.Encode(
            SccpSubmitValidation.ReplayWitnessSchemaName,
            Concat(
                CompactField(SccpReplayV1.EmptyHashes()[SccpReplayV1.Depth]),
                CompactField(Enumerable.Repeat((byte)0x5a, 32).ToArray()),
                CompactField(new byte[32]),
                CompactField(UInt64(0))),
            flags: 0x02));
        Assert.Throws<ArgumentException>(() => new SccpBridgeMessageSubmitRequest(
            authority,
            nativeArtifact,
            membershipWitness,
            BridgeFeePayment));
        foreach (var (nativeBackendTag, accepted) in new[] { (3U, true), (4U, false) })
        {
            var taggedPayload = CanonicalTransactionPayload(
                7,
                nativeBackendTag: nativeBackendTag);
            var taggedSignature = Convert.ToBase64String(Ed25519Signer.Sign(
                IrohaHash.Hash(taggedPayload),
                pair.PrivateKeySeed));
            var create = () => BridgeMessageRequest(
                authority,
                nativeArtifact,
                taggedSignature,
                Convert.ToBase64String(taggedPayload),
                creationTimeMs: 7);
            if (accepted)
            {
                _ = create();
            }
            else
            {
                Assert.Throws<ArgumentException>(create);
            }
        }
        using var json = JsonDocument.Parse(JsonSerializer.SerializeToUtf8Bytes(request));
        var fields = json.RootElement.EnumerateObject()
            .Select(static property => property.Name)
            .ToHashSet(StringComparer.Ordinal);
        Assert.True(fields.SetEquals(
            [
                "authority",
                "signature_b64",
                "transaction_payload_b64",
                "fee_payment",
                "destination_proof_b64",
                "creation_time_ms",
            ]));
        var prepared = BridgeProofRequest(
            authority,
            artifact,
            creationTimeMs: 7);
        using var preparedJson = JsonDocument.Parse(JsonSerializer.SerializeToUtf8Bytes(prepared));
        Assert.False(preparedJson.RootElement.TryGetProperty("signature_b64", out _));
        Assert.False(preparedJson.RootElement.TryGetProperty("transaction_payload_b64", out _));
        foreach (var retired in new[]
        {
            "public_key_hex", "message_bundle_b64", "network_id_hex", "proof_bytes_hex", "allow_unready",
        })
        {
            Assert.False(json.RootElement.TryGetProperty(retired, out _));
        }

        Assert.Throws<ArgumentException>(() => BridgeProofRequest(authority, "AQ=="));
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            pair.ToAccountAddress().ToI105(AccountAddress.DefaultChainDiscriminant),
            artifact));
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(authority, nativeArtifact));
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(authority, unrelatedArtifact));
        Assert.Throws<ArgumentException>(() => BridgeMessageRequest(authority, artifact));
        Assert.Throws<ArgumentException>(() => BridgeMessageRequest(authority, unrelatedArtifact));
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            artifact,
            signatureBase64: "AQ=="));
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            artifact,
            transactionPayloadBase64: Convert.ToBase64String(transaction)));
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            artifact,
            signature,
            Convert.ToBase64String(transaction),
            creationTimeMs: null));
        var nativePayload = CanonicalTransactionPayload(7);
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            artifact,
            Convert.ToBase64String(Ed25519Signer.Sign(
                IrohaHash.Hash(nativePayload),
                pair.PrivateKeySeed)),
            Convert.ToBase64String(nativePayload),
            creationTimeMs: 7));
        var archivedChainPayload = CanonicalTransactionPayload(
            7,
            destinationProof: true,
            chainId: "809574f5-fee7-5e69-bfcf-52451e42d50f");
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            artifact,
            Convert.ToBase64String(Ed25519Signer.Sign(
                IrohaHash.Hash(archivedChainPayload),
                pair.PrivateKeySeed)),
            Convert.ToBase64String(archivedChainPayload),
            creationTimeMs: 7));
        var legacyPayload = CanonicalTransactionPayload(
            7,
            legacyOuterBinding: true,
            destinationProof: true);
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            artifact,
            Convert.ToBase64String(Ed25519Signer.Sign(
                IrohaHash.Hash(legacyPayload),
                pair.PrivateKeySeed)),
            Convert.ToBase64String(legacyPayload),
            creationTimeMs: 7));
        foreach (var payloadKind in new uint[] { 0, 1, uint.MaxValue })
        {
            var invalidKindPayload = CanonicalTransactionPayload(
                7,
                destinationProof: true,
                payloadKindOverride: payloadKind);
            Assert.Throws<ArgumentException>(() => BridgeProofRequest(
                authority,
                artifact,
                Convert.ToBase64String(Ed25519Signer.Sign(
                    IrohaHash.Hash(invalidKindPayload),
                    pair.PrivateKeySeed)),
                Convert.ToBase64String(invalidKindPayload),
                creationTimeMs: 7));
        }
        var truncatedPayload = transaction[..^1];
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            artifact,
            Convert.ToBase64String(Ed25519Signer.Sign(
                IrohaHash.Hash(truncatedPayload),
                pair.PrivateKeySeed)),
            Convert.ToBase64String(truncatedPayload),
            creationTimeMs: 7));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            BridgeMessageRequest(authority, nativeArtifact, creationTimeMs: 0));
    }

    [Fact]
    public void SubmitArtifactsRejectReservedNoritoFlagsPaddingAndAllSmallOrderSignatures()
    {
        Assert.Equal(
            16 * 1024 * 1024 + 64 * 1024,
            SccpSubmitValidation.MaximumGroth16ArtifactBytes);
        Assert.Equal(
            16 * 1024 * 1024 + 128 * 1024,
            SccpSubmitValidation.MaximumDestinationArtifactBytes);
        Assert.Equal(22_544_384, SccpSubmitValidation.MaximumDestinationArtifactBase64Bytes);
        Assert.Equal(
            "iroha_data_model::bridge::BridgeSccpDestinationProofV1",
            SccpSubmitValidation.DestinationArtifactSchemaName);
        var pair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)7, 32).ToArray());
        var authority = pair.ToAccountAddress().ToI105(AccountAddress.TestChainDiscriminant);
        var archive = NoritoCodec.Encode(SccpSubmitValidation.DestinationArtifactSchemaName, [1]);
        var legacyBn254Artifact = NoritoCodec.Encode(
            "iroha_sccp::SccpGroth16Bn254ProofArtifactV1",
            [1]);
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            Convert.ToBase64String(legacyBn254Artifact)));

        foreach (var mutation in new Func<byte[], byte[]>[]
        {
            value => { value[39] = 0x80; return value; },
            value => { value.AsSpan(6, 16).Clear(); return value; },
            value => value.Concat([(byte)0]).ToArray(),
            value => value[..NoritoHeader.EncodedLength].Concat(new byte[8]).Concat(value[NoritoHeader.EncodedLength..]).ToArray(),
            value => { value[31] ^= 1; return value; },
        })
        {
            var malformed = mutation(archive.ToArray());
            Assert.Throws<ArgumentException>(() => BridgeProofRequest(
                authority,
                Convert.ToBase64String(malformed)));
        }

        var canonical = Convert.ToBase64String(archive);
        Assert.Throws<ArgumentException>(() => BridgeProofRequest(
            authority,
            canonical.TrimEnd('=')));
        Assert.Throws<ArgumentException>(() => SccpSubmitValidation.CanonicalBase64(
            "QUFBQQ==",
            "bounded",
            maximumBytes: 3));

        string[] smallOrderPoints =
        [
            new string('0', 64),
            new string('0', 62) + "80",
            "01" + new string('0', 62),
            "01" + new string('0', 60) + "80",
            "ec" + new string('f', 60) + "7f",
            "ec" + new string('f', 62),
            "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05",
            "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a",
            "13888ecb61c5c95739d95c69ce5177c450e99128e7a90b3ecbc595e035c15500",
            "b4dfc53e58080246839b2c4e6f3db63e185f6c730b31e990b6f3f2519295550f",
        ];
        foreach (var point in smallOrderPoints)
        {
            var signature = new byte[64];
            Convert.FromHexString(point).CopyTo(signature, 0);
            signature[32] = 1;
            Assert.Throws<ArgumentException>(() => BridgeProofRequest(
                authority,
                canonical,
                Convert.ToBase64String(signature),
                Convert.ToBase64String(CanonicalTransactionPayload(7, destinationProof: true)),
                creationTimeMs: 7));
        }
    }

    [Fact]
    public async Task ToriiProofRequestNoritoAcceptsOnlyTheTwoConcreteCurveTypes()
    {
        foreach (var schemaName in SccpSubmitValidation.ProofRequestSchemaNames)
        {
            var frame = NoritoCodec.Encode(schemaName, [1]);
            string? observedAccept = null;
            var handler = new StubHandler(request =>
            {
                observedAccept = request.Headers.Accept.Single().MediaType;
                return new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new ByteArrayContent(frame),
                }.WithNoritoContentType();
            });
            using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));
            Assert.Equal(
                frame,
                await client.GetSccpProofRequestNoritoAsync(
                    MessageId,
                    TestContext.Current.CancellationToken));
            Assert.Equal("application/x-norito", observedAccept);
        }

        var unknown = NoritoCodec.Encode("example::UnknownProofRequestV1", [1]);
        var unknownHandler = new StubHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new ByteArrayContent(unknown),
        }.WithNoritoContentType());
        using var unknownClient = new ToriiClient(
            new Uri("https://example.test"),
            new HttpClient(unknownHandler));
        await Assert.ThrowsAsync<InvalidDataException>(() =>
            unknownClient.GetSccpProofRequestNoritoAsync(
                MessageId,
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public void CapabilitiesUseOnlyFixedExactEndpoints()
    {
        var parsed = SccpCapabilities.Parse(CapabilitiesJson());
        Assert.Equal("/v1/sccp/registry", parsed.RegistryPath);
        Assert.Equal("/v1/sccp/proof-requests/{message_id}", parsed.ProofRequestPath);
        Assert.Equal(64U, parsed.RegistryLimits.MaxRetainedRoutesPerLane);
        Assert.Equal(4_096U, parsed.RegistryLimits.MaxRetainedNativeTrustAnchorsPerLane);
        Assert.Equal(512U, parsed.ResourceLimits.MaxOutboundMessagesPerBlock);
        Assert.Equal(4_096UL, parsed.ResourceLimits.MaxOutboundMessagePayloadBytes);
        Assert.Equal(65_536UL, parsed.ResourceLimits.MaxPendingOutboundMessages);
        Assert.Equal(256UL * 1024 * 1024, parsed.ResourceLimits.MaxPendingOutboundPayloadBytes);
        Assert.Equal(131_713U, parsed.ResourceLimits.MaxBlsSignerContributionsPerTransaction);
        var readOnly = CapabilitiesObject();
        readOnly.Remove("proof_submit_path");
        readOnly.Remove("native_message_submit_path");
        _ = SccpCapabilities.Parse(Json(readOnly));
        foreach (var mutation in new Action<Dictionary<string, object?>>[]
        {
            value => value["registry_path"] = "/v1/sccp/manifests",
            value => value["proof_request_path"] = "/v1/sccp/proof-requests/{message_id}?network=bsc",
            value => value["proof_artifact_path"] = "/v1/sccp/artifacts/message/{message_id}",
            value => value["proof_job_path"] = "/v1/sccp/jobs/message/{message_id}",
            value => value["outbound"] = new Dictionary<string, object?>(),
            value => value["registry_revision"] = "0x" + new string('0', 64),
            value => value.Remove("proof_submit_path"),
            value => value.Remove("native_message_submit_path"),
        })
        {
            var value = CapabilitiesObject();
            mutation(value);
            Assert.Throws<ArgumentException>(() => SccpCapabilities.Parse(Json(value)));
        }

        var resourceKeys = ((Dictionary<string, object?>)CapabilitiesObject()["resource_limits"]!)
            .Keys.ToArray();
        foreach (var field in resourceKeys)
        {
            var value = CapabilitiesObject();
            ((Dictionary<string, object?>)value["resource_limits"]!)[field] = 0;
            Assert.Throws<ArgumentException>(() => SccpCapabilities.Parse(Json(value)));
        }

        (string Lower, string Upper)[] orderingRelations =
        [
            ("max_proof_bytes_per_proof", "max_proof_bytes_per_transaction"),
            ("max_proofs_per_transaction", "max_proofs_per_block"),
            ("max_proof_bytes_per_transaction", "max_proof_bytes_per_block"),
            ("max_native_headers_per_transaction", "max_native_headers_per_block"),
            (
                "max_ethereum_light_client_updates_per_transaction",
                "max_ethereum_light_client_updates_per_block"
            ),
            (
                "max_native_header_bytes_per_transaction",
                "max_native_header_bytes_per_block"
            ),
            (
                "max_secp256k1_recoveries_per_transaction",
                "max_secp256k1_recoveries_per_block"
            ),
            (
                "max_bls_aggregate_checks_per_transaction",
                "max_bls_aggregate_checks_per_block"
            ),
            (
                "max_bls_signer_contributions_per_transaction",
                "max_bls_signer_contributions_per_block"
            ),
            (
                "max_ed25519_signature_checks_per_transaction",
                "max_ed25519_signature_checks_per_block"
            ),
            (
                "max_ed25519_validator_key_checks_per_transaction",
                "max_ed25519_validator_key_checks_per_block"
            ),
            (
                "max_bn254_pairing_checks_per_transaction",
                "max_bn254_pairing_checks_per_block"
            ),
            (
                "max_bls12_381_pairing_checks_per_transaction",
                "max_bls12_381_pairing_checks_per_block"
            ),
        ];
        foreach (var (lower, upper) in orderingRelations)
        {
            var reversed = CapabilitiesObject();
            var limits = (Dictionary<string, object?>)reversed["resource_limits"]!;
            limits[lower] = Convert.ToUInt64(
                limits[upper],
                System.Globalization.CultureInfo.InvariantCulture) + 1;
            Assert.Throws<ArgumentException>(() => SccpCapabilities.Parse(Json(reversed)));
        }

        var driftedRegistryLimits = CapabilitiesObject();
        ((Dictionary<string, object?>)driftedRegistryLimits["registry_limits"]!)[
            "max_retained_routes_per_lane"] = 65;
        Assert.Throws<ArgumentException>(
            () => SccpCapabilities.Parse(Json(driftedRegistryLimits)));

        foreach (var (field, value) in new (string Field, ulong Value)[]
        {
            ("max_outbound_messages_per_block", 511),
            ("max_outbound_messages_per_block", 513),
            ("max_outbound_message_payload_bytes", 4_095),
            ("max_outbound_message_payload_bytes", 4_097),
        })
        {
            var drifted = CapabilitiesObject();
            ((Dictionary<string, object?>)drifted["resource_limits"]!)[field] = value;
            Assert.ThrowsAny<ArgumentException>(() => SccpCapabilities.Parse(Json(drifted)));
        }

        var canonical = Encoding.UTF8.GetString(CapabilitiesJson());
        const string needle = "\"max_proofs_per_transaction\":1";
        Assert.Contains(needle, canonical, StringComparison.Ordinal);
        foreach (var token in new[] { "1.0", "1e0", "-0", "9007199254740992.5", "1e999" })
        {
            var hostile = canonical.Replace(
                needle,
                $"\"max_proofs_per_transaction\":{token}",
                StringComparison.Ordinal);
            Assert.Throws<ArgumentException>(() =>
                SccpCapabilities.Parse(Encoding.UTF8.GetBytes(hostile)));
        }

        const ulong jsonSafeMaximum = (1UL << 53) - 1;
        var boundary = CapabilitiesObject();
        var boundaryLimits = (Dictionary<string, object?>)boundary["resource_limits"]!;
        foreach (var field in new[]
        {
            "max_pending_outbound_messages", "max_pending_outbound_payload_bytes",
            "max_proof_bytes_per_proof", "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block", "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block",
        })
        {
            boundaryLimits[field] = jsonSafeMaximum;
        }
        Assert.Equal(
            jsonSafeMaximum,
            SccpCapabilities.Parse(Json(boundary)).ResourceLimits.MaxProofBytesPerBlock);
        boundaryLimits["max_proof_bytes_per_block"] = jsonSafeMaximum + 1;
        Assert.Throws<ArgumentException>(() => SccpCapabilities.Parse(Json(boundary)));
        foreach (var field in new[]
        {
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
        })
        {
            var oversized = CapabilitiesObject();
            ((Dictionary<string, object?>)oversized["resource_limits"]!)[field] =
                jsonSafeMaximum + 1;
            Assert.Throws<ArgumentException>(() => SccpCapabilities.Parse(Json(oversized)));
        }
    }

    [Fact]
    public void RegistryChecksRetainedHistoryCapsBeforeTraversal()
    {
        var exactAnchors = RegistryObject();
        GovernedLane(exactAnchors)["native_trust_anchors"] =
            Enumerable.Repeat<object?>(null, 4_096).ToArray();
        var exactAnchorError = Assert.Throws<ArgumentException>(
            () => SccpRegistryV1.Parse(Json(exactAnchors)));
        Assert.DoesNotContain("more than 4,096", exactAnchorError.Message);

        var overAnchors = RegistryObject();
        GovernedLane(overAnchors)["native_trust_anchors"] =
            Enumerable.Repeat<object?>(null, 4_097).ToArray();
        var overAnchorError = Assert.Throws<ArgumentException>(
            () => SccpRegistryV1.Parse(Json(overAnchors)));
        Assert.Contains("more than 4,096", overAnchorError.Message);

        var exactRoutes = RegistryObject();
        GovernedLane(exactRoutes)["routes"] = Enumerable.Range(0, 64)
            .Select(_ => (object)new Dictionary<string, object?>())
            .ToArray();
        var exactRouteError = Assert.Throws<ArgumentException>(
            () => SccpRegistryV1.Parse(Json(exactRoutes)));
        Assert.DoesNotContain("more than 64 retained", exactRouteError.Message);

        var overRoutes = RegistryObject();
        GovernedLane(overRoutes)["routes"] = Enumerable.Range(0, 65)
            .Select(_ => (object)new Dictionary<string, object?>())
            .ToArray();
        var overRouteError = Assert.Throws<ArgumentException>(
            () => SccpRegistryV1.Parse(Json(overRoutes)));
        Assert.Contains("more than 64 retained", overRouteError.Message);
    }

    [Fact]
    public void RegistryIsTypedAndRejectsRetiredManifestShapes()
    {
        var empty = SccpRegistryV1.Parse(Json(new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["lanes"] = Array.Empty<object>(),
        }));
        Assert.Empty(empty.Lanes);
        foreach (var invalid in new object[]
        {
            new Dictionary<string, object?> { ["version"] = 1, ["lanes"] = Array.Empty<object>(), ["manifests"] = Array.Empty<object>() },
            new Dictionary<string, object?> { ["version"] = 1, ["inbound_native_lanes"] = Array.Empty<object>(), ["outbound_destination_routes"] = Array.Empty<object>() },
            new Dictionary<string, object?> { ["version"] = 2, ["lanes"] = Array.Empty<object>() },
        })
        {
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(invalid)));
        }
    }

    [Fact]
    public void RegistryDeeplyValidatesGovernedRouteIdentityAndAdversarialMutations()
    {
        var valid = RegistryObject();
        var registry = SccpRegistryV1.Parse(Json(valid));
        var governedLane = Assert.Single(registry.Lanes);
        Assert.Empty(governedLane.NativeTrustAnchors);
        Assert.Null(governedLane.CurrentNativeTrustAnchorHash);
        var route = Assert.Single(governedLane.Routes);
        Assert.Equal("taira_bsc_xor", route.RouteId);
        Assert.Equal(1U, route.Revision);
        Assert.Equal(SccpRouteActivationV1.Staged, route.Activation);
        Assert.Equal((byte)1, route.Destination.OutboundProofPolicy.Version);
        Assert.Same(
            route.Destination.SemanticProofProfile,
            route.Destination.OutboundProofPolicy.SemanticProfile);
        Assert.Same(
            route.Destination.SoraFinalityAnchor,
            route.Destination.OutboundProofPolicy.SoraFinalityAnchor);
        Assert.Equal(Upper(0x24, 20), Convert.ToHexString(route.Destination.ReplayVerifierAddress));
        Assert.Equal(Upper(0x35, 32), Convert.ToHexString(route.Destination.ReplayVerifierCodeHash));
        Assert.Equal(Upper(0x25, 20), Convert.ToHexString(route.Destination.MintBreakerAddress));
        Assert.Equal(Upper(0x36, 32), Convert.ToHexString(route.Destination.MintBreakerCodeHash));
        Assert.Equal((byte)1, route.SoraOutboundExecutionPolicy.Version);
        Assert.Equal(
            "ivm_proved_record_sccp_message_v1",
            route.SoraOutboundExecutionPolicy.Semantics);
        Assert.Equal(1_000_000UL, route.SoraOutboundExecutionPolicy.GasLimit);
        Assert.Equal(
            "halo2/ipa",
            route.SoraOutboundExecutionPolicy.VerifyingKeyReference.Backend);
        Assert.Equal(
            "sccp_route_v1",
            route.SoraOutboundExecutionPolicy.VerifyingKeyReference.Name);
        Assert.Equal(1U, route.SoraOutboundExecutionPolicy.VerifyingKeyReference.Version);
        Assert.Equal(
            Upper(0x51, 32),
            Convert.ToHexString(route.SoraOutboundExecutionPolicy.ContractArtifactSha256));
        Assert.Equal(
            Upper(0x52, 32),
            Convert.ToHexString(route.SoraOutboundExecutionPolicy.VerifyingKeyReference.Commitment));

        var tron = SccpRegistryV1.Parse(Json(TronRegistryObject()));
        Assert.Equal("taira_tron_xor", Assert.Single(Assert.Single(tron.Lanes).Routes).RouteId);
        Assert.Equal(
            SccpDestinationProofBackendV1.TronGroth16Bn254,
            Assert.Single(Assert.Single(tron.Lanes).Routes).Destination.Family);
        Assert.Throws<ArgumentException>(() =>
            SccpRegistryV1.Parse(Json(TronRegistryObject(aliasBindingWithTokenCodeHash: true))));

        foreach (var source in new[] { SccpNetworkV1.BscMainnet, SccpNetworkV1.TronMainnet })
        {
            var baselineRegistry = source == SccpNetworkV1.TronMainnet
                ? TronRegistryObject()
                : RegistryObject();
            var baselineBinding = Assert.Single(
                Assert.Single(SccpRegistryV1.Parse(Json(baselineRegistry)).Lanes).Routes)
                .Destination.DestinationBindingHash;
            foreach (var (field, replacement) in new (string Field, string Replacement)[]
            {
                ("replay_verifier_address", Upper(0x28, 20)),
                ("replay_verifier_code_hash", Upper(0x38, 32)),
                ("mint_breaker_address", Upper(0x29, 20)),
                ("mint_breaker_code_hash", Upper(0x39, 32)),
            })
            {
                var candidate = DeepClone(baselineRegistry);
                ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(candidate)["destination"]!)["deployment"]!)[field] = replacement;
                RefreshRegistryRouteConfigurationHash(candidate, source);
                var changedBinding = Assert.Single(
                    Assert.Single(SccpRegistryV1.Parse(Json(candidate)).Lanes).Routes)
                    .Destination.DestinationBindingHash;
                Assert.False(
                    baselineBinding.AsSpan().SequenceEqual(changedBinding),
                    $"{source} destination binding did not commit {field}");
            }

            foreach (var fields in new[]
            {
                new[] { "replay_verifier_address", "mint_breaker_address" },
                new[] { "replay_verifier_code_hash", "mint_breaker_code_hash" },
                new[]
                {
                    "replay_verifier_address", "mint_breaker_address",
                    "replay_verifier_code_hash", "mint_breaker_code_hash",
                },
            })
            {
                var candidate = DeepClone(baselineRegistry);
                var deployment = (Dictionary<string, object?>)
                    ((Dictionary<string, object?>)Route(candidate)["destination"]!)["deployment"]!;
                for (var index = 0; index < fields.Length; index += 2)
                {
                    (deployment[fields[index]], deployment[fields[index + 1]]) =
                        (deployment[fields[index + 1]], deployment[fields[index]]);
                }

                Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(candidate)));
            }
        }

        var missingCutoff = DeepClone(valid);
        Route(missingCutoff).Remove("inbound_finality_cutoff");
        Assert.ThrowsAny<ArgumentException>(() => SccpRegistryV1.Parse(Json(missingCutoff)));

        var mutations = new Action<Dictionary<string, object?>>[]
        {
            value => ((Dictionary<string, object?>)Route(value)["source_identity"]!)["lane"] = Lane("ethereum-mainnet", "sora-taira"),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["source_identity"]!)["emitter"]!)["emitter"] = "tron",
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["source_identity"]!)["emitter"]!)["identity"]!)["route_config_hash"] = Upper(0x99, 32),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["route_address"] = Upper(0x21, 20),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["replay_verifier_address"] = Upper(0x21, 20),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["mint_breaker_address"] = Upper(0x22, 20),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["route_code_hash"] = Upper(0x32, 32),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["replay_verifier_code_hash"] = "C5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470",
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["verifier_key_hash"] = Upper(0x33, 32),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["max_wrapped_supply"] = "1000000000000000000001",
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["destination"]!)["deployment"]!)["taira_to_token_multiplier"] = 1_000_000_001,
            value => Route(value)["route_id"] = "Taira_Bsc_Xor",
            value => Route(value)["revision"] = 2,
            value => Route(value)["activation"] = new Dictionary<string, object?> { ["activation"] = "bidirectional", ["direction"] = null },
            value => Route(value).Remove("sora_outbound_execution_policy"),
            value => ((Dictionary<string, object?>)Route(value)["sora_outbound_execution_policy"]!)["semantics"] = "legacy_call_v0",
            value => ((Dictionary<string, object?>)Route(value)["sora_outbound_execution_policy"]!)["gas_limit"] = 0,
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["sora_outbound_execution_policy"]!)["vk_ref"]!)["backend"] = "Halo2/ipa",
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)Route(value)["sora_outbound_execution_policy"]!)["vk_ref"]!).Remove("version"),
            value => ((Dictionary<string, object?>)Route(value)["sora_outbound_execution_policy"]!)["legacy_gas_budget"] = 1,
            value =>
            {
                var executionPolicy = (Dictionary<string, object?>)Route(value)["sora_outbound_execution_policy"]!;
                ((Dictionary<string, object?>)executionPolicy["vk_ref"]!)["commitment"] =
                    executionPolicy["contract_artifact_sha256"];
            },
            value => ((Dictionary<string, object?>)Route(value)["settlement"]!)["payload_amount_scale"] = 8,
            value => ((Dictionary<string, object?>)Route(value)["settlement"]!)["asset_definition_id"] = "xor",
        };
        foreach (var mutation in mutations)
        {
            var candidate = DeepClone(valid);
            mutation(candidate);
            Assert.ThrowsAny<ArgumentException>(() => SccpRegistryV1.Parse(Json(candidate)));
        }

        var duplicateLane = DeepClone(valid);
        var lane = ((object[])duplicateLane["lanes"]!)[0];
        duplicateLane["lanes"] = new[] { lane, CloneValue(lane)! };
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(duplicateLane)));

        var tooManyLanes = DeepClone(valid);
        tooManyLanes["lanes"] = Enumerable.Repeat(lane, 17).ToArray();
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(tooManyLanes)));

        var reusedDeployment = DeepClone(valid);
        var laneObject = (Dictionary<string, object?>)((object[])reusedDeployment["lanes"]!)[0];
        var firstRoute = (Dictionary<string, object?>)((object[])laneObject["routes"]!)[0];
        var secondRoute = DeepClone(firstRoute);
        secondRoute["revision"] = 2;
        laneObject["routes"] = new object[] { firstRoute, secondRoute };
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(reusedDeployment)));
    }

    [Fact]
    public void RegistryParsesExactTonDeploymentAndRejectsCurveAddressAndProfileAliases()
    {
        var parsedLane = Assert.Single(SccpRegistryV1.Parse(Json(TonRegistryObject())).Lanes);
        var parsed = Assert.Single(parsedLane.Routes);
        Assert.Equal(SccpNetworkV1.TonMainnet, parsed.Lane.Source);
        Assert.Equal(SccpDestinationProofBackendV1.TonGroth16Bls12381, parsed.Destination.Family);
        Assert.Equal(SccpSemanticProofProfileKindV1.Groth16Bls12381, parsed.Destination.SemanticProofProfile.Kind);
        Assert.Equal((byte)1, parsed.Destination.OutboundProofPolicy.Version);
        Assert.Equal(0, parsed.Destination.TonRouteAddress!.Workchain);
        Assert.Equal(5, parsed.Destination.TonMintBreakerGuardianKeys!.Ordered.Count);
        Assert.Equal(
            Enumerable.Range(0, 5)
                .Select(index => Upper(checked((byte)(0xA1 + index)), 32)),
            new[]
            {
                Convert.ToHexString(parsed.Destination.TonMintBreakerGuardianKeys.Guardian0),
                Convert.ToHexString(parsed.Destination.TonMintBreakerGuardianKeys.Guardian1),
                Convert.ToHexString(parsed.Destination.TonMintBreakerGuardianKeys.Guardian2),
                Convert.ToHexString(parsed.Destination.TonMintBreakerGuardianKeys.Guardian3),
                Convert.ToHexString(parsed.Destination.TonMintBreakerGuardianKeys.Guardian4),
            });
        Assert.Equal((UInt128)1_000_000_000_000, parsed.Destination.MaxWrappedSupply);
        Assert.Equal(
            parsed.Destination.MaxWrappedSupply,
            parsed.Destination.RequireTonAmountWithinCap(parsed.Destination.MaxWrappedSupply));
        Assert.Throws<ArgumentOutOfRangeException>(() => parsed.Destination.RequireTonAmountWithinCap(0));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            parsed.Destination.RequireTonAmountWithinCap(parsed.Destination.MaxWrappedSupply + 1));
        Assert.Throws<InvalidOperationException>(() =>
            Assert.Single(Assert.Single(SccpRegistryV1.Parse(Json(RegistryObject())).Lanes).Routes)
                .Destination.RequireTonAmountWithinCap(1));
        Assert.IsType<SccpTonSourceEmitterV1>(parsed.SourceEmitter);

        var changedInitialData = TonRegistryObject();
        TonDeployment(changedInitialData)["jetton_master_initial_data_hash"] = Upper(0x38, 32);
        TonDeployment(changedInitialData)["route_initial_data_hash"] = Upper(0x39, 32);
        _ = SccpRegistryV1.Parse(Json(changedInitialData));

        var missingInitialData = TonRegistryObject();
        TonDeployment(missingInitialData).Remove("route_initial_data_hash");
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(missingInitialData)));

        var aliasedInitialData = TonRegistryObject();
        TonDeployment(aliasedInitialData)["route_initial_data_hash"] =
            TonDeployment(aliasedInitialData)["jetton_master_code_hash"];
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(aliasedInitialData)));

        var anchored = TonRegistryObject();
        var anchor = NativeTrustAnchor(0x91, 1);
        ((Dictionary<string, object?>)anchor["backend"]!)["backend"] = "ton_masterchain_v1";
        GovernedLane(anchored)["native_trust_anchors"] = new object[] { anchor };
        GovernedLane(anchored)["current_native_trust_anchor_hash"] = anchor["anchor_hash"];
        Route(anchored)["activation"] = Activation("inbound_only");
        Assert.Equal(
            SccpNativeBackendV1.TonMasterchain,
            Assert.Single(SccpRegistryV1.Parse(Json(anchored)).Lanes).NativeTrustAnchors[0].Backend);

        var wrongWorkchain = TonRegistryObject();
        TonDeployment(wrongWorkchain, "route_address")["workchain"] = -1;
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(wrongWorkchain)));

        var sameAddress = TonRegistryObject();
        TonDeployment(sameAddress)["route_address"] =
            CloneValue(TonDeployment(sameAddress)["jetton_master_address"]);
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(sameAddress)));

        var aliasedEmitterAddress = TonRegistryObject();
        TonEmitterIdentity(aliasedEmitterAddress)["address"] =
            CloneValue(TonDeployment(aliasedEmitterAddress)["jetton_master_address"]);
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(aliasedEmitterAddress)));

        var aliasedEmitterCode = TonRegistryObject();
        TonEmitterIdentity(aliasedEmitterCode)["code_hash"] =
            TonDeployment(aliasedEmitterCode)["jetton_master_code_hash"];
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(aliasedEmitterCode)));

        var uncompressedKey = TonRegistryObject();
        ((Dictionary<string, object?>)TonDeployment(uncompressedKey)["verifying_key"]!)["alpha1"] =
            Upper(0x01, 48);
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(uncompressedKey)));

        var wrongProofProfile = TonRegistryObject();
        TonDeployment(wrongProofProfile)["proof_profile_commitment"] = Upper(0x98, 32);
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(wrongProofProfile)));

        var wrongCircuit = TonRegistryObject();
        TonDeployment(wrongCircuit)["verifier_circuit_hash"] = Upper(0x98, 32);
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(wrongCircuit)));

        var zeroGuardian = TonRegistryObject();
        ((Dictionary<string, object?>)TonDeployment(zeroGuardian)["mint_breaker_guardian_keys"]!)["guardian_2"] = Upper(0, 32);
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(zeroGuardian)));

        var unsortedGuardians = TonRegistryObject();
        var guardianSet = (Dictionary<string, object?>)TonDeployment(unsortedGuardians)["mint_breaker_guardian_keys"]!;
        guardianSet["guardian_3"] = guardianSet["guardian_2"];
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(unsortedGuardians)));

        foreach (var invalidCap in new[] { "0", (UInt128.One << 120).ToString() })
        {
            var invalidSupply = TonRegistryObject();
            TonDeployment(invalidSupply)["max_wrapped_supply"] = invalidCap;
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(invalidSupply)));
        }

        var wrongEmitterShape = TonRegistryObject();
        var identity = TonEmitterIdentity(wrongEmitterShape);
        identity["runtime_code_hash"] = identity["code_hash"];
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(wrongEmitterShape)));
    }

    [Fact]
    public void RegistryTrustAnchorHistoryRejectsStalePointerDuplicateHashAndRollback()
    {
        var first = NativeTrustAnchor(0x91, 1);
        var second = NativeTrustAnchor(0x92, 2);
        var canonical = RegistryObject();
        var canonicalLane = GovernedLane(canonical);
        canonicalLane["native_trust_anchors"] = new object[] { first, second };
        canonicalLane["current_native_trust_anchor_hash"] = second["anchor_hash"];
        Route(canonical)["activation"] = Activation("inbound_only");

        var parsedLane = Assert.Single(SccpRegistryV1.Parse(Json(canonical)).Lanes);
        Assert.Equal(2, parsedLane.NativeTrustAnchors.Count);
        Assert.Equal(SccpNativeBackendV1.BscParlia, parsedLane.NativeTrustAnchors[1].Backend);
        Assert.Equal(2UL, parsedLane.NativeTrustAnchors[1].CheckpointHeight);
        Assert.NotNull(parsedLane.CurrentNativeTrustAnchorHash);
        Assert.True(parsedLane.NativeTrustAnchors[1].AnchorHash.AsSpan()
            .SequenceEqual(parsedLane.CurrentNativeTrustAnchorHash!));

        var retired = DeepClone(canonical);
        Route(retired)["activation"] = Activation("retired");
        Route(retired)["inbound_finality_cutoff"] = new Dictionary<string, object?>
        {
            ["trust_anchor_hash"] = first["anchor_hash"],
            ["max_anchor_interval_height"] = 2,
        };
        var parsedRetired = Assert.Single(SccpRegistryV1.Parse(Json(retired)).Lanes).Routes[0];
        var parsedCutoff = Assert.IsType<SccpInboundFinalityCutoffV1>(parsedRetired.InboundFinalityCutoff);
        Assert.Equal(2UL, parsedCutoff.MaxAnchorIntervalHeight);
        Assert.True(parsedCutoff.TrustAnchorHash.AsSpan()
            .SequenceEqual(Convert.FromHexString((string)first["anchor_hash"]!)));

        var nonRetiredCutoff = DeepClone(canonical);
        Route(nonRetiredCutoff)["inbound_finality_cutoff"] =
            CloneValue(Route(retired)["inbound_finality_cutoff"]);
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(nonRetiredCutoff)));

        var retiredWithoutCutoff = DeepClone(canonical);
        Route(retiredWithoutCutoff)["activation"] = Activation("retired");
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(retiredWithoutCutoff)));

        var openEndedCutoff = DeepClone(retired);
        Route(openEndedCutoff)["inbound_finality_cutoff"] = new Dictionary<string, object?>
        {
            ["trust_anchor_hash"] = second["anchor_hash"],
            ["max_anchor_interval_height"] = 3,
        };
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(openEndedCutoff)));

        var incompleteInterval = DeepClone(retired);
        ((Dictionary<string, object?>)Route(incompleteInterval)["inbound_finality_cutoff"]!)["max_anchor_interval_height"] = 1;
        Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(incompleteInterval)));

        var stalePointer = DeepClone(canonical);
        GovernedLane(stalePointer)["current_native_trust_anchor_hash"] = first["anchor_hash"];
        Assert.Contains(
            "last retained anchor",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(stalePointer))).Message,
            StringComparison.Ordinal);

        var duplicateHash = DeepClone(canonical);
        var duplicateAnchors = (object[])GovernedLane(duplicateHash)["native_trust_anchors"]!;
        ((Dictionary<string, object?>)duplicateAnchors[1])["anchor_hash"] = first["anchor_hash"];
        GovernedLane(duplicateHash)["current_native_trust_anchor_hash"] = first["anchor_hash"];
        Assert.Contains(
            "duplicate native trust-anchor hash",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(duplicateHash))).Message,
            StringComparison.Ordinal);

        var rollback = DeepClone(canonical);
        var rollbackAnchors = (object[])GovernedLane(rollback)["native_trust_anchors"]!;
        ((Dictionary<string, object?>)rollbackAnchors[1])["checkpoint_height"] = 1;
        Assert.Contains(
            "advance monotonically",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(rollback))).Message,
            StringComparison.Ordinal);
    }

    [Fact]
    public void RegistryTrustAnchorHistoryRejectsNullFamilyMismatchAndLegacySingularShape()
    {
        var first = NativeTrustAnchor(0x91, 1);
        var canonical = RegistryObject();
        var canonicalLane = GovernedLane(canonical);
        canonicalLane["native_trust_anchors"] = new object[] { first };
        canonicalLane["current_native_trust_anchor_hash"] = first["anchor_hash"];

        var nullAnchor = DeepClone(canonical);
        GovernedLane(nullAnchor)["native_trust_anchors"] = new object?[] { null };
        Assert.Contains(
            "must not be null",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(nullAnchor))).Message,
            StringComparison.Ordinal);

        var wrongFamily = DeepClone(canonical);
        var familyAnchors = (object[])GovernedLane(wrongFamily)["native_trust_anchors"]!;
        var backend = (Dictionary<string, object?>)((Dictionary<string, object?>)familyAnchors[0])["backend"]!;
        backend["backend"] = "ethereum_beacon_v1";
        Assert.Contains(
            "backend does not match its lane",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(wrongFamily))).Message,
            StringComparison.Ordinal);

        var missingPointer = DeepClone(canonical);
        GovernedLane(missingPointer)["current_native_trust_anchor_hash"] = null;
        Assert.Contains(
            "last retained anchor",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(missingPointer))).Message,
            StringComparison.Ordinal);

        var unexpectedPointer = RegistryObject();
        GovernedLane(unexpectedPointer)["current_native_trust_anchor_hash"] = first["anchor_hash"];
        Assert.Contains(
            "last retained anchor",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(unexpectedPointer))).Message,
            StringComparison.Ordinal);

        var legacy = DeepClone(canonical);
        var legacyLane = GovernedLane(legacy);
        legacyLane.Remove("native_trust_anchors");
        legacyLane.Remove("current_native_trust_anchor_hash");
        legacyLane["native_trust_anchor"] = first;
        Assert.Contains(
            "unknown or retired field `native_trust_anchor`",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(legacy))).Message,
            StringComparison.Ordinal);
    }

    [Fact]
    public void RegistryRouteCapacityCountsOnlyNonRetiredRevisions()
    {
        var historical = RegistryObject();
        var firstRoute = Route(historical);
        var firstAnchor = NativeTrustAnchor(0x91, 1);
        var secondAnchor = NativeTrustAnchor(0x92, 2);
        GovernedLane(historical)["native_trust_anchors"] = new object[] { firstAnchor, secondAnchor };
        GovernedLane(historical)["current_native_trust_anchor_hash"] = secondAnchor["anchor_hash"];
        GovernedLane(historical)["routes"] = Enumerable.Range(1, 10)
            .Select(revision => RouteRevision(
                firstRoute,
                checked((uint)revision),
                revision <= 8 ? "paused" : "retired"))
            .Cast<object>()
            .ToArray();

        var parsedRoutes = Assert.Single(SccpRegistryV1.Parse(Json(historical)).Lanes).Routes;
        Assert.Equal(10, parsedRoutes.Count);
        Assert.Equal(2, parsedRoutes.Count(static route => route.Activation == SccpRouteActivationV1.Retired));

        var tooManyLive = DeepClone(historical);
        var routes = (object[])GovernedLane(tooManyLive)["routes"]!;
        ((Dictionary<string, object?>)routes[8])["activation"] = Activation("paused");
        ((Dictionary<string, object?>)routes[8])["inbound_finality_cutoff"] = null;
        Assert.Contains(
            "route bounds",
            Assert.Throws<ArgumentException>(() => SccpRegistryV1.Parse(Json(tooManyLive))).Message,
            StringComparison.Ordinal);
    }

    [Fact]
    public void ProofRequestRequiresElevenSignalKeySemanticProfileAndFinalityAnchor()
    {
        var valid = ProofRequestObject();
        var parsed = SccpGroth16ProofRequestV1.Parse(Json(valid));
        Assert.Equal(SccpDestinationProofBackendV1.EvmGroth16Bn254, parsed.Backend);
        Assert.Equal(SccpNetworkV1.BscMainnet, parsed.TargetNetwork);
        Assert.Equal((ushort)4, parsed.SoraFinalityAnchor.ProtocolVersion);
        Assert.Equal(Upper(0xa2, 32), Convert.ToHexString(parsed.SoraFinalityAnchor.CheckpointContextId));
        Assert.Equal(Upper(0xa3, 32), Convert.ToHexString(parsed.SoraFinalityAnchor.CheckpointFinalityArtifactHash));
        Assert.Equal(
            "CDBEC097FED4AD21E44A354FE09A3C43AD489F4AC78CFF8944BA8BB5CC2FD577",
            Convert.ToHexString(parsed.SoraFinalityAnchor.AnchorHash));
        Assert.Equal("0x1168372c5c87f384d377a3ffb9140af5ac73cdcd693bcb1b4b89995a8fcad92a", parsed.StatementHash);
        Assert.Equal("0x3757838d0f35d387dd9467d4d9c4806fc528d45db22060ea4784402a0208cd5f", parsed.RequestHash);
        Assert.Throws<ArgumentException>(() => ProofRequestObject(2));
        Assert.Throws<ArgumentException>(() => ProofRequestObject(3));
        Assert.Throws<ArgumentException>(() => ProofRequestObject(5));
        var mutations = new Action<Dictionary<string, object?>>[]
        {
            value => value["allow_unready"] = true,
            value => value["backend"] = new Dictionary<string, object?> { ["backend"] = "solana_recursive_v1", ["family"] = null },
            value => value["backend"] = new Dictionary<string, object?> { ["backend"] = "tron_groth16_bn254_v1", ["family"] = null },
            value => value["target_network"] = Network("tron-mainnet"),
            value => ((Dictionary<string, object?>)value["public_inputs"]!)["target_domain"] = 3,
            value => ((Dictionary<string, object?>)value["public_inputs"]!)["message_id"] = PrefixHash(0x77),
            value => value["sora_finality_anchor_hash"] = PrefixHash(0x99),
            value => value["statement_hash"] = PrefixHash(0x61),
            value => value["request_hash"] = PrefixHash(0x64),
            value => value["route_configuration_hash"] = value["destination_binding_hash"],
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)value["verifying_key"]!)["ic"]!).Remove("signal_10"),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)value["verifying_key"]!)["ic"]!)["signal_11"] = G1(),
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)value["semantic_proof_profile"]!)["commitments"]!)["circuit_commitment"] = Upper(0xc2, 32),
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_height"] = 0,
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["protocol_version"] = 1,
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["protocol_version"] = 3,
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["protocol_version"] = "4",
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["protocol_version"] = 5,
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["protocol_version"] = "3",
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["protocol_version"] = true,
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["validator_set_epoch"] = 2,
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_context_id"] = Upper(0, 32),
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_context_id"] =
                ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["chain_id_hash"],
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_block_hash"] =
                ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_context_id"],
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_finality_artifact_hash"] = Upper(0, 32),
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_finality_artifact_hash"] =
                ((Dictionary<string, object?>)value["sora_finality_anchor"]!)["checkpoint_block_hash"],
            value => ((Dictionary<string, object?>)value["sora_finality_anchor"]!).Remove("checkpoint_finality_artifact_hash"),
            value => value["bundle_bytes"] = "0x",
            value => value["bundle_bytes"] = "0x0A",
            TamperProofRequestBundleRevision,
            value => value.Remove("semantic_proof_profile"),
        };
        foreach (var mutation in mutations)
        {
            var value = DeepClone(valid);
            mutation(value);
            Assert.ThrowsAny<ArgumentException>(() => SccpGroth16ProofRequestV1.Parse(Json(value)));
        }

        var text = Encoding.UTF8.GetString(Json(valid));
        Assert.Throws<ArgumentException>(() => SccpGroth16ProofRequestV1.Parse(Encoding.UTF8.GetBytes(
            text.Replace("\"target_domain\":2", "\"target_domain\":2,\"target_domain\":5", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpGroth16ProofRequestV1.Parse(Encoding.UTF8.GetBytes(
            text.Replace("\"protocol_version\":4", "\"protocol_version\":4.0", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpGroth16ProofRequestV1.Parse(Encoding.UTF8.GetBytes(text + "null")));

        var archivedIdentity = ProofRequestObject();
        ((Dictionary<string, object?>)archivedIdentity["sora_finality_anchor"]!)["chain_id_hash"] =
            Convert.ToHexString(SccpV1.Keccak256(
                Convert.FromHexString("809574F5FEE75E69BFCF52451E42D50F")));
        Assert.Throws<ArgumentException>(() => SccpGroth16ProofRequestV1.Parse(Json(archivedIdentity)));
    }

    [Fact]
    public void TonProofRequestRequiresBlsSignalsCircuitAndProofProfile()
    {
        var valid = TonProofRequestObject();
        var parsed = SccpGroth16ProofRequestV1.Parse(Json(valid));
        Assert.Equal(SccpDestinationProofBackendV1.TonGroth16Bls12381, parsed.Backend);
        Assert.Equal(SccpNetworkV1.TonMainnet, parsed.TargetNetwork);
        Assert.Equal(SccpSemanticProofProfileKindV1.Groth16Bls12381, parsed.SemanticProofProfile.Kind);
        Assert.Equal(11, parsed.TonPublicSignals!.Count);
        Assert.Equal(valid["verifier_circuit_hash"], parsed.TonVerifierCircuitHash);
        Assert.Equal(valid["proof_profile_commitment"], parsed.TonProofProfileCommitment);

        foreach (var mutation in new Action<Dictionary<string, object?>>[]
        {
            value => value.Remove("public_signals"),
            value => value["legacy_public_signals"] = value["public_signals"],
            value => value["proof_profile_commitment"] = PrefixHash(0x91),
            value => value["verifier_circuit_hash"] = PrefixHash(0x92),
            value => ((Dictionary<string, object?>)value["public_signals"]!)["route_configuration_hash"] = PrefixHash(0x93),
            value => ((Dictionary<string, object?>)value["verifying_key"]!)["beta2"] = Upper(0x40, 96),
            value => ((Dictionary<string, object?>)value["semantic_proof_profile"]!)["profile"] =
                "sora_taira_finality_inclusion_groth16_bn254",
        })
        {
            var candidate = DeepClone(valid);
            mutation(candidate);
            Assert.ThrowsAny<ArgumentException>(() => SccpGroth16ProofRequestV1.Parse(Json(candidate)));
        }
    }

    [Fact]
    public void BundleAndRecentDiscoveryRejectRetiredPayloadsLinksAndInjection()
    {
        var bundle = BundleObject(MessageId);
        var parsedBundle = SccpMessageBundleV1.Parse(Json(bundle));
        Assert.Equal(2U, parsedBundle.TargetDomain);
        Assert.Equal(1U, parsedBundle.Payload.RouteRevision);
        Assert.Empty(parsedBundle.MerkleProof);
        var retiredPayload = DeepClone(bundle);
        retiredPayload["payload"] = new Dictionary<string, object?> { ["Burn"] = new Dictionary<string, object?>() };
        Assert.Throws<ArgumentException>(() => SccpMessageBundleV1.Parse(Json(retiredPayload)));
        var selector = DeepClone(bundle);
        selector["network"] = "bsc-mainnet";
        Assert.Throws<ArgumentException>(() => SccpMessageBundleV1.Parse(Json(selector)));
        foreach (var (field, invalid) in new (string, object)[]
        {
            ("sender_codec", 2),
            ("recipient_codec", 5),
            ("asset_home_domain", 4),
            ("amount", "340282366920938463463374607431768211456"),
        })
        {
            var malformed = DeepClone(bundle);
            var payload = (Dictionary<string, object?>)malformed["payload"]!;
            var transfer = (Dictionary<string, object?>)payload["Transfer"]!;
            transfer[field] = invalid;
            Assert.Throws<ArgumentException>(() => SccpMessageBundleV1.Parse(Json(malformed)));
        }

        var first = RecentItem(9, MessageId);
        var second = RecentItem(8, new string('2', 64));
        var recent = SccpRecentMessages.Parse(Json(new Dictionary<string, object?>
        {
            ["items"] = new[] { first, second },
            ["next"] = new Dictionary<string, object?>
            {
                ["from"] = 8,
                ["after_index"] = 0,
            },
        }));
        Assert.Equal(new[] { 9UL, 8UL }, recent.Items.Select(static item => item.Height));
        Assert.Equal(new SccpRecentCursor(8, 0), recent.Next);
        var sameHeight = SccpRecentMessages.Parse(Json(new Dictionary<string, object?>
        {
            ["items"] = new[]
            {
                RecentItem(9, MessageId, 509),
                RecentItem(9, new string('2', 64), 510),
                RecentItem(9, new string('3', 64), 511),
                RecentItem(8, new string('4', 64), 0),
            },
        }));
        Assert.Equal(
            new uint[] { 509, 510, 511, 0 },
            sameHeight.Items.Select(static item => item.CommitmentIndex));
        Assert.Null(sameHeight.Next);
        var fullHeight = SccpRecentMessages.Parse(Json(new Dictionary<string, object?>
        {
            ["items"] = new[] { RecentItem(ulong.MaxValue, MessageId) },
            ["next"] = new Dictionary<string, object?>
            {
                ["from"] = ulong.MaxValue,
                ["after_index"] = 0,
            },
        }));
        Assert.Equal(ulong.MaxValue, fullHeight.Items[0].Height);
        Assert.Equal(ulong.MaxValue, fullHeight.Next!.From);
        var retired = DeepClone(first);
        ((Dictionary<string, object?>)retired["links"]!)["artifact_path"] = $"/v1/sccp/artifacts/message/{MessageId}";
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { retired } })));
        var injection = DeepClone(first);
        ((Dictionary<string, object?>)injection["links"]!)["bundle_path"] = $"/v1/sccp/proofs/message/{MessageId}?allow_unready=true";
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { injection } })));
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { second, first } })));
        var missingProjection = DeepClone(first);
        missingProjection.Remove("payload_projection");
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { missingProjection } })));
        var nullProjection = DeepClone(first);
        nullProjection["payload_projection"] = null;
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { nullProjection } })));
        var wrongProjectionDomain = DeepClone(first);
        ((Dictionary<string, object?>)((Dictionary<string, object?>)wrongProjectionDomain["payload_projection"]!)["Transfer"]!)["dest_domain"] = 5;
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { wrongProjectionDomain } })));
        var wrongProjectionRoute = DeepClone(first);
        ((Dictionary<string, object?>)((Dictionary<string, object?>)wrongProjectionRoute["payload_projection"]!)["Transfer"]!)["route_id"] =
            new Dictionary<string, object?>
            {
                ["CanonicalText"] = new Dictionary<string, object?> { ["value"] = "taira_eth_xor" },
            };
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { wrongProjectionRoute } })));
        var zeroProjectionAmount = DeepClone(first);
        ((Dictionary<string, object?>)((Dictionary<string, object?>)zeroProjectionAmount["payload_projection"]!)["Transfer"]!)["amount"] = 0;
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { zeroProjectionAmount } })));
        var unicodeProjectionAddress = DeepClone(first);
        ((Dictionary<string, object?>)((Dictionary<string, object?>)((Dictionary<string, object?>)unicodeProjectionAddress["payload_projection"]!)["Transfer"]!)["recipient"]!)["EvmAddress20"] =
            new Dictionary<string, object?>
            {
                ["bytes"] = "0x" + new string('\u0661', 40),
            };
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { unicodeProjectionAddress } })));
        var wrongSummaryAsset = DeepClone(first);
        wrongSummaryAsset["asset_id"] = "other";
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { wrongSummaryAsset } })));
        var wrongSummaryRoute = DeepClone(first);
        wrongSummaryRoute["route_id"] = "taira_eth_xor";
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { wrongSummaryRoute } })));
        var impossibleSummaryRecipient = DeepClone(first);
        impossibleSummaryRecipient["recipient"] = "0x" + new string('1', 40);
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(new Dictionary<string, object?> { ["items"] = new[] { impossibleSummaryRecipient } })));

        foreach (var mutation in new Action<Dictionary<string, object?>>[]
        {
            value => value.Remove("commitment_index"),
            value => value["commitment_index"] = -1,
            value => value["commitment_index"] = 512,
            value => value["commitment_index"] = 1.5,
        })
        {
            var invalid = RecentItem(9, MessageId);
            mutation(invalid);
            Assert.ThrowsAny<ArgumentException>(() => SccpRecentMessages.Parse(Json(
                new Dictionary<string, object?> { ["items"] = new[] { invalid } })));
        }

        foreach (var items in new[]
        {
            new[] { RecentItem(9, MessageId, 4), RecentItem(9, new string('2', 64), 6) },
            new[] { RecentItem(9, MessageId, 4), RecentItem(9, new string('2', 64), 3) },
            new[] { RecentItem(9, MessageId, 4), RecentItem(8, new string('2', 64), 1) },
        })
        {
            Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(
                new Dictionary<string, object?> { ["items"] = items })));
        }

        foreach (var response in new[]
        {
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId) },
                ["next"] = null,
            },
            new Dictionary<string, object?>
            {
                ["items"] = Array.Empty<object>(),
                ["next"] = new Dictionary<string, object?> { ["from"] = 9, ["after_index"] = 0 },
            },
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId, 3) },
                ["next"] = new Dictionary<string, object?> { ["from"] = 9, ["after_index"] = 2 },
            },
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId, 3) },
                ["next"] = new Dictionary<string, object?> { ["from"] = 8, ["after_index"] = 3 },
            },
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId, 3) },
                ["next"] = new Dictionary<string, object?> { ["from"] = 0, ["after_index"] = 3 },
            },
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId, 3) },
                ["next"] = new Dictionary<string, object?> { ["from"] = 9, ["after_index"] = -1 },
            },
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId, 3) },
                ["next"] = new Dictionary<string, object?> { ["from"] = 9, ["after_index"] = 1.5 },
            },
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId, 3) },
                ["next"] = new Dictionary<string, object?> { ["from"] = 9, ["after_index"] = 512 },
            },
            new Dictionary<string, object?>
            {
                ["items"] = new[] { RecentItem(9, MessageId, 3) },
                ["next"] = new Dictionary<string, object?>
                {
                    ["from"] = 9,
                    ["after_index"] = 3,
                    ["cursor"] = 1,
                },
            },
        })
        {
            Assert.ThrowsAny<ArgumentException>(() => SccpRecentMessages.Parse(Json(response)));
        }
    }

    [Fact]
    public void RecentDiscoveryAcceptsOnlyCanonicalTonAccount36Projection()
    {
        var item = RecentItem(9, MessageId);
        item["target_profile"] = "ton-mainnet";
        item["target_domain"] = 4;
        item["route_id"] = "taira_ton_xor";
        item["payload_projection"] = TransferProjection(4);
        var parsed = Assert.Single(SccpRecentMessages.Parse(Json(
            new Dictionary<string, object?> { ["items"] = new[] { item } })).Items);
        Assert.Equal(SccpNetworkV1.TonMainnet, parsed.Lane.Target);

        var masterchain = DeepClone(item);
        var transfer = (Dictionary<string, object?>)((Dictionary<string, object?>)masterchain["payload_projection"]!)["Transfer"]!;
        var recipient = (Dictionary<string, object?>)((Dictionary<string, object?>)transfer["recipient"]!)["TonAccount36"]!;
        recipient["workchain"] = -1;
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(
            new Dictionary<string, object?> { ["items"] = new[] { masterchain } })));

        var aliased = DeepClone(item);
        transfer = (Dictionary<string, object?>)((Dictionary<string, object?>)aliased["payload_projection"]!)["Transfer"]!;
        recipient = (Dictionary<string, object?>)((Dictionary<string, object?>)transfer["recipient"]!)["TonAccount36"]!;
        recipient["account"] = "0x" + new string('0', 64);
        Assert.Throws<ArgumentException>(() => SccpRecentMessages.Parse(Json(
            new Dictionary<string, object?> { ["items"] = new[] { aliased } })));
    }

    [Fact]
    public void BundleRejectsPayloadCommitmentMerkleAndStrictJsonTampering()
    {
        var valid = BundleObject(MessageId);
        var mutations = new Action<Dictionary<string, object?>>[]
        {
            value => ((Dictionary<string, object?>)((Dictionary<string, object?>)value["payload"]!)["Transfer"]!)["route_revision"] = 2,
            value => ((Dictionary<string, object?>)value["commitment"]!)["message_id"] = PrefixHash(0x44),
            value => ((Dictionary<string, object?>)value["commitment"]!)["payload_hash"] = ((Dictionary<string, object?>)value["commitment"]!)["message_id"],
            value => value["commitment_root"] = PrefixHash(0x45),
            value => value["finality_proof"] = "0x",
            value => value["finality_proof"] = "0x0",
            value => ((Dictionary<string, object?>)value["merkle_proof"]!)["steps"] = new object[]
            {
                new Dictionary<string, object?>
                {
                    ["sibling_hash"] = PrefixHash(0x46),
                    ["sibling_is_left"] = false,
                },
            },
            value => ((Dictionary<string, object?>)value["merkle_proof"]!)["steps"] = Enumerable.Range(0, 65)
                .Select(index => (object)new Dictionary<string, object?>
                {
                    ["sibling_hash"] = PrefixHash((byte)(index + 1)),
                    ["sibling_is_left"] = index % 2 == 0,
                }).ToArray(),
        };
        foreach (var mutation in mutations)
        {
            var candidate = DeepClone(valid);
            mutation(candidate);
            Assert.ThrowsAny<ArgumentException>(() => SccpMessageBundleV1.Parse(Json(candidate)));
        }

        var unknownStep = DeepClone(valid);
        ((Dictionary<string, object?>)unknownStep["merkle_proof"]!)["steps"] = new object[]
        {
            new Dictionary<string, object?>
            {
                ["sibling_hash"] = PrefixHash(0x46),
                ["sibling_is_left"] = false,
                ["direction"] = "right",
            },
        };
        Assert.Throws<ArgumentException>(() => SccpMessageBundleV1.Parse(Json(unknownStep)));

        var text = Encoding.UTF8.GetString(Json(valid));
        Assert.Throws<ArgumentException>(() => SccpMessageBundleV1.Parse(Encoding.UTF8.GetBytes(
            text.Replace("\"route_revision\":1", "\"route_revision\":1,\"route_revision\":2", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpMessageBundleV1.Parse(Encoding.UTF8.GetBytes(text + "{}")));
    }

    [Fact]
    public void BridgeResponseRejectsContradictionsDuplicatesAndLegacyFields()
    {
        var valid = ResponseJson(true, new string('3', 64), null, null);
        Assert.True(SccpBridgeSubmitResponse.Parse(valid).Submitted);
        var text = Encoding.UTF8.GetString(valid);
        Assert.True(SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            text.Replace("bridge/sccp/native/bsc-parlia-v1", "evm-groth16-bn254-v1", StringComparison.Ordinal))).Submitted);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            text.Replace("bridge/sccp/native/bsc-parlia-v1", "bridge/caller-chosen", StringComparison.Ordinal))));
        foreach (var retired in new[]
        {
            "ok", "proof_kind", "message_kind", "manifest_hash_hex",
            "transaction_scaffold_b64", "signed_transaction_b64", "proof_artifact_hash",
        })
        {
            Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(
                Encoding.UTF8.GetBytes(text[..^1] + $",\"{retired}\":null}}")));
        }

        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            text.Replace("\"submitted\":true", "\"submitted\":true,\"submitted\":false", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            text.Replace("\"counterparty_chain\":\"bsc-mainnet\"", "\"counterparty_chain\":\"solana-mainnet-beta\"", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            text.Replace("bridge/sccp/native/bsc-parlia-v1", "bridge/sccp/native/ethereum-beacon-v1", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            text.Replace("\"counterparty_domain\":2", "\"counterparty_domain\":3", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            text.Replace(new string('2', 64), new string('1', 64), StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(
            valid,
            new SccpBridgeResponseExpectation(
                Backend: "evm-groth16-bn254-v1",
                RouteConfigurationHashHex: new string('2', 64),
                RangeStartHeight: 4,
                RangeEndHeight: 10)));
        var tonExpectation = new SccpBridgeResponseExpectation(CounterpartyDomain: 4);
        tonExpectation.Validate();
        Assert.Equal((uint)4, tonExpectation.CounterpartyDomain);

        var transaction = CanonicalTransactionPayload(7);
        var prepared = ResponseJson(
            false,
            null,
            Convert.ToBase64String(transaction),
            Convert.ToBase64String(IrohaHash.Hash(transaction)));
        Assert.False(SccpBridgeSubmitResponse.Parse(prepared).Submitted);
        foreach (var invalidTimeToLiveMilliseconds in new ulong?[] { null, 99_999 })
        {
            var invalidTransaction = CanonicalTransactionPayload(
                7,
                timeToLiveMilliseconds: invalidTimeToLiveMilliseconds);
            Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(ResponseJson(
                false,
                null,
                Convert.ToBase64String(invalidTransaction),
                Convert.ToBase64String(IrohaHash.Hash(invalidTransaction)))));
        }
        var preparedAuthority = Ed25519KeyPair
            .FromSeed(Enumerable.Repeat((byte)0x57, 32).ToArray())
            .ToAccountAddress()
            .ToI105(AccountAddress.TestChainDiscriminant);
        var preparedProof = NoritoCodec.Encode(
            "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1",
            [1],
            flags: 0x02);
        Assert.False(SccpBridgeSubmitResponse.ParseForRequest(
            prepared,
            null,
            preparedAuthority,
            preparedProof).Submitted);

        var destinationTransaction = CanonicalTransactionPayload(7, destinationProof: true);
        var destinationProof = NoritoCodec.Encode(
            SccpSubmitValidation.DestinationArtifactSchemaName,
            [1],
            flags: 0x02);
        var destinationPrepared = ResponseJson(
            false,
            null,
            Convert.ToBase64String(destinationTransaction),
            Convert.ToBase64String(IrohaHash.Hash(destinationTransaction)),
            backend: "evm-groth16-bn254-v1");
        Assert.False(SccpBridgeSubmitResponse.ParseForRequest(
            destinationPrepared,
            null,
            preparedAuthority,
            destinationProof).Submitted);

        var directPair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)0x57, 32).ToArray());
        var directSignature = Ed25519Signer.Sign(
            IrohaHash.Hash(transaction),
            directPair.PrivateKeySeed);
        var transactionBase64 = Convert.ToBase64String(transaction);
        var signatureBase64 = Convert.ToBase64String(directSignature);
        var expectedTransactionHash = SccpSubmitValidation.RequireCanonicalDirectSubmission(
            transactionBase64,
            signatureBase64,
            7,
            "bridge/sccp/native/bsc-parlia-v1",
            Enumerable.Repeat((byte)0x22, 32).ToArray(),
            4,
            9,
            preparedAuthority,
            preparedProof);
        var submitted = ResponseJson(true, expectedTransactionHash, null, null);
        Assert.True(SccpBridgeSubmitResponse.ParseForRequest(
            submitted,
            null,
            preparedAuthority,
            preparedProof,
            transactionBase64,
            signatureBase64).Submitted);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.ParseForRequest(
            ResponseJson(true, new string('3', 64), null, null),
            null,
            preparedAuthority,
            preparedProof,
            transactionBase64,
            signatureBase64));
        var wrongSignature = directSignature.ToArray();
        wrongSignature[0] ^= 1;
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.ParseForRequest(
            submitted,
            null,
            preparedAuthority,
            preparedProof,
            transactionBase64,
            Convert.ToBase64String(wrongSignature)));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.ParseForRequest(
            prepared,
            null,
            preparedAuthority,
            preparedProof,
            transactionBase64,
            signatureBase64));
        var wrongAuthority = Ed25519KeyPair
            .FromSeed(Enumerable.Repeat((byte)0x58, 32).ToArray())
            .ToAccountAddress()
            .ToI105(AccountAddress.TestChainDiscriminant);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.ParseForRequest(
            prepared,
            null,
            wrongAuthority,
            preparedProof));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.ParseForRequest(
            prepared,
            null,
            preparedAuthority,
            NoritoCodec.Encode(
                "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1",
                [2],
                flags: 0x02)));
        foreach (var mutation in new Action<byte[]>[]
        {
            value => MutateCompactTransactionField(value, 0, field => field[0] = 0),
            value => MutateCompactTransactionField(value, 1, field => BinaryPrimitives.WriteUInt32LittleEndian(field, 2)),
            value => MutateCompactTransactionField(value, 2, field => field.AsSpan().Clear()),
            value => MutateCompactTransactionField(value, 3, field => BinaryPrimitives.WriteUInt32LittleEndian(field, 1)),
            value => MutateCompactTransactionField(value, 4, field => field[0] = 2),
            value => MutateCompactTransactionField(value, 5, field => field[0] = 2),
            value => MutateCompactTransactionField(value, 6, field => BinaryPrimitives.WriteUInt64LittleEndian(field, ulong.MaxValue)),
            value => MutateFirstTransactionInstructionArchive(value, archive => archive[6] ^= 1),
            value => MutateFirstTransactionInstructionArchive(value, archive => archive[31] ^= 1),
            value => MutateFirstTransactionInstructionArchive(value, archive =>
            {
                archive[^1] ^= 1;
                RewriteNoritoChecksum(archive);
            }),
        })
        {
            var malformed = transaction.ToArray();
            mutation(malformed);
            Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(ResponseJson(
                false,
                null,
                Convert.ToBase64String(malformed),
                Convert.ToBase64String(IrohaHash.Hash(malformed)))));
        }

        var overlong = new byte[transaction.Length + 1];
        overlong[0] = (byte)(transaction[0] | 0x80);
        overlong[1] = 0;
        transaction.AsSpan(1).CopyTo(overlong.AsSpan(2));
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(ResponseJson(
            false,
            null,
            Convert.ToBase64String(overlong),
            Convert.ToBase64String(IrohaHash.Hash(overlong)))));
        var preparedText = Encoding.UTF8.GetString(prepared);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(Encoding.UTF8.GetBytes(
            preparedText.Replace("\"creation_time_ms\":7", "\"creation_time_ms\":8", StringComparison.Ordinal))));
        var trailingTransaction = transaction.Concat([(byte)0]).ToArray();
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(ResponseJson(
            false,
            null,
            Convert.ToBase64String(trailingTransaction),
            Convert.ToBase64String(IrohaHash.Hash(trailingTransaction)))));

        var legacyOuterBinding = CanonicalTransactionPayload(7, legacyOuterBinding: true);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(ResponseJson(
            false,
            null,
            Convert.ToBase64String(legacyOuterBinding),
            Convert.ToBase64String(IrohaHash.Hash(legacyOuterBinding)))));
        var wrongRouteBinding = CanonicalTransactionPayload(7, routeHashByte: 0x23);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(ResponseJson(
            false,
            null,
            Convert.ToBase64String(wrongRouteBinding),
            Convert.ToBase64String(IrohaHash.Hash(wrongRouteBinding)))));
    }

    [Fact]
    public void PreparedBridgePayloadAcceptsTypedFeeIntentAndBindsExactSelection()
    {
        var pair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)0x57, 32).ToArray());
        var authorityIntent = FeePaymentIntent.Authority([], gasLimit: 700);
        var authorityPayload = CanonicalTransactionPayload(7, feePayment: authorityIntent);
        var authorityPrepared = ResponseJson(
            false,
            null,
            Convert.ToBase64String(authorityPayload),
            Convert.ToBase64String(IrohaHash.Hash(authorityPayload)));

        Assert.False(SccpBridgeSubmitResponse.Parse(
            authorityPrepared,
            new SccpBridgeResponseExpectation(FeePayment: authorityIntent)).Submitted);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(
            authorityPrepared,
            new SccpBridgeResponseExpectation(
                FeePayment: FeePaymentIntent.Authority([], gasLimit: 701))));

        var sponsorId = new FeeSponsorProgramId(
            pair.ToAccountAddress().ToI105(AccountAddress.DefaultChainDiscriminant),
            "wallet_fx");
        var sponsorIntent = FeePaymentIntent.Sponsor(
            sponsorId,
            programRevision: 3,
            chargeLimits: [],
            gasLimit: 700);
        var sponsorPayload = CanonicalTransactionPayload(7, feePayment: sponsorIntent);
        var sponsorPrepared = ResponseJson(
            false,
            null,
            Convert.ToBase64String(sponsorPayload),
            Convert.ToBase64String(IrohaHash.Hash(sponsorPayload)));

        Assert.False(SccpBridgeSubmitResponse.Parse(
            sponsorPrepared,
            new SccpBridgeResponseExpectation(FeePayment: sponsorIntent)).Submitted);
        Assert.Throws<ArgumentException>(() => SccpBridgeSubmitResponse.Parse(
            sponsorPrepared,
            new SccpBridgeResponseExpectation(
                FeePayment: FeePaymentIntent.Sponsor(
                    sponsorId,
                    programRevision: 4,
                    chargeLimits: [],
                    gasLimit: 700))));
    }

    [Fact]
    public async Task ToriiClientUsesExactPathsAndRejectsPathInjectionBeforeFetch()
    {
        var calls = new List<string>();
        var handler = new StubHandler(request =>
        {
            calls.Add(request.RequestUri!.PathAndQuery);
            var body = request.RequestUri.AbsolutePath switch
            {
                "/v1/sccp/capabilities" => CapabilitiesJson(),
                "/v1/sccp/registry" => Json(new Dictionary<string, object?> { ["version"] = 1, ["lanes"] = Array.Empty<object>() }),
                var path when path.Contains("proof-requests", StringComparison.Ordinal) => Json(ProofRequestObject()),
                var path when path.Contains("proofs/message", StringComparison.Ordinal) => Json(BundleObject(MessageId)),
                _ => Json(new Dictionary<string, object?> { ["items"] = Array.Empty<object>() }),
            };
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new ByteArrayContent(body),
            }.WithJsonContentType();
        });
        using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));
        _ = await client.GetSccpCapabilitiesAsync(TestContext.Current.CancellationToken);
        _ = await client.GetSccpRegistryAsync(TestContext.Current.CancellationToken);
        _ = await client.GetSccpMessageBundleAsync(MessageId, TestContext.Current.CancellationToken);
        _ = await client.GetSccpProofRequestAsync(MessageId, TestContext.Current.CancellationToken);
        _ = await client.GetSccpRecentMessagesAsync(
            from: 1,
            afterIndex: 0,
            limit: 1,
            cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(5, calls.Count);
        Assert.Equal("/v1/sccp/messages/recent?from=1&after_index=0&limit=1", calls[^1]);
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() =>
            client.GetSccpRecentMessagesAsync(
                from: 0,
                limit: 1,
                cancellationToken: TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() =>
            client.GetSccpRecentMessagesAsync(
                from: 1,
                limit: 0,
                cancellationToken: TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<ArgumentException>(() =>
            client.GetSccpRecentMessagesAsync(
                afterIndex: 0,
                cancellationToken: TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() =>
            client.GetSccpRecentMessagesAsync(
                from: 1,
                afterIndex: 512,
                cancellationToken: TestContext.Current.CancellationToken));
        foreach (var attack in new[]
        {
            "0x" + MessageId, new string('A', 64), MessageId + "?network=bsc",
            MessageId + "/../registry", new string('0', 64),
        })
        {
            await Assert.ThrowsAsync<ArgumentException>(() =>
                client.GetSccpProofRequestAsync(attack, TestContext.Current.CancellationToken));
        }

        Assert.Equal(5, calls.Count);
    }

    [Fact]
    public async Task BoundedSccpResponseReaderEnforcesDeclaredAndActualBytes()
    {
        using (var exact = new StreamContent(new MemoryStream(new byte[8], writable: false)))
        {
            var bytes = await ToriiClient.ReadBoundedSccpBodyAsync(
                exact,
                8,
                "SCCP test",
                TestContext.Current.CancellationToken);
            Assert.Equal(8, bytes.Length);
        }

        using (var declaredOver = new StreamContent(new MemoryStream([0x01], writable: false)))
        {
            declaredOver.Headers.ContentLength = 9;
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                ToriiClient.ReadBoundedSccpBodyAsync(
                    declaredOver,
                    8,
                    "SCCP test",
                    TestContext.Current.CancellationToken));
        }

        using (var undeclaredOver = new StreamContent(new MemoryStream(new byte[9], writable: false)))
        {
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                ToriiClient.ReadBoundedSccpBodyAsync(
                    undeclaredOver,
                    8,
                    "SCCP test",
                    TestContext.Current.CancellationToken));
        }

        using (var understated = new StreamContent(new MemoryStream(new byte[9], writable: false)))
        {
            understated.Headers.ContentLength = 1;
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                ToriiClient.ReadBoundedSccpBodyAsync(
                    understated,
                    8,
                    "SCCP test",
                    TestContext.Current.CancellationToken));
        }

        using var positiveBound = new ByteArrayContent([0x01]);
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() =>
            ToriiClient.ReadBoundedSccpBodyAsync(
                positiveBound,
                0,
                "SCCP test",
                TestContext.Current.CancellationToken));
    }

    [Theory]
    [InlineData("")]
    [InlineData("01")]
    [InlineData("+1")]
    [InlineData("-1")]
    [InlineData(" 1")]
    [InlineData("1 ")]
    [InlineData("1, 1")]
    [InlineData("9223372036854775808")]
    public async Task BoundedSccpResponseReaderRejectsNoncanonicalContentLength(string value)
    {
        using var content = new StreamContent(new MemoryStream([0x01], writable: false));
        if (!content.Headers.TryAddWithoutValidation("Content-Length", value))
        {
            return;
        }
        await Assert.ThrowsAsync<InvalidDataException>(() =>
            ToriiClient.ReadBoundedSccpBodyAsync(
                content,
                8,
                "SCCP test",
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task ToriiSccpErrorsAreBoundedIncrementallyAndDisposed()
    {
        var stream = new CountingReadStream(10 * 1024 * 1024, 4 * 1024);
        var handler = new StubHandler(_ => new HttpResponseMessage(HttpStatusCode.BadRequest)
        {
            Content = new StreamContent(stream),
        });
        using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.GetSccpCapabilitiesAsync(TestContext.Current.CancellationToken));

        Assert.Equal(HttpStatusCode.BadRequest, error.StatusCode);
        Assert.NotNull(error.ResponseBody);
        Assert.Contains("SCCP response limit", error.ResponseBody!, StringComparison.Ordinal);
        Assert.InRange(stream.BytesRead, 64 * 1024 + 1, 64 * 1024 + 4 * 1024);
        Assert.True(stream.BytesRead < stream.TotalBytes);
        Assert.True(stream.WasDisposed);
    }

    [Theory]
    [InlineData(HttpStatusCode.Created)]
    [InlineData(HttpStatusCode.Accepted)]
    [InlineData(HttpStatusCode.NoContent)]
    public async Task ToriiSccpRequiresExactOkStatus(HttpStatusCode status)
    {
        var handler = new StubHandler(_ => new HttpResponseMessage(status)
        {
            Content = new ByteArrayContent(CapabilitiesJson()),
        }.WithJsonContentType());
        using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.GetSccpCapabilitiesAsync(TestContext.Current.CancellationToken));
        Assert.Equal(status, error.StatusCode);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("text/html")]
    [InlineData("application/problem+json")]
    public async Task ToriiSccpRequiresExactJsonContentType(string? mediaType)
    {
        var handler = new StubHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new ByteArrayContent(CapabilitiesJson()),
            };
            if (mediaType is not null)
            {
                response.Content.Headers.ContentType =
                    new System.Net.Http.Headers.MediaTypeHeaderValue(mediaType);
            }
            return response;
        });
        using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));

        await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.GetSccpCapabilitiesAsync(TestContext.Current.CancellationToken));
    }

    [Theory]
    [InlineData("capabilities", 64 * 1024)]
    [InlineData("recent", 8 * 1024 * 1024)]
    [InlineData("registry", 64 * 1024 * 1024)]
    public async Task ToriiSccpAppliesEndpointSpecificDeclaredLimits(
        string endpoint,
        int maximumBytes)
    {
        var handler = new StubHandler(_ =>
        {
            var content = new ByteArrayContent([0x7b, 0x7d]);
            content.Headers.ContentType =
                new System.Net.Http.Headers.MediaTypeHeaderValue("application/json");
            content.Headers.ContentLength = maximumBytes + 1L;
            return new HttpResponseMessage(HttpStatusCode.OK) { Content = content };
        });
        using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));

        Task invoke = endpoint switch
        {
            "capabilities" => client.GetSccpCapabilitiesAsync(TestContext.Current.CancellationToken),
            "recent" => client.GetSccpRecentMessagesAsync(
                cancellationToken: TestContext.Current.CancellationToken),
            "registry" => client.GetSccpRegistryAsync(TestContext.Current.CancellationToken),
            _ => throw new InvalidOperationException("unknown SCCP endpoint test case"),
        };
        await Assert.ThrowsAsync<InvalidDataException>(async () => await invoke);
    }

    [Fact]
    public async Task ToriiClientPreservesExactPreparedPayloadAcrossDetachedSubmission()
    {
        var pair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)0x57, 32).ToArray());
        var authority = pair.ToAccountAddress().ToI105(AccountAddress.TestChainDiscriminant);
        var proof = NoritoCodec.Encode(
            SccpSubmitValidation.NativeInboundProofSchemaName,
            [1],
            flags: 0x02);
        var payload = CanonicalTransactionPayload(7);
        var signature = Ed25519Signer.Sign(IrohaHash.Hash(payload), pair.PrivateKeySeed);
        var payloadBase64 = Convert.ToBase64String(payload);
        var signatureBase64 = Convert.ToBase64String(signature);
        var expectedHash = SccpSubmitValidation.RequireCanonicalDirectSubmission(
            payloadBase64,
            signatureBase64,
            7,
            "bridge/sccp/native/bsc-parlia-v1",
            Enumerable.Repeat((byte)0x22, 32).ToArray(),
            4,
            9,
            authority,
            proof);
        var calls = 0;
        var handler = new StubHandler(request =>
        {
            calls++;
            Assert.Equal("/v1/bridge/messages", request.RequestUri!.AbsolutePath);
            var body = request.Content!.ReadAsByteArrayAsync().GetAwaiter().GetResult();
            using var document = JsonDocument.Parse(body);
            var direct = document.RootElement.TryGetProperty("signature_b64", out _);
            Assert.Equal(direct, document.RootElement.TryGetProperty("transaction_payload_b64", out _));
            var response = direct
                ? ResponseJson(true, expectedHash, null, null)
                : ResponseJson(
                    false,
                    null,
                    payloadBase64,
                    Convert.ToBase64String(IrohaHash.Hash(payload)));
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new ByteArrayContent(response),
            }.WithJsonContentType();
        });
        using var client = new ToriiClient(new Uri("https://example.test"), new HttpClient(handler));
        var prepared = await client.SubmitSccpBridgeMessageAsync(
            BridgeMessageRequest(
                authority,
                Convert.ToBase64String(proof),
                creationTimeMs: 7),
            cancellationToken: TestContext.Current.CancellationToken);
        Assert.False(prepared.Submitted);
        Assert.Equal(payloadBase64, prepared.TransactionPayloadBase64);

        var submitted = await client.SubmitSccpBridgeMessageAsync(
            BridgeMessageRequest(
                authority,
                Convert.ToBase64String(proof),
                signatureBase64,
                payloadBase64,
                creationTimeMs: 7),
            cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(submitted.Submitted);
        Assert.Equal(expectedHash, submitted.TxHashHex);
        Assert.Equal(2, calls);
    }

    private static byte[] CapabilitiesJson() => Json(CapabilitiesObject());

    private static Dictionary<string, object?> CapabilitiesObject() => new()
    {
        ["version"] = 1,
        ["registry_revision"] = PrefixHash(0x10),
        ["registry_path"] = "/v1/sccp/registry",
        ["message_bundle_path"] = "/v1/sccp/proofs/message/{message_id}",
        ["proof_request_path"] = "/v1/sccp/proof-requests/{message_id}",
        ["recent_messages_path"] = "/v1/sccp/messages/recent",
        ["sora_outbound_material_path"] = "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
        ["registry_limits"] = new Dictionary<string, object?>
        {
            ["max_governed_lanes"] = 16,
            ["max_live_governed_routes"] = 64,
            ["max_live_routes_per_lane"] = 8,
            ["max_retained_routes_per_lane"] = 64,
            ["max_retained_native_trust_anchors_per_lane"] = 4_096,
        },
        ["resource_limits"] = new Dictionary<string, object?>
        {
            ["max_outbound_messages_per_block"] = 512,
            ["max_outbound_message_payload_bytes"] = 4_096,
            ["max_pending_outbound_messages"] = 65_536,
            ["max_pending_outbound_payload_bytes"] = 256 * 1024 * 1024,
            ["max_proofs_per_transaction"] = 1,
            ["max_proofs_per_block"] = 4,
            ["max_proof_bytes_per_proof"] = 8 * 1024 * 1024,
            ["max_proof_bytes_per_transaction"] = 8 * 1024 * 1024,
            ["max_proof_bytes_per_block"] = 32 * 1024 * 1024,
            ["max_native_headers_per_transaction"] = 1_004,
            ["max_native_headers_per_block"] = 4_016,
            ["max_ethereum_light_client_updates_per_transaction"] = 128,
            ["max_ethereum_light_client_updates_per_block"] = 512,
            ["max_native_header_bytes_per_transaction"] = 8 * 1024 * 1024,
            ["max_native_header_bytes_per_block"] = 32 * 1024 * 1024,
            ["max_secp256k1_recoveries_per_transaction"] = 1_005,
            ["max_secp256k1_recoveries_per_block"] = 4_020,
            ["max_bls_aggregate_checks_per_transaction"] = 1_004,
            ["max_bls_aggregate_checks_per_block"] = 4_016,
            ["max_bls_signer_contributions_per_transaction"] = 131_713,
            ["max_bls_signer_contributions_per_block"] = 526_852,
            ["max_ed25519_signature_checks_per_transaction"] = 65_536,
            ["max_ed25519_signature_checks_per_block"] = 262_144,
            ["max_ed25519_validator_key_checks_per_transaction"] = 198_656,
            ["max_ed25519_validator_key_checks_per_block"] = 794_624,
            ["max_bn254_pairing_checks_per_transaction"] = 1,
            ["max_bn254_pairing_checks_per_block"] = 4,
            ["max_bls12_381_pairing_checks_per_transaction"] = 1,
            ["max_bls12_381_pairing_checks_per_block"] = 4,
        },
        ["proof_submit_path"] = "/v1/bridge/proofs/submit",
        ["native_message_submit_path"] = "/v1/bridge/messages",
    };

    private static Dictionary<string, object?> ProofRequestObject(ushort protocolVersion = 4)
    {
        var key = VerifyingKey();
        var policy = OutboundPolicy(protocolVersion);
        var semantic = (Dictionary<string, object?>)policy["semantic_profile"]!;
        var anchor = (Dictionary<string, object?>)policy["sora_finality_anchor"]!;
        var bundleBytes = CanonicalBundleBytes();
        var bundle = SccpV1.DecodeCanonicalMessageBundle(bundleBytes);
        var keyBytes = VerifyingKeyBytes(key);
        var keyHash = SccpV1.Keccak256(keyBytes);
        var semanticHash = SemanticProfileHash(semantic);
        var anchorHash = FinalityAnchorHash(anchor);
        var commitments = (Dictionary<string, object?>)semantic["commitments"]!;
        var semanticModel = new SccpSemanticProofProfileV1(
            Convert.FromHexString((string)commitments["circuit_commitment"]!),
            Convert.FromHexString((string)commitments["witness_generator_commitment"]!),
            Convert.FromHexString((string)commitments["public_signal_schema_hash"]!),
            semanticHash);
        var anchorModel = new SccpSoraFinalityAnchorV1(
            Convert.ToUInt16(
                anchor["protocol_version"],
                System.Globalization.CultureInfo.InvariantCulture),
            Convert.FromHexString((string)anchor["chain_id_hash"]!),
            7,
            Convert.FromHexString((string)anchor["checkpoint_block_hash"]!),
            Convert.FromHexString((string)anchor["checkpoint_context_id"]!),
            Convert.FromHexString((string)anchor["checkpoint_finality_artifact_hash"]!),
            anchorHash);
        var finalityHash = Enumerable.Repeat((byte)0x14, 32).ToArray();
        var binding = bundle.Commitment.Context.DestinationBindingHash;
        var configuration = bundle.Commitment.Context.RouteConfigurationHash;
        var hashes = SccpV1.CanonicalProofRequestHashes(
            SccpDestinationProofBackendV1.EvmGroth16Bn254,
            SccpNetworkV1.SoraTaira,
            SccpNetworkV1.BscMainnet,
            bundle.Commitment.MessageId,
            bundle.Commitment.PayloadHash,
            2,
            bundle.CommitmentRoot,
            9,
            finalityHash,
            bundleBytes,
            keyBytes,
            keyHash,
            semanticModel,
            semanticHash,
            anchorModel,
            anchorHash,
            binding,
            configuration);
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["backend"] = new Dictionary<string, object?> { ["backend"] = "evm_groth16_bn254_v1", ["family"] = null },
            ["source_network"] = Network("sora-taira"),
            ["target_network"] = Network("bsc-mainnet"),
            ["public_inputs"] = new Dictionary<string, object?>
            {
                ["version"] = 1,
                ["message_id"] = "0x" + SccpV1.LowerHex(bundle.Commitment.MessageId),
                ["payload_hash"] = "0x" + SccpV1.LowerHex(bundle.Commitment.PayloadHash),
                ["target_domain"] = 2,
                ["commitment_root"] = "0x" + SccpV1.LowerHex(bundle.CommitmentRoot),
                ["finality_height"] = "9",
                ["finality_block_hash"] = "0x" + SccpV1.LowerHex(finalityHash),
            },
            ["verifying_key"] = key,
            ["verifier_key_hash"] = "0x" + SccpV1.LowerHex(keyHash),
            ["semantic_proof_profile"] = semantic,
            ["semantic_proof_profile_hash"] = "0x" + SccpV1.LowerHex(semanticHash),
            ["sora_finality_anchor"] = anchor,
            ["sora_finality_anchor_hash"] = "0x" + SccpV1.LowerHex(anchorHash),
            ["bundle_bytes"] = "0x" + SccpV1.LowerHex(bundleBytes),
            ["statement_hash"] = "0x" + SccpV1.LowerHex(hashes.StatementHash),
            ["destination_binding_hash"] = "0x" + SccpV1.LowerHex(binding),
            ["route_configuration_hash"] = "0x" + SccpV1.LowerHex(configuration),
            ["request_hash"] = "0x" + SccpV1.LowerHex(hashes.RequestHash),
        };
    }

    private static Dictionary<string, object?> TonProofRequestObject()
    {
        var key = Bls12381VerifyingKey();
        var keyBytes = Bls12381VerifyingKeyBytes(key);
        var keyHash = SHA256.HashData(keyBytes);
        var policy = TonOutboundPolicy();
        var semantic = (Dictionary<string, object?>)policy["semantic_profile"]!;
        var anchor = (Dictionary<string, object?>)policy["sora_finality_anchor"]!;
        var semanticHash = SemanticProfileHash(semantic);
        var anchorHash = FinalityAnchorHash(anchor);
        var circuit = Convert.FromHexString((string)((Dictionary<string, object?>)semantic["commitments"]!)["circuit_commitment"]!);
        var proofProfile = TonProofProfileCommitment();
        var bundleBytes = TonCanonicalBundleBytes();
        var bundle = SccpV1.DecodeCanonicalMessageBundle(bundleBytes);
        var finalityHash = Enumerable.Repeat((byte)0x14, 32).ToArray();
        var binding = bundle.Commitment.Context.DestinationBindingHash;
        var configuration = bundle.Commitment.Context.RouteConfigurationHash;
        var hashes = TonProofRequestHashes(
            bundle,
            bundleBytes,
            finalityHash,
            keyBytes,
            keyHash,
            semantic,
            semanticHash,
            anchor,
            anchorHash,
            circuit,
            proofProfile);
        string[] signalFields =
        [
            "message_id",
            "payload_hash",
            "target_domain",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "source_domain",
            "statement_hash",
            "destination_binding_hash",
            "route_configuration_hash",
            "sora_finality_anchor_hash",
        ];
        var publicSignals = signalFields
            .Select((field, index) => new KeyValuePair<string, object?>(
                field,
                "0x" + SccpV1.LowerHex(hashes.PublicSignals[index])))
            .ToDictionary(static pair => pair.Key, static pair => pair.Value, StringComparer.Ordinal);
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["backend"] = new Dictionary<string, object?>
            {
                ["backend"] = "ton_groth16_bls12381_v1",
                ["family"] = null,
            },
            ["source_network"] = Network("sora-taira"),
            ["target_network"] = Network("ton-mainnet"),
            ["public_inputs"] = new Dictionary<string, object?>
            {
                ["version"] = 1,
                ["message_id"] = "0x" + SccpV1.LowerHex(bundle.Commitment.MessageId),
                ["payload_hash"] = "0x" + SccpV1.LowerHex(bundle.Commitment.PayloadHash),
                ["target_domain"] = 4,
                ["commitment_root"] = "0x" + SccpV1.LowerHex(bundle.CommitmentRoot),
                ["finality_height"] = "9",
                ["finality_block_hash"] = "0x" + SccpV1.LowerHex(finalityHash),
            },
            ["public_signals"] = publicSignals,
            ["verifying_key"] = key,
            ["verifier_key_hash"] = "0x" + SccpV1.LowerHex(keyHash),
            ["verifier_circuit_hash"] = "0x" + SccpV1.LowerHex(circuit),
            ["proof_profile_commitment"] = "0x" + SccpV1.LowerHex(proofProfile),
            ["semantic_proof_profile"] = semantic,
            ["semantic_proof_profile_hash"] = "0x" + SccpV1.LowerHex(semanticHash),
            ["sora_finality_anchor"] = anchor,
            ["sora_finality_anchor_hash"] = "0x" + SccpV1.LowerHex(anchorHash),
            ["bundle_bytes"] = "0x" + SccpV1.LowerHex(bundleBytes),
            ["statement_hash"] = "0x" + SccpV1.LowerHex(hashes.StatementHash),
            ["destination_binding_hash"] = "0x" + SccpV1.LowerHex(binding),
            ["route_configuration_hash"] = "0x" + SccpV1.LowerHex(configuration),
            ["request_hash"] = "0x" + SccpV1.LowerHex(hashes.RequestHash),
        };
    }

    private static Dictionary<string, object?> BundleObject(string messageId)
    {
        var lane = BundleLane();
        var transfer = ExactTransfer();
        var context = new SccpOutboundMessageContextV1(
            lane,
            Enumerable.Repeat((byte)0x71, 32).ToArray(),
            Enumerable.Repeat((byte)0x72, 32).ToArray());
        var commitment = SccpV1.Commitment(context, transfer);
        if (SccpV1.LowerHex(commitment.MessageId) != messageId)
        {
            throw new InvalidOperationException("Test bundle id must match its canonical payload.");
        }

        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["commitment_root"] = "0x" + SccpV1.LowerHex(SccpV1.CommitmentRoot(commitment)),
            ["commitment"] = new Dictionary<string, object?>
            {
                ["version"] = 1,
                ["kind"] = "Transfer",
                ["context"] = new Dictionary<string, object?>
                {
                    ["lane"] = Lane("sora-taira", "bsc-mainnet"),
                    ["destination_binding_hash"] = PrefixHash(0x71),
                    ["route_configuration_hash"] = PrefixHash(0x72),
                },
                ["message_id"] = "0x" + messageId,
                ["payload_hash"] = "0x" + SccpV1.LowerHex(commitment.PayloadHash),
            },
            ["merkle_proof"] = new Dictionary<string, object?> { ["steps"] = Array.Empty<object>() },
            ["payload"] = new Dictionary<string, object?> { ["Transfer"] = TransferPayload() },
            ["finality_proof"] = "0x" + SccpV1.LowerHex(FinalityProofBytes()),
        };
    }

    private static Dictionary<string, object?> RecentItem(
        ulong height,
        string id,
        uint commitmentIndex = 0) => new()
        {
            ["height"] = height,
            ["commitment_index"] = commitmentIndex,
            ["message_id_hex"] = id,
            ["kind"] = "transfer",
            ["source_profile"] = "sora-taira",
            ["target_profile"] = "bsc-mainnet",
            ["destination_binding_hash"] = PrefixHash(0x71),
            ["route_configuration_hash"] = PrefixHash(0x72),
            ["target_domain"] = 2,
            ["asset_id"] = "xor",
            ["route_id"] = "taira_bsc_xor",
            ["recipient"] = null,
            ["amount"] = "1000",
            ["payload_projection"] = TransferProjection(2),
            ["links"] = new Dictionary<string, object?>
            {
                ["bundle_path"] = $"/v1/sccp/proofs/message/{id}",
                ["proof_request_path"] = $"/v1/sccp/proof-requests/{id}",
            },
        };

    private static Dictionary<string, object?> TransferPayload() => new()
    {
        ["version"] = 1,
        ["source_domain"] = 0,
        ["dest_domain"] = 2,
        ["nonce"] = "7",
        ["route_revision"] = 1,
        ["asset_home_domain"] = 0,
        ["asset_id_codec"] = 1,
        ["asset_id"] = "0x786f72",
        ["amount"] = "1000",
        ["sender_codec"] = 1,
        ["sender"] = "0x616c696365407461697261",
        ["recipient_codec"] = 2,
        ["recipient"] = "0x" + new string('1', 40),
        ["route_id_codec"] = 1,
        ["route_id"] = "0x74616972615f6273635f786f72",
    };

    private static Dictionary<string, object?> TransferProjection(uint destinationDomain)
    {
        var route = destinationDomain switch
        {
            1 => "taira_eth_xor",
            2 => "taira_bsc_xor",
            4 => "taira_ton_xor",
            5 => "taira_tron_xor",
            _ => throw new ArgumentOutOfRangeException(nameof(destinationDomain)),
        };
        Dictionary<string, object?> recipient = destinationDomain switch
        {
            4 => new()
            {
                ["TonAccount36"] = new Dictionary<string, object?>
                {
                    ["workchain"] = 0,
                    ["account"] = "0x" + new string('1', 64),
                },
            },
            5 => new()
            {
                ["TronAddress21"] = new Dictionary<string, object?>
                {
                    ["bytes"] = "0x41" + new string('1', 40),
                },
            },
            _ => new()
            {
                ["EvmAddress20"] = new Dictionary<string, object?>
                {
                    ["bytes"] = "0x" + new string('1', 40),
                },
            },
        };
        return new Dictionary<string, object?>
        {
            ["Transfer"] = new Dictionary<string, object?>
            {
                ["version"] = 1,
                ["source_domain"] = 0,
                ["dest_domain"] = destinationDomain,
                ["nonce"] = 7,
                ["route_revision"] = 1,
                ["asset_home_domain"] = 0,
                ["asset_id"] = new Dictionary<string, object?>
                {
                    ["CanonicalText"] = new Dictionary<string, object?> { ["value"] = "xor" },
                },
                ["amount"] = 1000,
                ["sender"] = new Dictionary<string, object?>
                {
                    ["CanonicalText"] = new Dictionary<string, object?> { ["value"] = "alice@taira" },
                },
                ["recipient"] = recipient,
                ["route_id"] = new Dictionary<string, object?>
                {
                    ["CanonicalText"] = new Dictionary<string, object?> { ["value"] = route },
                },
            },
        };
    }

    private static SccpLaneIdV1 BundleLane() =>
        new(SccpNetworkV1.SoraTaira, SccpNetworkV1.BscMainnet);

    private static SccpTransferPayloadV1 ExactTransfer() => new(
        sourceDomain: 0,
        destinationDomain: 2,
        nonce: 7,
        routeRevision: 1,
        assetHomeDomain: 0,
        assetIdCodec: SccpCodecV1.CanonicalText,
        assetId: "xor"u8.ToArray(),
        amount: 1000,
        senderCodec: SccpCodecV1.CanonicalText,
        sender: "alice@taira"u8.ToArray(),
        recipientCodec: SccpCodecV1.EvmAddress20,
        recipient: Enumerable.Repeat((byte)0x11, 20).ToArray(),
        routeIdCodec: SccpCodecV1.CanonicalText,
        routeId: "taira_bsc_xor"u8.ToArray());

    private static byte[] CanonicalBundleBytes()
    {
        var context = new SccpOutboundMessageContextV1(
            BundleLane(),
            Enumerable.Repeat((byte)0x71, 32).ToArray(),
            Enumerable.Repeat((byte)0x72, 32).ToArray());
        var payload = ExactTransfer();
        return SccpV1.CanonicalMessageBundleBytes(
            SccpV1.Commitment(context, payload),
            payload,
            [],
            FinalityProofBytes());
    }

    private static byte[] TonCanonicalBundleBytes()
    {
        var lane = new SccpLaneIdV1(SccpNetworkV1.SoraTaira, SccpNetworkV1.TonMainnet);
        var context = new SccpOutboundMessageContextV1(
            lane,
            Enumerable.Repeat((byte)0x71, 32).ToArray(),
            Enumerable.Repeat((byte)0x72, 32).ToArray());
        var payload = new SccpTransferPayloadV1(
            0,
            4,
            7,
            1,
            0,
            SccpCodecV1.CanonicalText,
            "xor"u8.ToArray(),
            1000,
            SccpCodecV1.CanonicalText,
            "alice@taira"u8.ToArray(),
            SccpCodecV1.TonAccount36,
            new byte[4].Concat(Enumerable.Repeat((byte)0x11, 32)).ToArray(),
            SccpCodecV1.CanonicalText,
            "taira_ton_xor"u8.ToArray());
        return SccpV1.CanonicalMessageBundleBytes(
            SccpV1.Commitment(context, payload),
            payload,
            [],
            FinalityProofBytes());
    }

    private static byte[] FinalityProofBytes() =>
        NoritoCodec.Encode("iroha_sccp::TairaBridgeFinalityProofV1", [1]);

    private static void TamperProofRequestBundleRevision(Dictionary<string, object?> request)
    {
        var encoded = (string)request["bundle_bytes"]!;
        var bytes = Convert.FromHexString(encoded[2..]);
        var commitmentLength = checked((int)BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(33, 4)));
        var proofLengthOffset = 37 + commitmentLength;
        var proofLength = checked((int)BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(proofLengthOffset, 4)));
        var payloadLengthOffset = proofLengthOffset + 4 + proofLength;
        var payloadOffset = payloadLengthOffset + 4;
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(payloadOffset + 18, 4), 2);
        request["bundle_bytes"] = "0x" + SccpV1.LowerHex(bytes);
    }

    private static Dictionary<string, object?> RegistryObject()
    {
        var key = VerifyingKey();
        var policy = OutboundPolicy();
        var semantic = (Dictionary<string, object?>)policy["semantic_profile"]!;
        var anchor = (Dictionary<string, object?>)policy["sora_finality_anchor"]!;
        var tokenAddress = Enumerable.Repeat((byte)0x21, 20).ToArray();
        var verifierAddress = Enumerable.Repeat((byte)0x22, 20).ToArray();
        var routeAddress = Enumerable.Repeat((byte)0x23, 20).ToArray();
        var replayVerifierAddress = Enumerable.Repeat((byte)0x24, 20).ToArray();
        var mintBreakerAddress = Enumerable.Repeat((byte)0x25, 20).ToArray();
        var tokenCodeHash = Enumerable.Repeat((byte)0x31, 32).ToArray();
        var verifierCodeHash = Enumerable.Repeat((byte)0x32, 32).ToArray();
        var routeCodeHash = Enumerable.Repeat((byte)0x34, 32).ToArray();
        var replayVerifierCodeHash = Enumerable.Repeat((byte)0x35, 32).ToArray();
        var mintBreakerCodeHash = Enumerable.Repeat((byte)0x36, 32).ToArray();
        var verifierKeyHash = SccpV1.Keccak256(VerifyingKeyBytes(key));
        var semanticHash = SemanticProfileHash(semantic);
        var anchorHash = FinalityAnchorHash(anchor);
        var configurationHash = RegistryRouteConfigurationHash(
            tokenAddress,
            tokenCodeHash,
            verifierAddress,
            verifierCodeHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
            replayVerifierAddress,
            replayVerifierCodeHash,
            mintBreakerAddress,
            mintBreakerCodeHash);
        var lane = Lane("bsc-mainnet", "sora-taira");
        var route = new Dictionary<string, object?>
        {
            ["lane_id"] = lane,
            ["route_id"] = "taira_bsc_xor",
            ["asset_key"] = "xor",
            ["revision"] = 1,
            ["activation"] = new Dictionary<string, object?>
            {
                ["activation"] = "staged",
                ["direction"] = null,
            },
            ["inbound_finality_cutoff"] = null,
            ["source_identity"] = new Dictionary<string, object?>
            {
                ["lane"] = Lane("bsc-mainnet", "sora-taira"),
                ["emitter"] = new Dictionary<string, object?>
                {
                    ["emitter"] = "evm",
                    ["identity"] = new Dictionary<string, object?>
                    {
                        ["address"] = Convert.ToHexString(routeAddress),
                        ["runtime_code_hash"] = Convert.ToHexString(routeCodeHash),
                        ["route_config_hash"] = Convert.ToHexString(configurationHash),
                    },
                },
            },
            ["destination"] = new Dictionary<string, object?>
            {
                ["family"] = "evm",
                ["deployment"] = new Dictionary<string, object?>
                {
                    ["token_address"] = Convert.ToHexString(tokenAddress),
                    ["token_code_hash"] = Convert.ToHexString(tokenCodeHash),
                    ["verifier_address"] = Convert.ToHexString(verifierAddress),
                    ["verifier_code_hash"] = Convert.ToHexString(verifierCodeHash),
                    ["verifying_key"] = key,
                    ["verifier_key_hash"] = Convert.ToHexString(verifierKeyHash),
                    ["outbound_proof_policy"] = policy,
                    ["route_address"] = Convert.ToHexString(routeAddress),
                    ["route_code_hash"] = Convert.ToHexString(routeCodeHash),
                    ["replay_verifier_address"] = Convert.ToHexString(replayVerifierAddress),
                    ["replay_verifier_code_hash"] = Convert.ToHexString(replayVerifierCodeHash),
                    ["mint_breaker_address"] = Convert.ToHexString(mintBreakerAddress),
                    ["mint_breaker_code_hash"] = Convert.ToHexString(mintBreakerCodeHash),
                    ["taira_to_token_multiplier"] = 1_000_000_000,
                    ["max_wrapped_supply"] = "1000000000000000000000",
                },
            },
            ["sora_outbound_execution_policy"] = SoraOutboundExecutionPolicy(),
            ["settlement"] = new Dictionary<string, object?>
            {
                ["asset_definition_id"] = "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                ["payload_amount_scale"] = 9,
                ["max_outstanding_liability"] = "1000000000000",
            },
        };
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["lanes"] = new object[]
            {
                new Dictionary<string, object?>
                {
                    ["lane_id"] = Lane("bsc-mainnet", "sora-taira"),
                    ["native_trust_anchors"] = Array.Empty<object>(),
                    ["current_native_trust_anchor_hash"] = null,
                    ["routes"] = new object[] { route },
                },
            },
        };
    }

    private static Dictionary<string, object?> TronRegistryObject(
        bool aliasBindingWithTokenCodeHash = false)
    {
        var result = RegistryObject();
        var route = Route(result);
        var lane = Lane("tron-mainnet", "sora-taira");
        GovernedLane(result)["lane_id"] = lane;
        route["lane_id"] = Lane("tron-mainnet", "sora-taira");
        route["route_id"] = "taira_tron_xor";
        var source = (Dictionary<string, object?>)route["source_identity"]!;
        source["lane"] = Lane("tron-mainnet", "sora-taira");
        var emitter = (Dictionary<string, object?>)source["emitter"]!;
        emitter["emitter"] = "tron";
        var identity = (Dictionary<string, object?>)emitter["identity"]!;
        var destination = (Dictionary<string, object?>)route["destination"]!;
        destination["family"] = "tron";
        var deployment = (Dictionary<string, object?>)destination["deployment"]!;
        var policy = (Dictionary<string, object?>)deployment["outbound_proof_policy"]!;
        var semantic = (Dictionary<string, object?>)policy["semantic_profile"]!;
        var anchor = (Dictionary<string, object?>)policy["sora_finality_anchor"]!;
        var tokenAddress = Convert.FromHexString((string)deployment["token_address"]!);
        var tokenCodeHash = Convert.FromHexString((string)deployment["token_code_hash"]!);
        var verifierAddress = Convert.FromHexString((string)deployment["verifier_address"]!);
        var routeAddress = Convert.FromHexString((string)deployment["route_address"]!);
        var verifierCodeHash = Convert.FromHexString((string)deployment["verifier_code_hash"]!);
        var verifierKeyHash = Convert.FromHexString((string)deployment["verifier_key_hash"]!);
        var replayVerifierAddress = Convert.FromHexString((string)deployment["replay_verifier_address"]!);
        var replayVerifierCodeHash = Convert.FromHexString((string)deployment["replay_verifier_code_hash"]!);
        var mintBreakerAddress = Convert.FromHexString((string)deployment["mint_breaker_address"]!);
        var mintBreakerCodeHash = Convert.FromHexString((string)deployment["mint_breaker_code_hash"]!);
        var semanticHash = SemanticProfileHash(semantic);
        var anchorHash = FinalityAnchorHash(anchor);
        var binding = RegistryDestinationBindingHash(
            SccpNetworkV1.TronMainnet,
            verifierAddress,
            routeAddress,
            verifierCodeHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
            replayVerifierAddress,
            replayVerifierCodeHash,
            mintBreakerAddress,
            mintBreakerCodeHash);
        if (aliasBindingWithTokenCodeHash)
        {
            tokenCodeHash = binding;
            deployment["token_code_hash"] = Convert.ToHexString(tokenCodeHash);
        }

        identity["route_config_hash"] = Convert.ToHexString(RegistryRouteConfigurationHash(
            tokenAddress,
            tokenCodeHash,
            verifierAddress,
            verifierCodeHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
            replayVerifierAddress,
            replayVerifierCodeHash,
            mintBreakerAddress,
            mintBreakerCodeHash,
            source: SccpNetworkV1.TronMainnet,
            destinationBinding: binding));
        return result;
    }

    private static Dictionary<string, object?> TonRegistryObject()
    {
        var key = Bls12381VerifyingKey();
        var keyHash = SHA256.HashData(Bls12381VerifyingKeyBytes(key));
        var policy = TonOutboundPolicy();
        var semantic = (Dictionary<string, object?>)policy["semantic_profile"]!;
        var anchor = (Dictionary<string, object?>)policy["sora_finality_anchor"]!;
        var semanticHash = SemanticProfileHash(semantic);
        var anchorHash = FinalityAnchorHash(anchor);
        var master = TonAddress(0x21);
        var routeAddress = TonAddress(0x23);
        var masterCode = Enumerable.Repeat((byte)0x31, 32).ToArray();
        var masterInitialData = Enumerable.Repeat((byte)0x36, 32).ToArray();
        var walletCode = Enumerable.Repeat((byte)0x32, 32).ToArray();
        var routeCode = Enumerable.Repeat((byte)0x34, 32).ToArray();
        var routeInitialData = Enumerable.Repeat((byte)0x37, 32).ToArray();
        var verifierCode = Enumerable.Repeat((byte)0x35, 32).ToArray();
        var circuit = Convert.FromHexString((string)((Dictionary<string, object?>)semantic["commitments"]!)["circuit_commitment"]!);
        var proofProfile = TonProofProfileCommitment();
        var guardians = Enumerable.Range(0, 5)
            .Select(index => Enumerable.Repeat(checked((byte)(0xA1 + index)), 32).ToArray())
            .ToArray();
        var binding = TonDestinationBindingHash(
            SccpNetworkV1.TonMainnet,
            masterCode,
            walletCode,
            routeCode,
            verifierCode,
            circuit,
            keyHash,
            proofProfile,
            guardians,
            semanticHash,
            anchorHash);
        var configuration = TonRouteConfigurationHash(
            SccpNetworkV1.TonMainnet,
            masterCode,
            walletCode,
            routeCode,
            verifierCode,
            circuit,
            keyHash,
            proofProfile,
            guardians,
            semanticHash,
            anchorHash,
            binding,
            1);
        var lane = Lane("ton-mainnet", "sora-taira");
        var route = new Dictionary<string, object?>
        {
            ["lane_id"] = lane,
            ["route_id"] = "taira_ton_xor",
            ["asset_key"] = "xor",
            ["revision"] = 1,
            ["activation"] = Activation("staged"),
            ["inbound_finality_cutoff"] = null,
            ["source_identity"] = new Dictionary<string, object?>
            {
                ["lane"] = Lane("ton-mainnet", "sora-taira"),
                ["emitter"] = new Dictionary<string, object?>
                {
                    ["emitter"] = "ton",
                    ["identity"] = new Dictionary<string, object?>
                    {
                        ["address"] = routeAddress,
                        ["code_hash"] = Convert.ToHexString(routeCode),
                        ["route_config_hash"] = Convert.ToHexString(configuration),
                    },
                },
            },
            ["destination"] = new Dictionary<string, object?>
            {
                ["family"] = "ton",
                ["deployment"] = new Dictionary<string, object?>
                {
                    ["jetton_master_address"] = master,
                    ["jetton_master_code_hash"] = Convert.ToHexString(masterCode),
                    ["jetton_master_initial_data_hash"] = Convert.ToHexString(masterInitialData),
                    ["jetton_wallet_code_hash"] = Convert.ToHexString(walletCode),
                    ["route_address"] = routeAddress,
                    ["route_code_hash"] = Convert.ToHexString(routeCode),
                    ["route_initial_data_hash"] = Convert.ToHexString(routeInitialData),
                    ["embedded_verifier_code_hash"] = Convert.ToHexString(verifierCode),
                    ["verifier_circuit_hash"] = Convert.ToHexString(circuit),
                    ["verifying_key"] = key,
                    ["verifier_key_hash"] = Convert.ToHexString(keyHash),
                    ["proof_profile_commitment"] = Convert.ToHexString(proofProfile),
                    ["mint_breaker_guardian_keys"] = new Dictionary<string, object?>
                    {
                        ["guardian_0"] = Convert.ToHexString(guardians[0]),
                        ["guardian_1"] = Convert.ToHexString(guardians[1]),
                        ["guardian_2"] = Convert.ToHexString(guardians[2]),
                        ["guardian_3"] = Convert.ToHexString(guardians[3]),
                        ["guardian_4"] = Convert.ToHexString(guardians[4]),
                    },
                    ["outbound_proof_policy"] = policy,
                    ["taira_to_token_multiplier"] = 1,
                    ["max_wrapped_supply"] = "1000000000000",
                },
            },
            ["sora_outbound_execution_policy"] = SoraOutboundExecutionPolicy(),
            ["settlement"] = new Dictionary<string, object?>
            {
                ["asset_definition_id"] = "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                ["payload_amount_scale"] = 9,
                ["max_outstanding_liability"] = "1000000000000",
            },
        };
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["lanes"] = new object[]
            {
                new Dictionary<string, object?>
                {
                    ["lane_id"] = lane,
                    ["native_trust_anchors"] = Array.Empty<object>(),
                    ["current_native_trust_anchor_hash"] = null,
                    ["routes"] = new object[] { route },
                },
            },
        };
    }

    private static Dictionary<string, object?> TonDeployment(Dictionary<string, object?> registry) =>
        (Dictionary<string, object?>)((Dictionary<string, object?>)Route(registry)["destination"]!)["deployment"]!;

    private static Dictionary<string, object?> TonDeployment(
        Dictionary<string, object?> registry,
        string addressField) =>
        (Dictionary<string, object?>)TonDeployment(registry)[addressField]!;

    private static Dictionary<string, object?> TonEmitterIdentity(Dictionary<string, object?> registry) =>
        (Dictionary<string, object?>)((Dictionary<string, object?>)((Dictionary<string, object?>)Route(registry)["source_identity"]!)["emitter"]!)["identity"]!;

    private static Dictionary<string, object?> Route(Dictionary<string, object?> registry)
    {
        return (Dictionary<string, object?>)((object[])GovernedLane(registry)["routes"]!)[0];
    }

    private static Dictionary<string, object?> GovernedLane(Dictionary<string, object?> registry) =>
        (Dictionary<string, object?>)((object[])registry["lanes"]!)[0];

    private static Dictionary<string, object?> NativeTrustAnchor(byte hashByte, ulong checkpointHeight) => new()
    {
        ["backend"] = new Dictionary<string, object?>
        {
            ["backend"] = "bsc_parlia_v1",
            ["protocol"] = null,
        },
        ["anchor_hash"] = Upper(hashByte, 32),
        ["checkpoint_height"] = checkpointHeight,
    };

    private static Dictionary<string, object?> Activation(string activation) => new()
    {
        ["activation"] = activation,
        ["direction"] = null,
    };

    private static Dictionary<string, object?> RouteRevision(
        Dictionary<string, object?> template,
        uint revision,
        string activation)
    {
        var route = DeepClone(template);
        route["revision"] = revision;
        route["activation"] = Activation(activation);
        route["inbound_finality_cutoff"] = activation == "retired"
            ? new Dictionary<string, object?>
            {
                ["trust_anchor_hash"] = Upper(0x91, 32),
                ["max_anchor_interval_height"] = 2,
            }
            : null;

        var destination = (Dictionary<string, object?>)route["destination"]!;
        var deployment = (Dictionary<string, object?>)destination["deployment"]!;
        var tokenAddress = Enumerable.Repeat(checked((byte)(0x20 + revision)), 20).ToArray();
        var verifierAddress = Enumerable.Repeat(checked((byte)(0x40 + revision)), 20).ToArray();
        var routeAddress = Enumerable.Repeat(checked((byte)(0x60 + revision)), 20).ToArray();
        var replayVerifierAddress = Enumerable.Repeat(checked((byte)(0x80 + revision)), 20).ToArray();
        var mintBreakerAddress = Enumerable.Repeat(checked((byte)(0xA0 + revision)), 20).ToArray();
        deployment["token_address"] = Convert.ToHexString(tokenAddress);
        deployment["verifier_address"] = Convert.ToHexString(verifierAddress);
        deployment["route_address"] = Convert.ToHexString(routeAddress);
        deployment["replay_verifier_address"] = Convert.ToHexString(replayVerifierAddress);
        deployment["mint_breaker_address"] = Convert.ToHexString(mintBreakerAddress);

        var sourceIdentity = (Dictionary<string, object?>)route["source_identity"]!;
        var emitter = (Dictionary<string, object?>)sourceIdentity["emitter"]!;
        var identity = (Dictionary<string, object?>)emitter["identity"]!;
        identity["address"] = Convert.ToHexString(routeAddress);

        var policy = (Dictionary<string, object?>)deployment["outbound_proof_policy"]!;
        var semantic = (Dictionary<string, object?>)policy["semantic_profile"]!;
        var anchor = (Dictionary<string, object?>)policy["sora_finality_anchor"]!;
        var configurationHash = RegistryRouteConfigurationHash(
            tokenAddress,
            Convert.FromHexString((string)deployment["token_code_hash"]!),
            verifierAddress,
            Convert.FromHexString((string)deployment["verifier_code_hash"]!),
            Convert.FromHexString((string)deployment["verifier_key_hash"]!),
            SemanticProfileHash(semantic),
            FinalityAnchorHash(anchor),
            replayVerifierAddress,
            Convert.FromHexString((string)deployment["replay_verifier_code_hash"]!),
            mintBreakerAddress,
            Convert.FromHexString((string)deployment["mint_breaker_code_hash"]!),
            revision);
        identity["route_config_hash"] = Convert.ToHexString(configurationHash);
        return route;
    }

    private static void RefreshRegistryRouteConfigurationHash(
        Dictionary<string, object?> registry,
        SccpNetworkV1 source)
    {
        var route = Route(registry);
        var destination = (Dictionary<string, object?>)route["destination"]!;
        var deployment = (Dictionary<string, object?>)destination["deployment"]!;
        var policy = (Dictionary<string, object?>)deployment["outbound_proof_policy"]!;
        var semantic = (Dictionary<string, object?>)policy["semantic_profile"]!;
        var anchor = (Dictionary<string, object?>)policy["sora_finality_anchor"]!;
        var verifierAddress = Convert.FromHexString((string)deployment["verifier_address"]!);
        var routeAddress = Convert.FromHexString((string)deployment["route_address"]!);
        var verifierCodeHash = Convert.FromHexString((string)deployment["verifier_code_hash"]!);
        var verifierKeyHash = Convert.FromHexString((string)deployment["verifier_key_hash"]!);
        var replayVerifierAddress = Convert.FromHexString((string)deployment["replay_verifier_address"]!);
        var replayVerifierCodeHash = Convert.FromHexString((string)deployment["replay_verifier_code_hash"]!);
        var mintBreakerAddress = Convert.FromHexString((string)deployment["mint_breaker_address"]!);
        var mintBreakerCodeHash = Convert.FromHexString((string)deployment["mint_breaker_code_hash"]!);
        var semanticHash = SemanticProfileHash(semantic);
        var anchorHash = FinalityAnchorHash(anchor);
        var destinationBinding = source == SccpNetworkV1.TronMainnet
            ? RegistryDestinationBindingHash(
                source,
                verifierAddress,
                routeAddress,
                verifierCodeHash,
                verifierKeyHash,
                semanticHash,
                anchorHash,
                replayVerifierAddress,
                replayVerifierCodeHash,
                mintBreakerAddress,
                mintBreakerCodeHash)
            : null;
        var configuration = RegistryRouteConfigurationHash(
            Convert.FromHexString((string)deployment["token_address"]!),
            Convert.FromHexString((string)deployment["token_code_hash"]!),
            verifierAddress,
            verifierCodeHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
            replayVerifierAddress,
            replayVerifierCodeHash,
            mintBreakerAddress,
            mintBreakerCodeHash,
            Convert.ToUInt32(route["revision"]),
            source,
            destinationBinding);
        var sourceIdentity = (Dictionary<string, object?>)route["source_identity"]!;
        var emitter = (Dictionary<string, object?>)sourceIdentity["emitter"]!;
        ((Dictionary<string, object?>)emitter["identity"]!)["route_config_hash"] =
            Convert.ToHexString(configuration);
    }

    private static byte[] RegistryRouteConfigurationHash(
        byte[] tokenAddress,
        byte[] tokenCodeHash,
        byte[] verifierAddress,
        byte[] verifierCodeHash,
        byte[] verifierKeyHash,
        byte[] semanticHash,
        byte[] anchorHash,
        byte[] replayVerifierAddress,
        byte[] replayVerifierCodeHash,
        byte[] mintBreakerAddress,
        byte[] mintBreakerCodeHash,
        uint revision = 1,
        SccpNetworkV1 source = SccpNetworkV1.BscMainnet,
        byte[]? destinationBinding = null)
    {
        var (routeId, network) = source switch
        {
            SccpNetworkV1.EthereumMainnet => ("taira_eth_xor", 1UL),
            SccpNetworkV1.BscMainnet => ("taira_bsc_xor", 56UL),
            SccpNetworkV1.TronMainnet => ("taira_tron_xor", 0x2b66_53dcUL),
            _ => throw new ArgumentException("test registry source must be external"),
        };
        var inbound = new SccpLaneIdV1(source, SccpNetworkV1.SoraTaira);
        var outbound = new SccpLaneIdV1(SccpNetworkV1.SoraTaira, source);
        var deploymentParts = new List<byte[]>
        {
            AbiAddress(tokenAddress),
            tokenCodeHash,
            AbiAddress(verifierAddress),
            verifierCodeHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
        };
        if (source == SccpNetworkV1.TronMainnet)
        {
            deploymentParts.Add(destinationBinding
                ?? throw new ArgumentException("TRON test deployment requires its binding"));
        }
        deploymentParts.AddRange([
            AbiAddress(replayVerifierAddress), replayVerifierCodeHash,
            AbiAddress(mintBreakerAddress), mintBreakerCodeHash,
        ]);
        var deploymentHash = SccpV1.Keccak256(Concat(deploymentParts.ToArray()));
        var assetRouteHash = SccpV1.Keccak256(Concat(
            SccpV1.Keccak256("xor"u8),
            SccpV1.Keccak256(Encoding.UTF8.GetBytes(routeId)),
            AbiWord(revision),
            AbiWord(1_000_000_000),
            AbiWord((UInt128)1_000_000_000_000_000_000_000m)));
        return SccpV1.Keccak256(Concat(
            SccpV1.Keccak256("sccp:concrete-route-config:v1"u8),
            AbiWord(source.DomainId()),
            AbiWord((byte)source),
            AbiWord(network),
            SccpV1.LaneHash(inbound),
            SccpV1.LaneHash(outbound),
            deploymentHash,
            assetRouteHash));
    }

    private static byte[] AbiAddress(byte[] address) => Concat(new byte[12], address);

    private static byte[] AbiTronAddress(byte[] address) =>
        Concat(new byte[11], [0x41], address);

    private static byte[] RegistryDestinationBindingHash(
        SccpNetworkV1 source,
        byte[] verifierAddress,
        byte[] routeAddress,
        byte[] verifierCodeHash,
        byte[] verifierKeyHash,
        byte[] semanticHash,
        byte[] anchorHash,
        byte[] replayVerifierAddress,
        byte[] replayVerifierCodeHash,
        byte[] mintBreakerAddress,
        byte[] mintBreakerCodeHash)
    {
        var network = source switch
        {
            SccpNetworkV1.EthereumMainnet => 1UL,
            SccpNetworkV1.BscMainnet => 56UL,
            SccpNetworkV1.TronMainnet => 0x2b66_53dcUL,
            _ => throw new ArgumentException("test binding source must be external"),
        };
        var tron = source == SccpNetworkV1.TronMainnet;
        return SccpV1.Keccak256(Concat(
            SccpV1.Keccak256(Encoding.UTF8.GetBytes(tron
                ? "iroha:sccp:tron-destination-binding:v1"
                : "iroha:sccp:evm-destination-binding:v1")),
            SccpV1.Keccak256(Encoding.UTF8.GetBytes(tron
                ? "tron-groth16-bn254-v1"
                : "evm-groth16-bn254-v1")),
            AbiWord(network),
            AbiWord(0),
            AbiWord(source.DomainId()),
            tron ? AbiTronAddress(verifierAddress) : AbiAddress(verifierAddress),
            tron ? AbiTronAddress(routeAddress) : AbiAddress(routeAddress),
            verifierCodeHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
            tron ? AbiTronAddress(replayVerifierAddress) : AbiAddress(replayVerifierAddress),
            replayVerifierCodeHash,
            tron ? AbiTronAddress(mintBreakerAddress) : AbiAddress(mintBreakerAddress),
            mintBreakerCodeHash));
    }

    private static byte[] AbiWord(ulong value)
    {
        var result = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(24), value);
        return result;
    }

    private static byte[] AbiWord(UInt128 value)
    {
        var result = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(16), (ulong)(value >> 64));
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(24), (ulong)value);
        return result;
    }

    private static Dictionary<string, object?> OutboundPolicy(ushort protocolVersion = 4)
    {
        var anchor = FinalityAnchor(protocolVersion);
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["semantic_profile"] = new Dictionary<string, object?>
            {
                ["profile"] = "sora_taira_finality_inclusion_groth16_bn254",
                ["commitments"] = new Dictionary<string, object?>
                {
                    ["version"] = 1,
                    ["circuit_commitment"] = Upper(0xc1, 32),
                    ["witness_generator_commitment"] = Upper(0xc2, 32),
                    ["public_signal_schema_hash"] = Convert.ToHexString(PublicSignalSchemaHash()),
                },
            },
            ["sora_finality_anchor"] = anchor,
        };
    }

    private static Dictionary<string, object?> SoraOutboundExecutionPolicy() => new()
    {
        ["version"] = 1,
        ["semantics"] = "ivm_proved_record_sccp_message_v1",
        ["contract_artifact_sha256"] = Upper(0x51, 32),
        ["vk_ref"] = new Dictionary<string, object?>
        {
            ["backend"] = "halo2/ipa",
            ["name"] = "sccp_route_v1",
            ["version"] = 1,
            ["commitment"] = Upper(0x52, 32),
        },
        ["gas_limit"] = 1_000_000,
    };

    private static Dictionary<string, object?> FinalityAnchor(ushort protocolVersion = 4)
    {
        var chainHash = SccpV1.Keccak256(Convert.FromHexString("FC56984B2BE7431D840E21514D1883F0"));
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["source_network"] = Network("sora-taira"),
            ["protocol_version"] = protocolVersion,
            ["chain_id_hash"] = Convert.ToHexString(chainHash),
            ["checkpoint_height"] = 7,
            ["checkpoint_block_hash"] = Upper(0xa1, 32),
            ["checkpoint_context_id"] = Upper(0xa2, 32),
            ["checkpoint_finality_artifact_hash"] = Upper(0xa3, 32),
        };
    }

    private static Dictionary<string, object?> VerifyingKey()
    {
        var ic = new Dictionary<string, object?> { ["constant"] = G1() };
        for (var index = 0; index <= 10; index++)
        {
            ic[$"signal_{index}"] = G1();
        }

        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["alpha1"] = G1(),
            ["beta2"] = G2(),
            ["gamma2"] = G2(),
            ["delta2"] = G2(),
            ["ic"] = ic,
        };
    }

    private static Dictionary<string, object?> G1() => new() { ["x"] = Upper(1, 32), ["y"] = Upper(2, 32) };
    private static Dictionary<string, object?> G2() => new()
    {
        ["x_c0"] = Upper(3, 32),
        ["x_c1"] = Upper(4, 32),
        ["y_c0"] = Upper(5, 32),
        ["y_c1"] = Upper(6, 32),
    };

    private static byte[] VerifyingKeyBytes(Dictionary<string, object?> key)
    {
        var words = new List<byte[]>();
        static void AddG1(List<byte[]> values, Dictionary<string, object?> point)
        {
            values.Add(Convert.FromHexString((string)point["x"]!));
            values.Add(Convert.FromHexString((string)point["y"]!));
        }
        static void AddG2(List<byte[]> values, Dictionary<string, object?> point)
        {
            foreach (var field in new[] { "x_c0", "x_c1", "y_c0", "y_c1" })
            {
                values.Add(Convert.FromHexString((string)point[field]!));
            }
        }
        AddG1(words, (Dictionary<string, object?>)key["alpha1"]!);
        foreach (var field in new[] { "beta2", "gamma2", "delta2" })
        {
            AddG2(words, (Dictionary<string, object?>)key[field]!);
        }
        var ic = (Dictionary<string, object?>)key["ic"]!;
        AddG1(words, (Dictionary<string, object?>)ic["constant"]!);
        for (var index = 0; index <= 10; index++)
        {
            AddG1(words, (Dictionary<string, object?>)ic[$"signal_{index}"]!);
        }
        return Concat(words.ToArray());
    }

    private static Dictionary<string, object?> Bls12381VerifyingKey()
    {
        var g1 = "80" + new string('0', 94);
        var g2 = "80" + new string('0', 190);
        var ic = new Dictionary<string, object?> { ["constant"] = g1 };
        for (var index = 0; index <= 10; index++)
        {
            ic[$"signal_{index}"] = g1;
        }
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["alpha1"] = g1,
            ["beta2"] = g2,
            ["gamma2"] = g2,
            ["delta2"] = g2,
            ["ic"] = ic,
        };
    }

    private static byte[] Bls12381VerifyingKeyBytes(Dictionary<string, object?> key)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        output.Write(Convert.FromHexString((string)key["alpha1"]!));
        output.Write(Convert.FromHexString((string)key["beta2"]!));
        output.Write(Convert.FromHexString((string)key["gamma2"]!));
        output.Write(Convert.FromHexString((string)key["delta2"]!));
        var ic = (Dictionary<string, object?>)key["ic"]!;
        output.Write(Convert.FromHexString((string)ic["constant"]!));
        for (var index = 0; index <= 10; index++)
        {
            output.Write(Convert.FromHexString((string)ic[$"signal_{index}"]!));
        }
        return output.ToArray();
    }

    private static byte[] PublicSignalSchemaHash()
    {
        string[] labels =
        [
            "sccp:groth16-bn254:signal:message-id:v1",
            "sccp:groth16-bn254:signal:payload-hash:v1",
            "sccp:groth16-bn254:signal:target-domain:v1",
            "sccp:groth16-bn254:signal:commitment-root:v1",
            "sccp:groth16-bn254:signal:finality-height:v1",
            "sccp:groth16-bn254:signal:finality-block-hash:v1",
            "sccp:groth16-bn254:signal:source-domain:v1",
            "sccp:groth16-bn254:signal:statement-hash:v1",
            "sccp:groth16-bn254:signal:destination-binding-hash:v1",
            "sccp:groth16-bn254:signal:route-configuration-hash:v1",
            "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
        ];
        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        WriteUInt32(canonical, checked((uint)labels.Length));
        foreach (var label in labels)
        {
            var bytes = Encoding.UTF8.GetBytes(label);
            WriteUInt32(canonical, checked((uint)bytes.Length));
            canonical.Write(bytes);
        }
        return SccpV1.Keccak256(Concat("sccp:groth16-bn254:public-signal-schema:v1"u8.ToArray(), canonical.ToArray()));
    }

    private static byte[] Bls12381PublicSignalSchemaHash()
    {
        string[] labels =
        [
            "sccp:groth16-bls12381:signal:message-id:v1",
            "sccp:groth16-bls12381:signal:payload-hash:v1",
            "sccp:groth16-bls12381:signal:target-domain:v1",
            "sccp:groth16-bls12381:signal:commitment-root:v1",
            "sccp:groth16-bls12381:signal:finality-height:v1",
            "sccp:groth16-bls12381:signal:finality-block-hash:v1",
            "sccp:groth16-bls12381:signal:source-domain:v1",
            "sccp:groth16-bls12381:signal:statement-hash:v1",
            "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
            "sccp:groth16-bls12381:signal:route-config-hash:v1",
            "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
        ];
        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        WriteUInt32(canonical, checked((uint)labels.Length));
        foreach (var label in labels)
        {
            WriteVector(canonical, Encoding.UTF8.GetBytes(label));
        }
        return SHA256.HashData(Concat(
            "sccp:groth16-bls12381:public-signal-schema:v1"u8.ToArray(),
            canonical.ToArray()));
    }

    private static Dictionary<string, object?> TonOutboundPolicy()
    {
        var anchor = FinalityAnchor();
        return new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["semantic_profile"] = new Dictionary<string, object?>
            {
                ["profile"] = "sora_taira_finality_inclusion_groth16_bls12381",
                ["commitments"] = new Dictionary<string, object?>
                {
                    ["version"] = 1,
                    ["circuit_commitment"] = Upper(0xc1, 32),
                    ["witness_generator_commitment"] = Upper(0xc2, 32),
                    ["public_signal_schema_hash"] = Convert.ToHexString(Bls12381PublicSignalSchemaHash()),
                },
            },
            ["sora_finality_anchor"] = anchor,
        };
    }

    private static byte[] TonProofProfileCommitment() => SHA256.HashData(Concat(
        "sccp:ton:groth16-bls12381:proof-profile:v1"u8.ToArray(),
        [1],
        "ietf-bls12381-compressed-g1-48-g2-96"u8.ToArray(),
        "groth16-a-g1-b-g2-c-g1"u8.ToArray(),
        "sha256-sha256-label-value-mod-r"u8.ToArray(),
        Convert.FromHexString("73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001"),
        Bls12381PublicSignalSchemaHash()));

    private static Dictionary<string, object?> TonAddress(byte value) => new()
    {
        ["workchain"] = 0,
        ["account"] = Upper(value, 32),
    };

    private static byte[] TonDestinationBindingHash(
        SccpNetworkV1 network,
        byte[] masterCode,
        byte[] walletCode,
        byte[] routeCode,
        byte[] verifierCode,
        byte[] circuit,
        byte[] keyHash,
        byte[] proofProfile,
        IReadOnlyList<byte[]> guardians,
        byte[] semanticHash,
        byte[] anchorHash)
    {
        using var payload = new MemoryStream();
        payload.Write("iroha:sccp:ton-destination-binding:v1"u8);
        payload.WriteByte(1);
        WriteVector(payload, "ton-groth16-bls12381-v1"u8);
        WriteVector(payload, SccpV1.CanonicalNetworkBytes(network));
        WriteInt32(payload, -239);
        WriteUInt32(payload, 0);
        WriteUInt32(payload, 4);
        payload.Write(masterCode);
        payload.Write(walletCode);
        payload.Write(routeCode);
        payload.Write(verifierCode);
        payload.Write(circuit);
        payload.Write(keyHash);
        payload.Write(proofProfile);
        foreach (var guardian in guardians)
        {
            payload.Write(guardian);
        }
        payload.Write(semanticHash);
        payload.Write(anchorHash);
        return SHA256.HashData(payload.ToArray());
    }

    private static byte[] TonRouteConfigurationHash(
        SccpNetworkV1 network,
        byte[] masterCode,
        byte[] walletCode,
        byte[] routeCode,
        byte[] verifierCode,
        byte[] circuit,
        byte[] keyHash,
        byte[] proofProfile,
        IReadOnlyList<byte[]> guardians,
        byte[] semanticHash,
        byte[] anchorHash,
        byte[] binding,
        uint revision)
    {
        using var deployment = new MemoryStream();
        deployment.Write(masterCode);
        deployment.Write(walletCode);
        deployment.Write(routeCode);
        deployment.Write(verifierCode);
        deployment.Write(circuit);
        deployment.Write(keyHash);
        deployment.Write(proofProfile);
        foreach (var guardian in guardians)
        {
            deployment.Write(guardian);
        }
        deployment.Write(semanticHash);
        deployment.Write(anchorHash);
        deployment.Write(binding);
        var deploymentHash = SHA256.HashData(deployment.ToArray());
        using var assetRoute = new MemoryStream();
        WriteVector(assetRoute, "xor"u8);
        WriteVector(assetRoute, "taira_ton_xor"u8);
        WriteUInt32(assetRoute, revision);
        WriteUInt64(assetRoute, 1);
        WriteUInt128(assetRoute, (UInt128)1_000_000_000_000m);
        var assetRouteHash = SHA256.HashData(assetRoute.ToArray());
        var inbound = new SccpLaneIdV1(network, SccpNetworkV1.SoraTaira);
        var outbound = new SccpLaneIdV1(SccpNetworkV1.SoraTaira, network);
        using var payload = new MemoryStream();
        payload.Write("sccp:concrete-route-config:v1"u8);
        payload.WriteByte(1);
        WriteUInt32(payload, 4);
        WriteVector(payload, SccpV1.CanonicalNetworkBytes(network));
        WriteInt32(payload, -239);
        payload.Write(SccpV1.LaneHash(inbound));
        payload.Write(SccpV1.LaneHash(outbound));
        payload.Write(deploymentHash);
        payload.Write(assetRouteHash);
        return SHA256.HashData(payload.ToArray());
    }

    private static (
        byte[] StatementHash,
        byte[] RequestHash,
        IReadOnlyList<byte[]> PublicSignals) TonProofRequestHashes(
        SccpCanonicalMessageBundleV1 bundle,
        byte[] bundleBytes,
        byte[] finalityHash,
        byte[] keyBytes,
        byte[] keyHash,
        Dictionary<string, object?> semantic,
        byte[] semanticHash,
        Dictionary<string, object?> anchor,
        byte[] anchorHash,
        byte[] circuit,
        byte[] proofProfile)
    {
        var semanticCommitments = (Dictionary<string, object?>)semantic["commitments"]!;
        var semanticBytes = Concat(
            [1, 1, 1],
            Convert.FromHexString((string)semanticCommitments["circuit_commitment"]!),
            Convert.FromHexString((string)semanticCommitments["witness_generator_commitment"]!),
            Convert.FromHexString((string)semanticCommitments["public_signal_schema_hash"]!));
        var anchorBytes = CanonicalFinalityAnchorBytes(anchor);
        var publicInputs = CanonicalPublicInputsBytes(
            bundle.Commitment.MessageId,
            bundle.Commitment.PayloadHash,
            4,
            bundle.CommitmentRoot,
            9,
            finalityHash);
        var binding = bundle.Commitment.Context.DestinationBindingHash;
        var configuration = bundle.Commitment.Context.RouteConfigurationHash;
        using var statement = new MemoryStream();
        statement.WriteByte(1);
        statement.WriteByte(3);
        WriteVector(statement, SccpV1.CanonicalNetworkBytes(SccpNetworkV1.SoraTaira));
        WriteVector(statement, SccpV1.CanonicalNetworkBytes(SccpNetworkV1.TonMainnet));
        statement.Write(binding);
        statement.Write(configuration);
        statement.Write(circuit);
        statement.Write(keyHash);
        statement.Write(proofProfile);
        statement.Write(semanticHash);
        statement.Write(anchorHash);
        WriteVector(statement, semanticBytes);
        WriteVector(statement, anchorBytes);
        statement.Write(publicInputs);
        WriteVector(statement, bundle.Payload.CanonicalBytes());
        WriteVector(statement, bundleBytes);
        var statementHash = SHA256.HashData(Concat(
            "sccp:groth16-bls12381:statement:v1"u8.ToArray(),
            statement.ToArray()));
        string[] labels =
        [
            "sccp:groth16-bls12381:signal:message-id:v1",
            "sccp:groth16-bls12381:signal:payload-hash:v1",
            "sccp:groth16-bls12381:signal:target-domain:v1",
            "sccp:groth16-bls12381:signal:commitment-root:v1",
            "sccp:groth16-bls12381:signal:finality-height:v1",
            "sccp:groth16-bls12381:signal:finality-block-hash:v1",
            "sccp:groth16-bls12381:signal:source-domain:v1",
            "sccp:groth16-bls12381:signal:statement-hash:v1",
            "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
            "sccp:groth16-bls12381:signal:route-config-hash:v1",
            "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
        ];
        var values = new byte[][]
        {
            bundle.Commitment.MessageId,
            bundle.Commitment.PayloadHash,
            AbiWord(4),
            bundle.CommitmentRoot,
            AbiWord(9),
            finalityHash,
            AbiWord(0),
            statementHash,
            binding,
            configuration,
            anchorHash,
        };
        var signals = labels.Select((label, index) => Bls12381Signal(label, values[index])).ToArray();
        using var request = new MemoryStream();
        request.WriteByte(1);
        request.WriteByte(3);
        WriteVector(request, SccpV1.CanonicalNetworkBytes(SccpNetworkV1.SoraTaira));
        WriteVector(request, SccpV1.CanonicalNetworkBytes(SccpNetworkV1.TonMainnet));
        request.Write(publicInputs);
        foreach (var signal in signals)
        {
            request.Write(signal);
        }
        WriteVector(request, keyBytes);
        WriteVector(request, semanticBytes);
        WriteVector(request, anchorBytes);
        WriteVector(request, bundle.Payload.CanonicalBytes());
        WriteVector(request, bundleBytes);
        request.Write(statementHash);
        request.Write(binding);
        request.Write(configuration);
        request.Write(circuit);
        request.Write(keyHash);
        request.Write(proofProfile);
        request.Write(semanticHash);
        request.Write(anchorHash);
        var requestHash = SHA256.HashData(Concat(
            "sccp:groth16-bls12381:proof-request:v1"u8.ToArray(),
            request.ToArray()));
        return (statementHash, requestHash, signals);
    }

    private static byte[] CanonicalPublicInputsBytes(
        byte[] messageId,
        byte[] payloadHash,
        uint targetDomain,
        byte[] commitmentRoot,
        ulong finalityHeight,
        byte[] finalityHash)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        output.Write(messageId);
        output.Write(payloadHash);
        WriteUInt32(output, targetDomain);
        output.Write(commitmentRoot);
        WriteUInt64(output, finalityHeight);
        output.Write(finalityHash);
        return output.ToArray();
    }

    private static byte[] CanonicalFinalityAnchorBytes(Dictionary<string, object?> anchor)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        output.WriteByte((byte)SccpNetworkV1.SoraTaira);
        Span<byte> protocol = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(
            protocol,
            Convert.ToUInt16(anchor["protocol_version"], System.Globalization.CultureInfo.InvariantCulture));
        output.Write(protocol);
        output.Write(Convert.FromHexString((string)anchor["chain_id_hash"]!));
        WriteUInt64(
            output,
            Convert.ToUInt64(anchor["checkpoint_height"], System.Globalization.CultureInfo.InvariantCulture));
        output.Write(Convert.FromHexString((string)anchor["checkpoint_block_hash"]!));
        output.Write(Convert.FromHexString((string)anchor["checkpoint_context_id"]!));
        output.Write(Convert.FromHexString((string)anchor["checkpoint_finality_artifact_hash"]!));
        return output.ToArray();
    }

    private static byte[] Bls12381Signal(string label, byte[] value)
    {
        var scalar = Convert.FromHexString(
            "73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001");
        var word = SHA256.HashData(Concat(SHA256.HashData(Encoding.UTF8.GetBytes(label)), value));
        while (word.AsSpan().SequenceCompareTo(scalar) >= 0)
        {
            var borrow = 0;
            for (var index = word.Length - 1; index >= 0; index--)
            {
                var difference = word[index] - scalar[index] - borrow;
                if (difference < 0)
                {
                    difference += 256;
                    borrow = 1;
                }
                else
                {
                    borrow = 0;
                }
                word[index] = (byte)difference;
            }
        }
        return word;
    }

    private static byte[] SemanticProfileHash(Dictionary<string, object?> profile)
    {
        var values = (Dictionary<string, object?>)profile["commitments"]!;
        var profileTag = (string)profile["profile"]! ==
            "sora_taira_finality_inclusion_groth16_bls12381"
            ? (byte)1
            : (byte)0;
        return SccpV1.Keccak256(Concat(
            "sccp:semantic-proof-profile:v1"u8.ToArray(),
            [1, profileTag, 1],
            Convert.FromHexString((string)values["circuit_commitment"]!),
            Convert.FromHexString((string)values["witness_generator_commitment"]!),
            Convert.FromHexString((string)values["public_signal_schema_hash"]!)));
    }

    private static byte[] FinalityAnchorHash(Dictionary<string, object?> anchor)
    {
        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        canonical.WriteByte((byte)SccpNetworkV1.SoraTaira);
        Span<byte> protocol = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(
            protocol,
            Convert.ToUInt16(anchor["protocol_version"], System.Globalization.CultureInfo.InvariantCulture));
        canonical.Write(protocol);
        canonical.Write(Convert.FromHexString((string)anchor["chain_id_hash"]!));
        WriteUInt64(
            canonical,
            Convert.ToUInt64(anchor["checkpoint_height"], System.Globalization.CultureInfo.InvariantCulture));
        canonical.Write(Convert.FromHexString((string)anchor["checkpoint_block_hash"]!));
        canonical.Write(Convert.FromHexString((string)anchor["checkpoint_context_id"]!));
        canonical.Write(Convert.FromHexString((string)anchor["checkpoint_finality_artifact_hash"]!));
        Assert.Equal(140, canonical.Length);
        return SccpV1.Keccak256(Concat("sccp:sora-finality-anchor:v1"u8.ToArray(), canonical.ToArray()));
    }

    private static Dictionary<string, object?> Network(string profile) => new()
    {
        ["network"] = profile.Replace('-', '_'),
        ["profile"] = null,
    };

    private static Dictionary<string, object?> Lane(string source, string target) => new()
    {
        ["source"] = Network(source),
        ["target"] = Network(target),
    };

    private static string PrefixHash(byte value) => "0x" + Lower(value, 32);
    private static string Lower(byte value, int bytes) => string.Concat(Enumerable.Repeat(value.ToString("x2"), bytes));
    private static string Upper(byte value, int bytes) => string.Concat(Enumerable.Repeat(value.ToString("X2"), bytes));

    private static byte[] ResponseJson(
        bool submitted,
        string? txHash,
        string? payload,
        string? signing,
        string backend = "bridge/sccp/native/bsc-parlia-v1") => Json(new Dictionary<string, object?>
        {
            ["submitted"] = submitted,
            ["payload_kind"] = "transfer",
            ["message_id_hex"] = new string('1', 64),
            ["backend"] = backend,
            ["counterparty_domain"] = 2,
            ["counterparty_chain"] = "bsc-mainnet",
            ["route_configuration_hash_hex"] = new string('2', 64),
            ["range_start_height"] = 4,
            ["range_end_height"] = 9,
            ["creation_time_ms"] = 7,
            ["tx_hash_hex"] = txHash,
            ["transaction_payload_b64"] = payload,
            ["signing_message_b64"] = signing,
        });

    private static byte[] CanonicalTransactionPayload(
        ulong creationTimeMs,
        bool legacyOuterBinding = false,
        byte routeHashByte = 0x22,
        bool destinationProof = false,
        uint? payloadKindOverride = null,
        string chainId = "fc56984b-2be7-431d-840e-21514d1883f0",
        FeePaymentIntent? feePayment = null,
        ulong? timeToLiveMilliseconds = DefaultTransactionTimeToLiveMilliseconds,
        uint nativeBackendTag = 1)
    {
        const string submitBridgeProofWireId =
            "iroha.instruction.v1::bridge::SubmitBridgeProof";
        const string submitBridgeProofSchemaName =
            "iroha_data_model::isi::bridge::SubmitBridgeProof";
        var pair = Ed25519KeyPair.FromSeed(Enumerable.Repeat((byte)0x57, 32).ToArray());
        var compactPublicKey = new byte[1 + pair.PublicKey.Length];
        pair.PublicKey.CopyTo(compactPublicKey, 1);

        var authority = Concat(UInt32(0), CompactField(ByteVector(compactPublicKey)));
        var routeConfigurationHash = Enumerable.Repeat(routeHashByte, IrohaHash.Length).ToArray();
        var proofSchema = destinationProof
            ? SccpSubmitValidation.DestinationArtifactSchemaName
            : SccpSubmitValidation.NativeInboundProofSchemaName;
        var embeddedProof = NoritoCodec.Encode(proofSchema, [1], flags: 0x02);
        var typedContainer = Concat(
            CompactField(UInt32(destinationProof ? 0U : nativeBackendTag)),
            destinationProof || !legacyOuterBinding
                ? CompactField(FixedByteArray(routeConfigurationHash))
                : [],
            CompactField(RawByteVector(embeddedProof)));
        var bridgePayload = Concat(
            UInt32(payloadKindOverride ?? (destinationProof ? 3U : 2U)),
            CompactField(typedContainer));
        var range = Concat(CompactField(UInt64(4)), CompactField(UInt64(9)));
        var bridgeProof = Concat(
            CompactField(range),
            legacyOuterBinding ? CompactField(FixedByteArray(routeConfigurationHash)) : [],
            CompactField(bridgePayload));
        var instructionArchive = NoritoCodec.Encode(
            submitBridgeProofSchemaName,
            Concat(
                CompactField(bridgeProof),
                CompactField(destinationProof
                    ? [(byte)0]
                    : Concat([(byte)1], CompactField(ReplayWitnessPayload())))),
            flags: 0x02);
        var instruction = Concat(
            CompactField(CompactString(submitBridgeProofWireId)),
            CompactField(RawByteVector(instructionArchive)));
        var instructions = Concat(UInt64(1), CompactField(instruction));
        var executable = Concat(UInt32(0), CompactField(instructions));
        Span<byte> creation = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(creation, creationTimeMs);
        byte[] ttl = timeToLiveMilliseconds is { } value
            ? Concat([(byte)1], CompactField(UInt64(value)))
            : [0];
        var nonce = new byte[] { 0 };
        var encodedFeePayment = CanonicalFeePayment(
            feePayment ?? FeePaymentIntent.Authority([]),
            authority);
        var metadata = UInt64(0);
        return Concat(
            CompactField(CompactField(CompactString(chainId))),
            CompactField(authority),
            CompactField(creation.ToArray()),
            CompactField(executable),
            CompactField(ttl),
            CompactField(nonce),
            CompactField(encodedFeePayment),
            CompactField(UInt32(1)),
            CompactField(metadata),
            CompactField([0]));
    }

    private static byte[] CanonicalFeePayment(
        FeePaymentIntent feePayment,
        byte[] sponsorController)
    {
        Assert.Empty(feePayment.ChargeLimits);
        var limits = UInt64(0);
        var gasLimit = feePayment.GasLimit is { } gas
            ? Concat([(byte)1], CompactField(UInt64(gas)))
            : [(byte)0];
        byte[] value;
        uint payer;
        switch (feePayment)
        {
            case AuthorityFeePaymentIntent:
                payer = 0;
                value = Concat(CompactField(limits), CompactField(gasLimit));
                break;
            case SponsorFeePaymentIntent sponsor:
                payer = 1;
                var programId = Concat(
                    CompactField(sponsorController),
                    CompactField(CompactString(sponsor.ProgramId.Name)));
                value = Concat(
                    CompactField(programId),
                    CompactField(UInt64(sponsor.ProgramRevision)),
                    CompactField(limits),
                    CompactField(gasLimit));
                break;
            default:
                throw new InvalidOperationException("unknown fee payment intent");
        }

        return Concat(UInt32(payer), CompactField(value));
    }

    private static byte[] CompactString(string value) =>
        Concat(CompactLength(checked((ulong)Encoding.UTF8.GetByteCount(value))), Encoding.UTF8.GetBytes(value));

    private static byte[] CompactField(ReadOnlySpan<byte> value) =>
        Concat(CompactLength(checked((ulong)value.Length)), value.ToArray());

    private static byte[] RawByteVector(ReadOnlySpan<byte> value) =>
        Concat(UInt64(checked((ulong)value.Length)), value.ToArray());

    private static byte[] ByteVector(ReadOnlySpan<byte> value)
    {
        using var output = new MemoryStream();
        output.Write(UInt64(checked((ulong)value.Length)));
        foreach (var item in value)
        {
            output.WriteByte(1);
            output.WriteByte(item);
        }

        return output.ToArray();
    }

    private static byte[] FixedByteArray(ReadOnlySpan<byte> value)
        => value.ToArray();

    private static byte[] CompactLength(ulong value)
    {
        using var output = new MemoryStream();
        do
        {
            var item = (byte)(value & 0x7f);
            value >>= 7;
            if (value != 0)
            {
                item |= 0x80;
            }

            output.WriteByte(item);
        }
        while (value != 0);
        return output.ToArray();
    }

    private static byte[] UInt32(uint value)
    {
        var result = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(result, value);
        return result;
    }

    private static byte[] UInt64(ulong value)
    {
        var result = new byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(result, value);
        return result;
    }

    private static void MutateCompactTransactionField(
        byte[] transaction,
        int fieldIndex,
        Action<byte[]> mutation)
    {
        var offset = 0;
        (int Offset, int Length) field = default;
        for (var index = 0; index <= fieldIndex; index++)
        {
            field = ReadCompactFieldRange(transaction, ref offset);
        }

        var value = transaction.AsSpan(field.Offset, field.Length).ToArray();
        mutation(value);
        if (value.Length != field.Length)
        {
            throw new InvalidOperationException("Transaction test mutation changed a field length.");
        }

        value.CopyTo(transaction, field.Offset);
    }

    private static void MutateFirstTransactionInstructionArchive(
        byte[] transaction,
        Action<byte[]> mutation)
    {
        var offset = 0;
        (int Offset, int Length) executable = default;
        for (var index = 0; index <= 3; index++)
        {
            executable = ReadCompactFieldRange(transaction, ref offset);
        }

        offset = executable.Offset + sizeof(uint);
        var instructions = ReadCompactFieldRange(transaction, ref offset);
        offset = instructions.Offset + sizeof(ulong);
        var instruction = ReadCompactFieldRange(transaction, ref offset);
        offset = instruction.Offset;
        _ = ReadCompactFieldRange(transaction, ref offset);
        var framedPayload = ReadCompactFieldRange(transaction, ref offset);
        var archiveLength = BinaryPrimitives.ReadUInt64LittleEndian(
            transaction.AsSpan(framedPayload.Offset, sizeof(ulong)));
        if (archiveLength > int.MaxValue
            || archiveLength != (ulong)(framedPayload.Length - sizeof(ulong)))
        {
            throw new InvalidOperationException("Transaction fixture has an invalid instruction archive.");
        }

        var archiveOffset = framedPayload.Offset + sizeof(ulong);
        var archive = transaction.AsSpan(archiveOffset, (int)archiveLength).ToArray();
        mutation(archive);
        archive.CopyTo(transaction, archiveOffset);
    }

    private static void RewriteNoritoChecksum(byte[] archive)
    {
        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, 8));
        if (payloadLength > int.MaxValue
            || payloadLength != (ulong)(archive.Length - NoritoHeader.EncodedLength))
        {
            throw new InvalidOperationException("Norito test fixture has an invalid payload length.");
        }

        var checksum = Crc64Ecma.Compute(
            archive.AsSpan(NoritoHeader.EncodedLength, (int)payloadLength));
        BinaryPrimitives.WriteUInt64LittleEndian(archive.AsSpan(31, 8), checksum);
    }

    private static (int Offset, int Length) ReadCompactFieldRange(byte[] value, ref int offset)
    {
        var length = ReadCompactLength(value, ref offset);
        if (length > int.MaxValue || offset > value.Length - (int)length)
        {
            throw new InvalidOperationException("Transaction fixture has a truncated compact field.");
        }

        var result = (offset, (int)length);
        offset += (int)length;
        return result;
    }

    private static ulong ReadCompactLength(byte[] value, ref int offset)
    {
        ulong result = 0;
        var shift = 0;
        while (true)
        {
            if (offset >= value.Length)
            {
                throw new InvalidOperationException("Transaction fixture has a truncated compact length.");
            }

            var item = value[offset++];
            var chunk = item & 0x7f;
            if (shift == 63 && chunk > 1)
            {
                throw new InvalidOperationException("Transaction fixture compact length overflows.");
            }

            result |= (ulong)chunk << shift;
            if ((item & 0x80) == 0)
            {
                return result;
            }

            shift += 7;
            if (shift >= 64)
            {
                throw new InvalidOperationException("Transaction fixture compact length overflows.");
            }
        }
    }

    private static Dictionary<string, object?> DeepClone(Dictionary<string, object?> value) =>
        value.ToDictionary(static item => item.Key, static item => CloneValue(item.Value), StringComparer.Ordinal);

    private static object? CloneValue(object? value) => value switch
    {
        Dictionary<string, object?> objectValue => DeepClone(objectValue),
        object?[] array => array.Select(CloneValue).ToArray(),
        IEnumerable<Dictionary<string, object?>> objects => objects.Select(DeepClone).ToArray(),
        _ => value,
    };

    private static byte[] Json(object value) => JsonSerializer.SerializeToUtf8Bytes(value);

    private static SccpBridgeProofSubmitRequest BridgeProofRequest(
        string authority,
        string destinationProofBase64,
        string? signatureBase64 = null,
        string? transactionPayloadBase64 = null,
        ulong? creationTimeMs = null) => new(
            authority,
            destinationProofBase64,
            BridgeFeePayment,
            signatureBase64,
            transactionPayloadBase64,
            creationTimeMs);

    private static SccpBridgeMessageSubmitRequest BridgeMessageRequest(
        string authority,
        string nativeProofBase64,
        string? signatureBase64 = null,
        string? transactionPayloadBase64 = null,
        ulong? creationTimeMs = null) => new(
            authority,
            nativeProofBase64,
            ReplayWitnessArtifact(),
            BridgeFeePayment,
            signatureBase64,
            transactionPayloadBase64,
            creationTimeMs);

    private static string ReplayWitnessArtifact() => Convert.ToBase64String(
        NoritoCodec.Encode(
            SccpSubmitValidation.ReplayWitnessSchemaName,
            ReplayWitnessPayload(),
            flags: 0x02));

    private static byte[] ReplayWitnessPayload() => Concat(
        CompactField(SccpReplayV1.EmptyHashes()[SccpReplayV1.Depth]),
        CompactField(new byte[32]),
        CompactField(new byte[32]),
        CompactField(UInt64(0)));

    private static byte[] Concat(params byte[][] values)
    {
        var result = new byte[values.Sum(static value => value.Length)];
        var offset = 0;
        foreach (var value in values)
        {
            value.CopyTo(result, offset);
            offset += value.Length;
        }
        return result;
    }

    private static void WriteUInt32(Stream output, uint value)
    {
        Span<byte> bytes = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, value);
        output.Write(bytes);
    }

    private static void WriteInt32(Stream output, int value)
    {
        Span<byte> bytes = stackalloc byte[4];
        BinaryPrimitives.WriteInt32LittleEndian(bytes, value);
        output.Write(bytes);
    }

    private static void WriteUInt64(Stream output, ulong value)
    {
        Span<byte> bytes = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        output.Write(bytes);
    }

    private static void WriteUInt128(Stream output, UInt128 value)
    {
        Span<byte> bytes = stackalloc byte[16];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, (ulong)value);
        BinaryPrimitives.WriteUInt64LittleEndian(bytes[8..], (ulong)(value >> 64));
        output.Write(bytes);
    }

    private static void WriteVector(Stream output, ReadOnlySpan<byte> value)
    {
        WriteUInt32(output, checked((uint)value.Length));
        output.Write(value);
    }

    private sealed class StubHandler(Func<HttpRequestMessage, HttpResponseMessage> handler) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) => Task.FromResult(handler(request));
    }

    private sealed class CountingReadStream(int totalBytes, int maximumChunkBytes) : Stream
    {
        public int TotalBytes { get; } = totalBytes;
        public int BytesRead { get; private set; }
        public bool WasDisposed { get; private set; }

        public override bool CanRead => true;
        public override bool CanSeek => false;
        public override bool CanWrite => false;
        public override long Length => throw new NotSupportedException();
        public override long Position
        {
            get => BytesRead;
            set => throw new NotSupportedException();
        }

        public override int Read(byte[] buffer, int offset, int count)
        {
            var length = Math.Min(Math.Min(count, maximumChunkBytes), TotalBytes - BytesRead);
            if (length <= 0)
            {
                return 0;
            }
            buffer.AsSpan(offset, length).Fill((byte)'x');
            BytesRead += length;
            return length;
        }

        public override int Read(Span<byte> buffer)
        {
            var length = Math.Min(Math.Min(buffer.Length, maximumChunkBytes), TotalBytes - BytesRead);
            if (length <= 0)
            {
                return 0;
            }
            buffer[..length].Fill((byte)'x');
            BytesRead += length;
            return length;
        }

        public override ValueTask<int> ReadAsync(
            Memory<byte> buffer,
            CancellationToken cancellationToken = default)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return ValueTask.FromResult(Read(buffer.Span));
        }

        protected override void Dispose(bool disposing)
        {
            WasDisposed = true;
            base.Dispose(disposing);
        }

        public override void Flush() { }
        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();
        public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();
    }
}

internal static class SccpTestHttpExtensions
{
    internal static HttpResponseMessage WithNoritoContentType(this HttpResponseMessage response)
    {
        response.Content.Headers.ContentType =
            new System.Net.Http.Headers.MediaTypeHeaderValue("application/x-norito");
        return response;
    }

    internal static HttpResponseMessage WithJsonContentType(this HttpResponseMessage response)
    {
        response.Content.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("application/json");
        return response;
    }
}
