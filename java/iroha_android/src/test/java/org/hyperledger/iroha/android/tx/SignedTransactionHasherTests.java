package org.hyperledger.iroha.android.tx;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.Properties;
import java.util.Set;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.testing.TestAccountIds;

public final class SignedTransactionHasherTests {

  private static final Set<String> COMPACT_FIXTURE_KEYS =
      Set.of(
          "schema.version",
          "source.tag",
          "source.commit",
          "reference",
          "versioned.bytes",
          "versioned.sha256",
          "bare.bytes",
          "compact.length.hex",
          "canonical.prefix.hex",
          "canonical.hash",
          "payload.prehash",
          "pinned.sdk.defective.hash",
          "versioned.base64");

  private SignedTransactionHasherTests() {}

  public static void main(final String[] args) throws Exception {
    hashHexMatchesCanonicalBytes();
    byteHelpersRejectRawHashingAndDoubleWrapping();
    byteHelpersRejectMalformedOrNonBareEncodings();
    hashIgnoresExportedKeyBundle();
    hashRejectsInvalidPayload();
    compactLengthBoundariesAreCanonical();
    canonicalBytesWrapsEntrypoint();
    compactEntrypointGoldenMatchesNativeRust();
    compactFixtureParserRejectsDuplicateKeysAndBase64Aliases();
    System.out.println("[IrohaAndroid] Signed transaction hasher tests passed.");
  }

  private static void hashHexMatchesCanonicalBytes() throws Exception {
    final SignedTransaction transaction = newTransaction((byte) 0x11);
    final byte[] bare = SignedTransactionEncoder.encode(transaction);
    final String hashFromTransaction = SignedTransactionHasher.hashHex(transaction);
    final String hashFromCanonical = SignedTransactionHasher.hashCanonicalHex(bare);
    assert hashFromTransaction.equals(hashFromCanonical)
        : "Hash computed from transaction must match canonical bare bytes hash";
    assert Arrays.equals(
            SignedTransactionHasher.canonicalBytes(transaction),
            SignedTransactionHasher.canonicalBytesFromBare(bare))
        : "Object and byte-array paths must share the same External wrapper";
  }

  private static void byteHelpersRejectRawHashingAndDoubleWrapping() throws Exception {
    final SignedTransaction transaction = newTransaction((byte) 0x12);
    final byte[] bare = SignedTransactionEncoder.encode(transaction);
    final byte[] canonical = SignedTransactionHasher.canonicalBytesFromBare(bare);
    assert !toHex(IrohaHash.prehash(bare)).equals(SignedTransactionHasher.hashCanonicalHex(bare))
        : "Bare signed bytes must not be hashed without the External wrapper";
    expectInvalidBare(canonical, "entrypoint-wrapped bytes");
    expectInvalidBare(
        SignedTransactionHasher.canonicalBytesFromBare(bare),
        "double-wrapped entrypoint bytes");
  }

  private static void byteHelpersRejectMalformedOrNonBareEncodings() throws Exception {
    final SignedTransaction transaction = newTransaction((byte) 0x13);
    final byte[] bare = SignedTransactionEncoder.encode(transaction);
    expectInvalidBare(new byte[0], "empty bytes");
    expectInvalidBare(Arrays.copyOf(bare, bare.length - 1), "truncated bytes");
    final byte[] trailing = Arrays.copyOf(bare, bare.length + 1);
    expectInvalidBare(trailing, "trailing bytes");
    expectInvalidBare(SignedTransactionEncoder.encodeVersioned(transaction), "versioned bytes");

    final byte[] overlongFirstLength = new byte[bare.length + 1];
    assert (bare[0] & 0x80) != 0 && (bare[1] & 0x80) == 0
        : "Test transaction must begin with a two-byte compact length";
    overlongFirstLength[0] = bare[0];
    overlongFirstLength[1] = (byte) (bare[1] | 0x80);
    overlongFirstLength[2] = 0;
    System.arraycopy(bare, 2, overlongFirstLength, 3, bare.length - 2);
    expectInvalidBare(overlongFirstLength, "overlong compact field length");
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
    final byte[] expectedLength = SignedTransactionHasher.encodeCompactLength(encoded.length);
    assert canonical.length == encoded.length + 4 + expectedLength.length
        : "Canonical bytes must include entrypoint wrapper";
    for (int i = 0; i < 4; i++) {
      assert canonical[i] == 0 : "Entrypoint discriminant must be zero";
    }
    assert Arrays.equals(
            expectedLength,
            Arrays.copyOfRange(canonical, 4, 4 + expectedLength.length))
        : "Entrypoint length must use minimal COMPACT_LEN encoding";
    final byte[] payload =
        Arrays.copyOfRange(canonical, 4 + expectedLength.length, canonical.length);
    assert Arrays.equals(payload, encoded) : "Entrypoint payload must match signed transaction";
  }

