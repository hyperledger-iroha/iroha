package org.hyperledger.iroha.android.tx;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;

/**
 * Regenerates the full shared transaction fixture family from the deterministic fixture signing
 * seed.
 *
 * <p>The canonical, Android, Python, and Swift copies are one reviewed family. Swift's separately
 * generated {@code swift_*.norito} parity fixtures are preserved.
 */
public final class TransactionFixtureResourceExporter {

  private static final String SIGNING_SEED_HEX =
      "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

  private static final String ANDROID_PAYLOADS =
      "java/iroha_android/src/test/resources/transaction_payloads.json";
  private static final String ANDROID_MANIFEST =
      "java/iroha_android/src/test/resources/transaction_fixtures.manifest.json";

  private static final List<String> PAYLOAD_JSON_OUTPUTS =
      List.of(
          ANDROID_PAYLOADS,
          "fixtures/norito_rpc/transaction_payloads.json",
          "python/iroha_python/tests/fixtures/transaction_payloads.json",
          "IrohaSwift/Fixtures/transaction_payloads.json");

  private static final List<String> MANIFEST_OUTPUTS =
      List.of(
          ANDROID_MANIFEST,
          "fixtures/norito_rpc/transaction_fixtures.manifest.json",
          "python/iroha_python/tests/fixtures/transaction_fixtures.manifest.json",
          "IrohaSwift/Fixtures/transaction_fixtures.manifest.json");

  private static final List<String> NORITO_OUTPUT_DIRS =
      List.of(
          "java/iroha_android/src/test/resources",
          "fixtures/norito_rpc",
          "python/iroha_python/tests/fixtures",
          "IrohaSwift/Fixtures");
  private static final String SWIFT_NORITO_OUTPUT_DIR = "IrohaSwift/Fixtures";

