package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.function.LongSupplier;
import org.bouncycastle.asn1.ASN1Integer;
import org.bouncycastle.asn1.ASN1Sequence;
import org.bouncycastle.asn1.x9.ECNamedCurveTable;
import org.bouncycastle.asn1.x9.X9ECParameters;
import org.bouncycastle.crypto.params.ECDomainParameters;
import org.bouncycastle.crypto.params.ECPublicKeyParameters;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.ECDSASigner;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Hardware-backed Offline Bearer purse facade for real offline value transfer. */
public final class OfflineBearerWallet {
  public static final String SIGNATURE_ALGORITHM_ED25519 = "ed25519";
  public static final String SIGNATURE_ALGORITHM_ECDSA_P256_SHA256 = "ecdsa_p256_sha256";
  public static final String PUBLIC_KEY_ENCODING_RAW_ED25519 = "raw_ed25519";
  public static final String PUBLIC_KEY_ENCODING_X963_P256 = "x963_uncompressed_p256";

  private final String chainId;
  private final String accountId;
  private final SecureElement secureElement;
  private final PolicyProvider policyProvider;
  private final OfflineNoteIdGenerator idGenerator;
  private final LongSupplier clock;
  private final SignatureVerifying signatureVerifier;

  public OfflineBearerWallet(
      final String chainId,
      final String accountId,
      final SecureElement secureElement,
      final PolicyProvider policyProvider) {
    this(
        chainId,
        accountId,
        secureElement,
        policyProvider,
        new UuidOfflineNoteIdGenerator(),
        System::currentTimeMillis,
        new RejectingSignatureVerifier());
  }

  public OfflineBearerWallet(
      final String chainId,
      final String accountId,
      final SecureElement secureElement,
      final PolicyProvider policyProvider,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    this(
        chainId,
        accountId,
        secureElement,
        policyProvider,
        idGenerator,
        clock,
        new RejectingSignatureVerifier());
  }

  public OfflineBearerWallet(
      final String chainId,
      final String accountId,
      final SecureElement secureElement,
      final PolicyProvider policyProvider,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock,
      final SignatureVerifying signatureVerifier) {
    requireNonBlank(chainId, "chainId");
    requireNonBlank(accountId, "accountId");
    this.chainId = chainId;
    this.accountId = accountId;
    this.secureElement = Objects.requireNonNull(secureElement, "secureElement");
    this.policyProvider = Objects.requireNonNull(policyProvider, "policyProvider");
    this.idGenerator = Objects.requireNonNull(idGenerator, "idGenerator");
    this.clock = Objects.requireNonNull(clock, "clock");
    this.signatureVerifier = Objects.requireNonNull(signatureVerifier, "signatureVerifier");
  }

  public PurseStateV2 currentState() {
    return secureElement.currentState();
  }

  public void installLoadedPurse(final CertificateV2 certificate, final PurseStateV2 state) {
    final PolicyBundleV2 policy = currentVerifiedPolicy();
    final long now = clock.getAsLong();
    requireHardwareUsable(policy);
    if (!certificate.chainId().equals(chainId)
        || !certificate.accountId().equals(accountId)
        || !state.chainId().equals(chainId)
        || !state.accountId().equals(accountId)
        || !state.purseId().equals(certificate.purseId())
        || !state.assetDefinitionId().equals(certificate.assetDefinitionId())
        || !equalsIgnoreCase(state.policyHashHex(), policy.policyHashHex())
        || !equalsIgnoreCase(certificate.policyHashHex(), policy.policyHashHex())) {
      throw new PolicyException("Offline Bearer purse install does not match wallet or policy");
    }
    enforceCertificatePolicy(certificate, policy, now);
    requireAmountAtMost(
        state.balance(), policy.maxOfflineBalance(), "offline purse balance exceeds policy limit");
    secureElement.installPurse(certificate, state);
  }

  public ReceiveRequestV2 prepareReceive(final String assetDefinitionId, final String amount) {
    return prepareReceive(assetDefinitionId, amount, defaultTokenTtl());
  }

  public ReceiveRequestV2 prepareReceive(
      final String assetDefinitionId, final String amount, final long ttlMs) {
    final PolicyBundleV2 policy = currentVerifiedPolicy();
    final long now = clock.getAsLong();
    requireHardwareUsable(policy);
    requirePolicyFresh(policy, now);
    final CertificateV2 certificate = requireCurrentCertificate();
    final PurseStateV2 state = requireCurrentState();
    enforceCertificatePolicy(certificate, policy, now);
    if (!assetDefinitionId.equals(state.assetDefinitionId())) {
      throw new PolicyException("assetDefinitionId does not match purse asset");
    }
    final String canonicalAmount = canonicalAmountString(amount);
    requirePositiveAmount(canonicalAmount, "amount");
    requireAmountAtMost(
        canonicalAmount, policy.maxTransactionAmount(), "amount exceeds offline transaction policy");
    requireAmountAtMost(
        canonicalAmount,
        maxTransactionAmountForAsset(policy, assetDefinitionId),
        "amount exceeds offline asset transaction policy");
    final ReceiveRequestV2 request =
        secureElement.createReceiveRequest(
        idGenerator.nextId("offline-bearer-request"),
        canonicalAmount,
        now,
        safeAdd(now, Math.min(ttlMs, policy.maxTokenAgeMs())),
        policy.policyHashHex());
    signatureVerifier.verifyReceiveRequest(request);
    return request;
  }

  public DebitReceiptV2 pay(final ReceiveRequestV2 request) {
    return pay(request, defaultTokenTtl());
  }

  public DebitReceiptV2 pay(final ReceiveRequestV2 request, final long ttlMs) {
    final PolicyBundleV2 policy = currentVerifiedPolicy();
    final long now = clock.getAsLong();
    requireHardwareUsable(policy);
    requirePolicyFresh(policy, now);
    validateReceiveRequest(request, policy, now);
    final CertificateV2 senderCertificate = requireCurrentCertificate();
    enforceCertificatePolicy(senderCertificate, policy, now);
    if (!senderCertificate.assetDefinitionId().equals(request.assetDefinitionId())) {
      throw new PolicyException("sender purse asset does not match receive request");
    }
    requireAmountAtMost(
        request.amount(), policy.maxTransactionAmount(), "amount exceeds offline transaction policy");
    requireAmountAtMost(
        request.amount(),
        maxTransactionAmountForAsset(policy, request.assetDefinitionId()),
        "amount exceeds offline asset transaction policy");
    final DebitReceiptV2 receipt =
        secureElement.debit(
        request,
        idGenerator.nextId("offline-bearer-transfer"),
        now,
        safeAdd(now, Math.min(ttlMs, policy.maxTokenAgeMs())));
    signatureVerifier.verifyDebitReceipt(receipt);
    return receipt;
  }

  public CreditReceiptV2 accept(final DebitReceiptV2 receipt) {
    final PolicyBundleV2 policy = currentVerifiedPolicy();
    final long now = clock.getAsLong();
    requireHardwareUsable(policy);
    requirePolicyFresh(policy, now);
    validateDebitReceipt(receipt, policy, now);
    final CertificateV2 certificate = requireCurrentCertificate();
    final PurseStateV2 state = requireCurrentState();
    if (!receipt.recipientCertificate().purseId().equals(certificate.purseId())) {
      throw new PolicyException("debit receipt is not addressed to this purse");
    }
    if (!state.purseId().equals(certificate.purseId()) || !state.accountId().equals(accountId)) {
      throw new PolicyException("current purse state does not match wallet certificate");
    }
    if (!state.assetDefinitionId().equals(receipt.assetDefinitionId())) {
      throw new PolicyException("current purse asset does not match debit receipt");
    }
    if (!equalsIgnoreCase(state.policyHashHex(), policy.policyHashHex())) {
      throw new PolicyException("current purse policy hash does not match policy");
    }
    enforceCertificatePolicy(certificate, policy, now);
    requireAmountAtMost(
        decimal(state.balance()).add(decimal(receipt.amount())).toPlainString(),
        policy.maxOfflineBalance(),
        "offline purse balance exceeds policy limit");
    final CreditReceiptV2 creditReceipt = secureElement.credit(receipt, now);
    signatureVerifier.verifyCreditReceipt(creditReceipt);
    return creditReceipt;
  }

  public SettlementBatchV2 exportSettlementBatch() {
    return exportSettlementBatch(256);
  }

  public SettlementBatchV2 exportSettlementBatch(final int maxReceipts) {
    requireHardwareUsable(currentVerifiedPolicy());
    if (maxReceipts <= 0) {
      throw new IllegalArgumentException("maxReceipts must be positive");
    }
    final SettlementBatchV2 batch = secureElement.exportSettlementBatch(maxReceipts);
    for (final DebitReceiptV2 receipt : batch.debitReceipts()) {
      signatureVerifier.verifyDebitReceipt(receipt);
    }
    for (final CreditReceiptV2 receipt : batch.creditReceipts()) {
      signatureVerifier.verifyCreditReceipt(receipt);
    }
    return batch;
  }

  public void pruneSettled(final Collection<String> transferIds) {
    requireHardwareUsable(currentVerifiedPolicy());
    secureElement.pruneSettled(transferIds);
  }

  private void validateReceiveRequest(
      final ReceiveRequestV2 request, final PolicyBundleV2 policy, final long now) {
    if (!request.chainId().equals(chainId)) {
      throw new IllegalArgumentException("receive request chainId does not match wallet chainId");
    }
    if (request.expiresAtMs() <= now) {
      throw new IllegalArgumentException("receive request is expired");
    }
    if (request.createdAtMs() > now) {
      throw new IllegalArgumentException("receive request is from the future");
    }
    if (now - request.createdAtMs() > policy.maxTokenAgeMs()) {
      throw new IllegalArgumentException("receive request is too old");
    }
    if (!equalsIgnoreCase(request.policyHashHex(), policy.policyHashHex())) {
      throw new IllegalArgumentException("receive request policy hash does not match current policy");
    }
    if (policy.revokedTransferIds().contains(request.paymentRequestId())) {
      throw new PolicyException("Offline Bearer receive request is revoked");
    }
    enforceCertificatePolicy(request.recipientCertificate(), policy, now);
    signatureVerifier.verifyReceiveRequest(request);
  }

  private void validateDebitReceipt(
      final DebitReceiptV2 receipt, final PolicyBundleV2 policy, final long now) {
    if (!receipt.chainId().equals(chainId)) {
      throw new IllegalArgumentException("debit receipt chainId does not match wallet chainId");
    }
    if (receipt.expiresAtMs() <= now) {
      throw new IllegalArgumentException("debit receipt is expired");
    }
    if (receipt.createdAtMs() > now) {
      throw new IllegalArgumentException("debit receipt is from the future");
    }
    if (now - receipt.createdAtMs() > policy.maxTokenAgeMs()) {
      throw new IllegalArgumentException("debit receipt is too old");
    }
    if (!equalsIgnoreCase(receipt.policyHashHex(), policy.policyHashHex())) {
      throw new IllegalArgumentException("debit receipt policy hash does not match current policy");
    }
    if (!receipt.recipientCertificate().accountId().equals(accountId)) {
      throw new IllegalArgumentException("debit receipt recipient account does not match wallet account");
    }
    if (policy.revokedTransferIds().contains(receipt.transferId())) {
      throw new PolicyException("Offline Bearer transfer is revoked");
    }
    if (!receipt.senderCertificate().assetDefinitionId().equals(receipt.assetDefinitionId())) {
      throw new IllegalArgumentException("sender certificate asset does not match debit receipt");
    }
    if (!receipt.recipientCertificate().assetDefinitionId().equals(receipt.assetDefinitionId())) {
      throw new IllegalArgumentException("recipient certificate asset does not match debit receipt");
    }
    enforceCertificatePolicy(receipt.senderCertificate(), policy, now);
    enforceCertificatePolicy(receipt.recipientCertificate(), policy, now);
    requireAmountAtMost(
        receipt.amount(), policy.maxTransactionAmount(), "amount exceeds offline transaction policy");
    signatureVerifier.verifyDebitReceipt(receipt);
  }

