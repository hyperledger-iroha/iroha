import type { NetworkId } from "./dist/networkId.js";

/** Portable canonical codecs and orchestration bindings for Offline Cash V1. */
declare namespace OfflineCashV1 {
  type Bytes = ArrayBuffer | ArrayBufferView;
  type Unsigned = number | bigint;
  type RequestModeName = "singleExact" | "partialUntilTotal" | "boundedMultiPayment" | "openReceive";
  type OperationKind = "bootstrap" | "mintFold" | "sendSplit" | "receiveFoldBatch" | "redeemSplit" | "suiteUpgrade" | "rotate";
  type CreditPurpose = "mint" | "peer";
  type PayloadKind =
    | "paymentRequest"
    | "acceptanceIntent"
    | "acceptanceIntentAuthorization"
    | "acceptanceTicket"
    | "payment"
    | "acknowledgement"
    | "mintAuthorization"
    | "mintCredit"
    | "redemptionVoucher"
    | "encryptedCreditEnvelope"
    | "encryptedCreditAad"
    | "creditOpening";

  const wireVersion: 1;
  const deviceLifecycleVersion: 1;
  const handoffCapability: "cash_handoff_v1";
  const textPrefix: "oc1:";
  const maximumRequestRawBytes: 1024;
  const maximumRequestTextBytes: 1370;
  const maximumPreTicketRawBytes: 9984;
  const maximumPreTicketTextBytes: 13326;
  const maximumSessionRawBytes: 9211;
  const maximumSessionTextBytes: 12288;
  const completeExchangeTargetBytes: 16384;
  const maximumCompleteExchangeRawBytes: 18171;
  const maximumCompleteExchangeTextBytes: 24244;
  const maximumPairedProofBytes: 6528;
  const maximumCurrentProofsBytes: 4990;
  const maximumParityProofBytes: 2495;
  const historyAccumulatorBytes: 544;
  const maximumEncryptedCreditBytes: 384;
  const maximumCreditOpeningBytes: 256;
  const maximumNoCommitClosureBytes: 16384;
  const paymentOutboxMinimumBytes: 26112;
  const redemptionOutboxMinimumBytes: 26112;
  const maximumTopUpRequestBytes: 4096;
  const maximumRedemptionRequestBytes: 8192;
  const maximumOperationStatusBytes: 4194304;
  const maximumOperationStatusJsonBytes: 16777216;
  const payloadKinds: Readonly<Record<PayloadKind, Readonly<{ maximumRawBytes: number; maximumTextBytes: number }>>>;

  class AssetDefinitionId {
    constructor(value: string | Bytes);
    canonicalPayload(): Uint8Array;
  }

  class AssetIncarnation {
    constructor(value: Bytes);
    hashBytes(): Uint8Array;
  }

  class AccountId {
    constructor(value: string | Bytes);
    canonicalPayload(): Uint8Array;
  }

  class DevicePublicKey {
    constructor(value: Bytes);
    sec1Bytes(): Uint8Array;
  }

  class DeviceSignature {
    constructor(value: Bytes);
    rawBytes(): Uint8Array;
  }

  class AmountPolicy {
    constructor(value: { minimumAmount: Unsigned; maximumAmount: Unsigned });
    readonly minimumAmount: bigint;
    readonly maximumAmount: bigint;
  }

  class SingleExactRequest {
    constructor(value: { amount: Unsigned });
    readonly amount: bigint;
  }

  class PartialUntilTotalRequest {
    constructor(value: { totalAmount: Unsigned });
    readonly totalAmount: bigint;
  }

  class BoundedMultiPaymentRequest {
    constructor(value: { maxPayments: number; perPayment: AmountPolicy });
    readonly maxPayments: number;
    readonly perPayment: AmountPolicy;
  }

  class OpenReceiveRequest {
    constructor(value: { perPayment: AmountPolicy });
    readonly perPayment: AmountPolicy;
  }

