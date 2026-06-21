using System.Buffers.Binary;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sccp;

internal static class SccpMessageProofBundles
{
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

        if (summary.SourceDomain != DomainSora && sourceProofBytes.Length == 0)
        {
            throw new ArgumentException(
                "sourceProofBytes required for non-SORA source bundle",
                nameof(sourceProofBytes));
        }

        if (summary.SourceDomain != DomainSora
            && !sourceProofBytes.SequenceEqual(summary.FinalityProofBytes))
        {
            throw new ArgumentException(
                "sourceProofBytes must match bundleBytes finality proof",
                nameof(sourceProofBytes));
        }

        return summary;
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
}
