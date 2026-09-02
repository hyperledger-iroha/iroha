using System.Buffers.Binary;
using System.Numerics;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Kagemusha;

/// <summary>Exact canonical bare Norito asset-definition identity.</summary>
public sealed class KagemushaAssetDefinitionIdV1 : IEquatable<KagemushaAssetDefinitionIdV1>
{
    private readonly byte[] payload;

    public KagemushaAssetDefinitionIdV1(string literal)
    {
        var exact = TransactionEncodingContext.CanonicalizeAssetDefinitionId(literal);
        payload = EncodeAssetDefinitionPayload(exact);
    }

    public KagemushaAssetDefinitionIdV1(ReadOnlySpan<byte> canonicalPayload)
    {
        KagemushaModelValidation.RequireFixedArchive(canonicalPayload, 16, nameof(canonicalPayload));
        payload = canonicalPayload.ToArray();
    }

    public byte[] CanonicalPayload() => payload.ToArray();
    public bool Equals(KagemushaAssetDefinitionIdV1? other) =>
        other is not null && payload.AsSpan().SequenceEqual(other.payload);
    public override bool Equals(object? obj) => obj is KagemushaAssetDefinitionIdV1 other && Equals(other);
    public override int GetHashCode() => KagemushaModelValidation.ByteHash(payload);

    private static byte[] EncodeAssetDefinitionPayload(string literal)
    {
        const string alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        var zeroCount = literal.TakeWhile(static value => value == '1').Count();
        var decodedLittleEndian = new List<byte> { 0 };
        foreach (var character in literal)
        {
            var digit = alphabet.IndexOf(character);
            if (digit < 0) throw new ArgumentException("Asset definition id is not base58.", nameof(literal));
            var carry = digit;
            for (var index = 0; index < decodedLittleEndian.Count; index++)
            {
                var total = decodedLittleEndian[index] * 58 + carry;
                decodedLittleEndian[index] = (byte)total;
                carry = total >> 8;
            }
            while (carry > 0)
            {
                decodedLittleEndian.Add((byte)carry);
                carry >>= 8;
            }
        }
        var decoded = new byte[zeroCount + decodedLittleEndian.Count];
        for (var index = 0; index < decodedLittleEndian.Count; index++)
            decoded[decoded.Length - 1 - index] = decodedLittleEndian[index];
        if (decoded.Length != 21 || decoded[0] != 1)
            throw new ArgumentException("Asset definition id has an invalid address payload.", nameof(literal));
        var writer = new CanonicalNoritoWriter();
        writer.WriteByteElements(decoded.AsSpan(1, 16));
        return writer.ToArray();
    }
}

/// <summary>Exact canonical bare Norito universal account identity.</summary>
public sealed class KagemushaAccountIdV1 : IEquatable<KagemushaAccountIdV1>
{
    private readonly byte[] payload;

    public KagemushaAccountIdV1(string literal) =>
        payload = new TransactionEncodingContext(literal).EncodeAccountId(literal);

    public KagemushaAccountIdV1(ReadOnlySpan<byte> canonicalPayload)
    {
        KagemushaModelValidation.RequireCanonicalAccountPayload(canonicalPayload, nameof(canonicalPayload));
        payload = canonicalPayload.ToArray();
    }

    public byte[] CanonicalPayload() => payload.ToArray();
    public bool Equals(KagemushaAccountIdV1? other) =>
        other is not null && payload.AsSpan().SequenceEqual(other.payload);
    public override bool Equals(object? obj) => obj is KagemushaAccountIdV1 other && Equals(other);
    public override int GetHashCode() => KagemushaModelValidation.ByteHash(payload);
}

/// <summary>Canonical non-zero marked hash naming one asset-registration incarnation.</summary>
public sealed class KagemushaAssetIncarnationV1 : IEquatable<KagemushaAssetIncarnationV1>
{
    private readonly byte[] value;

    public KagemushaAssetIncarnationV1(ReadOnlySpan<byte> bytes)
    {
        value = KagemushaModelValidation.Raw32(bytes, nameof(bytes));
        if (value.AsSpan(0, 31).IndexOfAnyExcept((byte)0) < 0 && (value[31] & 0xfe) == 0)
            throw new ArgumentException("Asset incarnation cannot be the absence sentinel.", nameof(bytes));
        if ((value[31] & 1) == 0)
            throw new ArgumentException("Asset incarnation must carry the canonical Iroha hash marker.", nameof(bytes));
    }

