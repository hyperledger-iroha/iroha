package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineNoteV2Test {

  private OfflineNoteV2Test() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    offlineNoteV2ModelsMatchRustNoritoVectors();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    qrFixtureUsesSdkTextPrefix();
    System.out.println("[IrohaAndroid] OfflineNoteV2Test passed.");
  }

  private static void certificateSigningBytesMatchRustVector() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.KeyCertificateV2 sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final Map<String, Object> certificates = obj(obj(fixture, "chain_vectors"), "certificates");

    assertEquals(
        string(certificates, "sender_payload_base64"),
        base64(sender.signingBytes()),
        "sender certificate payload");
    assertEquals(
        string(certificates, "sender_payload_hash"),
        hex(sender.payloadHash()),
        "sender certificate payload hash");
  }

  private static void offlineNoteV2ModelsMatchRustNoritoVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");

    assertEquals(
        string(obj(chain, "issue"), "norito_base64"),
        base64(issue(fixture).noritoEncoded()),
        "issue norito");
    assertEquals(
        string(obj(chain, "audit"), "norito_base64"),
        base64(audit(fixture).noritoEncoded()),
        "audit norito");
    assertEquals(
        string(obj(chain, "redeem"), "norito_base64"),
        base64(redeem(fixture).noritoEncoded()),
        "redeem norito");
  }

  private static void publicInputHashesMatchRustVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);

    assertEquals(
        string(obj(chain, "audit"), "public_inputs_hash"),
        hex(audit.publicInputsHash()),
        "audit public inputs hash");
    assertEquals(
        string(obj(chain, "redeem"), "public_inputs_hash"),
        hex(redeem.publicInputsHash()),
        "redeem public inputs hash");
    audit.validateProofBinding();
    redeem.validateProofBinding();
  }

  private static void proofBindingRejectsMismatch() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.RecursiveProofV2 badProof =
        new OfflineNoteV2.RecursiveProofV2(
            OfflineNoteV2.hash("wrong-public-inputs".getBytes(StandardCharsets.UTF_8)),
            new OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".getBytes(StandardCharsets.UTF_8)));
    final OfflineNoteV2.RedeemV2 forged =
        new OfflineNoteV2.RedeemV2(
            redeem.sourceNoteCommitment(),
            redeem.inputNullifiers(),
            redeem.senderKeyCertificate(),
            redeem.recipient(),
            redeem.assetId(),
            redeem.amount(),
            badProof);

    assertThrows(forged::validateProofBinding, "proof binding mismatch should throw");
  }

  private static void qrFixtureUsesSdkTextPrefix() throws Exception {
    final Map<String, Object> fountain = obj(loadFixture(), "fountain_qr_v1");
    assertEquals("iroha:qr1:", string(fountain, "frame_prefix"), "fountain QR prefix");
  }

  private static OfflineNoteV2.IssueV2 issue(final Map<String, Object> fixture) {
    final Map<String, Object> chainIssue = obj(obj(fixture, "chain_vectors"), "issue");
    return new OfflineNoteV2.IssueV2(
        hexBytes(string(chainIssue, "note_commitment")),
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
        string(chainIssue, "asset_id"),
        string(chainIssue, "amount"));
  }

  private static OfflineNoteV2.RedeemV2 redeem(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    return new OfflineNoteV2.RedeemV2(
        hexBytes(string(vector, "source_note_commitment")),
        hexList(vector, "input_nullifiers"),
        certificate(obj(payment, "recipient_key_certificate")),
        string(payment, "recipient_account_id"),
        string(vector, "asset_id"),
        string(vector, "amount"),
        new OfflineNoteV2.RecursiveProofV2(
            hexBytes(string(vector, "public_inputs_hash")),
            new OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static OfflineNoteV2.AuditBundleV2 audit(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "audit");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final List<OfflineNoteV2.IssuedClaimV2> inputClaims = new ArrayList<>();
    for (final Object item : list(payment, "input_claims")) {
      inputClaims.add(issuedClaim(asMap(item, "input claim")));
    }
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims = new ArrayList<>();
    for (final Object item : list(payment, "output_claims")) {
      outputClaims.add(auditOutputClaim(asMap(item, "output claim")));
    }
    return new OfflineNoteV2.AuditBundleV2(
        hexBytes(string(vector, "token_id")),
        certificate(obj(payment, "sender_key_certificate")),
        hexList(vector, "input_nullifiers"),
        inputClaims,
        hexList(vector, "output_commitments"),
        outputClaims,
        new OfflineNoteV2.RecursiveProofV2(
            hexBytes(string(vector, "public_inputs_hash")),
            new OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-audit-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static OfflineNoteV2.KeyCertificateV2 certificate(final Map<String, Object> json) {
    return new OfflineNoteV2.KeyCertificateV2(
        intValue(json, "version"),
        string(json, "platform"),
        string(json, "key_id"),
        string(json, "device_id"),
        string(json, "account_id"),
        base64Bytes(string(json, "public_key")),
        string(json, "assertion_scheme"),
        string(json, "assertion_key_algorithm"),
        base64Bytes(string(json, "assertion_public_key")),
        nullableInt(json, "assertion_usage_count_limit"),
        bool(json, "one_use"),
        base64Bytes(string(json, "issuer_signature_base64")));
  }

  private static OfflineNoteV2.IssuedClaimV2 issuedClaim(final Map<String, Object> json) {
    return new OfflineNoteV2.IssuedClaimV2(
        string(json, "domain"),
        hexBytes(string(json, "note_commitment")),
        hexBytes(string(json, "key_certificate_payload_hash")),
        string(json, "asset_id"),
        string(json, "amount"));
  }

  private static OfflineNoteV2.AuditOutputClaimV2 auditOutputClaim(
      final Map<String, Object> json) {
    return new OfflineNoteV2.AuditOutputClaimV2(
        hexBytes(string(json, "note_commitment")),
        certificate(obj(json, "key_certificate")),
        string(json, "asset_definition_id") + "#" + string(json, "account_id"),
        string(json, "amount"));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> loadFixture() throws Exception {
    Path cursor = Paths.get("").toAbsolutePath();
    while (cursor != null) {
      final Path candidate = cursor.resolve("fixtures/offline/interop_contract_v2.json");
      if (Files.exists(candidate)) {
        final String json = Files.readString(candidate);
        return (Map<String, Object>) JsonParser.parse(json);
      }
      cursor = cursor.getParent();
    }
    throw new AssertionError("fixtures/offline/interop_contract_v2.json was not found");
  }

  private static Map<String, Object> obj(final Map<String, Object> map, final String key) {
    return asMap(map.get(key), key);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> asMap(final Object value, final String label) {
    if (!(value instanceof Map)) {
      throw new AssertionError(label + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof List)) {
      throw new AssertionError(key + " must be an array");
    }
    return (List<Object>) value;
  }

  private static List<byte[]> hexList(final Map<String, Object> map, final String key) {
    final List<byte[]> values = new ArrayList<>();
    for (final Object item : list(map, key)) {
      values.add(hexBytes((String) item));
    }
    return values;
  }

  private static String string(final Map<String, Object> map, final String key) {
    return (String) map.get(key);
  }

  private static boolean bool(final Map<String, Object> map, final String key) {
    return (Boolean) map.get(key);
  }

  private static int intValue(final Map<String, Object> map, final String key) {
    return ((Number) map.get(key)).intValue();
  }

  private static Integer nullableInt(final Map<String, Object> map, final String key) {
    final Number value = (Number) map.get(key);
    return value == null ? null : value.intValue();
  }

  private static String base64(final byte[] bytes) {
    return Base64.getEncoder().encodeToString(bytes);
  }

  private static byte[] base64Bytes(final String value) {
    return Base64.getDecoder().decode(value);
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xFF));
    }
    return builder.toString();
  }

  private static byte[] hexBytes(final String value) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[value.length() / 2];
    for (int offset = 0; offset < value.length(); offset += 2) {
      out[offset / 2] = (byte) Integer.parseInt(value.substring(offset, offset + 2), 16);
    }
    return out;
  }

  private static void assertEquals(
      final String expected, final String actual, final String message) {
    if (!expected.equals(actual)) {
      throw new AssertionError(message + ": expected " + expected + " but got " + actual);
    }
  }

  private static void assertThrows(final Runnable action, final String message) {
    try {
      action.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }
}