  private void requireHardwareUsable(final PolicyBundleV2 policy) {
    final SecureElementCapabilities capabilities = secureElement.capabilities();
    if (!capabilities.hardwareBacked() || !capabilities.statefulPurse()) {
      throw new PolicyException("Offline Bearer requires a hardware-backed stateful purse");
    }
    if (capabilities.attestationKeyId() == null
        || capabilities.attestationKeyId().trim().isEmpty()) {
      throw new PolicyException("Offline Bearer requires a non-extractable hardware attestation key");
    }
    if (!capabilities.rollbackResistantState()) {
      throw new PolicyException("Offline Bearer requires rollback-resistant purse state");
    }
    if (capabilities.attestationEvidence().length == 0) {
      throw new PolicyException("Offline Bearer requires secure-element attestation evidence");
    }
    requireSupportedSignatureAlgorithm(capabilities.signatureAlgorithm());
    requireSupportedPublicKeyEncoding(capabilities.publicKeyEncoding());
    if (!policy.allowedHardwareClasses().contains(capabilities.hardwareClass())) {
      throw new PolicyException("hardware class is not allowed by current Offline Bearer policy");
    }
  }

  private void enforceCertificatePolicy(
      final CertificateV2 certificate, final PolicyBundleV2 policy, final long now) {
    requirePolicyFresh(policy, now);
    if (certificate.issuedAtMs() > now) {
      throw new PolicyException("Offline Bearer certificate is from the future");
    }
    if (certificate.expiresAtMs() <= now) {
      throw new PolicyException("Offline Bearer certificate is expired");
    }
    if (now - certificate.issuedAtMs() > policy.maxCertificateAgeMs()) {
      throw new PolicyException("Offline Bearer certificate is too old");
    }
    if (!certificate.issuerId().equals(policy.issuerId())) {
      throw new PolicyException("Offline Bearer certificate issuer does not match policy");
    }
    if (!equalsIgnoreCase(certificate.policyHashHex(), policy.policyHashHex())) {
      throw new PolicyException("Offline Bearer certificate policy hash does not match policy");
    }
    requireSupportedSignatureAlgorithm(certificate.signatureAlgorithm());
    requireSupportedPublicKeyEncoding(certificate.publicKeyEncoding());
    if (!policy.allowedHardwareClasses().contains(certificate.hardwareClass())) {
      throw new PolicyException("certificate hardware class is not allowed by policy");
    }
    if (policy.blacklistedAccountIds().contains(certificate.accountId())
        || policy.blacklistedDeviceIds().contains(certificate.deviceId())
        || policy.blacklistedKeyIds().contains(certificate.keyId())
        || policy.revokedCertificateIds().contains(certificate.certificateId())
        || policy.revokedCertificateIds().contains(certificate.keyId())) {
      throw new PolicyException("Offline Bearer certificate is blacklisted");
    }
    signatureVerifier.verifyCertificate(certificate, policy);
  }

  private static String maxTransactionAmountForAsset(
      final PolicyBundleV2 policy, final String assetDefinitionId) {
    for (final AssetSendLimitV2 limit : policy.assetSendLimits()) {
      if (limit.assetDefinitionId().equals(assetDefinitionId)) {
        return limit.maxTransactionAmount();
      }
    }
    return policy.maxTransactionAmount();
  }

  private static void requirePolicyFresh(final PolicyBundleV2 policy, final long now) {
    if (policy.issuedAtMs() > now) {
      throw new PolicyException("Offline Bearer policy is from the future");
    }
    if (policy.expiresAtMs() <= now) {
      throw new PolicyException("Offline Bearer policy is expired");
    }
    if (now - policy.issuedAtMs() > policy.maxPolicyAgeMs()) {
      throw new PolicyException("Offline Bearer policy is too old");
    }
  }

  private CertificateV2 requireCurrentCertificate() {
    final CertificateV2 certificate = secureElement.currentCertificate();
    if (certificate == null) {
      throw new PolicyException("Offline Bearer purse certificate is not installed");
    }
    return certificate;
  }

  private PurseStateV2 requireCurrentState() {
    final PurseStateV2 state = secureElement.currentState();
    if (state == null) {
      throw new PolicyException("Offline Bearer purse state is not installed");
    }
    return state;
  }

  private long defaultTokenTtl() {
    return currentVerifiedPolicy().maxTokenAgeMs();
  }

  private PolicyBundleV2 currentVerifiedPolicy() {
    final PolicyBundleV2 policy = policyProvider.currentPolicy();
    signatureVerifier.verifyPolicy(policy);
    return policy;
  }

  public interface PolicyProvider {
    PolicyBundleV2 currentPolicy();
  }

  public interface SecureElement {
    SecureElementCapabilities capabilities();

    CertificateV2 currentCertificate();

    PurseStateV2 currentState();

    void installPurse(CertificateV2 certificate, PurseStateV2 state);

    ReceiveRequestV2 createReceiveRequest(
        String paymentRequestId,
        String amount,
        long createdAtMs,
        long expiresAtMs,
        String policyHashHex);

    DebitReceiptV2 debit(
        ReceiveRequestV2 request, String transferId, long createdAtMs, long expiresAtMs);

    CreditReceiptV2 credit(DebitReceiptV2 receipt, long acceptedAtMs);

    SettlementBatchV2 exportSettlementBatch(int maxReceipts);

    void pruneSettled(Collection<String> transferIds);
  }

  public static final class StaticPolicyProvider implements PolicyProvider {
    private final PolicyBundleV2 policy;

    public StaticPolicyProvider(final PolicyBundleV2 policy) {
      this.policy = Objects.requireNonNull(policy, "policy");
    }

    @Override
    public PolicyBundleV2 currentPolicy() {
      return policy;
    }
  }

  public static final class UnsupportedSecureElement implements SecureElement {
    private final String hardwareClass;

    public UnsupportedSecureElement() {
      this("unsupported");
    }

    public UnsupportedSecureElement(final String hardwareClass) {
      requireNonBlank(hardwareClass, "hardwareClass");
      this.hardwareClass = hardwareClass;
    }

    @Override
    public SecureElementCapabilities capabilities() {
      return new SecureElementCapabilities(false, false, hardwareClass, null);
    }

    @Override
    public CertificateV2 currentCertificate() {
      return null;
    }

    @Override
    public PurseStateV2 currentState() {
      return null;
    }

    @Override
    public void installPurse(final CertificateV2 certificate, final PurseStateV2 state) {
      throw unsupported();
    }

    @Override
    public ReceiveRequestV2 createReceiveRequest(
        final String paymentRequestId,
        final String amount,
        final long createdAtMs,
        final long expiresAtMs,
        final String policyHashHex) {
      throw unsupported();
    }

    @Override
    public DebitReceiptV2 debit(
        final ReceiveRequestV2 request,
        final String transferId,
        final long createdAtMs,
        final long expiresAtMs) {
      throw unsupported();
    }

    @Override
    public CreditReceiptV2 credit(final DebitReceiptV2 receipt, final long acceptedAtMs) {
      throw unsupported();
    }

    @Override
    public SettlementBatchV2 exportSettlementBatch(final int maxReceipts) {
      throw unsupported();
    }

    @Override
    public void pruneSettled(final Collection<String> transferIds) {
      throw unsupported();
    }

    private PolicyException unsupported() {
      return new PolicyException("Offline Bearer value is disabled on unsupported hardware");
    }
  }

  public static final class PolicyException extends RuntimeException {
    public PolicyException(final String message) {
      super(message);
    }
  }

  /** Canonical domain-separated unsigned payloads for Offline Bearer v2 signatures. */
  public static final class Payloads {
    private static final String POLICY_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerPolicyBundlePayloadV2";
    private static final String POLICY_TYPE =
        "iroha_data_model::offline::model::OfflineBearerPolicyBundleV2";
    private static final String CERTIFICATE_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCertificatePayloadV2";
    private static final String CERTIFICATE_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCertificateV2";
    private static final String RECEIVE_REQUEST_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerReceiveRequestPayloadV2";
    private static final String RECEIVE_REQUEST_TYPE =
        "iroha_data_model::offline::model::OfflineBearerReceiveRequestV2";
    private static final String DEBIT_RECEIPT_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerDebitReceiptPayloadV2";
    private static final String DEBIT_RECEIPT_TYPE =
        "iroha_data_model::offline::model::OfflineBearerDebitReceiptV2";
    private static final String CREDIT_RECEIPT_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCreditReceiptPayloadV2";
    private static final String CREDIT_RECEIPT_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCreditReceiptV2";
    private static final String SETTLEMENT_BATCH_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerSettlementBatchPayloadV2";
    private static final String SETTLEMENT_BATCH_TYPE =
        "iroha_data_model::offline::model::OfflineBearerSettlementBatchV2";

    private static final String POLICY_DOMAIN = "iroha:offline-bearer-v2:policy-bundle";
    private static final String CERTIFICATE_DOMAIN = "iroha:offline-bearer-v2:certificate";
    private static final String RECEIVE_REQUEST_DOMAIN = "iroha:offline-bearer-v2:receive-request";
    private static final String DEBIT_RECEIPT_DOMAIN = "iroha:offline-bearer-v2:debit-receipt";
    private static final String CREDIT_RECEIPT_DOMAIN = "iroha:offline-bearer-v2:credit-receipt";
    private static final String SETTLEMENT_BATCH_DOMAIN = "iroha:offline-bearer-v2:settlement-batch";

    private static final int FLAGS = NoritoHeader.COMPACT_LEN;

    private Payloads() {}

    public static byte[] policyUnsignedPayload(final PolicyBundleV2 policy) {
      return frame(POLICY_PAYLOAD_TYPE, policyPayload(policy, false, true));
    }

    public static byte[] certificateUnsignedPayload(final CertificateV2 certificate) {
      return frame(CERTIFICATE_PAYLOAD_TYPE, certificatePayload(certificate, false, true));
    }

    public static byte[] receiveRequestUnsignedPayload(final ReceiveRequestV2 request) {
      return frame(
          RECEIVE_REQUEST_PAYLOAD_TYPE,
          structPayload(
              stringPayload(RECEIVE_REQUEST_DOMAIN),
              u16Payload(request.version()),
              stringPayload(request.chainId()),
              stringPayload(request.paymentRequestId()),
              hashPayload(signedCertificateHash(request.recipientCertificate())),
              stringPayload(request.assetDefinitionId()),
              stringPayload(request.amount()),
              u64Payload(request.createdAtMs()),
              u64Payload(request.expiresAtMs()),
              stringPayload(request.policyHashHex()),
              stringPayload(request.signatureAlgorithm())));
    }