    public byte[] Bytes() => value.ToArray();
    public bool Equals(KagemushaAssetIncarnationV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is KagemushaAssetIncarnationV1 other && Equals(other);
    public override int GetHashCode() => KagemushaModelValidation.ByteHash(value);
}

/// <summary>Canonical uncompressed SEC1 P-256 hardware authority key.</summary>
public sealed class KagemushaDevicePublicKeyV1 : IEquatable<KagemushaDevicePublicKeyV1>
{
    private readonly byte[] value;

    public KagemushaDevicePublicKeyV1(ReadOnlySpan<byte> sec1Bytes)
    {
        if (sec1Bytes.Length != 65 || sec1Bytes[0] != 4)
            throw new ArgumentException("Device public key must be 65-byte uncompressed SEC1.", nameof(sec1Bytes));
        value = sec1Bytes.ToArray();
    }

    public byte[] Sec1Bytes() => value.ToArray();
    public bool Equals(KagemushaDevicePublicKeyV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is KagemushaDevicePublicKeyV1 other && Equals(other);
    public override int GetHashCode() => KagemushaModelValidation.ByteHash(value);
}

/// <summary>Canonical fixed-width low-S P-256 hardware signature.</summary>
public sealed class KagemushaDeviceSignatureV1 : IEquatable<KagemushaDeviceSignatureV1>
{
    private static readonly BigInteger Order = BigInteger.Parse(
        "0FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551",
        System.Globalization.NumberStyles.HexNumber);
    private readonly byte[] value;

    public KagemushaDeviceSignatureV1(ReadOnlySpan<byte> rawBytes)
    {
        if (rawBytes.Length != 64)
            throw new ArgumentException("Device signature must be fixed-width r || s.", nameof(rawBytes));
        var r = new BigInteger(rawBytes[..32], isUnsigned: true, isBigEndian: true);
        var s = new BigInteger(rawBytes[32..], isUnsigned: true, isBigEndian: true);
        if (r <= 0 || r >= Order || s <= 0 || s > Order / 2)
            throw new ArgumentException("Device signature must be canonical low-S P-256.", nameof(rawBytes));
        value = rawBytes.ToArray();
    }

    public byte[] RawBytes() => value.ToArray();
    public bool Equals(KagemushaDeviceSignatureV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is KagemushaDeviceSignatureV1 other && Equals(other);
    public override int GetHashCode() => KagemushaModelValidation.ByteHash(value);
}

/// <summary>Typed X25519 public key whose monetary validation belongs to the native core.</summary>
public sealed class KagemushaX25519PublicKeyV1 : IEquatable<KagemushaX25519PublicKeyV1>
{
    private readonly byte[] value;

    public KagemushaX25519PublicKeyV1(ReadOnlySpan<byte> bytes)
    {
        value = KagemushaModelValidation.Raw32(bytes, nameof(bytes));
        if (value.All(static item => item == 0))
            throw new ArgumentException("X25519 public key cannot be all zero.", nameof(bytes));
    }

