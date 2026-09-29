using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Exact decoders for canonical Norito <c>TransactionPayload</c> fields that
/// Torii draft responses must carry before the SDK returns them for local signing.
/// </summary>
internal static class ToriiSubmitValidation
{
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    /// <summary>
    /// Decodes one canonical <c>AccountId</c> authority payload and returns its
    /// canonical controller bytes.
    /// </summary>
    internal static byte[] RequireCanonicalAuthority(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        var controllerTag = cursor.TakeUInt32("authority.controller");
        byte[] canonicalController;
        switch (controllerTag)
        {
            case 0:
            {
                var publicKey = DecodeByteVector(
                    cursor.TakeField("authority.public_key"),
                    "authority.public_key",
                    ushort.MaxValue + 1);
                RequireCanonicalCompactPublicKey(publicKey, ushort.MaxValue, "authority.public_key");
                canonicalController = CanonicalSingleController(publicKey);
                break;
            }
            case 1:
                canonicalController = RequireCanonicalMultisigPolicy(
                    cursor.TakeField("authority.multisig"));
                break;
            default:
                throw new ArgumentException("Transaction authority uses an unknown controller tag.");
        }

        if (!cursor.IsFinished)
        {
            throw new ArgumentException("Transaction authority contains trailing bytes.");
        }

        return canonicalController;
    }

    /// <summary>Requires an empty canonical transaction metadata map.</summary>
    internal static void RequireEmptyTransactionMetadata(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        var count = cursor.TakeUInt64("metadata.count");
        if (count != 0 || !cursor.IsFinished)
        {
            throw new ArgumentException(
                "Transaction metadata must be empty; fee selection belongs in fee_payment.");
        }
    }

    /// <summary>Requires the signature-bound <c>QueuePlanSynced</c> admission intent.</summary>
    internal static void RequireQueuePlanSyncedAdmissionIntent(ReadOnlySpan<byte> payload)
    {
        if (payload.Length != sizeof(uint)
            || BinaryPrimitives.ReadUInt32LittleEndian(payload) != 1)
        {
            throw new ArgumentException(
                "Transaction admission_intent must be QueuePlanSynced.");
        }
    }

    /// <summary>Requires one canonical authority- or sponsor-paid fee payment intent.</summary>
    internal static void RequireCanonicalTransactionFeePayment(ReadOnlySpan<byte> payload)
    {
        var intent = new CompactTransactionCursor(payload);
        var payer = intent.TakeUInt32("fee_payment.payer");
        var value = new CompactTransactionCursor(intent.TakeField("fee_payment.value"));
        if (!intent.IsFinished)
        {
            throw new ArgumentException("Transaction fee_payment contains trailing bytes.");
        }

        switch (payer)
        {
            case 0:
                break;
            case 1:
            {
                var program = new CompactTransactionCursor(
                    value.TakeField("fee_payment.program_id"));
                _ = RequireCanonicalAuthority(
                    program.TakeField("fee_payment.program_id.sponsor"));
                var programName = DecodeCompactString(
                    program.TakeField("fee_payment.program_id.name"),
                    "fee_payment.program_id.name");
                if (!program.IsFinished
                    || string.IsNullOrEmpty(programName)
                    || !string.Equals(
                        programName.Normalize(NormalizationForm.FormC),
                        programName,
                        StringComparison.Ordinal)
                    || programName.Any(static character =>
                        char.IsWhiteSpace(character)
                        || char.IsControl(character)
                        || character is '@' or '#' or '$' or '/'))
                {
                    throw new ArgumentException(
                        "Sponsor fee_payment program id is not canonical.");
                }

                var programRevision = DecodeFramedUInt64(
                    ref value,
                    "fee_payment.program_revision");
                if (programRevision == 0)
                {
                    throw new ArgumentException(
                        "Sponsor fee_payment program revision must be positive.");
                }
                break;
            }
            default:
                throw new ArgumentException("Transaction fee_payment payer is unknown.");
        }

        RequireCanonicalFeeChargeLimits(
            value.TakeField("fee_payment.charge_limits"));
        _ = DecodeOptionalPositiveUInt64(
            value.TakeField("fee_payment.gas_limit"),
            "fee_payment.gas_limit");
        if (!value.IsFinished)
        {
            throw new ArgumentException("Transaction fee_payment contains trailing bytes.");
        }
    }