  type RequestPolicy = SingleExactRequest | PartialUntilTotalRequest | BoundedMultiPaymentRequest | OpenReceiveRequest;
  class PaymentRequestMode {
    constructor(value: { mode: RequestModeName; policy: RequestPolicy });
    readonly mode: RequestModeName;
    readonly policy: RequestPolicy;
    static singleExact(amount: Unsigned): PaymentRequestMode;
    static partialUntilTotal(totalAmount: Unsigned): PaymentRequestMode;
    static boundedMultiPayment(maxPayments: number, perPayment: AmountPolicy): PaymentRequestMode;
    static openReceive(perPayment: AmountPolicy): PaymentRequestMode;
    acceptsExactAmount(amount: Unsigned): boolean;
  }

  class HardwareCredential {
    constructor(value: {
      version: 1; credentialId: Bytes; networkId: NetworkId; hardwareProfileId: Bytes;
      suiteId: Bytes; firmwarePolicyDigest: Bytes; policyEpoch: Unsigned; laneCommitment: Bytes;
      hardwareEpochId: Bytes; hardwareEpochGeneration: Unsigned; devicePublicKey: DevicePublicKey;
      deviceKeyReference: Bytes; issuedAtMs: Unsigned; expiresAtMs: Unsigned;
      governanceSignature: DeviceSignature;
    });
    readonly version: 1; readonly credentialId: Uint8Array; readonly networkId: NetworkId;
    readonly hardwareProfileId: Uint8Array; readonly suiteId: Uint8Array;
    readonly firmwarePolicyDigest: Uint8Array; readonly policyEpoch: bigint;
    readonly laneCommitment: Uint8Array; readonly hardwareEpochId: Uint8Array;
    readonly hardwareEpochGeneration: bigint; readonly devicePublicKey: DevicePublicKey;
    readonly deviceKeyReference: Uint8Array; readonly issuedAtMs: bigint; readonly expiresAtMs: bigint;
    readonly governanceSignature: DeviceSignature;
  }

  class PastaStateCommitment {
    constructor(value: { eq: Bytes; ep: Bytes });
    readonly eq: Uint8Array;
    readonly ep: Uint8Array;
  }

  class PairedProof {
    constructor(value: {
      version: 1; eqProtocolDigest: Bytes; epProtocolDigest: Bytes; semanticDigest: Bytes;
      guardEqCredentialAudit: Bytes; guardEpCredentialAudit: Bytes; eqDeferredAudit: Bytes;
      epDeferredAudit: Bytes; eqProof: Bytes; epProof: Bytes; eqHistory: Bytes; epHistory: Bytes;
    });
    readonly version: 1; readonly eqProtocolDigest: Uint8Array; readonly epProtocolDigest: Uint8Array;
    readonly semanticDigest: Uint8Array; readonly guardEqCredentialAudit: Uint8Array;
    readonly guardEpCredentialAudit: Uint8Array; readonly eqDeferredAudit: Uint8Array;
    readonly epDeferredAudit: Uint8Array; readonly eqProof: Uint8Array; readonly epProof: Uint8Array;
    readonly eqHistory: Uint8Array; readonly epHistory: Uint8Array;
  }

  class PaymentRequest {
    constructor(value: {
      version: 1; releaseId: Bytes; networkId: NetworkId; asset: AssetDefinitionId;
      assetIncarnation: AssetIncarnation; scale: number; liabilityPoolId: Bytes; recipient: AccountId;
      requestMode: PaymentRequestMode; hardwareCredential: HardwareCredential; requestId: Bytes;
      issuedAtMs: Unsigned; expiresAtMs: Unsigned; signature: DeviceSignature;
    });
    readonly version: 1; readonly releaseId: Uint8Array; readonly networkId: NetworkId;
    readonly asset: AssetDefinitionId; readonly assetIncarnation: AssetIncarnation; readonly scale: number;
    readonly liabilityPoolId: Uint8Array; readonly recipient: AccountId; readonly requestMode: PaymentRequestMode;
    readonly hardwareCredential: HardwareCredential; readonly requestId: Uint8Array;
    readonly issuedAtMs: bigint; readonly expiresAtMs: bigint; readonly signature: DeviceSignature;
  }

