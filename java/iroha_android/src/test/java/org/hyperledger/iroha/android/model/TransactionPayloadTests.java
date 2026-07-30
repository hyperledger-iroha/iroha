package org.hyperledger.iroha.android.model;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.util.Arrays;
import java.util.Collections;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.junit.Test;

/** Admission checks performed while authoring transaction payloads. */
public final class TransactionPayloadTests {
  private static final String CONTRACT_ADDRESS =
      "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8";
  private static final String TAIRA_CHAIN_ID =
      "fc56984b-2be7-431d-840e-21514d1883f0";
  private static final String AUTHORITY = sampleAuthority();

  @Test
  public void chainIdMustBeSetExplicitlyForAuthoredAndDecodedPayloads() {
    final TransactionPayload.Builder missingChainId =
        TransactionPayload.builder()
            .setAuthority(AUTHORITY)
            .setInstructions(Collections.emptyList())
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()));
    assertIllegalState(missingChainId::build, "chainId must be set explicitly");
    assertIllegalState(
        missingChainId::buildDecodedForCodec, "chainId must be set explicitly");
  }

  @Test
  public void authorityMustBeSetExplicitlyForAuthoredAndDecodedPayloads() {
    final TransactionPayload.Builder missingAuthority =
        TransactionPayload.builder()
            .setChainId(TAIRA_CHAIN_ID)
            .setInstructions(Collections.emptyList())
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()));
    assertIllegalState(missingAuthority::build, "authority must be set explicitly");
    assertIllegalState(
        missingAuthority::buildDecodedForCodec, "authority must be set explicitly");
  }

  @Test
  public void vmAndContractExecutablesRequireGasBeforeSigning() {
    final ContractInvocation invocation =
        new ContractInvocation(CONTRACT_ADDRESS, repeatedByte(0x01, 32), "run", null);
    final Executable[] executables = {
      Executable.ivm(new byte[] {1}),
      Executable.contractCall(invocation),
      Executable.batch(
          Collections.singletonList(ExecutableBatchItem.contractCall(invocation)))
    };

    for (final Executable executable : executables) {
      assertIllegalState(
          () ->
              TransactionPayload.builder()
                  .setChainId(TAIRA_CHAIN_ID)
                  .setAuthority(AUTHORITY)
                  .setExecutable(executable)
                  .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
                  .build(),
          "feePayment.gasLimit is required");
      assertNotNull(
          TransactionPayload.builder()
              .setChainId(TAIRA_CHAIN_ID)
              .setAuthority(AUTHORITY)
              .setExecutable(executable)
              .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
              .build());
    }
  }

  @Test
  public void nativeInstructionsDoNotRequireGas() {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(TAIRA_CHAIN_ID)
            .setAuthority(AUTHORITY)
            .setInstructions(
                Collections.singletonList(
                    InstructionBox.fromWirePayload("iroha.test", new byte[] {1})))
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
            .build();
    assertNotNull(payload);
    assertEquals(Long.valueOf(100_000L), payload.timeToLiveMs().orElse(null));
  }

  @Test
  public void chainIdUsesCanonicalBoundedAsciiGrammar() {
    for (final String invalid :
        Arrays.asList(
            "-leading",
            "trailing_",
            "contains space",
            "unicode-\u00e9",
            repeatText("x", 129))) {
      assertIllegalArgument(
          () -> TransactionPayload.builder().setChainId(invalid), "chainId");
    }
    assertNotNull(
        TransactionPayload.builder()
            .setChainId("iroha.mainnet:v1-alpha_2")
            .setAuthority(AUTHORITY)
            .setInstructions(Collections.emptyList())
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
            .build());
  }

  @Test
  public void nonceSupportsTheFullNonzeroU32Range() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(TAIRA_CHAIN_ID)
            .setAuthority(AUTHORITY)
            .setInstructions(Collections.emptyList())
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
            .setNonce(0xffff_ffffL)
            .build();
    final NoritoJavaCodecAdapter adapter =
        new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final TransactionPayload decoded =
        adapter.decodeTransaction(adapter.encodeTransaction(payload));

    assertEquals(Long.valueOf(0xffff_ffffL), decoded.nonce().orElse(null));
    assertIllegalArgument(() -> TransactionPayload.builder().setNonce(0L), "nonzero u32");
    assertIllegalArgument(
        () -> TransactionPayload.builder().setNonce(0x1_0000_0000L), "nonzero u32");
  }

  private static byte[] repeatedByte(final int value, final int length) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static String repeatText(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      result.append(value);
    }
    return result.toString();
  }

  private static String sampleAuthority() {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x21), "ed25519")
          .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build sample authority", ex);
    }
  }

  private static void assertIllegalState(final Runnable action, final String messageFragment) {
    try {
      action.run();
      fail("Expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      assertTrue(expected.getMessage().contains(messageFragment));
    }
  }

  private static void assertIllegalArgument(final Runnable action, final String messageFragment) {
    try {
      action.run();
      fail("Expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assertTrue(expected.getMessage().contains(messageFragment));
    }
  }
}
