using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Kagemusha;

/// <summary>
/// Canonical KAGEMUSHA V1 codecs and non-authoritative shape checks.
/// Monetary verification and every state transition belong to the audited native core.
/// </summary>
public static class Kagemusha
{
    public const ushort WireVersion = 1;
    public const ushort DeviceLifecycleVersion = 1;
    public const string HandoffCapability = "kagemusha_handoff_v1";
    public const string TextPrefix = "kgm1:";
    public const int MaximumAssetScale = 28;
    public const ulong RequestMaximumTtlMilliseconds = 5 * 60 * 1_000;
    public const int MaximumAggregateBytes = 768;
    public const int MaximumRequestBytes = 928;
    public const int MaximumPaymentBytes = 7_552;
    public const int MaximumAcknowledgementBytes = 256;
    public const int MaximumMintAuthorizationBytes = 7_936;
    public const int MaximumMintCreditBytes = 7_936;
    /// <summary>Bound for a canonical operation-16 public command body, before any decoding.</summary>
    public const int MaximumDeviceMintStageCommandBytes = 65_536;
    /// <summary>Bound for a canonical operation-16 public result body, before any decoding.</summary>
    public const int MaximumDeviceMintStageResultBytes = 128;
    public const int MaximumRedemptionVoucherBytes = 7_936;
    public const int MaximumPairedProofBytes = 6_528;
    public const int MaximumPaymentProofBytes = 6_528;
    public const int MaximumRedemptionProofBytes = 6_528;
    public const int MaximumCurrentProofsBytes = 4_990;
    public const int MaximumParityProofBytes = 2_495;
    public const int HistoryAccumulatorBytes = 544;
    public const int MaximumEncryptedCreditBytes = 384;
    public const int MaximumCreditOpeningBytes = 256;
    public const int CreditOpeningCanonicalBytes = 200;
    public const int EncryptedCreditCiphertextAndTagBytes = 216;
    /// <summary>Hard raw-byte cap for the three separately transported messages together.</summary>
    public const int MaximumCompleteExchangeRawBytes = 9_211;
    /// <summary>Hard <c>kgm1:</c> text cap for the three messages together.</summary>
    public const int MaximumCompleteExchangeTextBytes = 12_288;
    public const int MaximumCommitCertificateBytes = 1_024;
    public const int PaymentOutboxMinimumBytes = 25_728;
    public const int RedemptionOutboxMinimumBytes = 26_112;
    public const int MaximumTopUpRequestBytes = 16_384;
    public const int MaximumRedemptionRequestBytes = 8_192;

    private const string Model = "iroha_data_model::kagemusha::kagemusha_v1::";
    private const string DeviceModel = "iroha_data_model::kagemusha::kagemusha_device_v1::";
    private const string DeviceMintStageCommandSchema = DeviceModel + "KagemushaDeviceMintStageCommandV1";
    private const string DeviceMintStageResultSchema = DeviceModel + "KagemushaDeviceMintStageResultV1";
    private const string AggregateSchema = Model + "KagemushaAggregateStateCommitmentV1";
    private const string HardwareProfileSchema = Model + "KagemushaHardwareProfileV1";
    private const string HardwareCredentialSchema = Model + "KagemushaHardwareCredentialV1";
    private const string RequestSchema = Model + "KagemushaPaymentRequestV1";
    private const string LifecycleSchema = Model + "KagemushaLifecycleBindingV1";
    private const string PeerCreditContextSchema = Model + "KagemushaPeerCreditContextV1";
    private const string CreditOpeningSchema = Model + "KagemushaCreditOpeningV1";
    private const string CreditAadSchema = Model + "KagemushaEncryptedCreditAadV1";
    private const string CreditEnvelopeSchema = Model + "KagemushaEncryptedCreditEnvelopeV1";
    private const string PaymentOutputSchema = Model + "KagemushaPaymentOutputV1";
    private const string PaymentSchema = Model + "KagemushaPaymentV1";
    private const string PaymentProofSchema = Model + "KagemushaPaymentProofV1";
    private const string CommitCertificateSchema = Model + "KagemushaCommitCertificateV1";
    private const string RedemptionProofSchema = Model + "KagemushaRedemptionProofV1";
    private const string HardwareTerminalBodySchema = Model + "KagemushaHardwareTerminalBodyV1";
    private const string AcknowledgementSchema = Model + "KagemushaAcknowledgementV1";
    private const string MintAuthorizationSchema = Model + "KagemushaMintAuthorizationV1";
    private const string MintAuthorizationContextSchema = Model + "KagemushaMintAuthorizationContextV1";
    private const string MintAuthorizationStatementSchema = Model + "KagemushaMintAuthorizationStatementV1";
    private const string MintCreditStatementSchema = Model + "KagemushaMintCreditStatementV1";
    private const string MintCreditSchema = Model + "KagemushaMintCreditV1";
    private const string RedemptionStatementSchema = Model + "KagemushaRedemptionStatementV1";
    private const string RedemptionVoucherSchema = Model + "KagemushaRedemptionVoucherV1";
    private const string RedemptionIdPreimageSchema = "iroha.kagemusha.v1.redemption-id-preimage";
    private const string TopUpRequestSchema = "iroha.torii.v1.kagemusha.top_up.request";
    private const string TopUpInstructionSchema =
        "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1";
    private const string RedemptionRequestSchema = "iroha.torii.v1.kagemusha.redeem.request";

    private static readonly byte[] DeviceKeyReferenceDomain = Ascii("iroha:kagemusha:v1:device-key-reference");
    private static readonly byte[] PastaStateCommitmentDomain = Ascii("iroha:kagemusha:v1:pasta-state-commitment");
    private static readonly byte[] LiabilityPoolDomain = Ascii("iroha:kagemusha:v1:liability-pool");
    private static readonly byte[] RequestDigestDomain = Ascii("iroha:kagemusha:v1:payment-request");
    private static readonly byte[] PreparedTransferDigestDomain =
        Ascii("iroha:kagemusha:v1:prepared-transfer");
    private static readonly byte[] CreditIdDomain = Ascii("iroha:kagemusha:v1:credit-id");
    private static readonly byte[] PeerCreditOpeningCommitmentDomain =
        Ascii("iroha:kagemusha:v1:peer-credit-opening-commitment");
    private static readonly byte[] PeerCreditContextDigestDomain =
        Ascii("iroha:kagemusha:v1:peer-credit-context");
    private static readonly byte[] LifecycleDigestDomain = Ascii("iroha:kagemusha:v1:lifecycle-binding");
    private static readonly byte[] PaymentOutputDigestDomain = Ascii("iroha:kagemusha:v1:send-split-statement");
    private static readonly byte[] PaymentDigestDomain = Ascii("iroha:kagemusha:v1:payment");
    private static readonly byte[] CommitCertificateIdDomain = Ascii("iroha:kagemusha:v1:commit-certificate-id");
    private static readonly byte[] CommitCertificateDigestDomain = Ascii("iroha:kagemusha:v1:commit-certificate");
    private static readonly byte[] HardwareTerminalBodyCommitmentDomain =
        Ascii("iroha:kagemusha:v1:hardware-terminal-body");
    private static readonly byte[] OutboxReservationCommitmentDomain = Ascii("iroha:kagemusha:v1:outbox-reservation");
    private static readonly byte[] CiphertextDigestDomain = Ascii("iroha:kagemusha:v1:ciphertext");
    private static readonly byte[] MintAuthorizationContextDigestDomain =
        Ascii("iroha:kagemusha:v1:mint-authorization-context");
    private static readonly byte[] MintAuthorizationStatementDigestDomain =
        Ascii("iroha:kagemusha:v1:mint-authorization-statement");
    private static readonly byte[] MintAuthorizationDigestDomain =
        Ascii("iroha:kagemusha:v1:mint-authorization");
    private static readonly byte[] MintStatementDigestDomain = Ascii("iroha:kagemusha:v1:mint-statement");
    private static readonly byte[] RedemptionIdDomain = Ascii("iroha:kagemusha:v1:redemption-id");
    private static readonly byte[] RedemptionStatementDigestDomain =
        Ascii("iroha:kagemusha:v1:redemption-statement");

    public enum PayloadKind
    {
        PaymentRequest,
        Payment,
        Acknowledgement,
        MintAuthorization,
        MintCredit,
        RedemptionVoucher,
    }

    public static byte[] EncodeAggregateState(KagemushaAggregateStateCommitmentV1 value)
    {
        ValidateAggregate(value);
        return Bounded(Frame(AggregateSchema, EncodeAggregatePayload(value), 16), MaximumAggregateBytes, nameof(value));
    }

    public static KagemushaAggregateStateCommitmentV1 DecodeAggregateState(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumAggregateBytes, AggregateSchema, DecodeAggregatePayload, EncodeAggregateState);

    public static byte[] EncodeHardwareProfile(KagemushaHardwareProfileV1 value)
    {
        ValidateHardwareProfile(value);
        return Bounded(Frame(HardwareProfileSchema, EncodeHardwareProfilePayload(value), 1), 512, nameof(value));
    }

    public static KagemushaHardwareProfileV1 DecodeHardwareProfile(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 512, HardwareProfileSchema, DecodeHardwareProfilePayload, EncodeHardwareProfile);

    public static byte[] EncodeHardwareCredential(KagemushaHardwareCredentialV1 value)
    {
        ValidateHardwareCredential(value);
        return Bounded(Frame(HardwareCredentialSchema, EncodeHardwareCredentialPayload(value), 1), 768, nameof(value));
    }

    public static KagemushaHardwareCredentialV1 DecodeHardwareCredential(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 768, HardwareCredentialSchema,
            bytes => DecodeHardwareCredentialPayload(bytes), EncodeHardwareCredential);

    public static byte[] EncodePaymentRequest(KagemushaPaymentRequestV1 value)
    {
        ValidateRequest(value);
        return Bounded(Frame(RequestSchema, EncodeRequestPayload(value), 16), MaximumRequestBytes, nameof(value));
    }

    public static KagemushaPaymentRequestV1 DecodePaymentRequest(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumRequestBytes, RequestSchema, DecodeRequestPayload, EncodePaymentRequest);

    public static byte[] EncodePayment(
        KagemushaPaymentV1 value,
        KagemushaPaymentRequestV1 request)
    {
        ValidatePayment(value, request);
        return Bounded(Frame(PaymentSchema, EncodePaymentPayload(value), 16), MaximumPaymentBytes, nameof(value));
    }

    public static KagemushaPaymentV1 DecodePayment(
        ReadOnlySpan<byte> archive,
        KagemushaPaymentRequestV1 request) =>
        DecodeExact(archive, MaximumPaymentBytes, PaymentSchema, DecodePaymentPayload,
            value => EncodePayment(value, request));

    public static byte[] EncodeAcknowledgement(
        KagemushaAcknowledgementV1 value,
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment)
    {
        ValidateAcknowledgement(value, request, payment);
        return Bounded(Frame(AcknowledgementSchema, EncodeAcknowledgementPayload(value), 2),
            MaximumAcknowledgementBytes, nameof(value));
    }

    public static KagemushaAcknowledgementV1 DecodeAcknowledgement(
        ReadOnlySpan<byte> archive,
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment) => DecodeExact(
            archive,
            MaximumAcknowledgementBytes,
            AcknowledgementSchema,
            DecodeAcknowledgementPayload,
            value => EncodeAcknowledgement(value, request, payment));

    public static byte[] EncodeMintAuthorization(KagemushaMintAuthorizationV1 value)
    {
        ValidateMintAuthorization(value);
        return Bounded(Frame(MintAuthorizationSchema, EncodeMintAuthorizationPayload(value), 16),
            MaximumMintAuthorizationBytes, nameof(value));
    }

    public static KagemushaMintAuthorizationV1 DecodeMintAuthorization(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumMintAuthorizationBytes, MintAuthorizationSchema,
            DecodeMintAuthorizationPayload, EncodeMintAuthorization);

    public static byte[] EncodeMintCredit(KagemushaMintCreditV1 value)
    {
        ValidateMintCredit(value, null);
        return Bounded(Frame(MintCreditSchema, EncodeMintCreditPayload(value), 16), MaximumMintCreditBytes, nameof(value));
    }

    public static byte[] EncodeMintCredit(
        KagemushaMintCreditV1 value,
        KagemushaMintAuthorizationV1 authorization)
    {
        ValidateMintCredit(value, authorization);
        return Bounded(Frame(MintCreditSchema, EncodeMintCreditPayload(value), 16), MaximumMintCreditBytes, nameof(value));
    }

    public static KagemushaMintCreditV1 DecodeMintCredit(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumMintCreditBytes, MintCreditSchema, DecodeMintCreditPayload, EncodeMintCredit);

    public static KagemushaMintCreditV1 DecodeMintCredit(
        ReadOnlySpan<byte> archive,
        KagemushaMintAuthorizationV1 authorization) => DecodeExact(
            archive,
            MaximumMintCreditBytes,
            MintCreditSchema,
            DecodeMintCreditPayload,
            value => EncodeMintCredit(value, authorization));

    /// <summary>Encode a bounded operation-16 command after exact nested public-shape validation.</summary>
    public static byte[] EncodeDeviceMintStageCommandShape(KagemushaDeviceMintStageCommandV1 value)
    {
        _ = ValidateDeviceMintStageCommandShape(value);
        return Bounded(Frame(DeviceMintStageCommandSchema, Fields(
            U16(value.Version), Vector(value.CanonicalAuthorizationSpan),
            Vector(value.CanonicalMintCreditSpan)), 8), MaximumDeviceMintStageCommandBytes, nameof(value));
    }

    /// <summary>Construct and encode an operation-16 command from exact bound authorization and credit archives.</summary>
    public static byte[] EncodeDeviceMintStageCommandShape(
        ReadOnlySpan<byte> canonicalAuthorization,
        ReadOnlySpan<byte> canonicalMintCredit) => EncodeDeviceMintStageCommandShape(
            new KagemushaDeviceMintStageCommandV1(DeviceLifecycleVersion,
                canonicalAuthorization, canonicalMintCredit));

    /// <summary>Decode one exact bounded canonical operation-16 command; this grants no monetary authority.</summary>
    public static KagemushaDeviceMintStageCommandV1 DecodeDeviceMintStageCommandShapeExact(
        ReadOnlySpan<byte> archive) => DecodeExact(archive, MaximumDeviceMintStageCommandBytes,
            DeviceMintStageCommandSchema, DecodeDeviceMintStageCommandPayload, EncodeDeviceMintStageCommandShape);

