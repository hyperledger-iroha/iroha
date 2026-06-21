using System.Buffers.Binary;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sccp;

internal static class SccpMessageProofBundles
{
    private const string SourceChainProofEnvelopeSchema = "iroha_sccp::SccpSourceChainProofEnvelopeV1";
    private const string SourceEventDigestPrefixV1 = "sccp:source:event:v1";
    private const int MaxSourceMerkleBranchNodes = 64;
    private const byte NoritoCompactLenFlag = 0x02;
    private const int DomainSora = 0;
    private const int DomainEthereum = 1;
    private const int DomainBsc = 2;
    private const int DomainSolana = 3;
    private const int DomainTon = 4;
    private const int DomainTron = 5;
    private const int CodecTextUtf8 = 1;
    private const int CodecEvmHex = 2;
    private const int CodecSolanaBase58 = 3;
    private const int CodecTonRaw = 4;
    private const int CodecTronBase58Check = 5;
    private const int CodecSoraAssetId = 6;
    private const int Keccak256Rate = 136;
    private const string Base58Alphabet =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    private const string PayloadHashPrefixV1 = "sccp:payload:v1";
    private const string HubLeafPrefixV1 = "sccp:hub:leaf:v1";
    private const string HubNodePrefixV1 = "sccp:hub:node:v1";
    private const string TransferPrefixV1 = "sccp:transfer:v1";
    private const string AssetRegisterPrefixV1 = "sccp:asset:register:v1";
    private const string RouteActivatePrefixV1 = "sccp:route:activate:v1";
    private const string TokenAddPrefixV1 = "sccp:token:add:v1";
    private const string TokenPausePrefixV1 = "sccp:token:pause:v1";
    private const string TokenResumePrefixV1 = "sccp:token:resume:v1";

    private static readonly int[] KeccakRhoOffsets =
    [
        0, 1, 62, 28, 27,
        36, 44, 6, 55, 20,
        3, 10, 43, 25, 39,
        41, 45, 15, 21, 8,
        18, 2, 61, 56, 14,
    ];

    private static readonly ulong[] KeccakRoundConstants =
    [
        0x0000000000000001UL,
        0x0000000000008082UL,
        0x800000000000808aUL,
        0x8000000080008000UL,
        0x000000000000808bUL,
        0x0000000080000001UL,
        0x8000000080008081UL,
        0x8000000000008009UL,
        0x000000000000008aUL,
        0x0000000000000088UL,
        0x0000000080008009UL,
        0x000000008000000aUL,
        0x000000008000808bUL,
        0x800000000000008bUL,
        0x8000000000008089UL,
        0x8000000000008003UL,
        0x8000000000008002UL,
        0x8000000000000080UL,
        0x000000000000800aUL,
        0x800000008000000aUL,
        0x8000000080008081UL,
        0x8000000000008080UL,
        0x0000000080000001UL,
        0x8000000080008008UL,
    ];

    internal sealed record BundleSummary(
        int SourceDomain,
        int TargetDomain,
        string MessageId,
        string PayloadHash,
        string CommitmentRoot,
        byte[] FinalityProofBytes);

