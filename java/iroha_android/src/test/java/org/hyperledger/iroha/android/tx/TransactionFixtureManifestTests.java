package org.hyperledger.iroha.android.tx;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.regex.Pattern;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.testing.SimpleJson;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

/**
 * Validates the Norito fixture manifest emitted by {@code scripts/export_norito_fixtures}.
 *
 * <p>The manifest advertises deterministic hashes, base64 payloads, and encoded blob lengths used
 * by downstream Android fixtures. This test ensures the entries stay in sync with the checked-in
 * {@code transaction_payloads.json} file and the generated {@code *.norito} blobs.
 */
public final class TransactionFixtureManifestTests {

  private static final Pattern HEX_64 = Pattern.compile("^[0-9a-fA-F]{64}$");
  private static final String HASH_ALGORITHM = "BLAKE2B-256";
  private static final String PAYLOAD_SCHEMA = "iroha.android.transaction.Payload.v1";
  private static final String SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1";
  private static final byte VERSION_BYTE = 0x01;
  private static final NoritoJavaCodecAdapter PAYLOAD_CODEC = new NoritoJavaCodecAdapter();
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static int compatChecked = 0;

  @Test
  public void validateManifest() throws Exception {
    runValidation();
  }

  @Test
  public void validateManifestSchema() throws Exception {
    final Path manifestPath = resolveFixturePath("transaction_fixtures.manifest.json");
    final Map<String, Object> manifest = loadManifest(manifestPath);
    assertSchemaMatches(manifest);
  }

  public static void main(final String[] args) throws Exception {
    runValidation();
  }

  private static void runValidation() throws Exception {
    compatChecked = 0;
    final Path manifestPath = resolveFixturePath("transaction_fixtures.manifest.json");
    final Map<String, Object> manifest = loadManifest(manifestPath);
    assertSchemaMatches(manifest);
    final SigningKeyInfo signingKey = parseSigningKey(manifest);

    final Object fixturesValue = manifest.get("fixtures");
    final List<Object> fixtures = asList(fixturesValue, "fixtures");
    if (fixtures.isEmpty()) {
      throw new IllegalStateException("Manifest must contain at least one fixture");
    }

    // Map fixtures from transaction_payloads.json by name for cross checks.
    final Map<String, TransactionPayloadFixtures.Fixture> payloadFixtures =
        loadPayloadFixtures(manifestPath.getParent());

    for (Object entry : fixtures) {
      validateFixture(entry, manifestPath, payloadFixtures, signingKey);
    }

    assertEquals(
        "All fixtures must be validated for SDK compatibility",
        fixtures.size(),
        compatChecked);
    System.out.println("[IrohaAndroid] Transaction fixture manifest tests passed.");
    System.out.println("[IrohaAndroid] Compatibility checks: " + compatChecked + " checked.");
  }

