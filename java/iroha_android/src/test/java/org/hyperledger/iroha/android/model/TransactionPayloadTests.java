package org.hyperledger.iroha.android.model;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.util.Arrays;
import java.util.Collections;
import java.util.Locale;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.LocalSigningContext;
import org.hyperledger.iroha.android.client.VerifyingKeyTransactionDraft;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.junit.Test;

/** Admission checks performed while authoring transaction payloads. */
public final class TransactionPayloadTests {
  private static final String CONTRACT_ADDRESS =
      "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh";
  private static final String TEST_NETWORK_ID_LITERAL =
      "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";
  private static final NetworkId TEST_NETWORK_ID = NetworkId.parse(TEST_NETWORK_ID_LITERAL);
  private static final String AUTHORITY = sampleAuthority();

  @Test
  public void networkIdMustBeSetExplicitlyForAuthoredAndDecodedPayloads() {
    final TransactionPayload.Builder missingNetworkId =
        TransactionPayload.builder()
            .setAuthority(AUTHORITY)
            .setInstructions(Collections.emptyList())
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()));
    assertIllegalState(missingNetworkId::build, "networkId must be set explicitly");
    assertIllegalState(
        missingNetworkId::buildDecodedForCodec, "networkId must be set explicitly");
  }

  @Test
  public void authorityMustBeSetExplicitlyForAuthoredAndDecodedPayloads() {
    final TransactionPayload.Builder missingAuthority =
        TransactionPayload.builder()
            .setNetworkId(TEST_NETWORK_ID)
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
                  .setNetworkId(TEST_NETWORK_ID)
                  .setAuthority(AUTHORITY)
                  .setExecutable(executable)
                  .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
                  .build(),
          "feePayment.gasLimit is required");
      assertNotNull(
          TransactionPayload.builder()
              .setNetworkId(TEST_NETWORK_ID)
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
            .setNetworkId(TEST_NETWORK_ID)
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
  public void transactionPayloadRequiresNominalNetworkId() {
    for (final String invalid :
        Arrays.asList(
            TEST_NETWORK_ID_LITERAL.toUpperCase(Locale.ROOT),
            TEST_NETWORK_ID_LITERAL.substring(0, TEST_NETWORK_ID_LITERAL.length() - 1) + "8",
            TEST_NETWORK_ID_LITERAL.substring(0, TEST_NETWORK_ID_LITERAL.length() - 1),
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
            "unicode-\u00e9",
            "network-label")) {
      assertIllegalArgument(() -> NetworkId.parse(invalid), "NetworkId");
    }
    assertNotNull(
        TransactionPayload.builder()
            .setNetworkId(TEST_NETWORK_ID)
            .setAuthority(AUTHORITY)
            .setInstructions(Collections.emptyList())
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
            .build());
  }

  @Test
  public void publicTransactionApiDoesNotExposeLegacyChainNames() {
    for (final Class<?> type :
        Arrays.asList(
            NetworkId.class,
            TransactionPayload.class,
            TransactionPayload.Builder.class,
            LocalSigningContext.class,
            VerifyingKeyTransactionDraft.class)) {
      Arrays.stream(type.getDeclaredMethods())
          .filter(method -> java.lang.reflect.Modifier.isPublic(method.getModifiers()))
          .forEach(
              method ->
                  assertFalse(
                      type.getName() + "." + method.getName(),
                      method.getName().toLowerCase(Locale.ROOT).contains("chain")));
      Arrays.stream(type.getDeclaredFields())
          .filter(field -> java.lang.reflect.Modifier.isPublic(field.getModifiers()))
          .forEach(
              field ->
                  assertFalse(
                      type.getName() + "." + field.getName(),
                      field.getName().toLowerCase(Locale.ROOT).contains("chain")));
      Arrays.stream(type.getDeclaredConstructors())
          .flatMap(constructor -> Arrays.stream(constructor.getParameterTypes()))
          .forEach(
              parameter ->
                  assertFalse(
                      type.getName() + " constructor " + parameter.getName(),
                      parameter.getSimpleName().toLowerCase(Locale.ROOT).contains("chain")));
    }
  }

  @Test
  public void nonceSupportsTheFullNonzeroU32Range() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setNetworkId(TEST_NETWORK_ID)
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
