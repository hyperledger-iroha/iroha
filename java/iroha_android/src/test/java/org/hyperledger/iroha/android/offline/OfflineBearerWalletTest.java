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
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.atomic.AtomicLong;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;

public final class OfflineBearerWalletTest {
  private static final String CHAIN = "test-chain";
  private static final String ASSET = "rupee#india";
  private static final String ISSUER = "offline-issuer";
  private static final String HARDWARE_CLASS = "test-stateful-secure-element";
  private static final String POLICY_HASH = "00112233445566778899aabbccddeeff";
  private static final long NOW = 1_700_000_000_000L;
  private static final TestEd25519Keypair ISSUER_KEYPAIR = new TestEd25519Keypair("issuer");

  private OfflineBearerWalletTest() {}

  public static void main(final String[] args) {
    statefulBearerPurseSupportsPartialSpendAndRespendingWithoutTrailGrowth();
    unsupportedHardwareDisablesOfflineValue();
    hardwareWithoutAttestationKeyDisablesOfflineValue();
    policyRejectsOldCertificatesAndBlacklistedAccounts();
    expiredReceiveRequestIsRejectedBeforeDebit();
    incomingCreditCannotExceedPolicyMaxBalance();
    tamperedDebitReceiptSignatureIsRejectedBeforeCredit();
    settlementBatchVerifierAcceptsExportsAndRejectsInvalidBalanceTransitions();
    bearerNoritoAndTextCodecsRoundTripCanonicalPayloadsAndRejectOfflineNotePrefixes();
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
        certificate("alice", "sender-purse", senderElement.publicKey()),
        state("alice", "sender-purse", "50"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse", recipientElement.publicKey()),
        state("bob", "recipient-purse", "0"));
    third.installLoadedPurse(
        certificate("carol", "third-purse", thirdElement.publicKey()),
        state("carol", "third-purse", "0"));

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
    assertEquals(2, recipient.exportSettlementBatch().debitReceipts().size(), "recipient debits");
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
                certificate("alice", "weak-purse", element.publicKey()),
                state("alice", "weak-purse", "1")));
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
                certificate("alice", "old-purse", oldElement.publicKey(), NOW - 10_000L),
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
                certificate("bob", "bob-purse", blacklistedElement.publicKey()),
                state("bob", "bob-purse", "1")));
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
        certificate("alice", "sender-purse", senderElement.publicKey()),
        state("alice", "sender-purse", "5"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse", recipientElement.publicKey()),
        state("bob", "recipient-purse", "0"));

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
        certificate("alice", "sender-purse", senderElement.publicKey()),
        state("alice", "sender-purse", "2"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse", recipientElement.publicKey()),
        state("bob", "recipient-purse", "2"));

    final OfflineBearerWallet.ReceiveRequestV2 request = recipient.prepareReceive(ASSET, "1");
    final OfflineBearerWallet.DebitReceiptV2 receipt = sender.pay(request);

    expectThrows(OfflineBearerWallet.PolicyException.class, () -> recipient.accept(receipt));
    assertEquals("2", recipient.currentState().balance(), "recipient balance after rejected credit");
  }

  private static void tamperedDebitReceiptSignatureIsRejectedBeforeCredit() {
    final AtomicLong clock = new AtomicLong(NOW);
    final OfflineBearerWallet.PolicyBundleV2 policy = policy();
    final TestStatefulSecureElement senderElement = new TestStatefulSecureElement("sender-purse");
    final TestStatefulSecureElement recipientElement =
        new TestStatefulSecureElement("recipient-purse");
    final OfflineBearerWallet sender = wallet("alice", senderElement, policy, clock);
    final OfflineBearerWallet recipient = wallet("bob", recipientElement, policy, clock);
    sender.installLoadedPurse(
        certificate("alice", "sender-purse", senderElement.publicKey()),
        state("alice", "sender-purse", "5"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse", recipientElement.publicKey()),
        state("bob", "recipient-purse", "0"));

    final OfflineBearerWallet.ReceiveRequestV2 request = recipient.prepareReceive(ASSET, "1");
    final OfflineBearerWallet.DebitReceiptV2 receipt = sender.pay(request);
    final OfflineBearerWallet.DebitReceiptV2 tampered =
        new OfflineBearerWallet.DebitReceiptV2(
            receipt.version(),
            receipt.transferId(),
            receipt.chainId(),
            receipt.paymentRequestId(),
            receipt.senderCertificate(),
            receipt.recipientCertificate(),
            receipt.assetDefinitionId(),
            "2",
            receipt.senderPreBalance(),
            receipt.senderPostBalance(),
            receipt.senderSequence(),
            receipt.createdAtMs(),
            receipt.expiresAtMs(),
            receipt.policyHashHex(),
            receipt.receiveChallengeSignature(),
            receipt.debitSignature());

    expectThrows(OfflineBearerWallet.PolicyException.class, () -> recipient.accept(tampered));
    assertEquals("0", recipient.currentState().balance(), "recipient balance after tamper");
  }

  private static void settlementBatchVerifierAcceptsExportsAndRejectsInvalidBalanceTransitions() {
    final AtomicLong clock = new AtomicLong(NOW);
    final OfflineBearerWallet.PolicyBundleV2 policy = policy();
    final TestStatefulSecureElement senderElement = new TestStatefulSecureElement("sender-purse");
    final TestStatefulSecureElement recipientElement =
        new TestStatefulSecureElement("recipient-purse");
    final OfflineBearerWallet sender = wallet("alice", senderElement, policy, clock);
    final OfflineBearerWallet recipient = wallet("bob", recipientElement, policy, clock);
    final OfflineBearerWallet.SignatureVerifier verifier =
        new OfflineBearerWallet.SignatureVerifier(
            Collections.singletonList(ISSUER_KEYPAIR.publicKey()));
    sender.installLoadedPurse(
        certificate("alice", "sender-purse", senderElement.publicKey()),
        state("alice", "sender-purse", "5"));
    recipient.installLoadedPurse(
        certificate("bob", "recipient-purse", recipientElement.publicKey()),
        state("bob", "recipient-purse", "0"));

    final OfflineBearerWallet.ReceiveRequestV2 request = recipient.prepareReceive(ASSET, "1");
    final OfflineBearerWallet.DebitReceiptV2 debit = sender.pay(request);
    recipient.accept(debit);

    OfflineBearerWallet.SettlementBatchVerifier.verify(
        sender.exportSettlementBatch(), policy, verifier, clock.get());
    OfflineBearerWallet.SettlementBatchVerifier.verify(
        recipient.exportSettlementBatch(), policy, verifier, clock.get());

    final OfflineBearerWallet.DebitReceiptV2 tamperedDebit =
        new OfflineBearerWallet.DebitReceiptV2(
            debit.version(),
            debit.transferId(),
            debit.chainId(),
            debit.paymentRequestId(),
            debit.senderCertificate(),
            debit.recipientCertificate(),
            debit.assetDefinitionId(),
            debit.amount(),
            debit.senderPreBalance(),
            "5",
            debit.senderSequence(),
            debit.createdAtMs(),
            debit.expiresAtMs(),
            debit.policyHashHex(),
            debit.receiveChallengeSignature(),
            debit.debitSignature());
    final OfflineBearerWallet.SettlementBatchV2 tamperedBatch =
        new OfflineBearerWallet.SettlementBatchV2(
            OfflineBearerWallet.SettlementBatchV2.VERSION,
            CHAIN,
            "sender-purse",
            Collections.singletonList(tamperedDebit),
            Collections.emptyList());

    expectThrows(
        OfflineBearerWallet.PolicyException.class,
        () -> OfflineBearerWallet.SettlementBatchVerifier.verify(
            tamperedBatch, policy, verifier, clock.get()));
  }

  private static void bearerNoritoAndTextCodecsRoundTripCanonicalPayloadsAndRejectOfflineNotePrefixes() {
    final AtomicLong clock = new AtomicLong(NOW);
    final OfflineBearerWallet.PolicyBundleV2 policy = policy();
    final TestStatefulSecureElement senderElement = new TestStatefulSecureElement("sender-purse");
    final TestStatefulSecureElement recipientElement =
        new TestStatefulSecureElement("recipient-purse");
    final OfflineBearerWallet sender = wallet("alice", senderElement, policy, clock);
    final OfflineBearerWallet recipient = wallet("bob", recipientElement, policy, clock);
    final OfflineBearerWallet.CertificateV2 senderCertificate =
        certificate("alice", "sender-purse", senderElement.publicKey());
    final OfflineBearerWallet.CertificateV2 recipientCertificate =
        certificate("bob", "recipient-purse", recipientElement.publicKey());
    sender.installLoadedPurse(senderCertificate, state("alice", "sender-purse", "5"));
    recipient.installLoadedPurse(recipientCertificate, state("bob", "recipient-purse", "0"));

    final OfflineBearerWallet.ReceiveRequestV2 request = recipient.prepareReceive(ASSET, "1");
    final OfflineBearerWallet.DebitReceiptV2 debit = sender.pay(request);
    final OfflineBearerWallet.CreditReceiptV2 credit = recipient.accept(debit);
    final OfflineBearerWallet.SettlementBatchV2 settlement = recipient.exportSettlementBatch();

    assertPolicyEquals(
        policy,
        OfflineBearerV2TextCodec.decodePolicyBundleNorito(
            OfflineBearerV2TextCodec.encodePolicyBundleNorito(policy)));
    assertCertificateEquals(
        senderCertificate,
        OfflineBearerV2TextCodec.decodeCertificateNorito(
            OfflineBearerV2TextCodec.encodeCertificateNorito(senderCertificate)));
    assertReceiveRequestEquals(
        request,
        OfflineBearerV2TextCodec.decodeReceiveRequestNorito(
            OfflineBearerV2TextCodec.encodeReceiveRequestNorito(request)));
    assertDebitReceiptEquals(
        debit,
        OfflineBearerV2TextCodec.decodeDebitReceiptNorito(
            OfflineBearerV2TextCodec.encodeDebitReceiptNorito(debit)));
    assertCreditReceiptEquals(
        credit,
        OfflineBearerV2TextCodec.decodeCreditReceiptNorito(
            OfflineBearerV2TextCodec.encodeCreditReceiptNorito(credit)));
    assertSettlementBatchEquals(
        settlement,
        OfflineBearerV2TextCodec.decodeSettlementBatchNorito(
            OfflineBearerV2TextCodec.encodeSettlementBatchNorito(settlement)));

    final String requestText = OfflineBearerV2TextCodec.encodeReceiveRequestText(request);
    final String paymentText = OfflineBearerV2TextCodec.encodePaymentText(debit);
    final String ackText = OfflineBearerV2TextCodec.encodeAckText(credit);
    assertTrue(
        requestText.startsWith(OfflineBearerV2TextCodec.RECEIVE_REQUEST_TEXT_PREFIX),
        "receive request prefix");
    assertTrue(
        paymentText.startsWith(OfflineBearerV2TextCodec.PAYMENT_TEXT_PREFIX),
        "payment prefix");
    assertTrue(ackText.startsWith(OfflineBearerV2TextCodec.ACK_TEXT_PREFIX), "ack prefix");
    assertEquals(
        OfflineBearerV2TextCodec.PayloadKind.RECEIVE_REQUEST,
        OfflineBearerV2TextCodec.payloadKind(requestText),
        "request kind");
    assertEquals(
        OfflineBearerV2TextCodec.PayloadKind.PAYMENT,
        OfflineBearerV2TextCodec.payloadKind(paymentText),
        "payment kind");
    assertEquals(
        OfflineBearerV2TextCodec.PayloadKind.ACK,
        OfflineBearerV2TextCodec.payloadKind(ackText),
        "ack kind");
    assertReceiveRequestEquals(request, OfflineBearerV2TextCodec.decodeReceiveRequestText(requestText));
    assertDebitReceiptEquals(debit, OfflineBearerV2TextCodec.decodePaymentText(paymentText));
    assertCreditReceiptEquals(credit, OfflineBearerV2TextCodec.decodeAckText(ackText));

    assertEquals(null, OfflineBearerV2TextCodec.payloadKind("wallet-offline-receive:AAAA"), "old receive kind");
    assertEquals(null, OfflineBearerV2TextCodec.payloadKind("wallet-offline-payment:AAAA"), "old payment kind");
    assertEquals(null, OfflineBearerV2TextCodec.payloadKind("wallet-offline-ack:AAAA"), "old ack kind");
    expectThrows(
        IllegalArgumentException.class,
        () -> OfflineBearerV2TextCodec.decodeReceiveRequestText("wallet-offline-receive:AAAA"));
    expectThrows(
        IllegalArgumentException.class,
        () -> OfflineBearerV2TextCodec.decodePaymentText("wallet-offline-payment:AAAA"));
    expectThrows(
        IllegalArgumentException.class,
        () -> OfflineBearerV2TextCodec.decodeAckText("wallet-offline-ack:AAAA"));
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
        clock::get,
        new OfflineBearerWallet.SignatureVerifier(
            Collections.singletonList(ISSUER_KEYPAIR.publicKey())));
  }

  private static void assertPolicyEquals(
      final OfflineBearerWallet.PolicyBundleV2 expected,
      final OfflineBearerWallet.PolicyBundleV2 actual) {
    assertEquals(expected.policyId(), actual.policyId(), "policyId");
    assertEquals(expected.policyHashHex(), actual.policyHashHex(), "policyHashHex");
    assertEquals(expected.issuerId(), actual.issuerId(), "issuerId");
    assertEquals(expected.issuedAtMs(), actual.issuedAtMs(), "policy issuedAtMs");
    assertEquals(expected.expiresAtMs(), actual.expiresAtMs(), "policy expiresAtMs");
    assertEquals(
        expected.maxCertificateAgeMs(), actual.maxCertificateAgeMs(), "maxCertificateAgeMs");
    assertEquals(expected.maxPolicyAgeMs(), actual.maxPolicyAgeMs(), "maxPolicyAgeMs");
    assertEquals(expected.maxTokenAgeMs(), actual.maxTokenAgeMs(), "maxTokenAgeMs");
    assertEquals(expected.maxOfflineBalance(), actual.maxOfflineBalance(), "maxOfflineBalance");
    assertEquals(
        expected.maxTransactionAmount(), actual.maxTransactionAmount(), "maxTransactionAmount");
    assertEquals(expected.allowedHardwareClasses(), actual.allowedHardwareClasses(), "hardware");
    assertEquals(expected.blacklistedAccountIds(), actual.blacklistedAccountIds(), "blacklist accounts");
    assertEquals(expected.blacklistedDeviceIds(), actual.blacklistedDeviceIds(), "blacklist devices");
    assertEquals(expected.blacklistedKeyIds(), actual.blacklistedKeyIds(), "blacklist keys");
    assertEquals(expected.signatureAlgorithm(), actual.signatureAlgorithm(), "policy algorithm");
    assertBytesEqual(expected.issuerSignature(), actual.issuerSignature(), "policy signature");
    assertEquals(expected.policyEpoch(), actual.policyEpoch(), "policy epoch");
    assertEquals(expected.policySource(), actual.policySource(), "policy source");
    assertEquals(expected.revokedCertificateIds(), actual.revokedCertificateIds(), "revoked certs");
    assertEquals(expected.revokedTransferIds(), actual.revokedTransferIds(), "revoked transfers");
    assertEquals(expected.assetSendLimits().size(), actual.assetSendLimits().size(), "asset limits");
  }

  private static void assertCertificateEquals(
      final OfflineBearerWallet.CertificateV2 expected,
      final OfflineBearerWallet.CertificateV2 actual) {
    assertEquals(expected.certificateId(), actual.certificateId(), "certificateId");
    assertEquals(expected.chainId(), actual.chainId(), "certificate chainId");
    assertEquals(expected.issuerId(), actual.issuerId(), "certificate issuer");
    assertEquals(expected.purseId(), actual.purseId(), "purseId");
    assertEquals(expected.accountId(), actual.accountId(), "accountId");
    assertEquals(expected.assetDefinitionId(), actual.assetDefinitionId(), "certificate asset");
    assertEquals(expected.deviceId(), actual.deviceId(), "deviceId");
    assertEquals(expected.keyId(), actual.keyId(), "keyId");
    assertEquals(expected.hardwareClass(), actual.hardwareClass(), "hardwareClass");
    assertEquals(expected.signatureAlgorithm(), actual.signatureAlgorithm(), "certificate algorithm");
    assertEquals(expected.publicKeyEncoding(), actual.publicKeyEncoding(), "public key encoding");
    assertBytesEqual(expected.publicKey(), actual.publicKey(), "public key");
    assertEquals(expected.issuedAtMs(), actual.issuedAtMs(), "certificate issuedAtMs");
    assertEquals(expected.expiresAtMs(), actual.expiresAtMs(), "certificate expiresAtMs");
    assertEquals(expected.policyId(), actual.policyId(), "certificate policyId");
    assertEquals(expected.policyHashHex(), actual.policyHashHex(), "certificate policy hash");
    assertBytesEqual(expected.issuerSignature(), actual.issuerSignature(), "certificate signature");
  }

  private static void assertReceiveRequestEquals(
      final OfflineBearerWallet.ReceiveRequestV2 expected,
      final OfflineBearerWallet.ReceiveRequestV2 actual) {
    assertEquals(expected.version(), actual.version(), "receive version");
    assertEquals(expected.chainId(), actual.chainId(), "receive chainId");
    assertEquals(expected.paymentRequestId(), actual.paymentRequestId(), "paymentRequestId");
    assertCertificateEquals(expected.recipientCertificate(), actual.recipientCertificate());
    assertEquals(expected.assetDefinitionId(), actual.assetDefinitionId(), "receive asset");
    assertEquals(expected.amount(), actual.amount(), "receive amount");
    assertEquals(expected.createdAtMs(), actual.createdAtMs(), "receive createdAtMs");
    assertEquals(expected.expiresAtMs(), actual.expiresAtMs(), "receive expiresAtMs");
    assertEquals(expected.policyHashHex(), actual.policyHashHex(), "receive policy hash");
    assertEquals(expected.signatureAlgorithm(), actual.signatureAlgorithm(), "receive algorithm");
    assertBytesEqual(expected.challengeSignature(), actual.challengeSignature(), "receive signature");
  }

  private static void assertDebitReceiptEquals(
      final OfflineBearerWallet.DebitReceiptV2 expected,
      final OfflineBearerWallet.DebitReceiptV2 actual) {
    assertEquals(expected.version(), actual.version(), "debit version");
    assertEquals(expected.transferId(), actual.transferId(), "debit transferId");
    assertEquals(expected.chainId(), actual.chainId(), "debit chainId");
    assertEquals(expected.paymentRequestId(), actual.paymentRequestId(), "debit request id");
    assertCertificateEquals(expected.senderCertificate(), actual.senderCertificate());
    assertCertificateEquals(expected.recipientCertificate(), actual.recipientCertificate());
    assertEquals(expected.assetDefinitionId(), actual.assetDefinitionId(), "debit asset");
    assertEquals(expected.amount(), actual.amount(), "debit amount");
    assertEquals(expected.senderPreBalance(), actual.senderPreBalance(), "sender pre");
    assertEquals(expected.senderPostBalance(), actual.senderPostBalance(), "sender post");
    assertEquals(expected.senderSequence(), actual.senderSequence(), "sender sequence");
    assertEquals(expected.createdAtMs(), actual.createdAtMs(), "debit createdAtMs");
    assertEquals(expected.expiresAtMs(), actual.expiresAtMs(), "debit expiresAtMs");
    assertEquals(expected.policyHashHex(), actual.policyHashHex(), "debit policy hash");
    assertBytesEqual(
        expected.receiveChallengeSignature(),
        actual.receiveChallengeSignature(),
        "receive challenge signature");
    assertEquals(expected.signatureAlgorithm(), actual.signatureAlgorithm(), "debit algorithm");
    assertBytesEqual(expected.debitSignature(), actual.debitSignature(), "debit signature");
  }

  private static void assertCreditReceiptEquals(
      final OfflineBearerWallet.CreditReceiptV2 expected,
      final OfflineBearerWallet.CreditReceiptV2 actual) {
    assertEquals(expected.version(), actual.version(), "credit version");
    assertEquals(expected.transferId(), actual.transferId(), "credit transferId");
    assertEquals(expected.chainId(), actual.chainId(), "credit chainId");
    assertCertificateEquals(expected.recipientCertificate(), actual.recipientCertificate());
    assertEquals(expected.amount(), actual.amount(), "credit amount");
    assertEquals(expected.recipientPreBalance(), actual.recipientPreBalance(), "recipient pre");
    assertEquals(expected.recipientPostBalance(), actual.recipientPostBalance(), "recipient post");
    assertEquals(expected.recipientSequence(), actual.recipientSequence(), "recipient sequence");
    assertEquals(expected.acceptedAtMs(), actual.acceptedAtMs(), "acceptedAtMs");
    assertEquals(expected.signatureAlgorithm(), actual.signatureAlgorithm(), "credit algorithm");
    assertBytesEqual(expected.creditSignature(), actual.creditSignature(), "credit signature");
  }

  private static void assertSettlementBatchEquals(
      final OfflineBearerWallet.SettlementBatchV2 expected,
      final OfflineBearerWallet.SettlementBatchV2 actual) {
    assertEquals(expected.version(), actual.version(), "settlement version");
    assertEquals(expected.chainId(), actual.chainId(), "settlement chainId");
    assertEquals(expected.purseId(), actual.purseId(), "settlement purseId");
    assertEquals(expected.debitReceipts().size(), actual.debitReceipts().size(), "settlement debits");
    assertEquals(expected.creditReceipts().size(), actual.creditReceipts().size(), "settlement credits");
    for (int index = 0; index < expected.debitReceipts().size(); index++) {
      assertDebitReceiptEquals(expected.debitReceipts().get(index), actual.debitReceipts().get(index));
    }
    for (int index = 0; index < expected.creditReceipts().size(); index++) {
      assertCreditReceiptEquals(
          expected.creditReceipts().get(index), actual.creditReceipts().get(index));
    }
  }

  private static void assertBytesEqual(
      final byte[] expected, final byte[] actual, final String label) {
    if (!Arrays.equals(expected, actual)) {
      throw new AssertionError(label + ": byte arrays differ");
    }
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
    final OfflineBearerWallet.PolicyBundleV2 unsigned =
        new OfflineBearerWallet.PolicyBundleV2(
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
        new byte[] {1});
    return new OfflineBearerWallet.PolicyBundleV2(
        unsigned.policyId(),
        unsigned.policyHashHex(),
        unsigned.issuerId(),
        unsigned.issuedAtMs(),
        unsigned.expiresAtMs(),
        unsigned.maxCertificateAgeMs(),
        unsigned.maxPolicyAgeMs(),
        unsigned.maxTokenAgeMs(),
        unsigned.maxOfflineBalance(),
        unsigned.maxTransactionAmount(),
        unsigned.allowedHardwareClasses(),
        unsigned.blacklistedAccountIds(),
        unsigned.blacklistedDeviceIds(),
        unsigned.blacklistedKeyIds(),
        ISSUER_KEYPAIR.sign(OfflineBearerWallet.Payloads.policyUnsignedPayload(unsigned)),
        unsigned.policyEpoch(),
        unsigned.policySource(),
        unsigned.revokedCertificateIds(),
        unsigned.revokedTransferIds(),
        unsigned.assetSendLimits());
  }

  private static OfflineBearerWallet.CertificateV2 certificate(
      final String accountId, final String purseId, final byte[] publicKey) {
    return certificate(accountId, purseId, publicKey, NOW - 1_000L);
  }

  private static OfflineBearerWallet.CertificateV2 certificate(
      final String accountId, final String purseId, final byte[] publicKey, final long issuedAtMs) {
    final OfflineBearerWallet.CertificateV2 unsigned =
        new OfflineBearerWallet.CertificateV2(
        "cert-" + purseId,
        CHAIN,
        ISSUER,
        purseId,
        accountId,
        ASSET,
        "device-" + purseId,
        "key-" + purseId,
        HARDWARE_CLASS,
        publicKey,
        issuedAtMs,
        NOW + 60L * 60L * 1_000L,
        "policy-1",
        POLICY_HASH,
        new byte[] {1});
    return new OfflineBearerWallet.CertificateV2(
        unsigned.certificateId(),
        unsigned.chainId(),
        unsigned.issuerId(),
        unsigned.purseId(),
        unsigned.accountId(),
        unsigned.assetDefinitionId(),
        unsigned.deviceId(),
        unsigned.keyId(),
        unsigned.hardwareClass(),
        unsigned.publicKey(),
        unsigned.issuedAtMs(),
        unsigned.expiresAtMs(),
        unsigned.policyId(),
        unsigned.policyHashHex(),
        ISSUER_KEYPAIR.sign(OfflineBearerWallet.Payloads.certificateUnsignedPayload(unsigned)));
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
    private final TestEd25519Keypair keypair;
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
      this.keypair = new TestEd25519Keypair("purse:" + purseId);
    }

    private byte[] publicKey() {
      return keypair.publicKey();
    }

    @Override
    public OfflineBearerWallet.SecureElementCapabilities capabilities() {
      return new OfflineBearerWallet.SecureElementCapabilities(
          true,
          true,
          HARDWARE_CLASS,
          attestationKeyId,
          OfflineBearerWallet.SIGNATURE_ALGORITHM_ED25519,
          OfflineBearerWallet.PUBLIC_KEY_ENCODING_RAW_ED25519,
          true,
          new byte[] {1});
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
      final OfflineBearerWallet.ReceiveRequestV2 unsigned =
          new OfflineBearerWallet.ReceiveRequestV2(
          OfflineBearerWallet.ReceiveRequestV2.VERSION,
          state.chainId(),
          paymentRequestId,
          certificate,
          state.assetDefinitionId(),
          amount,
          createdAtMs,
          expiresAtMs,
          policyHashHex,
          new byte[] {1});
      return new OfflineBearerWallet.ReceiveRequestV2(
          unsigned.version(),
          unsigned.chainId(),
          unsigned.paymentRequestId(),
          unsigned.recipientCertificate(),
          unsigned.assetDefinitionId(),
          unsigned.amount(),
          unsigned.createdAtMs(),
          unsigned.expiresAtMs(),
          unsigned.policyHashHex(),
          keypair.sign(OfflineBearerWallet.Payloads.receiveRequestUnsignedPayload(unsigned)));
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
      final OfflineBearerWallet.DebitReceiptV2 unsigned =
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
              new byte[] {1});
      final OfflineBearerWallet.DebitReceiptV2 receipt =
          new OfflineBearerWallet.DebitReceiptV2(
              unsigned.version(),
              unsigned.transferId(),
              unsigned.chainId(),
              unsigned.paymentRequestId(),
              unsigned.senderCertificate(),
              unsigned.recipientCertificate(),
              unsigned.assetDefinitionId(),
              unsigned.amount(),
              unsigned.senderPreBalance(),
              unsigned.senderPostBalance(),
              unsigned.senderSequence(),
              unsigned.createdAtMs(),
              unsigned.expiresAtMs(),
              unsigned.policyHashHex(),
              unsigned.receiveChallengeSignature(),
              keypair.sign(OfflineBearerWallet.Payloads.debitReceiptUnsignedPayload(unsigned)));
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
      final OfflineBearerWallet.CreditReceiptV2 unsigned =
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
              new byte[] {1});
      final OfflineBearerWallet.CreditReceiptV2 credit =
          new OfflineBearerWallet.CreditReceiptV2(
              unsigned.version(),
              unsigned.transferId(),
              unsigned.chainId(),
              unsigned.recipientCertificate(),
              unsigned.amount(),
              unsigned.recipientPreBalance(),
              unsigned.recipientPostBalance(),
              unsigned.recipientSequence(),
              unsigned.acceptedAtMs(),
              keypair.sign(OfflineBearerWallet.Payloads.creditReceiptUnsignedPayload(unsigned)));
      if (debits.stream().noneMatch(existing -> existing.transferId().equals(receipt.transferId()))) {
        debits.add(receipt);
      }
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

  private static final class TestEd25519Keypair {
    private final Ed25519PrivateKeyParameters privateKey;

    private TestEd25519Keypair(final String seedText) {
      this.privateKey = new Ed25519PrivateKeyParameters(signature(seedText), 0);
    }

    private byte[] publicKey() {
      return privateKey.generatePublicKey().getEncoded();
    }

    private byte[] sign(final byte[] payload) {
      final Ed25519Signer signer = new Ed25519Signer();
      signer.init(true, privateKey);
      signer.update(payload, 0, payload.length);
      return signer.generateSignature();
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

  private static void assertEquals(final long expected, final long actual, final String label) {
    if (expected != actual) {
      throw new AssertionError(label + ": expected " + expected + " but got " + actual);
    }
  }

  private static void assertEquals(final Object expected, final Object actual, final String label) {
    if (!Objects.equals(expected, actual)) {
      throw new AssertionError(label + ": expected " + expected + " but got " + actual);
    }
  }

  private static void assertTrue(final boolean value, final String label) {
    if (!value) {
      throw new AssertionError(label + ": expected true");
    }
  }

  @FunctionalInterface
  private interface ThrowingRunnable {
    void run() throws Throwable;
  }
}
