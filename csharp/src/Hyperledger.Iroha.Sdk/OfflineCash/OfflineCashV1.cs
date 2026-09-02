using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.OfflineCash;

/// <summary>
/// Canonical Offline Cash V1 codecs and non-authoritative shape checks.
/// Monetary verification and every state transition belong to the audited native core.
/// </summary>
public static class OfflineCashV1
{
    public const ushort WireVersion = 1;
    public const ushort DeviceLifecycleVersion = 1;
    public const string HandoffCapability = "cash_handoff_v1";
    public const string TextPrefix = "oc1:";
    public const int MaximumAssetScale = 28;
    public const ulong RequestMaximumTtlMilliseconds = 5 * 60 * 1_000;
    public const int MaximumAggregateBytes = 768;
    public const int MaximumRequestBytes = 1_024;
    public const int MaximumAcceptanceIntentBytes = 256;
    public const int MaximumAcceptanceIntentAuthorizationBytes = 7_936;
    public const int MaximumNoCommitClosureBytes = 16_384;
    public const int MaximumAcceptanceTicketBytes = 1_024;
    public const int MaximumPaymentBytes = 7_936;
    public const int MaximumAcknowledgementBytes = 512;
    public const int MaximumMintAuthorizationBytes = 7_936;
    public const int MaximumMintCreditBytes = 7_936;
    public const int MaximumRedemptionVoucherBytes = 7_936;
    public const int MaximumPairedProofBytes = 6_528;
    public const int MaximumCurrentProofsBytes = 4_990;
    public const int MaximumParityProofBytes = 2_495;
    public const int HistoryAccumulatorBytes = 544;
    public const int MaximumEncryptedCreditBytes = 384;
    public const int MaximumCreditOpeningBytes = 256;
    public const int CreditOpeningCanonicalBytes = 200;
    public const int EncryptedCreditCiphertextAndTagBytes = 216;
    public const int MaximumSessionRawBytes = 9_211;
    public const int MaximumSessionTextBytes = 12_288;
    public const int MaximumPreTicketExchangeRawBytes = 9_984;
    public const int MaximumPreTicketExchangeTextBytes = 13_326;
    public const int MaximumCompleteExchangeRawBytes = 18_171;
    public const int MaximumCompleteExchangeTextBytes = 24_244;
    public const int AcceptanceTicketMinimumReservedInboxBytes = 8_960;
    public const int PaymentOutboxMinimumBytes = 26_112;
    public const int RedemptionOutboxMinimumBytes = 26_112;
    public const int MaximumTopUpRequestBytes = 4_096;
    public const int MaximumRedemptionRequestBytes = 8_192;

    private const string Model = "iroha_data_model::offline::offline_cash_v1::";
    private const string AggregateSchema = Model + "OfflineCashAggregateStateCommitmentV1";
    private const string HardwareProfileSchema = Model + "OfflineCashHardwareProfileV1";
    private const string HardwareCredentialSchema = Model + "OfflineCashHardwareCredentialV1";
    private const string RequestSchema = Model + "OfflineCashPaymentRequestV1";
    private const string IntentSchema = Model + "OfflineCashAcceptanceIntentV1";
    private const string IntentAuthorizationSchema = Model + "OfflineCashAcceptanceIntentAuthorizationV1";
    private const string NoCommitClosureSchema = Model + "OfflineCashNoCommitClosureV1";
    private const string TicketSchema = Model + "OfflineCashAcceptanceTicketV1";
    private const string LifecycleSchema = Model + "OfflineCashLifecycleBindingV1";
    private const string PeerCreditContextSchema = Model + "OfflineCashPeerCreditContextV1";
    private const string CreditOpeningSchema = Model + "OfflineCashCreditOpeningV1";
    private const string CreditAadSchema = Model + "OfflineCashEncryptedCreditAadV1";
    private const string CreditEnvelopeSchema = Model + "OfflineCashEncryptedCreditEnvelopeV1";
    private const string StatementSchema = Model + "OfflineCashTransferStatementV1";
    private const string PaymentSchema = Model + "OfflineCashPaymentV1";
    private const string AcknowledgementSchema = Model + "OfflineCashAcknowledgementV1";
    private const string MintAuthorizationSchema = Model + "OfflineCashMintAuthorizationV1";
    private const string MintAuthorizationContextSchema = Model + "OfflineCashMintAuthorizationContextV1";
    private const string MintAuthorizationStatementSchema = Model + "OfflineCashMintAuthorizationStatementV1";
    private const string MintCreditStatementSchema = Model + "OfflineCashMintCreditStatementV1";
    private const string MintCreditSchema = Model + "OfflineCashMintCreditV1";
    private const string RedemptionStatementSchema = Model + "OfflineCashRedemptionStatementV1";
    private const string RedemptionVoucherSchema = Model + "OfflineCashRedemptionVoucherV1";
    private const string RedemptionIdPreimageSchema = "iroha.offline-cash.v1.redemption-id-preimage";
    private const string TopUpRequestSchema = "iroha.torii.v1.offline_cash.top_up.request";
    private const string RedemptionRequestSchema = "iroha.torii.v1.offline_cash.redeem.request";

    private static readonly byte[] DeviceKeyReferenceDomain = Ascii("iroha:offline-cash:v1:device-key-reference");
    private static readonly byte[] PastaStateCommitmentDomain = Ascii("iroha:offline-cash:v1:pasta-state-commitment");
    private static readonly byte[] LiabilityPoolDomain = Ascii("iroha:offline-cash:v1:liability-pool");
    private static readonly byte[] RequestDigestDomain = Ascii("iroha:offline-cash:v1:payment-request");
    private static readonly byte[] IntentDigestDomain = Ascii("iroha:offline-cash:v1:acceptance-intent");
    private static readonly byte[] IntentAuthorizationStatementDigestDomain =
        Ascii("iroha:offline-cash:v1:acceptance-intent-authorization-statement");
    private static readonly byte[] IntentAuthorizationDigestDomain =
        Ascii("iroha:offline-cash:v1:acceptance-intent-authorization");
    private static readonly byte[] NoCommitClosureStatementDigestDomain =
        Ascii("iroha:offline-cash:v1:no-commit-closure-statement");
    private static readonly byte[] TicketDigestDomain = Ascii("iroha:offline-cash:v1:acceptance-ticket");
    private static readonly byte[] LifecycleDigestDomain = Ascii("iroha:offline-cash:v1:lifecycle-binding");
    private static readonly byte[] StatementDigestDomain = Ascii("iroha:offline-cash:v1:send-split-statement");
    private static readonly byte[] PaymentDigestDomain = Ascii("iroha:offline-cash:v1:payment");
    private static readonly byte[] CommitCertificateIdDomain =
        Ascii("iroha:offline-cash:v1:commit-certificate-id");
    private static readonly byte[] CommitCertificateDigestDomain =
        Ascii("iroha:offline-cash:v1:commit-certificate");
    private static readonly byte[] OutboxReservationCommitmentDomain =
        Ascii("iroha:offline-cash:v1:outbox-reservation");
    private static readonly byte[] CiphertextDigestDomain = Ascii("iroha:offline-cash:v1:ciphertext");
    private static readonly byte[] MintAuthorizationContextDigestDomain =
        Ascii("iroha:offline-cash:v1:mint-authorization-context");
    private static readonly byte[] MintAuthorizationStatementDigestDomain =
        Ascii("iroha:offline-cash:v1:mint-authorization-statement");
    private static readonly byte[] MintAuthorizationDigestDomain =
        Ascii("iroha:offline-cash:v1:mint-authorization");
    private static readonly byte[] MintStatementDigestDomain = Ascii("iroha:offline-cash:v1:mint-statement");
    private static readonly byte[] RedemptionIdDomain = Ascii("iroha:offline-cash:v1:redemption-id");
    private static readonly byte[] RedemptionStatementDigestDomain =
        Ascii("iroha:offline-cash:v1:redemption-statement");

    public enum PayloadKind
    {
        PaymentRequest,
        AcceptanceIntent,
        AcceptanceIntentAuthorization,
        NoCommitClosure,
        AcceptanceTicket,
        Payment,
        Acknowledgement,
        MintAuthorization,
        MintCredit,
        RedemptionVoucher,
    }

    public static byte[] EncodeAggregateState(OfflineCashAggregateStateCommitmentV1 value)
    {
        ValidateAggregate(value);
        return Bounded(Frame(AggregateSchema, EncodeAggregatePayload(value), 16), MaximumAggregateBytes, nameof(value));
    }

    public static OfflineCashAggregateStateCommitmentV1 DecodeAggregateState(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumAggregateBytes, AggregateSchema, DecodeAggregatePayload, EncodeAggregateState);

    public static byte[] EncodeHardwareProfile(OfflineCashHardwareProfileV1 value)
    {
        ValidateHardwareProfile(value);
        return Bounded(Frame(HardwareProfileSchema, EncodeHardwareProfilePayload(value), 1), 512, nameof(value));
    }

    public static OfflineCashHardwareProfileV1 DecodeHardwareProfile(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 512, HardwareProfileSchema, DecodeHardwareProfilePayload, EncodeHardwareProfile);

    public static byte[] EncodeHardwareCredential(OfflineCashHardwareCredentialV1 value)
    {
        ValidateHardwareCredential(value);
        return Bounded(Frame(HardwareCredentialSchema, EncodeHardwareCredentialPayload(value), 1), 768, nameof(value));
    }

    public static OfflineCashHardwareCredentialV1 DecodeHardwareCredential(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 768, HardwareCredentialSchema,
            bytes => DecodeHardwareCredentialPayload(bytes), EncodeHardwareCredential);

    public static byte[] EncodePaymentRequest(OfflineCashPaymentRequestV1 value)
    {
        ValidateRequest(value);
        return Bounded(Frame(RequestSchema, EncodeRequestPayload(value), 16), MaximumRequestBytes, nameof(value));
    }

    public static OfflineCashPaymentRequestV1 DecodePaymentRequest(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumRequestBytes, RequestSchema, DecodeRequestPayload, EncodePaymentRequest);

    public static byte[] EncodeAcceptanceIntent(
        OfflineCashAcceptanceIntentV1 value,
        OfflineCashPaymentRequestV1 request)
    {
        ValidateIntent(value, request);
        return Bounded(Frame(IntentSchema, EncodeIntentPayload(value), 16), MaximumAcceptanceIntentBytes, nameof(value));
    }

    public static OfflineCashAcceptanceIntentV1 DecodeAcceptanceIntent(
        ReadOnlySpan<byte> archive,
        OfflineCashPaymentRequestV1 request) => DecodeExact(
            archive,
            MaximumAcceptanceIntentBytes,
            IntentSchema,
            bytes => DecodeIntentPayload(bytes),
            value => EncodeAcceptanceIntent(value, request));

    public static byte[] EncodeAcceptanceIntentAuthorization(
        OfflineCashAcceptanceIntentAuthorizationV1 value,
        OfflineCashPaymentRequestV1 request)
    {
        ValidateIntentAuthorization(value, request);
        return Bounded(
            Frame(IntentAuthorizationSchema, EncodeIntentAuthorizationPayload(value), 16),
            MaximumAcceptanceIntentAuthorizationBytes,
            nameof(value));
    }

    public static OfflineCashAcceptanceIntentAuthorizationV1 DecodeAcceptanceIntentAuthorization(
        ReadOnlySpan<byte> archive,
        OfflineCashPaymentRequestV1 request) => DecodeExact(
            archive,
            MaximumAcceptanceIntentAuthorizationBytes,
            IntentAuthorizationSchema,
            DecodeIntentAuthorizationPayload,
            value => EncodeAcceptanceIntentAuthorization(value, request));

    public static byte[] EncodeNoCommitClosure(OfflineCashNoCommitClosureV1 value)
    {
        ValidateNoCommitClosure(value);
        return Bounded(Frame(NoCommitClosureSchema, EncodeNoCommitClosurePayload(value), 16),
            MaximumNoCommitClosureBytes, nameof(value));
    }

    public static OfflineCashNoCommitClosureV1 DecodeNoCommitClosure(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumNoCommitClosureBytes, NoCommitClosureSchema,
            DecodeNoCommitClosurePayload, EncodeNoCommitClosure);

    public static byte[] EncodeAcceptanceTicket(
        OfflineCashAcceptanceTicketV1 value,
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentAuthorizationV1 authorization)
    {
        ValidateIntentAuthorization(authorization, request);
        ValidateTicket(value, request, authorization.Statement.Intent);
        return Bounded(Frame(TicketSchema, EncodeTicketPayload(value), 16), MaximumAcceptanceTicketBytes, nameof(value));
    }

    public static OfflineCashAcceptanceTicketV1 DecodeAcceptanceTicket(
        ReadOnlySpan<byte> archive,
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentAuthorizationV1 authorization) => DecodeExact(
            archive,
            MaximumAcceptanceTicketBytes,
            TicketSchema,
            bytes => DecodeTicketPayload(bytes),
            value => EncodeAcceptanceTicket(value, request, authorization));

