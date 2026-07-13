package org.hyperledger.iroha.android.model.instructions;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.fail;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.junit.Test;

/** Adversarial coverage for exact asset and RWA instruction quantities. */
public final class InstructionQuantityTests {
  private static final String SOURCE = TestAccountIds.ed25519Authority(0x51);
  private static final String DESTINATION = TestAccountIds.ed25519Authority(0x52);

  @Test
  public void assetAndRwaBuildersRejectAlternateQuantitySpellings() {
    for (final String quantity :
        new String[] {
          "", " ", "\t1", "1 ", "+1", "01", "1.", ".5", "1e0", "-1", "-0", "-0.0",
          "1.0", "1.2300", "0.0"
        }) {
      for (final Runnable construct : constructionAttempts(quantity)) {
        expectIllegalArgument(construct, "builder accepted '" + quantity + "'");
      }
    }
  }

  @Test
  public void assetAndRwaArgumentReadbackRejectsNoncanonicalQuantities() {
    for (final Runnable read : readbackAttempts("1.0")) {
      expectIllegalArgument(read, "argument readback accepted a noncanonical quantity");
    }
  }

  @Test
  public void assetAndRwaBuildersAcceptTheLosslessQuantityType() {
    final NumericV1.QuantityValue quantity = NumericV1.QuantityValue.parseCanonical("1.25");
    final List<String> values =
        Arrays.asList(
            MintAssetInstruction.builder().setAssetId("asset").setQuantity(quantity).build().quantity(),
            BurnAssetInstruction.builder().setAssetId("asset").setQuantity(quantity).build().quantity(),
            TransferAssetInstruction.builder()
                .setAssetId("asset")
                .setQuantity(quantity)
                .setDestinationAccountId(DESTINATION)
                .build()
                .quantity(),
            HoldRwaInstruction.builder().setRwaId("rwa").setQuantity(quantity).build().quantity(),
            ReleaseRwaInstruction.builder().setRwaId("rwa").setQuantity(quantity).build().quantity(),
            RedeemRwaInstruction.builder().setRwaId("rwa").setQuantity(quantity).build().quantity(),
            TransferRwaInstruction.builder()
                .setSourceAccountId(SOURCE)
                .setRwaId("rwa")
                .setQuantity(quantity)
                .setDestinationAccountId(DESTINATION)
                .build()
                .quantity(),
            ForceTransferRwaInstruction.builder()
                .setRwaId("rwa")
                .setQuantity(quantity)
                .setDestinationAccountId(DESTINATION)
                .build()
                .quantity());

    for (final String value : values) assertEquals("1.25", value);
  }

  private static List<Runnable> constructionAttempts(final String quantity) {
    return Arrays.asList(
        () -> MintAssetInstruction.builder().setAssetId("asset").setQuantity(quantity).build(),
        () -> BurnAssetInstruction.builder().setAssetId("asset").setQuantity(quantity).build(),
        () ->
            TransferAssetInstruction.builder()
                .setAssetId("asset")
                .setQuantity(quantity)
                .setDestinationAccountId(DESTINATION)
                .build(),
        () -> HoldRwaInstruction.builder().setRwaId("rwa").setQuantity(quantity).build(),
        () -> ReleaseRwaInstruction.builder().setRwaId("rwa").setQuantity(quantity).build(),
        () -> RedeemRwaInstruction.builder().setRwaId("rwa").setQuantity(quantity).build(),
        () ->
            TransferRwaInstruction.builder()
                .setSourceAccountId(SOURCE)
                .setRwaId("rwa")
                .setQuantity(quantity)
                .setDestinationAccountId(DESTINATION)
                .build(),
        () ->
            ForceTransferRwaInstruction.builder()
                .setRwaId("rwa")
                .setQuantity(quantity)
                .setDestinationAccountId(DESTINATION)
                .build());
  }

  private static List<Runnable> readbackAttempts(final String quantity) {
    return Arrays.asList(
        () -> MintAssetInstruction.fromArguments(arguments("asset", "asset", "quantity", quantity)),
        () -> BurnAssetInstruction.fromArguments(arguments("asset", "asset", "quantity", quantity)),
        () ->
            TransferAssetInstruction.fromArguments(
                arguments(
                    "asset", "asset", "quantity", quantity, "destination", DESTINATION)),
        () -> HoldRwaInstruction.fromArguments(arguments("rwa", "rwa", "quantity", quantity)),
        () -> ReleaseRwaInstruction.fromArguments(arguments("rwa", "rwa", "quantity", quantity)),
        () -> RedeemRwaInstruction.fromArguments(arguments("rwa", "rwa", "quantity", quantity)),
        () ->
            TransferRwaInstruction.fromArguments(
                arguments(
                    "source",
                    SOURCE,
                    "rwa",
                    "rwa",
                    "quantity",
                    quantity,
                    "destination",
                    DESTINATION)),
        () ->
            ForceTransferRwaInstruction.fromArguments(
                arguments("rwa", "rwa", "quantity", quantity, "destination", DESTINATION)));
  }

  private static Map<String, String> arguments(final String... pairs) {
    final Map<String, String> result = new LinkedHashMap<>();
    for (int index = 0; index < pairs.length; index += 2) {
      result.put(pairs[index], pairs[index + 1]);
    }
    return result;
  }

  private static void expectIllegalArgument(final Runnable action, final String message) {
    try {
      action.run();
      fail(message);
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }
}
