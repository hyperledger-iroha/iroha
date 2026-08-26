package org.hyperledger.iroha.android.alias;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.junit.Test;

/** Cross-language golden coverage for exact prepared-transaction authentication. */
public final class PreparedTransactionSignatureFixtureTests {
  private static final String FIXTURE_PATH =
      "fixtures/prepared_transactions/prepared_transaction_signature_v1.json";

  /** Authenticates prepared, proof-required, and faucet vectors against the Rust golden. */
  @Test
  public void sharedGoldenAuthenticatesPreparedProofRequiredAndFaucetVectors() throws Exception {
    final Map<String, Object> root =
        object(
            JsonParser.parse(
                new String(Files.readAllBytes(resolveFixture()), StandardCharsets.UTF_8)),
            "fixture");
    assertEquals(
        "iroha.taira.prepared-transaction-signature-fixture.v1", string(root, "schema"));
    assertEquals("u64_be", string(root, "frame_length_encoding"));
    assertEquals("iroha_blake2b_256", string(root, "digest_algorithm"));
    assertEquals(
        PreparedTransactionSignatureV1.TRANSCRIPT_SCHEMA, string(root, "transcript_schema"));
    final Map<String, Map<String, Object>> vectors = new LinkedHashMap<>();
    for (final Object value : array(root, "vectors")) {
      final Map<String, Object> vector = object(value, "vector");
      vectors.put(string(vector, "name"), vector);
    }

    final Map<String, Object> preparedVector = vectors.get("onboarding_prepared");
    final NetworkId networkId = NetworkId.parse(string(preparedVector, "network_id"));
    final AccountOnboardingPrepareResponseV1 parsedPrepared = parseResponse(preparedVector);
    assertTrue(parsedPrepared instanceof AccountOnboardingPreparedTransactionV1);
    final AccountOnboardingPreparedTransactionV1 prepared =
        (AccountOnboardingPreparedTransactionV1) parsedPrepared;
    expectIllegalArgument(
        () ->
            copyPrepared(
                prepared,
                prepared.binding(),
                "aa".repeat(32),
                prepared.signedTransactionWireHex(),
                prepared.serverSignature()));
    expectIllegalArgument(
        () ->
            new PreparedTransactionSubmitResponseV1(
                PreparedTransactionSubmitResponseV1.SCHEMA,
                prepared.binding(),
                prepared.operation(),
                "aa".repeat(32),
                PreparedTransactionOutcomeV1.PENDING));
    assertEquals(networkId, prepared.receipt().body().networkId());
    assertVectorTranscript(
        preparedVector,
        PreparedTransactionSignatureV1.onboardingPrepared(prepared),
        prepared.serverSignature());
    final AccountOnboardingPreparedTransactionV1 independentlyParsedPrepared =
        (AccountOnboardingPreparedTransactionV1) parseResponse(preparedVector);
    AccountOnboardingPreparedVerifier.requireValidPrepared(
        prepared,
        prepared.receipt().body().request(),
        independentlyParsedPrepared.receipt(),
        independentlyParsedPrepared.binding(),
        networkId,
        string(preparedVector, "signer_account_id"));
    final Map<String, Object> submitJson = new LinkedHashMap<>();
    submitJson.put("schema", PreparedTransactionSubmitResponseV1.SCHEMA);
    submitJson.put("binding", prepared.binding().toJsonMap());
    submitJson.put("operation", prepared.operation());
    submitJson.put("transaction_hash_hex", prepared.transactionHashHex());
    submitJson.put("outcome", PreparedTransactionOutcomeV1.PENDING.wireValue());
    final PreparedTransactionSubmitResponseV1 submitResponse =
        AccountOnboardingJsonParser.parseSubmitResponse(
            JsonEncoder.encode(submitJson).getBytes(StandardCharsets.UTF_8));
    AccountOnboardingPreparedVerifier.requireValidSubmitResponse(submitResponse, prepared, 200);
    AccountOnboardingPreparedVerifier.requireValidSubmitResponse(submitResponse, prepared, 202);
    final PreparedTransactionSubmitResponseV1 incorrectlyAppliedAtAcceptance =
        new PreparedTransactionSubmitResponseV1(
            PreparedTransactionSubmitResponseV1.SCHEMA,
            prepared.binding(),
            prepared.operation(),
            prepared.transactionHashHex(),
            PreparedTransactionOutcomeV1.APPLIED);
    expectIllegalArgument(
        () ->
            AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
                incorrectlyAppliedAtAcceptance, prepared, 202));

