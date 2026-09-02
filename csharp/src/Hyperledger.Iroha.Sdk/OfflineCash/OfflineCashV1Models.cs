using System.Buffers.Binary;
using System.Numerics;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.OfflineCash;

/// <summary>Exact canonical bare Norito asset-definition identity.</summary>
public sealed class OfflineCashAssetDefinitionIdV1 : IEquatable<OfflineCashAssetDefinitionIdV1>
{
    private readonly byte[] payload;

    public OfflineCashAssetDefinitionIdV1(string literal)
    {
        var exact = TransactionEncodingContext.CanonicalizeAssetDefinitionId(literal);
        payload = EncodeAssetDefinitionPayload(exact);
    }

    public OfflineCashAssetDefinitionIdV1(ReadOnlySpan<byte> canonicalPayload)
    {
        OfflineCashModelValidation.RequireFixedArchive(canonicalPayload, 16, nameof(canonicalPayload));
        payload = canonicalPayload.ToArray();
    }

    public byte[] CanonicalPayload() => payload.ToArray();
    public bool Equals(OfflineCashAssetDefinitionIdV1? other) =>
        other is not null && payload.AsSpan().SequenceEqual(other.payload);
    public override bool Equals(object? obj) => obj is OfflineCashAssetDefinitionIdV1 other && Equals(other);
    public override int GetHashCode() => OfflineCashModelValidation.ByteHash(payload);

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
public sealed class OfflineCashAccountIdV1 : IEquatable<OfflineCashAccountIdV1>
{
    private readonly byte[] payload;

    public OfflineCashAccountIdV1(string literal) =>
        payload = new TransactionEncodingContext(literal).EncodeAccountId(literal);

    public OfflineCashAccountIdV1(ReadOnlySpan<byte> canonicalPayload)
    {
        OfflineCashModelValidation.RequireCanonicalAccountPayload(canonicalPayload, nameof(canonicalPayload));
        payload = canonicalPayload.ToArray();
    }

    public byte[] CanonicalPayload() => payload.ToArray();
    public bool Equals(OfflineCashAccountIdV1? other) =>
        other is not null && payload.AsSpan().SequenceEqual(other.payload);
    public override bool Equals(object? obj) => obj is OfflineCashAccountIdV1 other && Equals(other);
    public override int GetHashCode() => OfflineCashModelValidation.ByteHash(payload);
}

/// <summary>Canonical non-zero marked hash naming one asset-registration incarnation.</summary>
public sealed class OfflineCashAssetIncarnationV1 : IEquatable<OfflineCashAssetIncarnationV1>
{
    private readonly byte[] value;

    public OfflineCashAssetIncarnationV1(ReadOnlySpan<byte> bytes)
    {
        value = OfflineCashModelValidation.Raw32(bytes, nameof(bytes));
        if (value.AsSpan(0, 31).IndexOfAnyExcept((byte)0) < 0 && (value[31] & 0xfe) == 0)
            throw new ArgumentException("Asset incarnation cannot be the absence sentinel.", nameof(bytes));
        if ((value[31] & 1) == 0)
            throw new ArgumentException("Asset incarnation must carry the canonical Iroha hash marker.", nameof(bytes));
    }

    public byte[] Bytes() => value.ToArray();
    public bool Equals(OfflineCashAssetIncarnationV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is OfflineCashAssetIncarnationV1 other && Equals(other);
    public override int GetHashCode() => OfflineCashModelValidation.ByteHash(value);
}

/// <summary>Canonical uncompressed SEC1 P-256 hardware authority key.</summary>
public sealed class OfflineCashDevicePublicKeyV1 : IEquatable<OfflineCashDevicePublicKeyV1>
{
    private readonly byte[] value;

    public OfflineCashDevicePublicKeyV1(ReadOnlySpan<byte> sec1Bytes)
    {
        if (sec1Bytes.Length != 65 || sec1Bytes[0] != 4)
            throw new ArgumentException("Device public key must be 65-byte uncompressed SEC1.", nameof(sec1Bytes));
        value = sec1Bytes.ToArray();
    }

