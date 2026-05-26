package org.hyperledger.iroha.android.offline;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.function.LongSupplier;

/** Hardware-backed Offline Bearer purse facade for real offline value transfer. */
public final class OfflineBearerWallet {
  private final String chainId;
  private final String accountId;
  private final SecureElement secureElement;
  private final PolicyProvider policyProvider;
  private final OfflineNoteIdGenerator idGenerator;
  private final LongSupplier clock;

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
        System::currentTimeMillis);
  }

  public OfflineBearerWallet(
      final String chainId,
      final String accountId,
      final SecureElement secureElement,
      final PolicyProvider policyProvider,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    requireNonBlank(chainId, "chainId");
    requireNonBlank(accountId, "accountId");
    this.chainId = chainId;
    this.accountId = accountId;
    this.secureElement = Objects.requireNonNull(secureElement, "secureElement");
    this.policyProvider = Objects.requireNonNull(policyProvider, "policyProvider");
    this.idGenerator = Objects.requireNonNull(idGenerator, "idGenerator");
    this.clock = Objects.requireNonNull(clock, "clock");
  }

  public PurseStateV2 currentState() {
    return secureElement.currentState();
  }

  public void installLoadedPurse(final CertificateV2 certificate, final PurseStateV2 state) {
    final PolicyBundleV2 policy = policyProvider.currentPolicy();
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
    final PolicyBundleV2 policy = policyProvider.currentPolicy();
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
    return secureElement.createReceiveRequest(
        idGenerator.nextId("offline-bearer-request"),
        canonicalAmount,
        now,
        safeAdd(now, Math.min(ttlMs, policy.maxTokenAgeMs())),
        policy.policyHashHex());
  }

  public DebitReceiptV2 pay(final ReceiveRequestV2 request) {
    return pay(request, defaultTokenTtl());
  }

  public DebitReceiptV2 pay(final ReceiveRequestV2 request, final long ttlMs) {
    final PolicyBundleV2 policy = policyProvider.currentPolicy();
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
    return secureElement.debit(
        request,
        idGenerator.nextId("offline-bearer-transfer"),
        now,
        safeAdd(now, Math.min(ttlMs, policy.maxTokenAgeMs())));
  }

  public CreditReceiptV2 accept(final DebitReceiptV2 receipt) {
    final PolicyBundleV2 policy = policyProvider.currentPolicy();
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
    return secureElement.credit(receipt, now);
  }

  public SettlementBatchV2 exportSettlementBatch() {
    return exportSettlementBatch(256);
  }

  public SettlementBatchV2 exportSettlementBatch(final int maxReceipts) {
    requireHardwareUsable(policyProvider.currentPolicy());
    if (maxReceipts <= 0) {
      throw new IllegalArgumentException("maxReceipts must be positive");
    }
    return secureElement.exportSettlementBatch(maxReceipts);
  }

  public void pruneSettled(final Collection<String> transferIds) {
    requireHardwareUsable(policyProvider.currentPolicy());
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
    if (!policy.allowedHardwareClasses().contains(capabilities.hardwareClass())) {
      throw new PolicyException("hardware class is not allowed by current Offline Bearer policy");
    }
  }

  private static void enforceCertificatePolicy(
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
    return policyProvider.currentPolicy().maxTokenAgeMs();
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

  public static final class SecureElementCapabilities {
    private final boolean hardwareBacked;
    private final boolean statefulPurse;
    private final String hardwareClass;
    private final String attestationKeyId;

    public SecureElementCapabilities(
        final boolean hardwareBacked,
        final boolean statefulPurse,
        final String hardwareClass,
        final String attestationKeyId) {
      requireNonBlank(hardwareClass, "hardwareClass");
      this.hardwareBacked = hardwareBacked;
      this.statefulPurse = statefulPurse;
      this.hardwareClass = hardwareClass;
      this.attestationKeyId = attestationKeyId;
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
      requireNonBlank(certificateId, "certificateId");
      requireNonBlank(chainId, "chainId");
      requireNonBlank(issuerId, "issuerId");
      requireNonBlank(purseId, "purseId");
      requireNonBlank(accountId, "accountId");
      requireNonBlank(assetDefinitionId, "assetDefinitionId");
      requireNonBlank(deviceId, "deviceId");
      requireNonBlank(keyId, "keyId");
      requireNonBlank(hardwareClass, "hardwareClass");
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
      this.version = version;
      this.chainId = chainId;
      this.paymentRequestId = paymentRequestId;
      this.recipientCertificate = Objects.requireNonNull(recipientCertificate, "recipientCertificate");
      this.assetDefinitionId = assetDefinitionId;
      this.amount = canonicalAmount;
      this.createdAtMs = createdAtMs;
      this.expiresAtMs = expiresAtMs;
      this.policyHashHex = policyHashHex;
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