    private static byte[] RequireCanonicalMultisigPolicy(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        var versionField = new CompactTransactionCursor(
            cursor.TakeField("authority.multisig.version"));
        var version = versionField.TakeByte("authority.multisig.version.value");
        var thresholdField = new CompactTransactionCursor(
            cursor.TakeField("authority.multisig.threshold"));
        var threshold = thresholdField.TakeUInt16("authority.multisig.threshold.value");
        var members = new CompactTransactionCursor(
            cursor.TakeField("authority.multisig.members"));
        var memberCount = members.TakeUInt64("authority.multisig.members.count");
        if (version != 1 || threshold == 0 || memberCount is 0 or > ushort.MaxValue)
        {
            throw new ArgumentException("Transaction multisig authority has invalid version, threshold, or member count.");
        }
        if (!versionField.IsFinished || !thresholdField.IsFinished)
        {
            throw new ArgumentException("Transaction multisig authority fields are not canonical.");
        }

        ulong totalWeight = 0;
        byte[]? previousMemberSortKey = null;
        var canonicalMembers = new List<(byte[] PublicKey, ushort Weight)>();
        for (var index = 0UL; index < memberCount; index++)
        {
            var member = new CompactTransactionCursor(members.TakeField("authority.multisig.member"));
            var publicKey = DecodeByteVector(
                member.TakeField("authority.multisig.member.public_key"),
                "authority.multisig.member.public_key",
                ushort.MaxValue + 1);
            RequireCanonicalCompactPublicKey(
                publicKey,
                ushort.MaxValue,
                "authority.multisig.member.public_key");
            var memberSortKey = CompactPublicKeySortKey(publicKey);
            var weightField = new CompactTransactionCursor(
                member.TakeField("authority.multisig.member.weight"));
            var weight = weightField.TakeUInt16("authority.multisig.member.weight.value");
            if (weight == 0
                || !weightField.IsFinished
                || !member.IsFinished
                || previousMemberSortKey is not null
                    && previousMemberSortKey.AsSpan().SequenceCompareTo(memberSortKey) >= 0)
            {
                throw new ArgumentException(
                    "Transaction multisig members must be nonzero, unique, and canonically sorted.");
            }

            totalWeight = checked(totalWeight + weight);
            previousMemberSortKey = memberSortKey;
            canonicalMembers.Add((publicKey, weight));
        }

        if (!members.IsFinished || !cursor.IsFinished || totalWeight < threshold)
        {
            throw new ArgumentException("Transaction multisig authority is not canonical.");
        }

        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        canonical.WriteByte(version);
        WriteBigEndian(canonical, threshold);
        WriteBigEndian(canonical, checked((ushort)canonicalMembers.Count));
        foreach (var member in canonicalMembers)
        {
            canonical.WriteByte(CanonicalCurveId(member.PublicKey[0]));
            WriteBigEndian(canonical, member.Weight);
            WriteBigEndian(canonical, checked((ushort)(member.PublicKey.Length - 1)));
            canonical.Write(member.PublicKey.AsSpan(1));
        }

        return canonical.ToArray();
    }

    private static byte[] CanonicalSingleController(byte[] publicKey)
    {
        var keyLength = checked((ushort)(publicKey.Length - 1));
        var extended = keyLength > byte.MaxValue;
        var keyOffset = extended ? 4 : 3;
        var result = new byte[keyOffset + keyLength];
        result[0] = extended ? (byte)2 : (byte)0;
        result[1] = CanonicalCurveId(publicKey[0]);
        if (extended) BinaryPrimitives.WriteUInt16BigEndian(result.AsSpan(2), keyLength);
        else result[2] = (byte)keyLength;
        publicKey.AsSpan(1).CopyTo(result.AsSpan(keyOffset));
        return result;
    }

    private static byte CanonicalCurveId(byte algorithm) => algorithm switch
    {
        0 => 1,
        1 => 4,
        2 => 3,
        3 => 5,
        4 => 2,
        5 => 10,
        6 => 11,
        7 => 12,
        8 => 13,
        9 => 14,
        10 => 15,
        _ => throw new ArgumentException("Transaction public key algorithm is unknown."),
    };

    private static void WriteBigEndian(Stream output, ushort value)
    {
        Span<byte> encoded = stackalloc byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16BigEndian(encoded, value);
        output.Write(encoded);
    }

    private static byte[] DecodeByteVector(
        ReadOnlySpan<byte> payload,
        string field,
        int maximumBytes)
    {
        var cursor = new CompactTransactionCursor(payload);
        var result = DecodeByteVector(ref cursor, field, maximumBytes);
        if (!cursor.IsFinished)
        {
            throw new ArgumentException($"{field} contains trailing bytes.");
        }

        return result;
    }

    private static byte[] DecodeByteVector(
        ref CompactTransactionCursor cursor,
        string field,
        int maximumBytes)
    {
        var count = cursor.TakeUInt64(field);
        if (count == 0 || count > (ulong)maximumBytes)
        {
            throw new ArgumentException($"{field} length is invalid.");
        }

        var result = new byte[checked((int)count)];
        for (var index = 0; index < result.Length; index++)
        {
            if (cursor.TakeCompactLength(field) != 1)
            {
                throw new ArgumentException($"{field} byte element is not canonically framed.");
            }

            result[index] = cursor.TakeByte(field);
        }

        return result;
    }

