package org.hyperledger.iroha.sdk.nexus;

import static org.junit.jupiter.api.Assertions.*;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.client.ClientResponse;
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.crypto.IrohaHash;
import org.hyperledger.iroha.sdk.tx.SignedTransaction;
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher;
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter;
import org.junit.jupiter.api.Test;

/** Java construction, signing and immutable ownership use the canonical Kotlin Nexus values. */
final class NexusModelsJavaConsumerTest {
  private static final String ACCOUNT =
      "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
  private static final String DESTINATION =
      "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L";
  private static final String ASSET = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#" + ACCOUNT;
  private static final FeePaymentIntent FEE = FeePaymentIntent.authority(Collections.emptyList());

  private static byte[] bytes(int length, int value) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static NetworkId network() {
    return NetworkId.fromBytes(bytes(32, 1));
  }

  private static NexusAppConfig config(byte[] key, Map<String, String> metadata) {
    return new NexusAppConfig(network(), "chain", AccountAddress.DEFAULT_I105_DISCRIMINANT,
        "app", null, null, ACCOUNT, key, metadata);
  }

  private static NexusTransferInput input(byte[] key, Map<String, String> metadata) {
    return new NexusTransferInput(ASSET, "12.34", DESTINATION, FEE, ACCOUNT, key,
        1_700_000_000_000L, 30_000L, 7L, metadata);
  }

  @Test
  void contextsSnapshotKeysMetadataAndScopesAndHaveContentEquality() {
    final byte[] key = bytes(32, 7);
    final Map<String, String> metadata = new LinkedHashMap<>();
    metadata.put("purpose", "transfer");
    final LinkedHashSet<String> scopes = new LinkedHashSet<>(Arrays.asList("accounts", "transfer"));
    final NexusAppConfig config = config(key, metadata);
    final NexusConnectOptions options = new NexusConnectOptions(scopes, "sora://wallet", null, metadata, "s1");
    final NexusConnectSession session = new NexusConnectSession("s1", "sora://wallet", "app",
        null, null, ACCOUNT, key, metadata);
    final NexusApprovedAccount approval = new NexusApprovedAccount(ACCOUNT, key, session);
    final NexusTransferInput input = input(key, metadata);
    final int[] hashes = {config.hashCode(), options.hashCode(), session.hashCode(),
        approval.hashCode(), input.hashCode()};

    Arrays.fill(key, (byte) 0);
    metadata.clear();
    scopes.clear();
    for (byte[] output : new byte[][] {config.getSigningPublicKey(), session.getSigningPublicKey(),
        approval.getSigningPublicKey(), input.getSigningPublicKey()}) {
      assertArrayEquals(bytes(32, 7), output);
      Arrays.fill(output, (byte) 0);
    }
    final Map<String, String> expected = Collections.singletonMap("purpose", "transfer");
    assertEquals(config(bytes(32, 7), expected), config);
    assertEquals(new NexusConnectOptions(new LinkedHashSet<>(Arrays.asList("accounts", "transfer")),
        "sora://wallet", null, expected, "s1"), options);
    final NexusConnectSession equalSession = new NexusConnectSession("s1", "sora://wallet", "app",
        null, null, ACCOUNT, bytes(32, 7), expected);
    assertEquals(equalSession, session);
    assertEquals(new NexusApprovedAccount(ACCOUNT, bytes(32, 7), equalSession), approval);
    assertEquals(input(bytes(32, 7), expected), input);
    assertArrayEquals(hashes, new int[] {config.hashCode(), options.hashCode(), session.hashCode(),
        approval.hashCode(), input.hashCode()});
    for (Map<String, String> output : Arrays.asList(config.appMetadata, options.metadata,
        session.metadata, input.metadata)) {
      assertEquals(expected, output);
      assertThrows(UnsupportedOperationException.class, () -> output.put("purpose", "changed"));
    }
    assertThrows(UnsupportedOperationException.class, () -> options.scopes.add("changed"));
    assertNotEquals(config(bytes(32, 8), expected), config);
    assertNotEquals(input(bytes(32, 8), expected), input);
    assertNotEquals(new NexusApprovedAccount(DESTINATION), approval);
    assertNotEquals(new NexusConnectSession("s2", "sora://wallet"), session);
    assertNotEquals(new NexusConnectOptions(), options);
    assertNull(new NexusApprovedAccount(ACCOUNT).getSigningPublicKey());
  }

