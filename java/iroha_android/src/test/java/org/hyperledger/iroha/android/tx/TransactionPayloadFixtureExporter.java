package org.hyperledger.iroha.android.tx;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.testing.SimpleJson;

/**
 * Strict fixture exporter for Android/JVM transaction payload parity tests.
 *
 * <p>The structured {@code payload} object is the only source of truth. Existing encoded blobs are
 * overwritten; missing or non-decodable payload objects are treated as fixture bugs.
 */
public final class TransactionPayloadFixtureExporter {

  private static final String SIGNING_SEED_HEX =
      "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
  private static final String SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1";

  private TransactionPayloadFixtureExporter() {}

  public static void main(final String[] args) throws Exception {
    final Path path = TransactionPayloadFixtures.resolveFixturePath();
    final Path outputDir = path.getParent();
    if (outputDir == null) {
      throw new IllegalStateException("transaction_payloads.json must have a parent directory");
    }

    final Object parsed =
        SimpleJson.parse(new String(Files.readAllBytes(path), StandardCharsets.UTF_8));
    if (!(parsed instanceof List)) {
      throw new IllegalStateException("Fixture root must be an array");
    }
    @SuppressWarnings("unchecked")
    final List<Object> entries = (List<Object>) parsed;

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final Ed25519PrivateKeyParameters privateKey =
        new Ed25519PrivateKeyParameters(hexToBytes(SIGNING_SEED_HEX), 0);
    final byte[] publicKey = privateKey.generatePublicKey().getEncoded();
    final List<Object> manifestFixtures = new java.util.ArrayList<>(entries.size());

    for (Object entry : entries) {
      if (!(entry instanceof Map)) {
        throw new IllegalStateException("Fixture entries must be objects");
      }
      @SuppressWarnings("unchecked")
      final Map<String, Object> map = (Map<String, Object>) entry;
      final TransactionPayloadFixtures.Fixture fixture =
          TransactionPayloadFixtures.Fixture.fromObject(map);
      if (!fixture.isDecodable()) {
        throw new IllegalStateException(fixture.name() + ": fixture missing structured payload");
      }
      final TransactionPayload payload = fixture.toPayload();
      final byte[] encoded = adapter.encodeTransaction(payload);
      final byte[] payloadHash = IrohaHash.prehash(encoded);
      final byte[] signature = sign(privateKey, payloadHash);
      final SignedTransaction signed =
          new SignedTransaction(encoded, signature, publicKey, SIGNED_SCHEMA);
      final byte[] signedBytes = SignedTransactionEncoder.encode(signed);

      final String payloadBase64 = Base64.getEncoder().encodeToString(encoded);
      final String signedBase64 = Base64.getEncoder().encodeToString(signedBytes);
      final String payloadHashHex = hex(payloadHash);
      final String signedHashHex = SignedTransactionHasher.hashCanonicalHex(signedBytes);
      final String encodedFile = fixture.name() + ".norito";

      Files.write(outputDir.resolve(encodedFile), encoded);
      rewriteFixtureEntry(map, fixture, payloadBase64, payloadHashHex, signedBase64, signedHashHex);
      manifestFixtures.add(
          manifestEntry(
              fixture,
              encodedFile,
              encoded.length,
              payloadBase64,
              payloadHashHex,
              signedBase64,
              signedHashHex,
              signedBytes.length));
      System.out.println(fixture.name() + "=" + payloadBase64);
    }

    Files.writeString(path, toPrettyJson(entries) + "\n", StandardCharsets.UTF_8);

    final Map<String, Object> manifest = new LinkedHashMap<>();
    manifest.put("fixtures", manifestFixtures);
    Files.writeString(
        outputDir.resolve("transaction_fixtures.manifest.json"),
        toPrettyJson(manifest) + "\n",
        StandardCharsets.UTF_8);
  }

  private static void rewriteFixtureEntry(
      final Map<String, Object> map,
      final TransactionPayloadFixtures.Fixture fixture,
      final String payloadBase64,
      final String payloadHash,
      final String signedBase64,
      final String signedHash) {
    final Object payload = map.get("payload");
    if (payload == null) {
      throw new IllegalStateException(fixture.name() + ": fixture missing payload object");
    }
    map.clear();
    map.put("authority", fixture.authority());
    map.put("chain", fixture.chain());
    map.put("creation_time_ms", fixture.creationTimeMs());
    map.put("encoded", payloadBase64);
    map.put("name", fixture.name());
    map.put("nonce", fixture.nonce().orElse(null));
    map.put("payload", payload);
    map.put("payload_base64", payloadBase64);
    map.put("payload_hash", payloadHash);
    map.put("signed_base64", signedBase64);
    map.put("signed_hash", signedHash);
    map.put("time_to_live_ms", fixture.timeToLiveMs().orElse(null));
  }