  class AcceptanceIntent {
    constructor(value: { version: 1; requestDigest: Bytes; intentId: Bytes; exactAmount: Unsigned; senderOneTimeCommitment: Bytes });
    readonly version: 1; readonly requestDigest: Uint8Array; readonly intentId: Uint8Array;
    readonly exactAmount: bigint; readonly senderOneTimeCommitment: Uint8Array;
  }

  class AcceptanceIntentAuthorizationStatement {
    constructor(value: { version: 1; intent: AcceptanceIntent; releaseId: Bytes; suiteId: Bytes; vkDigest: Bytes; artifactManifestDigest: Bytes });
    readonly version: 1; readonly intent: AcceptanceIntent; readonly releaseId: Uint8Array;
    readonly suiteId: Uint8Array; readonly vkDigest: Uint8Array; readonly artifactManifestDigest: Uint8Array;
  }

  class AcceptanceIntentAuthorization {
    constructor(value: { version: 1; statement: AcceptanceIntentAuthorizationStatement; proof: PairedProof });
    readonly version: 1; readonly statement: AcceptanceIntentAuthorizationStatement; readonly proof: PairedProof;
  }

  class NoCommitClosureStatement {
    constructor(value: {
      version: 1; releaseId: Bytes; suiteId: Bytes; vkDigest: Bytes;
      artifactManifestDigest: Bytes; senderHardwareBindingCommitment: Bytes;
      requestId: Bytes; requestDigest: Bytes; acceptanceTicketId: Bytes;
      ticketDigest: Bytes; intentAuthorizationDigest: Bytes; intentDigest: Bytes;
      exactAmount: Unsigned; senderOneTimeCommitment: Bytes; recoveryId: Bytes;
      cancellationNullifier: Bytes; equivalentDeliverySlotCommitment: Bytes;
    });
    readonly version: 1; readonly releaseId: Uint8Array; readonly suiteId: Uint8Array;
    readonly vkDigest: Uint8Array; readonly artifactManifestDigest: Uint8Array;
    readonly senderHardwareBindingCommitment: Uint8Array; readonly requestId: Uint8Array;
    readonly requestDigest: Uint8Array; readonly acceptanceTicketId: Uint8Array;
    readonly ticketDigest: Uint8Array; readonly intentAuthorizationDigest: Uint8Array;
    readonly intentDigest: Uint8Array; readonly exactAmount: bigint;
    readonly senderOneTimeCommitment: Uint8Array; readonly recoveryId: Uint8Array;
    readonly cancellationNullifier: Uint8Array;
    readonly equivalentDeliverySlotCommitment: Uint8Array;
  }

  class AcceptanceTicket {
    constructor(value: {
      version: 1; networkId: NetworkId; requestId: Bytes; requestDigest: Bytes; acceptanceTicketId: Bytes;
      asset: AssetDefinitionId; assetIncarnation: AssetIncarnation; scale: number; requestMode: PaymentRequestMode;
      intentDigest: Bytes; exactAmount: Unsigned; reservedInboxBytes: number; recipientOneTimeKey: Bytes;
      hardwareProfileId: Bytes; policyEpoch: Unsigned; issuedAtMs: Unsigned; expiresAtMs: Unsigned;
      signature: DeviceSignature;
    });
    readonly version: 1; readonly networkId: NetworkId; readonly requestId: Uint8Array;
    readonly requestDigest: Uint8Array; readonly acceptanceTicketId: Uint8Array;
    readonly asset: AssetDefinitionId; readonly assetIncarnation: AssetIncarnation; readonly scale: number;
    readonly requestMode: PaymentRequestMode; readonly intentDigest: Uint8Array; readonly exactAmount: bigint;
    readonly reservedInboxBytes: number; readonly recipientOneTimeKey: Uint8Array;
    readonly hardwareProfileId: Uint8Array; readonly policyEpoch: bigint; readonly issuedAtMs: bigint;
    readonly expiresAtMs: bigint; readonly signature: DeviceSignature;
  }

  class NoCommitClosure {
    constructor(value: {
      version: 1; statement: NoCommitClosureStatement; request: PaymentRequest;
      intentAuthorization: AcceptanceIntentAuthorization;
      acceptanceTicket: AcceptanceTicket; proof: PairedProof;
    });
    readonly version: 1; readonly statement: NoCommitClosureStatement;
    readonly request: PaymentRequest;
    readonly intentAuthorization: AcceptanceIntentAuthorization;
    readonly acceptanceTicket: AcceptanceTicket; readonly proof: PairedProof;
  }