  @Test
  void signableOwnsItsPayloadAndDerivesItsHashWithoutCallerAssertions() {
    final byte[] payload = {1, 2, 3};
    final byte[] key = bytes(32, 7);
    final byte[] signature = bytes(64, 9);
    final NexusSignableTransaction signable = new NexusSignableTransaction(payload, ACCOUNT, key);
    final NexusWalletSignature wallet = new NexusWalletSignature(signature);
    final String hash = signable.payloadHashHex;
    final int signableHash = signable.hashCode();
    final int walletHash = wallet.hashCode();
    payload[0] = 0;
    key[0] = 0;
    signature[0] = 0;
    signable.getPayloadBytes()[0] = 0;
    signable.getSigningPublicKey()[0] = 0;
    wallet.getSignature()[0] = 0;
    assertArrayEquals(new byte[] {1, 2, 3}, signable.getPayloadBytes());
    assertArrayEquals(bytes(32, 7), signable.getSigningPublicKey());
    assertArrayEquals(bytes(64, 9), wallet.getSignature());
    final StringBuilder expected = new StringBuilder();
    for (byte value : IrohaHash.prehash(new byte[] {1, 2, 3})) {
      expected.append(String.format(java.util.Locale.ROOT, "%02x", value & 0xff));
    }
    assertEquals(expected.toString(), hash);
    assertEquals(hash, signable.payloadHashHex);
    final NexusSignableTransaction equal = new NexusSignableTransaction(new byte[] {1, 2, 3},
        ACCOUNT, bytes(32, 7));
    assertEquals(equal, signable);
    assertEquals(signableHash, equal.hashCode());
    assertEquals(new NexusWalletSignature(bytes(64, 9)), wallet);
    assertEquals(walletHash, wallet.hashCode());
    assertNotEquals(new NexusWalletSignature(bytes(64, 8)), wallet);
    assertNotEquals(new NexusSignableTransaction(new byte[] {0, 2, 3}, ACCOUNT, bytes(32, 7)), signable);
    final NexusTransferDraft draft = new NexusTransferDraft(input(bytes(32, 7), Collections.emptyMap()), signable);
    final NexusTransferDraft same = new NexusTransferDraft(input(bytes(32, 7), Collections.emptyMap()), equal);
    assertEquals(same, draft);
    assertEquals(same.hashCode(), draft.hashCode());
  }

  @Test
  void constructionRejectsBlankIdentifiersAndAlgorithmAliases() {
    for (String blank : Arrays.asList("", " ", "\t\n")) {
      assertThrows(IllegalArgumentException.class, () -> new NexusAppConfig(network(), blank, 0));
      assertThrows(IllegalArgumentException.class, () -> new NexusConnectSession(blank, "sora://wallet"));
      assertThrows(IllegalArgumentException.class, () -> new NexusConnectSession("s1", blank));
      assertThrows(IllegalArgumentException.class, () -> new NexusTransferInput(blank, "1", DESTINATION, FEE));
      assertThrows(IllegalArgumentException.class, () -> new NexusTransferInput(ASSET, "1", blank, FEE));
      assertThrows(IllegalArgumentException.class, () -> new NexusSignableTransaction(new byte[] {1}, blank, bytes(32, 7)));
    }
    for (int discriminant : new int[] {-1, 65_536}) {
      assertThrows(IllegalArgumentException.class, () -> new NexusAppConfig(network(), "chain", discriminant));
    }
    for (String algorithm : Arrays.asList("0", "ED25519", "ed25519 ", "", "secp256k1")) {
      assertEquals("unsupported_signature_algorithm", assertThrows(NexusAppError.class,
          () -> new NexusWalletSignature(bytes(64, 9), algorithm)).code);
      assertEquals("unsupported_signature_algorithm", assertThrows(NexusAppError.class,
          () -> new NexusSignableTransaction(new byte[] {1}, ACCOUNT, bytes(32, 7), algorithm)).code);
    }
  }