  private static void validateFixture(
      final Object entry,
      final Path manifestPath,
      final Map<String, TransactionPayloadFixtures.Fixture> payloadFixtures,
      final SigningKeyInfo signingKey)
      throws Exception {
    final Map<String, Object> map = asMap(entry, "fixture");
    final String name = requireString(map.get("name"), "fixture.name");
    final String payloadBase64 = requireString(map.get("payload_base64"), name + ".payload_base64");
    final String signedBase64 = requireString(map.get("signed_base64"), name + ".signed_base64");
    final String encodedFile = requireString(map.get("encoded_file"), name + ".encoded_file");

    final long encodedLen = requireNumber(map.get("encoded_len"), name + ".encoded_len");
    final long signedLen = requireNumber(map.get("signed_len"), name + ".signed_len");
    final String chain = requireString(map.get("chain"), name + ".chain");
    final String authority = requireString(map.get("authority"), name + ".authority");
    final Long ttl = optionalNumber(map.get("time_to_live_ms"));
    final Long nonce = optionalNumber(map.get("nonce"));
    final String payloadHash = requireString(map.get("payload_hash"), name + ".payload_hash");
    final String signedHash = requireString(map.get("signed_hash"), name + ".signed_hash");

    final Path baseDir = manifestPath != null ? manifestPath.getParent() : null;
    final Path encodedPath = (baseDir == null)
        ? Paths.get(encodedFile)
        : baseDir.resolve(encodedFile).normalize();
    if (!Files.exists(encodedPath)) {
      throw new IllegalStateException(name + ": encoded file not found: " + encodedPath);
    }
    final byte[] encodedBytes = Files.readAllBytes(encodedPath);
    assertEquals(
        name + ": encoded_len mismatch (expected " + encodedLen + ", found " + encodedBytes.length + ")",
        encodedLen,
        encodedBytes.length);
    final String actualBase64 = Base64.getEncoder().encodeToString(encodedBytes);
    assertEquals(
        name + ": payload_base64 does not match encoded file",
        payloadBase64,
        actualBase64);

    // Validate the signed payload looks well-formed base64 and matches the advertised length.
    final byte[] payloadBytes = decodeBase64(payloadBase64, name + ".payload_base64");
    final byte[] signedBytes = decodeBase64(signedBase64, name + ".signed_base64");
    assertEquals(
        name + ": signed_len mismatch (expected " + signedLen + ", found " + signedBytes.length + ")",
        signedLen,
        signedBytes.length);

    assertTrue(name + ": payload_hash must be a 64-character hex string",
        HEX_64.matcher(payloadHash).matches());
    assertTrue(name + ": signed_hash must be a 64-character hex string",
        HEX_64.matcher(signedHash).matches());
    final String computedPayloadHash = canonicalHashHex(payloadBytes);
    assertEquals(
        name + ": payload hash mismatch (expected " + payloadHash + ", computed " + computedPayloadHash + ")",
        payloadHash,
        computedPayloadHash);
    final String computedSignedHash = SignedTransactionHasher.hashCanonicalHex(signedBytes);
    assertEquals(
        name + ": signed hash mismatch (expected " + signedHash + ", computed " + computedSignedHash + ")",
        signedHash,
        computedSignedHash);

    final TransactionPayloadFixtures.Fixture payloadFixture = payloadFixtures.get(name);
    if (payloadFixture != null) {
      payloadFixture.encoded().ifPresent(encoded -> {
        assertEquals(
            name + ": manifest payload mismatch vs transaction_payloads entry",
            payloadBase64,
            encoded);
      });
      if (payloadFixture.isDecodable()) {
        final TransactionPayload payload = payloadFixture.toPayload();
        assertEquals(
            name + ": chain mismatch vs transaction_payloads",
            chain,
            payload.chainId());
        assertEquals(
            name + ": authority mismatch vs transaction_payloads",
            authority,
            payload.authority());
        assertTrue(
            name + ": TTL mismatch vs transaction_payloads",
            optionalLongEquals(payload.timeToLiveMs(), ttl));
        assertTrue(
            name + ": nonce mismatch vs transaction_payloads",
            optionalIntEquals(payload.nonce(), nonce));
      }
    }

    validateCompatibility(name, payloadBytes, signedBytes, signingKey);
  }

  private static byte[] decodeBase64(final String value, final String fieldName) {
    try {
      return Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(fieldName + " is not valid base64", ex);
    }
  }

  private static String canonicalHashHex(final byte[] data) {
    final byte[] digest = canonicalHashBytes(data);
    final StringBuilder builder = new StringBuilder(digest.length * 2);
    for (byte b : digest) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }

  private static byte[] canonicalHashBytes(final byte[] data) {
    byte[] digest;
    try {
      final MessageDigest md = MessageDigest.getInstance(HASH_ALGORITHM);
      digest = md.digest(data);
    } catch (final NoSuchAlgorithmException ex) {
      digest = Blake2b.digest(data);
    }
    digest[digest.length - 1] |= 1;
    return digest;
  }

  private static Map<String, Object> loadManifest(final Path manifestPath) throws Exception {
    final String json = new String(Files.readAllBytes(manifestPath), StandardCharsets.UTF_8);
    return asMap(SimpleJson.parse(json), "manifest");
  }

  private static void assertSchemaMatches(final Map<String, Object> manifest) {
    final Map<String, Object> schema = asMap(manifest.get("schema"), "schema");
    assertEquals(
        "schema.payload mismatch",
        PAYLOAD_SCHEMA,
        requireString(schema.get("payload"), "schema.payload"));
    assertEquals(
        "schema.signed mismatch",
        SIGNED_SCHEMA,
        requireString(schema.get("signed"), "schema.signed"));
  }

  private static SigningKeyInfo parseSigningKey(final Map<String, Object> manifest) {
    final Map<String, Object> signingKey = asMap(manifest.get("signing_key"), "signing_key");
    final String algorithm =
        requireString(signingKey.get("algorithm"), "signing_key.algorithm").toLowerCase();
    if (!"ed25519".equals(algorithm)) {
      throw new IllegalStateException("Unsupported signing key algorithm: " + algorithm);
    }
    final String publicKeyHex =
        requireString(signingKey.get("public_key_hex"), "signing_key.public_key_hex");
    final byte[] publicKey = hexToBytes(publicKeyHex, "signing_key.public_key_hex");
    if (publicKey.length != 32) {
      throw new IllegalStateException(
          "signing_key.public_key_hex must be 32 bytes (found " + publicKey.length + ")");
    }
    return new SigningKeyInfo(publicKey);
  }