  private static void compactLengthBoundariesAreCanonical() {
    assertCompactLength(0L, 0x00);
    assertCompactLength(127L, 0x7F);
    assertCompactLength(128L, 0x80, 0x01);
    assertCompactLength(16_383L, 0xFF, 0x7F);
    assertCompactLength(16_384L, 0x80, 0x80, 0x01);
    assertCompactLength(Integer.MAX_VALUE, 0xFF, 0xFF, 0xFF, 0xFF, 0x07);
    assertCompactLength(
        Long.MAX_VALUE,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0x7F);
    try {
      SignedTransactionHasher.encodeCompactLength(-1L);
      throw new AssertionError("Negative compact lengths must be rejected");
    } catch (IllegalArgumentException expected) {
      assert expected.getMessage().contains("non-negative");
    }
  }

  private static void compactEntrypointGoldenMatchesNativeRust() throws Exception {
    final Path fixturePath = resolveCompactHashFixture();
    final Properties fixture = parseCompactHashFixture(Files.readString(fixturePath));
    final byte[] versioned =
        decodeCanonicalBase64(fixture.getProperty("versioned.base64"), "versioned.base64");
    assert versioned.length == Integer.parseInt(fixture.getProperty("versioned.bytes"));
    assert versioned[0] == 1 : "Golden must carry SignedTransaction version 1";
    assert hex(MessageDigest.getInstance("SHA-256").digest(versioned))
        .equals(fixture.getProperty("versioned.sha256"));

    final SignedTransaction decoded = SignedTransactionEncoder.decodeVersioned(versioned);
    assert hex(IrohaHash.prehash(decoded.encodedPayload()))
        .equals(fixture.getProperty("payload.prehash"))
        : "Decoded payload prehash must match the shared signer golden";
    final byte[] bare = SignedTransactionEncoder.encode(decoded);
    assert bare.length == Integer.parseInt(fixture.getProperty("bare.bytes"));
    assert Arrays.equals(bare, Arrays.copyOfRange(versioned, 1, versioned.length));

    final byte[] canonical = SignedTransactionHasher.canonicalBytes(decoded);
    final byte[] expectedPrefix = decodeHex(fixture.getProperty("canonical.prefix.hex"));
    assert Arrays.equals(expectedPrefix, Arrays.copyOf(canonical, expectedPrefix.length));
    assert SignedTransactionHasher.hashHex(decoded).equals(fixture.getProperty("canonical.hash"))
        : "Java compact entrypoint hash must match the native Rust golden";
    assert SignedTransactionHasher.hashCanonicalHex(bare)
        .equals(fixture.getProperty("canonical.hash"))
        : "Java bare-byte helper must match the native Rust golden";
    assert !toHex(IrohaHash.prehash(bare)).equals(fixture.getProperty("canonical.hash"))
        : "Raw bare-byte hashing must remain distinguishable from the entrypoint hash";
  }