    public static byte[] debitReceiptUnsignedPayload(final DebitReceiptV2 receipt) {
      return frame(
          DEBIT_RECEIPT_PAYLOAD_TYPE,
          structPayload(
              stringPayload(DEBIT_RECEIPT_DOMAIN),
              u16Payload(receipt.version()),
              stringPayload(receipt.transferId()),
              stringPayload(receipt.chainId()),
              stringPayload(receipt.paymentRequestId()),
              hashPayload(signedCertificateHash(receipt.senderCertificate())),
              hashPayload(signedCertificateHash(receipt.recipientCertificate())),
              stringPayload(receipt.assetDefinitionId()),
              stringPayload(receipt.amount()),
              stringPayload(receipt.senderPreBalance()),
              stringPayload(receipt.senderPostBalance()),
              u64Payload(receipt.senderSequence()),
              u64Payload(receipt.createdAtMs()),
              u64Payload(receipt.expiresAtMs()),
              stringPayload(receipt.policyHashHex()),
              bytesVecPayload(receipt.receiveChallengeSignature()),
              stringPayload(receipt.signatureAlgorithm())));
    }

    public static byte[] creditReceiptUnsignedPayload(final CreditReceiptV2 receipt) {
      return frame(
          CREDIT_RECEIPT_PAYLOAD_TYPE,
          structPayload(
              stringPayload(CREDIT_RECEIPT_DOMAIN),
              u16Payload(receipt.version()),
              stringPayload(receipt.transferId()),
              stringPayload(receipt.chainId()),
              hashPayload(signedCertificateHash(receipt.recipientCertificate())),
              stringPayload(receipt.amount()),
              stringPayload(receipt.recipientPreBalance()),
              stringPayload(receipt.recipientPostBalance()),
              u64Payload(receipt.recipientSequence()),
              u64Payload(receipt.acceptedAtMs()),
              stringPayload(receipt.signatureAlgorithm())));
    }

    public static byte[] settlementBatchUnsignedPayload(final SettlementBatchV2 batch) {
      final List<byte[]> debitHashes = new ArrayList<>();
      for (final DebitReceiptV2 receipt : batch.debitReceipts()) {
        debitHashes.add(signedDebitReceiptHash(receipt));
      }
      final List<byte[]> creditHashes = new ArrayList<>();
      for (final CreditReceiptV2 receipt : batch.creditReceipts()) {
        creditHashes.add(signedCreditReceiptHash(receipt));
      }
      return frame(
          SETTLEMENT_BATCH_PAYLOAD_TYPE,
          structPayload(
              stringPayload(SETTLEMENT_BATCH_DOMAIN),
              u16Payload(batch.version()),
              stringPayload(batch.chainId()),
              stringPayload(batch.purseId()),
              hashVecPayload(debitHashes),
              hashVecPayload(creditHashes)));
    }

    private static byte[] policyPayload(
        final PolicyBundleV2 policy, final boolean includeSignature, final boolean includeDomain) {
      final List<byte[]> fields = new ArrayList<>();
      if (includeDomain) {
        fields.add(stringPayload(POLICY_DOMAIN));
      }
      fields.add(stringPayload(policy.policyId()));
      fields.add(stringPayload(policy.policyHashHex()));
      fields.add(stringPayload(policy.issuerId()));
      fields.add(u64Payload(policy.issuedAtMs()));
      fields.add(u64Payload(policy.expiresAtMs()));
      fields.add(u64Payload(policy.maxCertificateAgeMs()));
      fields.add(u64Payload(policy.maxPolicyAgeMs()));
      fields.add(u64Payload(policy.maxTokenAgeMs()));
      fields.add(stringPayload(policy.maxOfflineBalance()));
      fields.add(stringPayload(policy.maxTransactionAmount()));
      fields.add(stringVecPayload(policy.allowedHardwareClasses()));
      fields.add(stringVecPayload(policy.blacklistedAccountIds()));
      fields.add(stringVecPayload(policy.blacklistedDeviceIds()));
      fields.add(stringVecPayload(policy.blacklistedKeyIds()));
      fields.add(stringPayload(policy.signatureAlgorithm()));
      if (includeSignature) {
        fields.add(bytesVecPayload(policy.issuerSignature()));
      }
      fields.add(u64Payload(policy.policyEpoch()));
      fields.add(stringPayload(policy.policySource()));
      fields.add(stringVecPayload(policy.revokedCertificateIds()));
      fields.add(stringVecPayload(policy.revokedTransferIds()));
      fields.add(assetSendLimitVecPayload(policy.assetSendLimits()));
      return structPayload(fields);
    }

    private static byte[] certificatePayload(
        final CertificateV2 certificate,
        final boolean includeSignature,
        final boolean includeDomain) {
      final List<byte[]> fields = new ArrayList<>();
      if (includeDomain) {
        fields.add(stringPayload(CERTIFICATE_DOMAIN));
      }
      fields.add(stringPayload(certificate.certificateId()));
      fields.add(stringPayload(certificate.chainId()));
      fields.add(stringPayload(certificate.issuerId()));
      fields.add(stringPayload(certificate.purseId()));
      fields.add(stringPayload(certificate.accountId()));
      fields.add(stringPayload(certificate.assetDefinitionId()));
      fields.add(stringPayload(certificate.deviceId()));
      fields.add(stringPayload(certificate.keyId()));
      fields.add(stringPayload(certificate.hardwareClass()));
      fields.add(stringPayload(certificate.signatureAlgorithm()));
      fields.add(stringPayload(certificate.publicKeyEncoding()));
      fields.add(bytesVecPayload(certificate.publicKey()));
      fields.add(u64Payload(certificate.issuedAtMs()));
      fields.add(u64Payload(certificate.expiresAtMs()));
      fields.add(stringPayload(certificate.policyId()));
      fields.add(stringPayload(certificate.policyHashHex()));
      if (includeSignature) {
        fields.add(bytesVecPayload(certificate.issuerSignature()));
      }
      return structPayload(fields);
    }

    private static byte[] receiveRequestPayload(
        final ReceiveRequestV2 request, final boolean includeSignature) {
      final List<byte[]> fields = new ArrayList<>();
      fields.add(u16Payload(request.version()));
      fields.add(stringPayload(request.chainId()));
      fields.add(stringPayload(request.paymentRequestId()));
      fields.add(certificatePayload(request.recipientCertificate(), true, false));
      fields.add(stringPayload(request.assetDefinitionId()));
      fields.add(stringPayload(request.amount()));
      fields.add(u64Payload(request.createdAtMs()));
      fields.add(u64Payload(request.expiresAtMs()));
      fields.add(stringPayload(request.policyHashHex()));
      fields.add(stringPayload(request.signatureAlgorithm()));
      if (includeSignature) {
        fields.add(bytesVecPayload(request.challengeSignature()));
      }
      return structPayload(fields);
    }

    private static byte[] debitReceiptPayload(
        final DebitReceiptV2 receipt, final boolean includeSignature) {
      final List<byte[]> fields = new ArrayList<>();
      fields.add(u16Payload(receipt.version()));
      fields.add(stringPayload(receipt.transferId()));
      fields.add(stringPayload(receipt.chainId()));
      fields.add(stringPayload(receipt.paymentRequestId()));
      fields.add(certificatePayload(receipt.senderCertificate(), true, false));
      fields.add(certificatePayload(receipt.recipientCertificate(), true, false));
      fields.add(stringPayload(receipt.assetDefinitionId()));
      fields.add(stringPayload(receipt.amount()));
      fields.add(stringPayload(receipt.senderPreBalance()));
      fields.add(stringPayload(receipt.senderPostBalance()));
      fields.add(u64Payload(receipt.senderSequence()));
      fields.add(u64Payload(receipt.createdAtMs()));
      fields.add(u64Payload(receipt.expiresAtMs()));
      fields.add(stringPayload(receipt.policyHashHex()));
      fields.add(bytesVecPayload(receipt.receiveChallengeSignature()));
      fields.add(stringPayload(receipt.signatureAlgorithm()));
      if (includeSignature) {
        fields.add(bytesVecPayload(receipt.debitSignature()));
      }
      return structPayload(fields);
    }

    private static byte[] creditReceiptPayload(
        final CreditReceiptV2 receipt, final boolean includeSignature) {
      final List<byte[]> fields = new ArrayList<>();
      fields.add(u16Payload(receipt.version()));
      fields.add(stringPayload(receipt.transferId()));
      fields.add(stringPayload(receipt.chainId()));
      fields.add(certificatePayload(receipt.recipientCertificate(), true, false));
      fields.add(stringPayload(receipt.amount()));
      fields.add(stringPayload(receipt.recipientPreBalance()));
      fields.add(stringPayload(receipt.recipientPostBalance()));
      fields.add(u64Payload(receipt.recipientSequence()));
      fields.add(u64Payload(receipt.acceptedAtMs()));
      fields.add(stringPayload(receipt.signatureAlgorithm()));
      if (includeSignature) {
        fields.add(bytesVecPayload(receipt.creditSignature()));
      }
      return structPayload(fields);
    }

    private static byte[] settlementBatchPayload(final SettlementBatchV2 batch) {
      final List<byte[]> debitReceipts = new ArrayList<>();
      for (final DebitReceiptV2 receipt : batch.debitReceipts()) {
        debitReceipts.add(debitReceiptPayload(receipt, true));
      }
      final List<byte[]> creditReceipts = new ArrayList<>();
      for (final CreditReceiptV2 receipt : batch.creditReceipts()) {
        creditReceipts.add(creditReceiptPayload(receipt, true));
      }
      return structPayload(
          u16Payload(batch.version()),
          stringPayload(batch.chainId()),
          stringPayload(batch.purseId()),
          vecPayload(debitReceipts),
          vecPayload(creditReceipts));
    }

    private static byte[] signedCertificateHash(final CertificateV2 certificate) {
      return IrohaHash.prehash(
          frame(CERTIFICATE_TYPE, certificatePayload(certificate, true, false)));
    }

    private static byte[] signedDebitReceiptHash(final DebitReceiptV2 receipt) {
      return IrohaHash.prehash(frame(DEBIT_RECEIPT_TYPE, debitReceiptPayload(receipt, true)));
    }

    private static byte[] signedCreditReceiptHash(final CreditReceiptV2 receipt) {
      return IrohaHash.prehash(frame(CREDIT_RECEIPT_TYPE, creditReceiptPayload(receipt, true)));
    }

    @SuppressWarnings("unused")
    private static byte[] signedPolicyBytes(final PolicyBundleV2 policy) {
      return frame(POLICY_TYPE, policyPayload(policy, true, false));
    }

    @SuppressWarnings("unused")
    private static byte[] signedReceiveRequestBytes(final ReceiveRequestV2 request) {
      return frame(RECEIVE_REQUEST_TYPE, receiveRequestPayload(request, true));
    }

    @SuppressWarnings("unused")
    private static byte[] signedSettlementBatchBytes(final SettlementBatchV2 batch) {
      return frame(SETTLEMENT_BATCH_TYPE, settlementBatchPayload(batch));
    }