  class CreditOpening {
    constructor(value: { version: 1; creditId: Bytes; amount: Unsigned; creditCommitmentOpening: Bytes; recipientBindingOpening: Bytes; recoveryNonce: Bytes });
    readonly version: 1; readonly creditId: Uint8Array; readonly amount: bigint;
    readonly creditCommitmentOpening: Uint8Array; readonly recipientBindingOpening: Uint8Array;
    readonly recoveryNonce: Uint8Array;
  }

  class EncryptedCreditAad {
    constructor(value: { version: 1; purpose: CreditPurpose; contextDigest: Bytes; issuanceOrTransitionCommitment: Bytes; creditId: Bytes; amount: Unsigned });
    readonly version: 1; readonly purpose: CreditPurpose; readonly contextDigest: Uint8Array;
    readonly issuanceOrTransitionCommitment: Uint8Array; readonly creditId: Uint8Array; readonly amount: bigint;
  }

  class EncryptedCreditEnvelope {
    constructor(value: { version: 1; ephemeralX25519PublicKey: Bytes; nonce: Bytes; ciphertextAndTag: Bytes });
    readonly version: 1; readonly ephemeralX25519PublicKey: Uint8Array;
    readonly nonce: Uint8Array; readonly ciphertextAndTag: Uint8Array;
  }

  class TrustedCommitTime {
    constructor(value: { timeEvidenceCommitment: Bytes });
    readonly timeEvidenceCommitment: Uint8Array;
  }
  class MonotonicCommitLease {
    constructor(value: { leaseEvidenceCommitment: Bytes });
    readonly leaseEvidenceCommitment: Uint8Array;
  }
  class CommitEvidence {
    constructor(value: { source: "trustedTime"; evidence: TrustedCommitTime } | { source: "monotonicLease"; evidence: MonotonicCommitLease });
    readonly source: "trustedTime" | "monotonicLease";
    readonly evidence: TrustedCommitTime | MonotonicCommitLease;
    static trustedTime(commitment: Bytes): CommitEvidence;
    static monotonicLease(commitment: Bytes): CommitEvidence;
  }

  class OutboxReservation {
    constructor(value: {
      reservationId: Bytes; operationKind: "sendSplit" | "redeemSplit";
      reservedOutboxBytes: number; issuedAtMs: Unsigned; expiresAtMs: Unsigned;
    });
    readonly reservationId: Uint8Array;
    readonly operationKind: "sendSplit" | "redeemSplit";
    readonly reservedOutboxBytes: number;
    readonly issuedAtMs: bigint;
    readonly expiresAtMs: bigint;
  }

  class LifecycleBinding {
    constructor(value: {
      version: 1; networkId: NetworkId; protocolVersion: 1; suiteId: Bytes; vkDigest: Bytes;
      releaseId: Bytes; asset: AssetDefinitionId; assetIncarnation: AssetIncarnation; scale: number;
      liabilityPoolId: Bytes; hardwareProfileId: Bytes; policyEpoch: Unsigned; operationKind: OperationKind;
      requestId: Bytes; acceptanceTicketId: Bytes; creditId: Bytes; ciphertextDigest: Bytes;
    });
    readonly version: 1; readonly networkId: NetworkId; readonly protocolVersion: 1;
    readonly suiteId: Uint8Array; readonly vkDigest: Uint8Array; readonly releaseId: Uint8Array;
    readonly asset: AssetDefinitionId; readonly assetIncarnation: AssetIncarnation; readonly scale: number;
    readonly liabilityPoolId: Uint8Array; readonly hardwareProfileId: Uint8Array; readonly policyEpoch: bigint;
    readonly operationKind: OperationKind; readonly requestId: Uint8Array; readonly acceptanceTicketId: Uint8Array;
    readonly creditId: Uint8Array; readonly ciphertextDigest: Uint8Array;
  }