    private static void RequireCanonicalCompactPublicKey(
        byte[] payload,
        int maximumKeyBytes,
        string field)
    {
        if (payload.Length is < 2
            || payload.Length - 1 > maximumKeyBytes
            || payload[0] > 10
            || payload[0] == 0 && payload.Length != 1 + Ed25519Signer.PublicKeyLength)
        {
            throw new ArgumentException($"{field} is not a closed compact public key.");
        }
    }

    private static byte[] CompactPublicKeySortKey(byte[] payload)
    {
        var algorithm = payload[0] switch
        {
            0 => "ed25519",
            1 => "secp256k1",
            2 => "bls_normal",
            3 => "bls_small",
            4 => "ml-dsa",
            5 => "gost3410-2012-256-paramset-a",
            6 => "gost3410-2012-256-paramset-b",
            7 => "gost3410-2012-256-paramset-c",
            8 => "gost3410-2012-512-paramset-a",
            9 => "gost3410-2012-512-paramset-b",
            10 => "sm2",
            _ => throw new ArgumentException("Transaction public key algorithm is unknown."),
        };
        var prefix = StrictUtf8.GetBytes(algorithm);
        var result = new byte[prefix.Length + payload.Length];
        prefix.CopyTo(result, 0);
        payload.AsSpan(1).CopyTo(result.AsSpan(prefix.Length + 1));
        return result;
    }

    private static ulong DecodeFramedUInt64(ref CompactTransactionCursor cursor, string field)
    {
        var value = cursor.TakeField(field);
        if (value.Length != sizeof(ulong))
        {
            throw new ArgumentException($"{field} is not a canonical UInt64.");
        }

        return BinaryPrimitives.ReadUInt64LittleEndian(value);
    }

    private static void RequireCanonicalFeeChargeLimits(ReadOnlySpan<byte> payload)
    {
        var limits = new CompactTransactionCursor(payload);
        var count = limits.TakeUInt64("fee_payment.charge_limits.count");
        if (count > 2)
        {
            throw new ArgumentException(
                "Transaction fee_payment contains too many charge limits.");
        }

        var previousKind = -1;
        for (var index = 0UL; index < count; index++)
        {
            var limit = new CompactTransactionCursor(
                limits.TakeField("fee_payment.charge_limits.item"));
            var kindBytes = limit.TakeField("fee_payment.charge_limits.item.kind");
            if (kindBytes.Length != sizeof(uint))
            {
                throw new ArgumentException("Fee charge kind is malformed.");
            }

            var kind = BinaryPrimitives.ReadUInt32LittleEndian(kindBytes);
            if (kind > 1 || checked((int)kind) <= previousKind)
            {
                throw new ArgumentException(
                    "Fee charge limits must be unique and ordered nexus before pipeline gas.");
            }

            RequireCanonicalAssetDefinitionAddress(
                limit.TakeField("fee_payment.charge_limits.item.asset_definition_id"));
            RequireCanonicalPositiveQuantity(
                limit.TakeField("fee_payment.charge_limits.item.max_amount"));
            if (!limit.IsFinished)
            {
                throw new ArgumentException("Fee charge limit contains trailing bytes.");
            }

            previousKind = checked((int)kind);
        }

        if (!limits.IsFinished)
        {
            throw new ArgumentException("Fee charge limits contain trailing bytes.");
        }
    }

    private static void RequireCanonicalAssetDefinitionAddress(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        Span<byte> uuid = stackalloc byte[16];
        for (var index = 0; index < uuid.Length; index++)
        {
            if (cursor.TakeCompactLength("fee_payment.asset_definition_id") != 1)
            {
                throw new ArgumentException(
                    "Fee asset definition address is not canonically framed.");
            }
            uuid[index] = cursor.TakeByte("fee_payment.asset_definition_id");
        }

        if (!cursor.IsFinished
            || (uuid[6] >> 4) != 0x4
            || (uuid[8] & 0xc0) != 0x80)
        {
            throw new ArgumentException("Fee asset definition address is not canonical.");
        }
    }