    final Map<String, Object> proofRequiredVector = vectors.get("onboarding_proof_required");
    assertEquals(networkId, NetworkId.parse(string(proofRequiredVector, "network_id")));
    final AccountOnboardingPrepareResponseV1 parsedProofRequired =
        parseResponse(proofRequiredVector);
    assertTrue(parsedProofRequired instanceof AccountOnboardingProofRequiredPrepareResponseV1);
    final AccountOnboardingProofRequiredPrepareResponseV1 proofRequired =
        (AccountOnboardingProofRequiredPrepareResponseV1) parsedProofRequired;
    assertEquals("ProofRequired", proofRequired.outcome());
    assertEquals("account_alias_current_state", proofRequired.proofKind());
    assertVectorTranscript(
        proofRequiredVector,
        PreparedTransactionSignatureV1.onboardingProofRequired(proofRequired),
        proofRequired.serverSignature());
    AccountOnboardingPreparedVerifier.requireValidProofRequired(
        proofRequired,
        prepared.receipt().body().request(),
        prepared.receipt(),
        ((AccountOnboardingProofRequiredPrepareResponseV1) parseResponse(proofRequiredVector))
            .binding(),
        networkId,
        string(proofRequiredVector, "signer_account_id"));
    final AccountOnboardingPlanRequestV1 substitutedRequest =
        new AccountOnboardingPlanRequestV1(
            prepared.receipt().body().request().alias(),
            prepared.receipt().body().request().accountId(),
            Collections.singletonList("CanSetKeyValueInAccount"));
    expectIllegalArgument(
        () ->
            AccountOnboardingPreparedVerifier.requireValidPrepared(
                prepared,
                substitutedRequest,
                prepared.receipt(),
                prepared.binding(),
                networkId,
                string(preparedVector, "signer_account_id")));
    expectIllegalArgument(
        () ->
            AccountOnboardingPreparedVerifier.requireValidProofRequired(
                proofRequired,
                substitutedRequest,
                prepared.receipt(),
                proofRequired.binding(),
                networkId,
                string(proofRequiredVector, "signer_account_id")));