  class CommitCertificate {
    constructor(value: {
      version: 1; certificateId: Bytes; candidateEnvelopeDigest: Bytes; lifecycleBindingDigest: Bytes;
      transitionNullifier: Bytes; outboxReservationCommitment: Bytes; commitEvidence: CommitEvidence;
      hardwareProfileId: Bytes; policyEpoch: Unsigned; hardwareTerminalCommitment: Bytes;
    });
    readonly version: 1; readonly certificateId: Uint8Array; readonly candidateEnvelopeDigest: Uint8Array;
    readonly lifecycleBindingDigest: Uint8Array; readonly transitionNullifier: Uint8Array;
    readonly outboxReservationCommitment: Uint8Array; readonly commitEvidence: CommitEvidence;
    readonly hardwareProfileId: Uint8Array; readonly policyEpoch: bigint; readonly hardwareTerminalCommitment: Uint8Array;
  }

  class CommitWrapperProof {
    constructor(value: {
      version: 1; eqProtocolDigest: Bytes; epProtocolDigest: Bytes; semanticDigest: Bytes;
      candidateEnvelopeDigest: Bytes; commitCertificateDigest: Bytes; eqDeferredAudit: Bytes;
      epDeferredAudit: Bytes; eqProof: Bytes; epProof: Bytes; eqHistory: Bytes; epHistory: Bytes;
    });
    readonly version: 1; readonly eqProtocolDigest: Uint8Array; readonly epProtocolDigest: Uint8Array;
    readonly semanticDigest: Uint8Array; readonly candidateEnvelopeDigest: Uint8Array;
    readonly commitCertificateDigest: Uint8Array; readonly eqDeferredAudit: Uint8Array;
    readonly epDeferredAudit: Uint8Array; readonly eqProof: Uint8Array; readonly epProof: Uint8Array;
    readonly eqHistory: Uint8Array; readonly epHistory: Uint8Array;
  }

  class TransferStatement {
    constructor(value: {
      version: 1; lifecycle: LifecycleBinding; amount: Unsigned; transitionNullifier: Bytes;
      requestDigest: Bytes; acceptanceTicketDigest: Bytes; recipientOneTimeKey: Bytes;
      ciphertextCommitment: Bytes; commitEvidence: CommitEvidence;
    });
    readonly version: 1; readonly lifecycle: LifecycleBinding; readonly amount: bigint;
    readonly transitionNullifier: Uint8Array; readonly requestDigest: Uint8Array;
    readonly acceptanceTicketDigest: Uint8Array; readonly recipientOneTimeKey: Uint8Array;
    readonly ciphertextCommitment: Uint8Array; readonly commitEvidence: CommitEvidence;
  }

  class Payment {
    constructor(value: {
      version: 1; statement: TransferStatement; acceptanceIntent: AcceptanceIntent;
      acceptanceTicket: AcceptanceTicket; commitCertificate: CommitCertificate;
      proof: CommitWrapperProof; encryptedCredit: Bytes; artifactManifestDigest: Bytes;
    });
    readonly version: 1; readonly statement: TransferStatement; readonly acceptanceIntent: AcceptanceIntent;
    readonly acceptanceTicket: AcceptanceTicket; readonly commitCertificate: CommitCertificate;
    readonly proof: CommitWrapperProof; readonly encryptedCredit: Uint8Array; readonly artifactManifestDigest: Uint8Array;
  }

  class InboxReceipt {
    constructor(value: { version: 1; creditId: Bytes; receiptCommitment: Bytes });
    readonly version: 1; readonly creditId: Uint8Array; readonly receiptCommitment: Uint8Array;
  }
  class Acknowledgement {
    constructor(value: { version: 1; requestDigest: Bytes; paymentDigest: Bytes; inboxReceipt: InboxReceipt; signature: DeviceSignature });
    readonly version: 1; readonly requestDigest: Uint8Array; readonly paymentDigest: Uint8Array;
    readonly inboxReceipt: InboxReceipt; readonly signature: DeviceSignature;
  }