    public static byte[] EncodePayment(OfflineCashPaymentV1 value, OfflineCashPaymentRequestV1 request)
    {
        ValidatePayment(value, request);
        return Bounded(Frame(PaymentSchema, EncodePaymentPayload(value), 16), MaximumPaymentBytes, nameof(value));
    }

    public static OfflineCashPaymentV1 DecodePayment(ReadOnlySpan<byte> archive, OfflineCashPaymentRequestV1 request) =>
        DecodeExact(archive, MaximumPaymentBytes, PaymentSchema, DecodePaymentPayload,
            value => EncodePayment(value, request));

    public static byte[] EncodeAcknowledgement(
        OfflineCashAcknowledgementV1 value,
        OfflineCashPaymentRequestV1 request,
        OfflineCashPaymentV1 payment)
    {
        ValidateAcknowledgement(value, request, payment);
        return Bounded(Frame(AcknowledgementSchema, EncodeAcknowledgementPayload(value), 1),
            MaximumAcknowledgementBytes, nameof(value));
    }

    public static OfflineCashAcknowledgementV1 DecodeAcknowledgement(
        ReadOnlySpan<byte> archive,
        OfflineCashPaymentRequestV1 request,
        OfflineCashPaymentV1 payment) => DecodeExact(
            archive,
            MaximumAcknowledgementBytes,
            AcknowledgementSchema,
            DecodeAcknowledgementPayload,
            value => EncodeAcknowledgement(value, request, payment));

    public static byte[] EncodeMintAuthorization(OfflineCashMintAuthorizationV1 value)
    {
        ValidateMintAuthorization(value);
        return Bounded(Frame(MintAuthorizationSchema, EncodeMintAuthorizationPayload(value), 16),
            MaximumMintAuthorizationBytes, nameof(value));
    }

    public static OfflineCashMintAuthorizationV1 DecodeMintAuthorization(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumMintAuthorizationBytes, MintAuthorizationSchema,
            DecodeMintAuthorizationPayload, EncodeMintAuthorization);

    public static byte[] EncodeMintCredit(OfflineCashMintCreditV1 value)
    {
        ValidateMintCredit(value, null);
        return Bounded(Frame(MintCreditSchema, EncodeMintCreditPayload(value), 16), MaximumMintCreditBytes, nameof(value));
    }

    public static byte[] EncodeMintCredit(
        OfflineCashMintCreditV1 value,
        OfflineCashMintAuthorizationV1 authorization)
    {
        ValidateMintCredit(value, authorization);
        return Bounded(Frame(MintCreditSchema, EncodeMintCreditPayload(value), 16), MaximumMintCreditBytes, nameof(value));
    }

    public static OfflineCashMintCreditV1 DecodeMintCredit(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumMintCreditBytes, MintCreditSchema, DecodeMintCreditPayload, EncodeMintCredit);

    public static OfflineCashMintCreditV1 DecodeMintCredit(
        ReadOnlySpan<byte> archive,
        OfflineCashMintAuthorizationV1 authorization) => DecodeExact(
            archive,
            MaximumMintCreditBytes,
            MintCreditSchema,
            DecodeMintCreditPayload,
            value => EncodeMintCredit(value, authorization));

    public static byte[] EncodePeerCreditContext(OfflineCashPeerCreditContextV1 value)
    {
        ValidateVersion(value.Version);
        _ = Fixed(value.RequestDigest);
        _ = Fixed(value.AcceptanceIntentDigest);
        _ = Fixed(value.AcceptanceTicketDigest);
        _ = Fixed(value.LifecycleContextDigest);
        return Frame(PeerCreditContextSchema, EncodePeerCreditContextPayload(value), 1);
    }

    public static OfflineCashPeerCreditContextV1 DecodePeerCreditContext(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 512, PeerCreditContextSchema,
            DecodePeerCreditContextPayload, EncodePeerCreditContext);

    public static byte[] EncodeRedemptionVoucher(OfflineCashRedemptionVoucherV1 value)
    {
        ValidateRedemptionVoucher(value);
        return Bounded(Frame(RedemptionVoucherSchema, EncodeRedemptionVoucherPayload(value), 16),
            MaximumRedemptionVoucherBytes, nameof(value));
    }

    public static OfflineCashRedemptionVoucherV1 DecodeRedemptionVoucher(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumRedemptionVoucherBytes, RedemptionVoucherSchema,
            DecodeRedemptionVoucherPayload, EncodeRedemptionVoucher);

    public static byte[] EncodeCreditOpening(OfflineCashCreditOpeningV1 value)
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

    public static OfflineCashCreditOpeningV1 DecodeCreditOpening(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumCreditOpeningBytes, CreditOpeningSchema, DecodeCreditOpeningPayload, EncodeCreditOpening);

    public static byte[] EncodeEncryptedCreditAad(OfflineCashEncryptedCreditAadV1 value)
    {
        ValidateCreditAad(value);
        return Frame(CreditAadSchema, EncodeCreditAadPayload(value), 16);
    }

    public static OfflineCashEncryptedCreditAadV1 DecodeEncryptedCreditAad(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, 512, CreditAadSchema, DecodeCreditAadPayload, EncodeEncryptedCreditAad);

    public static byte[] EncodeEncryptedCreditEnvelope(OfflineCashEncryptedCreditEnvelopeV1 value)
    {
        ValidateCreditEnvelope(value);
        return Bounded(Frame(CreditEnvelopeSchema, EncodeCreditEnvelopePayload(value), 1), MaximumEncryptedCreditBytes, nameof(value));
    }

    public static OfflineCashEncryptedCreditEnvelopeV1 DecodeEncryptedCreditEnvelope(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumEncryptedCreditBytes, CreditEnvelopeSchema,
            DecodeCreditEnvelopePayload, EncodeEncryptedCreditEnvelope);

    public static byte[] EncodeTopUpRequest(OfflineCashTopUpRequestV1 value)
    {
        ValidateTopUpRequest(value);
        return Bounded(Frame(TopUpRequestSchema, EncodeTopUpRequestPayload(value), 16), MaximumTopUpRequestBytes, nameof(value));
    }

    public static OfflineCashTopUpRequestV1 DecodeTopUpRequest(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumTopUpRequestBytes, TopUpRequestSchema,
            DecodeTopUpRequestPayload, EncodeTopUpRequest);

    public static byte[] EncodeRedemptionRequest(OfflineCashRedemptionRequestV1 value)
    {
        ValidateVersion(value.Version);
        _ = Fixed(value.OperationId);
        ValidateRedemptionVoucher(value.Voucher);
        return Bounded(Frame(RedemptionRequestSchema, Fields(U16(value.Version), Fixed(value.OperationId),
            EncodeRedemptionVoucherPayload(value.Voucher)), 16), MaximumRedemptionRequestBytes, nameof(value));
    }

    public static OfflineCashRedemptionRequestV1 DecodeRedemptionRequest(ReadOnlySpan<byte> archive) =>
        DecodeExact(archive, MaximumRedemptionRequestBytes, RedemptionRequestSchema,
            DecodeRedemptionRequestPayload, EncodeRedemptionRequest);

    public static string EncodeText(PayloadKind kind, ReadOnlySpan<byte> canonicalPayload)
    {
        var (maximumRaw, maximumText) = Limits(kind);
        if (canonicalPayload.IsEmpty || canonicalPayload.Length > maximumRaw)
            throw new ArgumentException("Offline Cash V1 payload is empty or oversized.", nameof(canonicalPayload));
        var body = Convert.ToBase64String(canonicalPayload).TrimEnd('=').Replace('+', '-').Replace('/', '_');
        var text = TextPrefix + body;
        if (text.Length > maximumText) throw new ArgumentException("Offline Cash V1 text is oversized.", nameof(canonicalPayload));
        return text;
    }

    public static byte[] DecodeText(PayloadKind kind, string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        var (maximumRaw, maximumText) = Limits(kind);
        if (text.Length > maximumText || !text.StartsWith(TextPrefix, StringComparison.Ordinal))
            throw new FormatException("Offline Cash V1 text prefix or size is invalid.");
        var body = text[TextPrefix.Length..];
        if (body.Length == 0 || body.Length % 4 == 1 || body.Any(static value =>
                !(value is >= 'A' and <= 'Z' or >= 'a' and <= 'z' or >= '0' and <= '9' or '-' or '_')))
            throw new FormatException("Offline Cash V1 text is not canonical unpadded base64url.");
        var padded = body.Replace('-', '+').Replace('_', '/').PadRight((body.Length + 3) / 4 * 4, '=');
        byte[] raw;
        try { raw = Convert.FromBase64String(padded); }
        catch (FormatException error) { throw new FormatException("Offline Cash V1 base64url is invalid.", error); }
        if (raw.Length > maximumRaw || EncodeText(kind, raw) != text)
            throw new FormatException("Offline Cash V1 text is not canonical.");
        return raw;
    }

    public static byte[] DeviceKeyReference(OfflineCashDevicePublicKeyV1 publicKey) =>
        Hash(DeviceKeyReferenceDomain, [0], publicKey.Sec1Bytes());

    public static byte[] PastaStateCommitment(OfflineCashPastaStateCommitmentV1 value)
    {
        ValidatePastaState(value);
        return Hash(PastaStateCommitmentDomain, [0], value.Eq.ToArray(), value.Ep.ToArray());
    }

    public static byte[] LiabilityPoolId(
        NetworkId networkId,
        OfflineCashAssetDefinitionIdV1 asset,
        OfflineCashAssetIncarnationV1 incarnation) => DigestEncoded(
            LiabilityPoolDomain,
            Frame("iroha.offline-cash.v1.liability-pool-preimage",
                Fields(networkId.ToBytes(), asset.CanonicalPayload(),
                    EncodeAssetIncarnationPayload(incarnation)), 1));

    public static byte[] PaymentRequestDigest(OfflineCashPaymentRequestV1 value)
    {
        ValidateRequest(value);
        return DigestEncoded(RequestDigestDomain, Frame(RequestSchema, EncodeRequestPayload(value), 16));
    }

    public static byte[] AcceptanceIntentDigest(
        OfflineCashAcceptanceIntentV1 value,
        OfflineCashPaymentRequestV1 request)
    {
        ValidateIntent(value, request);
        return IntentDigestUnchecked(value);
    }

    /// <summary>Return the canonical authorization-statement digest; this does not verify its proof.</summary>
    public static byte[] AcceptanceIntentAuthorizationStatementDigest(
        OfflineCashAcceptanceIntentAuthorizationStatementV1 value,
        OfflineCashPaymentRequestV1 request)
    {
        ValidateIntent(value.Intent, request);
        if (value.Version != WireVersion)
            throw new ArgumentException("Offline Cash V1 authorization statement version differs.", nameof(value));
        _ = Fixed(value.ReleaseId);
        _ = Fixed(value.SuiteId);
        _ = Fixed(value.VkDigest);
        _ = Fixed(value.ArtifactManifestDigest);
        RequireEqual(value.ReleaseId.Span, request.ReleaseId.Span, "authorization release");
        RequireEqual(value.SuiteId.Span, request.HardwareCredential.SuiteId.Span, "authorization suite");
        return IntentAuthorizationStatementDigestUnchecked(value);
    }

    /// <summary>Return the canonical proof-bearing authorization digest; this does not verify its proof.</summary>
    public static byte[] AcceptanceIntentAuthorizationDigest(
        OfflineCashAcceptanceIntentAuthorizationV1 value,
        OfflineCashPaymentRequestV1 request)
    {
        ValidateIntentAuthorization(value, request);
        return IntentAuthorizationDigestUnchecked(value);
    }

    /// <summary>Return the semantic digest an authenticated no-commit closure proof must carry.</summary>
    public static byte[] NoCommitClosureStatementDigest(OfflineCashNoCommitClosureStatementV1 value)
    {
        ValidateVersion(value.Version);
        if (value.ExactAmount == 0)
            throw new ArgumentException("Offline Cash V1 no-commit closure amount is invalid.", nameof(value));
        foreach (var field in new[] { value.ReleaseId, value.SuiteId, value.VkDigest,
                     value.ArtifactManifestDigest, value.SenderHardwareBindingCommitment, value.RequestId,
                     value.RequestDigest, value.AcceptanceTicketId, value.TicketDigest,
                     value.IntentAuthorizationDigest, value.IntentDigest, value.SenderOneTimeCommitment,
                     value.RecoveryId, value.CancellationNullifier,
                     value.EquivalentDeliverySlotCommitment })
            _ = Fixed(field);
        return NoCommitClosureStatementDigestUnchecked(value);
    }

