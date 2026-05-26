package org.hyperledger.iroha.android.offline;

import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicLong;

public final class OfflineBearerWalletTest {
  private static final String CHAIN = "test-chain";
  private static final String ASSET = "rupee#india";
  private static final String ISSUER = "offline-issuer";
  private static final String HARDWARE_CLASS = "test-stateful-secure-element";
  private static final String POLICY_HASH = "00112233445566778899aabbccddeeff";
  private static final long NOW = 1_700_000_000_000L;

  private OfflineBearerWalletTest() {}

  public static void main(final String[] args) {
    statefulBearerPurseSupportsPartialSpendAndRespendingWithoutTrailGrowth();
    unsupportedHardwareDisablesOfflineValue();
    hardwareWithoutAttestationKeyDisablesOfflineValue();
    policyRejectsOldCertificatesAndBlacklistedAccounts();
    expiredReceiveRequestIsRejectedBeforeDebit();
    incomingCreditCannotExceedPolicyMaxBalance();
    System.out.println("[IrohaAndroid] OfflineBearerWalletTest passed.");
  }

  private static void statefulBearerPurseSupportsPartialSpendAndRespendingWithoutTrailGrowth() {
    final AtomicLong clock = new AtomicLong(NOW);
    final OfflineBearerWallet.PolicyBundleV2 policy = policy();
    final TestStatefulSecureElement senderElement = new TestStatefulSecureElement("sender-purse");
    final TestStatefulSecureElement recipientElement =
        new TestStatefulSecureElement("recipient-purse");
    final TestStatefulSecureElement thirdElement = new TestStatefulSecureElement("third-purse");
    final OfflineBearerWallet sender = wallet("alice", senderElement, policy, clock);
    final OfflineBearerWallet recipient = wallet("bob", recipientElement, policy, clock);
    final OfflineBearerWallet third = wallet("carol", thirdElement, policy, clock);

    sender.installLoadedPurse(
        certificate("alice", "sender-purse"), state("alice", "sender-purse", "50"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse"), state("bob", "recipient-purse", "0"));
    third.installLoadedPurse(
        certificate("carol", "third-purse"), state("carol", "third-purse", "0"));

    final OfflineBearerWallet.ReceiveRequestV2 requestTwoRupees =
        recipient.prepareReceive(ASSET, "2");
    final OfflineBearerWallet.DebitReceiptV2 debitTwoRupees = sender.pay(requestTwoRupees);
    final OfflineBearerWallet.CreditReceiptV2 creditTwoRupees =
        recipient.accept(debitTwoRupees);

    assertEquals("50", debitTwoRupees.senderPreBalance(), "sender pre-balance");
    assertEquals("48", debitTwoRupees.senderPostBalance(), "sender post-balance");
    assertEquals("0", creditTwoRupees.recipientPreBalance(), "recipient pre-balance");
    assertEquals("2", creditTwoRupees.recipientPostBalance(), "recipient post-balance");
    assertEquals("48", sender.currentState().balance(), "sender state");
    assertEquals("2", recipient.currentState().balance(), "recipient state");

    clock.addAndGet(1_000L);
    final OfflineBearerWallet.ReceiveRequestV2 requestOneRupee = third.prepareReceive(ASSET, "1");
    final OfflineBearerWallet.DebitReceiptV2 debitOneRupee = recipient.pay(requestOneRupee);
    final OfflineBearerWallet.CreditReceiptV2 creditOneRupee = third.accept(debitOneRupee);

    assertEquals("2", debitOneRupee.senderPreBalance(), "respend sender pre-balance");
    assertEquals("1", debitOneRupee.senderPostBalance(), "respend sender post-balance");
    assertEquals("0", creditOneRupee.recipientPreBalance(), "respend recipient pre-balance");
    assertEquals("1", creditOneRupee.recipientPostBalance(), "respend recipient post-balance");
    assertEquals("1", recipient.currentState().balance(), "respend recipient state");
    assertEquals("1", third.currentState().balance(), "third state");
    assertEquals(1, sender.exportSettlementBatch().debitReceipts().size(), "sender debits");
    assertEquals(1, recipient.exportSettlementBatch().debitReceipts().size(), "recipient debits");
    assertEquals(1, recipient.exportSettlementBatch().creditReceipts().size(), "recipient credits");
  }

  private static void unsupportedHardwareDisablesOfflineValue() {
    final OfflineBearerWallet wallet =
        new OfflineBearerWallet(
            CHAIN,
            "alice",
            new OfflineBearerWallet.UnsupportedSecureElement(),
            new OfflineBearerWallet.StaticPolicyProvider(policy()));

    expectThrows(
        OfflineBearerWallet.PolicyException.class, () -> wallet.prepareReceive(ASSET, "1"));
  }

  private static void hardwareWithoutAttestationKeyDisablesOfflineValue() {
    final AtomicLong clock = new AtomicLong(NOW);
    final TestStatefulSecureElement element = new TestStatefulSecureElement("weak-purse", null);
    final OfflineBearerWallet wallet = wallet("alice", element, policy(), clock);

    expectThrows(
        OfflineBearerWallet.PolicyException.class,
        () ->
            wallet.installLoadedPurse(
                certificate("alice", "weak-purse"), state("alice", "weak-purse", "1")));
  }

  private static void policyRejectsOldCertificatesAndBlacklistedAccounts() {
    final AtomicLong clock = new AtomicLong(NOW);
    final OfflineBearerWallet.PolicyBundleV2 oldCertificatePolicy =
        policy(1_000L, OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_TOKEN_AGE_MS, "100",
            Collections.emptySet());
    final TestStatefulSecureElement oldElement = new TestStatefulSecureElement("old-purse");
    final OfflineBearerWallet oldWallet = wallet("alice", oldElement, oldCertificatePolicy, clock);

    expectThrows(
        OfflineBearerWallet.PolicyException.class,
        () ->
            oldWallet.installLoadedPurse(
                certificate("alice", "old-purse", NOW - 10_000L),
                state("alice", "old-purse", "1")));

    final OfflineBearerWallet.PolicyBundleV2 blacklistPolicy =
        policy(
            OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_CERTIFICATE_AGE_MS,
            OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_TOKEN_AGE_MS,
            "100",
            Collections.singleton("bob"));
    final TestStatefulSecureElement blacklistedElement =
        new TestStatefulSecureElement("bob-purse");
    final OfflineBearerWallet blacklistedWallet =
        wallet("bob", blacklistedElement, blacklistPolicy, clock);

    expectThrows(
        OfflineBearerWallet.PolicyException.class,
        () ->
            blacklistedWallet.installLoadedPurse(
                certificate("bob", "bob-purse"), state("bob", "bob-purse", "1")));
  }

  private static void expiredReceiveRequestIsRejectedBeforeDebit() {
    final AtomicLong clock = new AtomicLong(NOW);
    final OfflineBearerWallet.PolicyBundleV2 policy =
        policy(
            OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_CERTIFICATE_AGE_MS,
            1_000L,
            "100",
            Collections.emptySet());
    final TestStatefulSecureElement senderElement = new TestStatefulSecureElement("sender-purse");
    final TestStatefulSecureElement recipientElement =
        new TestStatefulSecureElement("recipient-purse");
    final OfflineBearerWallet sender = wallet("alice", senderElement, policy, clock);
    final OfflineBearerWallet recipient = wallet("bob", recipientElement, policy, clock);
    sender.installLoadedPurse(
        certificate("alice", "sender-purse"), state("alice", "sender-purse", "5"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse"), state("bob", "recipient-purse", "0"));

    final OfflineBearerWallet.ReceiveRequestV2 request =
        recipient.prepareReceive(ASSET, "1", 1_000L);
    clock.addAndGet(1_001L);

    expectThrows(IllegalArgumentException.class, () -> sender.pay(request));
    assertEquals("5", sender.currentState().balance(), "sender balance after expired request");
  }

  private static void incomingCreditCannotExceedPolicyMaxBalance() {
    final AtomicLong clock = new AtomicLong(NOW);
    final OfflineBearerWallet.PolicyBundleV2 policy =
        policy(
            OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_CERTIFICATE_AGE_MS,
            OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_TOKEN_AGE_MS,
            "2",
            Collections.emptySet());
    final TestStatefulSecureElement senderElement = new TestStatefulSecureElement("sender-purse");
    final TestStatefulSecureElement recipientElement =
        new TestStatefulSecureElement("recipient-purse");
    final OfflineBearerWallet sender = wallet("alice", senderElement, policy, clock);
    final OfflineBearerWallet recipient = wallet("bob", recipientElement, policy, clock);
    sender.installLoadedPurse(
        certificate("alice", "sender-purse"), state("alice", "sender-purse", "2"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse"), state("bob", "recipient-purse", "2"));

    final OfflineBearerWallet.ReceiveRequestV2 request = recipient.prepareReceive(ASSET, "1");
    final OfflineBearerWallet.DebitReceiptV2 receipt = sender.pay(request);

    expectThrows(OfflineBearerWallet.PolicyException.class, () -> recipient.accept(receipt));
    assertEquals("2", recipient.currentState().balance(), "recipient balance after rejected credit");
  }

  private static OfflineBearerWallet wallet(
      final String accountId,
      final TestStatefulSecureElement secureElement,
      final OfflineBearerWallet.PolicyBundleV2 policy,
      final AtomicLong clock) {
    return new OfflineBearerWallet(
        CHAIN,
        accountId,
        secureElement,
        new OfflineBearerWallet.StaticPolicyProvider(policy),
        new OfflineNoteIdGenerator() {
          private int next = 0;

          @Override
          public String nextId(final String prefix) {
            next += 1;
            return prefix + "-" + accountId + "-" + next;
          }
        },
        clock::get);
  }

  private static OfflineBearerWallet.PolicyBundleV2 policy() {
    return policy(
        OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_CERTIFICATE_AGE_MS,
        OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_TOKEN_AGE_MS,
        "100",
        Collections.emptySet());
  }

  private static OfflineBearerWallet.PolicyBundleV2 policy(
      final long maxCertificateAgeMs,
      final long maxTokenAgeMs,
      final String maxOfflineBalance,
      final Collection<String> blacklistedAccountIds) {
    return new OfflineBearerWallet.PolicyBundleV2(
        "policy-1",
        POLICY_HASH,
        ISSUER,
        NOW - 1_000L,
        NOW + 60L * 60L * 1_000L,
        maxCertificateAgeMs,
        OfflineBearerWallet.PolicyBundleV2.DEFAULT_MAX_POLICY_AGE_MS,
        maxTokenAgeMs,
        maxOfflineBalance,
        "10",
        Collections.singletonList(HARDWARE_CLASS),
        blacklistedAccountIds,
        Collections.emptySet(),
        Collections.emptySet(),
        new byte[] {9});
  }

  private static OfflineBearerWallet.CertificateV2 certificate(
      final String accountId, final String purseId) {
    return certificate(accountId, purseId, NOW - 1_000L);
  }

  private static OfflineBearerWallet.CertificateV2 certificate(
      final String accountId, final String purseId, final long issuedAtMs) {
    return new OfflineBearerWallet.CertificateV2(
        "cert-" + purseId,
        CHAIN,
        ISSUER,
        purseId,
        accountId,
        ASSET,
        "device-" + purseId,
        "key-" + purseId,
        HARDWARE_CLASS,
        signature("pub:" + purseId),
        issuedAtMs,
        NOW + 60L * 60L * 1_000L,
        "policy-1",
        POLICY_HASH,
        new byte[] {1, 2, 3});
  }

  private static OfflineBearerWallet.PurseStateV2 state(
      final String accountId, final String purseId, final String balance) {
    return new OfflineBearerWallet.PurseStateV2(
        CHAIN, accountId, ASSET, purseId, balance, 0L, POLICY_HASH, NOW);
  }

  private static final class TestStatefulSecureElement
      implements OfflineBearerWallet.SecureElement {
    private final String purseId;
    private final String attestationKeyId;
    private OfflineBearerWallet.CertificateV2 certificate;
    private OfflineBearerWallet.PurseStateV2 state;
    private final List<OfflineBearerWallet.DebitReceiptV2> debits = new ArrayList<>();
    private final List<OfflineBearerWallet.CreditReceiptV2> credits = new ArrayList<>();

    private TestStatefulSecureElement(final String purseId) {
      this(purseId, "attestation-" + purseId);
    }

    private TestStatefulSecureElement(final String purseId, final String attestationKeyId) {
      this.purseId = purseId;
      this.attestationKeyId = attestationKeyId;
    }

    @Override
    public OfflineBearerWallet.SecureElementCapabilities capabilities() {
      return new OfflineBearerWallet.SecureElementCapabilities(
          true, true, HARDWARE_CLASS, attestationKeyId);
    }

    @Override
    public OfflineBearerWallet.CertificateV2 currentCertificate() {
      return certificate;
    }

    @Override
    public OfflineBearerWallet.PurseStateV2 currentState() {
      return state;
    }

    @Override
    public void installPurse(
        final OfflineBearerWallet.CertificateV2 certificate,
        final OfflineBearerWallet.PurseStateV2 state) {
      this.certificate = certificate;
      this.state = state;
    }

    @Override
    public OfflineBearerWallet.ReceiveRequestV2 createReceiveRequest(
        final String paymentRequestId,
        final String amount,
        final long createdAtMs,
        final long expiresAtMs,
        final String policyHashHex) {
      return new OfflineBearerWallet.ReceiveRequestV2(
          OfflineBearerWallet.ReceiveRequestV2.VERSION,
          state.chainId(),
          paymentRequestId,
          certificate,
          state.assetDefinitionId(),
          amount,
          createdAtMs,
          expiresAtMs,
          policyHashHex,
          signature("receive:" + paymentRequestId + ":" + amount + ":" + state.sequence()));
    }

    @Override
    public OfflineBearerWallet.DebitReceiptV2 debit(
        final OfflineBearerWallet.ReceiveRequestV2 request,
        final String transferId,
        final long createdAtMs,
        final long expiresAtMs) {
      final BigDecimal pre = decimal(state.balance());
      final BigDecimal amount = decimal(request.amount());
      if (pre.compareTo(amount) < 0) {
        throw new IllegalArgumentException("insufficient Offline Bearer balance");
      }
      final String post = canonical(pre.subtract(amount));
      final long nextSequence = state.sequence() + 1L;
      final OfflineBearerWallet.PurseStateV2 previous = state;
      state =
          new OfflineBearerWallet.PurseStateV2(
              previous.chainId(),
              previous.accountId(),
              previous.assetDefinitionId(),
              previous.purseId(),
              post,
              nextSequence,
              previous.policyHashHex(),
              createdAtMs);
      final OfflineBearerWallet.DebitReceiptV2 receipt =
          new OfflineBearerWallet.DebitReceiptV2(
              OfflineBearerWallet.DebitReceiptV2.VERSION,
              transferId,
              request.chainId(),
              request.paymentRequestId(),
              certificate,
              request.recipientCertificate(),
              request.assetDefinitionId(),
              request.amount(),
              previous.balance(),
              post,
              nextSequence,
              createdAtMs,
              expiresAtMs,
              request.policyHashHex(),
              request.challengeSignature(),
              signature(
                  "debit:"
                      + transferId
                      + ":"
                      + previous.balance()
                      + ":"
                      + post
                      + ":"
                      + nextSequence));
      debits.add(receipt);
      return receipt;
    }

    @Override
    public OfflineBearerWallet.CreditReceiptV2 credit(
        final OfflineBearerWallet.DebitReceiptV2 receipt, final long acceptedAtMs) {
      final String post = canonical(decimal(state.balance()).add(decimal(receipt.amount())));
      final long nextSequence = state.sequence() + 1L;
      final OfflineBearerWallet.PurseStateV2 previous = state;
      state =
          new OfflineBearerWallet.PurseStateV2(
              previous.chainId(),
              previous.accountId(),
              previous.assetDefinitionId(),
              previous.purseId(),
              post,
              nextSequence,
              previous.policyHashHex(),
              acceptedAtMs);
      final OfflineBearerWallet.CreditReceiptV2 credit =
          new OfflineBearerWallet.CreditReceiptV2(
              OfflineBearerWallet.CreditReceiptV2.VERSION,
              receipt.transferId(),
              receipt.chainId(),
              certificate,
              receipt.amount(),
              previous.balance(),
              post,
              nextSequence,
              acceptedAtMs,
              signature(
                  "credit:"
                      + receipt.transferId()
                      + ":"
                      + previous.balance()
                      + ":"
                      + post
                      + ":"
                      + nextSequence));
      credits.add(credit);
      return credit;
    }

    @Override
    public OfflineBearerWallet.SettlementBatchV2 exportSettlementBatch(final int maxReceipts) {
      return new OfflineBearerWallet.SettlementBatchV2(
          OfflineBearerWallet.SettlementBatchV2.VERSION,
          state.chainId(),
          state.purseId(),
          new ArrayList<>(debits.subList(0, Math.min(maxReceipts, debits.size()))),
          new ArrayList<>(credits.subList(0, Math.min(maxReceipts, credits.size()))));
    }

    @Override
    public void pruneSettled(final Collection<String> transferIds) {
      final Set<String> ids = new HashSet<>(transferIds);
      debits.removeIf(receipt -> ids.contains(receipt.transferId()));
      credits.removeIf(receipt -> ids.contains(receipt.transferId()));
    }
  }

  private static byte[] signature(final String value) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(value.getBytes(StandardCharsets.UTF_8));
    } catch (final Exception ex) {
      throw new IllegalStateException("SHA-256 is unavailable", ex);
    }
  }

  private static BigDecimal decimal(final String value) {
    return new BigDecimal(value);
  }

  private static String canonical(final BigDecimal value) {
    BigDecimal normalized = value.stripTrailingZeros();
    if (normalized.scale() < 0) {
      normalized = normalized.setScale(0);
    }
    return normalized.toPlainString();
  }

  private static void expectThrows(
      final Class<? extends Throwable> expected, final ThrowingRunnable runnable) {
    try {
      runnable.run();
    } catch (final Throwable ex) {
      if (expected.isInstance(ex)) {
        return;
      }
      throw new AssertionError("expected " + expected.getName() + " but got " + ex, ex);
    }
    throw new AssertionError("expected " + expected.getName());
  }

  private static void assertEquals(final String expected, final String actual, final String label) {
    if (!expected.equals(actual)) {
      throw new AssertionError(label + ": expected " + expected + " but got " + actual);
    }
  }

  private static void assertEquals(final int expected, final int actual, final String label) {
    if (expected != actual) {
      throw new AssertionError(label + ": expected " + expected + " but got " + actual);
    }
  }

  @FunctionalInterface
  private interface ThrowingRunnable {
    void run() throws Throwable;
  }
}