    /// <summary>Encode a bounded public result; response authentication and hardware validation are separate.</summary>
    public static byte[] EncodeDeviceMintStageResultShape(KagemushaDeviceMintStageResultV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return Bounded(Frame(DeviceMintStageResultSchema, Fields(U16(value.Version), [value.Disposition],
            value.CreditIdSpan.ToArray()), 2), MaximumDeviceMintStageResultBytes, nameof(value));
    }

    /// <summary>Decode one exact bounded canonical operation-16 result without granting device authority.</summary>
    public static KagemushaDeviceMintStageResultV1 DecodeDeviceMintStageResultShapeExact(
        ReadOnlySpan<byte> archive) => DecodeExact(archive, MaximumDeviceMintStageResultBytes,
            DeviceMintStageResultSchema, DecodeDeviceMintStageResultPayload, EncodeDeviceMintStageResultShape);

    /// <summary>Additionally bind the decoded public result to the exact finalized credit in its command.</summary>
    public static KagemushaDeviceMintStageResultV1 DecodeDeviceMintStageResultShapeExact(
        ReadOnlySpan<byte> archive,
        KagemushaDeviceMintStageCommandV1 command)
    {
        var value = DecodeDeviceMintStageResultShapeExact(archive);
        var credit = ValidateDeviceMintStageCommandShape(command);
        RequireEqual(value.CreditIdSpan, credit.Statement.Lifecycle.CreditId.Span,
            "device mint-stage result credit id");
        return value;
    }

    public static byte[] EncodePeerCreditContext(KagemushaPeerCreditContextV1 value)
    {
        ValidatePeerCreditContext(value);
        return Frame(PeerCreditContextSchema, EncodePeerCreditContextPayload(value), 8);
    }

    public static KagemushaPeerCreditContextV1 DecodePeerCreditContext(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 512, PeerCreditContextSchema,
            DecodePeerCreditContextPayload, EncodePeerCreditContext);

    public static byte[] EncodeCommitCertificate(KagemushaCommitCertificateV1 value)
    {
        ValidateCommitCertificateShape(value);
        return Bounded(Frame(CommitCertificateSchema, EncodeCommitCertificatePayload(value), 8),
            MaximumCommitCertificateBytes, nameof(value));
    }

    public static KagemushaCommitCertificateV1 DecodeCommitCertificate(
        ReadOnlySpan<byte> archive,
        KagemushaLifecycleBindingV1 lifecycle,
        KagemushaCommitEvidenceV1 evidence,
        ReadOnlyMemory<byte> transitionNullifier) => DecodeExact(
            archive,
            MaximumCommitCertificateBytes,
            CommitCertificateSchema,
            DecodeCommitCertificatePayload,
            value =>
            {
                ValidateCommitCertificate(value, lifecycle, evidence, transitionNullifier);
                return EncodeCommitCertificate(value);
            });

    public static byte[] EncodeRedemptionProof(KagemushaRedemptionProofV1 value)
    {
        ValidateRedemptionProofShape(value);
        return Bounded(Frame(RedemptionProofSchema, EncodeRedemptionProofPayload(value), 8),
            MaximumRedemptionProofBytes, nameof(value));
    }

    public static KagemushaRedemptionProofV1 DecodeRedemptionProof(
        ReadOnlySpan<byte> archive) => DecodeExact(
            archive,
            MaximumRedemptionProofBytes,
            RedemptionProofSchema,
            DecodeRedemptionProofPayload,
            EncodeRedemptionProof);

    /// <summary>Encode the sole post-commit payment proof without granting monetary authority.</summary>
    public static byte[] EncodePaymentProof(KagemushaPaymentProofV1 value)
    {
        ValidatePaymentProofShape(value);
        return Bounded(Frame(PaymentProofSchema, EncodePaymentProofPayload(value), 8),
            MaximumPaymentProofBytes, nameof(value));
    }

    public static KagemushaPaymentProofV1 DecodePaymentProof(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumPaymentProofBytes, PaymentProofSchema,
            DecodePaymentProofPayload, EncodePaymentProof);

    public static byte[] EncodeRedemptionVoucher(KagemushaRedemptionVoucherV1 value)
    {
        ValidateRedemptionVoucher(value);
        return Bounded(Frame(RedemptionVoucherSchema, EncodeRedemptionVoucherPayload(value), 16),
            MaximumRedemptionVoucherBytes, nameof(value));
    }

    public static KagemushaRedemptionVoucherV1 DecodeRedemptionVoucher(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumRedemptionVoucherBytes, RedemptionVoucherSchema,
            DecodeRedemptionVoucherPayload, EncodeRedemptionVoucher);

    public static byte[] EncodeCreditOpening(KagemushaCreditOpeningV1 value)
    {
        ValidateCreditOpening(value);
        var archive = Bounded(Frame(CreditOpeningSchema, EncodeCreditOpeningPayload(value), 16), MaximumCreditOpeningBytes, nameof(value));
        if (archive.Length != CreditOpeningCanonicalBytes)
        {
            throw new ArgumentException(
                $"Credit opening must encode to exactly {CreditOpeningCanonicalBytes} bytes.",
                nameof(value));
        }

        return archive;
    }

    public static KagemushaCreditOpeningV1 DecodeCreditOpening(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumCreditOpeningBytes, CreditOpeningSchema, DecodeCreditOpeningPayload, EncodeCreditOpening);

    public static byte[] EncodeEncryptedCreditAad(KagemushaEncryptedCreditAadV1 value)
    {
        ValidateCreditAad(value);
        return Frame(CreditAadSchema, EncodeCreditAadPayload(value), 16);
    }

    public static KagemushaEncryptedCreditAadV1 DecodeEncryptedCreditAad(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 512, CreditAadSchema, DecodeCreditAadPayload, EncodeEncryptedCreditAad);

    public static byte[] EncodeEncryptedCreditEnvelope(KagemushaEncryptedCreditEnvelopeV1 value)
    {
        ValidateCreditEnvelope(value);
        return Bounded(Frame(CreditEnvelopeSchema, EncodeCreditEnvelopePayload(value), 1), MaximumEncryptedCreditBytes, nameof(value));
    }

    public static KagemushaEncryptedCreditEnvelopeV1 DecodeEncryptedCreditEnvelope(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumEncryptedCreditBytes, CreditEnvelopeSchema,
            DecodeCreditEnvelopePayload, EncodeEncryptedCreditEnvelope);

    public static byte[] EncodeTopUpRequest(KagemushaTopUpRequestV1 value)
    {
        ValidateTopUpRequest(value);
        return Bounded(Frame(TopUpRequestSchema, EncodeTopUpRequestPayload(value), 16), MaximumTopUpRequestBytes, nameof(value));
    }

    internal static byte[] EncodeTopUpInstructionPayload(KagemushaTopUpRequestV1 value)
    {
        ValidateTopUpRequest(value);
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(EncodeTopUpRequestPayload(value));
        return writer.ToArray();
    }

    internal static byte[] EncodeTopUpInstructionFrame(KagemushaTopUpRequestV1 value) =>
        Frame(TopUpInstructionSchema, EncodeTopUpInstructionPayload(value), 16);

    public static KagemushaTopUpRequestV1 DecodeTopUpRequest(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumTopUpRequestBytes, TopUpRequestSchema,
            DecodeTopUpRequestPayload, EncodeTopUpRequest);

    public static byte[] EncodeRedemptionRequest(KagemushaRedemptionRequestV1 value)
    {
        ValidateVersion(value.Version);
        _ = Fixed(value.OperationId);
        ValidateRedemptionVoucher(value.Voucher);
        return Bounded(Frame(RedemptionRequestSchema, Fields(U16(value.Version), Fixed(value.OperationId),
            EncodeRedemptionVoucherPayload(value.Voucher)), 16), MaximumRedemptionRequestBytes, nameof(value));
    }

    public static KagemushaRedemptionRequestV1 DecodeRedemptionRequest(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumRedemptionRequestBytes, RedemptionRequestSchema,
            DecodeRedemptionRequestPayload, EncodeRedemptionRequest);

    public static string EncodeText(PayloadKind kind, ReadOnlySpan<byte> canonicalPayload)
    {
        var (maximumRaw, maximumText) = Limits(kind);
        if (canonicalPayload.IsEmpty || canonicalPayload.Length > maximumRaw)
            throw new ArgumentException("KAGEMUSHA V1 payload is empty or oversized.", nameof(canonicalPayload));
        var body = Convert.ToBase64String(canonicalPayload).TrimEnd('=').Replace('+', '-').Replace('/', '_');
        var text = TextPrefix + body;
        if (text.Length > maximumText) throw new ArgumentException("KAGEMUSHA V1 text is oversized.", nameof(canonicalPayload));
        return text;
    }

    public static byte[] DecodeText(PayloadKind kind, string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        var (maximumRaw, maximumText) = Limits(kind);
        if (text.Length > maximumText || !text.StartsWith(TextPrefix, StringComparison.Ordinal))
            throw new FormatException("KAGEMUSHA V1 text prefix or size is invalid.");
        var body = text[TextPrefix.Length..];
        if (body.Length == 0 || body.Length % 4 == 1 || body.Any(static value =>
                !(value is >= 'A' and <= 'Z' or >= 'a' and <= 'z' or >= '0' and <= '9' or '-' or '_')))
            throw new FormatException("KAGEMUSHA V1 text is not canonical unpadded base64url.");
        var padded = body.Replace('-', '+').Replace('_', '/').PadRight((body.Length + 3) / 4 * 4, '=');
        byte[] raw;
        try { raw = Convert.FromBase64String(padded); }
        catch (FormatException error) { throw new FormatException("KAGEMUSHA V1 base64url is invalid.", error); }
        if (raw.Length > maximumRaw || EncodeText(kind, raw) != text)
            throw new FormatException("KAGEMUSHA V1 text is not canonical.");
        return raw;
    }

    public static byte[] DeviceKeyReference(KagemushaDevicePublicKeyV1 publicKey) =>
        Hash(DeviceKeyReferenceDomain, [0], publicKey.Sec1Bytes());

    public static byte[] PastaStateCommitment(KagemushaPastaStateCommitmentV1 value)
    {
        ValidatePastaState(value);
        return Hash(PastaStateCommitmentDomain, [0], value.Eq.ToArray(), value.Ep.ToArray());
    }

    public static byte[] LiabilityPoolId(
        NetworkId networkId,
        KagemushaAssetDefinitionIdV1 asset,
        KagemushaAssetIncarnationV1 incarnation) => DigestEncoded(
            LiabilityPoolDomain,
            Frame("iroha.kagemusha.v1.liability-pool-preimage",
                Fields(networkId.ToBytes(), asset.CanonicalPayload(),
                    EncodeAssetIncarnationPayload(incarnation)), 1));

    public static byte[] PaymentRequestDigest(KagemushaPaymentRequestV1 value)
    {
        ValidateRequest(value);
        return PaymentRequestDigestUnchecked(value);
    }

    /// <summary>Return the fixed-width commitment proven for a durable sender-outbox reservation.</summary>
    public static byte[] OutboxReservationCommitment(KagemushaOutboxReservationV1 value)
    {
        ValidateOutboxReservation(value);
        return DigestEncoded(
            OutboxReservationCommitmentDomain,
            OutboxReservationCircuitTranscript(value));
    }

    /// <summary>Return the hiding commitment installed in a recoverable terminal certificate.</summary>
    public static byte[] HardwareTerminalBodyCommitment(KagemushaHardwareTerminalBodyV1 value)
    {
        ValidateHardwareTerminalBody(value);
        return DigestEncoded(
            HardwareTerminalBodyCommitmentDomain,
            Frame(HardwareTerminalBodySchema, EncodeHardwareTerminalBodyPayload(value), 8));
    }

    public static byte[] PaymentDigest(
        KagemushaPaymentV1 value,
        KagemushaPaymentRequestV1 request)
    {
        ValidatePayment(value, request);
        return PaymentDigestUnchecked(value);
    }

    /// <summary>Compute a terminal certificate identity without self-reference.</summary>
    public static byte[] ExpectedCommitCertificateId(KagemushaCommitCertificateV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        ValidateVersion(value.Version);
        ValidateCommitEvidence(value.CommitEvidence);
        foreach (var field in new[] { value.CandidateEnvelopeDigest, value.LifecycleBindingDigest,
                     value.TransitionNullifier, value.OutboxReservationCommitment,
                     value.HardwareProfileId, value.HardwareTerminalCommitment })
            _ = Fixed(field);
        if (value.PolicyEpoch == 0)
            throw new ArgumentException("KAGEMUSHA V1 commit certificate policy epoch is zero.", nameof(value));
        return DigestEncoded(CommitCertificateIdDomain, CommitCertificateIdCircuitTranscript(value));
    }

    /// <summary>Return the fixed transcript digest bound by a terminal redemption proof.</summary>
    public static byte[] CommitCertificateDigest(
        KagemushaCommitCertificateV1 value,
        KagemushaLifecycleBindingV1 lifecycle,
        KagemushaCommitEvidenceV1 evidence,
        ReadOnlyMemory<byte> transitionNullifier)
    {
        ValidateCommitCertificate(value, lifecycle, evidence, transitionNullifier);
        return CommitCertificateDigestUnchecked(value);
    }

    /// <summary>Digest a shape-validated certificate; this does not authenticate its hardware authority.</summary>
    public static byte[] CommitCertificateDigest(KagemushaCommitCertificateV1 value)
    {
        ValidateCommitCertificateShape(value);
        return CommitCertificateDigestUnchecked(value);
    }

    /// <summary>Return the canonical digest of one complete lifecycle binding.</summary>
    public static byte[] LifecycleBindingDigest(KagemushaLifecycleBindingV1 value)
    {
        ValidateLifecycle(value);
        return LifecycleBindingDigestUnchecked(value);
    }

    /// <summary>Return the semantic digest for an unlinkable payment output.</summary>
    public static byte[] PaymentOutputDigest(
        KagemushaPaymentOutputV1 value,
        KagemushaPaymentRequestV1 request)
    {
        ValidatePaymentOutput(value, request);
        return PaymentOutputDigestUnchecked(value);
    }

    /// <summary>Derive the request-bound peer credit identity.</summary>
    public static byte[] CreditId(
        ReadOnlyMemory<byte> transitionNullifier,
        ReadOnlyMemory<byte> requestDigest)
    {
        foreach (var field in new[] { transitionNullifier, requestDigest })
            _ = Fixed(field);
        return Hash(
            CreditIdDomain,
            new byte[] { 0 },
            Fixed(transitionNullifier),
            Fixed(requestDigest));
    }