    /// <summary>Return the fixed-width commitment proven for a durable sender-outbox reservation.</summary>
    public static byte[] OutboxReservationCommitment(OfflineCashOutboxReservationV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        _ = Fixed(value.ReservationId);
        var minimum = value.OperationKind switch
        {
            OfflineCashOperationKindV1.SendSplit => (uint)PaymentOutboxMinimumBytes,
            OfflineCashOperationKindV1.RedeemSplit => (uint)RedemptionOutboxMinimumBytes,
            _ => throw new ArgumentException(
                "Offline Cash V1 outbox reservation has no terminal-envelope operation.", nameof(value)),
        };
        if (value.ReservedOutboxBytes < minimum
            || value.IssuedAtMilliseconds >= value.ExpiresAtMilliseconds)
            throw new ArgumentException("Offline Cash V1 outbox reservation is invalid.", nameof(value));
        return DigestEncoded(
            OutboxReservationCommitmentDomain,
            OutboxReservationCircuitTranscript(value));
    }

    public static byte[] AcceptanceTicketDigest(
        OfflineCashAcceptanceTicketV1 value,
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentV1 intent)
    {
        ValidateTicket(value, request, intent);
        return DigestEncoded(TicketDigestDomain, Frame(TicketSchema, EncodeTicketPayload(value), 16));
    }

    public static byte[] PaymentDigest(OfflineCashPaymentV1 value, OfflineCashPaymentRequestV1 request)
    {
        ValidatePayment(value, request);
        return DigestEncoded(PaymentDigestDomain, Frame(PaymentSchema, EncodePaymentPayload(value), 16));
    }

    /// <summary>Return the canonical encrypted-envelope digest without decrypting or authenticating it.</summary>
    public static byte[] CiphertextDigest(ReadOnlySpan<byte> encryptedCredit) =>
        DigestEncoded(CiphertextDigestDomain, encryptedCredit);

    /// <summary>Return the canonical mint-authorization context digest; this grants no monetary authority.</summary>
    public static byte[] MintAuthorizationContextDigest(OfflineCashMintAuthorizationContextV1 value)
    {
        ValidateMintContext(value);
        return MintAuthorizationContextDigestUnchecked(value);
    }

    /// <summary>Return the semantic digest a mint-authorization proof must carry.</summary>
    public static byte[] MintAuthorizationStatementDigest(OfflineCashMintAuthorizationStatementV1 value)
    {
        ValidateMintContext(value.Context);
        if (value.Version != WireVersion || value.Context.Version != value.Version)
            throw new ArgumentException("Offline Cash V1 mint authorization statement version differs.", nameof(value));
        _ = Fixed(value.IssuanceCommitment);
        _ = Fixed(value.CreditId);
        _ = Fixed(value.CiphertextDigest);
        return MintAuthorizationStatementDigestUnchecked(value);
    }

    /// <summary>Return the canonical mint-authorization digest; this does not verify its proof.</summary>
    public static byte[] MintAuthorizationDigest(OfflineCashMintAuthorizationV1 value)
    {
        ValidateMintAuthorization(value);
        return MintAuthorizationDigestUnchecked(value);
    }

    /// <summary>Return the semantic digest a mint-credit proof must carry.</summary>
    public static byte[] MintCreditStatementDigest(OfflineCashMintCreditStatementV1 value)
    {
        ValidateLifecycle(value.Lifecycle);
        if (value.Version != WireVersion || value.Lifecycle.OperationKind != OfflineCashOperationKindV1.MintFold
            || value.Amount == 0 || value.MintedAtMilliseconds == 0)
            throw new ArgumentException("Offline Cash V1 mint credit statement is invalid.", nameof(value));
        return MintStatementDigestUnchecked(value);
    }

    public static int ValidateTerminalSession(
        OfflineCashPaymentRequestV1 request,
        OfflineCashPaymentV1 payment,
        OfflineCashAcknowledgementV1 acknowledgement)
    {
        var sizes = new[]
        {
            EncodePaymentRequest(request).Length,
            EncodePayment(payment, request).Length,
            EncodeAcknowledgement(acknowledgement, request, payment).Length,
        };
        var raw = sizes.Sum();
        if (raw > MaximumSessionRawBytes || sizes.Sum(TextLength) > MaximumSessionTextBytes)
            throw new ArgumentException("Offline Cash V1 terminal session is oversized.");
        return raw;
    }

    public static int ValidatePreTicketExchange(
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentAuthorizationV1 authorization,
        OfflineCashAcceptanceTicketV1 ticket)
    {
        var sizes = new[]
        {
            EncodePaymentRequest(request).Length,
            EncodeAcceptanceIntentAuthorization(authorization, request).Length,
            EncodeAcceptanceTicket(ticket, request, authorization).Length,
        };
        var raw = sizes.Sum();
        if (raw > MaximumPreTicketExchangeRawBytes || sizes.Sum(TextLength) > MaximumPreTicketExchangeTextBytes)
            throw new ArgumentException("Offline Cash V1 pre-ticket exchange is oversized.");
        return raw;
    }

    public static int ValidateCompleteExchange(
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentAuthorizationV1 authorization,
        OfflineCashAcceptanceTicketV1 ticket,
        OfflineCashPaymentV1 payment,
        OfflineCashAcknowledgementV1 acknowledgement)
    {
        ValidatePreTicketExchange(request, authorization, ticket);
        ValidateTerminalSession(request, payment, acknowledgement);
        RequireEqual(EncodeIntentPayload(payment.AcceptanceIntent),
            EncodeIntentPayload(authorization.Statement.Intent), "payment intent");
        RequireEqual(EncodeTicketPayload(payment.AcceptanceTicket), EncodeTicketPayload(ticket), "payment ticket");
        var sizes = new[]
        {
            EncodePaymentRequest(request).Length,
            EncodeAcceptanceIntentAuthorization(authorization, request).Length,
            EncodeAcceptanceTicket(ticket, request, authorization).Length,
            EncodePayment(payment, request).Length,
            EncodeAcknowledgement(acknowledgement, request, payment).Length,
        };
        var raw = sizes.Sum();
        if (raw > MaximumCompleteExchangeRawBytes || sizes.Sum(TextLength) > MaximumCompleteExchangeTextBytes)
            throw new ArgumentException("Offline Cash V1 complete exchange is oversized.");
        return raw;
    }

    [Obsolete("Use ValidateTerminalSession; Offline Cash V1 is a five-message flow.")]
    public static int ValidateSession(
        OfflineCashPaymentRequestV1 request,
        OfflineCashPaymentV1 payment,
        OfflineCashAcknowledgementV1 acknowledgement) => ValidateTerminalSession(request, payment, acknowledgement);