  class MintAuthorizationContext {
    constructor(value: {
      version: 1; operationId: Bytes; releaseId: Bytes; suiteId: Bytes; vkDigest: Bytes;
      artifactManifestDigest: Bytes; networkId: NetworkId; asset: AssetDefinitionId;
      assetIncarnation: AssetIncarnation; scale: number; liabilityPoolId: Bytes; amount: Unsigned;
      payer: AccountId; recipient: AccountId; hardwareCredentialId: Bytes; hardwareProfileId: Bytes;
      policyEpoch: Unsigned; recipientCredentialCommitment: Bytes; creditCommitment: Bytes;
      recipientOneTimeKey: Bytes;
    });
    readonly version: 1; readonly operationId: Uint8Array; readonly releaseId: Uint8Array;
    readonly suiteId: Uint8Array; readonly vkDigest: Uint8Array; readonly artifactManifestDigest: Uint8Array;
    readonly networkId: NetworkId; readonly asset: AssetDefinitionId; readonly assetIncarnation: AssetIncarnation;
    readonly scale: number; readonly liabilityPoolId: Uint8Array; readonly amount: bigint;
    readonly payer: AccountId; readonly recipient: AccountId; readonly hardwareCredentialId: Uint8Array;
    readonly hardwareProfileId: Uint8Array; readonly policyEpoch: bigint;
    readonly recipientCredentialCommitment: Uint8Array; readonly creditCommitment: Uint8Array;
    readonly recipientOneTimeKey: Uint8Array;
  }
  class MintAuthorizationStatement {
    constructor(value: { version: 1; context: MintAuthorizationContext; issuanceCommitment: Bytes; creditId: Bytes; ciphertextDigest: Bytes });
    readonly version: 1; readonly context: MintAuthorizationContext; readonly issuanceCommitment: Uint8Array;
    readonly creditId: Uint8Array; readonly ciphertextDigest: Uint8Array;
  }
  class MintAuthorization {
    constructor(value: { version: 1; statement: MintAuthorizationStatement; proof: PairedProof });
    readonly version: 1; readonly statement: MintAuthorizationStatement; readonly proof: PairedProof;
  }
  class MintCreditStatement {
    constructor(value: {
      version: 1; lifecycle: LifecycleBinding; recipientCredentialCommitment: Bytes;
      authorizationContextDigest: Bytes; mintAuthorizationDigest: Bytes; amount: Unsigned;
      issuanceCommitment: Bytes; recipient: AccountId; creditCommitment: Bytes; mintedAtMs: Unsigned;
    });
    readonly version: 1; readonly lifecycle: LifecycleBinding; readonly recipientCredentialCommitment: Uint8Array;
    readonly authorizationContextDigest: Uint8Array; readonly mintAuthorizationDigest: Uint8Array;
    readonly amount: bigint; readonly issuanceCommitment: Uint8Array; readonly recipient: AccountId;
    readonly creditCommitment: Uint8Array; readonly mintedAtMs: bigint;
  }
  class MintCredit {
    constructor(value: {
      version: 1; statement: MintCreditStatement; proof: PairedProof; finalityCertificateBinding: Bytes;
      finalityAuthorityHead: Bytes; finalityGenesisRosterId: Bytes; finalityProofBindingDigest: Bytes;
      encryptedCredit: Bytes; artifactManifestDigest: Bytes;
    });
    readonly version: 1; readonly statement: MintCreditStatement; readonly proof: PairedProof;
    readonly finalityCertificateBinding: Uint8Array; readonly finalityAuthorityHead: Uint8Array;
    readonly finalityGenesisRosterId: Uint8Array; readonly finalityProofBindingDigest: Uint8Array;
    readonly encryptedCredit: Uint8Array; readonly artifactManifestDigest: Uint8Array;
  }

