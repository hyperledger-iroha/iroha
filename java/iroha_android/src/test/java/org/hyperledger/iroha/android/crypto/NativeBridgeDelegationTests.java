package org.hyperledger.iroha.android.crypto;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.model.FeeChargeKind;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;

/**
 * {@code connect_norito_bridge} exports JNI only for the Kotlin SDK classes, so the Java SDK must
 * declare no native methods and must hand the Kotlin owners values that encode identically.
 */
public final class NativeBridgeDelegationTests {
  private static final String GAS_ASSET = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";
  private static final Pattern NATIVE_METHOD =
      Pattern.compile(
          "^\\s*(?:(?:public|protected|private|static|final|synchronized)\\s+)*native\\s",
          Pattern.MULTILINE);

  public static void main(final String[] args) throws Exception {
    final NativeBridgeDelegationTests tests = new NativeBridgeDelegationTests();
    tests.feeIntentsEncodeIdenticallyInBothSdks();
    tests.javaSourcesDeclareNoNativeMethods();
    tests.bridgeRequirementsComeFromTheKotlinOwner();
    System.out.println("[IrohaAndroid] Native bridge delegation tests passed.");
  }

  private void feeIntentsEncodeIdenticallyInBothSdks() throws Exception {
    final List<FeeChargeLimit> limits =
        Arrays.asList(
            new FeeChargeLimit(FeeChargeKind.NEXUS, GAS_ASSET, "5"),
            new FeeChargeLimit(FeeChargeKind.PIPELINE_GAS, GAS_ASSET, "1000"));
    final String sponsor =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x11), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final List<FeePaymentIntent> intents = new ArrayList<>();
    intents.add(FeePaymentIntent.authority(Collections.emptyList()));
    intents.add(FeePaymentIntent.authority(limits, 1_000L));
    intents.add(
        FeePaymentIntent.sponsor(new FeeSponsorProgramId(sponsor, "public-reset"), 3L, limits));
    intents.add(
        FeePaymentIntent.sponsor(
            new FeeSponsorProgramId(sponsor, "public-reset"), 7L, limits.subList(1, 2), 9L));
    for (final FeePaymentIntent intent : intents) {
      final String java = JsonEncoder.encode(intent.toJsonMap());
      final String kotlin =
          org.hyperledger.iroha.sdk.client.JsonEncoder.encode(
              NativeSignerBridge.kotlinFeePayment(intent).toJsonMap());
      check(java.equals(kotlin), "fee intent JSON differs: " + java + " vs " + kotlin);
    }
  }

  private void javaSourcesDeclareNoNativeMethods() throws IOException {
    final Path main = locateMainSources();
    final List<String> offenders = new ArrayList<>();
    try (Stream<Path> files = Files.walk(main)) {
      final Iterable<Path> sources = files.filter(p -> p.toString().endsWith(".java"))::iterator;
      for (final Path file : sources) {
        final String text = new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
        if (NATIVE_METHOD.matcher(text).find()) {
          offenders.add(main.relativize(file).toString());
        }
      }
    }
    check(offenders.isEmpty(), "Java SDK sources must not declare JNI methods: " + offenders);
  }

  private void bridgeRequirementsComeFromTheKotlinOwner() {
    check(
        NativeSignerBridge.REQUIRED_BRIDGE_ABI_VERSION
            == org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.REQUIRED_BRIDGE_ABI_VERSION,
        "bridge ABI requirement differs from the Kotlin owner");
    check(
        NativeSignerBridge.REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION
            == org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
                .REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION,
        "signer contract requirement differs from the Kotlin owner");
    check(
        NativeSignerBridge.isNativeAvailable()
            == org.hyperledger.iroha.sdk.crypto.NativeSignerBridge.isNativeAvailable(),
        "native availability differs from the Kotlin owner");
  }

  private static Path locateMainSources() {
    final Path relative = Paths.get("src", "main", "java", "org", "hyperledger", "iroha", "android");
    final Path cwd = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
    for (final Path candidate :
        new Path[] {
          cwd.resolve(relative),
          cwd.resolve("..").resolve(relative),
          cwd.resolve("java").resolve("iroha_android").resolve(relative),
        }) {
      if (Files.isDirectory(candidate)) {
        return candidate.normalize();
      }
    }
    throw new AssertionError("Java SDK main sources not found from " + cwd);
  }

  private static void check(final boolean condition, final String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }
}