    public byte[] Sec1Bytes() => value.ToArray();
    public bool Equals(OfflineCashDevicePublicKeyV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is OfflineCashDevicePublicKeyV1 other && Equals(other);
    public override int GetHashCode() => OfflineCashModelValidation.ByteHash(value);
}

/// <summary>Canonical fixed-width low-S P-256 hardware signature.</summary>
public sealed class OfflineCashDeviceSignatureV1 : IEquatable<OfflineCashDeviceSignatureV1>
{
    private static readonly BigInteger Order = BigInteger.Parse(
        "0FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551",
        System.Globalization.NumberStyles.HexNumber);
    private readonly byte[] value;

    public OfflineCashDeviceSignatureV1(ReadOnlySpan<byte> rawBytes)
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
    public bool Equals(OfflineCashDeviceSignatureV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is OfflineCashDeviceSignatureV1 other && Equals(other);
    public override int GetHashCode() => OfflineCashModelValidation.ByteHash(value);
}

/// <summary>Typed X25519 public key whose monetary validation belongs to the native core.</summary>
public sealed class OfflineCashX25519PublicKeyV1 : IEquatable<OfflineCashX25519PublicKeyV1>
{
    private readonly byte[] value;

    public OfflineCashX25519PublicKeyV1(ReadOnlySpan<byte> bytes)
    {
        value = OfflineCashModelValidation.Raw32(bytes, nameof(bytes));
        if (value.All(static item => item == 0))
            throw new ArgumentException("X25519 public key cannot be all zero.", nameof(bytes));
    }