    public byte[] Bytes() => value.ToArray();
    public bool Equals(KagemushaX25519PublicKeyV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is KagemushaX25519PublicKeyV1 other && Equals(other);
    public override int GetHashCode() => KagemushaModelValidation.ByteHash(value);
}

public sealed record KagemushaAggregateStateCommitmentV1(
    ushort Version, ReadOnlyMemory<byte> ReleaseId, NetworkId NetworkId,
    KagemushaAssetDefinitionIdV1 Asset, KagemushaAssetIncarnationV1 AssetIncarnation,
    uint Scale, ReadOnlyMemory<byte> LiabilityPoolId, ReadOnlyMemory<byte> LaneId,
    ReadOnlyMemory<byte> HardwareEpochId, ReadOnlyMemory<byte> KeyReference,
    ReadOnlyMemory<byte> HardwarePolicyId, UInt128 Sequence, ReadOnlyMemory<byte> StateCommitment);

public sealed record KagemushaPastaStateCommitmentV1(ReadOnlyMemory<byte> Eq, ReadOnlyMemory<byte> Ep)
{
    public bool IsZero => Eq.Span.IndexOfAnyExcept((byte)0) < 0 && Ep.Span.IndexOfAnyExcept((byte)0) < 0;
}

/// <summary>Closed paired proof with no public predecessor or successor state.</summary>
public sealed record KagemushaPairedProofV1(
    ushort Version, ReadOnlyMemory<byte> EqProtocolDigest, ReadOnlyMemory<byte> EpProtocolDigest,
    ReadOnlyMemory<byte> SemanticDigest, ReadOnlyMemory<byte> GuardEqCredentialAudit,
    ReadOnlyMemory<byte> GuardEpCredentialAudit, ReadOnlyMemory<byte> EqDeferredAudit,
    ReadOnlyMemory<byte> EpDeferredAudit, ReadOnlyMemory<byte> EqProof,
    ReadOnlyMemory<byte> EpProof, ReadOnlyMemory<byte> EqHistory, ReadOnlyMemory<byte> EpHistory);

public enum KagemushaHardwarePlatformClassV1 : uint
{
    AndroidOemService = 0,
    AppleOemService = 1,
    DedicatedSecureElement = 2,
    OtherQualified = 3,
}

public sealed record KagemushaHardwareProfileV1(
    ushort Version, ushort ProtocolVersion, ReadOnlyMemory<byte> HardwareProfileId,
    ReadOnlyMemory<byte> ProviderId, KagemushaHardwarePlatformClassV1 PlatformClass,
    ReadOnlyMemory<byte> ProductClassDigest, ReadOnlyMemory<byte> FirmwarePolicyDigest,
    ReadOnlyMemory<byte> EnrollmentAttestationVerifierDigest,
    ReadOnlyMemory<byte> AttestationTrustRootsDigest, ReadOnlyMemory<byte> AllowedSuiteCommitment,
    ulong PolicyEpoch, KagemushaDevicePublicKeyV1 GovernanceCredentialPublicKey,
    ushort CapabilityMask, ReadOnlyMemory<byte> QualificationReportDigest,
    ulong ValidFromMilliseconds, ulong ExpiresAtMilliseconds);

public sealed record KagemushaHardwareCredentialV1(
    ushort Version, ReadOnlyMemory<byte> CredentialId, NetworkId NetworkId,
    ReadOnlyMemory<byte> HardwareProfileId, ReadOnlyMemory<byte> SuiteId,
    ReadOnlyMemory<byte> FirmwarePolicyDigest, ulong PolicyEpoch,
    ReadOnlyMemory<byte> LaneCommitment, ReadOnlyMemory<byte> HardwareEpochId,
    ulong HardwareEpochGeneration, KagemushaDevicePublicKeyV1 DevicePublicKey,
    ReadOnlyMemory<byte> DeviceKeyReference, ulong IssuedAtMilliseconds,
    ulong ExpiresAtMilliseconds, KagemushaDeviceSignatureV1 GovernanceSignature);

public sealed record KagemushaPeerCreditContextV1(
    ushort Version, ReadOnlyMemory<byte> RequestDigest,
    KagemushaPastaStateCommitmentV1 SenderBeforeCommitment,
    KagemushaPastaStateCommitmentV1 SenderAfterCommitment,
    ReadOnlyMemory<byte> LifecycleContextDigest, ReadOnlyMemory<byte> RecipientLaneId,
    KagemushaX25519PublicKeyV1 RecipientEncryptionKey, ulong CommittedAtMilliseconds,
    ReadOnlyMemory<byte> HardwareTransitionCommitment);

public sealed record KagemushaCreditOpeningV1(
    ushort Version, ReadOnlyMemory<byte> CreditId, UInt128 Amount,
    ReadOnlyMemory<byte> CreditCommitmentOpening, ReadOnlyMemory<byte> RecipientBindingOpening,
    ReadOnlyMemory<byte> RecoveryNonce);

public enum KagemushaEncryptedCreditPurposeV1 : uint
{
    Mint = 0,
    Peer = 1,
}

public sealed record KagemushaEncryptedCreditAadV1(
    ushort Version, KagemushaEncryptedCreditPurposeV1 Purpose,
    ReadOnlyMemory<byte> ContextDigest, ReadOnlyMemory<byte> IssuanceOrTransitionCommitment,
    ReadOnlyMemory<byte> CreditId, UInt128 Amount);

public sealed record KagemushaEncryptedCreditEnvelopeV1(
    ushort Version, KagemushaX25519PublicKeyV1 EphemeralX25519PublicKey,
    ReadOnlyMemory<byte> Nonce, ReadOnlyMemory<byte> CiphertextAndTag);

public enum KagemushaOperationKindV1 : uint
{
    Bootstrap = 0,
    MintFold = 1,
    SendSplit = 2,
    ReceiveFold = 3,
    RedeemSplit = 4,
    Rotate = 5,
}

public sealed record KagemushaLifecycleBindingV1(
    ushort Version, NetworkId NetworkId, ushort ProtocolVersion,
    ReadOnlyMemory<byte> SuiteId, ReadOnlyMemory<byte> VkDigest,
    ReadOnlyMemory<byte> ReleaseId, KagemushaAssetDefinitionIdV1 Asset,
    KagemushaAssetIncarnationV1 AssetIncarnation, uint Scale,
    ReadOnlyMemory<byte> LiabilityPoolId, ReadOnlyMemory<byte> HardwareProfileId,
    ulong PolicyEpoch, KagemushaOperationKindV1 OperationKind,
    ReadOnlyMemory<byte> RequestId, ReadOnlyMemory<byte> CreditId,
    ReadOnlyMemory<byte> CiphertextDigest);

public abstract record KagemushaCommitEvidenceV1;
public sealed record KagemushaTrustedCommitTimeV1(
    ReadOnlyMemory<byte> TimeEvidenceCommitment) : KagemushaCommitEvidenceV1;
public sealed record KagemushaMonotonicCommitLeaseV1(
    ReadOnlyMemory<byte> LeaseEvidenceCommitment) : KagemushaCommitEvidenceV1;

public sealed record KagemushaOutboxReservationV1(
    ReadOnlyMemory<byte> ReservationId, KagemushaOperationKindV1 OperationKind,
    uint ReservedOutboxBytes, ulong IssuedAtMilliseconds, ulong ExpiresAtMilliseconds);

public sealed record KagemushaHardwareTerminalBodyV1(
    ushort Version, ReadOnlyMemory<byte> CandidateEnvelopeDigest,
    ReadOnlyMemory<byte> LifecycleBindingDigest, ReadOnlyMemory<byte> TransitionNullifier,
    ReadOnlyMemory<byte> OutboxReservationCommitment, KagemushaCommitEvidenceV1 CommitEvidence,
    ReadOnlyMemory<byte> HardwareProfileId, ulong PolicyEpoch,
    ReadOnlyMemory<byte> PrivateSuccessorCommitment,
    ReadOnlyMemory<byte> PrivateJournalCommitment,
    ReadOnlyMemory<byte> PrivateRecoveryCommitment);

public sealed record KagemushaPaymentRequestV1(
    ushort Version, ReadOnlyMemory<byte> ReleaseId, NetworkId NetworkId,
    KagemushaAssetDefinitionIdV1 Asset, KagemushaAssetIncarnationV1 AssetIncarnation,
    uint Scale, ReadOnlyMemory<byte> LiabilityPoolId, KagemushaAccountIdV1 Recipient,
    ReadOnlyMemory<byte> RecipientLaneId, KagemushaX25519PublicKeyV1 RecipientEncryptionKey,
    UInt128 Amount, KagemushaHardwareCredentialV1 HardwareCredential,
    ReadOnlyMemory<byte> RequestId, ulong IssuedAtMilliseconds, ulong ExpiresAtMilliseconds,
    KagemushaDeviceSignatureV1 Signature);

/// <summary>Unlinkable public send statement binding opaque sender predecessor and successor commitments.</summary>
public sealed record KagemushaTransferStatementV1(
    ushort Version, KagemushaLifecycleBindingV1 Lifecycle, UInt128 Amount,
    ReadOnlyMemory<byte> TransitionNullifier, KagemushaPastaStateCommitmentV1 SenderBeforeCommitment,
    KagemushaPastaStateCommitmentV1 SenderAfterCommitment, ReadOnlyMemory<byte> RequestDigest,
    ReadOnlyMemory<byte> RecipientLaneId, KagemushaX25519PublicKeyV1 RecipientEncryptionKey,
    ReadOnlyMemory<byte> CiphertextCommitment, ulong CommittedAtMilliseconds,
    ReadOnlyMemory<byte> HardwareTransitionCommitment);

public sealed record KagemushaPaymentV1(
    ushort Version, KagemushaTransferStatementV1 Statement,
    KagemushaPairedProofV1 Proof, ReadOnlyMemory<byte> EncryptedCredit);

public sealed record KagemushaInboxReceiptV1(
    ushort Version, ReadOnlyMemory<byte> CreditId, ReadOnlyMemory<byte> ReceiptCommitment);

public sealed record KagemushaAcknowledgementV1(
    ushort Version, ReadOnlyMemory<byte> RequestDigest, ReadOnlyMemory<byte> PaymentDigest,
    KagemushaInboxReceiptV1 InboxReceipt, KagemushaDeviceSignatureV1 Signature);

public sealed record KagemushaMintAuthorizationContextV1(
    ushort Version, ReadOnlyMemory<byte> OperationId, ReadOnlyMemory<byte> ReleaseId,
    ReadOnlyMemory<byte> SuiteId, ReadOnlyMemory<byte> VkDigest,
    ReadOnlyMemory<byte> ArtifactManifestDigest, NetworkId NetworkId,
    KagemushaAssetDefinitionIdV1 Asset, KagemushaAssetIncarnationV1 AssetIncarnation,
    uint Scale, ReadOnlyMemory<byte> LiabilityPoolId, UInt128 Amount,
    KagemushaAccountIdV1 Payer, KagemushaAccountIdV1 Recipient,
    ReadOnlyMemory<byte> HardwareCredentialId, ReadOnlyMemory<byte> HardwareProfileId,
    ulong PolicyEpoch, ReadOnlyMemory<byte> RecipientCredentialCommitment,
    ReadOnlyMemory<byte> CreditCommitment, KagemushaX25519PublicKeyV1 RecipientOneTimeKey);

public sealed record KagemushaMintAuthorizationStatementV1(
    ushort Version, KagemushaMintAuthorizationContextV1 Context,
    ReadOnlyMemory<byte> IssuanceCommitment, ReadOnlyMemory<byte> CreditId,
    ReadOnlyMemory<byte> CiphertextDigest);

public sealed record KagemushaMintAuthorizationV1(
    ushort Version, KagemushaMintAuthorizationStatementV1 Statement,
    KagemushaPairedProofV1 Proof);

public sealed record KagemushaMintCreditStatementV1(
    ushort Version, KagemushaLifecycleBindingV1 Lifecycle,
    ReadOnlyMemory<byte> RecipientCredentialCommitment,
    ReadOnlyMemory<byte> AuthorizationContextDigest,
    ReadOnlyMemory<byte> MintAuthorizationDigest, UInt128 Amount,
    ReadOnlyMemory<byte> IssuanceCommitment, KagemushaAccountIdV1 Recipient,
    ReadOnlyMemory<byte> CreditCommitment, ulong MintedAtMilliseconds);

public sealed record KagemushaMintCreditV1(
    ushort Version, KagemushaMintCreditStatementV1 Statement,
    KagemushaPairedProofV1 Proof, ReadOnlyMemory<byte> FinalityCertificateBinding,
    ReadOnlyMemory<byte> FinalityAuthorityHead, ReadOnlyMemory<byte> FinalityGenesisRosterId,
    ReadOnlyMemory<byte> FinalityProofBindingDigest, ReadOnlyMemory<byte> EncryptedCredit,
    ReadOnlyMemory<byte> ArtifactManifestDigest);

public sealed record KagemushaRedemptionStatementV1(
    ushort Version, KagemushaLifecycleBindingV1 Lifecycle, UInt128 Amount,
    KagemushaAccountIdV1 Beneficiary, ReadOnlyMemory<byte> TerminalNullifier,
    KagemushaPastaStateCommitmentV1 SenderBeforeCommitment,
    KagemushaPastaStateCommitmentV1 SenderAfterCommitment,
    ReadOnlyMemory<byte> RedemptionCommitment, ReadOnlyMemory<byte> RedemptionId,
    ulong CommittedAtMilliseconds, ReadOnlyMemory<byte> HardwareTransitionCommitment);

public sealed record KagemushaRedemptionVoucherV1(
    ushort Version, KagemushaRedemptionStatementV1 Statement,
    KagemushaPairedProofV1 Proof);

/// <summary>Canonical chain-facing request for a reserve-backed mint.</summary>
public sealed record KagemushaTopUpRequestV1(
    ushort Version, ReadOnlyMemory<byte> OperationId,
    ReadOnlyMemory<byte> IssuanceCommitment, ReadOnlyMemory<byte> CreditId,
    ReadOnlyMemory<byte> ReleaseId, ReadOnlyMemory<byte> SuiteId,
    ReadOnlyMemory<byte> VkDigest, NetworkId NetworkId,
    KagemushaAssetDefinitionIdV1 Asset, KagemushaAssetIncarnationV1 AssetIncarnation,
    uint Scale, UInt128 Amount, ReadOnlyMemory<byte> LiabilityPoolId,
    KagemushaAccountIdV1 Payer, KagemushaAccountIdV1 Recipient,
    KagemushaHardwareCredentialV1 HardwareCredential,
    ReadOnlyMemory<byte> RecipientCredentialCommitment,
    ReadOnlyMemory<byte> CreditCommitment, KagemushaX25519PublicKeyV1 RecipientOneTimeKey,
    ReadOnlyMemory<byte> EncryptedCredit, ReadOnlyMemory<byte> ArtifactManifestDigest,
    KagemushaMintAuthorizationV1? MintAuthorization);

public sealed record KagemushaRedemptionRequestV1(
    ushort Version, ReadOnlyMemory<byte> OperationId, KagemushaRedemptionVoucherV1 Voucher);

internal static class KagemushaModelValidation
{
    internal static byte[] Fixed32(ReadOnlyMemory<byte> value, string name) => Fixed32(value.Span, name);