    private static void RequireCanonicalPositiveQuantity(ReadOnlySpan<byte> payload)
    {
        var quantity = new CompactTransactionCursor(payload);
        var encodedMantissa = new CompactTransactionCursor(
            quantity.TakeField("fee_payment.max_amount.mantissa"));
        var mantissaLength = encodedMantissa.TakeUInt32("fee_payment.max_amount.mantissa.length");
        if (mantissaLength is 0 or > 64)
        {
            throw new ArgumentException("Fee maximum mantissa length is invalid.");
        }

        var mantissaBytes = encodedMantissa.TakeExact(
            checked((int)mantissaLength),
            "fee_payment.max_amount.mantissa");
        var scaleBytes = quantity.TakeField("fee_payment.max_amount.scale");
        if (!encodedMantissa.IsFinished
            || scaleBytes.Length != sizeof(uint)
            || !quantity.IsFinished)
        {
            throw new ArgumentException("Fee maximum is malformed.");
        }

        var mantissa = new System.Numerics.BigInteger(
            mantissaBytes,
            isUnsigned: false,
            isBigEndian: false);
        var scale = BinaryPrimitives.ReadUInt32LittleEndian(scaleBytes);
        if (mantissa.Sign <= 0 || scale > NumericV1.MaxScale)
        {
            throw new ArgumentException("Fee maximum must be a positive canonical quantity.");
        }

        NumericV1.QuantityValue canonical;
        try
        {
            canonical = NumericV1.QuantityValue.FromMantissa(mantissa, checked((int)scale));
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException(
                "Fee maximum must be a positive canonical quantity.",
                error);
        }
        if (canonical.Mantissa != mantissa || checked((uint)canonical.Scale) != scale)
        {
            throw new ArgumentException("Fee maximum is not canonically normalized.");
        }
    }

    private static ulong? DecodeOptionalPositiveUInt64(
        ReadOnlySpan<byte> payload,
        string field)
    {
        var option = new CompactTransactionCursor(payload);
        var tag = option.TakeByte(field);
        if (tag == 0)
        {
            if (!option.IsFinished)
            {
                throw new ArgumentException($"{field} None encoding contains trailing bytes.");
            }
            return null;
        }
        if (tag != 1)
        {
            throw new ArgumentException($"{field} option tag is unknown.");
        }

        var value = DecodeFramedUInt64(ref option, field);
        if (value == 0 || !option.IsFinished)
        {
            throw new ArgumentException($"{field} must contain one positive UInt64.");
        }
        return value;
    }

    private static string DecodeCompactString(ReadOnlySpan<byte> payload, string field)
    {
        var cursor = new CompactTransactionCursor(payload);
        var length = cursor.TakeCompactLength(field);
        if (length > int.MaxValue)
        {
            throw new ArgumentException($"{field} length exceeds the runtime bound.");
        }

        var bytes = cursor.TakeExact(checked((int)length), field);
        if (!cursor.IsFinished)
        {
            throw new ArgumentException($"{field} contains trailing bytes.");
        }

        try
        {
            return StrictUtf8.GetString(bytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new ArgumentException($"{field} is not strict UTF-8.", field, error);
        }
    }

    private ref struct CompactTransactionCursor
    {
        private readonly ReadOnlySpan<byte> input;
        private int offset;

        internal CompactTransactionCursor(ReadOnlySpan<byte> input)
        {
            this.input = input;
            offset = 0;
        }

        internal bool IsFinished => offset == input.Length;

        internal byte TakeByte(string field) => TakeExact(1, field)[0];

        internal ushort TakeUInt16(string field) =>
            BinaryPrimitives.ReadUInt16LittleEndian(TakeExact(sizeof(ushort), field));

        internal uint TakeUInt32(string field) =>
            BinaryPrimitives.ReadUInt32LittleEndian(TakeExact(sizeof(uint), field));

        internal ulong TakeUInt64(string field) =>
            BinaryPrimitives.ReadUInt64LittleEndian(TakeExact(sizeof(ulong), field));

        internal ulong TakeCompactLength(string field)
        {
            ulong result = 0;
            var shift = 0;
            while (true)
            {
                var value = TakeByte(field);
                var chunk = value & 0x7f;
                if (shift == 63 && chunk > 1)
                {
                    throw new ArgumentException($"{field} compact length exceeds UInt64.");
                }

                result |= (ulong)chunk << shift;
                if ((value & 0x80) == 0)
                {
                    if (shift > 0 && chunk == 0)
                    {
                        throw new ArgumentException($"{field} compact length is overlong.");
                    }

                    return result;
                }

                shift += 7;
                if (shift >= 64)
                {
                    throw new ArgumentException($"{field} compact length exceeds UInt64.");
                }
            }
        }

        internal ReadOnlySpan<byte> TakeField(string field)
        {
            var length = TakeCompactLength(field);
            if (length > int.MaxValue)
            {
                throw new ArgumentException($"{field} length exceeds the runtime bound.");
            }

            return TakeExact(checked((int)length), field);
        }

        internal ReadOnlySpan<byte> TakeExact(int length, string field)
        {
            if (length < 0 || offset > input.Length - length)
            {
                throw new ArgumentException($"{field} is truncated.");
            }

            var result = input.Slice(offset, length);
            offset += length;
            return result;
        }
    }
}