  private static Map<String, Object> manifestEntry(
      final TransactionPayloadFixtures.Fixture fixture,
      final String encodedFile,
      final int encodedLen,
      final String payloadBase64,
      final String payloadHash,
      final String signedBase64,
      final String signedHash,
      final int signedLen) {
    final Map<String, Object> entry = new LinkedHashMap<>();
    entry.put("authority", fixture.authority());
    entry.put("chain", fixture.chain());
    entry.put("creation_time_ms", fixture.creationTimeMs());
    entry.put("encoded_file", encodedFile);
    entry.put("encoded_len", encodedLen);
    entry.put("name", fixture.name());
    entry.put("nonce", fixture.nonce().orElse(null));
    entry.put("payload_base64", payloadBase64);
    entry.put("payload_hash", payloadHash);
    entry.put("signed_base64", signedBase64);
    entry.put("signed_hash", signedHash);
    entry.put("signed_len", signedLen);
    entry.put("time_to_live_ms", fixture.timeToLiveMs().orElse(null));
    return entry;
  }

  private static byte[] sign(
      final Ed25519PrivateKeyParameters privateKey, final byte[] payloadHash) {
    final Ed25519Signer signer = new Ed25519Signer();
    signer.init(true, privateKey);
    signer.update(payloadHash, 0, payloadHash.length);
    return signer.generateSignature();
  }

  private static byte[] hexToBytes(final String hex) {
    if ((hex.length() & 1) != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[hex.length() / 2];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(hex.charAt(i * 2), 16);
      final int lo = Character.digit(hex.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException("invalid hex character");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (byte b : bytes) {
      out.append(String.format("%02x", b));
    }
    return out.toString();
  }

  private static String toPrettyJson(final Object value) {
    final StringBuilder out = new StringBuilder();
    writeJson(out, value, 0);
    return out.toString();
  }

  private static void writeJson(final StringBuilder out, final Object value, final int indent) {
    if (value == null) {
      out.append("null");
    } else if (value instanceof String string) {
      writeJsonString(out, string);
    } else if (value instanceof Number || value instanceof Boolean) {
      out.append(value);
    } else if (value instanceof Map<?, ?> map) {
      writeObject(out, map, indent);
    } else if (value instanceof List<?> list) {
      writeArray(out, list, indent);
    } else {
      writeJsonString(out, value.toString());
    }
  }

  private static void writeObject(
      final StringBuilder out, final Map<?, ?> map, final int indent) {
    out.append('{');
    if (!map.isEmpty()) {
      boolean first = true;
      for (Map.Entry<?, ?> entry : map.entrySet()) {
        if (!first) {
          out.append(',');
        }
        out.append('\n');
        appendIndent(out, indent + 2);
        writeJsonString(out, String.valueOf(entry.getKey()));
        out.append(": ");
        writeJson(out, entry.getValue(), indent + 2);
        first = false;
      }
      out.append('\n');
      appendIndent(out, indent);
    }
    out.append('}');
  }

  private static void writeArray(
      final StringBuilder out, final List<?> list, final int indent) {
    out.append('[');
    if (!list.isEmpty()) {
      for (int i = 0; i < list.size(); i++) {
        if (i > 0) {
          out.append(',');
        }
        out.append('\n');
        appendIndent(out, indent + 2);
        writeJson(out, list.get(i), indent + 2);
      }
      out.append('\n');
      appendIndent(out, indent);
    }
    out.append(']');
  }

  private static void writeJsonString(final StringBuilder out, final String value) {
    out.append('"');
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      switch (ch) {
        case '"':
          out.append("\\\"");
          break;
        case '\\':
          out.append("\\\\");
          break;
        case '\b':
          out.append("\\b");
          break;
        case '\f':
          out.append("\\f");
          break;
        case '\n':
          out.append("\\n");
          break;
        case '\r':
          out.append("\\r");
          break;
        case '\t':
          out.append("\\t");
          break;
        default:
          if (ch < 0x20) {
            out.append(String.format("\\u%04x", (int) ch));
          } else {
            out.append(ch);
          }
      }
    }
    out.append('"');
  }

  private static void appendIndent(final StringBuilder out, final int indent) {
    for (int i = 0; i < indent; i++) {
      out.append(' ');
    }
  }
}