  private static void compactFixtureParserRejectsDuplicateKeysAndBase64Aliases()
      throws Exception {
    final String contents = Files.readString(resolveCompactHashFixture());
    try {
      parseCompactHashFixture(contents + "\ncanonical.hash=duplicate\n");
      throw new AssertionError("Duplicate compact fixture keys must fail closed");
    } catch (IllegalStateException expected) {
      assert expected.getMessage().contains("Duplicate compact fixture property");
    }
    for (final String malformed :
        Arrays.asList("YQ!!", "Y Q==", "YQ=", "YQ===", "YR==")) {
      try {
        decodeCanonicalBase64(malformed, "versioned.base64");
        throw new AssertionError("Malformed base64 must fail closed: " + malformed);
      } catch (IllegalStateException expected) {
        assert expected.getMessage().contains("base64");
      }
    }
  }

  private static Properties parseCompactHashFixture(final String contents) {
    final Properties result = new Properties();
    for (final String line : contents.split("\\R")) {
      if (line.isEmpty() || line.startsWith("#")) {
        continue;
      }
      final int separator = line.indexOf('=');
      if (separator <= 0 || separator == line.length() - 1) {
        throw new IllegalStateException("Malformed compact fixture property: " + line);
      }
      final String key = line.substring(0, separator);
      final String value = line.substring(separator + 1);
      if (result.containsKey(key)) {
        throw new IllegalStateException("Duplicate compact fixture property: " + key);
      }
      result.setProperty(key, value);
    }
    if (!result.stringPropertyNames().equals(COMPACT_FIXTURE_KEYS)) {
      throw new IllegalStateException("Compact fixture property keys must match the required set");
    }
    decodeCanonicalBase64(result.getProperty("versioned.base64"), "versioned.base64");
    return result;
  }

  private static byte[] decodeCanonicalBase64(final String value, final String context) {
    try {
      final byte[] decoded = Base64.getDecoder().decode(value);
      if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
        throw new IllegalStateException(context + " is not canonical base64");
      }
      return decoded;
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(context + " is not valid base64", ex);
    }
  }

  private static void expectInvalidBare(final byte[] bytes, final String context) {
    try {
      SignedTransactionHasher.hashCanonicalHex(bytes);
      throw new AssertionError("Expected " + context + " to be rejected");
    } catch (IllegalArgumentException expected) {
      assert expected.getMessage().contains("canonical bare")
          : "Invalid bare input should explain the canonical encoding requirement";
    }
  }

  private static Path resolveCompactHashFixture() {
    Path current = Paths.get("").toAbsolutePath().normalize();
    while (current != null) {
      final Path candidate =
          current.resolve("fixtures/norito_rpc/iroha_compact_hash_vector.properties");
      if (Files.isRegularFile(candidate)) {
        return candidate;
      }
      current = current.getParent();
    }
    throw new IllegalStateException("Unable to locate compact transaction hash fixture");
  }

  private static byte[] decodeHex(final String value) {
    if (value == null || (value.length() & 1) != 0) {
      throw new IllegalArgumentException("Expected even-length hexadecimal fixture value");
    }
    final byte[] result = new byte[value.length() / 2];
    for (int i = 0; i < result.length; i++) {
      final int high = Character.digit(value.charAt(i * 2), 16);
      final int low = Character.digit(value.charAt(i * 2 + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException("Invalid hexadecimal fixture value");
      }
      result[i] = (byte) ((high << 4) | low);
    }
    return result;
  }

  private static String hex(final byte[] bytes) {
    return toHex(bytes);
  }

  private static String toHex(final byte[] bytes) {
    final StringBuilder output = new StringBuilder(bytes.length * 2);
    for (byte value : bytes) {
      output.append(String.format("%02x", value));
    }
    return output.toString();
  }

  private static void assertCompactLength(final long value, final int... expected) {
    final byte[] expectedBytes = new byte[expected.length];
    for (int i = 0; i < expected.length; i++) {
      expectedBytes[i] = (byte) expected[i];
    }
    assert Arrays.equals(expectedBytes, SignedTransactionHasher.encodeCompactLength(value))
        : "Unexpected COMPACT_LEN encoding for " + value;
  }

  private static SignedTransaction newTransaction(final byte seed) throws NoritoException {
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final TransactionPayload payload =
        TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList(), 1L))
            .setChainId(String.format("%08x", seed))
            .setAuthority(TestAccountIds.ed25519Authority(0x2C))
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