    private static byte[] assetSendLimitVecPayload(final List<AssetSendLimitV2> limits) {
      final List<AssetSendLimitV2> sorted = new ArrayList<>(limits);
      sorted.sort((lhs, rhs) -> lhs.assetDefinitionId().compareTo(rhs.assetDefinitionId()));
      final List<byte[]> encoded = new ArrayList<>();
      for (final AssetSendLimitV2 limit : sorted) {
        encoded.add(
            structPayload(
                stringPayload(limit.assetDefinitionId()),
                stringPayload(limit.maxTransactionAmount()),
                stringPayload(limit.dailySendLimit()),
                stringPayload(limit.monthlySendLimit())));
      }
      return vecPayload(encoded);
    }

    private static byte[] stringVecPayload(final Collection<String> values) {
      final List<String> normalized = new ArrayList<>();
      for (final String value : values) {
        if (value != null && !value.trim().isEmpty()) {
          normalized.add(value.trim());
        }
      }
      Collections.sort(normalized);
      final List<byte[]> encoded = new ArrayList<>();
      String previous = null;
      for (final String value : normalized) {
        if (!value.equals(previous)) {
          encoded.add(stringPayload(value));
          previous = value;
        }
      }
      return vecPayload(encoded);
    }

    private static byte[] hashVecPayload(final List<byte[]> values) {
      final List<byte[]> encoded = new ArrayList<>();
      for (final byte[] value : values) {
        encoded.add(hashPayload(value));
      }
      return vecPayload(encoded);
    }

    private static byte[] vecPayload(final List<byte[]> values) {
      final NoritoEncoder encoder = new NoritoEncoder(FLAGS);
      encoder.writeLength(values.size(), true);
      for (final byte[] value : values) {
        encoder.writeLength(value.length, true);
        encoder.writeBytes(value);
      }
      return encoder.toByteArray();
    }

    private static byte[] structPayload(final byte[]... fields) {
      return structPayload(Arrays.asList(fields));
    }

    private static byte[] structPayload(final List<byte[]> fields) {
      final NoritoEncoder encoder = new NoritoEncoder(FLAGS);
      for (final byte[] field : fields) {
        encoder.writeLength(field.length, true);
        encoder.writeBytes(field);
      }
      return encoder.toByteArray();
    }

    private static byte[] stringPayload(final String value) {
      final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
      final NoritoEncoder encoder = new NoritoEncoder(FLAGS);
      encoder.writeLength(bytes.length, true);
      encoder.writeBytes(bytes);
      return encoder.toByteArray();
    }

    private static byte[] bytesVecPayload(final byte[] value) {
      final NoritoEncoder encoder = new NoritoEncoder(FLAGS);
      encoder.writeLength(value.length, true);
      encoder.writeBytes(value);
      return encoder.toByteArray();
    }

    private static byte[] hashPayload(final byte[] value) {
      if (value.length != 32) {
        throw new IllegalArgumentException("Offline Bearer hash fields must be 32 bytes");
      }
      return Arrays.copyOf(value, value.length);
    }

    private static byte[] u16Payload(final int value) {
      if (value < 0 || value > 0xFFFF) {
        throw new IllegalArgumentException("u16 value is out of range");
      }
      final NoritoEncoder encoder = new NoritoEncoder(FLAGS);
      encoder.writeUInt(value, 16);
      return encoder.toByteArray();
    }

    private static byte[] u64Payload(final long value) {
      if (value < 0L) {
        throw new IllegalArgumentException("u64 value must be non-negative");
      }
      final NoritoEncoder encoder = new NoritoEncoder(FLAGS);
      encoder.writeUInt(value, 64);
      return encoder.toByteArray();
    }

    private static byte[] frame(final String typeName, final byte[] payload) {
      final byte[] header =
          new NoritoHeader(
                  SchemaHash.hash16(typeName),
                  payload.length,
                  CRC64.compute(payload),
                  FLAGS,
                  NoritoHeader.COMPRESSION_NONE)
              .encode();
      final ByteArrayOutputStream output = new ByteArrayOutputStream(header.length + payload.length);
      output.write(header, 0, header.length);
      output.write(payload, 0, payload.length);
      return output.toByteArray();
    }
  }

  /** Verifies Offline Bearer v2 issuer and hardware-purse signatures. */
  public interface SignatureVerifying {
    void verifyPolicy(PolicyBundleV2 policy);

    void verifyCertificate(CertificateV2 certificate, PolicyBundleV2 policy);

    void verifyReceiveRequest(ReceiveRequestV2 request);

    void verifyDebitReceipt(DebitReceiptV2 receipt);

    void verifyCreditReceipt(CreditReceiptV2 receipt);
  }

  /** Fail-closed verifier used when an app has not configured trusted issuer keys. */
  public static final class RejectingSignatureVerifier implements SignatureVerifying {
    @Override
    public void verifyPolicy(final PolicyBundleV2 policy) {
      throw new PolicyException("Offline Bearer issuer signature verifier is not configured");
    }

    @Override
    public void verifyCertificate(final CertificateV2 certificate, final PolicyBundleV2 policy) {
      throw new PolicyException("Offline Bearer issuer signature verifier is not configured");
    }

    @Override
    public void verifyReceiveRequest(final ReceiveRequestV2 request) {
      throw new PolicyException("Offline Bearer device signature verifier is not configured");
    }

    @Override
    public void verifyDebitReceipt(final DebitReceiptV2 receipt) {
      throw new PolicyException("Offline Bearer device signature verifier is not configured");
    }

    @Override
    public void verifyCreditReceipt(final CreditReceiptV2 receipt) {
      throw new PolicyException("Offline Bearer device signature verifier is not configured");
    }
  }

  /** Ed25519 and P-256 verifier for issuer roots and hardware-purse public keys. */
  public static final class SignatureVerifier implements SignatureVerifying {
    private final List<byte[]> trustedIssuerPublicKeys;

    public SignatureVerifier(final Collection<byte[]> trustedIssuerPublicKeys) {
      this.trustedIssuerPublicKeys = new ArrayList<>();
      for (final byte[] key : Objects.requireNonNull(trustedIssuerPublicKeys, "trustedIssuerPublicKeys")) {
        this.trustedIssuerPublicKeys.add(Arrays.copyOf(key, key.length));
      }
    }

    @Override
    public void verifyPolicy(final PolicyBundleV2 policy) {
      verifyIssuerSignature(
          policy.signatureAlgorithm(),
          Payloads.policyUnsignedPayload(policy),
          policy.issuerSignature(),
          "Offline Bearer policy issuer signature is invalid");
    }

    @Override
    public void verifyCertificate(final CertificateV2 certificate, final PolicyBundleV2 policy) {
      if (!certificate.issuerId().equals(policy.issuerId())
          || !equalsIgnoreCase(certificate.policyHashHex(), policy.policyHashHex())) {
        throw new PolicyException("Offline Bearer certificate does not match policy");
      }
      verifyIssuerSignature(
          policy.signatureAlgorithm(),
          Payloads.certificateUnsignedPayload(certificate),
          certificate.issuerSignature(),
          "Offline Bearer certificate issuer signature is invalid");
    }

    @Override
    public void verifyReceiveRequest(final ReceiveRequestV2 request) {
      if (!request.signatureAlgorithm().equals(request.recipientCertificate().signatureAlgorithm())) {
        throw new PolicyException("Offline Bearer receive request algorithm does not match certificate");
      }
      verifyDeviceSignature(
          request.signatureAlgorithm(),
          request.recipientCertificate().publicKey(),
          Payloads.receiveRequestUnsignedPayload(request),
          request.challengeSignature(),
          "Offline Bearer receive request signature is invalid");
    }

    @Override
    public void verifyDebitReceipt(final DebitReceiptV2 receipt) {
      if (!receipt.signatureAlgorithm().equals(receipt.senderCertificate().signatureAlgorithm())) {
        throw new PolicyException("Offline Bearer debit receipt algorithm does not match certificate");
      }
      verifyDeviceSignature(
          receipt.signatureAlgorithm(),
          receipt.senderCertificate().publicKey(),
          Payloads.debitReceiptUnsignedPayload(receipt),
          receipt.debitSignature(),
          "Offline Bearer debit receipt signature is invalid");
    }

    @Override
    public void verifyCreditReceipt(final CreditReceiptV2 receipt) {
      if (!receipt.signatureAlgorithm().equals(receipt.recipientCertificate().signatureAlgorithm())) {
        throw new PolicyException("Offline Bearer credit receipt algorithm does not match certificate");
      }
      verifyDeviceSignature(
          receipt.signatureAlgorithm(),
          receipt.recipientCertificate().publicKey(),
          Payloads.creditReceiptUnsignedPayload(receipt),
          receipt.creditSignature(),
          "Offline Bearer credit receipt signature is invalid");
    }

    private void verifyIssuerSignature(
        final String algorithm, final byte[] payload, final byte[] signature, final String message) {
      for (final byte[] key : trustedIssuerPublicKeys) {
        if (verifySignature(algorithm, key, payload, signature)) {
          return;
        }
      }
      throw new PolicyException(message);
    }

    private void verifyDeviceSignature(
        final String algorithm,
        final byte[] publicKey,
        final byte[] payload,
        final byte[] signature,
        final String message) {
      if (!verifySignature(algorithm, publicKey, payload, signature)) {
        throw new PolicyException(message);
      }
    }

    private static boolean verifySignature(
        final String algorithm,
        final byte[] publicKey,
        final byte[] payload,
        final byte[] signature) {
      if (SIGNATURE_ALGORITHM_ED25519.equals(algorithm)) {
        return verifyEd25519(publicKey, payload, signature);
      }
      if (SIGNATURE_ALGORITHM_ECDSA_P256_SHA256.equals(algorithm)) {
        return verifyP256(publicKey, payload, signature);
      }
      return false;
    }

    private static boolean verifyEd25519(
        final byte[] publicKey, final byte[] payload, final byte[] signature) {
      if (publicKey.length != 32) {
        return false;
      }
      try {
        final Ed25519Signer verifier = new Ed25519Signer();
        verifier.init(false, new Ed25519PublicKeyParameters(publicKey, 0));
        verifier.update(payload, 0, payload.length);
        return verifier.verifySignature(signature);
      } catch (final RuntimeException ex) {
        return false;
      }
    }

    private static boolean verifyP256(
        final byte[] publicKey, final byte[] payload, final byte[] signature) {
      try {
        final X9ECParameters params = ECNamedCurveTable.getByName("secp256r1");
        if (params == null) {
          return false;
        }
        final ECDomainParameters domain =
            new ECDomainParameters(params.getCurve(), params.getG(), params.getN(), params.getH());
        final ECPublicKeyParameters key =
            new ECPublicKeyParameters(params.getCurve().decodePoint(publicKey), domain);
        final ASN1Sequence sequence = ASN1Sequence.getInstance(signature);
        if (sequence.size() != 2) {
          return false;
        }
        final BigInteger r = ((ASN1Integer) sequence.getObjectAt(0)).getPositiveValue();
        final BigInteger s = ((ASN1Integer) sequence.getObjectAt(1)).getPositiveValue();
        final byte[] digest = MessageDigest.getInstance("SHA-256").digest(payload);
        final ECDSASigner verifier = new ECDSASigner();
        verifier.init(false, key);
        return verifier.verifySignature(digest, r, s);
      } catch (final RuntimeException ex) {
        return false;
      } catch (final Exception ex) {
        return false;
      }
    }
  }