    /// <summary>Commit one private peer-credit opening before deriving its credit ID.</summary>
    public static byte[] PeerCreditOpeningCommitment(
        ReadOnlyMemory<byte> requestDigest,
        KagemushaX25519PublicKeyV1 recipientEncryptionKey,
        UInt128 amount,
        ReadOnlyMemory<byte> creditCommitmentOpening,
        ReadOnlyMemory<byte> recipientBindingOpening,
        ReadOnlyMemory<byte> recoveryNonce)
    {
        foreach (var field in new[] { requestDigest, creditCommitmentOpening,
                     recipientBindingOpening, recoveryNonce })
            _ = Fixed(field);
        ArgumentNullException.ThrowIfNull(recipientEncryptionKey);
        if (amount == 0)
            throw new ArgumentException("KAGEMUSHA V1 credit amount must be positive.", nameof(amount));
        return Hash(
            PeerCreditOpeningCommitmentDomain,
            new byte[] { 0 },
            U16(WireVersion),
            Fixed(requestDigest),
            recipientEncryptionKey.Bytes(),
            U128(amount),
            Fixed(creditCommitmentOpening),
            Fixed(recipientBindingOpening),
            Fixed(recoveryNonce));
    }

    /// <summary>Derive the request-bound, pre-encryption transfer commitment.</summary>
    public static byte[] PreparedTransferDigest(
        KagemushaPaymentRequestV1 request,
        ReadOnlyMemory<byte> senderBeforeCommitment,
        ReadOnlyMemory<byte> senderAfterCommitment,
        ReadOnlyMemory<byte> transitionNullifier,
        ReadOnlyMemory<byte> ciphertextCommitment)
    {
        ValidateRequest(request);
        _ = Fixed(senderBeforeCommitment);
        _ = Fixed(senderAfterCommitment);
        if (senderBeforeCommitment.Span.SequenceEqual(senderAfterCommitment.Span))
            throw new ArgumentException("KAGEMUSHA V1 sender state commitments must differ.");
        _ = Fixed(transitionNullifier);
        _ = Fixed(ciphertextCommitment);
        return DigestEncoded(PreparedTransferDigestDomain,
            Join(U16(WireVersion), Fixed(PaymentRequestDigest(request)),
                U128(request.Amount), Fixed(senderBeforeCommitment), Fixed(senderAfterCommitment),
                Fixed(transitionNullifier), request.RecipientEncryptionKey.Bytes(),
                Fixed(ciphertextCommitment)));
    }

    /// <summary>Build the exact peer context authenticated by encrypted-credit AAD.</summary>
    public static KagemushaPeerCreditContextV1 PeerCreditContext(
        KagemushaPaymentOutputV1 output,
        KagemushaPaymentRequestV1 request)
    {
        ValidateRequest(request);
        ValidatePaymentOutput(output, request);
        var context = new KagemushaPeerCreditContextV1(
            WireVersion,
            output.RequestDigest,
            output.Amount,
            output.SenderBeforeCommitment,
            output.SenderAfterCommitment,
            PreparedTransferDigest(request, output.SenderBeforeCommitment,
                output.SenderAfterCommitment,
                output.TransitionNullifier, output.CiphertextCommitment),
            request.RecipientEncryptionKey);
        ValidatePeerCreditContext(context);
        return context;
    }

    /// <summary>Build the exact AEAD associated data for a peer credit.</summary>
    public static KagemushaEncryptedCreditAadV1 PeerEncryptedCreditAad(
        KagemushaPaymentOutputV1 output,
        KagemushaPaymentRequestV1 request)
    {
        var context = PeerCreditContext(output, request);
        return new KagemushaEncryptedCreditAadV1(
            WireVersion,
            KagemushaEncryptedCreditPurposeV1.Peer,
            DigestEncoded(PeerCreditContextDigestDomain, EncodePeerCreditContext(context)),
            output.CiphertextCommitment,
            output.CreditId,
            output.Amount);
    }

    /// <summary>Return the canonical encrypted-envelope digest without decrypting or authenticating it.</summary>
    public static byte[] CiphertextDigest(ReadOnlySpan<byte> encryptedCredit) =>
        DigestEncoded(CiphertextDigestDomain, encryptedCredit);

    /// <summary>Return the canonical mint-authorization context digest; this grants no monetary authority.</summary>
    public static byte[] MintAuthorizationContextDigest(KagemushaMintAuthorizationContextV1 value)
    {
        ValidateMintContext(value);
        return MintAuthorizationContextDigestUnchecked(value);
    }

    /// <summary>Return the semantic digest a mint-authorization proof must carry.</summary>
    public static byte[] MintAuthorizationStatementDigest(KagemushaMintAuthorizationStatementV1 value)
    {
        ValidateMintContext(value.Context);
        if (value.Version != WireVersion || value.Context.Version != value.Version)
            throw new ArgumentException("KAGEMUSHA V1 mint authorization statement version differs.", nameof(value));
        _ = Fixed(value.IssuanceCommitment);
        _ = Fixed(value.CreditId);
        _ = Fixed(value.CiphertextDigest);
        return MintAuthorizationStatementDigestUnchecked(value);
    }

    /// <summary>Return the canonical mint-authorization digest; this does not verify its proof.</summary>
    public static byte[] MintAuthorizationDigest(KagemushaMintAuthorizationV1 value)
    {
        ValidateMintAuthorization(value);
        return MintAuthorizationDigestUnchecked(value);
    }

    /// <summary>Return the semantic digest a mint-credit proof must carry.</summary>
    public static byte[] MintCreditStatementDigest(KagemushaMintCreditStatementV1 value)
    {
        ValidateLifecycle(value.Lifecycle);
        if (value.Version != WireVersion || value.Lifecycle.OperationKind != KagemushaOperationKindV1.MintFold
            || value.Amount == 0 || value.MintedAtMilliseconds == 0)
            throw new ArgumentException("KAGEMUSHA V1 mint credit statement is invalid.", nameof(value));
        return MintStatementDigestUnchecked(value);
    }

    /// <summary>Validate exactly all three separately framed KAGEMUSHA peer messages.</summary>
    public static int ValidateCompleteExchange(
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment,
        KagemushaAcknowledgementV1 acknowledgement)
    {
        ValidatePayment(payment, request);
        ValidateAcknowledgement(acknowledgement, request, payment);
        var sizes = new[]
        {
            EncodePaymentRequest(request).Length,
            EncodePayment(payment, request).Length,
            EncodeAcknowledgement(acknowledgement, request, payment).Length,
        };
        var raw = sizes.Sum();
        if (raw > MaximumCompleteExchangeRawBytes
            || sizes.Sum(TextLength) > MaximumCompleteExchangeTextBytes)
            throw new ArgumentException("KAGEMUSHA V1 complete three-message exchange is oversized.");
        return raw;
    }

