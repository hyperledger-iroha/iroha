package org.hyperledger.iroha.android.sccp;

import java.util.Arrays;

public final class TonSccpProverTests {
  private TonSccpProverTests() {}

  public static void main(final String[] args) {
    buildsTonMessageBodyBoc();
    proverRequiresLinkedProofEngine();
    proverWrapsExternalProofBytes();
    System.out.println("[IrohaAndroid] TON SCCP prover tests passed.");
  }

  private static void buildsTonMessageBodyBoc() {
    final byte[] body = TonSccpProver.buildMessageBodyBoc(sampleMessageBodyInput());
    final byte[] magic = {(byte) 0xb5, (byte) 0xee, (byte) 0x9c, 0x72};
    assert Arrays.equals(magic, Arrays.copyOfRange(body, 0, 4))
        : "TON message body must be a BOC";
    assert body.length
            > TonSccpProver.canonicalPublicInputsBytes(samplePublicInputs()).length
        : "BOC must carry refs beyond public inputs";

    final TonSccpProver.Submission submission =
        TonSccpProver.buildSubmission(sampleMessageBodyInput());
    assert TonSccpProver.MESSAGE_BODY_BOC_V1.equals(submission.envelopeEncoding())
        : "submission encoding must be TON BOC";
    assert Arrays.equals(body, submission.messageBodyBoc()) : "submission body must match BOC";
    assert submission.messageBodyBocHex().startsWith("0xb5ee9c72")
        : "submission hex must expose BOC magic";
  }

  private static void proverRequiresLinkedProofEngine() {
    boolean threw = false;
    try {
      new TonSccpProver().prove(sampleProofRequestInput());
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("not linked");
    }
    assert threw : "expected missing local prover to throw";
  }

  private static void proverWrapsExternalProofBytes() {
    final TonSccpProver prover =
        new TonSccpProver(
            null,
            request -> {
              assert TonSccpProver.CONTRACT_PROOF_BACKEND_V1.equals(request.backend())
                  : "backend must be TON";
              return new byte[] {1, 2, 3, 4};
            });

    final TonSccpProver.ProofResult result = prover.prove(sampleProofRequestInput());
    assert Arrays.equals(new byte[] {1, 2, 3, 4}, result.proofBytes())
        : "proof bytes must be preserved";
    assert "AQIDBA==".equals(result.proofBase64()) : "proof base64 must be exposed";
    assert result.requestHash().matches("0x[0-9a-f]{64}") : "request hash must be hex";
  }

  private static TonSccpProver.MessageBodyInput sampleMessageBodyInput() {
    return new TonSccpProver.MessageBodyInput(
        samplePublicInputs(),
        new byte[] {1, 2, 3, 4},
        new byte[] {5, 6, 7},
        repeat("bb", 32),
        repeat("56", 32),
        new byte[] {8, 9});
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput() {
    return new TonSccpProver.ProofRequestInput(samplePublicInputs(), new byte[] {5, 6, 7});
  }

  private static TonSccpProver.PublicInputsInput samplePublicInputs() {
    return new TonSccpProver.PublicInputsInput(
        repeat("dd", 32),
        repeat("ee", 32),
        repeat("12", 32),
        "19",
        repeat("aa", 32));
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }
}