  private static void verifySignature(
      final String name,
      final SigningKeyInfo signingKey,
      final byte[] payloadBytes,
      final byte[] signature) {
    if (signature.length != 64) {
      throw new IllegalStateException(
          name + ": signature length mismatch (expected 64, found " + signature.length + ")");
    }
    final byte[] hash = canonicalHashBytes(payloadBytes);
    final Ed25519Signer verifier = new Ed25519Signer();
    verifier.init(false, new Ed25519PublicKeyParameters(signingKey.publicKey(), 0));
    verifier.update(hash, 0, hash.length);
    if (!verifier.verifySignature(signature)) {
      throw new IllegalStateException(name + ": signature verification failed");
    }
  }

  private static void validateCompatibility(
      final String name,
      final byte[] payloadBytes,
      final byte[] signedBytes,
      final SigningKeyInfo signingKey) {
    final TransactionPayload payload;
    try {
      payload = PAYLOAD_CODEC.decodeTransaction(payloadBytes);
    } catch (final Exception ex) {
      throw new IllegalStateException(name + ": failed to decode payload", ex);
    }
    final byte[] reencoded;
    try {
      reencoded = PAYLOAD_CODEC.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException(name + ": failed to re-encode payload", ex);
    }
    assertArrayEquals(
        name + ": payload bytes differ after Android re-encoding",
        payloadBytes,
        reencoded);

    final SignedParts signedParts = decodeSignedParts(name, signedBytes);
    assertArrayEquals(
        name + ": signed payload mismatch vs manifest payload bytes",
        payloadBytes,
        signedParts.payloadBytes());
    verifySignature(name, signingKey, payloadBytes, signedParts.signature());
    final SignedTransaction signed =
        new SignedTransaction(payloadBytes, signedParts.signature(), new byte[0], SIGNED_SCHEMA);
    final byte[] encodedSigned;
    try {
      encodedSigned = SignedTransactionEncoder.encode(signed);
    } catch (final Exception ex) {
      throw new IllegalStateException(name + ": failed to encode signed transaction", ex);
    }
    assertArrayEquals(
        name + ": signed bytes differ after Android re-encoding",
        signedBytes,
        encodedSigned);

    final byte[] versioned;
    try {
      versioned = SignedTransactionEncoder.encodeVersioned(signed);
    } catch (final Exception ex) {
      throw new IllegalStateException(name + ": failed to encode versioned signed transaction", ex);
    }
    assertEquals(
        name + ": versioned length mismatch",
        signedBytes.length + 1,
        versioned.length);
    assertEquals(name + ": versioned prefix mismatch", VERSION_BYTE, versioned[0]);
    assertArrayEquals(
        name + ": versioned payload mismatch",
        signedBytes,
        Arrays.copyOfRange(versioned, 1, versioned.length));

    compatChecked++;
  }