    private static void ValidateAggregate(KagemushaAggregateStateCommitmentV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale);
        foreach (var field in new[] { value.ReleaseId, value.LiabilityPoolId, value.LaneId,
                     value.HardwareEpochId, value.KeyReference, value.HardwarePolicyId, value.StateCommitment })
            _ = Fixed(field);
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "aggregate liability pool");
    }

    private static void ValidateHardwareProfile(KagemushaHardwareProfileV1 value)
    {
        ValidateVersion(value.Version);
        if (value.ProtocolVersion != WireVersion || value.PolicyEpoch == 0 || value.CapabilityMask != ushort.MaxValue
            || value.ExpiresAtMilliseconds <= value.ValidFromMilliseconds
            || !Enum.IsDefined(value.PlatformClass))
            throw new ArgumentException("KAGEMUSHA V1 hardware profile is invalid.");
        foreach (var field in new[] { value.HardwareProfileId, value.ProviderId, value.ProductClassDigest,
                     value.FirmwarePolicyDigest, value.EnrollmentAttestationVerifierDigest,
                     value.AttestationTrustRootsDigest, value.AllowedSuiteCommitment,
                     value.QualificationReportDigest })
            _ = Fixed(field);
    }

    private static void ValidateHardwareCredential(KagemushaHardwareCredentialV1 value)
    {
        ValidateVersion(value.Version);
        if (value.PolicyEpoch == 0 || value.ExpiresAtMilliseconds <= value.IssuedAtMilliseconds)
            throw new ArgumentException("KAGEMUSHA V1 hardware credential is invalid.");
        foreach (var field in new[] { value.CredentialId, value.HardwareProfileId, value.SuiteId,
                     value.FirmwarePolicyDigest, value.LaneCommitment, value.HardwareEpochId,
                     value.DeviceKeyReference })
            _ = Fixed(field);
    }

    private static void ValidateRequest(KagemushaPaymentRequestV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale, value.Amount);
        ValidateHardwareCredential(value.HardwareCredential);
        ArgumentNullException.ThrowIfNull(value.RecipientEncryptionKey);
        _ = Fixed(value.ReleaseId);
        _ = Fixed(value.LiabilityPoolId);
        _ = Fixed(value.RequestId);
        if (value.HardwareCredential.NetworkId != value.NetworkId
            || value.IssuedAtMilliseconds < value.HardwareCredential.IssuedAtMilliseconds
            || value.ExpiresAtMilliseconds > value.HardwareCredential.ExpiresAtMilliseconds
            || value.ExpiresAtMilliseconds <= value.IssuedAtMilliseconds
            || value.ExpiresAtMilliseconds - value.IssuedAtMilliseconds > RequestMaximumTtlMilliseconds)
            throw new ArgumentException("KAGEMUSHA V1 request lifetime or credential binding is invalid.");
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "request liability pool");
        RequireEqual(value.HardwareCredential.DeviceKeyReference.Span,
            DeviceKeyReference(value.HardwareCredential.DevicePublicKey), "request device key reference");
    }

    private static void ValidatePayment(
        KagemushaPaymentV1 value,
        KagemushaPaymentRequestV1 request)
    {
        ValidateVersion(value.Version);
        if (value.Output.Version != value.Version)
            throw new ArgumentException("KAGEMUSHA V1 payment version binding is invalid.");
        ValidatePaymentOutput(value.Output, request);
        _ = DecodeEncryptedCreditEnvelope(value.EncryptedCredit.Span);
        _ = EncodeEncryptedCreditAad(PeerEncryptedCreditAad(value.Output, request));
        ValidateCommitCertificateShape(value.CommitCertificate);
        RequireEqual(value.CommitCertificate.TransitionNullifier.Span,
            value.Output.TransitionNullifier.Span, "payment certificate nullifier");
        RequireEqual(CommitEvidenceCircuitTranscript(value.CommitCertificate.CommitEvidence),
            CommitEvidenceCircuitTranscript(value.Output.CommitEvidence), "payment certificate evidence");
        ValidatePaymentProof(value.Proof, PaymentBodyDigestUnchecked(value), value.CommitCertificate);
    }

    private static void ValidatePaymentOutput(
        KagemushaPaymentOutputV1 value,
        KagemushaPaymentRequestV1 request)
    {
        ValidateVersion(value.Version);
        ValidateRequest(request);
        ValidateCommitEvidence(value.CommitEvidence);
        foreach (var field in new[] { value.RequestDigest, value.SenderBeforeCommitment,
                     value.SenderAfterCommitment, value.TransitionNullifier,
                     value.CreditId, value.CiphertextCommitment })
            _ = Fixed(field);
        var requestDigest = PaymentRequestDigestUnchecked(request);
        RequireEqual(value.RequestDigest.Span, requestDigest, "payment request digest");
        if (value.Amount == 0 || value.Amount != request.Amount
            || value.SenderBeforeCommitment.Span.SequenceEqual(value.SenderAfterCommitment.Span)
            || value.CommittedAtMilliseconds < request.IssuedAtMilliseconds
            || value.CommittedAtMilliseconds >= request.ExpiresAtMilliseconds)
            throw new ArgumentException("KAGEMUSHA V1 payment output is invalid.", nameof(value));
        RequireEqual(value.CreditId.Span,
            CreditId(value.TransitionNullifier, requestDigest), "payment credit id");
    }

    private static void ValidateLifecycle(KagemushaLifecycleBindingV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale);
        if (value.ProtocolVersion != WireVersion || value.PolicyEpoch == 0 || !Enum.IsDefined(value.OperationKind))
            throw new ArgumentException("KAGEMUSHA V1 lifecycle is invalid.");
        foreach (var field in new[] { value.SuiteId, value.VkDigest, value.ReleaseId,
                     value.LiabilityPoolId, value.HardwareProfileId })
            _ = Fixed(field);
        var requestPresent = IsNonzero32(value.RequestId);
        var requestAbsent = IsZero32(value.RequestId);
        var lanePresent = IsNonzero32(value.ReceiverLaneCommitment);
        var laneAbsent = IsZero32(value.ReceiverLaneCommitment);
        var creditPresent = IsNonzero32(value.CreditId) && IsNonzero32(value.CiphertextDigest);
        var creditAbsent = IsZero32(value.CreditId) && IsZero32(value.CiphertextDigest);
        var shapeValid = value.OperationKind switch
        {
            KagemushaOperationKindV1.SendSplit => requestPresent && lanePresent && creditPresent,
            KagemushaOperationKindV1.MintFold => requestAbsent && laneAbsent && creditPresent,
            _ => requestAbsent && laneAbsent && creditAbsent,
        };
        if (!shapeValid) throw new ArgumentException("KAGEMUSHA V1 lifecycle operation fields are invalid.");
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "lifecycle liability pool");
    }

    private static void ValidateProof(KagemushaPairedProofV1 value)
    {
        ValidateVersion(value.Version);
        foreach (var field in new[] { value.EqProtocolDigest, value.EpProtocolDigest,
                     value.SemanticDigest, value.GuardEqCredentialAudit,
                     value.GuardEpCredentialAudit, value.EqDeferredAudit, value.EpDeferredAudit })
            _ = Fixed(field);
        if (value.EqProtocolDigest.Span.SequenceEqual(value.EpProtocolDigest.Span)
            || value.GuardEqCredentialAudit.Span.SequenceEqual(value.GuardEpCredentialAudit.Span)
            || value.EqDeferredAudit.Span.SequenceEqual(value.EpDeferredAudit.Span))
            throw new ArgumentException("KAGEMUSHA V1 proof parity bindings alias.", nameof(value));
        ValidateProofBytes(value.EqProof, value.EpProof, value.EqHistory, value.EpHistory);
    }

    private static void ValidateCommitEvidence(KagemushaCommitEvidenceV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        _ = Fixed(value.Commitment);
        if (value is not KagemushaTrustedCommitTimeV1
            && value is not KagemushaMonotonicLeaseV1)
            throw new ArgumentException("KAGEMUSHA V1 commit evidence source is invalid.", nameof(value));
    }

    private static void ValidateOutboxReservation(KagemushaOutboxReservationV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        _ = Fixed(value.ReservationId);
        var minimum = value.OperationKind switch
        {
            KagemushaOperationKindV1.SendSplit => (uint)PaymentOutboxMinimumBytes,
            KagemushaOperationKindV1.RedeemSplit => (uint)RedemptionOutboxMinimumBytes,
            _ => throw new ArgumentException(
                "KAGEMUSHA V1 outbox reservation has no terminal-envelope operation.", nameof(value)),
        };
        if (value.ReservedOutboxBytes < minimum
            || value.IssuedAtMilliseconds >= value.ExpiresAtMilliseconds)
            throw new ArgumentException("KAGEMUSHA V1 outbox reservation is invalid.", nameof(value));
    }

    private static void ValidateHardwareTerminalBody(KagemushaHardwareTerminalBodyV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        ValidateVersion(value.Version);
        ValidateCommitEvidence(value.CommitEvidence);
        foreach (var field in new[] { value.CandidateEnvelopeDigest, value.LifecycleBindingDigest,
                     value.TransitionNullifier, value.OutboxReservationCommitment,
                     value.HardwareProfileId, value.PrivateSuccessorCommitment,
                     value.PrivateJournalCommitment, value.PrivateRecoveryCommitment })
            _ = Fixed(field);
        if (value.PolicyEpoch == 0)
            throw new ArgumentException("KAGEMUSHA V1 hardware terminal-body policy epoch is zero.", nameof(value));
    }

    private static void ValidateCommitCertificateShape(KagemushaCommitCertificateV1 value)
    {
        ValidateVersion(value.Version);
        ValidateCommitEvidence(value.CommitEvidence);
        foreach (var field in new[] { value.CertificateId, value.CandidateEnvelopeDigest,
                     value.LifecycleBindingDigest, value.TransitionNullifier,
                     value.OutboxReservationCommitment, value.HardwareProfileId,
                     value.HardwareTerminalCommitment })
            _ = Fixed(field);
        if (value.PolicyEpoch == 0)
            throw new ArgumentException("KAGEMUSHA V1 commit certificate policy epoch is zero.", nameof(value));
        RequireEqual(value.CertificateId.Span, ExpectedCommitCertificateId(value), "commit certificate id");
    }

    private static void ValidateCommitCertificate(
        KagemushaCommitCertificateV1 value,
        KagemushaLifecycleBindingV1 lifecycle,
        KagemushaCommitEvidenceV1 evidence,
        ReadOnlyMemory<byte> transitionNullifier)
    {
        ValidateLifecycle(lifecycle);
        ValidateCommitCertificateShape(value);
        RequireEqual(value.LifecycleBindingDigest.Span,
            LifecycleBindingDigestUnchecked(lifecycle), "commit certificate lifecycle");
        RequireEqual(value.TransitionNullifier.Span, transitionNullifier.Span,
            "commit certificate transition nullifier");
        RequireEqual(value.CommitEvidence.Commitment.Span, evidence.Commitment.Span,
            "commit certificate evidence");
        if (value.CommitEvidence.WireTag != evidence.WireTag)
            throw new ArgumentException("KAGEMUSHA V1 commit certificate evidence source differs.", nameof(value));
        RequireEqual(value.HardwareProfileId.Span, lifecycle.HardwareProfileId.Span,
            "commit certificate hardware profile");
        if (value.PolicyEpoch != lifecycle.PolicyEpoch)
            throw new ArgumentException("KAGEMUSHA V1 commit certificate policy epoch differs.", nameof(value));
    }

    private static void ValidateRedemptionProofShape(KagemushaRedemptionProofV1 value)
    {
        ValidateVersion(value.Version);
        foreach (var field in new[] { value.EqProtocolDigest, value.EpProtocolDigest,
                     value.SemanticDigest, value.CandidateEnvelopeDigest,
                     value.CommitCertificateDigest, value.EqDeferredAudit, value.EpDeferredAudit })
            _ = Fixed(field);
        if (value.EqProtocolDigest.Span.SequenceEqual(value.EpProtocolDigest.Span)
            || value.EqDeferredAudit.Span.SequenceEqual(value.EpDeferredAudit.Span))
            throw new ArgumentException("KAGEMUSHA V1 redemption proof parity bindings alias.", nameof(value));
        ValidateProofBytes(value.EqProof, value.EpProof, value.EqHistory, value.EpHistory);
    }

    private static void ValidatePaymentProofShape(KagemushaPaymentProofV1 value)
    {
        ValidateVersion(value.Version);
        foreach (var field in new[] { value.EqProtocolDigest, value.EpProtocolDigest,
                     value.SemanticDigest, value.CandidateEnvelopeDigest,
                     value.CommitCertificateDigest, value.EqDeferredAudit, value.EpDeferredAudit })
            _ = Fixed(field);
        if (value.EqProtocolDigest.Span.SequenceEqual(value.EpProtocolDigest.Span)
            || value.EqDeferredAudit.Span.SequenceEqual(value.EpDeferredAudit.Span))
            throw new ArgumentException("KAGEMUSHA V1 payment proof parity bindings alias.", nameof(value));
        ValidateProofBytes(value.EqProof, value.EpProof, value.EqHistory, value.EpHistory);
        Bounded(Frame(PaymentProofSchema, EncodePaymentProofPayload(value), 8),
            MaximumPaymentProofBytes, nameof(value));
    }

    private static void ValidatePaymentProof(KagemushaPaymentProofV1 value,
        ReadOnlySpan<byte> bodyDigest, KagemushaCommitCertificateV1 certificate)
    {
        ValidatePaymentProofShape(value);
        RequireEqual(value.SemanticDigest.Span, bodyDigest, "payment proof body digest");
        RequireEqual(value.CandidateEnvelopeDigest.Span, certificate.CandidateEnvelopeDigest.Span,
            "payment proof candidate digest");
        RequireEqual(value.CommitCertificateDigest.Span, CommitCertificateDigestUnchecked(certificate),
            "payment proof certificate digest");
    }

    private static void ValidateRedemptionProof(
        KagemushaRedemptionProofV1 value,
        ReadOnlySpan<byte> semanticDigest,
        KagemushaCommitCertificateV1 certificate)
    {
        ValidateRedemptionProofShape(value);
        RequireEqual(value.SemanticDigest.Span, semanticDigest, "redemption proof semantic digest");
        RequireEqual(value.CandidateEnvelopeDigest.Span, certificate.CandidateEnvelopeDigest.Span,
            "redemption proof candidate digest");
        RequireEqual(value.CommitCertificateDigest.Span,
            CommitCertificateDigestUnchecked(certificate), "redemption proof certificate digest");
    }

    private static void ValidateProofBytes(
        ReadOnlyMemory<byte> eqProof,
        ReadOnlyMemory<byte> epProof,
        ReadOnlyMemory<byte> eqHistory,
        ReadOnlyMemory<byte> epHistory)
    {
        if (eqProof.IsEmpty || epProof.IsEmpty || eqProof.Length > MaximumParityProofBytes
            || epProof.Length > MaximumParityProofBytes || eqProof.Length + epProof.Length > MaximumCurrentProofsBytes
            || eqHistory.Length != HistoryAccumulatorBytes || epHistory.Length != HistoryAccumulatorBytes
            || eqHistory.Span.IndexOfAnyExcept((byte)0) < 0 || epHistory.Span.IndexOfAnyExcept((byte)0) < 0
            || eqHistory.Span.SequenceEqual(epHistory.Span))
            throw new ArgumentException("KAGEMUSHA V1 recursive proof bytes are invalid.");
    }

    private static void ValidatePastaState(KagemushaPastaStateCommitmentV1 value)
    {
        if (value.Eq.Length != 32 || value.Ep.Length != 32)
            throw new ArgumentException("KAGEMUSHA V1 Pasta state components must be exactly 32 bytes.");
        var eqZero = value.Eq.Span.IndexOfAnyExcept((byte)0) < 0;
        var epZero = value.Ep.Span.IndexOfAnyExcept((byte)0) < 0;
        if (eqZero != epZero) throw new ArgumentException("KAGEMUSHA V1 Pasta state is half-present.");
    }

    private static void ValidateAcknowledgement(
        KagemushaAcknowledgementV1 value,
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment)
    {
        ValidatePayment(payment, request);
        ValidateVersion(value.Version);
        if (value.InboxReceipt.Version != value.Version)
            throw new ArgumentException("KAGEMUSHA V1 acknowledgement receipt version differs.");
        _ = Fixed(value.RequestDigest);
        _ = Fixed(value.PaymentDigest);
        _ = Fixed(value.InboxReceipt.CreditId);
        _ = Fixed(value.InboxReceipt.ReceiptCommitment);
        RequireEqual(value.RequestDigest.Span, PaymentRequestDigestUnchecked(request), "acknowledgement request digest");
        RequireEqual(value.PaymentDigest.Span, PaymentDigestUnchecked(payment), "acknowledgement payment digest");
        RequireEqual(value.InboxReceipt.CreditId.Span,
            payment.Output.CreditId.Span, "acknowledgement credit id");
    }

    private static void ValidateCreditOpening(KagemushaCreditOpeningV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Amount == 0) throw new ArgumentException("KAGEMUSHA V1 credit opening amount must be positive.");
        foreach (var field in new[] { value.CreditId, value.CreditCommitmentOpening,
                     value.RecipientBindingOpening, value.RecoveryNonce })
            _ = Fixed(field);
    }

    private static void ValidateCreditAad(KagemushaEncryptedCreditAadV1 value)
    {
        ValidateVersion(value.Version);
        if (!Enum.IsDefined(value.Purpose) || value.Amount == 0)
            throw new ArgumentException("KAGEMUSHA V1 encrypted-credit AAD is invalid.");
        _ = Fixed(value.ContextDigest);
        _ = Fixed(value.IssuanceOrTransitionCommitment);
        _ = Fixed(value.CreditId);
    }

    private static void ValidatePeerCreditContext(KagemushaPeerCreditContextV1 value)
    {
        ValidateVersion(value.Version);
        foreach (var field in new[] { value.RequestDigest, value.SenderBeforeCommitment,
                     value.SenderAfterCommitment, value.PreparedTransferDigest })
            _ = Fixed(field);
        if (value.Amount == 0
            || value.SenderBeforeCommitment.Span.SequenceEqual(value.SenderAfterCommitment.Span))
            throw new ArgumentException("KAGEMUSHA V1 peer credit context is invalid.", nameof(value));
        ArgumentNullException.ThrowIfNull(value.RecipientEncryptionKey);
    }

    private static void ValidateCreditEnvelope(KagemushaEncryptedCreditEnvelopeV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Nonce.Length != 24 || value.CiphertextAndTag.Length != EncryptedCreditCiphertextAndTagBytes)
            throw new ArgumentException("KAGEMUSHA V1 encrypted-credit envelope length is invalid.");
    }

    private static void ValidateMintAuthorization(KagemushaMintAuthorizationV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.Statement.Context.Version != value.Version)
            throw new ArgumentException("KAGEMUSHA V1 mint authorization version differs.");
        ValidateMintContext(value.Statement.Context);
        _ = Fixed(value.Statement.IssuanceCommitment);
        _ = Fixed(value.Statement.CreditId);
        _ = Fixed(value.Statement.CiphertextDigest);
        ValidateProof(value.Proof);
        RequireEqual(value.Proof.SemanticDigest.Span,
            MintAuthorizationStatementDigestUnchecked(value.Statement),
            "mint authorization proof semantic digest");
    }

    private static KagemushaMintCreditV1 ValidateDeviceMintStageCommandShape(
        KagemushaDeviceMintStageCommandV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        var authorization = DecodeMintAuthorization(value.CanonicalAuthorizationSpan);
        return DecodeMintCredit(value.CanonicalMintCreditSpan, authorization);
    }

    private static void ValidateMintContext(KagemushaMintAuthorizationContextV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale, value.Amount);
        if (value.PolicyEpoch == 0) throw new ArgumentException("KAGEMUSHA V1 mint policy epoch must be positive.");
        foreach (var field in new[] { value.OperationId, value.ReleaseId, value.SuiteId,
                     value.VkDigest, value.ArtifactManifestDigest, value.LiabilityPoolId,
                     value.HardwareCredentialId, value.HardwareProfileId,
                     value.RecipientCredentialCommitment, value.CreditCommitment })
            _ = Fixed(field);
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "mint context liability pool");
    }

    private static void ValidateMintCredit(
        KagemushaMintCreditV1 value,
        KagemushaMintAuthorizationV1? authorization)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.Proof.Version != value.Version)
            throw new ArgumentException("KAGEMUSHA V1 mint credit version differs.");
        ValidateLifecycle(value.Statement.Lifecycle);
        if (value.Statement.Lifecycle.OperationKind != KagemushaOperationKindV1.MintFold
            || value.Statement.Amount == 0 || value.Statement.MintedAtMilliseconds == 0)
            throw new ArgumentException("KAGEMUSHA V1 mint statement is invalid.");
        foreach (var field in new[] { value.Statement.RecipientCredentialCommitment,
                     value.Statement.AuthorizationContextDigest, value.Statement.MintAuthorizationDigest,
                     value.Statement.IssuanceCommitment, value.Statement.CreditCommitment,
                     value.FinalityCertificateBinding, value.FinalityAuthorityHead,
                     value.FinalityGenesisRosterId, value.FinalityProofBindingDigest,
                     value.ArtifactManifestDigest })
            _ = Fixed(field);
        ValidateProof(value.Proof);
        _ = DecodeEncryptedCreditEnvelope(value.EncryptedCredit.Span);
        RequireEqual(value.Statement.Lifecycle.CiphertextDigest.Span,
            CiphertextDigest(value.EncryptedCredit.Span), "mint ciphertext digest");
        RequireEqual(value.Proof.SemanticDigest.Span,
            MintStatementDigestUnchecked(value.Statement), "mint proof semantic digest");
        if (authorization is not null)
        {
            ValidateMintAuthorization(authorization);
            var context = authorization.Statement.Context;
            if (value.Statement.Lifecycle.NetworkId != context.NetworkId
                || !value.Statement.Lifecycle.Asset.Equals(context.Asset)
                || !value.Statement.Lifecycle.AssetIncarnation.Equals(context.AssetIncarnation)
                || value.Statement.Lifecycle.Scale != context.Scale
                || value.Statement.Lifecycle.PolicyEpoch != context.PolicyEpoch
                || value.Statement.Amount != context.Amount
                || !value.Statement.Recipient.Equals(context.Recipient))
                throw new ArgumentException("KAGEMUSHA V1 mint credit authorization context differs.");
            RequireEqual(value.Statement.Lifecycle.ReleaseId.Span, context.ReleaseId.Span, "mint release");
            RequireEqual(value.Statement.Lifecycle.SuiteId.Span, context.SuiteId.Span, "mint suite");
            RequireEqual(value.Statement.Lifecycle.VkDigest.Span, context.VkDigest.Span, "mint verifier set");
            RequireEqual(value.Statement.Lifecycle.LiabilityPoolId.Span,
                context.LiabilityPoolId.Span, "mint liability pool");
            RequireEqual(value.Statement.Lifecycle.HardwareProfileId.Span,
                context.HardwareProfileId.Span, "mint hardware profile");
            RequireEqual(value.Statement.RecipientCredentialCommitment.Span,
                context.RecipientCredentialCommitment.Span, "mint recipient credential");
            RequireEqual(value.Statement.CreditCommitment.Span,
                context.CreditCommitment.Span, "mint credit commitment");
            RequireEqual(value.Statement.AuthorizationContextDigest.Span,
                MintAuthorizationContextDigestUnchecked(context), "mint authorization context digest");
            RequireEqual(value.Statement.MintAuthorizationDigest.Span,
                MintAuthorizationDigestUnchecked(authorization), "mint authorization digest");
            RequireEqual(value.Statement.IssuanceCommitment.Span,
                authorization.Statement.IssuanceCommitment.Span, "mint issuance commitment");
            RequireEqual(value.Statement.Lifecycle.CreditId.Span,
                authorization.Statement.CreditId.Span, "mint credit id");
            RequireEqual(value.Statement.Lifecycle.CiphertextDigest.Span,
                authorization.Statement.CiphertextDigest.Span, "mint ciphertext digest");
            RequireEqual(value.ArtifactManifestDigest.Span,
                context.ArtifactManifestDigest.Span, "mint artifact manifest");
        }
    }

    private static void ValidateRedemptionVoucher(KagemushaRedemptionVoucherV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.CommitCertificate.Version != value.Version
            || value.Proof.Version != value.Version)
            throw new ArgumentException("KAGEMUSHA V1 redemption version differs.");
        ValidateLifecycle(value.Statement.Lifecycle);
        ValidateCommitEvidence(value.Statement.CommitEvidence);
        if (value.Statement.Lifecycle.OperationKind != KagemushaOperationKindV1.RedeemSplit
            || value.Statement.Amount == 0
            || value.Statement.TerminalNullifier.Span.SequenceEqual(value.Statement.RedemptionCommitment.Span)
            || value.Statement.TerminalNullifier.Span.SequenceEqual(value.Statement.RedemptionId.Span)
            || value.Statement.RedemptionCommitment.Span.SequenceEqual(value.Statement.RedemptionId.Span))
            throw new ArgumentException("KAGEMUSHA V1 redemption statement is invalid.");
        foreach (var field in new[] { value.Statement.TerminalNullifier,
                     value.Statement.RedemptionCommitment, value.Statement.RedemptionId,
                     value.ArtifactManifestDigest })
            _ = Fixed(field);
        RequireEqual(value.Statement.RedemptionId.Span,
            RedemptionIdUnchecked(value.Statement), "redemption id");
        ValidateCommitCertificate(value.CommitCertificate, value.Statement.Lifecycle,
            value.Statement.CommitEvidence, value.Statement.TerminalNullifier);
        ValidateRedemptionProof(value.Proof,
            RedemptionStatementDigestUnchecked(value.Statement), value.CommitCertificate);
    }

    private static void ValidateTopUpRequest(KagemushaTopUpRequestV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale, value.Amount);
        ValidateHardwareCredential(value.HardwareCredential);
        foreach (var field in new[] { value.OperationId, value.IssuanceCommitment, value.CreditId,
                     value.ReleaseId, value.SuiteId, value.VkDigest, value.LiabilityPoolId,
                     value.RecipientCredentialCommitment, value.CreditCommitment,
                     value.ArtifactManifestDigest })
            _ = Fixed(field);
        _ = DecodeEncryptedCreditEnvelope(value.EncryptedCredit.Span);
        if (value.MintAuthorization is null)
            throw new ArgumentException("Canonical KAGEMUSHA V1 top-up requests require mint authorization.");
        ValidateMintAuthorization(value.MintAuthorization);
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "top-up liability pool");
        var statement = value.MintAuthorization.Statement;
        var context = statement.Context;
        if (context.NetworkId != value.NetworkId
            || !context.Asset.Equals(value.Asset)
            || !context.AssetIncarnation.Equals(value.AssetIncarnation)
            || context.Scale != value.Scale
            || context.Amount != value.Amount
            || !context.Payer.Equals(value.Payer)
            || !context.Recipient.Equals(value.Recipient)
            || context.PolicyEpoch != value.HardwareCredential.PolicyEpoch)
            throw new ArgumentException("KAGEMUSHA V1 top-up mint context differs.");
        RequireEqual(context.OperationId.Span, value.OperationId.Span, "top-up operation id");
        RequireEqual(context.ReleaseId.Span, value.ReleaseId.Span, "top-up release");
        RequireEqual(context.SuiteId.Span, value.SuiteId.Span, "top-up suite");
        RequireEqual(context.VkDigest.Span, value.VkDigest.Span, "top-up verifier set");
        RequireEqual(context.ArtifactManifestDigest.Span,
            value.ArtifactManifestDigest.Span, "top-up artifact manifest");
        RequireEqual(context.LiabilityPoolId.Span, value.LiabilityPoolId.Span, "top-up liability pool");
        RequireEqual(context.HardwareCredentialId.Span,
            value.HardwareCredential.CredentialId.Span, "top-up hardware credential");
        RequireEqual(context.HardwareProfileId.Span,
            value.HardwareCredential.HardwareProfileId.Span, "top-up hardware profile");
        RequireEqual(context.RecipientCredentialCommitment.Span,
            value.RecipientCredentialCommitment.Span, "top-up recipient credential commitment");
        RequireEqual(context.CreditCommitment.Span,
            value.CreditCommitment.Span, "top-up credit commitment");
        RequireEqual(context.RecipientOneTimeKey.Bytes(),
            value.RecipientOneTimeKey.Bytes(), "top-up recipient key");
        RequireEqual(statement.IssuanceCommitment.Span,
            value.IssuanceCommitment.Span, "top-up issuance commitment");
        RequireEqual(statement.CreditId.Span, value.CreditId.Span, "top-up credit id");
        RequireEqual(statement.CiphertextDigest.Span,
            CiphertextDigest(value.EncryptedCredit.Span), "top-up ciphertext digest");
    }

    private static void ValidateHeader(ushort version, NetworkId networkId, uint scale, UInt128? amount = null)
    {
        ValidateVersion(version);
        ArgumentNullException.ThrowIfNull(networkId);
        if (scale > MaximumAssetScale) throw new ArgumentOutOfRangeException(nameof(scale));
        if (amount == UInt128.Zero) throw new ArgumentOutOfRangeException(nameof(amount));
    }

    private static void ValidateVersion(ushort version)
    {
        if (version != WireVersion) throw new ArgumentException("KAGEMUSHA wire version must be 1.");
    }

    private static byte[] EncodeAggregatePayload(KagemushaAggregateStateCommitmentV1 value) => Fields(
        U16(value.Version), Fixed(value.ReleaseId), value.NetworkId.ToBytes(), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale), Fixed(value.LiabilityPoolId),
        Fixed(value.LaneId),
        Fixed(value.HardwareEpochId), Fixed(value.KeyReference), Fixed(value.HardwarePolicyId),
        U128(value.Sequence), Fixed(value.StateCommitment));

    private static byte[] EncodePastaStatePayload(KagemushaPastaStateCommitmentV1 value)
    {
        ValidatePastaState(value);
        return Fields(Raw32(value.Eq), Raw32(value.Ep));
    }

    private static byte[] EncodeProofPayload(KagemushaPairedProofV1 value) => Fields(
        U16(value.Version), Fixed(value.EqProtocolDigest), Fixed(value.EpProtocolDigest),
        Fixed(value.SemanticDigest), Fixed(value.GuardEqCredentialAudit), Fixed(value.GuardEpCredentialAudit),
        Fixed(value.EqDeferredAudit), Fixed(value.EpDeferredAudit), Vector(value.EqProof.Span),
        Vector(value.EpProof.Span), Vector(value.EqHistory.Span), Vector(value.EpHistory.Span));

    private static byte[] EncodeHardwareProfilePayload(KagemushaHardwareProfileV1 value) => Fields(
        U16(value.Version), U16(value.ProtocolVersion), Fixed(value.HardwareProfileId), Fixed(value.ProviderId),
        U32((uint)value.PlatformClass), Fixed(value.ProductClassDigest), Fixed(value.FirmwarePolicyDigest),
        Fixed(value.EnrollmentAttestationVerifierDigest), Fixed(value.AttestationTrustRootsDigest),
        Fixed(value.AllowedSuiteCommitment), U64(value.PolicyEpoch), value.GovernanceCredentialPublicKey.Sec1Bytes(),
        U16(value.CapabilityMask), Fixed(value.QualificationReportDigest), U64(value.ValidFromMilliseconds),
        U64(value.ExpiresAtMilliseconds));

    private static byte[] EncodeHardwareCredentialPayload(KagemushaHardwareCredentialV1 value) => Fields(
        U16(value.Version), Fixed(value.CredentialId), value.NetworkId.ToBytes(), Fixed(value.HardwareProfileId),
        Fixed(value.SuiteId), Fixed(value.FirmwarePolicyDigest), U64(value.PolicyEpoch), Fixed(value.LaneCommitment),
        Fixed(value.HardwareEpochId), U64(value.HardwareEpochGeneration), value.DevicePublicKey.Sec1Bytes(),
        Fixed(value.DeviceKeyReference), U64(value.IssuedAtMilliseconds), U64(value.ExpiresAtMilliseconds),
        value.GovernanceSignature.RawBytes());

    private static byte[] EncodeRequestPayload(KagemushaPaymentRequestV1 value) => Fields(
        U16(value.Version), Fixed(value.ReleaseId), value.NetworkId.ToBytes(), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale), Fixed(value.LiabilityPoolId),
        value.Recipient.CanonicalPayload(), U128(value.Amount), value.RecipientEncryptionKey.Bytes(),
        EncodeHardwareCredentialPayload(value.HardwareCredential),
        Fixed(value.RequestId), U64(value.IssuedAtMilliseconds),
        U64(value.ExpiresAtMilliseconds), value.Signature.RawBytes());

    private static byte[] EncodePeerCreditContextPayload(KagemushaPeerCreditContextV1 value) => Fields(
        U16(value.Version), Fixed(value.RequestDigest), U128(value.Amount),
        Fixed(value.SenderBeforeCommitment), Fixed(value.SenderAfterCommitment),
        Fixed(value.PreparedTransferDigest), value.RecipientEncryptionKey.Bytes());

    private static byte[] EncodeCreditOpeningPayload(KagemushaCreditOpeningV1 value) => Fields(
        U16(value.Version), Fixed(value.CreditId), U128(value.Amount), Fixed(value.CreditCommitmentOpening),
        Fixed(value.RecipientBindingOpening), Fixed(value.RecoveryNonce));

    private static byte[] EncodeCreditAadPayload(KagemushaEncryptedCreditAadV1 value) => Fields(
        U16(value.Version), U32((uint)value.Purpose), Fixed(value.ContextDigest),
        Fixed(value.IssuanceOrTransitionCommitment), Fixed(value.CreditId), U128(value.Amount));

    private static byte[] EncodeCreditEnvelopePayload(KagemushaEncryptedCreditEnvelopeV1 value) => Fields(
        U16(value.Version), value.EphemeralX25519PublicKey.Bytes(), FixedWidth(value.Nonce, 24, "nonce"),
        Vector(value.CiphertextAndTag.Span));

    private static byte[] EncodeLifecyclePayload(KagemushaLifecycleBindingV1 value) => Fields(
        U16(value.Version), value.NetworkId.ToBytes(), U16(value.ProtocolVersion), Fixed(value.SuiteId),
        Fixed(value.VkDigest), Fixed(value.ReleaseId), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation),
        U32(value.Scale), Fixed(value.LiabilityPoolId), Fixed(value.HardwareProfileId), U64(value.PolicyEpoch),
        U32((uint)value.OperationKind), Raw32(value.RequestId), Raw32(value.ReceiverLaneCommitment), Raw32(value.CreditId),
        Raw32(value.CiphertextDigest));

    private static byte[] EncodeCommitEvidencePayload(KagemushaCommitEvidenceV1 value) => value switch
    {
        KagemushaTrustedCommitTimeV1 trusted => EnumPayload(
            trusted.WireTag, Fields(Fixed(trusted.TimeEvidenceCommitment))),
        KagemushaMonotonicLeaseV1 lease => EnumPayload(
            lease.WireTag, Fields(Fixed(lease.LeaseEvidenceCommitment))),
        _ => throw new ArgumentOutOfRangeException(nameof(value)),
    };

    private static byte[] EncodeHardwareTerminalBodyPayload(KagemushaHardwareTerminalBodyV1 value) => Fields(
        U16(value.Version), Fixed(value.CandidateEnvelopeDigest), Fixed(value.LifecycleBindingDigest),
        Fixed(value.TransitionNullifier), Fixed(value.OutboxReservationCommitment),
        EncodeCommitEvidencePayload(value.CommitEvidence), Fixed(value.HardwareProfileId), U64(value.PolicyEpoch),
        Fixed(value.PrivateSuccessorCommitment), Fixed(value.PrivateJournalCommitment),
        Fixed(value.PrivateRecoveryCommitment));

    private static byte[] EncodeCommitCertificatePayload(KagemushaCommitCertificateV1 value) => Fields(
        U16(value.Version), Fixed(value.CertificateId), Fixed(value.CandidateEnvelopeDigest),
        Fixed(value.LifecycleBindingDigest), Fixed(value.TransitionNullifier),
        Fixed(value.OutboxReservationCommitment), EncodeCommitEvidencePayload(value.CommitEvidence),
        Fixed(value.HardwareProfileId), U64(value.PolicyEpoch), Fixed(value.HardwareTerminalCommitment));

    private static byte[] EncodeRedemptionProofPayload(KagemushaRedemptionProofV1 value) => Fields(
        U16(value.Version), Fixed(value.EqProtocolDigest), Fixed(value.EpProtocolDigest),
        Fixed(value.SemanticDigest), Fixed(value.CandidateEnvelopeDigest),
        Fixed(value.CommitCertificateDigest), Fixed(value.EqDeferredAudit), Fixed(value.EpDeferredAudit),
        Vector(value.EqProof.Span), Vector(value.EpProof.Span), Vector(value.EqHistory.Span),
        Vector(value.EpHistory.Span));

    private static byte[] EncodePaymentProofPayload(KagemushaPaymentProofV1 value) => Fields(
        U16(value.Version), Fixed(value.EqProtocolDigest), Fixed(value.EpProtocolDigest),
        Fixed(value.SemanticDigest), Fixed(value.CandidateEnvelopeDigest),
        Fixed(value.CommitCertificateDigest), Fixed(value.EqDeferredAudit), Fixed(value.EpDeferredAudit),
        Vector(value.EqProof.Span), Vector(value.EpProof.Span), Vector(value.EqHistory.Span),
        Vector(value.EpHistory.Span));

    private static byte[] EncodePaymentOutputPayload(KagemushaPaymentOutputV1 value) => Fields(
        U16(value.Version), Fixed(value.RequestDigest), U128(value.Amount),
        Fixed(value.SenderBeforeCommitment), Fixed(value.SenderAfterCommitment),
        Fixed(value.TransitionNullifier), Fixed(value.CreditId), Fixed(value.CiphertextCommitment),
        EncodeCommitEvidencePayload(value.CommitEvidence), U64(value.CommittedAtMilliseconds));

    private static byte[] EncodePaymentPayload(KagemushaPaymentV1 value) => Fields(
        U16(value.Version), EncodePaymentOutputPayload(value.Output),
        Vector(value.EncryptedCredit.Span), EncodeCommitCertificatePayload(value.CommitCertificate),
        EncodePaymentProofPayload(value.Proof));

    private static byte[] EncodeReceiptPayload(KagemushaInboxReceiptV1 value) => Fields(
        U16(value.Version), Fixed(value.CreditId), Fixed(value.ReceiptCommitment));

    private static byte[] EncodeAcknowledgementPayload(KagemushaAcknowledgementV1 value) => Fields(
        U16(value.Version), Fixed(value.RequestDigest), Fixed(value.PaymentDigest), EncodeReceiptPayload(value.InboxReceipt),
        value.Signature.RawBytes());

    private static byte[] EncodeMintContextPayload(KagemushaMintAuthorizationContextV1 value) => Fields(
        U16(value.Version), Fixed(value.OperationId), Fixed(value.ReleaseId), Fixed(value.SuiteId), Fixed(value.VkDigest),
        Fixed(value.ArtifactManifestDigest), value.NetworkId.ToBytes(), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale), Fixed(value.LiabilityPoolId),
        U128(value.Amount),
        value.Payer.CanonicalPayload(), value.Recipient.CanonicalPayload(), Fixed(value.HardwareCredentialId),
        Fixed(value.HardwareProfileId), U64(value.PolicyEpoch), Fixed(value.RecipientCredentialCommitment),
        Fixed(value.CreditCommitment), value.RecipientOneTimeKey.Bytes());

    private static byte[] EncodeMintAuthorizationStatementPayload(KagemushaMintAuthorizationStatementV1 value) => Fields(
        U16(value.Version), EncodeMintContextPayload(value.Context), Fixed(value.IssuanceCommitment),
        Fixed(value.CreditId), Fixed(value.CiphertextDigest));

    private static byte[] EncodeMintAuthorizationPayload(KagemushaMintAuthorizationV1 value) => Fields(
        U16(value.Version), EncodeMintAuthorizationStatementPayload(value.Statement), EncodeProofPayload(value.Proof));

    private static byte[] EncodeMintStatementPayload(KagemushaMintCreditStatementV1 value) => Fields(
        U16(value.Version), EncodeLifecyclePayload(value.Lifecycle), Fixed(value.RecipientCredentialCommitment),
        Fixed(value.AuthorizationContextDigest), Fixed(value.MintAuthorizationDigest), U128(value.Amount),
        Fixed(value.IssuanceCommitment), value.Recipient.CanonicalPayload(), Fixed(value.CreditCommitment),
        U64(value.MintedAtMilliseconds));

    private static byte[] EncodeMintCreditPayload(KagemushaMintCreditV1 value) => Fields(
        U16(value.Version), EncodeMintStatementPayload(value.Statement), EncodeProofPayload(value.Proof),
        Fixed(value.FinalityCertificateBinding), Fixed(value.FinalityAuthorityHead), Fixed(value.FinalityGenesisRosterId),
        Fixed(value.FinalityProofBindingDigest), Vector(value.EncryptedCredit.Span), Fixed(value.ArtifactManifestDigest));

    private static byte[] EncodeRedemptionStatementPayload(KagemushaRedemptionStatementV1 value) => Fields(
        U16(value.Version), EncodeLifecyclePayload(value.Lifecycle), U128(value.Amount), value.Beneficiary.CanonicalPayload(),
        Fixed(value.TerminalNullifier), Fixed(value.RedemptionCommitment), Fixed(value.RedemptionId),
        EncodeCommitEvidencePayload(value.CommitEvidence));

    private static byte[] EncodeRedemptionVoucherPayload(KagemushaRedemptionVoucherV1 value) => Fields(
        U16(value.Version), EncodeRedemptionStatementPayload(value.Statement),
        EncodeCommitCertificatePayload(value.CommitCertificate), EncodeRedemptionProofPayload(value.Proof),
        Fixed(value.ArtifactManifestDigest));

    private static byte[] EncodeTopUpRequestPayload(KagemushaTopUpRequestV1 value) => Fields(
        U16(value.Version), Fixed(value.OperationId), Fixed(value.IssuanceCommitment), Fixed(value.CreditId),
        Fixed(value.ReleaseId), Fixed(value.SuiteId), Fixed(value.VkDigest), value.NetworkId.ToBytes(),
        value.Asset.CanonicalPayload(), EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale),
        U128(value.Amount),
        Fixed(value.LiabilityPoolId), value.Payer.CanonicalPayload(), value.Recipient.CanonicalPayload(),
        EncodeHardwareCredentialPayload(value.HardwareCredential), Fixed(value.RecipientCredentialCommitment),
        Fixed(value.CreditCommitment), value.RecipientOneTimeKey.Bytes(), Vector(value.EncryptedCredit.Span),
        Fixed(value.ArtifactManifestDigest), Option(value.MintAuthorization, EncodeMintAuthorizationPayload));

    private static KagemushaAggregateStateCommitmentV1 DecodeAggregatePayload(byte[] payload)
    {
        var reader = Reader(payload, "aggregate state");
        var value = new KagemushaAggregateStateCommitmentV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "releaseId"),
            ReadNetwork(ref reader, "networkId"), ReadAsset(ref reader, "asset"),
            ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadFixed32(ref reader, "liabilityPoolId"), ReadFixed32(ref reader, "laneId"),
            ReadFixed32(ref reader, "hardwareEpochId"), ReadFixed32(ref reader, "keyReference"),
            ReadFixed32(ref reader, "hardwarePolicyId"), ReadU128(ref reader, "sequence"),
            ReadFixed32(ref reader, "stateCommitment"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaPastaStateCommitmentV1 DecodePastaStatePayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "Pasta state commitment");
        var value = new KagemushaPastaStateCommitmentV1(
            ReadRaw32(ref reader, "eq"), ReadRaw32(ref reader, "ep"));
        reader.RequireEnd();
        ValidatePastaState(value);
        return value;
    }

    private static KagemushaPairedProofV1 DecodeProofPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "paired proof");
        var value = new KagemushaPairedProofV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "eqProtocolDigest"),
            ReadFixed32(ref reader, "epProtocolDigest"), ReadFixed32(ref reader, "semanticDigest"),
            ReadFixed32(ref reader, "guardEqCredentialAudit"), ReadFixed32(ref reader, "guardEpCredentialAudit"),
            ReadFixed32(ref reader, "eqDeferredAudit"), ReadFixed32(ref reader, "epDeferredAudit"),
            ReadVector(ref reader, "eqProof"), ReadVector(ref reader, "epProof"),
            ReadVector(ref reader, "eqHistory"), ReadVector(ref reader, "epHistory"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaHardwareProfileV1 DecodeHardwareProfilePayload(byte[] payload)
    {
        var reader = Reader(payload, "hardware profile");
        var value = new KagemushaHardwareProfileV1(
            ReadU16(ref reader, "version"), ReadU16(ref reader, "protocolVersion"),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadFixed32(ref reader, "providerId"),
            ReadUnitEnum<KagemushaHardwarePlatformClassV1>(ref reader, 4, "platformClass"),
            ReadFixed32(ref reader, "productClassDigest"), ReadFixed32(ref reader, "firmwarePolicyDigest"),
            ReadFixed32(ref reader, "enrollmentAttestationVerifierDigest"),
            ReadFixed32(ref reader, "attestationTrustRootsDigest"), ReadFixed32(ref reader, "allowedSuiteCommitment"),
            ReadU64(ref reader, "policyEpoch"), ReadPublicKey(ref reader, "governanceCredentialPublicKey"),
            ReadU16(ref reader, "capabilityMask"), ReadFixed32(ref reader, "qualificationReportDigest"),
            ReadU64(ref reader, "validFrom"), ReadU64(ref reader, "expiresAt"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaHardwareCredentialV1 DecodeHardwareCredentialPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "hardware credential");
        var value = new KagemushaHardwareCredentialV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "credentialId"), ReadNetwork(ref reader, "networkId"),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadFixed32(ref reader, "suiteId"),
            ReadFixed32(ref reader, "firmwarePolicyDigest"), ReadU64(ref reader, "policyEpoch"),
            ReadFixed32(ref reader, "laneCommitment"), ReadFixed32(ref reader, "hardwareEpochId"),
            ReadU64(ref reader, "hardwareEpochGeneration"), ReadPublicKey(ref reader, "devicePublicKey"),
            ReadFixed32(ref reader, "deviceKeyReference"), ReadU64(ref reader, "issuedAt"),
            ReadU64(ref reader, "expiresAt"), ReadSignature(ref reader, "governanceSignature"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaPaymentRequestV1 DecodeRequestPayload(byte[] payload)
    {
        var reader = Reader(payload, "payment request");
        var value = new KagemushaPaymentRequestV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "releaseId"), ReadNetwork(ref reader, "networkId"),
            ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadFixed32(ref reader, "liabilityPoolId"), ReadAccount(ref reader, "recipient"),
            ReadU128(ref reader, "amount"),
            new KagemushaX25519PublicKeyV1(ReadRaw32(ref reader, "recipientEncryptionKey")),
            DecodeHardwareCredentialPayload(reader.ReadField("hardwareCredential")),
            ReadFixed32(ref reader, "requestId"), ReadU64(ref reader, "issuedAt"), ReadU64(ref reader, "expiresAt"),
            ReadSignature(ref reader, "signature"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaPeerCreditContextV1 DecodePeerCreditContextPayload(byte[] payload)
    {
        var reader = Reader(payload, "peer credit context");
        var value = new KagemushaPeerCreditContextV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "requestDigest"),
            ReadU128(ref reader, "amount"),
            ReadFixed32(ref reader, "senderBeforeCommitment"),
            ReadFixed32(ref reader, "senderAfterCommitment"),
            ReadFixed32(ref reader, "preparedTransferDigest"),
            new KagemushaX25519PublicKeyV1(ReadRaw32(ref reader, "recipientEncryptionKey")));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaCreditOpeningV1 DecodeCreditOpeningPayload(byte[] payload)
    {
        var reader = Reader(payload, "credit opening");
        var value = new KagemushaCreditOpeningV1(ReadU16(ref reader, "version"), ReadFixed32(ref reader, "creditId"),
            ReadU128(ref reader, "amount"), ReadFixed32(ref reader, "creditCommitmentOpening"),
            ReadFixed32(ref reader, "recipientBindingOpening"), ReadFixed32(ref reader, "recoveryNonce"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaEncryptedCreditAadV1 DecodeCreditAadPayload(byte[] payload)
    {
        var reader = Reader(payload, "encrypted credit AAD");
        var value = new KagemushaEncryptedCreditAadV1(ReadU16(ref reader, "version"),
            ReadUnitEnum<KagemushaEncryptedCreditPurposeV1>(ref reader, 2, "purpose"),
            ReadFixed32(ref reader, "contextDigest"), ReadFixed32(ref reader, "issuanceOrTransitionCommitment"),
            ReadFixed32(ref reader, "creditId"), ReadU128(ref reader, "amount"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaEncryptedCreditEnvelopeV1 DecodeCreditEnvelopePayload(byte[] payload)
    {
        var reader = Reader(payload, "encrypted credit envelope");
        var value = new KagemushaEncryptedCreditEnvelopeV1(ReadU16(ref reader, "version"),
            new KagemushaX25519PublicKeyV1(ReadRaw32(ref reader, "ephemeralX25519PublicKey")),
            ReadFixedWidth(ref reader, 24, "nonce"), ReadVector(ref reader, "ciphertextAndTag"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaLifecycleBindingV1 DecodeLifecyclePayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "lifecycle");
        var value = new KagemushaLifecycleBindingV1(ReadU16(ref reader, "version"), ReadNetwork(ref reader, "networkId"),
            ReadU16(ref reader, "protocolVersion"), ReadFixed32(ref reader, "suiteId"), ReadFixed32(ref reader, "vkDigest"),
            ReadFixed32(ref reader, "releaseId"), ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"),
            ReadU32(ref reader, "scale"), ReadFixed32(ref reader, "liabilityPoolId"),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadU64(ref reader, "policyEpoch"),
            ReadUnitEnum<KagemushaOperationKindV1>(ref reader, 6, "operationKind"),
            ReadRaw32(ref reader, "requestId"), ReadRaw32(ref reader, "receiverLaneCommitment"),
            ReadRaw32(ref reader, "creditId"),
            ReadRaw32(ref reader, "ciphertextDigest"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaCommitEvidenceV1 DecodeCommitEvidencePayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "commit evidence");
        var tag = reader.ReadUInt32LittleEndian("source");
        var evidencePayload = reader.ReadField("evidence");
        reader.RequireEnd();
        var evidence = Reader(evidencePayload, "commit evidence payload");
        KagemushaCommitEvidenceV1 value = tag switch
        {
            0 => new KagemushaTrustedCommitTimeV1(ReadFixed32(ref evidence, "timeEvidenceCommitment")),
            1 => new KagemushaMonotonicLeaseV1(ReadFixed32(ref evidence, "leaseEvidenceCommitment")),
            _ => throw new ArgumentException("KAGEMUSHA V1 commit evidence tag is invalid.", nameof(payload)),
        };
        evidence.RequireEnd();
        return value;
    }

    private static KagemushaCommitCertificateV1 DecodeCommitCertificatePayload(byte[] payload)
    {
        var reader = Reader(payload, "commit certificate");
        var value = new KagemushaCommitCertificateV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "certificateId"),
            ReadFixed32(ref reader, "candidateEnvelopeDigest"),
            ReadFixed32(ref reader, "lifecycleBindingDigest"),
            ReadFixed32(ref reader, "transitionNullifier"),
            ReadFixed32(ref reader, "outboxReservationCommitment"),
            DecodeCommitEvidencePayload(reader.ReadField("commitEvidence")),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadU64(ref reader, "policyEpoch"),
            ReadFixed32(ref reader, "hardwareTerminalCommitment"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaRedemptionProofV1 DecodeRedemptionProofPayload(byte[] payload)
    {
        var reader = Reader(payload, "redemption proof");
        var value = new KagemushaRedemptionProofV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "eqProtocolDigest"),
            ReadFixed32(ref reader, "epProtocolDigest"), ReadFixed32(ref reader, "semanticDigest"),
            ReadFixed32(ref reader, "candidateEnvelopeDigest"),
            ReadFixed32(ref reader, "commitCertificateDigest"),
            ReadFixed32(ref reader, "eqDeferredAudit"), ReadFixed32(ref reader, "epDeferredAudit"),
            ReadVector(ref reader, "eqProof"), ReadVector(ref reader, "epProof"),
            ReadVector(ref reader, "eqHistory"), ReadVector(ref reader, "epHistory"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaPaymentOutputV1 DecodePaymentOutputPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "payment output");
        var value = new KagemushaPaymentOutputV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "requestDigest"), ReadU128(ref reader, "amount"),
            ReadFixed32(ref reader, "senderBeforeCommitment"),
            ReadFixed32(ref reader, "senderAfterCommitment"),
            ReadFixed32(ref reader, "transitionNullifier"),
            ReadFixed32(ref reader, "creditId"),
            ReadFixed32(ref reader, "ciphertextCommitment"),
            DecodeCommitEvidencePayload(reader.ReadField("commitEvidence")),
            ReadU64(ref reader, "committedAt"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaPaymentV1 DecodePaymentPayload(byte[] payload)
    {
        var reader = Reader(payload, "payment");
        var value = new KagemushaPaymentV1(ReadU16(ref reader, "version"),
            DecodePaymentOutputPayload(reader.ReadField("output")),
            ReadVector(ref reader, "encryptedCredit"),
            DecodeCommitCertificatePayload(reader.ReadField("commitCertificate").ToArray()),
            DecodePaymentProofPayload(reader.ReadField("proof").ToArray()));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaPaymentProofV1 DecodePaymentProofPayload(byte[] payload)
    {
        var reader = Reader(payload, "payment proof");
        var value = new KagemushaPaymentProofV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "eqProtocolDigest"),
            ReadFixed32(ref reader, "epProtocolDigest"), ReadFixed32(ref reader, "semanticDigest"),
            ReadFixed32(ref reader, "candidateEnvelopeDigest"),
            ReadFixed32(ref reader, "commitCertificateDigest"),
            ReadFixed32(ref reader, "eqDeferredAudit"), ReadFixed32(ref reader, "epDeferredAudit"),
            ReadVector(ref reader, "eqProof"), ReadVector(ref reader, "epProof"),
            ReadVector(ref reader, "eqHistory"), ReadVector(ref reader, "epHistory"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaInboxReceiptV1 DecodeReceiptPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "inbox receipt");
        var value = new KagemushaInboxReceiptV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "creditId"), ReadFixed32(ref reader, "receiptCommitment"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaAcknowledgementV1 DecodeAcknowledgementPayload(byte[] payload)
    {
        var reader = Reader(payload, "acknowledgement");
        var value = new KagemushaAcknowledgementV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "requestDigest"), ReadFixed32(ref reader, "paymentDigest"),
            DecodeReceiptPayload(reader.ReadField("inboxReceipt")), ReadSignature(ref reader, "signature"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaMintAuthorizationContextV1 DecodeMintContextPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "mint authorization context");
        var value = new KagemushaMintAuthorizationContextV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "operationId"), ReadFixed32(ref reader, "releaseId"),
            ReadFixed32(ref reader, "suiteId"), ReadFixed32(ref reader, "vkDigest"),
            ReadFixed32(ref reader, "artifactManifestDigest"), ReadNetwork(ref reader, "networkId"),
            ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadFixed32(ref reader, "liabilityPoolId"), ReadU128(ref reader, "amount"), ReadAccount(ref reader, "payer"),
            ReadAccount(ref reader, "recipient"), ReadFixed32(ref reader, "hardwareCredentialId"),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadU64(ref reader, "policyEpoch"),
            ReadFixed32(ref reader, "recipientCredentialCommitment"), ReadFixed32(ref reader, "creditCommitment"),
            new KagemushaX25519PublicKeyV1(ReadRaw32(ref reader, "recipientOneTimeKey")));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaMintAuthorizationStatementV1 DecodeMintAuthorizationStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "mint authorization statement");
        var value = new KagemushaMintAuthorizationStatementV1(ReadU16(ref reader, "version"),
            DecodeMintContextPayload(reader.ReadField("context")), ReadFixed32(ref reader, "issuanceCommitment"),
            ReadFixed32(ref reader, "creditId"), ReadFixed32(ref reader, "ciphertextDigest"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaMintAuthorizationV1 DecodeMintAuthorizationPayload(byte[] payload)
    {
        var reader = Reader(payload, "mint authorization");
        var value = new KagemushaMintAuthorizationV1(ReadU16(ref reader, "version"),
            DecodeMintAuthorizationStatementPayload(reader.ReadField("statement")), DecodeProofPayload(reader.ReadField("proof")));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaMintCreditStatementV1 DecodeMintStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "mint statement");
        var value = new KagemushaMintCreditStatementV1(ReadU16(ref reader, "version"),
            DecodeLifecyclePayload(reader.ReadField("lifecycle")), ReadFixed32(ref reader, "recipientCredentialCommitment"),
            ReadFixed32(ref reader, "authorizationContextDigest"), ReadFixed32(ref reader, "mintAuthorizationDigest"),
            ReadU128(ref reader, "amount"), ReadFixed32(ref reader, "issuanceCommitment"), ReadAccount(ref reader, "recipient"),
            ReadFixed32(ref reader, "creditCommitment"), ReadU64(ref reader, "mintedAt"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaMintCreditV1 DecodeMintCreditPayload(byte[] payload)
    {
        var reader = Reader(payload, "mint credit");
        var value = new KagemushaMintCreditV1(ReadU16(ref reader, "version"),
            DecodeMintStatementPayload(reader.ReadField("statement")), DecodeProofPayload(reader.ReadField("proof")),
            ReadFixed32(ref reader, "finalityCertificateBinding"), ReadFixed32(ref reader, "finalityAuthorityHead"),
            ReadFixed32(ref reader, "finalityGenesisRosterId"), ReadFixed32(ref reader, "finalityProofBindingDigest"),
            ReadVector(ref reader, "encryptedCredit"), ReadFixed32(ref reader, "artifactManifestDigest"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaRedemptionStatementV1 DecodeRedemptionStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "redemption statement");
        var value = new KagemushaRedemptionStatementV1(ReadU16(ref reader, "version"),
            DecodeLifecyclePayload(reader.ReadField("lifecycle")), ReadU128(ref reader, "amount"),
            ReadAccount(ref reader, "beneficiary"), ReadFixed32(ref reader, "terminalNullifier"),
            ReadFixed32(ref reader, "redemptionCommitment"), ReadFixed32(ref reader, "redemptionId"),
            DecodeCommitEvidencePayload(reader.ReadField("commitEvidence")));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaRedemptionVoucherV1 DecodeRedemptionVoucherPayload(byte[] payload)
    {
        var reader = Reader(payload, "redemption voucher");
        var value = new KagemushaRedemptionVoucherV1(ReadU16(ref reader, "version"),
            DecodeRedemptionStatementPayload(reader.ReadField("statement")),
            DecodeCommitCertificatePayload(reader.ReadField("commitCertificate").ToArray()),
            DecodeRedemptionProofPayload(reader.ReadField("proof").ToArray()),
            ReadFixed32(ref reader, "artifactManifestDigest"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaTopUpRequestV1 DecodeTopUpRequestPayload(byte[] payload)
    {
        var reader = Reader(payload, "top-up request");
        var value = new KagemushaTopUpRequestV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "operationId"), ReadFixed32(ref reader, "issuanceCommitment"),
            ReadFixed32(ref reader, "creditId"), ReadFixed32(ref reader, "releaseId"),
            ReadFixed32(ref reader, "suiteId"), ReadFixed32(ref reader, "vkDigest"), ReadNetwork(ref reader, "networkId"),
            ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadU128(ref reader, "amount"), ReadFixed32(ref reader, "liabilityPoolId"), ReadAccount(ref reader, "payer"),
            ReadAccount(ref reader, "recipient"), DecodeHardwareCredentialPayload(reader.ReadField("hardwareCredential")),
            ReadFixed32(ref reader, "recipientCredentialCommitment"), ReadFixed32(ref reader, "creditCommitment"),
            new KagemushaX25519PublicKeyV1(ReadRaw32(ref reader, "recipientOneTimeKey")),
            ReadVector(ref reader, "encryptedCredit"), ReadFixed32(ref reader, "artifactManifestDigest"),
            ReadOption(ref reader, DecodeMintAuthorizationPayload, "mintAuthorization"));
        reader.RequireEnd();
        return value;
    }

    private static KagemushaRedemptionRequestV1 DecodeRedemptionRequestPayload(byte[] payload)
    {
        var reader = Reader(payload, "redemption request");
        var value = new KagemushaRedemptionRequestV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "operationId"),
            DecodeRedemptionVoucherPayload(reader.ReadField("voucher").ToArray()));
        reader.RequireEnd();
        return value;
    }

    private static T DecodeExact<T>(
        ReadOnlySpan<byte> archive,
        int maximum,
        string schema,
        Func<byte[], T> decode,
        Func<T, byte[]> encode)
    {
        if (archive.IsEmpty || archive.Length > maximum)
            throw new ArgumentException("KAGEMUSHA V1 archive is empty or oversized.", nameof(archive));
        var (payload, flags) = NoritoCodec.Decode(schema, archive);
        if (flags != NoritoCodec.CanonicalLayoutFlags)
            throw new ArgumentException("KAGEMUSHA V1 archive has noncanonical layout flags.", nameof(archive));
        var value = decode(payload);
        if (!archive.SequenceEqual(encode(value)))
            throw new ArgumentException("KAGEMUSHA V1 archive is not canonical.", nameof(archive));
        return value;
    }

    private static KagemushaDeviceMintStageCommandV1 DecodeDeviceMintStageCommandPayload(byte[] payload)
    {
        var reader = Reader(payload, "device mint-stage command");
        var version = ReadU16(ref reader, "version");
        var authorization = ReadBoundedVector(ref reader, MaximumMintAuthorizationBytes, "canonicalAuthorization");
        var credit = ReadBoundedVector(ref reader, MaximumMintCreditBytes, "canonicalMintCredit");
        reader.RequireEnd();
        return new KagemushaDeviceMintStageCommandV1(version, authorization, credit);
    }

    private static KagemushaDeviceMintStageResultV1 DecodeDeviceMintStageResultPayload(byte[] payload)
    {
        var reader = Reader(payload, "device mint-stage result");
        var version = ReadU16(ref reader, "version");
        var disposition = reader.ReadField("disposition");
        if (disposition.Length != 1) throw new ArgumentException("disposition must be u8.");
        var creditId = ReadFixed32(ref reader, "creditId");
        reader.RequireEnd();
        return new KagemushaDeviceMintStageResultV1(version, disposition[0], creditId);
    }

    private static CanonicalNoritoReader Reader(ReadOnlySpan<byte> payload, string context) =>
        new(payload, $"KAGEMUSHA V1 {context}", nameof(payload));

    private static ushort ReadU16(ref CanonicalNoritoReader reader, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length != 2) throw new ArgumentException($"{field} must be u16.");
        return BinaryPrimitives.ReadUInt16LittleEndian(payload);
    }

    private static uint ReadU32(ref CanonicalNoritoReader reader, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length != 4) throw new ArgumentException($"{field} must be u32.");
        return BinaryPrimitives.ReadUInt32LittleEndian(payload);
    }

    private static ulong ReadU64(ref CanonicalNoritoReader reader, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length != 8) throw new ArgumentException($"{field} must be u64.");
        return BinaryPrimitives.ReadUInt64LittleEndian(payload);
    }

    private static UInt128 ReadU128(ref CanonicalNoritoReader reader, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length != 16) throw new ArgumentException($"{field} must be u128.");
        return BinaryPrimitives.ReadUInt128LittleEndian(payload);
    }

    private static byte[] ReadFixed32(ref CanonicalNoritoReader reader, string field) =>
        KagemushaModelValidation.Fixed32(ReadRaw32(ref reader, field).AsSpan(), field);

    private static byte[] ReadRaw32(ref CanonicalNoritoReader reader, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length != 32) throw new ArgumentException($"{field} must be exactly 32 bytes.");
        return payload.ToArray();
    }

    private static byte[] ReadFixedWidth(ref CanonicalNoritoReader reader, int width, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length != width) throw new ArgumentException($"{field} must be exactly {width} bytes.");
        return payload.ToArray();
    }

    private static NetworkId ReadNetwork(ref CanonicalNoritoReader reader, string field) =>
        NetworkId.FromBytes(reader.ReadField(field));
    private static KagemushaAssetDefinitionIdV1 ReadAsset(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));
    private static KagemushaAssetIncarnationV1 ReadIncarnation(
        ref CanonicalNoritoReader reader,
        string field)
    {
        var incarnation = Reader(reader.ReadField(field), field);
        var value = new KagemushaAssetIncarnationV1(ReadRaw32(ref incarnation, "hash"));
        incarnation.RequireEnd();
        return value;
    }
    private static KagemushaAccountIdV1 ReadAccount(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));
    private static KagemushaDevicePublicKeyV1 ReadPublicKey(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));
    private static KagemushaDeviceSignatureV1 ReadSignature(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));

    private static byte[] ReadVector(ref CanonicalNoritoReader reader, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length < 8) throw new ArgumentException($"{field} is truncated.");
        var length = BinaryPrimitives.ReadUInt64LittleEndian(payload);
        if (length != (ulong)payload.Length - 8) throw new ArgumentException($"{field} length does not match.");
        return payload[8..].ToArray();
    }

    private static byte[] ReadBoundedVector(ref CanonicalNoritoReader reader, int maximum, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length < 8) throw new ArgumentException($"{field} is truncated.");
        var length = BinaryPrimitives.ReadUInt64LittleEndian(payload);
        if (length == 0 || length > (ulong)maximum || length != (ulong)payload.Length - 8)
            throw new ArgumentException($"{field} is empty, oversized, or has a mismatched length.");
        return payload[8..].ToArray();
    }

    private static TEnum ReadUnitEnum<TEnum>(
        ref CanonicalNoritoReader reader,
        uint variants,
        string field) where TEnum : struct, Enum
    {
        var payload = reader.ReadField(field);
        if (payload.Length != 4) throw new ArgumentException($"{field} enum is malformed.");
        var tag = BinaryPrimitives.ReadUInt32LittleEndian(payload);
        if (tag >= variants) throw new ArgumentException($"{field} enum tag is invalid.");
        return (TEnum)Enum.ToObject(typeof(TEnum), tag);
    }

    private static T? ReadOption<T>(
        ref CanonicalNoritoReader reader,
        Func<byte[], T> decode,
        string field) where T : class
    {
        var payload = reader.ReadField(field);
        var option = Reader(payload, $"{field} option");
        var tag = option.ReadByte("tag");
        if (tag == 0)
        {
            option.RequireEnd();
            return null;
        }
        if (tag != 1) throw new ArgumentException($"{field} option tag is invalid.");
        var value = decode(option.ReadField("value").ToArray());
        option.RequireEnd();
        return value;
    }

    private static byte[] Frame(string schema, ReadOnlySpan<byte> payload, int alignment)
    {
        var archive = NoritoCodec.Encode(schema, payload, NoritoCodec.CanonicalLayoutFlags);
        var padding = (alignment - NoritoHeader.EncodedLength % alignment) % alignment;
        if (padding == 0) return archive;
        var result = new byte[archive.Length + padding];
        archive.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(result);
        archive.AsSpan(NoritoHeader.EncodedLength).CopyTo(result.AsSpan(NoritoHeader.EncodedLength + padding));
        return result;
    }

    private static byte[] Fields(params byte[][] values)
    {
        var writer = new CanonicalNoritoWriter();
        foreach (var value in values) writer.WriteField(value);
        return writer.ToArray();
    }

    private static byte[] EncodeAssetIncarnationPayload(KagemushaAssetIncarnationV1 value) =>
        Fields(Raw32(value.Bytes()));

    private static byte[] EnumPayload(uint tag, byte[] payload)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(tag);
        writer.WriteField(payload);
        return writer.ToArray();
    }

    private static byte[] Option<T>(T? value, Func<T, byte[]> encode) where T : class
    {
        var writer = new CanonicalNoritoWriter();
        if (value is null)
        {
            writer.WriteByte(0);
            return writer.ToArray();
        }
        writer.WriteByte(1);
        writer.WriteField(encode(value));
        return writer.ToArray();
    }

    private static byte[] Fixed(ReadOnlyMemory<byte> value) =>
        KagemushaModelValidation.Fixed32(value, "fixed32");
    private static byte[] Raw32(ReadOnlyMemory<byte> value) =>
        KagemushaModelValidation.Raw32(value.Span, "raw32");
    private static byte[] FixedWidth(ReadOnlyMemory<byte> value, int width, string field)
    {
        if (value.Length != width) throw new ArgumentException($"{field} must be exactly {width} bytes.");
        return value.ToArray();
    }

    private static byte[] U16(ushort value)
    {
        var result = new byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(result, value);
        return result;
    }

    private static byte[] U32(uint value)
    {
        var result = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(result, value);
        return result;
    }

    private static byte[] U64(ulong value)
    {
        var result = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(result, value);
        return result;
    }

    private static byte[] U64(int value) => U64(checked((ulong)value));

    private static byte[] U128(UInt128 value)
    {
        var result = new byte[16];
        BinaryPrimitives.WriteUInt128LittleEndian(result, value);
        return result;
    }

    private static byte[] Vector(ReadOnlySpan<byte> value)
    {
        var result = new byte[checked(8 + value.Length)];
        BinaryPrimitives.WriteUInt64LittleEndian(result, (ulong)value.Length);
        value.CopyTo(result.AsSpan(8));
        return result;
    }

    private static byte[] DigestEncoded(ReadOnlySpan<byte> domain, ReadOnlySpan<byte> canonical) =>
        Hash(domain.ToArray(), [0], U64(canonical.Length), canonical.ToArray());
    private static byte[] Hash(params byte[][] values) => SHA256.HashData(Join(values));
    private static byte[] Ascii(string value) => Encoding.ASCII.GetBytes(value);

    private static byte[] Join(params byte[][] values)
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

    private static byte[] PaymentRequestDigestUnchecked(KagemushaPaymentRequestV1 value) =>
        DigestEncoded(RequestDigestDomain, PaymentRequestCircuitTranscript(value));

    /// <summary>Return the fixed 390-byte signed request semantic transcript, not Norito wire bytes.</summary>
    public static byte[] PaymentRequestCircuitTranscript(KagemushaPaymentRequestV1 value) =>
        Join(PaymentRequestUnsignedTranscript(value), value.Signature.RawBytes());

    /// <summary>Return the exact native request-signing domain and unsigned semantic transcript.</summary>
    public static byte[] PaymentRequestSigningBytes(KagemushaPaymentRequestV1 value) =>
        Join(Ascii("iroha:kagemusha:v1:payment-request-signing"), [0], PaymentRequestUnsignedTranscript(value));

    private static byte[] PaymentRequestUnsignedTranscript(KagemushaPaymentRequestV1 value) =>
        Join(U16(value.Version), Fixed(value.ReleaseId), value.NetworkId.ToBytes(),
            DigestEncoded(Ascii("iroha:kagemusha:v1:asset-identity"),
                Frame("iroha_data_model::asset::id::model::AssetDefinitionId", value.Asset.CanonicalPayload(), 1)),
            value.AssetIncarnation.Bytes(), U32(value.Scale), Fixed(value.LiabilityPoolId),
            DigestEncoded(Ascii("iroha:kagemusha:v1:account-identity"),
                Frame("iroha_data_model::account::model::AccountId", value.Recipient.CanonicalPayload(), 8)),
            U128(value.Amount), value.RecipientEncryptionKey.Bytes(),
            Fixed(value.HardwareCredential.CredentialId),
            Fixed(value.RequestId), U64(value.IssuedAtMilliseconds), U64(value.ExpiresAtMilliseconds));
    private static byte[] CommitEvidenceCircuitTranscript(KagemushaCommitEvidenceV1 value) =>
        Join(U32(value.WireTag), Fixed(value.Commitment));
    private static byte[] OutboxReservationCircuitTranscript(KagemushaOutboxReservationV1 value) =>
        Join(Fixed(value.ReservationId), U32((uint)value.OperationKind), U32(value.ReservedOutboxBytes),
            U64(value.IssuedAtMilliseconds), U64(value.ExpiresAtMilliseconds));
    private static byte[] CommitCertificateIdCircuitTranscript(KagemushaCommitCertificateV1 value) =>
        Join(U16(value.Version), Fixed(value.CandidateEnvelopeDigest), Fixed(value.LifecycleBindingDigest),
            Fixed(value.TransitionNullifier), Fixed(value.OutboxReservationCommitment),
            CommitEvidenceCircuitTranscript(value.CommitEvidence), Fixed(value.HardwareProfileId),
            U64(value.PolicyEpoch), Fixed(value.HardwareTerminalCommitment));
    private static byte[] CommitCertificateDigestUnchecked(KagemushaCommitCertificateV1 value) =>
        DigestEncoded(CommitCertificateDigestDomain,
            Join(U16(value.Version), Fixed(value.CertificateId), Fixed(value.CandidateEnvelopeDigest),
                Fixed(value.LifecycleBindingDigest), Fixed(value.TransitionNullifier),
                Fixed(value.OutboxReservationCommitment), CommitEvidenceCircuitTranscript(value.CommitEvidence),
                Fixed(value.HardwareProfileId), U64(value.PolicyEpoch), Fixed(value.HardwareTerminalCommitment)));
    private static byte[] PaymentDigestUnchecked(KagemushaPaymentV1 value) =>
        DigestEncoded(PaymentDigestDomain, Frame(PaymentSchema, EncodePaymentPayload(value), 16));
    private static byte[] PaymentBodyDigestUnchecked(KagemushaPaymentV1 value) =>
        PaymentBodyDigestUnchecked(value.Output, value.EncryptedCredit.Span);
    private static byte[] PaymentBodyDigestUnchecked(KagemushaPaymentOutputV1 output, ReadOnlySpan<byte> ciphertext) =>
        DigestEncoded(Ascii("iroha:kagemusha:v1:payment-body"),
            Join(PaymentOutputDigestUnchecked(output), CiphertextDigest(ciphertext)));

    /// <summary>Commit the exact output and ciphertext before a proof or certificate exists.</summary>
    public static byte[] PaymentBodyDigest(KagemushaPaymentOutputV1 output, ReadOnlySpan<byte> ciphertext,
        KagemushaPaymentRequestV1 request)
    {
        ValidatePaymentOutput(output, request);
        _ = DecodeEncryptedCreditEnvelope(ciphertext);
        return PaymentBodyDigestUnchecked(output, ciphertext);
    }
    internal static byte[] LifecycleBindingDigestUnchecked(KagemushaLifecycleBindingV1 value) =>
        DigestEncoded(LifecycleDigestDomain, Frame(LifecycleSchema, EncodeLifecyclePayload(value), 8));
    internal static byte[] PaymentOutputDigestUnchecked(KagemushaPaymentOutputV1 value) =>
        DigestEncoded(PaymentOutputDigestDomain,
            Join(U16(value.Version), Fixed(value.RequestDigest), U128(value.Amount),
                Fixed(value.SenderBeforeCommitment), Fixed(value.SenderAfterCommitment),
                Fixed(value.TransitionNullifier), Fixed(value.CreditId), Fixed(value.CiphertextCommitment),
                CommitEvidenceCircuitTranscript(value.CommitEvidence), U64(value.CommittedAtMilliseconds)));
    private static byte[] RedemptionIdUnchecked(KagemushaRedemptionStatementV1 value) =>
        DigestEncoded(
            RedemptionIdDomain,
            Frame(
                RedemptionIdPreimageSchema,
                Fields(LifecycleBindingDigestUnchecked(value.Lifecycle), Fixed(value.TerminalNullifier),
                    U128(value.Amount), value.Beneficiary.CanonicalPayload(),
                    Fixed(value.RedemptionCommitment)),
                16));
    private static byte[] RedemptionStatementDigestUnchecked(KagemushaRedemptionStatementV1 value) =>
        DigestEncoded(
            RedemptionStatementDigestDomain,
            Frame(RedemptionStatementSchema, EncodeRedemptionStatementPayload(value), 16));
    private static byte[] MintAuthorizationContextDigestUnchecked(
        KagemushaMintAuthorizationContextV1 value) =>
        DigestEncoded(
            MintAuthorizationContextDigestDomain,
            Frame(MintAuthorizationContextSchema, EncodeMintContextPayload(value), 16));
    private static byte[] MintAuthorizationStatementDigestUnchecked(
        KagemushaMintAuthorizationStatementV1 value) =>
        DigestEncoded(
            MintAuthorizationStatementDigestDomain,
            Frame(MintAuthorizationStatementSchema, EncodeMintAuthorizationStatementPayload(value), 16));
    private static byte[] MintAuthorizationDigestUnchecked(KagemushaMintAuthorizationV1 value) =>
        DigestEncoded(
            MintAuthorizationDigestDomain,
            Frame(MintAuthorizationSchema, EncodeMintAuthorizationPayload(value), 16));
    private static byte[] MintStatementDigestUnchecked(KagemushaMintCreditStatementV1 value) =>
        DigestEncoded(
            MintStatementDigestDomain,
            Frame(MintCreditStatementSchema, EncodeMintStatementPayload(value), 16));

    private static bool IsZero32(ReadOnlyMemory<byte> value) =>
        value.Length == 32 && value.Span.IndexOfAnyExcept((byte)0) < 0;
    private static bool IsNonzero32(ReadOnlyMemory<byte> value) =>
        value.Length == 32 && value.Span.IndexOfAnyExcept((byte)0) >= 0;
    private static int TextLength(int rawBytes) => TextPrefix.Length + rawBytes / 3 * 4 + (rawBytes % 3 switch { 0 => 0, 1 => 2, _ => 3 });

    private static byte[] Bounded(byte[] value, int maximum, string name)
    {
        if (value.Length > maximum) throw new ArgumentException($"KAGEMUSHA V1 {name} exceeds {maximum} bytes.", name);
        return value;
    }

    private static void RequireEqual(ReadOnlySpan<byte> actual, ReadOnlySpan<byte> expected, string name)
    {
        if (!actual.SequenceEqual(expected)) throw new ArgumentException($"KAGEMUSHA V1 {name} does not match.");
    }

    private static (int Raw, int Text) Limits(PayloadKind kind) => kind switch
    {
        PayloadKind.PaymentRequest => (MaximumRequestBytes, 1_243),
        PayloadKind.Payment => (MaximumPaymentBytes, 10_075),
        PayloadKind.Acknowledgement => (MaximumAcknowledgementBytes, 347),
        PayloadKind.MintAuthorization => (MaximumMintAuthorizationBytes, 10_587),
        PayloadKind.MintCredit => (MaximumMintCreditBytes, 10_587),
        PayloadKind.RedemptionVoucher => (MaximumRedemptionVoucherBytes, 10_587),
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };
}
