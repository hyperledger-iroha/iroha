package org.hyperledger.iroha.android.alias;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.junit.Test;

/** Cross-language golden coverage for exact prepared-transaction authentication. */
public final class PreparedTransactionSignatureFixtureTests {
  private static final String FIXTURE_PATH =
      "fixtures/prepared_transactions/prepared_transaction_signature_v1.json";
  private static final String FAUCET_ASSET_DEFINITION_ID =
      "4rPeAP6jAjiLVZThZYwwPRBuQagt";
  private static final String FAUCET_AMOUNT = "5";

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
    final FeePaymentIntent expectedFeePayment =
        FeePaymentIntent.authority(Collections.emptyList());
    AccountOnboardingPreparedVerifier.requireValidPrepared(
        prepared,
        prepared.receipt().body().request(),
        independentlyParsedPrepared.receipt(),
        independentlyParsedPrepared.binding(),
        expectedFeePayment,
        networkId,
        string(preparedVector, "signer_account_id"));
    expectIllegalArgument(
        () ->
            AccountOnboardingPreparedVerifier.requireValidPrepared(
                prepared,
                prepared.receipt().body().request(),
                independentlyParsedPrepared.receipt(),
                independentlyParsedPrepared.binding(),
                FeePaymentIntent.authority(Collections.emptyList(), 1L),
                networkId,
                string(preparedVector, "signer_account_id")));
    final Map<String, Object> submitJson = new LinkedHashMap<>();
    submitJson.put("schema", PreparedTransactionSubmitResponseV1.SCHEMA);
    submitJson.put("binding", prepared.binding().toJsonMap());
    submitJson.put("operation", prepared.operation());
    submitJson.put("transaction_hash_hex", prepared.transactionHashHex());
    submitJson.put("outcome", PreparedTransactionOutcomeV1.PENDING.wireValue());
    final PreparedTransactionSubmitResponseV1 submitResponse =
        AccountOnboardingJsonParser.parseSubmitResponse(
            JsonEncoder.encode(submitJson).getBytes(StandardCharsets.UTF_8));
    AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
        submitResponse, prepared, expectedFeePayment, 200);
    AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
        submitResponse, prepared, expectedFeePayment, 202);
    expectIllegalArgument(
        () ->
            AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
                submitResponse,
                prepared,
                FeePaymentIntent.authority(Collections.emptyList(), 1L),
                200));
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
                incorrectlyAppliedAtAcceptance, prepared, expectedFeePayment, 202));

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
                expectedFeePayment,
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
        FeePaymentIntent.authority(Collections.emptyList()),
        networkId,
        authority);
  }

  private static void assertFaucetVector(
      final Map<String, Object> vector, final NetworkId networkId) throws Exception {
    final Map<String, Object> response = object(vector.get("response"), "faucet response");
    final AccountFaucetPreparedTransactionV1 prepared =
        AccountOnboardingJsonParser.parseFaucetPrepareResponse(
            JsonEncoder.encode(response).getBytes(StandardCharsets.UTF_8));
    assertVectorTranscript(
        vector,
        PreparedTransactionSignatureV1.faucetPrepared(prepared),
        prepared.serverSignature());
    assertEquals(prepared.claim().semanticHashHex(), prepared.semanticHashHex());
    final FeePaymentIntent expectedFeePayment =
        FeePaymentIntent.authority(Collections.emptyList());
    final AccountFaucetPolicyV1 policy =
        new AccountFaucetPolicyV1(
            string(vector, "signer_account_id"),
            FAUCET_ASSET_DEFINITION_ID,
            NumericV1.QuantityValue.parseCanonical(FAUCET_AMOUNT));
    AccountFaucetPreparedVerifier.requireValidPrepared(
        prepared,
        prepared.claim(),
        prepared.binding(),
        expectedFeePayment,
        policy,
        networkId);
    final PreparedTransactionSubmitResponseV1 submitResponse =
        new PreparedTransactionSubmitResponseV1(
            PreparedTransactionSubmitResponseV1.SCHEMA,
            prepared.binding(),
            prepared.operation(),
            prepared.transactionHashHex(),
            PreparedTransactionOutcomeV1.PENDING);
    AccountFaucetPreparedVerifier.requireValidSubmitResponse(
        submitResponse, prepared, expectedFeePayment, policy, networkId, 202);
    expectIllegalArgument(
        () ->
            AccountFaucetPreparedVerifier.requireValidPrepared(
                prepared,
                prepared.claim(),
                prepared.binding(),
                FeePaymentIntent.authority(Collections.emptyList(), 1L),
                policy,
                networkId));
    final AccountFaucetPolicyV1[] substitutedPolicies = {
      new AccountFaucetPolicyV1(
          prepared.accountId(),
          FAUCET_ASSET_DEFINITION_ID,
          NumericV1.QuantityValue.parseCanonical(FAUCET_AMOUNT)),
      new AccountFaucetPolicyV1(
          policy.faucetAuthority(),
          otherAssetDefinition(),
          NumericV1.QuantityValue.parseCanonical(FAUCET_AMOUNT)),
      new AccountFaucetPolicyV1(
          policy.faucetAuthority(),
          FAUCET_ASSET_DEFINITION_ID,
          NumericV1.QuantityValue.parseCanonical("6"))
    };
    for (final AccountFaucetPolicyV1 substitutedPolicy : substitutedPolicies) {
      expectIllegalArgument(
          () ->
              AccountFaucetPreparedVerifier.requireValidPrepared(
                  prepared,
                  prepared.claim(),
                  prepared.binding(),
                  expectedFeePayment,
                  substitutedPolicy,
                  networkId));
      expectIllegalArgument(
          () ->
              AccountFaucetPreparedVerifier.requireValidSubmitResponse(
                  submitResponse,
                  prepared,
                  expectedFeePayment,
                  substitutedPolicy,
                  networkId,
                  202));
    }
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

  private static String otherAssetDefinition() {
    final byte[] bytes = new byte[16];
    for (int index = 0; index < bytes.length; index++) bytes[index] = (byte) (index + 1);
    bytes[6] = 0x47;
    bytes[8] = (byte) 0x89;
    return AssetDefinitionIdEncoder.encodeFromBytes(bytes);
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