  private static SignedTransaction signedTransaction() {
    final Ed25519PrivateKeyParameters privateKey = new Ed25519PrivateKeyParameters(bytes(32, 17), 0);
    final byte[] key = privateKey.generatePublicKey().getEncoded();
    final NexusTransferDraft draft = new NexusAppClient(config(key, Collections.emptyMap()))
        .buildTransferDraft(input(key, Collections.emptyMap()));
    final Ed25519Signer signer = new Ed25519Signer();
    signer.init(true, privateKey);
    final byte[] message = IrohaHash.prehash(draft.signable.getPayloadBytes());
    signer.update(message, 0, message.length);
    final byte[] signature = signer.generateSignature();
    final Ed25519Signer verifier = new Ed25519Signer();
    verifier.init(false, privateKey.generatePublicKey());
    verifier.update(message, 0, message.length);
    assertTrue(verifier.verifySignature(signature));
    return SignedTransaction.builder().setEncodedPayload(draft.signable.getPayloadBytes())
        .setSignature(signature).setPublicKey(key)
        .setSchemaName(new NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT).schemaName())
        .build();
  }

  @Test
  @SuppressWarnings("unchecked")
  void receiptSnapshotsNestedJsonAndPreservesTheExactSignedTransactionIdentity() {
    final SignedTransaction signed = signedTransaction();
    final String hash = SignedTransactionHasher.hashHex(signed);
    final ClientResponse submission = new ClientResponse(202, new byte[0], "accepted", hash, null);
    final Map<String, Object> inner = new LinkedHashMap<>();
    inner.put("kind", "Applied");
    inner.put("optional", null);
    final List<Object> list = new ArrayList<>();
    list.add(inner);
    final Map<String, Object> status = new LinkedHashMap<>();
    status.put("status", inner);
    status.put("history", list);
    final NexusTransferReceipt receipt = new NexusTransferReceipt(hash, signed, submission, status);
    inner.clear();
    list.clear();
    status.clear();
    final Map<String, Object> frozen = (Map<String, Object>) receipt.finalStatus.get("status");
    final List<Object> history = (List<Object>) receipt.finalStatus.get("history");
    assertEquals("Applied", frozen.get("kind"));
    assertTrue(frozen.containsKey("optional"));
    assertEquals(frozen, history.get(0));
    assertThrows(UnsupportedOperationException.class, frozen::clear);
    assertThrows(UnsupportedOperationException.class, history::clear);
    assertThrows(UnsupportedOperationException.class, receipt.finalStatus::clear);
    assertEquals(hash, SignedTransactionHasher.hashHex(receipt.signedTransaction));
    assertNull(new NexusTransferReceipt(hash, signed, submission).finalStatus);
    assertThrows(IllegalArgumentException.class, () -> new NexusTransferReceipt("bad", signed, submission));
    final String wrongHash = (hash.charAt(0) == '0' ? "1" : "0") + hash.substring(1);
    assertThrows(IllegalArgumentException.class, () -> new NexusTransferReceipt(wrongHash, signed, submission));
  }

  @Test
  void receiptRejectsMutableObjectsCyclesAndNonJsonNumbers() {
    final SignedTransaction signed = signedTransaction();
    final String hash = SignedTransactionHasher.hashHex(signed);
    final ClientResponse submission = new ClientResponse(202, new byte[0], "accepted", hash, null);
    final List<Object> cycle = new ArrayList<>();
    cycle.add(cycle);
    for (Object value : Arrays.asList(new byte[] {1}, new StringBuilder("mutable"),
        Double.NaN, Float.POSITIVE_INFINITY, cycle, Collections.singletonMap(1, "bad key"))) {
      assertThrows(IllegalArgumentException.class, () -> new NexusTransferReceipt(hash, signed,
          submission, Collections.singletonMap("value", value)));
    }
    final NexusFinalizeOptions options = new NexusFinalizeOptions();
    assertTrue(options.waitForFinalStatus);
    assertNull(options.pipelineStatusOptions);
    assertNotEquals(options, new NexusFinalizeOptions());
  }
}