    internal static BundleSummary RequireMatchesPublicInputs(
        int targetDomain,
        string messageId,
        string payloadHash,
        string commitmentRoot,
        ulong finalityHeight,
        string finalityBlockHash,
        byte[] bundleBytes,
        byte[] sourceProofBytes)
    {
        ArgumentNullException.ThrowIfNull(bundleBytes);
        ArgumentNullException.ThrowIfNull(sourceProofBytes);

        var summary = DecodeMessageProofBundleSummary(bundleBytes, "bundleBytes");
        if (summary.TargetDomain != targetDomain
            || !string.Equals(summary.MessageId, messageId, StringComparison.Ordinal)
            || !string.Equals(summary.PayloadHash, payloadHash, StringComparison.Ordinal)
            || !string.Equals(summary.CommitmentRoot, commitmentRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException("bundleBytes must match publicInputs", nameof(bundleBytes));
        }

        RequireSourceProofMatchesBundle(summary, finalityHeight, finalityBlockHash, sourceProofBytes);
        return summary;
    }

    private static void RequireSourceProofMatchesBundle(
        BundleSummary summary,
        ulong finalityHeight,
        string finalityBlockHash,
        byte[] sourceProofBytes)
    {
        if (summary.SourceDomain == DomainSora)
        {
            if (sourceProofBytes.Length != 0)
            {
                throw new ArgumentException(
                    "sourceProofBytes must be empty for SORA source bundle",
                    nameof(sourceProofBytes));
            }

            return;
        }

        if (sourceProofBytes.Length == 0)
        {
            throw new ArgumentException(
                "sourceProofBytes required for non-SORA source bundle",
                nameof(sourceProofBytes));
        }

        if (!sourceProofBytes.SequenceEqual(summary.FinalityProofBytes))
        {
            throw new ArgumentException(
                "sourceProofBytes must match bundleBytes finality proof",
                nameof(sourceProofBytes));
        }

        var sourceProof = DecodeSourceChainProofSummary(sourceProofBytes, "sourceProofBytes");
        var normalizedFinalityBlockHash = NormalizeHex32(finalityBlockHash, "publicInputs.finalityBlockHash");
        if (sourceProof.SourceDomain != summary.SourceDomain
            || sourceProof.TargetDomain != summary.TargetDomain
            || !string.Equals(sourceProof.MessageId, summary.MessageId, StringComparison.Ordinal)
            || !string.Equals(sourceProof.PayloadHash, summary.PayloadHash, StringComparison.Ordinal)
            || !string.Equals(sourceProof.CommitmentRoot, summary.CommitmentRoot, StringComparison.Ordinal)
            || sourceProof.FinalityHeight != finalityHeight
            || !string.Equals(sourceProof.FinalityBlockHash, normalizedFinalityBlockHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "sourceProofBytes must match bundleBytes and publicInputs",
                nameof(sourceProofBytes));
        }
    }

    private static BundleSummary DecodeMessageProofBundleSummary(byte[] bundleBytes, string label)
    {
        var offset = 0;
        var version = ReadU8At(bundleBytes, offset, $"{label}.version");
        offset += 1;
        if (version != 1)
        {
            throw new ArgumentException($"{label}.version must be 1", nameof(bundleBytes));
        }

        if (offset + 32 > bundleBytes.Length)
        {
            throw new ArgumentException($"{label}.commitment_root is too short", nameof(bundleBytes));
        }

        var commitmentRoot = ToHex(bundleBytes.AsSpan(offset, 32));
        offset += 32;
        var commitmentVec = ReadCanonicalVec(bundleBytes, offset, $"{label}.commitment");
        offset = commitmentVec.NextOffset;
        var merkleProofVec = ReadCanonicalVec(bundleBytes, offset, $"{label}.merkle_proof");
        offset = merkleProofVec.NextOffset;
        var payloadVec = ReadCanonicalVec(bundleBytes, offset, $"{label}.payload");
        offset = payloadVec.NextOffset;
        var finalityProofVec = ReadCanonicalVec(bundleBytes, offset, $"{label}.finality_proof");
        offset = finalityProofVec.NextOffset;
        RequireExactEnd(offset, bundleBytes, label);

        var payload = DecodePayloadSummary(payloadVec.Bytes, $"{label}.payload");
        var expectedCommitment = CanonicalCommitmentBytes(
            payload.Kind,
            payload.TargetDomain,
            payload.MessageId,
            payload.PayloadHash);
        if (!commitmentVec.Bytes.SequenceEqual(expectedCommitment))
        {
            throw new ArgumentException($"{label}.commitment must match payload", nameof(bundleBytes));
        }

        var commitment = DecodeCommitmentSummary(commitmentVec.Bytes, label);
        if (commitment.KindCode != MessageKindCode(payload.Kind))
        {
            throw new ArgumentException($"{label}.commitment kind must match payload", nameof(bundleBytes));
        }

        var expectedRoot = MerkleRootFromCommitmentBytes(
            commitmentVec.Bytes,
            merkleProofVec.Bytes,
            $"{label}.merkle_proof");
        if (!string.Equals(commitmentRoot, expectedRoot, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{label}.commitment_root must match merkle proof", nameof(bundleBytes));
        }

        return new BundleSummary(
            payload.SourceDomain,
            commitment.TargetDomain,
            commitment.MessageId,
            commitment.PayloadHash,
            commitmentRoot,
            finalityProofVec.Bytes.ToArray());
    }

    private static SourceProofSummary DecodeSourceChainProofSummary(byte[] sourceProofBytes, string label)
    {
        var (payload, flags) = NoritoFramePayload(
            sourceProofBytes,
            NoritoCodec.SchemaHash(SourceChainProofEnvelopeSchema),
            label);
        var reader = new NoritoReader(payload, flags);
        var version = ReadNoritoField(reader, $"{label}.version", child => ReadNoritoU8(child, $"{label}.version"));
        if (version != 1)
        {
            throw new ArgumentException($"{label}.version must be 1", nameof(sourceProofBytes));
        }

        var sourceDomain = ReadNoritoField(reader, $"{label}.source_domain", child => ReadNoritoU32(child, $"{label}.source_domain"));
        var targetDomain = ReadNoritoField(reader, $"{label}.target_domain", child => ReadNoritoU32(child, $"{label}.target_domain"));
        var sourceChain = ReadNoritoField(reader, $"{label}.source_chain", child => ReadNoritoString(child, $"{label}.source_chain"));
        var sourceProofPlan = ReadNoritoField(reader, $"{label}.source_proof_plan", child => ReadNoritoU32(child, $"{label}.source_proof_plan"));
        var finalityModel = ReadNoritoField(reader, $"{label}.finality_model", child => ReadNoritoU32(child, $"{label}.finality_model"));
        var messageId = ReadNoritoField(reader, $"{label}.message_id", child => ToHex(ReadNoritoBytes(child, 32, $"{label}.message_id")));
        var payloadHash = ReadNoritoField(reader, $"{label}.payload_hash", child => ToHex(ReadNoritoBytes(child, 32, $"{label}.payload_hash")));
        var sourceEventDigest = ReadNoritoField(reader, $"{label}.source_event_digest", child => ToHex(ReadNoritoBytes(child, 32, $"{label}.source_event_digest")));
        var commitmentRoot = ReadNoritoField(reader, $"{label}.commitment_root", child => ToHex(ReadNoritoBytes(child, 32, $"{label}.commitment_root")));
        var finalityHeight = ReadNoritoField(reader, $"{label}.finality_height", child => ReadNoritoU64(child, $"{label}.finality_height"));
        var finalityBlockHash = ReadNoritoField(reader, $"{label}.finality_block_hash", child => ToHex(ReadNoritoBytes(child, 32, $"{label}.finality_block_hash")));
        var finalizedHeaderHash = ReadNoritoField(reader, $"{label}.finalized_header_hash", child => ToHex(ReadNoritoBytes(child, 32, $"{label}.finalized_header_hash")));
        var receiptOrMessageRoot = ReadNoritoField(reader, $"{label}.receipt_or_message_root", child => ToHex(ReadNoritoBytes(child, 32, $"{label}.receipt_or_message_root")));
        var consensusProof = ReadNoritoField(reader, $"{label}.consensus_proof", child => ReadNoritoRawByteVec(child, $"{label}.consensus_proof"));
        var messageInclusionProof = ReadNoritoField(reader, $"{label}.message_inclusion_proof", child => ReadNoritoRawByteVec(child, $"{label}.message_inclusion_proof"));
        var inclusionBranch = ReadNoritoField(reader, $"{label}.inclusion_branch", child => ReadNoritoRawByteVecSequence(child, $"{label}.inclusion_branch"));
        if (reader.Remaining != 0)
        {
            throw new ArgumentException($"{label} must not contain trailing bytes", nameof(sourceProofBytes));
        }

        if (sourceDomain == DomainSora)
        {
            throw new ArgumentException($"{label}.source_domain must not be SORA", nameof(sourceProofBytes));
        }

        RequireSupportedBundleDomain(sourceDomain, $"{label}.source_domain");
        RequireSupportedBundleDomain(targetDomain, $"{label}.target_domain");
        if (sourceDomain == targetDomain)
        {
            throw new ArgumentException($"{label}.target_domain must differ from source_domain", nameof(sourceProofBytes));
        }

        if (!string.Equals(sourceChain, SourceChainKeyForDomain(sourceDomain), StringComparison.Ordinal))
        {
            throw new ArgumentException($"{label}.source_chain must match source_domain", nameof(sourceProofBytes));
        }

        if (sourceProofPlan != SourceProofPlanCodeForDomain(sourceDomain))
        {
            throw new ArgumentException($"{label}.source_proof_plan must match source_domain", nameof(sourceProofBytes));
        }

        if (finalityModel != FinalityModelCodeForDomain(sourceDomain))
        {
            throw new ArgumentException($"{label}.finality_model must match source_domain", nameof(sourceProofBytes));
        }

        if (finalityHeight == 0)
        {
            throw new ArgumentException($"{label}.finality_height must not be zero", nameof(sourceProofBytes));
        }

        RequireNonZeroHex32(messageId, $"{label}.message_id");
        RequireNonZeroHex32(payloadHash, $"{label}.payload_hash");
        RequireNonZeroHex32(sourceEventDigest, $"{label}.source_event_digest");
        RequireNonZeroHex32(commitmentRoot, $"{label}.commitment_root");
        RequireNonZeroHex32(finalityBlockHash, $"{label}.finality_block_hash");
        RequireNonZeroHex32(finalizedHeaderHash, $"{label}.finalized_header_hash");
        RequireNonZeroHex32(receiptOrMessageRoot, $"{label}.receipt_or_message_root");
        if (consensusProof.Length == 0)
        {
            throw new ArgumentException($"{label}.consensus_proof must not be empty", nameof(sourceProofBytes));
        }

        if (messageInclusionProof.Length == 0)
        {
            throw new ArgumentException($"{label}.message_inclusion_proof must not be empty", nameof(sourceProofBytes));
        }

        if (inclusionBranch.Count == 0)
        {
            throw new ArgumentException($"{label}.inclusion_branch must not be empty", nameof(sourceProofBytes));
        }

        if (inclusionBranch.Count > MaxSourceMerkleBranchNodes)
        {
            throw new ArgumentException($"{label}.inclusion_branch is too deep", nameof(sourceProofBytes));
        }

        for (var index = 0; index < inclusionBranch.Count; index++)
        {
            if (inclusionBranch[index].Length != 32)
            {
                throw new ArgumentException($"{label}.inclusion_branch[{index}] must be 32 bytes", nameof(sourceProofBytes));
            }
        }

        if (!string.Equals(
            sourceEventDigest,
            SourceEventDigest(sourceDomain, targetDomain, messageId, payloadHash),
            StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{label}.source_event_digest must match source domains and message",
                nameof(sourceProofBytes));
        }

        return new SourceProofSummary(
            sourceDomain,
            targetDomain,
            messageId,
            payloadHash,
            commitmentRoot,
            finalityHeight,
            finalityBlockHash);
    }

    private static PayloadSummary DecodePayloadSummary(byte[] payloadBytes, string label)
    {
        if (payloadBytes.Length < 2)
        {
            throw new ArgumentException($"{label} is too short", nameof(payloadBytes));
        }

        var discriminant = ReadU8At(payloadBytes, 0, $"{label}.kind");
        var body = payloadBytes[1..];
        var version = ReadU8At(body, 0, $"{label}.version");
        if (version != 1)
        {
            throw new ArgumentException($"{label}.version must be 1", nameof(payloadBytes));
        }

        var cursor = 1;
        int ReadDomain(string field)
        {
            var domain = ReadU32LeAt(body, cursor, $"{label}.{field}");
            cursor += 4;
            RequireSupportedBundleDomain(domain, $"{label}.{field}");
            return domain;
        }

        void ReadU64(string field)
        {
            _ = ReadU64LeAt(body, cursor, $"{label}.{field}");
            cursor += 8;
        }

        int ReadCodec(string field)
        {
            var codec = NormalizeCodecId(ReadU8At(body, cursor, $"{label}.{field}"), $"{label}.{field}");
            cursor += 1;
            return codec;
        }

        void ReadCodecValue(int codec, string field)
        {
            var value = ReadCanonicalVec(body, cursor, $"{label}.{field}");
            cursor = value.NextOffset;
            ValidateCodecBytes(codec, value.Bytes, $"{label}.{field}");
        }

        PayloadSummary Summary(string kind, int sourceDomain, int targetDomain, string prefix)
            => new(
                kind,
                sourceDomain,
                targetDomain,
                ToHex(PrefixedKeccakBytes(prefix, body)),
                ToHex(PrefixedHashBytes(PayloadHashPrefixV1, payloadBytes)));

        switch (discriminant)
        {
            case 0:
            {
                var targetDomain = ReadDomain("target_domain");
                var sourceDomain = ReadDomain("home_domain");
                ReadU64("nonce");
                ReadCodecValue(ReadCodec("asset_id_codec"), "asset_id");
                _ = ReadU8At(body, cursor, $"{label}.decimals");
                cursor += 1;
                RequireExactEnd(cursor, body, label);
                return Summary("AssetRegister", sourceDomain, targetDomain, AssetRegisterPrefixV1);
            }
            case 1:
            {
                var sourceDomain = ReadDomain("source_domain");
                var targetDomain = ReadDomain("target_domain");
                if (sourceDomain == targetDomain)
                {
                    throw new ArgumentException($"{label}.target_domain must differ from source_domain");
                }

                ReadU64("nonce");
                ReadCodecValue(ReadCodec("asset_id_codec"), "asset_id");
                ReadCodecValue(ReadCodec("route_id_codec"), "route_id");
                RequireExactEnd(cursor, body, label);
                return Summary("RouteActivate", sourceDomain, targetDomain, RouteActivatePrefixV1);
            }
            case 2:
            {
                var sourceDomain = ReadDomain("source_domain");
                var targetDomain = ReadDomain("dest_domain");
                if (sourceDomain == targetDomain)
                {
                    throw new ArgumentException($"{label}.dest_domain must differ from source_domain");
                }

                ReadU64("nonce");
                _ = ReadDomain("asset_home_domain");
                ReadCodecValue(ReadCodec("asset_id_codec"), "asset_id");
                var amount = ReadU128LeAt(body, cursor, $"{label}.amount");
                cursor += 16;
                if (amount <= BigInteger.Zero)
                {
                    throw new ArgumentException($"{label}.amount must be greater than zero");
                }

                var senderCodec = ReadCodec("sender_codec");
                if (senderCodec != CounterpartyAccountCodec(sourceDomain))
                {
                    throw new ArgumentException($"{label}.sender_codec must match source_domain");
                }

                ReadCodecValue(senderCodec, "sender");
                var recipientCodec = ReadCodec("recipient_codec");
                if (recipientCodec != CounterpartyAccountCodec(targetDomain))
                {
                    throw new ArgumentException($"{label}.recipient_codec must match dest_domain");
                }

                ReadCodecValue(recipientCodec, "recipient");
                ReadCodecValue(ReadCodec("route_id_codec"), "route_id");
                RequireExactEnd(cursor, body, label);
                return Summary("Transfer", sourceDomain, targetDomain, TransferPrefixV1);
            }
            case 3:
            {
                var targetDomain = ReadDomain("target_domain");
                ReadU64("nonce");
                var assetId = ReadFixed(body, ref cursor, 32, $"{label}.sora_asset_id");
                if (IsAllZero(assetId))
                {
                    throw new ArgumentException($"{label}.sora_asset_id must be non-zero");
                }

                _ = ReadU8At(body, cursor, $"{label}.decimals");
                cursor += 1;
                var name = ReadFixed(body, ref cursor, 32, $"{label}.name");
                if (!FixedAsciiFieldIsNonEmpty(name))
                {
                    throw new ArgumentException($"{label}.name must be non-empty");
                }

                var symbol = ReadFixed(body, ref cursor, 32, $"{label}.symbol");
                if (!FixedAsciiFieldIsNonEmpty(symbol))
                {
                    throw new ArgumentException($"{label}.symbol must be non-empty");
                }

                RequireExactEnd(cursor, body, label);
                return Summary("TokenAdd", DomainSora, targetDomain, TokenAddPrefixV1);
            }
            case 4:
            case 5:
            {
                var targetDomain = ReadDomain("target_domain");
                ReadU64("nonce");
                var assetId = ReadFixed(body, ref cursor, 32, $"{label}.sora_asset_id");
                if (IsAllZero(assetId))
                {
                    throw new ArgumentException($"{label}.sora_asset_id must be non-zero");
                }

                RequireExactEnd(cursor, body, label);
                return discriminant == 4
                    ? Summary("TokenPause", DomainSora, targetDomain, TokenPausePrefixV1)
                    : Summary("TokenResume", DomainSora, targetDomain, TokenResumePrefixV1);
            }
            default:
                throw new ArgumentException($"{label} contains unsupported SCCP payload kind");
        }
    }

    private static CommitmentSummary DecodeCommitmentSummary(byte[] commitmentBytes, string label)
    {
        if (commitmentBytes.Length != 70)
        {
            throw new ArgumentException($"{label}.commitment must be 70 bytes", nameof(commitmentBytes));
        }

        var version = ReadU8At(commitmentBytes, 0, $"{label}.commitment.version");
        if (version != 1)
        {
            throw new ArgumentException($"{label}.commitment.version must be 1", nameof(commitmentBytes));
        }

        return new CommitmentSummary(
            ReadU8At(commitmentBytes, 1, $"{label}.commitment.kind"),
            ReadU32LeAt(commitmentBytes, 2, $"{label}.commitment.target_domain"),
            ToHex(commitmentBytes.AsSpan(6, 32)),
            ToHex(commitmentBytes.AsSpan(38, 32)));
    }

    private static string MerkleRootFromCommitmentBytes(
        byte[] commitmentBytes,
        byte[] merkleProofBytes,
        string label)
    {
        var offset = 0;
        var stepCount = ReadU32LeAt(merkleProofBytes, offset, $"{label}.steps");
        offset += 4;
        var current = PrefixedHashBytes(HubLeafPrefixV1, commitmentBytes);
        for (var index = 0; index < stepCount; index++)
        {
            if (offset + 33 > merkleProofBytes.Length)
            {
                throw new ArgumentException($"{label}.steps[{index}] is too short", nameof(merkleProofBytes));
            }

            var sibling = merkleProofBytes[offset..(offset + 32)];
            offset += 32;
            var siblingIsLeft = ReadU8At(merkleProofBytes, offset, $"{label}.steps[{index}].sibling_is_left");
            offset += 1;
            if (siblingIsLeft is not (0 or 1))
            {
                throw new ArgumentException($"{label}.steps[{index}].sibling_is_left must be 0 or 1");
            }

            current = PrefixedHashBytes(
                HubNodePrefixV1,
                siblingIsLeft == 1 ? Concat(sibling, current) : Concat(current, sibling));
        }

        RequireExactEnd(offset, merkleProofBytes, label);
        return ToHex(current);
    }

    private static byte[] CanonicalCommitmentBytes(
        string kind,
        int targetDomain,
        string messageId,
        string payloadHash)
    {
        using var payload = new MemoryStream();
        payload.WriteByte(1);
        payload.WriteByte((byte)MessageKindCode(kind));
        payload.Write(LeU32(targetDomain));
        payload.Write(Hex32Bytes(messageId, "commitment.messageId"));
        payload.Write(Hex32Bytes(payloadHash, "commitment.payloadHash"));
        return payload.ToArray();
    }

    private static int MessageKindCode(string kind)
        => kind switch
        {
            "Burn" => 0,
            "TokenAdd" => 1,
            "TokenPause" => 2,
            "TokenResume" => 3,
            "AssetRegister" => 4,
            "RouteActivate" => 5,
            "Transfer" => 6,
            _ => throw new ArgumentException("SCCP message kind is unsupported", nameof(kind)),
        };

    private static void RequireSupportedBundleDomain(int domain, string label)
    {
        if (domain is not (DomainSora or DomainEthereum or DomainBsc or DomainSolana or DomainTon or DomainTron))
        {
            throw new ArgumentException($"{label} must be a supported SCCP domain");
        }
    }

    private static int NormalizeCodecId(int value, string label)
    {
        if (value is not (CodecTextUtf8 or CodecEvmHex or CodecSolanaBase58
            or CodecTonRaw or CodecTronBase58Check or CodecSoraAssetId))
        {
            throw new ArgumentException($"{label} codec is unsupported");
        }

        return value;
    }

    private static int CounterpartyAccountCodec(int domain)
        => domain switch
        {
            DomainSora => CodecTextUtf8,
            DomainEthereum or DomainBsc => CodecEvmHex,
            DomainSolana => CodecSolanaBase58,
            DomainTon => CodecTonRaw,
            DomainTron => CodecTronBase58Check,
            _ => throw new ArgumentException("SCCP domain must be supported", nameof(domain)),
        };

    private static string SourceChainKeyForDomain(int domain)
        => domain switch
        {
            DomainSora => "sora",
            DomainEthereum => "eth",
            DomainBsc => "bsc",
            DomainSolana => "sol",
            DomainTon => "ton",
            DomainTron => "tron",
            _ => throw new ArgumentException("SCCP domain must be supported", nameof(domain)),
        };

    private static int SourceProofPlanCodeForDomain(int domain)
        => domain switch
        {
            DomainEthereum => 1,
            DomainBsc => 2,
            DomainSolana => 3,
            DomainTon => 4,
            DomainTron => 5,
            _ => throw new ArgumentException("SCCP source domain must support source proofs", nameof(domain)),
        };

    private static int FinalityModelCodeForDomain(int domain)
        => domain switch
        {
            DomainEthereum => 0,
            DomainBsc => 1,
            DomainSolana => 2,
            DomainTon => 3,
            DomainTron => 4,
            _ => throw new ArgumentException("SCCP source domain must support source proofs", nameof(domain)),
        };

    private static void ValidateCodecBytes(int codec, byte[] raw, string label)
    {
        switch (codec)
        {
            case CodecTextUtf8:
                if (DecodeCanonicalUtf8Bytes(raw, label).Length == 0)
                {
                    throw new ArgumentException($"{label} must not be empty");
                }
                break;
            case CodecEvmHex:
                ValidateCanonicalEvmHexAddress(DecodeCanonicalUtf8Bytes(raw, label), label);
                break;
            case CodecSolanaBase58:
                _ = DecodeBase58Fixed(DecodeCanonicalUtf8Bytes(raw, label), label, 32);
                break;
            case CodecTonRaw:
                ValidateTonRawAddress(DecodeCanonicalUtf8Bytes(raw, label), label);
                break;
            case CodecTronBase58Check:
                _ = TronBase58CheckPayload(DecodeCanonicalUtf8Bytes(raw, label), label);
                break;
            case CodecSoraAssetId:
                if (raw.Length != 32)
                {
                    throw new ArgumentException($"{label} must be 32 bytes");
                }
                break;
            default:
                throw new ArgumentException($"{label} codec is unsupported");
        }
    }

    private static string DecodeCanonicalUtf8Bytes(byte[] raw, string label)
    {
        var text = Encoding.UTF8.GetString(raw);
        if (!Encoding.UTF8.GetBytes(text).SequenceEqual(raw))
        {
            throw new ArgumentException($"{label} must be canonical UTF-8");
        }

        return text;
    }

    private static void ValidateCanonicalEvmHexAddress(string text, string label)
    {
        if (text.Length != 42 || !text.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{label} must be a 0x-prefixed 20-byte EVM address");
        }

        var payload = text[2..];
        if (!payload.All(static c => c is >= '0' and <= '9' or >= 'a' and <= 'f' or >= 'A' and <= 'F'))
        {
            throw new ArgumentException($"{label} must be a 0x-prefixed 20-byte EVM address");
        }

        var checksum = Keccak256(Encoding.UTF8.GetBytes(payload.ToLowerInvariant()));
        for (var index = 0; index < payload.Length; index++)
        {
            var character = payload[index];
            if (character is >= '0' and <= '9')
            {
                continue;
            }

            var checksumByte = checksum[index / 2];
            var checksumNibble = index % 2 == 0 ? checksumByte >> 4 : checksumByte & 0x0f;
            var expected = checksumNibble >= 8
                ? char.ToUpperInvariant(character)
                : char.ToLowerInvariant(character);
            if (character != expected)
            {
                throw new ArgumentException($"{label} must be a canonical EIP-55 EVM address");
            }
        }
    }

    private static void ValidateTonRawAddress(string text, string label)
    {
        var parts = text.Split(':');
        if (parts.Length != 2 || parts[0] != "0")
        {
            throw new ArgumentException($"{label} must be a basechain TON raw address");
        }

        if (parts[1].Length != 64 || !parts[1].All(static c => c is >= '0' and <= '9' or >= 'a' and <= 'f'))
        {
            throw new ArgumentException($"{label} must be a canonical TON raw address");
        }

        var account = HexBytes(parts[1], label);
        if (IsAllZero(account))
        {
            throw new ArgumentException($"{label} must not be zero");
        }
    }

    private static ReadVec ReadCanonicalVec(byte[] raw, int offset, string label)
    {
        var length = ReadU32LeAt(raw, offset, $"{label}.length");
        var start = offset + 4;
        var end = (long)start + length;
        if (end > raw.Length)
        {
            throw new ArgumentException($"{label} is too short", nameof(raw));
        }

        return new ReadVec(raw[start..(int)end], (int)end);
    }

    private static byte[] ReadFixed(byte[] raw, ref int cursor, int length, string label)
    {
        var end = cursor + length;
        if (end > raw.Length)
        {
            throw new ArgumentException($"{label} is too short", nameof(raw));
        }

        var output = raw[cursor..end];
        cursor = end;
        return output;
    }

    private static int ReadU8At(byte[] raw, int offset, string label)
    {
        if (offset + 1 > raw.Length)
        {
            throw new ArgumentException($"{label} is too short", nameof(raw));
        }

        return raw[offset];
    }

    private static int ReadU32LeAt(byte[] raw, int offset, string label)
    {
        if (offset + 4 > raw.Length)
        {
            throw new ArgumentException($"{label} is too short", nameof(raw));
        }

        var value = BinaryPrimitives.ReadUInt32LittleEndian(raw.AsSpan(offset, 4));
        if (value > int.MaxValue)
        {
            throw new ArgumentException($"{label} must fit platform size");
        }

        return (int)value;
    }

    private static BigInteger ReadU64LeAt(byte[] raw, int offset, string label)
    {
        if (offset + 8 > raw.Length)
        {
            throw new ArgumentException($"{label} is too short", nameof(raw));
        }

        return new BigInteger(raw.AsSpan(offset, 8), isUnsigned: true, isBigEndian: false);
    }

    private static BigInteger ReadU128LeAt(byte[] raw, int offset, string label)
    {
        if (offset + 16 > raw.Length)
        {
            throw new ArgumentException($"{label} is too short", nameof(raw));
        }

        return new BigInteger(raw.AsSpan(offset, 16), isUnsigned: true, isBigEndian: false);
    }

    private static void RequireExactEnd(int offset, byte[] raw, string label)
    {
        if (offset != raw.Length)
        {
            throw new ArgumentException($"{label} must not contain trailing bytes", nameof(raw));
        }
    }

    private static bool FixedAsciiFieldIsNonEmpty(byte[] raw)
    {
        var end = Array.IndexOf(raw, (byte)0);
        var limit = end < 0 ? raw.Length : end;
        return raw.AsSpan(0, limit).IndexOfAnyExcept((byte)0) >= 0;
    }

    private static byte[] DecodeBase58Fixed(string value, string field, int byteLength)
    {
        var raw = DecodeBase58(value, field);
        if (raw.Length != byteLength)
        {
            throw new ArgumentException($"{field} must decode to {byteLength} bytes");
        }

        return raw;
    }

    private static byte[] DecodeBase58(string value, string field)
    {
        if (value.Trim() != value || value.Length == 0)
        {
            throw new ArgumentException($"{field} must be canonical base58");
        }

        var numeric = BigInteger.Zero;
        foreach (var character in value)
        {
            var digit = Base58Alphabet.IndexOf(character, StringComparison.Ordinal);
            if (digit < 0)
            {
                throw new ArgumentException($"{field} must be canonical base58");
            }

            numeric = numeric * 58 + digit;
        }

        var encoded = numeric == BigInteger.Zero
            ? Array.Empty<byte>()
            : numeric.ToByteArray(isUnsigned: true, isBigEndian: true);
        var leadingZeroes = value.TakeWhile(static c => c == '1').Count();
        if (leadingZeroes == 0)
        {
            return encoded;
        }

        return Concat(new byte[leadingZeroes], encoded);
    }

    private static byte[] TronBase58CheckPayload(string value, string field)
    {
        var raw = DecodeBase58(value, field);
        if (raw.Length != 25)
        {
            throw new ArgumentException($"{field} must be a TRON Base58Check address");
        }

        var payload = raw[..21];
        if (payload[0] != 0x41)
        {
            throw new ArgumentException($"{field} must be a TRON mainnet address");
        }

        var checksum = SHA256.HashData(SHA256.HashData(payload))[..4];
        if (!raw.AsSpan(21, 4).SequenceEqual(checksum))
        {
            throw new ArgumentException($"{field} must have a valid Base58Check checksum");
        }

        return payload;
    }

    private static byte[] PrefixedKeccakBytes(string prefix, byte[] payload)
        => Keccak256(Concat(Encoding.UTF8.GetBytes(prefix), payload));

    private static byte[] PrefixedHashBytes(string prefix, byte[] payload)
        => Blake2b.Hash256(Concat(Encoding.UTF8.GetBytes(prefix), payload));

    private static string SourceEventDigest(int sourceDomain, int targetDomain, string messageId, string payloadHash)
    {
        using var payload = new MemoryStream();
        payload.WriteByte(1);
        payload.Write(LeU32(sourceDomain));
        payload.Write(LeU32(targetDomain));
        payload.Write(Hex32Bytes(messageId, "sourceProofBytes.message_id"));
        payload.Write(Hex32Bytes(payloadHash, "sourceProofBytes.payload_hash"));
        return ToHex(PrefixedHashBytes(SourceEventDigestPrefixV1, payload.ToArray()));
    }

    private static string NormalizeHex32(string value, string field)
        => ToHex(Hex32Bytes(value, field));

    private static void RequireNonZeroHex32(string value, string field)
    {
        if (IsAllZero(Hex32Bytes(value, field)))
        {
            throw new ArgumentException($"{field} must not be zero");
        }
    }

    private static byte[] Hex32Bytes(string value, string field)
    {
        var body = value.StartsWith("0x", StringComparison.OrdinalIgnoreCase) ? value[2..] : value;
        if (body.Length != 64)
        {
            throw new ArgumentException($"{field} must be 32 bytes");
        }

        return HexBytes(body, field);
    }

    private static byte[] HexBytes(string value, string field)
    {
        if (value.Length % 2 != 0)
        {
            throw new ArgumentException($"{field} must have even hex length");
        }

        var output = new byte[value.Length / 2];
        for (var index = 0; index < output.Length; index++)
        {
            var high = HexDigit(value[index * 2]);
            var low = HexDigit(value[index * 2 + 1]);
            if (high < 0 || low < 0)
            {
                throw new ArgumentException($"{field} must be hex");
            }

            output[index] = (byte)((high << 4) | low);
        }

        return output;
    }

    private static int HexDigit(char character)
        => character switch
        {
            >= '0' and <= '9' => character - '0',
            >= 'a' and <= 'f' => character - 'a' + 10,
            >= 'A' and <= 'F' => character - 'A' + 10,
            _ => -1,
        };

    private static byte[] LeU32(int value)
    {
        var bytes = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, checked((uint)value));
        return bytes;
    }

    private static bool IsAllZero(ReadOnlySpan<byte> value)
        => value.IndexOfAnyExcept((byte)0) < 0;

    private static string ToHex(ReadOnlySpan<byte> value)
        => "0x" + Convert.ToHexString(value).ToLowerInvariant();

    private static (byte[] Payload, byte Flags) NoritoFramePayload(
        byte[] raw,
        ReadOnlySpan<byte> expectedSchemaHash,
        string label)
    {
        if (raw.Length < NoritoHeader.EncodedLength)
        {
            throw new ArgumentException($"{label} must decode as SccpSourceChainProofEnvelopeV1");
        }

        if (!raw.AsSpan(0, 4).SequenceEqual("NRT0"u8)
            || raw[4] != 0
            || raw[5] != 0
            || raw[22] != (byte)NoritoCompression.None
            || !raw.AsSpan(6, 16).SequenceEqual(expectedSchemaHash))
        {
            throw new ArgumentException($"{label} must decode as SccpSourceChainProofEnvelopeV1");
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(raw.AsSpan(23, 8));
        if (payloadLength > int.MaxValue || payloadLength > (ulong)(raw.Length - NoritoHeader.EncodedLength))
        {
            throw new ArgumentException($"{label} payload length is invalid");
        }

        var payloadStart = raw.Length - (int)payloadLength;
        if (payloadStart < NoritoHeader.EncodedLength
            || raw.AsSpan(NoritoHeader.EncodedLength, payloadStart - NoritoHeader.EncodedLength)
                .IndexOfAnyExcept((byte)0) >= 0)
        {
            throw new ArgumentException($"{label} payload length is invalid");
        }

        var payload = raw[payloadStart..];
        var expectedChecksum = BinaryPrimitives.ReadUInt64LittleEndian(raw.AsSpan(31, 8));
        if (Crc64Ecma.Compute(payload) != expectedChecksum)
        {
            throw new ArgumentException($"{label} checksum is invalid");
        }

        var flags = raw[39];
        if ((flags & 0xD8) != 0
            || ((flags & 0x20) != 0 && (flags & 0x06) != 0x06))
        {
            throw new ArgumentException($"{label} uses unsupported Norito flags");
        }

        return (payload, flags);
    }

    private static T ReadNoritoField<T>(NoritoReader reader, string label, Func<NoritoReader, T> read)
    {
        var length = ReadNoritoLength(reader, label, CompactLenActive(reader));
        var child = new NoritoReader(ReadNoritoBytes(reader, length, label), reader.Flags);
        var value = read(child);
        if (child.Remaining != 0)
        {
            throw new ArgumentException($"{label} must not contain trailing bytes");
        }

        return value;
    }

    private static string ReadNoritoString(NoritoReader reader, string label)
    {
        var length = ReadNoritoLength(reader, label, CompactLenActive(reader));
        var raw = ReadNoritoBytes(reader, length, label);
        var value = Encoding.UTF8.GetString(raw);
        if (!Encoding.UTF8.GetBytes(value).SequenceEqual(raw))
        {
            throw new ArgumentException($"{label} must be canonical UTF-8");
        }

        return value;
    }

    private static byte[] ReadNoritoRawByteVec(NoritoReader reader, string label)
    {
        var length = ReadNoritoLength(reader, label, compact: false);
        return ReadNoritoBytes(reader, length, label);
    }

    private static List<byte[]> ReadNoritoRawByteVecSequence(NoritoReader reader, string label)
    {
        var count = ReadNoritoLength(reader, label, compact: false);
        var output = new List<byte[]>(count);
        for (var index = 0; index < count; index++)
        {
            var elementLength = ReadNoritoLength(reader, $"{label}[{index}]", CompactLenActive(reader));
            var child = new NoritoReader(ReadNoritoBytes(reader, elementLength, $"{label}[{index}]"), reader.Flags);
            var value = ReadNoritoRawByteVec(child, $"{label}[{index}]");
            if (child.Remaining != 0)
            {
                throw new ArgumentException($"{label}[{index}] must not contain trailing bytes");
            }

            output.Add(value);
        }

        return output;
    }

    private static int ReadNoritoU8(NoritoReader reader, string label)
    {
        var raw = ReadNoritoBytes(reader, 1, label);
        return raw[0];
    }

    private static int ReadNoritoU32(NoritoReader reader, string label)
    {
        var raw = ReadNoritoBytes(reader, 4, label);
        var value = BinaryPrimitives.ReadUInt32LittleEndian(raw);
        if (value > int.MaxValue)
        {
            throw new ArgumentException($"{label} must fit platform size");
        }

        return (int)value;
    }

    private static ulong ReadNoritoU64(NoritoReader reader, string label)
    {
        var raw = ReadNoritoBytes(reader, 8, label);
        return BinaryPrimitives.ReadUInt64LittleEndian(raw);
    }

    private static byte[] ReadNoritoBytes(NoritoReader reader, int length, string label)
    {
        if (length < 0 || length > reader.Remaining)
        {
            throw new ArgumentException($"{label} is too short");
        }

        var output = reader.Data[reader.Offset..(reader.Offset + length)];
        reader.Offset += length;
        return output;
    }

    private static int ReadNoritoLength(NoritoReader reader, string label, bool compact)
    {
        if (!compact)
        {
            var length = ReadNoritoU64(reader, $"{label}.length");
            if (length > int.MaxValue)
            {
                throw new ArgumentException($"{label} is too large");
            }

            return (int)length;
        }

        var shift = 0;
        ulong value = 0;
        while (shift < 64)
        {
            var current = ReadNoritoU8(reader, $"{label}.length");
            value |= (ulong)(current & 0x7f) << shift;
            if ((current & 0x80) == 0)
            {
                if (value > int.MaxValue)
                {
                    throw new ArgumentException($"{label} is too large");
                }

                return (int)value;
            }

            shift += 7;
        }

        throw new ArgumentException($"{label}.length is invalid");
    }

    private static bool CompactLenActive(NoritoReader reader)
        => (reader.Flags & NoritoCompactLenFlag) != 0;

    private static byte[] Concat(params byte[][] parts)
    {
        var length = parts.Sum(static part => part.Length);
        var output = new byte[length];
        var offset = 0;
        foreach (var part in parts)
        {
            part.CopyTo(output.AsSpan(offset));
            offset += part.Length;
        }

        return output;
    }

    private static byte[] Keccak256(ReadOnlySpan<byte> data)
    {
        var state = new ulong[25];
        var offset = 0;
        while (offset + Keccak256Rate <= data.Length)
        {
            AbsorbKeccakBlock(state, data.Slice(offset, Keccak256Rate));
            KeccakF1600(state);
            offset += Keccak256Rate;
        }

        Span<byte> block = stackalloc byte[Keccak256Rate];
        data[offset..].CopyTo(block);
        block[data.Length - offset] ^= 0x01;
        block[Keccak256Rate - 1] ^= 0x80;
        AbsorbKeccakBlock(state, block);
        KeccakF1600(state);

        var output = new byte[32];
        var written = 0;
        Span<byte> laneBytes = stackalloc byte[8];
        for (var lane = 0; written < output.Length; lane++)
        {
            BinaryPrimitives.WriteUInt64LittleEndian(laneBytes, state[lane]);
            var count = Math.Min(8, output.Length - written);
            laneBytes[..count].CopyTo(output.AsSpan(written));
            written += count;
        }

        return output;
    }

    private static void AbsorbKeccakBlock(ulong[] state, ReadOnlySpan<byte> block)
    {
        for (var lane = 0; lane < Keccak256Rate / 8; lane++)
        {
            state[lane] ^= BinaryPrimitives.ReadUInt64LittleEndian(block.Slice(lane * 8, 8));
        }
    }

    private static void KeccakF1600(ulong[] state)
    {
        Span<ulong> c = stackalloc ulong[5];
        Span<ulong> d = stackalloc ulong[5];
        Span<ulong> b = stackalloc ulong[25];

        foreach (var roundConstant in KeccakRoundConstants)
        {
            for (var x = 0; x < 5; x++)
            {
                c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
            }

            for (var x = 0; x < 5; x++)
            {
                d[x] = c[(x + 4) % 5] ^ RotateLeft(c[(x + 1) % 5], 1);
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] ^= d[x];
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    var sourceIndex = x + 5 * y;
                    var targetIndex = y + 5 * ((2 * x + 3 * y) % 5);
                    b[targetIndex] = RotateLeft(state[sourceIndex], KeccakRhoOffsets[sourceIndex]);
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] =
                        b[x + 5 * y] ^ ((~b[((x + 1) % 5) + 5 * y]) & b[((x + 2) % 5) + 5 * y]);
                }
            }

            state[0] ^= roundConstant;
        }
    }

    private static ulong RotateLeft(ulong value, int amount)
        => amount == 0 ? value : (value << amount) | (value >> (64 - amount));

    private readonly record struct ReadVec(byte[] Bytes, int NextOffset);

    private sealed record PayloadSummary(
        string Kind,
        int SourceDomain,
        int TargetDomain,
        string MessageId,
        string PayloadHash);

    private readonly record struct CommitmentSummary(
        int KindCode,
        int TargetDomain,
        string MessageId,
        string PayloadHash);

    private sealed record SourceProofSummary(
        int SourceDomain,
        int TargetDomain,
        string MessageId,
        string PayloadHash,
        string CommitmentRoot,
        ulong FinalityHeight,
        string FinalityBlockHash);

    private sealed class NoritoReader(byte[] data, byte flags)
    {
        internal byte[] Data { get; } = data;

        internal byte Flags { get; } = flags;

        internal int Offset { get; set; }

        internal int Remaining => Data.Length - Offset;
    }
}