  /** Verifies exported Offline Bearer settlement batches before online submission. */
  public static final class SettlementBatchVerifier {
    private SettlementBatchVerifier() {}

    public static void verify(
        final SettlementBatchV2 batch,
        final PolicyBundleV2 policy,
        final SignatureVerifying signatureVerifier) {
      verify(batch, policy, signatureVerifier, System.currentTimeMillis());
    }

    public static void verify(
        final SettlementBatchV2 batch,
        final PolicyBundleV2 policy,
        final SignatureVerifying signatureVerifier,
        final long now) {
      Objects.requireNonNull(batch, "batch");
      Objects.requireNonNull(policy, "policy");
      Objects.requireNonNull(signatureVerifier, "signatureVerifier");
      signatureVerifier.verifyPolicy(policy);
      requirePolicyFresh(policy, now);
      if (!policyHashMatches(batch, policy)) {
        throw new PolicyException("settlement batch policy hash does not match current policy");
      }
      final Set<String> creditTransferIds = new LinkedHashSet<>();
      for (final CreditReceiptV2 receipt : batch.creditReceipts()) {
        creditTransferIds.add(receipt.transferId());
      }
      for (final DebitReceiptV2 receipt : batch.debitReceipts()) {
        verifyDebitReceipt(batch, receipt, policy, signatureVerifier, now, creditTransferIds);
      }
      final Map<String, DebitReceiptV2> debitByTransferId = new HashMap<>();
      for (final DebitReceiptV2 receipt : batch.debitReceipts()) {
        debitByTransferId.put(receipt.transferId(), receipt);
      }
      for (final CreditReceiptV2 receipt : batch.creditReceipts()) {
        verifyCreditReceipt(batch, receipt, policy, signatureVerifier, now);
        final DebitReceiptV2 debit = debitByTransferId.get(receipt.transferId());
        if (debit == null) {
          throw new PolicyException("settlement credit is missing its accepted debit receipt");
        }
        if (!receipt.amount().equals(debit.amount())) {
          throw new PolicyException("settlement credit amount does not match debit receipt");
        }
        if (!receipt.chainId().equals(debit.chainId())) {
          throw new PolicyException("settlement credit chainId does not match debit receipt");
        }
        if (!receipt.recipientCertificate().purseId().equals(debit.recipientCertificate().purseId())) {
          throw new PolicyException("settlement credit recipient purse does not match debit receipt");
        }
        if (receipt.acceptedAtMs() > debit.expiresAtMs()) {
          throw new PolicyException("settlement credit was accepted after debit receipt expiry");
        }
      }
    }

    private static boolean policyHashMatches(
        final SettlementBatchV2 batch, final PolicyBundleV2 policy) {
      for (final DebitReceiptV2 receipt : batch.debitReceipts()) {
        if (!equalsIgnoreCase(receipt.policyHashHex(), policy.policyHashHex())) {
          return false;
        }
      }
      for (final CreditReceiptV2 receipt : batch.creditReceipts()) {
        if (!equalsIgnoreCase(receipt.recipientCertificate().policyHashHex(), policy.policyHashHex())) {
          return false;
        }
      }
      return true;
    }

    private static void verifyDebitReceipt(
        final SettlementBatchV2 batch,
        final DebitReceiptV2 receipt,
        final PolicyBundleV2 policy,
        final SignatureVerifying signatureVerifier,
        final long now,
        final Set<String> creditTransferIds) {
      if (!receipt.chainId().equals(batch.chainId())) {
        throw new PolicyException("debit receipt chainId does not match settlement batch");
      }
      final boolean senderExport = receipt.senderCertificate().purseId().equals(batch.purseId());
      final boolean recipientExport = receipt.recipientCertificate().purseId().equals(batch.purseId());
      if (!senderExport && !recipientExport) {
        throw new PolicyException("debit receipt purse does not match settlement batch");
      }
      if (recipientExport && !senderExport && !creditTransferIds.contains(receipt.transferId())) {
        throw new PolicyException("receiver settlement batch must include a credit receipt for its accepted debit");
      }
      if (receipt.createdAtMs() > now) {
        throw new PolicyException("debit receipt is from the future");
      }
      if (!equalsIgnoreCase(receipt.policyHashHex(), policy.policyHashHex())) {
        throw new PolicyException("debit receipt policy hash does not match current policy");
      }
      if (policy.revokedTransferIds().contains(receipt.transferId())) {
        throw new PolicyException("Offline Bearer transfer is revoked");
      }
      if (policy.revokedTransferIds().contains(receipt.paymentRequestId())) {
        throw new PolicyException("Offline Bearer receive request is revoked");
      }
      if (!receipt.senderCertificate().assetDefinitionId().equals(receipt.assetDefinitionId())) {
        throw new PolicyException("sender certificate asset does not match debit receipt");
      }
      if (!receipt.recipientCertificate().assetDefinitionId().equals(receipt.assetDefinitionId())) {
        throw new PolicyException("recipient certificate asset does not match debit receipt");
      }
      requireAmountAtMost(
          receipt.amount(), policy.maxTransactionAmount(), "amount exceeds offline transaction policy");
      requireAmountAtMost(
          receipt.amount(),
          maxTransactionAmountForAsset(policy, receipt.assetDefinitionId()),
          "amount exceeds offline asset transaction policy");
      if (decimal(receipt.senderPreBalance())
              .subtract(decimal(receipt.amount()))
              .compareTo(decimal(receipt.senderPostBalance()))
          != 0) {
        throw new PolicyException("debit receipt balance transition is invalid");
      }
      enforceCertificatePolicyAt(
          receipt.senderCertificate(), policy, receipt.createdAtMs(), signatureVerifier);
      enforceCertificatePolicyAt(
          receipt.recipientCertificate(), policy, receipt.createdAtMs(), signatureVerifier);
      signatureVerifier.verifyDebitReceipt(receipt);
    }

    private static void verifyCreditReceipt(
        final SettlementBatchV2 batch,
        final CreditReceiptV2 receipt,
        final PolicyBundleV2 policy,
        final SignatureVerifying signatureVerifier,
        final long now) {
      if (!receipt.chainId().equals(batch.chainId())) {
        throw new PolicyException("credit receipt chainId does not match settlement batch");
      }
      if (!receipt.recipientCertificate().purseId().equals(batch.purseId())) {
        throw new PolicyException("credit receipt recipient purse does not match settlement batch");
      }
      if (receipt.acceptedAtMs() > now) {
        throw new PolicyException("credit receipt is from the future");
      }
      if (policy.revokedTransferIds().contains(receipt.transferId())) {
        throw new PolicyException("Offline Bearer transfer is revoked");
      }
      requireAmountAtMost(
          receipt.amount(), policy.maxTransactionAmount(), "amount exceeds offline transaction policy");
      requireAmountAtMost(
          receipt.amount(),
          maxTransactionAmountForAsset(policy, receipt.recipientCertificate().assetDefinitionId()),
          "amount exceeds offline asset transaction policy");
      if (decimal(receipt.recipientPreBalance())
              .add(decimal(receipt.amount()))
              .compareTo(decimal(receipt.recipientPostBalance()))
          != 0) {
        throw new PolicyException("credit receipt balance transition is invalid");
      }
      enforceCertificatePolicyAt(
          receipt.recipientCertificate(), policy, receipt.acceptedAtMs(), signatureVerifier);
      signatureVerifier.verifyCreditReceipt(receipt);
    }

    private static void enforceCertificatePolicyAt(
        final CertificateV2 certificate,
        final PolicyBundleV2 policy,
        final long eventTimeMs,
        final SignatureVerifying signatureVerifier) {
      requirePolicyFresh(policy, eventTimeMs);
      if (certificate.issuedAtMs() > eventTimeMs) {
        throw new PolicyException("Offline Bearer certificate is from the future");
      }
      if (certificate.expiresAtMs() <= eventTimeMs) {
        throw new PolicyException("Offline Bearer certificate is expired");
      }
      if (eventTimeMs - certificate.issuedAtMs() > policy.maxCertificateAgeMs()) {
        throw new PolicyException("Offline Bearer certificate is too old");
      }
      if (!certificate.issuerId().equals(policy.issuerId())) {
        throw new PolicyException("Offline Bearer certificate issuer does not match policy");
      }
      if (!equalsIgnoreCase(certificate.policyHashHex(), policy.policyHashHex())) {
        throw new PolicyException("Offline Bearer certificate policy hash does not match policy");
      }
      requireSupportedSignatureAlgorithm(certificate.signatureAlgorithm());
      requireSupportedPublicKeyEncoding(certificate.publicKeyEncoding());
      if (!policy.allowedHardwareClasses().contains(certificate.hardwareClass())) {
        throw new PolicyException("certificate hardware class is not allowed by policy");
      }
      if (policy.blacklistedAccountIds().contains(certificate.accountId())
          || policy.blacklistedDeviceIds().contains(certificate.deviceId())
          || policy.blacklistedKeyIds().contains(certificate.keyId())
          || policy.revokedCertificateIds().contains(certificate.certificateId())
          || policy.revokedCertificateIds().contains(certificate.keyId())) {
        throw new PolicyException("Offline Bearer certificate is blacklisted");
      }
      signatureVerifier.verifyCertificate(certificate, policy);
    }
  }

  public static final class SecureElementCapabilities {
    private final boolean hardwareBacked;
    private final boolean statefulPurse;
    private final String hardwareClass;
    private final String attestationKeyId;
    private final String signatureAlgorithm;
    private final String publicKeyEncoding;
    private final boolean rollbackResistantState;
    private final byte[] attestationEvidence;

    public SecureElementCapabilities(
        final boolean hardwareBacked,
        final boolean statefulPurse,
        final String hardwareClass,
        final String attestationKeyId) {
      this(
          hardwareBacked,
          statefulPurse,
          hardwareClass,
          attestationKeyId,
          SIGNATURE_ALGORITHM_ED25519,
          PUBLIC_KEY_ENCODING_RAW_ED25519,
          false,
          new byte[0]);
    }

    public SecureElementCapabilities(
        final boolean hardwareBacked,
        final boolean statefulPurse,
        final String hardwareClass,
        final String attestationKeyId,
        final String signatureAlgorithm,
        final String publicKeyEncoding,
        final boolean rollbackResistantState,
        final byte[] attestationEvidence) {
      requireNonBlank(hardwareClass, "hardwareClass");
      requireSupportedSignatureAlgorithm(signatureAlgorithm);
      requireSupportedPublicKeyEncoding(publicKeyEncoding);
      this.hardwareBacked = hardwareBacked;
      this.statefulPurse = statefulPurse;
      this.hardwareClass = hardwareClass;
      this.attestationKeyId = attestationKeyId;
      this.signatureAlgorithm = signatureAlgorithm;
      this.publicKeyEncoding = publicKeyEncoding;
      this.rollbackResistantState = rollbackResistantState;
      this.attestationEvidence =
          attestationEvidence == null ? new byte[0] : Arrays.copyOf(attestationEvidence, attestationEvidence.length);
    }

    public boolean hardwareBacked() {
      return hardwareBacked;
    }

    public boolean statefulPurse() {
      return statefulPurse;
    }