    assertPreparedTamperRejected(
        prepared, networkId, string(preparedVector, "signer_account_id"));
    final Map<String, Object> faucetVector = vectors.get("faucet_prepared");
    assertFaucetVector(faucetVector, NetworkId.parse(string(faucetVector, "network_id")));
  }

  private static void assertPreparedTamperRejected(
      final AccountOnboardingPreparedTransactionV1 prepared,
      final NetworkId networkId,
      final String authority) {
    expectIllegalArgument(
        () ->
            verifyPrepared(
                copyPrepared(
                    prepared,
                    prepared.binding(),
                    prepared.transactionHashHex(),
                    prepared.signedTransactionWireHex(),
                    flipHex(prepared.serverSignature())),
                prepared,
                networkId,
                authority));
    expectIllegalArgument(
        () ->
            verifyPrepared(
                copyPrepared(
                    prepared,
                    prepared.binding(),
                    prepared.transactionHashHex(),
                    flipHex(prepared.signedTransactionWireHex()),
                    prepared.serverSignature()),
                prepared,
                networkId,
                authority));
    expectIllegalArgument(
        () ->
            verifyPrepared(
                copyPrepared(
                    prepared,
                    prepared.binding(),
                    flipHex(prepared.transactionHashHex()),
                    prepared.signedTransactionWireHex(),
                    prepared.serverSignature()),
                prepared,
                networkId,
                authority));
    final TairaPublicResetMutationBindingV1 binding = prepared.binding();
    final TairaPublicResetMutationBindingV1 alteredBinding =
        new TairaPublicResetMutationBindingV1(
            flipHex(binding.authorizationSha256()),
            binding.authorizationNonce(),
            binding.kind(),
            binding.phase(),
            binding.idempotencyKey(),
            binding.executionExpiresAtUnixMs());
    expectIllegalArgument(
        () ->
            verifyPrepared(
                copyPrepared(
                    prepared,
                    alteredBinding,
                    prepared.transactionHashHex(),
                    prepared.signedTransactionWireHex(),
                    prepared.serverSignature()),
                prepared,
                networkId,
                authority));
  }

  private static void verifyPrepared(
      final AccountOnboardingPreparedTransactionV1 candidate,
      final AccountOnboardingPreparedTransactionV1 source,
      final NetworkId networkId,
      final String authority) {
    AccountOnboardingPreparedVerifier.requireValidPrepared(
        candidate,
        source.receipt().body().request(),
        source.receipt(),
        candidate.binding(),
        networkId,
        authority);
  }

  private static void assertFaucetVector(
      final Map<String, Object> vector, final NetworkId networkId) throws Exception {
    final Map<String, Object> response = object(vector.get("response"), "faucet response");
    final byte[] transcript = faucetTranscript(response);
    assertVectorTranscript(vector, transcript, string(response, "server_signature"));
    final byte[] wire = decodeHex(string(response, "signed_transaction_wire_hex"));
    assertEquals(
        string(response, "signed_transaction_wire_sha256"),
        hex(MessageDigest.getInstance("SHA-256").digest(wire)));
    final SignedTransaction transaction = SignedTransactionEncoder.decodeVersioned(wire);
    assertArrayEquals(wire, SignedTransactionEncoder.encodeVersioned(transaction));
    assertEquals(
        string(response, "transaction_hash_hex"), SignedTransactionHasher.hashHex(transaction));
    final TransactionPayload payload = SignedTransactionEncoder.decodeCanonicalPayload(transaction);
    assertEquals(networkId, payload.networkId());
    assertTrue(
        AccountOnboardingReceiptVerifier.sameAccountIdentity(
            string(vector, "signer_account_id"), payload.authority()));
    assertTrue(
        AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
            payload.authority(),
            IrohaHash.prehash(transaction.encodedPayload()),
            transaction.signature()));
  }

  private static void assertVectorTranscript(
      final Map<String, Object> vector,
      final byte[] transcript,
      final String responseSignature) {
    assertEquals(string(vector, "transcript_hex"), hex(transcript));
    assertEquals(
        string(vector, "digest_hex"), hex(PreparedTransactionSignatureV1.digest(transcript)));
    assertEquals(string(vector, "server_signature_hex"), responseSignature.toLowerCase());
    assertTrue(
        AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
            string(vector, "signer_account_id"),
            PreparedTransactionSignatureV1.digest(transcript),
            responseSignature));
  }

  private static byte[] faucetTranscript(final Map<String, Object> response) {
    final Map<String, Object> binding = object(response.get("binding"), "faucet binding");
    final Map<String, Object> claim = object(response.get("claim"), "faucet claim");
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    frame(
        output,
        decodeHex(
            "69726f68613a74616972613a70726570617265642d7472616e73616374696f6e3a763100"));
    field(output, "transcript_schema", PreparedTransactionSignatureV1.TRANSCRIPT_SCHEMA);
    field(output, "envelope_schema", string(response, "schema"));
    field(output, "operation", string(response, "operation"));
    field(output, "binding.schema", string(binding, "schema"));
    field(output, "binding.authorization_sha256", string(binding, "authorization_sha256"));
    field(output, "binding.authorization_nonce", string(binding, "authorization_nonce"));
    field(output, "binding.kind", string(binding, "kind"));
    field(output, "binding.phase", string(binding, "phase"));
    field(output, "binding.idempotency_key", string(binding, "idempotency_key"));
    field(
        output,
        "binding.execution_expires_at_unix_ms",
        Long.toString(number(binding, "execution_expires_at_unix_ms")));
    field(output, "claim.account_id", string(claim, "account_id"));
    field(output, "claim.pow_anchor_height", optionalNumber(claim, "pow_anchor_height"));
    field(output, "claim.pow_nonce_hex", optionalString(claim, "pow_nonce_hex"));
    field(output, "semantic_hash_hex", string(response, "semantic_hash_hex"));
    field(output, "account_id", string(response, "account_id"));
    field(output, "asset_definition_id", string(response, "asset_definition_id"));
    field(output, "asset_id", string(response, "asset_id"));
    field(output, "amount", string(response, "amount"));
    field(output, "transaction_hash_hex", string(response, "transaction_hash_hex"));
    field(
        output,
        "signed_transaction_wire_sha256",
        string(response, "signed_transaction_wire_sha256"));
    field(
        output,
        "signed_transaction_wire",
        decodeHex(string(response, "signed_transaction_wire_hex")));
    return output.toByteArray();
  }

  private static AccountOnboardingPreparedTransactionV1 copyPrepared(
      final AccountOnboardingPreparedTransactionV1 source,
      final TairaPublicResetMutationBindingV1 binding,
      final String transactionHashHex,
      final String signedTransactionWireHex,
      final String serverSignature) {
    return new AccountOnboardingPreparedTransactionV1(
        source.schema(),
        binding,
        source.operation(),
        source.receipt(),
        source.semanticHashHex(),
        source.accountId(),
        source.alias(),
        source.disposition(),
        transactionHashHex,
        signedTransactionWireHex,
        source.signedTransactionWireSha256(),
        source.feePayment(),
        serverSignature);
  }

  private static AccountOnboardingPrepareResponseV1 parseResponse(
      final Map<String, Object> vector) {
    return AccountOnboardingJsonParser.parsePrepareResponse(
        JsonEncoder.encode(object(vector.get("response"), "response"))
            .getBytes(StandardCharsets.UTF_8));
  }

  private static void field(
      final ByteArrayOutputStream output, final String label, final String value) {
    field(output, label, value.getBytes(StandardCharsets.UTF_8));
  }

  private static void field(
      final ByteArrayOutputStream output, final String label, final byte[] value) {
    frame(output, label.getBytes(StandardCharsets.UTF_8));
    frame(output, value);
  }

  private static void frame(final ByteArrayOutputStream output, final byte[] value) {
    final long length = value.length;
    for (int shift = 56; shift >= 0; shift -= 8) {
      output.write((int) ((length >>> shift) & 0xffL));
    }
    output.write(value, 0, value.length);
  }

  private static String optionalNumber(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    return value == null ? "none" : "some:" + ((Number) value).longValue();
  }

  private static String optionalString(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    return value == null ? "none" : "some:" + value;
  }

  private static String flipHex(final String value) {
    return (Character.toLowerCase(value.charAt(0)) == '0' ? "1" : "0") + value.substring(1);
  }

  private static byte[] decodeHex(final String value) {
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] =
          (byte)
              ((Character.digit(value.charAt(index * 2), 16) << 4)
                  | Character.digit(value.charAt(index * 2 + 1), 16));
    }
    return bytes;
  }

  private static String hex(final byte[] bytes) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final char[] output = new char[bytes.length * 2];
    for (int index = 0; index < bytes.length; index++) {
      final int value = bytes[index] & 0xff;
      output[index * 2] = digits[value >>> 4];
      output[index * 2 + 1] = digits[value & 0xf];
    }
    return new String(output);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value, final String path) {
    if (!(value instanceof Map)) throw new AssertionError(path + " must be an object");
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> array(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof List)) throw new AssertionError(key + " must be an array");
    return (List<Object>) value;
  }

  private static String string(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof String)) throw new AssertionError(key + " must be a string");
    return (String) value;
  }

  private static long number(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof Number)) throw new AssertionError(key + " must be a number");
    return ((Number) value).longValue();
  }

  private static void expectIllegalArgument(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("tampered prepared envelope must be rejected");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static Path resolveFixture() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve(FIXTURE_PATH);
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError(FIXTURE_PATH + " was not found from the test working directory");
  }
}
