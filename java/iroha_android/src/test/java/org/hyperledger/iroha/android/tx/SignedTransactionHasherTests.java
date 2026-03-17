package org.hyperledger.iroha.android.tx;

import java.util.Arrays;
import java.util.Collections;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;

public final class SignedTransactionHasherTests {

  private SignedTransactionHasherTests() {}

  public static void main(final String[] args) throws Exception {
    hashHexMatchesCanonicalBytes();
    hashIgnoresExportedKeyBundle();
    hashRejectsInvalidPayload();
    canonicalBytesWrapsEntrypoint();
    System.out.println("[IrohaAndroid] Signed transaction hasher tests passed.");
  }

  private static void hashHexMatchesCanonicalBytes() throws Exception {
    final SignedTransaction transaction = newTransaction((byte) 0x11);
    final byte[] canonical = SignedTransactionHasher.canonicalBytes(transaction);
    final String hashFromTransaction = SignedTransactionHasher.hashHex(transaction);
    final String hashFromCanonical = SignedTransactionHasher.hashCanonicalHex(canonical);
    assert hashFromTransaction.equals(hashFromCanonical)
        : "Hash computed from transaction must match canonical bytes hash";
  }

  private static void hashIgnoresExportedKeyBundle() throws Exception {
    final SignedTransaction base = newTransaction((byte) 0x42);
    final byte[] exported = new byte[48];
    Arrays.fill(exported, (byte) 0x7A);
    final SignedTransaction withBundle =
        new SignedTransaction(
            base.encodedPayload(),
            base.signature(),
            base.publicKey(),
            base.schemaName(),
            base.keyAlias().orElse("alias-bundle"),
            exported);
    final String baseHash = SignedTransactionHasher.hashHex(base);
    final String bundleHash = SignedTransactionHasher.hashHex(withBundle);
    assert baseHash.equals(bundleHash)
        : "Exported key bundles must not affect canonical signed transaction hash";
  }

  private static void hashRejectsInvalidPayload() {
    final SignedTransaction invalid =
        new SignedTransaction(
            new byte[] {0x01, 0x02, 0x03},
            new byte[64],
            new byte[32],
            "iroha.android.transaction.Payload.v1");
    try {
      SignedTransactionHasher.hashHex(invalid);
      throw new AssertionError("Expected invalid payload to fail Norito encoding");
    } catch (IllegalStateException ex) {
      assert ex.getMessage().contains("Failed to encode signed transaction")
          : "Invalid payloads should surface encoding failures";
    }
  }

  private static void canonicalBytesWrapsEntrypoint() throws Exception {
    final SignedTransaction transaction = newTransaction((byte) 0x33);
    final byte[] encoded = SignedTransactionEncoder.encode(transaction);
    final byte[] canonical = SignedTransactionHasher.canonicalBytes(transaction);
    assert canonical.length == encoded.length + 12
        : "Canonical bytes must include entrypoint wrapper";
    for (int i = 0; i < 4; i++) {
      assert canonical[i] == 0 : "Entrypoint discriminant must be zero";
    }
    long length = 0L;
    for (int i = 0; i < 8; i++) {
      length |= (long) (canonical[4 + i] & 0xFF) << (i * 8);
    }
    assert length == encoded.length : "Entrypoint length must match encoded payload length";
    final byte[] payload = Arrays.copyOfRange(canonical, 12, canonical.length);
    assert Arrays.equals(payload, encoded) : "Entrypoint payload must match signed transaction";
  }

  private static SignedTransaction newTransaction(final byte seed) throws NoritoException {
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(String.format("%08x", seed))
            .setAuthority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
            .setCreationTimeMs(1_700_000_000_000L + seed)
            .setInstructionBytes(new byte[] {seed, (byte) (seed + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce(99)
            .setMetadata(Collections.singletonMap("note", "txn-" + seed))
            .build();
    final byte[] encodedPayload = codec.encodeTransaction(payload);
    final byte[] signature = new byte[64];
    Arrays.fill(signature, (byte) (seed + 2));
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) (seed + 3));
    return new SignedTransaction(
        encodedPayload, signature, publicKey, codec.schemaName(), "alias-" + seed);
  }
}