    public byte[] Bytes() => value.ToArray();
    public bool Equals(OfflineCashX25519PublicKeyV1? other) =>
        other is not null && value.AsSpan().SequenceEqual(other.value);
    public override bool Equals(object? obj) => obj is OfflineCashX25519PublicKeyV1 other && Equals(other);
    public override int GetHashCode() => OfflineCashModelValidation.ByteHash(value);
}

public sealed record OfflineCashAggregateStateCommitmentV1(
    ushort Version, ReadOnlyMemory<byte> ReleaseId, NetworkId NetworkId,
    OfflineCashAssetDefinitionIdV1 Asset, OfflineCashAssetIncarnationV1 AssetIncarnation,
    uint Scale, ReadOnlyMemory<byte> LiabilityPoolId, ReadOnlyMemory<byte> LaneId,
    ReadOnlyMemory<byte> HardwareEpochId, ReadOnlyMemory<byte> KeyReference,
    ReadOnlyMemory<byte> HardwarePolicyId, UInt128 Sequence, ReadOnlyMemory<byte> StateCommitment);

public sealed record OfflineCashPastaStateCommitmentV1(ReadOnlyMemory<byte> Eq, ReadOnlyMemory<byte> Ep)
{
    public bool IsZero => Eq.Span.IndexOfAnyExcept((byte)0) < 0 && Ep.Span.IndexOfAnyExcept((byte)0) < 0;
}

/// <summary>Closed paired proof with no public predecessor or successor state.</summary>
public sealed record OfflineCashPairedProofV1(
    ushort Version, ReadOnlyMemory<byte> EqProtocolDigest, ReadOnlyMemory<byte> EpProtocolDigest,
    ReadOnlyMemory<byte> SemanticDigest, ReadOnlyMemory<byte> GuardEqCredentialAudit,
    ReadOnlyMemory<byte> GuardEpCredentialAudit, ReadOnlyMemory<byte> EqDeferredAudit,
    ReadOnlyMemory<byte> EpDeferredAudit, ReadOnlyMemory<byte> EqProof,
    ReadOnlyMemory<byte> EpProof, ReadOnlyMemory<byte> EqHistory, ReadOnlyMemory<byte> EpHistory);

public enum OfflineCashHardwarePlatformClassV1 : uint
{
    AndroidOemService = 0,
    AppleOemService = 1,
    DedicatedSecureElement = 2,
    OtherQualified = 3,
}

public sealed record OfflineCashHardwareProfileV1(
    ushort Version, ushort ProtocolVersion, ReadOnlyMemory<byte> HardwareProfileId,
    ReadOnlyMemory<byte> ProviderId, OfflineCashHardwarePlatformClassV1 PlatformClass,
    ReadOnlyMemory<byte> ProductClassDigest, ReadOnlyMemory<byte> FirmwarePolicyDigest,
    ReadOnlyMemory<byte> EnrollmentAttestationVerifierDigest,
    ReadOnlyMemory<byte> AttestationTrustRootsDigest, ReadOnlyMemory<byte> AllowedSuiteCommitment,
    ulong PolicyEpoch, OfflineCashDevicePublicKeyV1 GovernanceCredentialPublicKey,
    ushort CapabilityMask, ReadOnlyMemory<byte> QualificationReportDigest,
    ulong ValidFromMilliseconds, ulong ExpiresAtMilliseconds);

public sealed record OfflineCashHardwareCredentialV1(
    ushort Version, ReadOnlyMemory<byte> CredentialId, NetworkId NetworkId,
    ReadOnlyMemory<byte> HardwareProfileId, ReadOnlyMemory<byte> SuiteId,
    ReadOnlyMemory<byte> FirmwarePolicyDigest, ulong PolicyEpoch,
    ReadOnlyMemory<byte> LaneCommitment, ReadOnlyMemory<byte> HardwareEpochId,
    ulong HardwareEpochGeneration, OfflineCashDevicePublicKeyV1 DevicePublicKey,
    ReadOnlyMemory<byte> DeviceKeyReference, ulong IssuedAtMilliseconds,
    ulong ExpiresAtMilliseconds, OfflineCashDeviceSignatureV1 GovernanceSignature);

public sealed record OfflineCashAcceptanceIntentV1(
    ushort Version, ReadOnlyMemory<byte> RequestDigest, ReadOnlyMemory<byte> IntentId,
    UInt128 ExactAmount, ReadOnlyMemory<byte> SenderOneTimeCommitment);

public sealed record OfflineCashAcceptanceIntentAuthorizationStatementV1(
    ushort Version, OfflineCashAcceptanceIntentV1 Intent, ReadOnlyMemory<byte> ReleaseId,
    ReadOnlyMemory<byte> SuiteId, ReadOnlyMemory<byte> VkDigest,
    ReadOnlyMemory<byte> ArtifactManifestDigest);

public sealed record OfflineCashAcceptanceIntentAuthorizationV1(
    ushort Version, OfflineCashAcceptanceIntentAuthorizationStatementV1 Statement,
    OfflineCashPairedProofV1 Proof);

public sealed record OfflineCashNoCommitClosureStatementV1(
    ushort Version, ReadOnlyMemory<byte> ReleaseId, ReadOnlyMemory<byte> SuiteId,
    ReadOnlyMemory<byte> VkDigest, ReadOnlyMemory<byte> ArtifactManifestDigest,
    ReadOnlyMemory<byte> SenderHardwareBindingCommitment, ReadOnlyMemory<byte> RequestId,
    ReadOnlyMemory<byte> RequestDigest, ReadOnlyMemory<byte> AcceptanceTicketId,
    ReadOnlyMemory<byte> TicketDigest, ReadOnlyMemory<byte> IntentAuthorizationDigest,
    ReadOnlyMemory<byte> IntentDigest, UInt128 ExactAmount,
    ReadOnlyMemory<byte> SenderOneTimeCommitment, ReadOnlyMemory<byte> RecoveryId,
    ReadOnlyMemory<byte> CancellationNullifier,
    ReadOnlyMemory<byte> EquivalentDeliverySlotCommitment);

public sealed record OfflineCashNoCommitClosureV1(
    ushort Version, OfflineCashNoCommitClosureStatementV1 Statement,
    OfflineCashPaymentRequestV1 Request,
    OfflineCashAcceptanceIntentAuthorizationV1 IntentAuthorization,
    OfflineCashAcceptanceTicketV1 AcceptanceTicket,
    OfflineCashPairedProofV1 Proof);

public sealed record OfflineCashAcceptanceTicketV1(
    ushort Version, NetworkId NetworkId, ReadOnlyMemory<byte> RequestId,
    ReadOnlyMemory<byte> RequestDigest, ReadOnlyMemory<byte> AcceptanceTicketId,
    OfflineCashAssetDefinitionIdV1 Asset, OfflineCashAssetIncarnationV1 AssetIncarnation,
    uint Scale, ReadOnlyMemory<byte> IntentDigest, UInt128 ExactAmount,
    uint ReservedInboxBytes, OfflineCashX25519PublicKeyV1 RecipientOneTimeKey,
    ReadOnlyMemory<byte> HardwareProfileId, ulong PolicyEpoch, ulong IssuedAtMilliseconds,
    ulong ExpiresAtMilliseconds, OfflineCashDeviceSignatureV1 Signature);

public sealed record OfflineCashPeerCreditContextV1(
    ushort Version, ReadOnlyMemory<byte> RequestDigest,
    ReadOnlyMemory<byte> AcceptanceIntentDigest, ReadOnlyMemory<byte> AcceptanceTicketDigest,
    ReadOnlyMemory<byte> LifecycleContextDigest, OfflineCashX25519PublicKeyV1 RecipientOneTimeKey);

public sealed record OfflineCashCreditOpeningV1(
    ushort Version, ReadOnlyMemory<byte> CreditId, UInt128 Amount,
    ReadOnlyMemory<byte> CreditCommitmentOpening, ReadOnlyMemory<byte> RecipientBindingOpening,
    ReadOnlyMemory<byte> RecoveryNonce);

public enum OfflineCashEncryptedCreditPurposeV1 : uint
{
    Mint = 0,
    Peer = 1,
}

public sealed record OfflineCashEncryptedCreditAadV1(
    ushort Version, OfflineCashEncryptedCreditPurposeV1 Purpose,
    ReadOnlyMemory<byte> ContextDigest, ReadOnlyMemory<byte> IssuanceOrTransitionCommitment,
    ReadOnlyMemory<byte> CreditId, UInt128 Amount);

public sealed record OfflineCashEncryptedCreditEnvelopeV1(
    ushort Version, OfflineCashX25519PublicKeyV1 EphemeralX25519PublicKey,
    ReadOnlyMemory<byte> Nonce, ReadOnlyMemory<byte> CiphertextAndTag);

public enum OfflineCashOperationKindV1 : uint
{
    Bootstrap = 0,
    MintFold = 1,
    SendSplit = 2,
    ReceiveFold = 3,
    RedeemSplit = 4,
    SuiteUpgrade = 5,
    Rotate = 6,
}

public sealed record OfflineCashLifecycleBindingV1(
    ushort Version, NetworkId NetworkId, ushort ProtocolVersion,
    ReadOnlyMemory<byte> SuiteId, ReadOnlyMemory<byte> VkDigest,
    ReadOnlyMemory<byte> ReleaseId, OfflineCashAssetDefinitionIdV1 Asset,
    OfflineCashAssetIncarnationV1 AssetIncarnation, uint Scale,
    ReadOnlyMemory<byte> LiabilityPoolId, ReadOnlyMemory<byte> HardwareProfileId,
    ulong PolicyEpoch, OfflineCashOperationKindV1 OperationKind,
    ReadOnlyMemory<byte> RequestId, ReadOnlyMemory<byte> AcceptanceTicketId,
    ReadOnlyMemory<byte> CreditId, ReadOnlyMemory<byte> CiphertextDigest);

public abstract record OfflineCashCommitEvidenceV1;
public sealed record OfflineCashTrustedCommitTimeV1(
    ReadOnlyMemory<byte> TimeEvidenceCommitment) : OfflineCashCommitEvidenceV1;
public sealed record OfflineCashMonotonicCommitLeaseV1(
    ReadOnlyMemory<byte> LeaseEvidenceCommitment) : OfflineCashCommitEvidenceV1;

public sealed record OfflineCashOutboxReservationV1(
    ReadOnlyMemory<byte> ReservationId, OfflineCashOperationKindV1 OperationKind,
    uint ReservedOutboxBytes, ulong IssuedAtMilliseconds, ulong ExpiresAtMilliseconds);

public sealed record OfflineCashHardwareTerminalBodyV1(
    ushort Version, ReadOnlyMemory<byte> CandidateEnvelopeDigest,
    ReadOnlyMemory<byte> LifecycleBindingDigest, ReadOnlyMemory<byte> TransitionNullifier,
    ReadOnlyMemory<byte> OutboxReservationCommitment, OfflineCashCommitEvidenceV1 CommitEvidence,
    ReadOnlyMemory<byte> HardwareProfileId, ulong PolicyEpoch,
    ReadOnlyMemory<byte> PrivateSuccessorCommitment,
    ReadOnlyMemory<byte> PrivateJournalCommitment,
    ReadOnlyMemory<byte> PrivateRecoveryCommitment);

public sealed record OfflineCashCommitCertificateV1(
    ushort Version, ReadOnlyMemory<byte> CertificateId,
    ReadOnlyMemory<byte> CandidateEnvelopeDigest, ReadOnlyMemory<byte> LifecycleBindingDigest,
    ReadOnlyMemory<byte> TransitionNullifier, ReadOnlyMemory<byte> OutboxReservationCommitment,
    OfflineCashCommitEvidenceV1 CommitEvidence, ReadOnlyMemory<byte> HardwareProfileId,
    ulong PolicyEpoch, ReadOnlyMemory<byte> HardwareTerminalCommitment);

public sealed record OfflineCashCommitWrapperProofV1(
    ushort Version, ReadOnlyMemory<byte> EqProtocolDigest, ReadOnlyMemory<byte> EpProtocolDigest,
    ReadOnlyMemory<byte> SemanticDigest, ReadOnlyMemory<byte> CandidateEnvelopeDigest,
    ReadOnlyMemory<byte> CommitCertificateDigest, ReadOnlyMemory<byte> EqDeferredAudit,
    ReadOnlyMemory<byte> EpDeferredAudit, ReadOnlyMemory<byte> EqProof,
    ReadOnlyMemory<byte> EpProof, ReadOnlyMemory<byte> EqHistory, ReadOnlyMemory<byte> EpHistory);

public sealed record OfflineCashPaymentRequestV1(
    ushort Version, ReadOnlyMemory<byte> ReleaseId, NetworkId NetworkId,
    OfflineCashAssetDefinitionIdV1 Asset, OfflineCashAssetIncarnationV1 AssetIncarnation,
    uint Scale, ReadOnlyMemory<byte> LiabilityPoolId, OfflineCashAccountIdV1 Recipient,
    UInt128 Amount, OfflineCashHardwareCredentialV1 HardwareCredential,
    ReadOnlyMemory<byte> RequestId, ulong IssuedAtMilliseconds, ulong ExpiresAtMilliseconds,
    OfflineCashDeviceSignatureV1 Signature);

/// <summary>Unlinkable public send statement with no sender state links.</summary>
public sealed record OfflineCashTransferStatementV1(
    ushort Version, OfflineCashLifecycleBindingV1 Lifecycle, UInt128 Amount,
    ReadOnlyMemory<byte> TransitionNullifier, ReadOnlyMemory<byte> RequestDigest,
    ReadOnlyMemory<byte> AcceptanceTicketDigest, OfflineCashX25519PublicKeyV1 RecipientOneTimeKey,
    ReadOnlyMemory<byte> CiphertextCommitment, OfflineCashCommitEvidenceV1 CommitEvidence);

public sealed record OfflineCashPaymentV1(
    ushort Version, OfflineCashTransferStatementV1 Statement,
    OfflineCashAcceptanceIntentV1 AcceptanceIntent,
    OfflineCashAcceptanceTicketV1 AcceptanceTicket,
    OfflineCashCommitCertificateV1 CommitCertificate,
    OfflineCashCommitWrapperProofV1 Proof, ReadOnlyMemory<byte> EncryptedCredit,
    ReadOnlyMemory<byte> ArtifactManifestDigest);

public sealed record OfflineCashInboxReceiptV1(
    ushort Version, ReadOnlyMemory<byte> CreditId, ReadOnlyMemory<byte> ReceiptCommitment);

public sealed record OfflineCashAcknowledgementV1(
    ushort Version, ReadOnlyMemory<byte> RequestDigest, ReadOnlyMemory<byte> PaymentDigest,
    OfflineCashInboxReceiptV1 InboxReceipt, OfflineCashDeviceSignatureV1 Signature);

public sealed record OfflineCashMintAuthorizationContextV1(
    ushort Version, ReadOnlyMemory<byte> OperationId, ReadOnlyMemory<byte> ReleaseId,
    ReadOnlyMemory<byte> SuiteId, ReadOnlyMemory<byte> VkDigest,
    ReadOnlyMemory<byte> ArtifactManifestDigest, NetworkId NetworkId,
    OfflineCashAssetDefinitionIdV1 Asset, OfflineCashAssetIncarnationV1 AssetIncarnation,
    uint Scale, ReadOnlyMemory<byte> LiabilityPoolId, UInt128 Amount,
    OfflineCashAccountIdV1 Payer, OfflineCashAccountIdV1 Recipient,
    ReadOnlyMemory<byte> HardwareCredentialId, ReadOnlyMemory<byte> HardwareProfileId,
    ulong PolicyEpoch, ReadOnlyMemory<byte> RecipientCredentialCommitment,
    ReadOnlyMemory<byte> CreditCommitment, OfflineCashX25519PublicKeyV1 RecipientOneTimeKey);

public sealed record OfflineCashMintAuthorizationStatementV1(
    ushort Version, OfflineCashMintAuthorizationContextV1 Context,
    ReadOnlyMemory<byte> IssuanceCommitment, ReadOnlyMemory<byte> CreditId,
    ReadOnlyMemory<byte> CiphertextDigest);

public sealed record OfflineCashMintAuthorizationV1(
    ushort Version, OfflineCashMintAuthorizationStatementV1 Statement,
    OfflineCashPairedProofV1 Proof);

public sealed record OfflineCashMintCreditStatementV1(
    ushort Version, OfflineCashLifecycleBindingV1 Lifecycle,
    ReadOnlyMemory<byte> RecipientCredentialCommitment,
    ReadOnlyMemory<byte> AuthorizationContextDigest,
    ReadOnlyMemory<byte> MintAuthorizationDigest, UInt128 Amount,
    ReadOnlyMemory<byte> IssuanceCommitment, OfflineCashAccountIdV1 Recipient,
    ReadOnlyMemory<byte> CreditCommitment, ulong MintedAtMilliseconds);

public sealed record OfflineCashMintCreditV1(
    ushort Version, OfflineCashMintCreditStatementV1 Statement,
    OfflineCashPairedProofV1 Proof, ReadOnlyMemory<byte> FinalityCertificateBinding,
    ReadOnlyMemory<byte> FinalityAuthorityHead, ReadOnlyMemory<byte> FinalityGenesisRosterId,
    ReadOnlyMemory<byte> FinalityProofBindingDigest, ReadOnlyMemory<byte> EncryptedCredit,
    ReadOnlyMemory<byte> ArtifactManifestDigest);

public sealed record OfflineCashRedemptionStatementV1(
    ushort Version, OfflineCashLifecycleBindingV1 Lifecycle, UInt128 Amount,
    OfflineCashAccountIdV1 Beneficiary, ReadOnlyMemory<byte> TerminalNullifier,
    ReadOnlyMemory<byte> RedemptionCommitment, ReadOnlyMemory<byte> RedemptionId,
    OfflineCashCommitEvidenceV1 CommitEvidence);

public sealed record OfflineCashRedemptionVoucherV1(
    ushort Version, OfflineCashRedemptionStatementV1 Statement,
    OfflineCashCommitCertificateV1 CommitCertificate,
    OfflineCashCommitWrapperProofV1 Proof, ReadOnlyMemory<byte> ArtifactManifestDigest);

/// <summary>Canonical chain-facing request for a reserve-backed mint.</summary>
public sealed record OfflineCashTopUpRequestV1(
    ushort Version, ReadOnlyMemory<byte> OperationId,
    ReadOnlyMemory<byte> IssuanceCommitment, ReadOnlyMemory<byte> CreditId,
    ReadOnlyMemory<byte> ReleaseId, ReadOnlyMemory<byte> SuiteId,
    ReadOnlyMemory<byte> VkDigest, NetworkId NetworkId,
    OfflineCashAssetDefinitionIdV1 Asset, OfflineCashAssetIncarnationV1 AssetIncarnation,
    uint Scale, UInt128 Amount, ReadOnlyMemory<byte> LiabilityPoolId,
    OfflineCashAccountIdV1 Payer, OfflineCashAccountIdV1 Recipient,
    OfflineCashHardwareCredentialV1 HardwareCredential,
    ReadOnlyMemory<byte> RecipientCredentialCommitment,
    ReadOnlyMemory<byte> CreditCommitment, OfflineCashX25519PublicKeyV1 RecipientOneTimeKey,
    ReadOnlyMemory<byte> EncryptedCredit, ReadOnlyMemory<byte> ArtifactManifestDigest,
    OfflineCashMintAuthorizationV1? MintAuthorization);

public sealed record OfflineCashRedemptionRequestV1(
    ushort Version, ReadOnlyMemory<byte> OperationId, OfflineCashRedemptionVoucherV1 Voucher);

internal static class OfflineCashModelValidation
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
        var reader = new CanonicalNoritoReader(payload, "Offline Cash V1 account", name);
        if (reader.ReadUInt32LittleEndian("controller") != 0)
            throw new ArgumentException("Only canonical single-key accounts are supported.", name);
        var key = new CanonicalNoritoReader(reader.ReadField("publicKey"), "Offline Cash V1 public key", name);
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
