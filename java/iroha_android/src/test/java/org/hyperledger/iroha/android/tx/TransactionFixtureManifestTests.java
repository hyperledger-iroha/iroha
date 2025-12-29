package org.hyperledger.iroha.android.tx;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.testing.SimpleJson;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;

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

  private TransactionFixtureManifestTests() {}

  public static void main(final String[] args) throws Exception {
    Path manifestPath =
        Path.of("java/iroha_android/src/test/resources/transaction_fixtures.manifest.json");
    if (!Files.exists(manifestPath)) {
      manifestPath = Path.of("src/test/resources/transaction_fixtures.manifest.json");
    }
    final String json = Files.readString(manifestPath, StandardCharsets.UTF_8);
    final Map<String, Object> manifest = asMap(SimpleJson.parse(json), "manifest");

    final Object fixturesValue = manifest.get("fixtures");
    final List<Object> fixtures = asList(fixturesValue, "fixtures");
    if (fixtures.isEmpty()) {
      throw new IllegalStateException("Manifest must contain at least one fixture");
    }

    // Map fixtures from transaction_payloads.json by name for cross checks.
    final Map<String, TransactionPayloadFixtures.Fixture> payloadFixtures =
        loadPayloadFixtures(manifestPath.getParent());

    for (Object entry : fixtures) {
      validateFixture(entry, manifestPath, payloadFixtures);
    }

    System.out.println("[IrohaAndroid] Transaction fixture manifest tests passed.");
  }

  private static void validateFixture(
      final Object entry,
      final Path manifestPath,
      final Map<String, TransactionPayloadFixtures.Fixture> payloadFixtures)
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
        ? Path.of(encodedFile)
        : baseDir.resolve(encodedFile).normalize();
    if (!Files.exists(encodedPath)) {
      throw new IllegalStateException(name + ": encoded file not found: " + encodedPath);
    }
    final byte[] encodedBytes = Files.readAllBytes(encodedPath);
    assert encodedBytes.length == encodedLen
        : name + ": encoded_len mismatch (expected " + encodedLen + ", found " + encodedBytes.length + ")";
    final String actualBase64 = Base64.getEncoder().encodeToString(encodedBytes);
    assert Objects.equals(actualBase64, payloadBase64)
        : name + ": payload_base64 does not match encoded file";

    // Validate the signed payload looks well-formed base64 and matches the advertised length.
    final byte[] payloadBytes = decodeBase64(payloadBase64, name + ".payload_base64");
    final byte[] signedBytes = decodeBase64(signedBase64, name + ".signed_base64");
    assert signedBytes.length == signedLen
        : name + ": signed_len mismatch (expected " + signedLen + ", found " + signedBytes.length + ")";

    assert HEX_64.matcher(payloadHash).matches()
        : name + ": payload_hash must be a 64-character hex string";
    assert HEX_64.matcher(signedHash).matches()
        : name + ": signed_hash must be a 64-character hex string";
    final String computedPayloadHash = canonicalHashHex(payloadBytes);
    assert computedPayloadHash.equals(payloadHash)
        : name + ": payload hash mismatch (expected " + payloadHash + ", computed " + computedPayloadHash + ")";
    final String computedSignedHash = SignedTransactionHasher.hashCanonicalHex(signedBytes);
    assert computedSignedHash.equals(signedHash)
        : name + ": signed hash mismatch (expected " + signedHash + ", computed " + computedSignedHash + ")";

    final TransactionPayloadFixtures.Fixture payloadFixture = payloadFixtures.get(name);
    if (payloadFixture != null) {
      payloadFixture.encoded().ifPresent(encoded -> {
        assert Objects.equals(encoded, payloadBase64)
            : name + ": manifest payload mismatch vs transaction_payloads entry";
      });
      if (!payloadFixture.isDecodable()) {
        return;
      }
      final TransactionPayload payload = payloadFixture.toPayload();
      assert Objects.equals(payload.chainId(), chain)
          : name + ": chain mismatch vs transaction_payloads";
      assert Objects.equals(payload.authority(), authority)
          : name + ": authority mismatch vs transaction_payloads";
      assert optionalLongEquals(payload.timeToLiveMs(), ttl)
          : name + ": TTL mismatch vs transaction_payloads";
      assert optionalIntEquals(payload.nonce(), nonce)
          : name + ": nonce mismatch vs transaction_payloads";
      return;
    }

  }

  private static byte[] decodeBase64(final String value, final String fieldName) {
    try {
      return Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(fieldName + " is not valid base64", ex);
    }
  }

  private static String canonicalHashHex(final byte[] data) {
    byte[] digest;
    try {
      final MessageDigest md = MessageDigest.getInstance(HASH_ALGORITHM);
      digest = md.digest(data);
    } catch (final NoSuchAlgorithmException ex) {
      digest = Blake2b.digest(data);
    }
    digest[digest.length - 1] |= 1;
    final StringBuilder builder = new StringBuilder(digest.length * 2);
    for (byte b : digest) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }

  private static Map<String, TransactionPayloadFixtures.Fixture> loadPayloadFixtures(final Path baseDir)
      throws Exception {
    Path payloadPath = Path.of("java/iroha_android/src/test/resources/transaction_payloads.json");
    if (!Files.exists(payloadPath)) {
      payloadPath = Path.of("src/test/resources/transaction_payloads.json");
    }
    if (baseDir != null && Files.exists(baseDir.resolve("transaction_payloads.json"))) {
      payloadPath = baseDir.resolve("transaction_payloads.json");
    }
    final List<TransactionPayloadFixtures.Fixture> fixtures = TransactionPayloadFixtures.load(payloadPath);
    final Map<String, TransactionPayloadFixtures.Fixture> map = new LinkedHashMap<>();
    for (TransactionPayloadFixtures.Fixture fixture : fixtures) {
      map.put(fixture.name(), fixture);
    }
    return map;
  }

  private static boolean optionalLongEquals(final Optional<Long> optional, final Long expected) {
    if (optional.isEmpty() && expected == null) {
      return true;
    }
    return optional.isPresent() && expected != null && Objects.equals(optional.get(), expected);
  }

  private static boolean optionalIntEquals(final Optional<Integer> optional, final Long expected) {
    if (optional.isEmpty() && expected == null) {
      return true;
    }
    return optional.isPresent()
        && expected != null
        && Objects.equals(optional.get().longValue(), expected.longValue());
  }

  private static Map<String, Object> asMap(final Object value, final String field) {
    if (!(value instanceof Map<?, ?> raw)) {
      throw new IllegalStateException("Expected object for " + field);
    }
    final Map<String, Object> copy = new LinkedHashMap<>();
    raw.forEach((key, v) -> copy.put(Objects.toString(key), v));
    return copy;
  }

  private static List<Object> asList(final Object value, final String field) {
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException("Expected array for " + field);
    }
    return List.copyOf(list);
  }

  private static String requireString(final Object value, final String field) {
    if (!(value instanceof String str) || str.isBlank()) {
      throw new IllegalStateException(field + " must be a non-empty string");
    }
    return str;
  }

  private static long requireNumber(final Object value, final String field) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(field + " must be a number");
    }
    return number.longValue();
  }

  private static Long optionalNumber(final Object value) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof Number number)) {
      throw new IllegalStateException("Expected number or null");
    }
    return number.longValue();
  }
}