  private static SignedParts decodeSignedParts(final String name, final byte[] signedBytes) {
    final NoritoDecoder decoder = new NoritoDecoder(signedBytes, NoritoHeader.MINOR_VERSION);
    final byte[] signatureField = readField(decoder, name + ".signed.signature");
    final byte[] payloadField = readField(decoder, name + ".signed.payload");
    final byte[] attachmentsField = readField(decoder, name + ".signed.attachments");
    final byte[] multisigField = readField(decoder, name + ".signed.multisig_signatures");
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(name + ": signed transaction payload has trailing bytes");
    }
    final byte[] signature = decodeSignature(name, signatureField);
    decodeOptionField(name + ".signed.attachments", attachmentsField);
    decodeOptionField(name + ".signed.multisig_signatures", multisigField);
    return new SignedParts(signature, payloadField);
  }

  private static byte[] decodeSignature(final String name, final byte[] signatureField) {
    final NoritoDecoder fieldDecoder = new NoritoDecoder(signatureField, NoritoHeader.MINOR_VERSION);
    final byte[] inner = readField(fieldDecoder, name + ".signed.signature.inner");
    if (fieldDecoder.remaining() != 0) {
      throw new IllegalStateException(name + ": signature field has trailing bytes");
    }
    final NoritoDecoder decoder = new NoritoDecoder(inner, NoritoHeader.MINOR_VERSION);
    final byte[] signature = BYTE_VECTOR_ADAPTER.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(name + ": signature payload has trailing bytes");
    }
    return signature;
  }

  private static Optional<byte[]> decodeOptionField(final String name, final byte[] fieldBytes) {
    final NoritoDecoder decoder = new NoritoDecoder(fieldBytes, NoritoHeader.MINOR_VERSION);
    final int tag = decoder.readByte();
    if (tag == 0) {
      if (decoder.remaining() != 0) {
        throw new IllegalStateException(name + ": Option::None has trailing bytes");
      }
      return Optional.empty();
    }
    if (tag != 1) {
      throw new IllegalStateException(name + ": invalid Option tag " + tag);
    }
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalStateException(name + ": Option payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(name + ": Option payload has trailing bytes");
    }
    return Optional.of(payload);
  }

  private static byte[] readField(final NoritoDecoder decoder, final String field) {
    final boolean compact = decoder.compactLenActive();
    final long length = decoder.readLength(compact);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalStateException(field + " length too large: " + length);
    }
    return decoder.readBytes((int) length);
  }

  private static final class SignedParts {
    private final byte[] signature;
    private final byte[] payloadBytes;

    private SignedParts(final byte[] signature, final byte[] payloadBytes) {
      this.signature = signature;
      this.payloadBytes = payloadBytes;
    }

    private byte[] signature() {
      return Arrays.copyOf(signature, signature.length);
    }

    private byte[] payloadBytes() {
      return Arrays.copyOf(payloadBytes, payloadBytes.length);
    }
  }

  private static final class SigningKeyInfo {
    private final byte[] publicKey;

    private SigningKeyInfo(final byte[] publicKey) {
      this.publicKey = publicKey;
    }

    private byte[] publicKey() {
      return Arrays.copyOf(publicKey, publicKey.length);
    }
  }

  private static Map<String, TransactionPayloadFixtures.Fixture> loadPayloadFixtures(final Path baseDir)
      throws Exception {
    Path payloadPath = resolveFixturePath("transaction_payloads.json");
    if (baseDir != null) {
      final Path local = baseDir.resolve("transaction_payloads.json");
      if (Files.exists(local)) {
        payloadPath = local;
      }
    }
    final List<TransactionPayloadFixtures.Fixture> fixtures = TransactionPayloadFixtures.load(payloadPath);
    final Map<String, TransactionPayloadFixtures.Fixture> map = new LinkedHashMap<>();
    for (TransactionPayloadFixtures.Fixture fixture : fixtures) {
      map.put(fixture.name(), fixture);
    }
    return map;
  }

  private static Path resolveFixturePath(final String filename) {
    final String[] candidates =
        new String[] {
          "java/iroha_android/src/test/resources/" + filename,
          "src/test/resources/" + filename,
          "../src/test/resources/" + filename,
          "../../src/test/resources/" + filename
        };
    for (final String candidate : candidates) {
      final Path path = Paths.get(candidate);
      if (Files.exists(path)) {
        return path;
      }
    }
    throw new IllegalStateException(
        "Fixture not found: " + Paths.get(candidates[0]).toAbsolutePath());
  }

  private static boolean optionalLongEquals(final Optional<Long> optional, final Long expected) {
    if (!optional.isPresent() && expected == null) {
      return true;
    }
    return optional.isPresent() && expected != null && Objects.equals(optional.get(), expected);
  }

  private static boolean optionalIntEquals(final Optional<Integer> optional, final Long expected) {
    if (!optional.isPresent() && expected == null) {
      return true;
    }
    return optional.isPresent()
        && expected != null
        && Objects.equals(optional.get().longValue(), expected.longValue());
  }

  private static Map<String, Object> asMap(final Object value, final String field) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException("Expected object for " + field);
    }
    @SuppressWarnings("unchecked")
    final Map<?, ?> raw = (Map<?, ?>) value;
    final Map<String, Object> copy = new LinkedHashMap<>();
    raw.forEach((key, v) -> copy.put(Objects.toString(key), v));
    return copy;
  }

  private static List<Object> asList(final Object value, final String field) {
    if (!(value instanceof List)) {
      throw new IllegalStateException("Expected array for " + field);
    }
    @SuppressWarnings("unchecked")
    final List<Object> list = (List<Object>) value;
    return new ArrayList<>(list);
  }

  private static String requireString(final Object value, final String field) {
    if (!(value instanceof String) || ((String) value).trim().isEmpty()) {
      throw new IllegalStateException(field + " must be a non-empty string");
    }
    return (String) value;
  }

  private static long requireNumber(final Object value, final String field) {
    if (!(value instanceof Number)) {
      throw new IllegalStateException(field + " must be a number");
    }
    return ((Number) value).longValue();
  }

  private static Long optionalNumber(final Object value) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof Number)) {
      throw new IllegalStateException("Expected number or null");
    }
    return ((Number) value).longValue();
  }

  private static byte[] hexToBytes(final String hex, final String field) {
    if (hex == null) {
      throw new IllegalArgumentException(field + " must not be null");
    }
    final String normalized = hex.trim();
    if (normalized.length() % 2 != 0) {
      throw new IllegalArgumentException(field + " must have even length");
    }
    final byte[] out = new byte[normalized.length() / 2];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(normalized.charAt(i * 2), 16);
      final int lo = Character.digit(normalized.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " has invalid hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

}
