import type { NetworkId } from "./dist/networkId.js";

/** Portable canonical codecs and orchestration bindings for KAGEMUSHA V1. */
declare namespace Kagemusha {
  type Bytes = ArrayBuffer | ArrayBufferView;
  type Unsigned = number | bigint;
  type OperationKind = "bootstrap" | "mintFold" | "sendSplit" | "receiveFold" | "redeemSplit" | "rotate";
  type Ipm1PayloadKind = "request" | "payment" | "acknowledgement";
  type CreditPurpose = "mint" | "peer";
  type PayloadKind =
    | "paymentRequest"
    | "payment"
    | "acknowledgement"
    | "mintAuthorization"
    | "mintCredit"
    | "redemptionVoucher";

  const wireVersion: 1;
  const deviceLifecycleVersion: 1;
  const handoffCapability: "kagemusha_handoff_v1";
  const textPrefix: "kgm1:";
  const maximumRequestRawBytes: 928;
  const maximumRequestTextBytes: 1243;
  /** Bounds all three separately transported protocol messages together. */
  const targetCompleteExchangeRawBytes: 8960;
  const maximumCompleteExchangeRawBytes: 9211;
  const maximumCompleteExchangeTextBytes: 12288;
  const maximumPairedProofBytes: 6528;
  const maximumRedemptionProofBytes: 6528;
  const maximumPaymentProofBytes: 6528;
  const maximumCommitCertificateBytes: 1024;
  const maximumCurrentProofsBytes: 4990;
  const maximumParityProofBytes: 2495;
  const historyAccumulatorBytes: 544;
  const maximumEncryptedCreditBytes: 384;
  const maximumCreditOpeningBytes: 256;
  const paymentOutboxMinimumBytes: 25728;
  const redemptionOutboxMinimumBytes: 26112;
  const maximumTopUpRequestBytes: 16384;
  const maximumDeviceMintStageCommandBytes: 65536;
  const maximumDeviceMintStageResultBytes: 128;
  const deviceMintStageDispositions: Readonly<{ staged: 0; exactDuplicate: 1 }>;
  const topUpInstructionWireId: "iroha.kagemusha.v1.top_up";
  const maximumRedemptionRequestBytes: 8192;
  const maximumOperationStatusBytes: 4194304;
  const maximumOperationStatusJsonBytes: 16777216;
  const payloadKinds: Readonly<Record<PayloadKind, Readonly<{ maximumRawBytes: number; maximumTextBytes: number }>>>;
  const ipm1PayloadKinds: Readonly<Record<Ipm1PayloadKind, Readonly<{ tag: 1 | 2 | 3; payloadKind: PayloadKind }>>>;
  const operationKinds: Readonly<Record<OperationKind, 0 | 1 | 2 | 3 | 4 | 5>>;

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
      amount: Unsigned; recipientEncryptionKey: Bytes;
      hardwareCredential: HardwareCredential; requestId: Bytes;
      issuedAtMs: Unsigned; expiresAtMs: Unsigned; signature: DeviceSignature;
    });
    readonly version: 1; readonly releaseId: Uint8Array; readonly networkId: NetworkId;
    readonly asset: AssetDefinitionId; readonly assetIncarnation: AssetIncarnation; readonly scale: number;
    readonly liabilityPoolId: Uint8Array; readonly recipient: AccountId;
    readonly amount: bigint; readonly recipientEncryptionKey: Uint8Array;
    readonly hardwareCredential: HardwareCredential;
    readonly requestId: Uint8Array;
    readonly issuedAtMs: bigint; readonly expiresAtMs: bigint; readonly signature: DeviceSignature;
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

  class LifecycleBinding {
    constructor(value: {
      version: 1; networkId: NetworkId; protocolVersion: 1; suiteId: Bytes; vkDigest: Bytes;
      releaseId: Bytes; asset: AssetDefinitionId; assetIncarnation: AssetIncarnation; scale: number;
      liabilityPoolId: Bytes; hardwareProfileId: Bytes; policyEpoch: Unsigned; operationKind: OperationKind;
      requestId: Bytes; receiverLaneCommitment: Bytes; creditId: Bytes; ciphertextDigest: Bytes;
    });
    readonly version: 1; readonly networkId: NetworkId; readonly protocolVersion: 1;
    readonly suiteId: Uint8Array; readonly vkDigest: Uint8Array; readonly releaseId: Uint8Array;
    readonly asset: AssetDefinitionId; readonly assetIncarnation: AssetIncarnation; readonly scale: number;
    readonly liabilityPoolId: Uint8Array; readonly hardwareProfileId: Uint8Array; readonly policyEpoch: bigint;
    readonly operationKind: OperationKind; readonly requestId: Uint8Array;
    readonly receiverLaneCommitment: Uint8Array; readonly creditId: Uint8Array; readonly ciphertextDigest: Uint8Array;
  }

  class TrustedCommitTime {
    constructor(value: { timeEvidenceCommitment: Bytes });
    readonly timeEvidenceCommitment: Uint8Array;
  }
  class MonotonicLease {
    constructor(value: { leaseEvidenceCommitment: Bytes });
    readonly leaseEvidenceCommitment: Uint8Array;
  }
  type CommitEvidence = TrustedCommitTime | MonotonicLease;
  class OutboxReservation {
    constructor(value: { reservationId: Bytes; operationKind: "sendSplit" | "redeemSplit"; reservedOutboxBytes: number; issuedAtMs: Unsigned; expiresAtMs: Unsigned });
    readonly reservationId: Uint8Array; readonly operationKind: "sendSplit" | "redeemSplit";
    readonly reservedOutboxBytes: number; readonly issuedAtMs: bigint; readonly expiresAtMs: bigint;
  }
  class HardwareTerminalBody {
    constructor(value: {
      version: 1; candidateEnvelopeDigest: Bytes; lifecycleBindingDigest: Bytes; transitionNullifier: Bytes;
      outboxReservationCommitment: Bytes; commitEvidence: CommitEvidence; hardwareProfileId: Bytes;
      policyEpoch: Unsigned; privateSuccessorCommitment: Bytes; privateJournalCommitment: Bytes; privateRecoveryCommitment: Bytes;
    });
    readonly version: 1; readonly candidateEnvelopeDigest: Uint8Array; readonly lifecycleBindingDigest: Uint8Array;
    readonly transitionNullifier: Uint8Array; readonly outboxReservationCommitment: Uint8Array;
    readonly commitEvidence: CommitEvidence; readonly hardwareProfileId: Uint8Array; readonly policyEpoch: bigint;
    readonly privateSuccessorCommitment: Uint8Array; readonly privateJournalCommitment: Uint8Array;
    readonly privateRecoveryCommitment: Uint8Array;
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
  class RedemptionProof {
    constructor(value: {
      version: 1; eqProtocolDigest: Bytes; epProtocolDigest: Bytes; semanticDigest: Bytes;
      candidateEnvelopeDigest: Bytes; commitCertificateDigest: Bytes; eqDeferredAudit: Bytes; epDeferredAudit: Bytes;
      eqProof: Bytes; epProof: Bytes; eqHistory: Bytes; epHistory: Bytes;
    });
    readonly version: 1; readonly eqProtocolDigest: Uint8Array; readonly epProtocolDigest: Uint8Array;
    readonly semanticDigest: Uint8Array; readonly candidateEnvelopeDigest: Uint8Array;
    readonly commitCertificateDigest: Uint8Array; readonly eqDeferredAudit: Uint8Array;
    readonly epDeferredAudit: Uint8Array; readonly eqProof: Uint8Array; readonly epProof: Uint8Array;
    readonly eqHistory: Uint8Array; readonly epHistory: Uint8Array;
  }
  class PaymentProof {
    constructor(value: {
      version: 1; eqProtocolDigest: Bytes; epProtocolDigest: Bytes; semanticDigest: Bytes;
      candidateEnvelopeDigest: Bytes; commitCertificateDigest: Bytes; eqDeferredAudit: Bytes; epDeferredAudit: Bytes;
      eqProof: Bytes; epProof: Bytes; eqHistory: Bytes; epHistory: Bytes;
    });
    readonly version: 1; readonly eqProtocolDigest: Uint8Array; readonly epProtocolDigest: Uint8Array;
    readonly semanticDigest: Uint8Array; readonly candidateEnvelopeDigest: Uint8Array;
    readonly commitCertificateDigest: Uint8Array; readonly eqDeferredAudit: Uint8Array;
    readonly epDeferredAudit: Uint8Array; readonly eqProof: Uint8Array; readonly epProof: Uint8Array;
    readonly eqHistory: Uint8Array; readonly epHistory: Uint8Array;
  }

  class PeerCreditContext {
    constructor(value: { version: 1; requestDigest: Bytes; amount: Unsigned;
      senderBeforeCommitment: Bytes; senderAfterCommitment: Bytes;
      preparedTransferDigest: Bytes; recipientEncryptionKey: Bytes });
    readonly version: 1; readonly requestDigest: Uint8Array; readonly amount: bigint;
    readonly senderBeforeCommitment: Uint8Array; readonly senderAfterCommitment: Uint8Array;
    readonly preparedTransferDigest: Uint8Array; readonly recipientEncryptionKey: Uint8Array;
  }

  class PaymentOutput {
    constructor(value: {
      version: 1; requestDigest: Bytes; amount: Unsigned; senderBeforeCommitment: Bytes;
      senderAfterCommitment: Bytes; transitionNullifier: Bytes; creditId: Bytes;
      ciphertextCommitment: Bytes; commitEvidence: CommitEvidence; committedAtMs: Unsigned;
    });
    readonly version: 1; readonly requestDigest: Uint8Array; readonly amount: bigint;
    readonly senderBeforeCommitment: Uint8Array; readonly senderAfterCommitment: Uint8Array;
    readonly transitionNullifier: Uint8Array; readonly creditId: Uint8Array;
    readonly ciphertextCommitment: Uint8Array; readonly commitEvidence: CommitEvidence;
    readonly committedAtMs: bigint;
  }

  class Payment {
    constructor(value: {
      version: 1; output: PaymentOutput; encryptedCredit: Bytes; commitCertificate: CommitCertificate; proof: PaymentProof;
    });
    readonly version: 1; readonly output: PaymentOutput;
    readonly encryptedCredit: Uint8Array; readonly commitCertificate: CommitCertificate; readonly proof: PaymentProof;
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

  /** Public operation-16 body; private openings and Guard certificates stay in native storage. */
  class DeviceMintStageCommand {
    constructor(value: { version: 1; canonicalAuthorization: Bytes; canonicalMintCredit: Bytes });
    readonly version: 1; readonly canonicalAuthorization: Uint8Array; readonly canonicalMintCredit: Uint8Array;
  }
  /** An unauthenticated shape until the qualified native response authenticator is verified. */
  class DeviceMintStageResult {
    constructor(value: { version: 1; disposition: 0 | 1; creditId: Bytes });
    readonly version: 1; readonly disposition: 0 | 1; readonly creditId: Uint8Array;
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
    constructor(value: { version: 1; statement: RedemptionStatement; commitCertificate: CommitCertificate; proof: RedemptionProof; artifactManifestDigest: Bytes });
    readonly version: 1; readonly statement: RedemptionStatement; readonly commitCertificate: CommitCertificate;
    readonly proof: RedemptionProof; readonly artifactManifestDigest: Uint8Array;
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

  interface TopUpInstruction {
    readonly TopUpKagemushaV1: Readonly<{ request: Uint8Array }>;
  }

  function encodePaymentRequest(value: PaymentRequest): Uint8Array;
  function decodePaymentRequest(raw: Bytes): PaymentRequest;
  function encodeCommitCertificate(value: CommitCertificate, lifecycle?: LifecycleBinding, evidence?: CommitEvidence, nullifier?: Bytes): Uint8Array;
  function decodeCommitCertificate(raw: Bytes, lifecycle?: LifecycleBinding, evidence?: CommitEvidence, nullifier?: Bytes): CommitCertificate;
  function encodeRedemptionProof(value: RedemptionProof): Uint8Array;
  function decodeRedemptionProof(raw: Bytes): RedemptionProof;
  function encodePeerCreditContext(value: PeerCreditContext): Uint8Array;
  function decodePeerCreditContext(raw: Bytes): PeerCreditContext;
  function encodePaymentProof(value: PaymentProof): Uint8Array;
  function decodePaymentProof(raw: Bytes): PaymentProof;
  function encodePayment(value: Payment, request: PaymentRequest): Uint8Array;
  function decodePayment(raw: Bytes, request: PaymentRequest): Payment;
  function encodeAcknowledgement(value: Acknowledgement, request: PaymentRequest, payment: Payment): Uint8Array;
  function decodeAcknowledgement(raw: Bytes, request: PaymentRequest, payment: Payment): Acknowledgement;
  function encodeMintAuthorization(value: MintAuthorization): Uint8Array;
  function decodeMintAuthorization(raw: Bytes): MintAuthorization;
  function encodeMintCredit(value: MintCredit, authorization?: MintAuthorization): Uint8Array;
  function decodeMintCredit(raw: Bytes, authorization?: MintAuthorization): MintCredit;
  /** Validates both exact nested archives and their public binding, not their monetary proofs. */
  function encodeDeviceMintStageCommandShape(value: DeviceMintStageCommand): Uint8Array;
  function encodeDeviceMintStageCommandShape(canonicalAuthorization: Bytes, canonicalMintCredit: Bytes): Uint8Array;
  function decodeDeviceMintStageCommandShapeExact(raw: Bytes): DeviceMintStageCommand;
  function encodeDeviceMintStageResultShape(value: DeviceMintStageResult, command?: DeviceMintStageCommand): Uint8Array;
  function decodeDeviceMintStageResultShapeExact(raw: Bytes, command?: DeviceMintStageCommand): DeviceMintStageResult;
  /** Structural binding only; this never authenticates an inbox receipt. */
  function validateDeviceMintStageResultAgainstCommand(result: DeviceMintStageResult, command: DeviceMintStageCommand): true;
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
  function buildTopUpInstruction(value: TopUpRequest): TopUpInstruction;
  function encodeTopUpInstruction(value: TopUpRequest): Uint8Array;
  function decodeTopUpInstruction(raw: Bytes): TopUpRequest;
  function encodeRedemptionRequest(value: RedemptionRequest): Uint8Array;
  function decodeRedemptionRequest(raw: Bytes): RedemptionRequest;
  function encodeText(kind: PayloadKind, raw: Bytes): `kgm1:${string}`;
  function decodeText(kind: PayloadKind, text: string): Uint8Array;
  function encodeTypedText(kind: PayloadKind, value: object, ...bindings: object[]): `kgm1:${string}`;
  function decodeTypedText(kind: PayloadKind, text: string, ...bindings: object[]): object;
  /** Validates the exact three-message exchange and its combined raw and `kgm1:` limits. */
  function validateCompleteExchange(request: PaymentRequest, payment: Payment, acknowledgement: Acknowledgement): number;
  function validateMintCreditAgainstAuthorization(credit: MintCredit, authorization: MintAuthorization): true;
  function encryptedCreditAadForMint(statement: MintAuthorizationStatement): EncryptedCreditAad;
  function peerCreditContext(output: PaymentOutput, request: PaymentRequest): PeerCreditContext;
  function encryptedCreditAadForPeer(output: PaymentOutput, request: PaymentRequest): EncryptedCreditAad;
  function deviceKeyReference(publicKey: DevicePublicKey): Uint8Array;
  function pastaStateCommitment(value: PastaStateCommitment): Uint8Array;
  function liabilityPoolId(networkId: NetworkId, asset: AssetDefinitionId, assetIncarnation: AssetIncarnation): Uint8Array;
  function paymentRequestSigningBytes(value: PaymentRequest): Uint8Array;
  function acknowledgementSigningBytes(value: Acknowledgement): Uint8Array;
  function paymentRequestDigest(value: PaymentRequest): Uint8Array;
  function lifecycleBindingDigest(value: LifecycleBinding): Uint8Array;
  function preparedTransferDigest(request: PaymentRequest, senderBeforeCommitment: Bytes,
    senderAfterCommitment: Bytes, transitionNullifier: Bytes, ciphertextCommitment: Bytes): Uint8Array;
  function paymentOutputDigest(value: PaymentOutput, request?: PaymentRequest): Uint8Array;
  function redemptionId(value: RedemptionStatement): Uint8Array;
  function redemptionStatementDigest(value: RedemptionStatement): Uint8Array;
  function paymentDigest(value: Payment, request: PaymentRequest): Uint8Array;
  function ciphertextDigest(value: Bytes): Uint8Array;
  function paymentBodyDigest(output: PaymentOutput, encryptedCredit: Bytes): Uint8Array;
  function assetIdentityDigest(asset: AssetDefinitionId): Uint8Array;
  function accountIdentityDigest(account: AccountId): Uint8Array;
  function paymentRequestTranscript(request: PaymentRequest): Uint8Array;
  function paymentOutputTranscript(output: PaymentOutput): Uint8Array;
  function creditId(transitionNullifier: Bytes, requestDigest: Bytes): Uint8Array;
  function peerCreditOpeningCommitment(requestDigest: Bytes, recipientEncryptionKey: Bytes,
    amount: Unsigned, creditCommitmentOpening: Bytes, recipientBindingOpening: Bytes,
    recoveryNonce: Bytes): Uint8Array;
  function expectedCommitCertificateId(value: CommitCertificate): Uint8Array;
  function commitCertificateDigest(value: CommitCertificate, lifecycle?: LifecycleBinding, evidence?: CommitEvidence, nullifier?: Bytes): Uint8Array;
  function ipm1PayloadTag(kind: Ipm1PayloadKind): 1 | 2 | 3;
  function ipm1PayloadKindFromTag(tag: number): Ipm1PayloadKind;
  function mintAuthorizationContextDigest(value: MintAuthorizationContext): Uint8Array;
  function mintAuthorizationStatementDigest(value: MintAuthorizationStatement): Uint8Array;
  function mintAuthorizationDigest(value: MintAuthorization): Uint8Array;
  /** Derives the unique mint identity without using the current ID or authorization digest. */
  function mintCreditId(value: MintCreditStatement): Uint8Array;
  /** Validates the derived mint identity before returning the statement's semantic digest. */
  function mintCreditStatementDigest(value: MintCreditStatement): Uint8Array;
}

export { Kagemusha };