    public String hardwareClass() {
      return hardwareClass;
    }

    public String attestationKeyId() {
      return attestationKeyId;
    }

    public String signatureAlgorithm() {
      return signatureAlgorithm;
    }

    public String publicKeyEncoding() {
      return publicKeyEncoding;
    }

    public boolean rollbackResistantState() {
      return rollbackResistantState;
    }

    public byte[] attestationEvidence() {
      return Arrays.copyOf(attestationEvidence, attestationEvidence.length);
    }
  }

  public static final class CertificateV2 {
    private final String certificateId;
    private final String chainId;
    private final String issuerId;
    private final String purseId;
    private final String accountId;
    private final String assetDefinitionId;
    private final String deviceId;
    private final String keyId;
    private final String hardwareClass;
    private final String signatureAlgorithm;
    private final String publicKeyEncoding;
    private final byte[] publicKey;
    private final long issuedAtMs;
    private final long expiresAtMs;
    private final String policyId;
    private final String policyHashHex;
    private final byte[] issuerSignature;

    public CertificateV2(
        final String certificateId,
        final String chainId,
        final String issuerId,
        final String purseId,
        final String accountId,
        final String assetDefinitionId,
        final String deviceId,
        final String keyId,
        final String hardwareClass,
        final byte[] publicKey,
        final long issuedAtMs,
        final long expiresAtMs,
        final String policyId,
        final String policyHashHex,
        final byte[] issuerSignature) {
      this(
          certificateId,
          chainId,
          issuerId,
          purseId,
          accountId,
          assetDefinitionId,
          deviceId,
          keyId,
          hardwareClass,
          SIGNATURE_ALGORITHM_ED25519,
          PUBLIC_KEY_ENCODING_RAW_ED25519,
          publicKey,
          issuedAtMs,
          expiresAtMs,
          policyId,
          policyHashHex,
          issuerSignature);
    }

    public CertificateV2(
        final String certificateId,
        final String chainId,
        final String issuerId,
        final String purseId,
        final String accountId,
        final String assetDefinitionId,
        final String deviceId,
        final String keyId,
        final String hardwareClass,
        final String signatureAlgorithm,
        final String publicKeyEncoding,
        final byte[] publicKey,
        final long issuedAtMs,
        final long expiresAtMs,
        final String policyId,
        final String policyHashHex,
        final byte[] issuerSignature) {
      requireNonBlank(certificateId, "certificateId");
      requireNonBlank(chainId, "chainId");
      requireNonBlank(issuerId, "issuerId");
      requireNonBlank(purseId, "purseId");
      requireNonBlank(accountId, "accountId");
      requireNonBlank(assetDefinitionId, "assetDefinitionId");
      requireNonBlank(deviceId, "deviceId");
      requireNonBlank(keyId, "keyId");
      requireNonBlank(hardwareClass, "hardwareClass");
      requireSupportedSignatureAlgorithm(signatureAlgorithm);
      requireSupportedPublicKeyEncoding(publicKeyEncoding);
      if (issuedAtMs < 0 || expiresAtMs <= issuedAtMs) {
        throw new IllegalArgumentException("certificate time range is invalid");
      }
      requireNonBlank(policyId, "policyId");
      requireHexLike(policyHashHex, "policyHashHex");
      this.certificateId = certificateId;
      this.chainId = chainId;
      this.issuerId = issuerId;
      this.purseId = purseId;
      this.accountId = accountId;
      this.assetDefinitionId = assetDefinitionId;
      this.deviceId = deviceId;
      this.keyId = keyId;
      this.hardwareClass = hardwareClass;
      this.signatureAlgorithm = signatureAlgorithm;
      this.publicKeyEncoding = publicKeyEncoding;
      this.publicKey = requireNonEmptyBytes(publicKey, "publicKey");
      this.issuedAtMs = issuedAtMs;
      this.expiresAtMs = expiresAtMs;
      this.policyId = policyId;
      this.policyHashHex = policyHashHex;
      this.issuerSignature = requireNonEmptyBytes(issuerSignature, "issuerSignature");
    }

    public String certificateId() {
      return certificateId;
    }

    public String chainId() {
      return chainId;
    }

    public String issuerId() {
      return issuerId;
    }

    public String purseId() {
      return purseId;
    }

    public String accountId() {
      return accountId;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public String deviceId() {
      return deviceId;
    }

    public String keyId() {
      return keyId;
    }

    public String hardwareClass() {
      return hardwareClass;
    }

    public String signatureAlgorithm() {
      return signatureAlgorithm;
    }

    public String publicKeyEncoding() {
      return publicKeyEncoding;
    }

    public byte[] publicKey() {
      return Arrays.copyOf(publicKey, publicKey.length);
    }

    public long issuedAtMs() {
      return issuedAtMs;
    }

    public long expiresAtMs() {
      return expiresAtMs;
    }

    public String policyId() {
      return policyId;
    }

    public String policyHashHex() {
      return policyHashHex;
    }

    public byte[] issuerSignature() {
      return Arrays.copyOf(issuerSignature, issuerSignature.length);
    }
  }

  public static final class PolicyBundleV2 {
    public static final long DEFAULT_MAX_CERTIFICATE_AGE_MS = 24L * 60L * 60L * 1000L;
    public static final long DEFAULT_MAX_POLICY_AGE_MS = 12L * 60L * 60L * 1000L;
    public static final long DEFAULT_MAX_TOKEN_AGE_MS = 5L * 60L * 1000L;

    private final String policyId;
    private final String policyHashHex;
    private final String issuerId;
    private final long issuedAtMs;
    private final long expiresAtMs;
    private final long maxCertificateAgeMs;
    private final long maxPolicyAgeMs;
    private final long maxTokenAgeMs;
    private final String maxOfflineBalance;
    private final String maxTransactionAmount;
    private final Set<String> allowedHardwareClasses;
    private final Set<String> blacklistedAccountIds;
    private final Set<String> blacklistedDeviceIds;
    private final Set<String> blacklistedKeyIds;
    private final String signatureAlgorithm;
    private final long policyEpoch;
    private final String policySource;
    private final Set<String> revokedCertificateIds;
    private final Set<String> revokedTransferIds;
    private final List<AssetSendLimitV2> assetSendLimits;
    private final byte[] issuerSignature;

    public PolicyBundleV2(
        final String policyId,
        final String policyHashHex,
        final String issuerId,
        final long issuedAtMs,
        final long expiresAtMs,
        final long maxCertificateAgeMs,
        final long maxPolicyAgeMs,
        final long maxTokenAgeMs,
        final String maxOfflineBalance,
        final String maxTransactionAmount,
        final Collection<String> allowedHardwareClasses,
        final Collection<String> blacklistedAccountIds,
        final Collection<String> blacklistedDeviceIds,
        final Collection<String> blacklistedKeyIds,
        final byte[] issuerSignature) {
      this(
          policyId,
          policyHashHex,
          issuerId,
          issuedAtMs,
          expiresAtMs,
          maxCertificateAgeMs,
          maxPolicyAgeMs,
          maxTokenAgeMs,
          maxOfflineBalance,
          maxTransactionAmount,
          allowedHardwareClasses,
          blacklistedAccountIds,
          blacklistedDeviceIds,
          blacklistedKeyIds,
          SIGNATURE_ALGORITHM_ED25519,
          issuerSignature,
          0L,
          "middleware",
          Collections.emptySet(),
          Collections.emptySet(),
          Collections.emptyList());
    }

    public PolicyBundleV2(
        final String policyId,
        final String policyHashHex,
        final String issuerId,
        final long issuedAtMs,
        final long expiresAtMs,
        final long maxCertificateAgeMs,
        final long maxPolicyAgeMs,
        final long maxTokenAgeMs,
        final String maxOfflineBalance,
        final String maxTransactionAmount,
        final Collection<String> allowedHardwareClasses,
        final Collection<String> blacklistedAccountIds,
        final Collection<String> blacklistedDeviceIds,
        final Collection<String> blacklistedKeyIds,
        final byte[] issuerSignature,
        final long policyEpoch,
        final String policySource,
        final Collection<String> revokedCertificateIds,
        final Collection<String> revokedTransferIds,
        final Collection<AssetSendLimitV2> assetSendLimits) {
      this(
          policyId,
          policyHashHex,
          issuerId,
          issuedAtMs,
          expiresAtMs,
          maxCertificateAgeMs,
          maxPolicyAgeMs,
          maxTokenAgeMs,
          maxOfflineBalance,
          maxTransactionAmount,
          allowedHardwareClasses,
          blacklistedAccountIds,
          blacklistedDeviceIds,
          blacklistedKeyIds,
          SIGNATURE_ALGORITHM_ED25519,
          issuerSignature,
          policyEpoch,
          policySource,
          revokedCertificateIds,
          revokedTransferIds,
          assetSendLimits);
    }

    public PolicyBundleV2(
        final String policyId,
        final String policyHashHex,
        final String issuerId,
        final long issuedAtMs,
        final long expiresAtMs,
        final long maxCertificateAgeMs,
        final long maxPolicyAgeMs,
        final long maxTokenAgeMs,
        final String maxOfflineBalance,
        final String maxTransactionAmount,
        final Collection<String> allowedHardwareClasses,
        final Collection<String> blacklistedAccountIds,
        final Collection<String> blacklistedDeviceIds,
        final Collection<String> blacklistedKeyIds,
        final String signatureAlgorithm,
        final byte[] issuerSignature,
        final long policyEpoch,
        final String policySource,
        final Collection<String> revokedCertificateIds,
        final Collection<String> revokedTransferIds,
        final Collection<AssetSendLimitV2> assetSendLimits) {
      requireNonBlank(policyId, "policyId");
      requireHexLike(policyHashHex, "policyHashHex");
      requireNonBlank(issuerId, "issuerId");
      if (issuedAtMs < 0 || expiresAtMs <= issuedAtMs) {
        throw new IllegalArgumentException("policy time range is invalid");
      }
      if (maxCertificateAgeMs <= 0 || maxPolicyAgeMs <= 0 || maxTokenAgeMs <= 0) {
        throw new IllegalArgumentException("policy time limits must be positive");
      }
      if (policyEpoch < 0L) {
        throw new IllegalArgumentException("policyEpoch must be non-negative");
      }
      requireNonBlank(policySource, "policySource");
      requireSupportedSignatureAlgorithm(signatureAlgorithm);
      final Set<String> normalizedHardware = normalizedSet(allowedHardwareClasses);
      if (normalizedHardware.isEmpty()) {
        throw new IllegalArgumentException("allowedHardwareClasses must not be empty");
      }
      final String canonicalMaxBalance = canonicalAmountString(maxOfflineBalance);
      final String canonicalMaxTx = canonicalAmountString(maxTransactionAmount);
      requirePositiveAmount(canonicalMaxBalance, "maxOfflineBalance");
      requirePositiveAmount(canonicalMaxTx, "maxTransactionAmount");
      this.policyId = policyId;
      this.policyHashHex = policyHashHex;
      this.issuerId = issuerId;
      this.issuedAtMs = issuedAtMs;
      this.expiresAtMs = expiresAtMs;
      this.maxCertificateAgeMs = maxCertificateAgeMs;
      this.maxPolicyAgeMs = maxPolicyAgeMs;
      this.maxTokenAgeMs = maxTokenAgeMs;
      this.maxOfflineBalance = canonicalMaxBalance;
      this.maxTransactionAmount = canonicalMaxTx;
      this.allowedHardwareClasses = Collections.unmodifiableSet(normalizedHardware);
      this.blacklistedAccountIds = Collections.unmodifiableSet(normalizedSet(blacklistedAccountIds));
      this.blacklistedDeviceIds = Collections.unmodifiableSet(normalizedSet(blacklistedDeviceIds));
      this.blacklistedKeyIds = Collections.unmodifiableSet(normalizedSet(blacklistedKeyIds));
      this.signatureAlgorithm = signatureAlgorithm;
      this.policyEpoch = policyEpoch;
      this.policySource = policySource;
      this.revokedCertificateIds =
          Collections.unmodifiableSet(normalizedSet(revokedCertificateIds));
      this.revokedTransferIds = Collections.unmodifiableSet(normalizedSet(revokedTransferIds));
      this.assetSendLimits =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(assetSendLimits)));
      this.issuerSignature = requireNonEmptyBytes(issuerSignature, "issuerSignature");
    }