  class RedemptionStatement {
    constructor(value: {
      version: 1; lifecycle: LifecycleBinding; amount: Unsigned; beneficiary: AccountId;
      terminalNullifier: Bytes; redemptionCommitment: Bytes; redemptionId: Bytes; commitEvidence: CommitEvidence;
    });
    readonly version: 1; readonly lifecycle: LifecycleBinding; readonly amount: bigint;
    readonly beneficiary: AccountId; readonly terminalNullifier: Uint8Array;
    readonly redemptionCommitment: Uint8Array; readonly redemptionId: Uint8Array;
    readonly commitEvidence: CommitEvidence;
  }
  class RedemptionVoucher {
    constructor(value: { version: 1; statement: RedemptionStatement; commitCertificate: CommitCertificate; proof: CommitWrapperProof; artifactManifestDigest: Bytes });
    readonly version: 1; readonly statement: RedemptionStatement; readonly commitCertificate: CommitCertificate;
    readonly proof: CommitWrapperProof; readonly artifactManifestDigest: Uint8Array;
  }

  class TopUpRequest {
    constructor(value: {
      version: 1; operationId: Bytes; issuanceCommitment: Bytes; creditId: Bytes; releaseId: Bytes;
      suiteId: Bytes; vkDigest: Bytes; networkId: NetworkId; asset: AssetDefinitionId;
      assetIncarnation: AssetIncarnation; scale: number; amount: Unsigned; liabilityPoolId: Bytes;
      payer: AccountId; recipient: AccountId; hardwareCredential: HardwareCredential;
      recipientCredentialCommitment: Bytes; creditCommitment: Bytes; recipientOneTimeKey: Bytes;
      encryptedCredit: Bytes; artifactManifestDigest: Bytes; mintAuthorization: MintAuthorization | null;
    });
    readonly version: 1; readonly operationId: Uint8Array; readonly issuanceCommitment: Uint8Array;
    readonly creditId: Uint8Array; readonly releaseId: Uint8Array; readonly suiteId: Uint8Array;
    readonly vkDigest: Uint8Array; readonly networkId: NetworkId; readonly asset: AssetDefinitionId;
    readonly assetIncarnation: AssetIncarnation; readonly scale: number; readonly amount: bigint;
    readonly liabilityPoolId: Uint8Array; readonly payer: AccountId; readonly recipient: AccountId;
    readonly hardwareCredential: HardwareCredential; readonly recipientCredentialCommitment: Uint8Array;
    readonly creditCommitment: Uint8Array; readonly recipientOneTimeKey: Uint8Array;
    readonly encryptedCredit: Uint8Array; readonly artifactManifestDigest: Uint8Array;
    readonly mintAuthorization: MintAuthorization | null;
  }
  class RedemptionRequest {
    constructor(value: { version: 1; operationId: Bytes; voucher: RedemptionVoucher });
    readonly version: 1; readonly operationId: Uint8Array; readonly voucher: RedemptionVoucher;
  }

