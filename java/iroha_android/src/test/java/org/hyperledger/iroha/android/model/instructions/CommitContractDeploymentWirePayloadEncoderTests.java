package org.hyperledger.iroha.android.model.instructions;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertThrows;

import java.math.BigInteger;
import java.util.Arrays;
import org.junit.Test;

/** Canonical hash admission for atomic contract deployment payloads. */
public final class CommitContractDeploymentWirePayloadEncoderTests {
  private static final String CONTRACT_ADDRESS =
      "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";

  /** The Iroha hash marker is mandatory on both deployment encode and field decode. */
  @Test
  public void deploymentCodeHashRequiresCanonicalMarkerOnEncodeAndDecode() {
    final byte[] canonicalHash = new byte[32];
    Arrays.fill(canonicalHash, (byte) 0xab);

    CommitContractDeploymentWirePayloadEncoder.encode(
        BigInteger.ZERO,
        CONTRACT_ADDRESS,
        "ab".repeat(32),
        "audit_contract",
        null,
        null);
    assertArrayEquals(
        canonicalHash,
        CommitContractDeploymentWirePayloadEncoder.decodeCanonicalCodeHashBytes(canonicalHash));

    assertThrows(
        IllegalArgumentException.class,
        () ->
            CommitContractDeploymentWirePayloadEncoder.encode(
                BigInteger.ZERO,
                CONTRACT_ADDRESS,
                "ab".repeat(31) + "aa",
                "audit_contract",
                null,
                null));
    final byte[] evenMarker = canonicalHash.clone();
    evenMarker[evenMarker.length - 1] = (byte) 0xaa;
    assertThrows(
        IllegalArgumentException.class,
        () ->
            CommitContractDeploymentWirePayloadEncoder.decodeCanonicalCodeHashBytes(evenMarker));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            CommitContractDeploymentWirePayloadEncoder.decodeCanonicalCodeHashBytes(
                Arrays.copyOf(canonicalHash, 33)));
  }
}