  private static final NoritoJavaCodecAdapter CODEC = new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT);

  private TransactionFixtureResourceExporter() {}

  public static void main(final String[] args) throws Exception {
    final Path repoRoot = resolveRepoRoot();
    final Path payloadPath = repoRoot.resolve(ANDROID_PAYLOADS);
    final List<Object> rawEntries = parseFixtureEntries(payloadPath);
    final List<TransactionPayloadFixtures.Fixture> fixtures = TransactionPayloadFixtures.load(payloadPath);
    if (rawEntries.size() != fixtures.size()) {
      throw new IllegalStateException(
          "fixture entry count mismatch: json="
              + rawEntries.size()
              + ", decoded="
              + fixtures.size());
    }

    final Ed25519PrivateKeyParameters privateKey =
        new Ed25519PrivateKeyParameters(hexToBytes(SIGNING_SEED_HEX), 0);
    final byte[] publicKey = privateKey.generatePublicKey().getEncoded();
    final String authority = canonicalAuthority(publicKey);

    final List<Map<String, Object>> manifestFixtures = new ArrayList<>(fixtures.size());
    final List<FixtureOutput> outputs = new ArrayList<>(fixtures.size());
    for (int i = 0; i < fixtures.size(); i++) {
      final TransactionPayloadFixtures.Fixture fixture = fixtures.get(i);
      final Object rawEntry = rawEntries.get(i);
      final FixtureOutput output =
          regenerateFixture(fixture, rawEntry, authority, privateKey, publicKey);
      rewriteRawEntry(rawEntry, output);
      manifestFixtures.add(output.toManifestEntry());
      outputs.add(output);
    }

    final String renderedPayloads = renderJson(rawEntries) + "\n";
    for (final String relative : PAYLOAD_JSON_OUTPUTS) {
      writeString(repoRoot.resolve(relative), renderedPayloads);
    }

    final Map<String, Object> manifest = new LinkedHashMap<>();
    manifest.put("fixtures", manifestFixtures);
    final String renderedManifest = renderJson(manifest) + "\n";
    for (final String relative : MANIFEST_OUTPUTS) {
      writeString(repoRoot.resolve(relative), renderedManifest);
    }

    for (final String relativeDir : NORITO_OUTPUT_DIRS) {
      writeNoritoFamily(repoRoot, relativeDir, outputs);
    }

    System.out.println(
        "[IrohaAndroid] Regenerated "
            + outputs.size()
            + " transaction fixtures with authority "
            + authority);
  }

  private static void writeNoritoFamily(
      final Path repoRoot, final String relativeDir, final List<FixtureOutput> outputs)
      throws IOException {
    final Path dir = repoRoot.resolve(relativeDir);
    Files.createDirectories(dir);
    final Set<String> expected = new HashSet<>();
    for (final FixtureOutput output : outputs) {
      expected.add(output.name + ".norito");
    }
    try (var entries = Files.list(dir)) {
      for (final Path existing : entries.toList()) {
        final String name = existing.getFileName().toString();
        if (!name.endsWith(".norito") || expected.contains(name)) {
          continue;
        }
        if (relativeDir.equals(SWIFT_NORITO_OUTPUT_DIR) && name.startsWith("swift_")) {
          continue;
        }
        Files.delete(existing);
      }
    }
    for (final FixtureOutput output : outputs) {
      Files.write(dir.resolve(output.name + ".norito"), output.payloadBytes);
    }
  }

  private static FixtureOutput regenerateFixture(
      final TransactionPayloadFixtures.Fixture fixture,
      final Object rawEntry,
      final String authority,
      final Ed25519PrivateKeyParameters privateKey,
      final byte[] publicKey)
      throws Exception {
    final TransactionPayload sourcePayload = fixture.toPayload();
    final TransactionPayload payload;
    final byte[] payloadBytes;
    if ("ivm_transfer".equals(fixture.name())) {
      if (!sourcePayload.authority().equals(authority)) {
        throw new IllegalStateException(
            "ivm_transfer: historical gasless payload authority cannot be rewritten");
      }
      payload = sourcePayload;
      payloadBytes =
          decodeCanonicalBase64(
              fixture
                  .encoded()
                  .orElseThrow(
                      () ->
                          new IllegalStateException(
                              "ivm_transfer: historical encoded payload is required")),
              "ivm_transfer.encoded");
      return historicalFixtureOutput(fixture.name(), payload, payloadBytes, rawEntry);
    } else {
      payload = sourcePayload.toBuilder().setAuthority(authority).build();
      payloadBytes = CODEC.encodeTransaction(payload);
    }
    return fixtureOutput(fixture.name(), payload, payloadBytes, privateKey, publicKey);
  }

  private static FixtureOutput historicalFixtureOutput(
      final String name,
      final TransactionPayload payload,
      final byte[] payloadBytes,
      final Object rawEntry) {
    final Map<String, Object> entry = requireMap(rawEntry, name);
    final byte[] signedBytes =
        decodeCanonicalBase64(requireEntryString(entry, "signed_base64", name), name + ".signed");
    if (!containsSubsequence(signedBytes, payloadBytes)) {
      throw new IllegalStateException(name + ": signed bytes do not contain the payload bytes");
    }
    final String payloadBase64 = Base64.getEncoder().encodeToString(payloadBytes);
    final String signedBase64 = Base64.getEncoder().encodeToString(signedBytes);
    final String payloadHash = toHex(IrohaHash.prehash(payloadBytes));
    final String signedHash = hashHistoricalCanonicalSignedBytes(signedBytes);
    if (!payloadHash.equals(requireEntryString(entry, "payload_hash", name))
        || !signedHash.equals(requireEntryString(entry, "signed_hash", name))) {
      throw new IllegalStateException(name + ": historical fixture hash drift");
    }
    return new FixtureOutput(
        name,
        payload.chainId(),
        payload.authority(),
        payload.creationTimeMs(),
        payload.timeToLiveMs().orElse(null),
        payload.nonce().orElse(null),
        payloadBytes,
        signedBytes,
        payloadBase64,
        signedBase64,
        payloadHash,
        signedHash);
  }

  private static String hashHistoricalCanonicalSignedBytes(final byte[] signedBytes) {
    final byte[] compactLength = SignedTransactionHasher.encodeCompactLength(signedBytes.length);
    final byte[] external = new byte[4 + compactLength.length + signedBytes.length];
    System.arraycopy(compactLength, 0, external, 4, compactLength.length);
    System.arraycopy(signedBytes, 0, external, 4 + compactLength.length, signedBytes.length);
    return toHex(IrohaHash.prehash(external));
  }

  private static FixtureOutput fixtureOutput(
      final String name,
      final TransactionPayload payload,
      final byte[] payloadBytes,
      final Ed25519PrivateKeyParameters privateKey,
      final byte[] publicKey)
      throws Exception {
    final byte[] signature = signPayload(privateKey, payloadBytes);
    final SignedTransaction signed =
        SignedTransaction.builder()
            .setEncodedPayload(payloadBytes)
            .setSignature(signature)
            .setPublicKey(publicKey)
            .setSchemaName(CODEC.schemaName())
            .build();
    final byte[] signedBytes = SignedTransactionEncoder.encode(signed);
    final String payloadBase64 = Base64.getEncoder().encodeToString(payloadBytes);
    final String signedBase64 = Base64.getEncoder().encodeToString(signedBytes);

    return new FixtureOutput(
        name,
        payload.chainId(),
        payload.authority(),
        payload.creationTimeMs(),
        payload.timeToLiveMs().orElse(null),
        payload.nonce().orElse(null),
        payloadBytes,
        signedBytes,
        payloadBase64,
        signedBase64,
        toHex(IrohaHash.prehash(payloadBytes)),
        SignedTransactionHasher.hashCanonicalHex(signedBytes));
  }

  private static byte[] decodeCanonicalBase64(final String value, final String context) {
    try {
      final byte[] decoded = Base64.getDecoder().decode(value);
      if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
        throw new IllegalStateException(context + " contains non-canonical base64");
      }
      return decoded;
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(context + " contains invalid base64", ex);
    }
  }

  private static String requireEntryString(
      final Map<String, Object> entry, final String field, final String context) {
    final Object value = entry.get(field);
    if (!(value instanceof String string) || string.isEmpty()) {
      throw new IllegalStateException(context + " entry is missing " + field);
    }
    return string;
  }

  private static boolean containsSubsequence(final byte[] haystack, final byte[] needle) {
    if (needle.length == 0 || needle.length > haystack.length) {
      return false;
    }
    for (int start = 0; start <= haystack.length - needle.length; start++) {
      int index = 0;
      while (index < needle.length && haystack[start + index] == needle[index]) {
        index++;
      }
      if (index == needle.length) {
        return true;
      }
    }
    return false;
  }

  private static void rewriteRawEntry(final Object rawEntry, final FixtureOutput output) {
    final Map<String, Object> entry = requireMap(rawEntry, "fixture");
    entry.put("authority", output.authority);
    entry.put("chain", output.chain);
    entry.put("creation_time_ms", output.creationTimeMs);
    entry.put("nonce", output.nonce == null ? null : output.nonce.longValue());
    entry.put("time_to_live_ms", output.timeToLiveMs);
    entry.put("encoded", output.payloadBase64);
    entry.put("payload_base64", output.payloadBase64);
    entry.put("payload_hash", output.payloadHash);
    entry.put("signed_base64", output.signedBase64);
    entry.put("signed_hash", output.signedHash);

    final Object payloadValue = entry.get("payload");
    if (payloadValue instanceof Map<?, ?>) {
      final Map<String, Object> payload = requireMap(payloadValue, output.name + ".payload");
      payload.put("authority", output.authority);
      payload.put("chain", output.chain);
      payload.put("creation_time_ms", output.creationTimeMs);
      payload.put("nonce", output.nonce == null ? null : output.nonce.longValue());
      payload.put("time_to_live_ms", output.timeToLiveMs);
    }
  }

  private static byte[] signPayload(
      final Ed25519PrivateKeyParameters privateKey, final byte[] payloadBytes) {
    final byte[] hash = IrohaHash.prehash(payloadBytes);
    final Ed25519Signer signer = new Ed25519Signer();
    signer.init(true, privateKey);
    signer.update(hash, 0, hash.length);
    return signer.generateSignature();
  }

  private static String canonicalAuthority(final byte[] publicKey)
      throws AccountAddress.AccountAddressException {
    return AccountAddress.fromAccount(publicKey, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }

  private static List<Object> parseFixtureEntries(final Path path) throws IOException {
    final String json = Files.readString(path, StandardCharsets.UTF_8);
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof List<?> list)) {
      throw new IllegalStateException("transaction_payloads.json must be a JSON array");
    }
    return new ArrayList<>(list);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> requireMap(final Object value, final String context) {
    if (!(value instanceof Map<?, ?> raw)) {
      throw new IllegalStateException(context + " must be a JSON object");
    }
    return (Map<String, Object>) raw;
  }

  private static void writeString(final Path path, final String value) throws IOException {
    Files.createDirectories(path.getParent());
    Files.writeString(path, value, StandardCharsets.UTF_8);
  }

  private static String renderJson(final Object value) {
    final StringBuilder builder = new StringBuilder();
    writeJson(builder, value, 0);
    return builder.toString();
  }

  @SuppressWarnings("unchecked")
  private static void writeJson(final StringBuilder builder, final Object value, final int indent) {
    if (value == null) {
      builder.append("null");
      return;
    }
    if (value instanceof String string) {
      writeString(builder, string);
      return;
    }
    if (value instanceof Number number) {
      builder.append(number.toString());
      return;
    }
    if (value instanceof Boolean bool) {
      builder.append(bool.booleanValue() ? "true" : "false");
      return;
    }
    if (value instanceof List<?> list) {
      if (list.isEmpty()) {
        builder.append("[]");
        return;
      }
      builder.append("[\n");
      for (int i = 0; i < list.size(); i++) {
        indent(builder, indent + 2);
        writeJson(builder, list.get(i), indent + 2);
        if (i + 1 < list.size()) {
          builder.append(',');
        }
        builder.append('\n');
      }
      indent(builder, indent);
      builder.append(']');
      return;
    }
    if (value instanceof Map<?, ?> map) {
      if (map.isEmpty()) {
        builder.append("{}");
        return;
      }
      builder.append("{\n");
      int index = 0;
      for (final Map.Entry<String, Object> entry : ((Map<String, Object>) map).entrySet()) {
        indent(builder, indent + 2);
        writeString(builder, entry.getKey());
        builder.append(": ");
        writeJson(builder, entry.getValue(), indent + 2);
        if (index + 1 < map.size()) {
          builder.append(',');
        }
        builder.append('\n');
        index++;
      }
      indent(builder, indent);
      builder.append('}');
      return;
    }
    throw new IllegalStateException("unsupported JSON value: " + value.getClass());
  }

  private static void writeString(final StringBuilder builder, final String value) {
    builder.append('"');
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      switch (c) {
        case '"' -> builder.append("\\\"");
        case '\\' -> builder.append("\\\\");
        case '\b' -> builder.append("\\b");
        case '\f' -> builder.append("\\f");
        case '\n' -> builder.append("\\n");
        case '\r' -> builder.append("\\r");
        case '\t' -> builder.append("\\t");
        default -> {
          if (c < 0x20) {
            builder.append(String.format("\\u%04x", (int) c));
          } else {
            builder.append(c);
          }
        }
      }
    }
    builder.append('"');
  }

  private static void indent(final StringBuilder builder, final int indent) {
    for (int i = 0; i < indent; i++) {
      builder.append(' ');
    }
  }

  private static Path resolveRepoRoot() {
    Path current = Paths.get("").toAbsolutePath().normalize();
    while (current != null) {
      if (Files.exists(current.resolve("Cargo.toml")) && Files.exists(current.resolve("roadmap.md"))) {
        return current;
      }
      current = current.getParent();
    }
    throw new IllegalStateException("failed to locate repository root");
  }

  private static byte[] hexToBytes(final String hex) {
    if ((hex.length() & 1) != 0) {
      throw new IllegalArgumentException("hex string must have even length");
    }
    final byte[] out = new byte[hex.length() / 2];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
    }
    return out;
  }

  private static String toHex(final byte[] value) {
    final StringBuilder builder = new StringBuilder(value.length * 2);
    for (final byte b : value) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }

  private static final class FixtureOutput {
    private final String name;
    private final String chain;
    private final String authority;
    private final long creationTimeMs;
    private final Long timeToLiveMs;
    private final Long nonce;
    private final byte[] payloadBytes;
    private final byte[] signedBytes;
    private final String payloadBase64;
    private final String signedBase64;
    private final String payloadHash;
    private final String signedHash;

    private FixtureOutput(
        final String name,
        final String chain,
        final String authority,
        final long creationTimeMs,
        final Long timeToLiveMs,
        final Long nonce,
        final byte[] payloadBytes,
        final byte[] signedBytes,
        final String payloadBase64,
        final String signedBase64,
        final String payloadHash,
        final String signedHash) {
      this.name = name;
      this.chain = chain;
      this.authority = authority;
      this.creationTimeMs = creationTimeMs;
      this.timeToLiveMs = timeToLiveMs;
      this.nonce = nonce;
      this.payloadBytes = payloadBytes;
      this.signedBytes = signedBytes;
      this.payloadBase64 = payloadBase64;
      this.signedBase64 = signedBase64;
      this.payloadHash = payloadHash;
      this.signedHash = signedHash;
    }

    private Map<String, Object> toManifestEntry() {
      final Map<String, Object> entry = new LinkedHashMap<>();
      entry.put("authority", authority);
      entry.put("chain", chain);
      entry.put("creation_time_ms", creationTimeMs);
      entry.put("encoded_file", name + ".norito");
      entry.put("encoded_len", payloadBytes.length);
      entry.put("name", name);
      entry.put("nonce", nonce == null ? null : nonce.longValue());
      entry.put("payload_base64", payloadBase64);
      entry.put("payload_hash", payloadHash);
      entry.put("signed_base64", signedBase64);
      entry.put("signed_hash", signedHash);
      entry.put("signed_len", signedBytes.length);
      entry.put("time_to_live_ms", timeToLiveMs);
      return entry;
    }
  }
}