  function encodePaymentRequest(value: PaymentRequest): Uint8Array;
  function decodePaymentRequest(raw: Bytes): PaymentRequest;
  function encodeAcceptanceIntent(value: AcceptanceIntent): Uint8Array;
  function decodeAcceptanceIntent(raw: Bytes): AcceptanceIntent;
  function encodeAcceptanceIntentAuthorization(value: AcceptanceIntentAuthorization, request: PaymentRequest): Uint8Array;
  function decodeAcceptanceIntentAuthorization(raw: Bytes, request: PaymentRequest): AcceptanceIntentAuthorization;
  function encodeAcceptanceTicket(value: AcceptanceTicket, request: PaymentRequest, intent: AcceptanceIntent): Uint8Array;
  function decodeAcceptanceTicket(raw: Bytes, request: PaymentRequest, intent: AcceptanceIntent): AcceptanceTicket;
  function encodeNoCommitClosure(value: NoCommitClosure): Uint8Array;
  function decodeNoCommitClosure(raw: Bytes): NoCommitClosure;
  function encodePayment(value: Payment, request: PaymentRequest): Uint8Array;
  function decodePayment(raw: Bytes, request: PaymentRequest): Payment;
  function encodeAcknowledgement(value: Acknowledgement, request: PaymentRequest, payment: Payment): Uint8Array;
  function decodeAcknowledgement(raw: Bytes, request: PaymentRequest, payment: Payment): Acknowledgement;
  function encodeMintAuthorization(value: MintAuthorization): Uint8Array;
  function decodeMintAuthorization(raw: Bytes): MintAuthorization;
  function encodeMintCredit(value: MintCredit, authorization?: MintAuthorization): Uint8Array;
  function decodeMintCredit(raw: Bytes, authorization?: MintAuthorization): MintCredit;
  function encodeRedemptionVoucher(value: RedemptionVoucher): Uint8Array;
  function decodeRedemptionVoucher(raw: Bytes): RedemptionVoucher;
  function encodeCreditOpening(value: CreditOpening): Uint8Array;
  function decodeCreditOpening(raw: Bytes, creditId?: Bytes, amount?: Unsigned): CreditOpening;
  function encodeEncryptedCreditAad(value: EncryptedCreditAad): Uint8Array;
  function decodeEncryptedCreditAad(raw: Bytes): EncryptedCreditAad;
  function encodeEncryptedCreditEnvelope(value: EncryptedCreditEnvelope, recipientKey?: Bytes): Uint8Array;
  function decodeEncryptedCreditEnvelope(raw: Bytes, recipientKey?: Bytes): EncryptedCreditEnvelope;
  function encodeTopUpRequest(value: TopUpRequest): Uint8Array;
  function decodeTopUpRequest(raw: Bytes): TopUpRequest;
  function encodeRedemptionRequest(value: RedemptionRequest): Uint8Array;
  function decodeRedemptionRequest(raw: Bytes): RedemptionRequest;
  function encodeText(kind: PayloadKind, raw: Bytes): `oc1:${string}`;
  function decodeText(kind: PayloadKind, text: string): Uint8Array;
  function encodeTypedText(kind: PayloadKind, value: object, ...bindings: object[]): `oc1:${string}`;
  function decodeTypedText(kind: PayloadKind, text: string, ...bindings: object[]): object;
  function validatePreTicketExchange(request: PaymentRequest, authorization: AcceptanceIntentAuthorization, ticket: AcceptanceTicket): number;
  function validateSession(request: PaymentRequest, payment: Payment, acknowledgement: Acknowledgement): number;
  function validateCompleteExchange(request: PaymentRequest, authorization: AcceptanceIntentAuthorization, ticket: AcceptanceTicket, payment: Payment, acknowledgement: Acknowledgement): number;
  function validateMintCreditAgainstAuthorization(credit: MintCredit, authorization: MintAuthorization): true;
  function encryptedCreditAadForMint(statement: MintAuthorizationStatement): EncryptedCreditAad;
  function deviceKeyReference(publicKey: DevicePublicKey): Uint8Array;
  function pastaStateCommitment(value: PastaStateCommitment): Uint8Array;
  function liabilityPoolId(networkId: NetworkId, asset: AssetDefinitionId, assetIncarnation: AssetIncarnation): Uint8Array;
  function paymentRequestSigningBytes(value: PaymentRequest): Uint8Array;
  function paymentRequestDigest(value: PaymentRequest): Uint8Array;
  function acceptanceIntentDigest(value: AcceptanceIntent): Uint8Array;
  function acceptanceAuthorizationStatementDigest(value: AcceptanceIntentAuthorizationStatement): Uint8Array;
  function acceptanceAuthorizationDigest(value: AcceptanceIntentAuthorization): Uint8Array;
  function acceptanceTicketDigest(value: AcceptanceTicket): Uint8Array;
  function noCommitClosureStatementDigest(value: NoCommitClosureStatement): Uint8Array;
  function noCommitClosureDigest(value: NoCommitClosure): Uint8Array;
  function outboxReservationCommitment(value: OutboxReservation): Uint8Array;
  function paymentDigest(value: Payment, request: PaymentRequest): Uint8Array;
  function ciphertextDigest(value: Bytes): Uint8Array;
  function creditId(transitionNullifier: Bytes, requestDigest: Bytes, ticketDigest: Bytes, recipientOneTimeKey: Bytes, amount: Unsigned, ciphertextCommitment: Bytes): Uint8Array;
  function mintAuthorizationContextDigest(value: MintAuthorizationContext): Uint8Array;
  function mintAuthorizationStatementDigest(value: MintAuthorizationStatement): Uint8Array;
  function mintAuthorizationDigest(value: MintAuthorization): Uint8Array;
}

export { OfflineCashV1 };