    internal static byte[] Fixed32(ReadOnlySpan<byte> value, string name)
    {
        var bytes = Raw32(value, name);
        if (bytes.All(static item => item == 0))
            throw new ArgumentException($"{name} must be nonzero.", name);
        return bytes;
    }

    internal static byte[] Raw32(ReadOnlySpan<byte> value, string name)
    {
        if (value.Length != 32) throw new ArgumentException($"{name} must be exactly 32 bytes.", name);
        return value.ToArray();
    }

    internal static void RequireFixedArchive(ReadOnlySpan<byte> payload, int width, string name)
    {
        if (payload.Length != checked(width * 2))
            throw new ArgumentException($"{name} has an invalid fixed-array length.", name);
        for (var index = 0; index < width; index++)
            if (payload[index * 2] != 1)
                throw new ArgumentException($"{name} is not a canonical fixed-array payload.", name);
    }

    internal static void RequireCanonicalAccountPayload(ReadOnlySpan<byte> payload, string name)
    {
        if (payload.Length is 0 or > 512)
            throw new ArgumentException("Account payload is empty or oversized.", name);
        var reader = new CanonicalNoritoReader(payload, "Kagemusha V1 account", name);
        if (reader.ReadUInt32LittleEndian("controller") != 0)
            throw new ArgumentException("Only canonical single-key accounts are supported.", name);
        var key = new CanonicalNoritoReader(reader.ReadField("publicKey"), "Kagemusha V1 public key", name);
        var count = key.ReadSequenceLength("count");
        if (count is 0 or > 4096) throw new ArgumentException("Account public key is empty or oversized.", name);
        for (ulong index = 0; index < count; index++)
            if (key.ReadField($"item{index}").Length != 1)
                throw new ArgumentException("Account public key byte field is invalid.", name);
        key.RequireEnd();
        reader.RequireEnd();
    }

    internal static int ByteHash(ReadOnlySpan<byte> value) =>
        value.Length >= 4 ? BinaryPrimitives.ReadInt32LittleEndian(value) : value.Length;
}