    public PolicyBundleV2(
        final String policyId,
        final String policyHashHex,
        final String issuerId,
        final long issuedAtMs,
        final long expiresAtMs,
        final String maxOfflineBalance,
        final String maxTransactionAmount,
        final Collection<String> allowedHardwareClasses,
        final byte[] issuerSignature) {
      this(
          policyId,
          policyHashHex,
          issuerId,
          issuedAtMs,
          expiresAtMs,
          DEFAULT_MAX_CERTIFICATE_AGE_MS,
          DEFAULT_MAX_POLICY_AGE_MS,
          DEFAULT_MAX_TOKEN_AGE_MS,
          maxOfflineBalance,
          maxTransactionAmount,
          allowedHardwareClasses,
          Collections.emptySet(),
          Collections.emptySet(),
          Collections.emptySet(),
          issuerSignature);
    }

    public String policyId() {
      return policyId;
    }

    public String policyHashHex() {
      return policyHashHex;
    }

    public String issuerId() {
      return issuerId;
    }

    public long issuedAtMs() {
      return issuedAtMs;
    }

    public long expiresAtMs() {
      return expiresAtMs;
    }

    public long maxCertificateAgeMs() {
      return maxCertificateAgeMs;
    }

    public long maxPolicyAgeMs() {
      return maxPolicyAgeMs;
    }

    public long maxTokenAgeMs() {
      return maxTokenAgeMs;
    }

    public String maxOfflineBalance() {
      return maxOfflineBalance;
    }

    public String maxTransactionAmount() {
      return maxTransactionAmount;
    }

    public Set<String> allowedHardwareClasses() {
      return allowedHardwareClasses;
    }

    public Set<String> blacklistedAccountIds() {
      return blacklistedAccountIds;
    }

    public Set<String> blacklistedDeviceIds() {
      return blacklistedDeviceIds;
    }

    public Set<String> blacklistedKeyIds() {
      return blacklistedKeyIds;
    }

    public String signatureAlgorithm() {
      return signatureAlgorithm;
    }

    public long policyEpoch() {
      return policyEpoch;
    }

    public String policySource() {
      return policySource;
    }

    public Set<String> revokedCertificateIds() {
      return revokedCertificateIds;
    }

    public Set<String> revokedTransferIds() {
      return revokedTransferIds;
    }

    public List<AssetSendLimitV2> assetSendLimits() {
      return assetSendLimits;
    }