    private static void ValidateAggregate(OfflineCashAggregateStateCommitmentV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale);
        foreach (var field in new[] { value.ReleaseId, value.LiabilityPoolId, value.LaneId,
                     value.HardwareEpochId, value.KeyReference, value.HardwarePolicyId, value.StateCommitment })
            _ = Fixed(field);
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "aggregate liability pool");
    }

    private static void ValidateHardwareProfile(OfflineCashHardwareProfileV1 value)
    {
        ValidateVersion(value.Version);
        if (value.ProtocolVersion != WireVersion || value.PolicyEpoch == 0 || value.CapabilityMask != ushort.MaxValue
            || value.ExpiresAtMilliseconds <= value.ValidFromMilliseconds
            || !Enum.IsDefined(value.PlatformClass))
            throw new ArgumentException("Offline Cash V1 hardware profile is invalid.");
        foreach (var field in new[] { value.HardwareProfileId, value.ProviderId, value.ProductClassDigest,
                     value.FirmwarePolicyDigest, value.EnrollmentAttestationVerifierDigest,
                     value.AttestationTrustRootsDigest, value.AllowedSuiteCommitment,
                     value.QualificationReportDigest })
            _ = Fixed(field);
    }

    private static void ValidateHardwareCredential(OfflineCashHardwareCredentialV1 value)
    {
        ValidateVersion(value.Version);
        if (value.PolicyEpoch == 0 || value.ExpiresAtMilliseconds <= value.IssuedAtMilliseconds)
            throw new ArgumentException("Offline Cash V1 hardware credential is invalid.");
        foreach (var field in new[] { value.CredentialId, value.HardwareProfileId, value.SuiteId,
                     value.FirmwarePolicyDigest, value.LaneCommitment, value.HardwareEpochId,
                     value.DeviceKeyReference })
            _ = Fixed(field);
    }

    private static void ValidateRequest(OfflineCashPaymentRequestV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale, value.Amount);
        ValidateHardwareCredential(value.HardwareCredential);
        _ = Fixed(value.ReleaseId);
        _ = Fixed(value.LiabilityPoolId);
        _ = Fixed(value.RequestId);
        if (value.HardwareCredential.NetworkId != value.NetworkId
            || value.IssuedAtMilliseconds < value.HardwareCredential.IssuedAtMilliseconds
            || value.ExpiresAtMilliseconds > value.HardwareCredential.ExpiresAtMilliseconds
            || value.ExpiresAtMilliseconds <= value.IssuedAtMilliseconds
            || value.ExpiresAtMilliseconds - value.IssuedAtMilliseconds > RequestMaximumTtlMilliseconds)
            throw new ArgumentException("Offline Cash V1 request lifetime or credential binding is invalid.");
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "request liability pool");
        RequireEqual(value.HardwareCredential.DeviceKeyReference.Span,
            DeviceKeyReference(value.HardwareCredential.DevicePublicKey), "request device key reference");
    }

    private static void ValidateIntent(OfflineCashAcceptanceIntentV1 value, OfflineCashPaymentRequestV1 request)
    {
        ValidateRequest(request);
        ValidateVersion(value.Version);
        _ = Fixed(value.RequestDigest);
        _ = Fixed(value.IntentId);
        _ = Fixed(value.SenderOneTimeCommitment);
        RequireEqual(value.RequestDigest.Span, PaymentRequestDigestUnchecked(request), "intent request digest");
        if (value.ExactAmount != request.Amount)
            throw new ArgumentException("Offline Cash V1 intent amount differs from the request amount.");
    }

    private static void ValidateIntentAuthorization(
        OfflineCashAcceptanceIntentAuthorizationV1 value,
        OfflineCashPaymentRequestV1 request)
    {
        ValidateVersion(value.Version);
        ValidateIntent(value.Statement.Intent, request);
        if (value.Statement.Version != value.Version)
            throw new ArgumentException("Offline Cash V1 authorization statement version differs.");
        foreach (var field in new[] { value.Statement.ReleaseId, value.Statement.SuiteId,
                     value.Statement.VkDigest, value.Statement.ArtifactManifestDigest })
            _ = Fixed(field);
        RequireEqual(value.Statement.ReleaseId.Span, request.ReleaseId.Span, "authorization release");
        RequireEqual(value.Statement.SuiteId.Span, request.HardwareCredential.SuiteId.Span, "authorization suite");
        ValidateProof(value.Proof);
        RequireEqual(value.Proof.SemanticDigest.Span,
            IntentAuthorizationStatementDigestUnchecked(value.Statement),
            "authorization proof semantic digest");
    }

    private static void ValidateNoCommitClosure(OfflineCashNoCommitClosureV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.Statement.ExactAmount == 0)
            throw new ArgumentException("Offline Cash V1 no-commit closure header is invalid.");
        foreach (var field in new[] { value.Statement.ReleaseId, value.Statement.SuiteId,
                     value.Statement.VkDigest, value.Statement.ArtifactManifestDigest,
                     value.Statement.SenderHardwareBindingCommitment, value.Statement.RequestId,
                     value.Statement.RequestDigest, value.Statement.AcceptanceTicketId,
                     value.Statement.TicketDigest, value.Statement.IntentAuthorizationDigest,
                     value.Statement.IntentDigest, value.Statement.SenderOneTimeCommitment,
                     value.Statement.RecoveryId, value.Statement.CancellationNullifier,
                     value.Statement.EquivalentDeliverySlotCommitment })
            _ = Fixed(field);
        ValidateRequest(value.Request);
        ValidateIntentAuthorization(value.IntentAuthorization, value.Request);
        ValidateTicket(value.AcceptanceTicket, value.Request, value.IntentAuthorization.Statement.Intent);
        var intent = value.IntentAuthorization.Statement.Intent;
        RequireEqual(value.Statement.RequestId.Span, value.Request.RequestId.Span,
            "no-commit request id");
        RequireEqual(value.Statement.RequestDigest.Span, PaymentRequestDigestUnchecked(value.Request),
            "no-commit request digest");
        RequireEqual(value.Statement.AcceptanceTicketId.Span, value.AcceptanceTicket.AcceptanceTicketId.Span,
            "no-commit ticket id");
        RequireEqual(value.Statement.TicketDigest.Span, TicketDigestUnchecked(value.AcceptanceTicket),
            "no-commit ticket digest");
        RequireEqual(value.Statement.IntentAuthorizationDigest.Span,
            IntentAuthorizationDigestUnchecked(value.IntentAuthorization),
            "no-commit authorization digest");
        RequireEqual(value.Statement.IntentDigest.Span, IntentDigestUnchecked(intent),
            "no-commit intent digest");
        RequireEqual(value.Statement.SenderOneTimeCommitment.Span, intent.SenderOneTimeCommitment.Span,
            "no-commit sender commitment");
        RequireEqual(value.Statement.ReleaseId.Span, value.IntentAuthorization.Statement.ReleaseId.Span,
            "no-commit authorization release");
        RequireEqual(value.Statement.SuiteId.Span, value.IntentAuthorization.Statement.SuiteId.Span,
            "no-commit authorization suite");
        RequireEqual(value.Statement.VkDigest.Span, value.IntentAuthorization.Statement.VkDigest.Span,
            "no-commit authorization verifying key");
        RequireEqual(value.Statement.ArtifactManifestDigest.Span,
            value.IntentAuthorization.Statement.ArtifactManifestDigest.Span,
            "no-commit authorization artifact manifest");
        if (value.Statement.ExactAmount != intent.ExactAmount
            || value.Statement.ExactAmount != value.AcceptanceTicket.ExactAmount)
            throw new ArgumentException("Offline Cash V1 no-commit amount binding differs.");
        ValidateProof(value.Proof);
        RequireEqual(value.Proof.SemanticDigest.Span,
            NoCommitClosureStatementDigestUnchecked(value.Statement),
            "no-commit proof semantic digest");
    }

    private static void ValidateTicket(
        OfflineCashAcceptanceTicketV1 value,
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentV1 intent)
    {
        ValidateIntent(intent, request);
        ValidateHeader(value.Version, value.NetworkId, value.Scale, value.ExactAmount);
        foreach (var field in new[] { value.RequestId, value.RequestDigest, value.AcceptanceTicketId,
                     value.IntentDigest, value.HardwareProfileId })
            _ = Fixed(field);
        if (value.NetworkId != request.NetworkId || !value.Asset.Equals(request.Asset)
            || !value.AssetIncarnation.Equals(request.AssetIncarnation) || value.Scale != request.Scale
            || value.ExactAmount != request.Amount || value.ExactAmount != intent.ExactAmount
            || value.ReservedInboxBytes < AcceptanceTicketMinimumReservedInboxBytes
            || value.PolicyEpoch != request.HardwareCredential.PolicyEpoch
            || value.IssuedAtMilliseconds < request.IssuedAtMilliseconds
            || value.ExpiresAtMilliseconds > request.ExpiresAtMilliseconds
            || value.ExpiresAtMilliseconds <= value.IssuedAtMilliseconds)
            throw new ArgumentException("Offline Cash V1 ticket request binding is invalid.");
        RequireEqual(value.RequestId.Span, request.RequestId.Span, "ticket request id");
        RequireEqual(value.RequestDigest.Span, PaymentRequestDigestUnchecked(request), "ticket request digest");
        RequireEqual(value.IntentDigest.Span, IntentDigestUnchecked(intent), "ticket intent digest");
        RequireEqual(value.HardwareProfileId.Span,
            request.HardwareCredential.HardwareProfileId.Span, "ticket hardware profile");
    }

    private static void ValidatePayment(OfflineCashPaymentV1 value, OfflineCashPaymentRequestV1 request)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.AcceptanceIntent.Version != value.Version
            || value.AcceptanceTicket.Version != value.Version || value.CommitCertificate.Version != value.Version
            || value.Proof.Version != value.Version)
            throw new ArgumentException("Offline Cash V1 payment version binding is invalid.");
        ValidateIntent(value.AcceptanceIntent, request);
        ValidateTicket(value.AcceptanceTicket, request, value.AcceptanceIntent);
        ValidateTransferStatement(value.Statement, request, value.AcceptanceTicket);
        ValidateCommitCertificate(value.CommitCertificate, value.Statement.Lifecycle,
            value.Statement.TransitionNullifier, value.Statement.CommitEvidence);
        ValidateCommitWrapper(value.Proof, TransferStatementDigestUnchecked(value.Statement),
            value.CommitCertificate);
        _ = DecodeEncryptedCreditEnvelope(value.EncryptedCredit.Span);
        _ = Fixed(value.ArtifactManifestDigest);
        RequireEqual(value.Statement.Lifecycle.CiphertextDigest.Span,
            CiphertextDigest(value.EncryptedCredit.Span), "payment ciphertext digest");
    }

    private static void ValidateTransferStatement(
        OfflineCashTransferStatementV1 value,
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceTicketV1 ticket)
    {
        ValidateVersion(value.Version);
        ValidateLifecycle(value.Lifecycle);
        ValidateCommitEvidence(value.CommitEvidence);
        if (value.Amount == 0 || value.Amount != ticket.ExactAmount
            || value.Lifecycle.OperationKind != OfflineCashOperationKindV1.SendSplit
            || value.Lifecycle.NetworkId != request.NetworkId || !value.Lifecycle.Asset.Equals(request.Asset)
            || !value.Lifecycle.AssetIncarnation.Equals(request.AssetIncarnation)
            || value.Lifecycle.Scale != request.Scale)
            throw new ArgumentException("Offline Cash V1 transfer statement is invalid.");
        foreach (var field in new[] { value.TransitionNullifier, value.RequestDigest,
                     value.AcceptanceTicketDigest, value.CiphertextCommitment })
            _ = Fixed(field);
        RequireEqual(value.RequestDigest.Span, PaymentRequestDigestUnchecked(request), "payment request digest");
        RequireEqual(value.AcceptanceTicketDigest.Span, TicketDigestUnchecked(ticket), "payment ticket digest");
        RequireEqual(value.RecipientOneTimeKey.Bytes(), ticket.RecipientOneTimeKey.Bytes(), "payment recipient key");
        RequireEqual(value.Lifecycle.RequestId.Span, request.RequestId.Span, "payment lifecycle request");
        RequireEqual(value.Lifecycle.AcceptanceTicketId.Span,
            ticket.AcceptanceTicketId.Span, "payment lifecycle ticket");
    }

    private static void ValidateLifecycle(OfflineCashLifecycleBindingV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale);
        if (value.ProtocolVersion != WireVersion || value.PolicyEpoch == 0 || !Enum.IsDefined(value.OperationKind))
            throw new ArgumentException("Offline Cash V1 lifecycle is invalid.");
        foreach (var field in new[] { value.SuiteId, value.VkDigest, value.ReleaseId,
                     value.LiabilityPoolId, value.HardwareProfileId })
            _ = Fixed(field);
        var requestPresent = IsNonzero32(value.RequestId) && IsNonzero32(value.AcceptanceTicketId);
        var requestAbsent = IsZero32(value.RequestId) && IsZero32(value.AcceptanceTicketId);
        var creditPresent = IsNonzero32(value.CreditId) && IsNonzero32(value.CiphertextDigest);
        var creditAbsent = IsZero32(value.CreditId) && IsZero32(value.CiphertextDigest);
        var shapeValid = value.OperationKind switch
        {
            OfflineCashOperationKindV1.SendSplit => requestPresent && creditPresent,
            OfflineCashOperationKindV1.MintFold => requestAbsent && creditPresent,
            _ => requestAbsent && creditAbsent,
        };
        if (!shapeValid) throw new ArgumentException("Offline Cash V1 lifecycle operation fields are invalid.");
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "lifecycle liability pool");
    }

    private static void ValidateCommitCertificate(
        OfflineCashCommitCertificateV1 value,
        OfflineCashLifecycleBindingV1 lifecycle,
        ReadOnlyMemory<byte> transitionNullifier,
        OfflineCashCommitEvidenceV1 evidence)
    {
        ValidateVersion(value.Version);
        ValidateCommitEvidence(value.CommitEvidence);
        if (value.PolicyEpoch != lifecycle.PolicyEpoch)
            throw new ArgumentException("Offline Cash V1 commit-certificate policy epoch differs.");
        foreach (var field in new[] { value.CertificateId, value.CandidateEnvelopeDigest,
                     value.LifecycleBindingDigest, value.TransitionNullifier,
                     value.OutboxReservationCommitment, value.HardwareProfileId,
                     value.HardwareTerminalCommitment })
            _ = Fixed(field);
        RequireEqual(value.TransitionNullifier.Span, transitionNullifier.Span, "certificate nullifier");
        RequireEqual(value.LifecycleBindingDigest.Span,
            LifecycleBindingDigestUnchecked(lifecycle), "certificate lifecycle binding");
        RequireEqual(value.HardwareProfileId.Span, lifecycle.HardwareProfileId.Span, "certificate hardware profile");
        RequireEqual(EncodeCommitEvidencePayload(value.CommitEvidence), EncodeCommitEvidencePayload(evidence),
            "certificate commit evidence");
        RequireEqual(value.CertificateId.Span,
            CommitCertificateIdUnchecked(value), "certificate id");
    }

    private static void ValidateCommitWrapper(
        OfflineCashCommitWrapperProofV1 value,
        ReadOnlySpan<byte> semanticDigest,
        OfflineCashCommitCertificateV1 certificate)
    {
        ValidateVersion(value.Version);
        foreach (var field in new[] { value.EqProtocolDigest, value.EpProtocolDigest,
                     value.SemanticDigest, value.CandidateEnvelopeDigest,
                     value.CommitCertificateDigest, value.EqDeferredAudit, value.EpDeferredAudit })
            _ = Fixed(field);
        ValidateProofBytes(value.EqProof, value.EpProof, value.EqHistory, value.EpHistory);
        RequireEqual(value.SemanticDigest.Span, semanticDigest, "commit-wrapper semantic digest");
        RequireEqual(value.CandidateEnvelopeDigest.Span,
            certificate.CandidateEnvelopeDigest.Span, "commit-wrapper candidate digest");
        RequireEqual(value.CommitCertificateDigest.Span,
            CommitCertificateDigestUnchecked(certificate), "commit-wrapper certificate digest");
    }

    private static void ValidateProof(OfflineCashPairedProofV1 value)
    {
        ValidateVersion(value.Version);
        foreach (var field in new[] { value.EqProtocolDigest, value.EpProtocolDigest,
                     value.SemanticDigest, value.GuardEqCredentialAudit,
                     value.GuardEpCredentialAudit, value.EqDeferredAudit, value.EpDeferredAudit })
            _ = Fixed(field);
        ValidateProofBytes(value.EqProof, value.EpProof, value.EqHistory, value.EpHistory);
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
            throw new ArgumentException("Offline Cash V1 recursive proof bytes are invalid.");
    }

    private static void ValidatePastaState(OfflineCashPastaStateCommitmentV1 value)
    {
        if (value.Eq.Length != 32 || value.Ep.Length != 32)
            throw new ArgumentException("Offline Cash V1 Pasta state components must be exactly 32 bytes.");
        var eqZero = value.Eq.Span.IndexOfAnyExcept((byte)0) < 0;
        var epZero = value.Ep.Span.IndexOfAnyExcept((byte)0) < 0;
        if (eqZero != epZero) throw new ArgumentException("Offline Cash V1 Pasta state is half-present.");
    }

    private static void ValidateAcknowledgement(
        OfflineCashAcknowledgementV1 value,
        OfflineCashPaymentRequestV1 request,
        OfflineCashPaymentV1 payment)
    {
        ValidatePayment(payment, request);
        ValidateVersion(value.Version);
        if (value.InboxReceipt.Version != value.Version)
            throw new ArgumentException("Offline Cash V1 acknowledgement receipt version differs.");
        _ = Fixed(value.RequestDigest);
        _ = Fixed(value.PaymentDigest);
        _ = Fixed(value.InboxReceipt.CreditId);
        _ = Fixed(value.InboxReceipt.ReceiptCommitment);
        RequireEqual(value.RequestDigest.Span, PaymentRequestDigestUnchecked(request), "acknowledgement request digest");
        RequireEqual(value.PaymentDigest.Span, PaymentDigestUnchecked(payment), "acknowledgement payment digest");
        RequireEqual(value.InboxReceipt.CreditId.Span,
            payment.Statement.Lifecycle.CreditId.Span, "acknowledgement credit id");
    }

    private static void ValidateCreditOpening(OfflineCashCreditOpeningV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Amount == 0) throw new ArgumentException("Offline Cash V1 credit opening amount must be positive.");
        foreach (var field in new[] { value.CreditId, value.CreditCommitmentOpening,
                     value.RecipientBindingOpening, value.RecoveryNonce })
            _ = Fixed(field);
    }

    private static void ValidateCreditAad(OfflineCashEncryptedCreditAadV1 value)
    {
        ValidateVersion(value.Version);
        if (!Enum.IsDefined(value.Purpose) || value.Amount == 0)
            throw new ArgumentException("Offline Cash V1 encrypted-credit AAD is invalid.");
        _ = Fixed(value.ContextDigest);
        _ = Fixed(value.IssuanceOrTransitionCommitment);
        _ = Fixed(value.CreditId);
    }

    private static void ValidateCreditEnvelope(OfflineCashEncryptedCreditEnvelopeV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Nonce.Length != 24 || value.CiphertextAndTag.Length != EncryptedCreditCiphertextAndTagBytes)
            throw new ArgumentException("Offline Cash V1 encrypted-credit envelope length is invalid.");
    }

    private static void ValidateMintAuthorization(OfflineCashMintAuthorizationV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.Statement.Context.Version != value.Version)
            throw new ArgumentException("Offline Cash V1 mint authorization version differs.");
        ValidateMintContext(value.Statement.Context);
        _ = Fixed(value.Statement.IssuanceCommitment);
        _ = Fixed(value.Statement.CreditId);
        _ = Fixed(value.Statement.CiphertextDigest);
        ValidateProof(value.Proof);
        RequireEqual(value.Proof.SemanticDigest.Span,
            MintAuthorizationStatementDigestUnchecked(value.Statement),
            "mint authorization proof semantic digest");
    }

    private static void ValidateMintContext(OfflineCashMintAuthorizationContextV1 value)
    {
        ValidateHeader(value.Version, value.NetworkId, value.Scale, value.Amount);
        if (value.PolicyEpoch == 0) throw new ArgumentException("Offline Cash V1 mint policy epoch must be positive.");
        foreach (var field in new[] { value.OperationId, value.ReleaseId, value.SuiteId,
                     value.VkDigest, value.ArtifactManifestDigest, value.LiabilityPoolId,
                     value.HardwareCredentialId, value.HardwareProfileId,
                     value.RecipientCredentialCommitment, value.CreditCommitment })
            _ = Fixed(field);
        RequireEqual(value.LiabilityPoolId.Span,
            LiabilityPoolId(value.NetworkId, value.Asset, value.AssetIncarnation), "mint context liability pool");
    }

    private static void ValidateMintCredit(
        OfflineCashMintCreditV1 value,
        OfflineCashMintAuthorizationV1? authorization)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.Proof.Version != value.Version)
            throw new ArgumentException("Offline Cash V1 mint credit version differs.");
        ValidateLifecycle(value.Statement.Lifecycle);
        if (value.Statement.Lifecycle.OperationKind != OfflineCashOperationKindV1.MintFold
            || value.Statement.Amount == 0 || value.Statement.MintedAtMilliseconds == 0)
            throw new ArgumentException("Offline Cash V1 mint statement is invalid.");
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
                throw new ArgumentException("Offline Cash V1 mint credit authorization context differs.");
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

    private static void ValidateRedemptionVoucher(OfflineCashRedemptionVoucherV1 value)
    {
        ValidateVersion(value.Version);
        if (value.Statement.Version != value.Version || value.CommitCertificate.Version != value.Version
            || value.Proof.Version != value.Version)
            throw new ArgumentException("Offline Cash V1 redemption version differs.");
        ValidateLifecycle(value.Statement.Lifecycle);
        ValidateCommitEvidence(value.Statement.CommitEvidence);
        if (value.Statement.Lifecycle.OperationKind != OfflineCashOperationKindV1.RedeemSplit
            || value.Statement.Amount == 0
            || value.Statement.TerminalNullifier.Span.SequenceEqual(value.Statement.RedemptionCommitment.Span)
            || value.Statement.TerminalNullifier.Span.SequenceEqual(value.Statement.RedemptionId.Span)
            || value.Statement.RedemptionCommitment.Span.SequenceEqual(value.Statement.RedemptionId.Span))
            throw new ArgumentException("Offline Cash V1 redemption statement is invalid.");
        foreach (var field in new[] { value.Statement.TerminalNullifier,
                     value.Statement.RedemptionCommitment, value.Statement.RedemptionId,
                     value.ArtifactManifestDigest })
            _ = Fixed(field);
        RequireEqual(value.Statement.RedemptionId.Span,
            RedemptionIdUnchecked(value.Statement), "redemption id");
        ValidateCommitCertificate(value.CommitCertificate, value.Statement.Lifecycle,
            value.Statement.TerminalNullifier, value.Statement.CommitEvidence);
        ValidateCommitWrapper(value.Proof, RedemptionStatementDigestUnchecked(value.Statement),
            value.CommitCertificate);
    }

    private static void ValidateTopUpRequest(OfflineCashTopUpRequestV1 value)
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
            throw new ArgumentException("Canonical Offline Cash V1 top-up requests require mint authorization.");
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
            throw new ArgumentException("Offline Cash V1 top-up mint context differs.");
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

    private static void ValidateCommitEvidence(OfflineCashCommitEvidenceV1 value)
    {
        switch (value)
        {
            case OfflineCashTrustedCommitTimeV1 trusted:
                _ = Fixed(trusted.TimeEvidenceCommitment);
                break;
            case OfflineCashMonotonicCommitLeaseV1 lease:
                _ = Fixed(lease.LeaseEvidenceCommitment);
                break;
            default:
                throw new ArgumentException("Offline Cash V1 commit evidence is invalid.", nameof(value));
        }
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
        if (version != WireVersion) throw new ArgumentException("Offline Cash wire version must be 1.");
    }

    private static byte[] EncodeAggregatePayload(OfflineCashAggregateStateCommitmentV1 value) => Fields(
        U16(value.Version), Fixed(value.ReleaseId), value.NetworkId.ToBytes(), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale), Fixed(value.LiabilityPoolId),
        Fixed(value.LaneId),
        Fixed(value.HardwareEpochId), Fixed(value.KeyReference), Fixed(value.HardwarePolicyId),
        U128(value.Sequence), Fixed(value.StateCommitment));

    private static byte[] EncodeProofPayload(OfflineCashPairedProofV1 value) => Fields(
        U16(value.Version), Fixed(value.EqProtocolDigest), Fixed(value.EpProtocolDigest),
        Fixed(value.SemanticDigest), Fixed(value.GuardEqCredentialAudit), Fixed(value.GuardEpCredentialAudit),
        Fixed(value.EqDeferredAudit), Fixed(value.EpDeferredAudit), Vector(value.EqProof.Span),
        Vector(value.EpProof.Span), Vector(value.EqHistory.Span), Vector(value.EpHistory.Span));

    private static byte[] EncodeHardwareProfilePayload(OfflineCashHardwareProfileV1 value) => Fields(
        U16(value.Version), U16(value.ProtocolVersion), Fixed(value.HardwareProfileId), Fixed(value.ProviderId),
        U32((uint)value.PlatformClass), Fixed(value.ProductClassDigest), Fixed(value.FirmwarePolicyDigest),
        Fixed(value.EnrollmentAttestationVerifierDigest), Fixed(value.AttestationTrustRootsDigest),
        Fixed(value.AllowedSuiteCommitment), U64(value.PolicyEpoch), value.GovernanceCredentialPublicKey.Sec1Bytes(),
        U16(value.CapabilityMask), Fixed(value.QualificationReportDigest), U64(value.ValidFromMilliseconds),
        U64(value.ExpiresAtMilliseconds));

    private static byte[] EncodeHardwareCredentialPayload(OfflineCashHardwareCredentialV1 value) => Fields(
        U16(value.Version), Fixed(value.CredentialId), value.NetworkId.ToBytes(), Fixed(value.HardwareProfileId),
        Fixed(value.SuiteId), Fixed(value.FirmwarePolicyDigest), U64(value.PolicyEpoch), Fixed(value.LaneCommitment),
        Fixed(value.HardwareEpochId), U64(value.HardwareEpochGeneration), value.DevicePublicKey.Sec1Bytes(),
        Fixed(value.DeviceKeyReference), U64(value.IssuedAtMilliseconds), U64(value.ExpiresAtMilliseconds),
        value.GovernanceSignature.RawBytes());

    private static byte[] EncodeRequestPayload(OfflineCashPaymentRequestV1 value) => Fields(
        U16(value.Version), Fixed(value.ReleaseId), value.NetworkId.ToBytes(), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale), Fixed(value.LiabilityPoolId),
        value.Recipient.CanonicalPayload(), U128(value.Amount), EncodeHardwareCredentialPayload(value.HardwareCredential),
        Fixed(value.RequestId), U64(value.IssuedAtMilliseconds), U64(value.ExpiresAtMilliseconds), value.Signature.RawBytes());

    private static byte[] EncodeIntentPayload(OfflineCashAcceptanceIntentV1 value) => Fields(
        U16(value.Version), Fixed(value.RequestDigest), Fixed(value.IntentId), U128(value.ExactAmount),
        Fixed(value.SenderOneTimeCommitment));

    private static byte[] EncodeIntentAuthorizationStatementPayload(
        OfflineCashAcceptanceIntentAuthorizationStatementV1 value) => Fields(
            U16(value.Version), EncodeIntentPayload(value.Intent), Fixed(value.ReleaseId), Fixed(value.SuiteId),
            Fixed(value.VkDigest), Fixed(value.ArtifactManifestDigest));

    private static byte[] EncodeIntentAuthorizationPayload(OfflineCashAcceptanceIntentAuthorizationV1 value) => Fields(
        U16(value.Version), EncodeIntentAuthorizationStatementPayload(value.Statement), EncodeProofPayload(value.Proof));

    private static byte[] EncodeNoCommitClosureStatementPayload(OfflineCashNoCommitClosureStatementV1 value) => Fields(
        U16(value.Version), Fixed(value.ReleaseId), Fixed(value.SuiteId), Fixed(value.VkDigest),
        Fixed(value.ArtifactManifestDigest), Fixed(value.SenderHardwareBindingCommitment), Fixed(value.RequestId),
        Fixed(value.RequestDigest), Fixed(value.AcceptanceTicketId), Fixed(value.TicketDigest),
        Fixed(value.IntentAuthorizationDigest), Fixed(value.IntentDigest), U128(value.ExactAmount),
        Fixed(value.SenderOneTimeCommitment), Fixed(value.RecoveryId), Fixed(value.CancellationNullifier),
        Fixed(value.EquivalentDeliverySlotCommitment));

    private static byte[] EncodeNoCommitClosurePayload(OfflineCashNoCommitClosureV1 value) => Fields(
        U16(value.Version), EncodeNoCommitClosureStatementPayload(value.Statement),
        EncodeRequestPayload(value.Request), EncodeIntentAuthorizationPayload(value.IntentAuthorization),
        EncodeTicketPayload(value.AcceptanceTicket), EncodeProofPayload(value.Proof));

    private static byte[] EncodeTicketPayload(OfflineCashAcceptanceTicketV1 value) => Fields(
        U16(value.Version), value.NetworkId.ToBytes(), Fixed(value.RequestId), Fixed(value.RequestDigest),
        Fixed(value.AcceptanceTicketId), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale),
        Fixed(value.IntentDigest), U128(value.ExactAmount),
        U32(value.ReservedInboxBytes), value.RecipientOneTimeKey.Bytes(), Fixed(value.HardwareProfileId),
        U64(value.PolicyEpoch), U64(value.IssuedAtMilliseconds), U64(value.ExpiresAtMilliseconds), value.Signature.RawBytes());

    private static byte[] EncodePeerCreditContextPayload(OfflineCashPeerCreditContextV1 value) => Fields(
        U16(value.Version), Fixed(value.RequestDigest), Fixed(value.AcceptanceIntentDigest),
        Fixed(value.AcceptanceTicketDigest), Fixed(value.LifecycleContextDigest), value.RecipientOneTimeKey.Bytes());

    private static byte[] EncodeCreditOpeningPayload(OfflineCashCreditOpeningV1 value) => Fields(
        U16(value.Version), Fixed(value.CreditId), U128(value.Amount), Fixed(value.CreditCommitmentOpening),
        Fixed(value.RecipientBindingOpening), Fixed(value.RecoveryNonce));

    private static byte[] EncodeCreditAadPayload(OfflineCashEncryptedCreditAadV1 value) => Fields(
        U16(value.Version), U32((uint)value.Purpose), Fixed(value.ContextDigest),
        Fixed(value.IssuanceOrTransitionCommitment), Fixed(value.CreditId), U128(value.Amount));

    private static byte[] EncodeCreditEnvelopePayload(OfflineCashEncryptedCreditEnvelopeV1 value) => Fields(
        U16(value.Version), value.EphemeralX25519PublicKey.Bytes(), FixedWidth(value.Nonce, 24, "nonce"),
        Vector(value.CiphertextAndTag.Span));

    private static byte[] EncodeLifecyclePayload(OfflineCashLifecycleBindingV1 value) => Fields(
        U16(value.Version), value.NetworkId.ToBytes(), U16(value.ProtocolVersion), Fixed(value.SuiteId),
        Fixed(value.VkDigest), Fixed(value.ReleaseId), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation),
        U32(value.Scale), Fixed(value.LiabilityPoolId), Fixed(value.HardwareProfileId), U64(value.PolicyEpoch),
        U32((uint)value.OperationKind), Raw32(value.RequestId), Raw32(value.AcceptanceTicketId),
        Raw32(value.CreditId), Raw32(value.CiphertextDigest));

    private static byte[] EncodeCommitEvidencePayload(OfflineCashCommitEvidenceV1 value) => value switch
    {
        OfflineCashTrustedCommitTimeV1 trusted => EnumPayload(0, Fields(Fixed(trusted.TimeEvidenceCommitment))),
        OfflineCashMonotonicCommitLeaseV1 lease => EnumPayload(1, Fields(Fixed(lease.LeaseEvidenceCommitment))),
        _ => throw new ArgumentException("Unknown Offline Cash V1 commit evidence.", nameof(value)),
    };

    private static byte[] EncodeCommitCertificatePayload(OfflineCashCommitCertificateV1 value) => Fields(
        U16(value.Version), Fixed(value.CertificateId), Fixed(value.CandidateEnvelopeDigest),
        Fixed(value.LifecycleBindingDigest), Fixed(value.TransitionNullifier), Fixed(value.OutboxReservationCommitment),
        EncodeCommitEvidencePayload(value.CommitEvidence), Fixed(value.HardwareProfileId), U64(value.PolicyEpoch),
        Fixed(value.HardwareTerminalCommitment));

    private static byte[] EncodeCommitWrapperPayload(OfflineCashCommitWrapperProofV1 value) => Fields(
        U16(value.Version), Fixed(value.EqProtocolDigest), Fixed(value.EpProtocolDigest), Fixed(value.SemanticDigest),
        Fixed(value.CandidateEnvelopeDigest), Fixed(value.CommitCertificateDigest), Fixed(value.EqDeferredAudit),
        Fixed(value.EpDeferredAudit), Vector(value.EqProof.Span), Vector(value.EpProof.Span),
        Vector(value.EqHistory.Span), Vector(value.EpHistory.Span));

    private static byte[] EncodeStatementPayload(OfflineCashTransferStatementV1 value) => Fields(
        U16(value.Version), EncodeLifecyclePayload(value.Lifecycle), U128(value.Amount), Fixed(value.TransitionNullifier),
        Fixed(value.RequestDigest), Fixed(value.AcceptanceTicketDigest), value.RecipientOneTimeKey.Bytes(),
        Fixed(value.CiphertextCommitment), EncodeCommitEvidencePayload(value.CommitEvidence));

    private static byte[] EncodePaymentPayload(OfflineCashPaymentV1 value) => Fields(
        U16(value.Version), EncodeStatementPayload(value.Statement), EncodeIntentPayload(value.AcceptanceIntent),
        EncodeTicketPayload(value.AcceptanceTicket), EncodeCommitCertificatePayload(value.CommitCertificate),
        EncodeCommitWrapperPayload(value.Proof), Vector(value.EncryptedCredit.Span), Fixed(value.ArtifactManifestDigest));

    private static byte[] EncodeReceiptPayload(OfflineCashInboxReceiptV1 value) => Fields(
        U16(value.Version), Fixed(value.CreditId), Fixed(value.ReceiptCommitment));

    private static byte[] EncodeAcknowledgementPayload(OfflineCashAcknowledgementV1 value) => Fields(
        U16(value.Version), Fixed(value.RequestDigest), Fixed(value.PaymentDigest), EncodeReceiptPayload(value.InboxReceipt),
        value.Signature.RawBytes());

    private static byte[] EncodeMintContextPayload(OfflineCashMintAuthorizationContextV1 value) => Fields(
        U16(value.Version), Fixed(value.OperationId), Fixed(value.ReleaseId), Fixed(value.SuiteId), Fixed(value.VkDigest),
        Fixed(value.ArtifactManifestDigest), value.NetworkId.ToBytes(), value.Asset.CanonicalPayload(),
        EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale), Fixed(value.LiabilityPoolId),
        U128(value.Amount),
        value.Payer.CanonicalPayload(), value.Recipient.CanonicalPayload(), Fixed(value.HardwareCredentialId),
        Fixed(value.HardwareProfileId), U64(value.PolicyEpoch), Fixed(value.RecipientCredentialCommitment),
        Fixed(value.CreditCommitment), value.RecipientOneTimeKey.Bytes());

    private static byte[] EncodeMintAuthorizationStatementPayload(OfflineCashMintAuthorizationStatementV1 value) => Fields(
        U16(value.Version), EncodeMintContextPayload(value.Context), Fixed(value.IssuanceCommitment),
        Fixed(value.CreditId), Fixed(value.CiphertextDigest));

    private static byte[] EncodeMintAuthorizationPayload(OfflineCashMintAuthorizationV1 value) => Fields(
        U16(value.Version), EncodeMintAuthorizationStatementPayload(value.Statement), EncodeProofPayload(value.Proof));

    private static byte[] EncodeMintStatementPayload(OfflineCashMintCreditStatementV1 value) => Fields(
        U16(value.Version), EncodeLifecyclePayload(value.Lifecycle), Fixed(value.RecipientCredentialCommitment),
        Fixed(value.AuthorizationContextDigest), Fixed(value.MintAuthorizationDigest), U128(value.Amount),
        Fixed(value.IssuanceCommitment), value.Recipient.CanonicalPayload(), Fixed(value.CreditCommitment),
        U64(value.MintedAtMilliseconds));

    private static byte[] EncodeMintCreditPayload(OfflineCashMintCreditV1 value) => Fields(
        U16(value.Version), EncodeMintStatementPayload(value.Statement), EncodeProofPayload(value.Proof),
        Fixed(value.FinalityCertificateBinding), Fixed(value.FinalityAuthorityHead), Fixed(value.FinalityGenesisRosterId),
        Fixed(value.FinalityProofBindingDigest), Vector(value.EncryptedCredit.Span), Fixed(value.ArtifactManifestDigest));

    private static byte[] EncodeRedemptionStatementPayload(OfflineCashRedemptionStatementV1 value) => Fields(
        U16(value.Version), EncodeLifecyclePayload(value.Lifecycle), U128(value.Amount), value.Beneficiary.CanonicalPayload(),
        Fixed(value.TerminalNullifier), Fixed(value.RedemptionCommitment), Fixed(value.RedemptionId),
        EncodeCommitEvidencePayload(value.CommitEvidence));

    private static byte[] EncodeRedemptionVoucherPayload(OfflineCashRedemptionVoucherV1 value) => Fields(
        U16(value.Version), EncodeRedemptionStatementPayload(value.Statement),
        EncodeCommitCertificatePayload(value.CommitCertificate), EncodeCommitWrapperPayload(value.Proof),
        Fixed(value.ArtifactManifestDigest));

    private static byte[] EncodeTopUpRequestPayload(OfflineCashTopUpRequestV1 value) => Fields(
        U16(value.Version), Fixed(value.OperationId), Fixed(value.IssuanceCommitment), Fixed(value.CreditId),
        Fixed(value.ReleaseId), Fixed(value.SuiteId), Fixed(value.VkDigest), value.NetworkId.ToBytes(),
        value.Asset.CanonicalPayload(), EncodeAssetIncarnationPayload(value.AssetIncarnation), U32(value.Scale),
        U128(value.Amount),
        Fixed(value.LiabilityPoolId), value.Payer.CanonicalPayload(), value.Recipient.CanonicalPayload(),
        EncodeHardwareCredentialPayload(value.HardwareCredential), Fixed(value.RecipientCredentialCommitment),
        Fixed(value.CreditCommitment), value.RecipientOneTimeKey.Bytes(), Vector(value.EncryptedCredit.Span),
        Fixed(value.ArtifactManifestDigest), Option(value.MintAuthorization, EncodeMintAuthorizationPayload));

    private static OfflineCashAggregateStateCommitmentV1 DecodeAggregatePayload(byte[] payload)
    {
        var reader = Reader(payload, "aggregate state");
        var value = new OfflineCashAggregateStateCommitmentV1(
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

    private static OfflineCashPairedProofV1 DecodeProofPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "paired proof");
        var value = new OfflineCashPairedProofV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "eqProtocolDigest"),
            ReadFixed32(ref reader, "epProtocolDigest"), ReadFixed32(ref reader, "semanticDigest"),
            ReadFixed32(ref reader, "guardEqCredentialAudit"), ReadFixed32(ref reader, "guardEpCredentialAudit"),
            ReadFixed32(ref reader, "eqDeferredAudit"), ReadFixed32(ref reader, "epDeferredAudit"),
            ReadVector(ref reader, "eqProof"), ReadVector(ref reader, "epProof"),
            ReadVector(ref reader, "eqHistory"), ReadVector(ref reader, "epHistory"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashHardwareProfileV1 DecodeHardwareProfilePayload(byte[] payload)
    {
        var reader = Reader(payload, "hardware profile");
        var value = new OfflineCashHardwareProfileV1(
            ReadU16(ref reader, "version"), ReadU16(ref reader, "protocolVersion"),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadFixed32(ref reader, "providerId"),
            ReadUnitEnum<OfflineCashHardwarePlatformClassV1>(ref reader, 4, "platformClass"),
            ReadFixed32(ref reader, "productClassDigest"), ReadFixed32(ref reader, "firmwarePolicyDigest"),
            ReadFixed32(ref reader, "enrollmentAttestationVerifierDigest"),
            ReadFixed32(ref reader, "attestationTrustRootsDigest"), ReadFixed32(ref reader, "allowedSuiteCommitment"),
            ReadU64(ref reader, "policyEpoch"), ReadPublicKey(ref reader, "governanceCredentialPublicKey"),
            ReadU16(ref reader, "capabilityMask"), ReadFixed32(ref reader, "qualificationReportDigest"),
            ReadU64(ref reader, "validFrom"), ReadU64(ref reader, "expiresAt"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashHardwareCredentialV1 DecodeHardwareCredentialPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "hardware credential");
        var value = new OfflineCashHardwareCredentialV1(
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

    private static OfflineCashPaymentRequestV1 DecodeRequestPayload(byte[] payload)
    {
        var reader = Reader(payload, "payment request");
        var value = new OfflineCashPaymentRequestV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "releaseId"), ReadNetwork(ref reader, "networkId"),
            ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadFixed32(ref reader, "liabilityPoolId"), ReadAccount(ref reader, "recipient"),
            ReadU128(ref reader, "amount"),
            DecodeHardwareCredentialPayload(reader.ReadField("hardwareCredential")),
            ReadFixed32(ref reader, "requestId"), ReadU64(ref reader, "issuedAt"), ReadU64(ref reader, "expiresAt"),
            ReadSignature(ref reader, "signature"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashAcceptanceIntentV1 DecodeIntentPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "acceptance intent");
        var value = new OfflineCashAcceptanceIntentV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "requestDigest"), ReadFixed32(ref reader, "intentId"),
            ReadU128(ref reader, "exactAmount"), ReadFixed32(ref reader, "senderOneTimeCommitment"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashAcceptanceIntentAuthorizationStatementV1 DecodeIntentAuthorizationStatementPayload(
        ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "acceptance intent authorization statement");
        var value = new OfflineCashAcceptanceIntentAuthorizationStatementV1(
            ReadU16(ref reader, "version"), DecodeIntentPayload(reader.ReadField("intent")),
            ReadFixed32(ref reader, "releaseId"), ReadFixed32(ref reader, "suiteId"),
            ReadFixed32(ref reader, "vkDigest"), ReadFixed32(ref reader, "artifactManifestDigest"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashAcceptanceIntentAuthorizationV1 DecodeIntentAuthorizationPayload(byte[] payload)
    {
        var reader = Reader(payload, "acceptance intent authorization");
        var value = new OfflineCashAcceptanceIntentAuthorizationV1(ReadU16(ref reader, "version"),
            DecodeIntentAuthorizationStatementPayload(reader.ReadField("statement")),
            DecodeProofPayload(reader.ReadField("proof")));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashNoCommitClosureStatementV1 DecodeNoCommitClosureStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "no-commit closure statement");
        var value = new OfflineCashNoCommitClosureStatementV1(
            ReadU16(ref reader, "version"), ReadFixed32(ref reader, "releaseId"), ReadFixed32(ref reader, "suiteId"),
            ReadFixed32(ref reader, "vkDigest"), ReadFixed32(ref reader, "artifactManifestDigest"),
            ReadFixed32(ref reader, "senderHardwareBindingCommitment"), ReadFixed32(ref reader, "requestId"),
            ReadFixed32(ref reader, "requestDigest"), ReadFixed32(ref reader, "acceptanceTicketId"),
            ReadFixed32(ref reader, "ticketDigest"), ReadFixed32(ref reader, "intentAuthorizationDigest"),
            ReadFixed32(ref reader, "intentDigest"), ReadU128(ref reader, "exactAmount"),
            ReadFixed32(ref reader, "senderOneTimeCommitment"), ReadFixed32(ref reader, "recoveryId"),
            ReadFixed32(ref reader, "cancellationNullifier"),
            ReadFixed32(ref reader, "equivalentDeliverySlotCommitment"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashNoCommitClosureV1 DecodeNoCommitClosurePayload(byte[] payload)
    {
        var reader = Reader(payload, "no-commit closure");
        var value = new OfflineCashNoCommitClosureV1(ReadU16(ref reader, "version"),
            DecodeNoCommitClosureStatementPayload(reader.ReadField("statement")),
            DecodeRequestPayload(reader.ReadField("request").ToArray()),
            DecodeIntentAuthorizationPayload(reader.ReadField("intentAuthorization").ToArray()),
            DecodeTicketPayload(reader.ReadField("acceptanceTicket")),
            DecodeProofPayload(reader.ReadField("proof")));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashAcceptanceTicketV1 DecodeTicketPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "acceptance ticket");
        var value = new OfflineCashAcceptanceTicketV1(
            ReadU16(ref reader, "version"), ReadNetwork(ref reader, "networkId"), ReadFixed32(ref reader, "requestId"),
            ReadFixed32(ref reader, "requestDigest"), ReadFixed32(ref reader, "acceptanceTicketId"),
            ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadFixed32(ref reader, "intentDigest"),
            ReadU128(ref reader, "exactAmount"), ReadU32(ref reader, "reservedInboxBytes"),
            new OfflineCashX25519PublicKeyV1(ReadRaw32(ref reader, "recipientOneTimeKey")),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadU64(ref reader, "policyEpoch"),
            ReadU64(ref reader, "issuedAt"), ReadU64(ref reader, "expiresAt"), ReadSignature(ref reader, "signature"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashPeerCreditContextV1 DecodePeerCreditContextPayload(byte[] payload)
    {
        var reader = Reader(payload, "peer credit context");
        var value = new OfflineCashPeerCreditContextV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "requestDigest"), ReadFixed32(ref reader, "acceptanceIntentDigest"),
            ReadFixed32(ref reader, "acceptanceTicketDigest"), ReadFixed32(ref reader, "lifecycleContextDigest"),
            new OfflineCashX25519PublicKeyV1(ReadRaw32(ref reader, "recipientOneTimeKey")));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashCreditOpeningV1 DecodeCreditOpeningPayload(byte[] payload)
    {
        var reader = Reader(payload, "credit opening");
        var value = new OfflineCashCreditOpeningV1(ReadU16(ref reader, "version"), ReadFixed32(ref reader, "creditId"),
            ReadU128(ref reader, "amount"), ReadFixed32(ref reader, "creditCommitmentOpening"),
            ReadFixed32(ref reader, "recipientBindingOpening"), ReadFixed32(ref reader, "recoveryNonce"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashEncryptedCreditAadV1 DecodeCreditAadPayload(byte[] payload)
    {
        var reader = Reader(payload, "encrypted credit AAD");
        var value = new OfflineCashEncryptedCreditAadV1(ReadU16(ref reader, "version"),
            ReadUnitEnum<OfflineCashEncryptedCreditPurposeV1>(ref reader, 2, "purpose"),
            ReadFixed32(ref reader, "contextDigest"), ReadFixed32(ref reader, "issuanceOrTransitionCommitment"),
            ReadFixed32(ref reader, "creditId"), ReadU128(ref reader, "amount"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashEncryptedCreditEnvelopeV1 DecodeCreditEnvelopePayload(byte[] payload)
    {
        var reader = Reader(payload, "encrypted credit envelope");
        var value = new OfflineCashEncryptedCreditEnvelopeV1(ReadU16(ref reader, "version"),
            new OfflineCashX25519PublicKeyV1(ReadRaw32(ref reader, "ephemeralX25519PublicKey")),
            ReadFixedWidth(ref reader, 24, "nonce"), ReadVector(ref reader, "ciphertextAndTag"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashLifecycleBindingV1 DecodeLifecyclePayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "lifecycle");
        var value = new OfflineCashLifecycleBindingV1(ReadU16(ref reader, "version"), ReadNetwork(ref reader, "networkId"),
            ReadU16(ref reader, "protocolVersion"), ReadFixed32(ref reader, "suiteId"), ReadFixed32(ref reader, "vkDigest"),
            ReadFixed32(ref reader, "releaseId"), ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"),
            ReadU32(ref reader, "scale"), ReadFixed32(ref reader, "liabilityPoolId"),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadU64(ref reader, "policyEpoch"),
            ReadUnitEnum<OfflineCashOperationKindV1>(ref reader, 7, "operationKind"),
            ReadRaw32(ref reader, "requestId"), ReadRaw32(ref reader, "acceptanceTicketId"),
            ReadRaw32(ref reader, "creditId"), ReadRaw32(ref reader, "ciphertextDigest"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashCommitEvidenceV1 DecodeCommitEvidencePayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "commit evidence");
        var tag = reader.ReadUInt32LittleEndian("tag");
        var nested = Reader(reader.ReadField("evidence"), "commit evidence payload");
        OfflineCashCommitEvidenceV1 value = tag switch
        {
            0 => new OfflineCashTrustedCommitTimeV1(ReadFixed32(ref nested, "timeEvidenceCommitment")),
            1 => new OfflineCashMonotonicCommitLeaseV1(ReadFixed32(ref nested, "leaseEvidenceCommitment")),
            _ => throw new ArgumentException("Offline Cash V1 commit-evidence tag is invalid."),
        };
        nested.RequireEnd();
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashCommitCertificateV1 DecodeCommitCertificatePayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "commit certificate");
        var value = new OfflineCashCommitCertificateV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "certificateId"), ReadFixed32(ref reader, "candidateEnvelopeDigest"),
            ReadFixed32(ref reader, "lifecycleBindingDigest"), ReadFixed32(ref reader, "transitionNullifier"),
            ReadFixed32(ref reader, "outboxReservationCommitment"),
            DecodeCommitEvidencePayload(reader.ReadField("commitEvidence")), ReadFixed32(ref reader, "hardwareProfileId"),
            ReadU64(ref reader, "policyEpoch"), ReadFixed32(ref reader, "hardwareTerminalCommitment"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashCommitWrapperProofV1 DecodeCommitWrapperPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "commit wrapper");
        var value = new OfflineCashCommitWrapperProofV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "eqProtocolDigest"), ReadFixed32(ref reader, "epProtocolDigest"),
            ReadFixed32(ref reader, "semanticDigest"), ReadFixed32(ref reader, "candidateEnvelopeDigest"),
            ReadFixed32(ref reader, "commitCertificateDigest"), ReadFixed32(ref reader, "eqDeferredAudit"),
            ReadFixed32(ref reader, "epDeferredAudit"), ReadVector(ref reader, "eqProof"),
            ReadVector(ref reader, "epProof"), ReadVector(ref reader, "eqHistory"), ReadVector(ref reader, "epHistory"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashTransferStatementV1 DecodeStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "transfer statement");
        var value = new OfflineCashTransferStatementV1(ReadU16(ref reader, "version"),
            DecodeLifecyclePayload(reader.ReadField("lifecycle")), ReadU128(ref reader, "amount"),
            ReadFixed32(ref reader, "transitionNullifier"), ReadFixed32(ref reader, "requestDigest"),
            ReadFixed32(ref reader, "acceptanceTicketDigest"),
            new OfflineCashX25519PublicKeyV1(ReadRaw32(ref reader, "recipientOneTimeKey")),
            ReadFixed32(ref reader, "ciphertextCommitment"),
            DecodeCommitEvidencePayload(reader.ReadField("commitEvidence")));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashPaymentV1 DecodePaymentPayload(byte[] payload)
    {
        var reader = Reader(payload, "payment");
        var value = new OfflineCashPaymentV1(ReadU16(ref reader, "version"),
            DecodeStatementPayload(reader.ReadField("statement")), DecodeIntentPayload(reader.ReadField("acceptanceIntent")),
            DecodeTicketPayload(reader.ReadField("acceptanceTicket")),
            DecodeCommitCertificatePayload(reader.ReadField("commitCertificate")),
            DecodeCommitWrapperPayload(reader.ReadField("proof")), ReadVector(ref reader, "encryptedCredit"),
            ReadFixed32(ref reader, "artifactManifestDigest"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashInboxReceiptV1 DecodeReceiptPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "inbox receipt");
        var value = new OfflineCashInboxReceiptV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "creditId"), ReadFixed32(ref reader, "receiptCommitment"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashAcknowledgementV1 DecodeAcknowledgementPayload(byte[] payload)
    {
        var reader = Reader(payload, "acknowledgement");
        var value = new OfflineCashAcknowledgementV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "requestDigest"), ReadFixed32(ref reader, "paymentDigest"),
            DecodeReceiptPayload(reader.ReadField("inboxReceipt")), ReadSignature(ref reader, "signature"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashMintAuthorizationContextV1 DecodeMintContextPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "mint authorization context");
        var value = new OfflineCashMintAuthorizationContextV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "operationId"), ReadFixed32(ref reader, "releaseId"),
            ReadFixed32(ref reader, "suiteId"), ReadFixed32(ref reader, "vkDigest"),
            ReadFixed32(ref reader, "artifactManifestDigest"), ReadNetwork(ref reader, "networkId"),
            ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadFixed32(ref reader, "liabilityPoolId"), ReadU128(ref reader, "amount"), ReadAccount(ref reader, "payer"),
            ReadAccount(ref reader, "recipient"), ReadFixed32(ref reader, "hardwareCredentialId"),
            ReadFixed32(ref reader, "hardwareProfileId"), ReadU64(ref reader, "policyEpoch"),
            ReadFixed32(ref reader, "recipientCredentialCommitment"), ReadFixed32(ref reader, "creditCommitment"),
            new OfflineCashX25519PublicKeyV1(ReadRaw32(ref reader, "recipientOneTimeKey")));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashMintAuthorizationStatementV1 DecodeMintAuthorizationStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "mint authorization statement");
        var value = new OfflineCashMintAuthorizationStatementV1(ReadU16(ref reader, "version"),
            DecodeMintContextPayload(reader.ReadField("context")), ReadFixed32(ref reader, "issuanceCommitment"),
            ReadFixed32(ref reader, "creditId"), ReadFixed32(ref reader, "ciphertextDigest"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashMintAuthorizationV1 DecodeMintAuthorizationPayload(byte[] payload)
    {
        var reader = Reader(payload, "mint authorization");
        var value = new OfflineCashMintAuthorizationV1(ReadU16(ref reader, "version"),
            DecodeMintAuthorizationStatementPayload(reader.ReadField("statement")), DecodeProofPayload(reader.ReadField("proof")));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashMintCreditStatementV1 DecodeMintStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "mint statement");
        var value = new OfflineCashMintCreditStatementV1(ReadU16(ref reader, "version"),
            DecodeLifecyclePayload(reader.ReadField("lifecycle")), ReadFixed32(ref reader, "recipientCredentialCommitment"),
            ReadFixed32(ref reader, "authorizationContextDigest"), ReadFixed32(ref reader, "mintAuthorizationDigest"),
            ReadU128(ref reader, "amount"), ReadFixed32(ref reader, "issuanceCommitment"), ReadAccount(ref reader, "recipient"),
            ReadFixed32(ref reader, "creditCommitment"), ReadU64(ref reader, "mintedAt"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashMintCreditV1 DecodeMintCreditPayload(byte[] payload)
    {
        var reader = Reader(payload, "mint credit");
        var value = new OfflineCashMintCreditV1(ReadU16(ref reader, "version"),
            DecodeMintStatementPayload(reader.ReadField("statement")), DecodeProofPayload(reader.ReadField("proof")),
            ReadFixed32(ref reader, "finalityCertificateBinding"), ReadFixed32(ref reader, "finalityAuthorityHead"),
            ReadFixed32(ref reader, "finalityGenesisRosterId"), ReadFixed32(ref reader, "finalityProofBindingDigest"),
            ReadVector(ref reader, "encryptedCredit"), ReadFixed32(ref reader, "artifactManifestDigest"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashRedemptionStatementV1 DecodeRedemptionStatementPayload(ReadOnlySpan<byte> payload)
    {
        var reader = Reader(payload, "redemption statement");
        var value = new OfflineCashRedemptionStatementV1(ReadU16(ref reader, "version"),
            DecodeLifecyclePayload(reader.ReadField("lifecycle")), ReadU128(ref reader, "amount"),
            ReadAccount(ref reader, "beneficiary"), ReadFixed32(ref reader, "terminalNullifier"),
            ReadFixed32(ref reader, "redemptionCommitment"), ReadFixed32(ref reader, "redemptionId"),
            DecodeCommitEvidencePayload(reader.ReadField("commitEvidence")));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashRedemptionVoucherV1 DecodeRedemptionVoucherPayload(byte[] payload)
    {
        var reader = Reader(payload, "redemption voucher");
        var value = new OfflineCashRedemptionVoucherV1(ReadU16(ref reader, "version"),
            DecodeRedemptionStatementPayload(reader.ReadField("statement")),
            DecodeCommitCertificatePayload(reader.ReadField("commitCertificate")),
            DecodeCommitWrapperPayload(reader.ReadField("proof")), ReadFixed32(ref reader, "artifactManifestDigest"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashTopUpRequestV1 DecodeTopUpRequestPayload(byte[] payload)
    {
        var reader = Reader(payload, "top-up request");
        var value = new OfflineCashTopUpRequestV1(ReadU16(ref reader, "version"),
            ReadFixed32(ref reader, "operationId"), ReadFixed32(ref reader, "issuanceCommitment"),
            ReadFixed32(ref reader, "creditId"), ReadFixed32(ref reader, "releaseId"),
            ReadFixed32(ref reader, "suiteId"), ReadFixed32(ref reader, "vkDigest"), ReadNetwork(ref reader, "networkId"),
            ReadAsset(ref reader, "asset"), ReadIncarnation(ref reader, "assetIncarnation"), ReadU32(ref reader, "scale"),
            ReadU128(ref reader, "amount"), ReadFixed32(ref reader, "liabilityPoolId"), ReadAccount(ref reader, "payer"),
            ReadAccount(ref reader, "recipient"), DecodeHardwareCredentialPayload(reader.ReadField("hardwareCredential")),
            ReadFixed32(ref reader, "recipientCredentialCommitment"), ReadFixed32(ref reader, "creditCommitment"),
            new OfflineCashX25519PublicKeyV1(ReadRaw32(ref reader, "recipientOneTimeKey")),
            ReadVector(ref reader, "encryptedCredit"), ReadFixed32(ref reader, "artifactManifestDigest"),
            ReadOption(ref reader, DecodeMintAuthorizationPayload, "mintAuthorization"));
        reader.RequireEnd();
        return value;
    }

    private static OfflineCashRedemptionRequestV1 DecodeRedemptionRequestPayload(byte[] payload)
    {
        var reader = Reader(payload, "redemption request");
        var value = new OfflineCashRedemptionRequestV1(ReadU16(ref reader, "version"),
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
            throw new ArgumentException("Offline Cash V1 archive is empty or oversized.", nameof(archive));
        var (payload, flags) = NoritoCodec.Decode(schema, archive);
        if (flags != NoritoCodec.CanonicalLayoutFlags)
            throw new ArgumentException("Offline Cash V1 archive has noncanonical layout flags.", nameof(archive));
        var value = decode(payload);
        if (!archive.SequenceEqual(encode(value)))
            throw new ArgumentException("Offline Cash V1 archive is not canonical.", nameof(archive));
        return value;
    }

    private static CanonicalNoritoReader Reader(ReadOnlySpan<byte> payload, string context) =>
        new(payload, $"Offline Cash V1 {context}", nameof(payload));

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
        OfflineCashModelValidation.Fixed32(ReadRaw32(ref reader, field).AsSpan(), field);

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
    private static OfflineCashAssetDefinitionIdV1 ReadAsset(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));
    private static OfflineCashAssetIncarnationV1 ReadIncarnation(
        ref CanonicalNoritoReader reader,
        string field)
    {
        var incarnation = Reader(reader.ReadField(field), field);
        var value = new OfflineCashAssetIncarnationV1(ReadRaw32(ref incarnation, "hash"));
        incarnation.RequireEnd();
        return value;
    }
    private static OfflineCashAccountIdV1 ReadAccount(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));
    private static OfflineCashDevicePublicKeyV1 ReadPublicKey(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));
    private static OfflineCashDeviceSignatureV1 ReadSignature(ref CanonicalNoritoReader reader, string field) =>
        new(reader.ReadField(field));

    private static byte[] ReadVector(ref CanonicalNoritoReader reader, string field)
    {
        var payload = reader.ReadField(field);
        if (payload.Length < 8) throw new ArgumentException($"{field} is truncated.");
        var length = BinaryPrimitives.ReadUInt64LittleEndian(payload);
        if (length != (ulong)payload.Length - 8) throw new ArgumentException($"{field} length does not match.");
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

    private static byte[] EncodeAssetIncarnationPayload(OfflineCashAssetIncarnationV1 value) =>
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
        OfflineCashModelValidation.Fixed32(value, "fixed32");
    private static byte[] Raw32(ReadOnlyMemory<byte> value) =>
        OfflineCashModelValidation.Raw32(value.Span, "raw32");
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

    // Circuit-bound digests intentionally hash fixed semantic transcripts. These helpers must
    // remain independent of Norito transport framing, field-length prefixes, and alignment.
    private static byte[] IntentCircuitTranscript(OfflineCashAcceptanceIntentV1 value) => Join(
        U16(value.Version), Fixed(value.RequestDigest), Fixed(value.IntentId), U128(value.ExactAmount),
        Fixed(value.SenderOneTimeCommitment));

    private static byte[] IntentAuthorizationStatementCircuitTranscript(
        OfflineCashAcceptanceIntentAuthorizationStatementV1 value) => Join(
            U16(value.Version), IntentCircuitTranscript(value.Intent), Fixed(value.ReleaseId),
            Fixed(value.SuiteId), Fixed(value.VkDigest), Fixed(value.ArtifactManifestDigest));

    private static byte[] NoCommitClosureStatementCircuitTranscript(
        OfflineCashNoCommitClosureStatementV1 value) => Join(
            U16(value.Version), Fixed(value.ReleaseId), Fixed(value.SuiteId), Fixed(value.VkDigest),
            Fixed(value.ArtifactManifestDigest), Fixed(value.SenderHardwareBindingCommitment),
            Fixed(value.RequestId), Fixed(value.RequestDigest), Fixed(value.AcceptanceTicketId),
            Fixed(value.TicketDigest), Fixed(value.IntentAuthorizationDigest), Fixed(value.IntentDigest),
            U128(value.ExactAmount), Fixed(value.SenderOneTimeCommitment), Fixed(value.RecoveryId),
            Fixed(value.CancellationNullifier), Fixed(value.EquivalentDeliverySlotCommitment));

    private static byte[] OutboxReservationCircuitTranscript(OfflineCashOutboxReservationV1 value) => Join(
        Fixed(value.ReservationId), U32((uint)value.OperationKind), U32(value.ReservedOutboxBytes),
        U64(value.IssuedAtMilliseconds), U64(value.ExpiresAtMilliseconds));

    private static byte[] CommitEvidenceCircuitTranscript(OfflineCashCommitEvidenceV1 value) => value switch
    {
        OfflineCashTrustedCommitTimeV1 trusted =>
            Join(U32(0), Fixed(trusted.TimeEvidenceCommitment)),
        OfflineCashMonotonicCommitLeaseV1 lease =>
            Join(U32(1), Fixed(lease.LeaseEvidenceCommitment)),
        _ => throw new ArgumentException("Unknown Offline Cash V1 commit evidence.", nameof(value)),
    };

    private static byte[] CommitCertificateIdCircuitTranscript(OfflineCashCommitCertificateV1 value) => Join(
        U16(value.Version), Fixed(value.CandidateEnvelopeDigest), Fixed(value.LifecycleBindingDigest),
        Fixed(value.TransitionNullifier), Fixed(value.OutboxReservationCommitment),
        CommitEvidenceCircuitTranscript(value.CommitEvidence), Fixed(value.HardwareProfileId),
        U64(value.PolicyEpoch), Fixed(value.HardwareTerminalCommitment));

    private static byte[] CommitCertificateCircuitTranscript(OfflineCashCommitCertificateV1 value) => Join(
        U16(value.Version), Fixed(value.CertificateId), Fixed(value.CandidateEnvelopeDigest),
        Fixed(value.LifecycleBindingDigest), Fixed(value.TransitionNullifier),
        Fixed(value.OutboxReservationCommitment), CommitEvidenceCircuitTranscript(value.CommitEvidence),
        Fixed(value.HardwareProfileId), U64(value.PolicyEpoch), Fixed(value.HardwareTerminalCommitment));

    private static byte[] PaymentRequestDigestUnchecked(OfflineCashPaymentRequestV1 value) =>
        DigestEncoded(RequestDigestDomain, Frame(RequestSchema, EncodeRequestPayload(value), 16));
    private static byte[] IntentDigestUnchecked(OfflineCashAcceptanceIntentV1 value) =>
        DigestEncoded(IntentDigestDomain, IntentCircuitTranscript(value));
    private static byte[] IntentAuthorizationStatementDigestUnchecked(
        OfflineCashAcceptanceIntentAuthorizationStatementV1 value) =>
        DigestEncoded(
            IntentAuthorizationStatementDigestDomain,
            IntentAuthorizationStatementCircuitTranscript(value));
    private static byte[] IntentAuthorizationDigestUnchecked(
        OfflineCashAcceptanceIntentAuthorizationV1 value) =>
        DigestEncoded(
            IntentAuthorizationDigestDomain,
            Frame(IntentAuthorizationSchema, EncodeIntentAuthorizationPayload(value), 16));
    private static byte[] NoCommitClosureStatementDigestUnchecked(
        OfflineCashNoCommitClosureStatementV1 value) =>
        DigestEncoded(
            NoCommitClosureStatementDigestDomain,
            NoCommitClosureStatementCircuitTranscript(value));
    private static byte[] TicketDigestUnchecked(OfflineCashAcceptanceTicketV1 value) =>
        DigestEncoded(TicketDigestDomain, Frame(TicketSchema, EncodeTicketPayload(value), 16));
    private static byte[] PaymentDigestUnchecked(OfflineCashPaymentV1 value) =>
        DigestEncoded(PaymentDigestDomain, Frame(PaymentSchema, EncodePaymentPayload(value), 16));
    internal static byte[] LifecycleBindingDigestUnchecked(OfflineCashLifecycleBindingV1 value) =>
        DigestEncoded(LifecycleDigestDomain, Frame(LifecycleSchema, EncodeLifecyclePayload(value), 8));
    internal static byte[] TransferStatementDigestUnchecked(OfflineCashTransferStatementV1 value) =>
        DigestEncoded(StatementDigestDomain, Frame(StatementSchema, EncodeStatementPayload(value), 16));
    internal static byte[] CommitCertificateIdUnchecked(OfflineCashCommitCertificateV1 value) =>
        DigestEncoded(
            CommitCertificateIdDomain,
            CommitCertificateIdCircuitTranscript(value));
    internal static byte[] CommitCertificateDigestUnchecked(OfflineCashCommitCertificateV1 value) =>
        DigestEncoded(
            CommitCertificateDigestDomain,
            CommitCertificateCircuitTranscript(value));
    private static byte[] RedemptionIdUnchecked(OfflineCashRedemptionStatementV1 value) =>
        DigestEncoded(
            RedemptionIdDomain,
            Frame(
                RedemptionIdPreimageSchema,
                Fields(LifecycleBindingDigestUnchecked(value.Lifecycle), Fixed(value.TerminalNullifier),
                    U128(value.Amount), value.Beneficiary.CanonicalPayload(),
                    Fixed(value.RedemptionCommitment)),
                16));
    private static byte[] RedemptionStatementDigestUnchecked(OfflineCashRedemptionStatementV1 value) =>
        DigestEncoded(
            RedemptionStatementDigestDomain,
            Frame(RedemptionStatementSchema, EncodeRedemptionStatementPayload(value), 16));
    private static byte[] MintAuthorizationContextDigestUnchecked(
        OfflineCashMintAuthorizationContextV1 value) =>
        DigestEncoded(
            MintAuthorizationContextDigestDomain,
            Frame(MintAuthorizationContextSchema, EncodeMintContextPayload(value), 16));
    private static byte[] MintAuthorizationStatementDigestUnchecked(
        OfflineCashMintAuthorizationStatementV1 value) =>
        DigestEncoded(
            MintAuthorizationStatementDigestDomain,
            Frame(MintAuthorizationStatementSchema, EncodeMintAuthorizationStatementPayload(value), 16));
    private static byte[] MintAuthorizationDigestUnchecked(OfflineCashMintAuthorizationV1 value) =>
        DigestEncoded(
            MintAuthorizationDigestDomain,
            Frame(MintAuthorizationSchema, EncodeMintAuthorizationPayload(value), 16));
    private static byte[] MintStatementDigestUnchecked(OfflineCashMintCreditStatementV1 value) =>
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
        if (value.Length > maximum) throw new ArgumentException($"Offline Cash V1 {name} exceeds {maximum} bytes.", name);
        return value;
    }

    private static void RequireEqual(ReadOnlySpan<byte> actual, ReadOnlySpan<byte> expected, string name)
    {
        if (!actual.SequenceEqual(expected)) throw new ArgumentException($"Offline Cash V1 {name} does not match.");
    }

    private static (int Raw, int Text) Limits(PayloadKind kind) => kind switch
    {
        PayloadKind.PaymentRequest => (MaximumRequestBytes, 1_370),
        PayloadKind.AcceptanceIntent => (MaximumAcceptanceIntentBytes, 346),
        PayloadKind.AcceptanceIntentAuthorization => (MaximumAcceptanceIntentAuthorizationBytes, 10_586),
        PayloadKind.NoCommitClosure => (MaximumNoCommitClosureBytes, 21_850),
        PayloadKind.AcceptanceTicket => (MaximumAcceptanceTicketBytes, 1_370),
        PayloadKind.Payment => (MaximumPaymentBytes, 10_586),
        PayloadKind.Acknowledgement => (MaximumAcknowledgementBytes, 687),
        PayloadKind.MintAuthorization => (MaximumMintAuthorizationBytes, 10_586),
        PayloadKind.MintCredit => (MaximumMintCreditBytes, 10_586),
        PayloadKind.RedemptionVoucher => (MaximumRedemptionVoucherBytes, 10_586),
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };
}