    public byte[] issuerSignature() {
      return Arrays.copyOf(issuerSignature, issuerSignature.length);
    }
  }

  public static final class AssetSendLimitV2 {
    private final String assetDefinitionId;
    private final String maxTransactionAmount;
    private final String dailySendLimit;
    private final String monthlySendLimit;

    public AssetSendLimitV2(
        final String assetDefinitionId,
        final String maxTransactionAmount,
        final String dailySendLimit,
        final String monthlySendLimit) {
      requireNonBlank(assetDefinitionId, "assetDefinitionId");
      final String canonicalMaxTransaction = canonicalAmountString(maxTransactionAmount);
      final String canonicalDaily = canonicalAmountString(dailySendLimit);
      final String canonicalMonthly = canonicalAmountString(monthlySendLimit);
      requirePositiveAmount(canonicalMaxTransaction, "maxTransactionAmount");
      requirePositiveAmount(canonicalDaily, "dailySendLimit");
      requirePositiveAmount(canonicalMonthly, "monthlySendLimit");
      this.assetDefinitionId = assetDefinitionId;
      this.maxTransactionAmount = canonicalMaxTransaction;
      this.dailySendLimit = canonicalDaily;
      this.monthlySendLimit = canonicalMonthly;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public String maxTransactionAmount() {
      return maxTransactionAmount;
    }

    public String dailySendLimit() {
      return dailySendLimit;
    }

    public String monthlySendLimit() {
      return monthlySendLimit;
    }
  }

  public static final class PurseStateV2 {
    private final String chainId;
    private final String accountId;
    private final String assetDefinitionId;
    private final String purseId;
    private final String balance;
    private final long sequence;
    private final String policyHashHex;
    private final long updatedAtMs;

    public PurseStateV2(
        final String chainId,
        final String accountId,
        final String assetDefinitionId,
        final String purseId,
        final String balance,
        final long sequence,
        final String policyHashHex,
        final long updatedAtMs) {
      requireNonBlank(chainId, "chainId");
      requireNonBlank(accountId, "accountId");
      requireNonBlank(assetDefinitionId, "assetDefinitionId");
      requireNonBlank(purseId, "purseId");
      final String canonicalBalance = canonicalAmountString(balance);
      requireNonNegativeAmount(canonicalBalance, "balance");
      requireHexLike(policyHashHex, "policyHashHex");
      if (sequence < 0) {
        throw new IllegalArgumentException("sequence must be non-negative");
      }
      this.chainId = chainId;
      this.accountId = accountId;
      this.assetDefinitionId = assetDefinitionId;
      this.purseId = purseId;
      this.balance = canonicalBalance;
      this.sequence = sequence;
      this.policyHashHex = policyHashHex;
      this.updatedAtMs = updatedAtMs;
    }

    public String chainId() {
      return chainId;
    }

    public String accountId() {
      return accountId;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public String purseId() {
      return purseId;
    }

    public String balance() {
      return balance;
    }

    public long sequence() {
      return sequence;
    }

    public String policyHashHex() {
      return policyHashHex;
    }

    public long updatedAtMs() {
      return updatedAtMs;
    }
  }

  public static final class ReceiveRequestV2 {
    public static final int VERSION = 2;

    private final int version;
    private final String chainId;
    private final String paymentRequestId;
    private final CertificateV2 recipientCertificate;
    private final String assetDefinitionId;
    private final String amount;
    private final long createdAtMs;
    private final long expiresAtMs;
    private final String policyHashHex;
    private final String signatureAlgorithm;
    private final byte[] challengeSignature;

    public ReceiveRequestV2(
        final int version,
        final String chainId,
        final String paymentRequestId,
        final CertificateV2 recipientCertificate,
        final String assetDefinitionId,
        final String amount,
        final long createdAtMs,
        final long expiresAtMs,
        final String policyHashHex,
        final byte[] challengeSignature) {
      this(
          version,
          chainId,
          paymentRequestId,
          recipientCertificate,
          assetDefinitionId,
          amount,
          createdAtMs,
          expiresAtMs,
          policyHashHex,
          recipientCertificate.signatureAlgorithm(),
          challengeSignature);
    }

    public ReceiveRequestV2(
        final int version,
        final String chainId,
        final String paymentRequestId,
        final CertificateV2 recipientCertificate,
        final String assetDefinitionId,
        final String amount,
        final long createdAtMs,
        final long expiresAtMs,
        final String policyHashHex,
        final String signatureAlgorithm,
        final byte[] challengeSignature) {
      if (version != VERSION) {
        throw new IllegalArgumentException("unsupported receive request version");
      }
      requireNonBlank(chainId, "chainId");
      requireNonBlank(paymentRequestId, "paymentRequestId");
      requireNonBlank(assetDefinitionId, "assetDefinitionId");
      final String canonicalAmount = canonicalAmountString(amount);
      requirePositiveAmount(canonicalAmount, "amount");
      if (expiresAtMs <= createdAtMs) {
        throw new IllegalArgumentException("expiresAtMs must be after createdAtMs");
      }
      requireHexLike(policyHashHex, "policyHashHex");
      requireSupportedSignatureAlgorithm(signatureAlgorithm);
      this.version = version;
      this.chainId = chainId;
      this.paymentRequestId = paymentRequestId;
      this.recipientCertificate = Objects.requireNonNull(recipientCertificate, "recipientCertificate");
      this.assetDefinitionId = assetDefinitionId;
      this.amount = canonicalAmount;
      this.createdAtMs = createdAtMs;
      this.expiresAtMs = expiresAtMs;
      this.policyHashHex = policyHashHex;
      this.signatureAlgorithm = signatureAlgorithm;
      this.challengeSignature = requireNonEmptyBytes(challengeSignature, "challengeSignature");
    }

    public int version() {
      return version;
    }

    public String chainId() {
      return chainId;
    }

    public String paymentRequestId() {
      return paymentRequestId;
    }

    public CertificateV2 recipientCertificate() {
      return recipientCertificate;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public String amount() {
      return amount;
    }

    public long createdAtMs() {
      return createdAtMs;
    }

    public long expiresAtMs() {
      return expiresAtMs;
    }

    public String policyHashHex() {
      return policyHashHex;
    }

    public String signatureAlgorithm() {
      return signatureAlgorithm;
    }

    public byte[] challengeSignature() {
      return Arrays.copyOf(challengeSignature, challengeSignature.length);
    }
  }

  public static final class DebitReceiptV2 {
    public static final int VERSION = 2;

    private final int version;
    private final String transferId;
    private final String chainId;
    private final String paymentRequestId;
    private final CertificateV2 senderCertificate;
    private final CertificateV2 recipientCertificate;
    private final String assetDefinitionId;
    private final String amount;
    private final String senderPreBalance;
    private final String senderPostBalance;
    private final long senderSequence;
    private final long createdAtMs;
    private final long expiresAtMs;
    private final String policyHashHex;
    private final byte[] receiveChallengeSignature;
    private final String signatureAlgorithm;
    private final byte[] debitSignature;

    public DebitReceiptV2(
        final int version,
        final String transferId,
        final String chainId,
        final String paymentRequestId,
        final CertificateV2 senderCertificate,
        final CertificateV2 recipientCertificate,
        final String assetDefinitionId,
        final String amount,
        final String senderPreBalance,
        final String senderPostBalance,
        final long senderSequence,
        final long createdAtMs,
        final long expiresAtMs,
        final String policyHashHex,
        final byte[] receiveChallengeSignature,
        final byte[] debitSignature) {
      this(
          version,
          transferId,
          chainId,
          paymentRequestId,
          senderCertificate,
          recipientCertificate,
          assetDefinitionId,
          amount,
          senderPreBalance,
          senderPostBalance,
          senderSequence,
          createdAtMs,
          expiresAtMs,
          policyHashHex,
          receiveChallengeSignature,
          senderCertificate.signatureAlgorithm(),
          debitSignature);
    }

    public DebitReceiptV2(
        final int version,
        final String transferId,
        final String chainId,
        final String paymentRequestId,
        final CertificateV2 senderCertificate,
        final CertificateV2 recipientCertificate,
        final String assetDefinitionId,
        final String amount,
        final String senderPreBalance,
        final String senderPostBalance,
        final long senderSequence,
        final long createdAtMs,
        final long expiresAtMs,
        final String policyHashHex,
        final byte[] receiveChallengeSignature,
        final String signatureAlgorithm,
        final byte[] debitSignature) {
      if (version != VERSION) {
        throw new IllegalArgumentException("unsupported debit receipt version");
      }
      requireNonBlank(transferId, "transferId");
      requireNonBlank(chainId, "chainId");
      requireNonBlank(paymentRequestId, "paymentRequestId");
      requireNonBlank(assetDefinitionId, "assetDefinitionId");
      final String canonicalAmount = canonicalAmountString(amount);
      final String canonicalPre = canonicalAmountString(senderPreBalance);
      final String canonicalPost = canonicalAmountString(senderPostBalance);
      requirePositiveAmount(canonicalAmount, "amount");
      requireNonNegativeAmount(canonicalPre, "senderPreBalance");
      requireNonNegativeAmount(canonicalPost, "senderPostBalance");
      if (senderSequence <= 0 || expiresAtMs <= createdAtMs) {
        throw new IllegalArgumentException("debit receipt sequence/time is invalid");
      }
      requireHexLike(policyHashHex, "policyHashHex");
      requireSupportedSignatureAlgorithm(signatureAlgorithm);
      this.version = version;
      this.transferId = transferId;
      this.chainId = chainId;
      this.paymentRequestId = paymentRequestId;
      this.senderCertificate = Objects.requireNonNull(senderCertificate, "senderCertificate");
      this.recipientCertificate =
          Objects.requireNonNull(recipientCertificate, "recipientCertificate");
      this.assetDefinitionId = assetDefinitionId;
      this.amount = canonicalAmount;
      this.senderPreBalance = canonicalPre;
      this.senderPostBalance = canonicalPost;
      this.senderSequence = senderSequence;
      this.createdAtMs = createdAtMs;
      this.expiresAtMs = expiresAtMs;
      this.policyHashHex = policyHashHex;
      this.receiveChallengeSignature =
          requireNonEmptyBytes(receiveChallengeSignature, "receiveChallengeSignature");
      this.signatureAlgorithm = signatureAlgorithm;
      this.debitSignature = requireNonEmptyBytes(debitSignature, "debitSignature");
    }

    public int version() {
      return version;
    }

    public String transferId() {
      return transferId;
    }

    public String chainId() {
      return chainId;
    }

    public String paymentRequestId() {
      return paymentRequestId;
    }

    public CertificateV2 senderCertificate() {
      return senderCertificate;
    }

    public CertificateV2 recipientCertificate() {
      return recipientCertificate;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public String amount() {
      return amount;
    }

    public String senderPreBalance() {
      return senderPreBalance;
    }

    public String senderPostBalance() {
      return senderPostBalance;
    }

    public long senderSequence() {
      return senderSequence;
    }

    public long createdAtMs() {
      return createdAtMs;
    }

    public long expiresAtMs() {
      return expiresAtMs;
    }

    public String policyHashHex() {
      return policyHashHex;
    }

    public byte[] receiveChallengeSignature() {
      return Arrays.copyOf(receiveChallengeSignature, receiveChallengeSignature.length);
    }

    public String signatureAlgorithm() {
      return signatureAlgorithm;
    }

    public byte[] debitSignature() {
      return Arrays.copyOf(debitSignature, debitSignature.length);
    }
  }

  public static final class CreditReceiptV2 {
    public static final int VERSION = 2;

    private final int version;
    private final String transferId;
    private final String chainId;
    private final CertificateV2 recipientCertificate;
    private final String amount;
    private final String recipientPreBalance;
    private final String recipientPostBalance;
    private final long recipientSequence;
    private final long acceptedAtMs;
    private final String signatureAlgorithm;
    private final byte[] creditSignature;

    public CreditReceiptV2(
        final int version,
        final String transferId,
        final String chainId,
        final CertificateV2 recipientCertificate,
        final String amount,
        final String recipientPreBalance,
        final String recipientPostBalance,
        final long recipientSequence,
        final long acceptedAtMs,
        final byte[] creditSignature) {
      this(
          version,
          transferId,
          chainId,
          recipientCertificate,
          amount,
          recipientPreBalance,
          recipientPostBalance,
          recipientSequence,
          acceptedAtMs,
          recipientCertificate.signatureAlgorithm(),
          creditSignature);
    }

    public CreditReceiptV2(
        final int version,
        final String transferId,
        final String chainId,
        final CertificateV2 recipientCertificate,
        final String amount,
        final String recipientPreBalance,
        final String recipientPostBalance,
        final long recipientSequence,
        final long acceptedAtMs,
        final String signatureAlgorithm,
        final byte[] creditSignature) {
      if (version != VERSION) {
        throw new IllegalArgumentException("unsupported credit receipt version");
      }
      requireNonBlank(transferId, "transferId");
      requireNonBlank(chainId, "chainId");
      final String canonicalAmount = canonicalAmountString(amount);
      final String canonicalPre = canonicalAmountString(recipientPreBalance);
      final String canonicalPost = canonicalAmountString(recipientPostBalance);
      requirePositiveAmount(canonicalAmount, "amount");
      requireNonNegativeAmount(canonicalPre, "recipientPreBalance");
      requireNonNegativeAmount(canonicalPost, "recipientPostBalance");
      if (recipientSequence <= 0) {
        throw new IllegalArgumentException("recipientSequence must be positive");
      }
      requireSupportedSignatureAlgorithm(signatureAlgorithm);
      this.version = version;
      this.transferId = transferId;
      this.chainId = chainId;
      this.recipientCertificate =
          Objects.requireNonNull(recipientCertificate, "recipientCertificate");
      this.amount = canonicalAmount;
      this.recipientPreBalance = canonicalPre;
      this.recipientPostBalance = canonicalPost;
      this.recipientSequence = recipientSequence;
      this.acceptedAtMs = acceptedAtMs;
      this.signatureAlgorithm = signatureAlgorithm;
      this.creditSignature = requireNonEmptyBytes(creditSignature, "creditSignature");
    }

    public int version() {
      return version;
    }

    public String transferId() {
      return transferId;
    }

    public String chainId() {
      return chainId;
    }

    public CertificateV2 recipientCertificate() {
      return recipientCertificate;
    }

    public String amount() {
      return amount;
    }

    public String recipientPreBalance() {
      return recipientPreBalance;
    }

    public String recipientPostBalance() {
      return recipientPostBalance;
    }

    public long recipientSequence() {
      return recipientSequence;
    }

    public long acceptedAtMs() {
      return acceptedAtMs;
    }

    public String signatureAlgorithm() {
      return signatureAlgorithm;
    }

    public byte[] creditSignature() {
      return Arrays.copyOf(creditSignature, creditSignature.length);
    }
  }

  public static final class SettlementBatchV2 {
    public static final int VERSION = 2;

    private final int version;
    private final String chainId;
    private final String purseId;
    private final List<DebitReceiptV2> debitReceipts;
    private final List<CreditReceiptV2> creditReceipts;

    public SettlementBatchV2(
        final int version,
        final String chainId,
        final String purseId,
        final List<DebitReceiptV2> debitReceipts,
        final List<CreditReceiptV2> creditReceipts) {
      if (version != VERSION) {
        throw new IllegalArgumentException("unsupported settlement batch version");
      }
      requireNonBlank(chainId, "chainId");
      requireNonBlank(purseId, "purseId");
      this.version = version;
      this.chainId = chainId;
      this.purseId = purseId;
      this.debitReceipts =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(debitReceipts)));
      this.creditReceipts =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(creditReceipts)));
    }

    public int version() {
      return version;
    }

    public String chainId() {
      return chainId;
    }

    public String purseId() {
      return purseId;
    }

    public List<DebitReceiptV2> debitReceipts() {
      return debitReceipts;
    }

    public List<CreditReceiptV2> creditReceipts() {
      return creditReceipts;
    }
  }

  private static Set<String> normalizedSet(final Collection<String> values) {
    final Set<String> set = new LinkedHashSet<>();
    if (values == null) {
      return set;
    }
    for (final String value : values) {
      if (value != null) {
        final String trimmed = value.trim();
        if (!trimmed.isEmpty()) {
          set.add(trimmed);
        }
      }
    }
    return set;
  }

  private static void requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
  }

  private static void requireHexLike(final String value, final String field) {
    requireNonBlank(value, field);
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException(field + " must have an even number of hex characters");
    }
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      final boolean hex =
          (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F');
      if (!hex) {
        throw new IllegalArgumentException(field + " must be hex");
      }
    }
  }

  private static byte[] requireNonEmptyBytes(final byte[] bytes, final String field) {
    if (bytes == null || bytes.length == 0) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    return Arrays.copyOf(bytes, bytes.length);
  }

  private static void requireSupportedSignatureAlgorithm(final String value) {
    if (!SIGNATURE_ALGORITHM_ED25519.equals(value)
        && !SIGNATURE_ALGORITHM_ECDSA_P256_SHA256.equals(value)) {
      throw new IllegalArgumentException("unsupported Offline Bearer signature algorithm");
    }
  }

  private static void requireSupportedPublicKeyEncoding(final String value) {
    if (!PUBLIC_KEY_ENCODING_RAW_ED25519.equals(value)
        && !PUBLIC_KEY_ENCODING_X963_P256.equals(value)) {
      throw new IllegalArgumentException("unsupported Offline Bearer public key encoding");
    }
  }

  private static void requirePositiveAmount(final String value, final String field) {
    if (decimal(value).signum() <= 0) {
      throw new IllegalArgumentException(field + " must be positive");
    }
  }

  private static void requireNonNegativeAmount(final String value, final String field) {
    if (decimal(value).signum() < 0) {
      throw new IllegalArgumentException(field + " must be non-negative");
    }
  }

  private static void requireAmountAtMost(
      final String value, final String max, final String message) {
    if (decimal(value).compareTo(decimal(max)) > 0) {
      throw new PolicyException(message);
    }
  }

  private static BigDecimal decimal(final String value) {
    try {
      return new BigDecimal(value.trim());
    } catch (final RuntimeException ex) {
      throw new IllegalArgumentException("invalid offline bearer amount: " + value, ex);
    }
  }

  private static String canonicalAmountString(final String value) {
    BigDecimal normalized = decimal(value).stripTrailingZeros();
    if (normalized.scale() < 0) {
      normalized = normalized.setScale(0);
    }
    return normalized.toPlainString();
  }

  private static long safeAdd(final long lhs, final long rhs) {
    if (rhs <= 0) {
      throw new IllegalArgumentException("duration must be positive");
    }
    final long maxDelta = Long.MAX_VALUE - lhs;
    return rhs > maxDelta ? Long.MAX_VALUE : lhs + rhs;
  }

  private static boolean equalsIgnoreCase(final String lhs, final String rhs) {
    return lhs.equalsIgnoreCase(rhs);
  }
}
