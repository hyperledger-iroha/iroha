package org.hyperledger.iroha.android;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.Set;
import java.util.stream.Collectors;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.Parameterized;

/**
 * Gradle harness that runs the existing main-based tests under the Gradle {@code test} task.
 */
@RunWith(Parameterized.class)
public final class GradleHarnessTests {
  private static final String[] MAIN_CLASSES =
      new String[] {
        "org.hyperledger.iroha.android.IrohaKeyManagerAttestationTelemetryTests",
        "org.hyperledger.iroha.android.IrohaKeyManagerDeterministicExportTests",
        "org.hyperledger.iroha.android.IrohaKeyManagerKeyValidationTelemetryTests",
        "org.hyperledger.iroha.android.IrohaKeyManagerMetadataTests",
        "org.hyperledger.iroha.android.IrohaKeyManagerTests",
        "org.hyperledger.iroha.android.address.AccountAddressTests",
        "org.hyperledger.iroha.android.client.CanonicalRequestSignerTests",
        "org.hyperledger.iroha.android.client.ClientConfigKeystoreTelemetryTests",
        "org.hyperledger.iroha.android.client.ClientConfigManifestLoaderTests",
        "org.hyperledger.iroha.android.client.ClientConfigNoritoRpcTests",
        "org.hyperledger.iroha.android.client.ClientConfigOfflineQueueTests",
        "org.hyperledger.iroha.android.client.ConfigWatcherTests",
        "org.hyperledger.iroha.android.client.HttpClientTransportExportOptionsTests",
        "org.hyperledger.iroha.android.client.HttpClientTransportHarnessTests",
        "org.hyperledger.iroha.android.client.HttpClientTransportOfflineQueueTests",
        "org.hyperledger.iroha.android.client.HttpClientTransportPendingQueueTests",
        "org.hyperledger.iroha.android.client.HttpClientTransportStatusTests",
        "org.hyperledger.iroha.android.client.HttpClientTransportTests",
        "org.hyperledger.iroha.android.client.HttpTransportExecutorFakeTests",
        "org.hyperledger.iroha.android.client.JsonEncoderTests",
        "org.hyperledger.iroha.android.client.JsonParserTests",
        "org.hyperledger.iroha.android.client.PlatformHttpTransportExecutorFallbackTests",
        "org.hyperledger.iroha.android.client.NoritoRpcClientTests",
        "org.hyperledger.iroha.android.client.OfflineToriiClientTests",
        "org.hyperledger.iroha.android.client.SubscriptionToriiClientTests",
        "org.hyperledger.iroha.android.client.PipelineStatusExtractorTests",
        "org.hyperledger.iroha.android.client.mock.ToriiMockServerTests",
        "org.hyperledger.iroha.android.client.queue.DirectoryPendingTransactionQueueTests",
        "org.hyperledger.iroha.android.client.queue.FilePendingTransactionQueueTests",
        "org.hyperledger.iroha.android.client.queue.OfflineJournalPendingTransactionQueueTest",
        "org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests",
        "org.hyperledger.iroha.android.client.stream.ToriiEventStreamSubscriptionTests",
        "org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests",
        "org.hyperledger.iroha.android.client.websocket.ToriiWebSocketSubscriptionTests",
        "org.hyperledger.iroha.android.connect.ConnectErrorTests",
        "org.hyperledger.iroha.android.connect.ConnectQueueJournalTests",
        "org.hyperledger.iroha.android.connect.ConnectRetryPolicyTests",
        "org.hyperledger.iroha.android.crypto.Blake2sTests",
        "org.hyperledger.iroha.android.crypto.SoftwareKeyProviderFallbackTests",
        "org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests",
        "org.hyperledger.iroha.android.crypto.keystore.AndroidKeystoreBackendDetectionTests",
        "org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests",
        "org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerifierTests",
        "org.hyperledger.iroha.android.crypto.keystore.attestation.IrohaKeyManagerTelemetryTests",
        "org.hyperledger.iroha.android.governance.GovernanceInstructionBuilderTests",
        "org.hyperledger.iroha.android.gpu.CudaAcceleratorsKotlinFacadeTests",
        "org.hyperledger.iroha.android.gpu.CudaAcceleratorsNativeSmokeTests",
        "org.hyperledger.iroha.android.gpu.CudaAcceleratorsTests",
        "org.hyperledger.iroha.android.model.instructions.KaigiInstructionValidationTests",
        "org.hyperledger.iroha.android.model.instructions.InstructionSchemaManifestTests",
        "org.hyperledger.iroha.android.model.instructions.VerifyingKeyInstructionUtilsTests",
        "org.hyperledger.iroha.android.nexus.SpaceDirectoryInstructionBuilderTests",
        "org.hyperledger.iroha.android.nexus.UaidJsonParserTests",
        "org.hyperledger.iroha.android.norito.NoritoCodecAdapterTests",
        "org.hyperledger.iroha.android.offline.AndroidProvisionedProofTest",
        "org.hyperledger.iroha.android.offline.OfflineAllowanceInstructionBuilderTests",
        "org.hyperledger.iroha.android.offline.OfflineAuditLoggerTest",
        "org.hyperledger.iroha.android.offline.OfflineBalanceProofTest",
        "org.hyperledger.iroha.android.offline.OfflineCounterJournalTest",
        "org.hyperledger.iroha.android.offline.OfflineJournalTest",
        "org.hyperledger.iroha.android.offline.OfflineJsonParserTest",
        "org.hyperledger.iroha.android.offline.OfflineQueryEnvelopeTest",
        "org.hyperledger.iroha.android.offline.OfflineReceiptChallengeTest",
        "org.hyperledger.iroha.android.offline.OfflineQrStreamTest",
        "org.hyperledger.iroha.android.offline.OfflinePetalStreamTest",
        "org.hyperledger.iroha.android.offline.OfflineTransferPayloadsTest",
        "org.hyperledger.iroha.android.offline.OfflineVerdictJournalTest",
        "org.hyperledger.iroha.android.offline.OfflineWalletPlayIntegrityTests",
        "org.hyperledger.iroha.android.offline.OfflineWalletSafetyDetectTests",
        "org.hyperledger.iroha.android.offline.OfflineWalletTest",
        "org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectServiceTests",
        "org.hyperledger.iroha.android.runtime.RuntimeUpgradeInstructionTests",
        "org.hyperledger.iroha.android.sorafs.SorafsCapacityMarketplaceInstructionTests",
        "org.hyperledger.iroha.android.sorafs.SorafsCapacityTelemetryInstructionTests",
        "org.hyperledger.iroha.android.sorafs.SorafsGatewayClientTests",
        "org.hyperledger.iroha.android.sorafs.SorafsGatewayFetchOptionsTests",
        "org.hyperledger.iroha.android.sorafs.SorafsManifestInstructionBuilderTests",
        "org.hyperledger.iroha.android.sorafs.SorafsRegisterPinManifestBuilderTests",
        "org.hyperledger.iroha.android.sorafs.SorafsReplicationInstructionBuilderTests",
        "org.hyperledger.iroha.android.telemetry.AndroidNetworkContextProviderTests",
        "org.hyperledger.iroha.android.telemetry.AuthorityHashTest",
        "org.hyperledger.iroha.android.telemetry.ChaosScenarioLoggerTests",
        "org.hyperledger.iroha.android.telemetry.CrashTelemetryHandlerTests",
        "org.hyperledger.iroha.android.telemetry.CrashTelemetryReporterTests",
        "org.hyperledger.iroha.android.telemetry.TelemetryExportStatusSinkTests",
        "org.hyperledger.iroha.android.telemetry.TelemetryObserverTests",
        "org.hyperledger.iroha.android.telemetry.TelemetryOptionsTests",
        "org.hyperledger.iroha.android.tools.AndroidKeystoreAttestationHarnessTests",
        "org.hyperledger.iroha.android.tools.PendingQueueInspectorTests",
        "org.hyperledger.iroha.android.tx.SignedTransactionHasherTests",
        "org.hyperledger.iroha.android.tx.TransactionBuilderOfflineEnvelopeTests",
        "org.hyperledger.iroha.android.tx.TransactionBuilderTests",
        "org.hyperledger.iroha.android.tx.TransactionFixtureManifestTests",
        "org.hyperledger.iroha.android.tx.TransactionPayloadFixtureExporter",
        "org.hyperledger.iroha.android.tx.TransactionPayloadFixtureTests",
        "org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelopeCodecTests",
      };

  private final String className;

  public GradleHarnessTests(final String className) {
    this.className = className;
  }

  @Parameterized.Parameters(name = "{0}")
  public static Collection<Object[]> parameters() {
    return Arrays.stream(selectedMains()).map(name -> new Object[] {name}).toList();
  }

  @Test
  public void runHarnessMain() throws Exception {
    invokeMain(className);
  }

  private static String[] selectedMains() {
    final String overrides = System.getProperty("android.test.mains", "").trim();
    if (overrides.isEmpty()) {
      return MAIN_CLASSES;
    }
    final Set<String> allowList =
        Arrays.stream(overrides.split(","))
            .map(String::trim)
            .filter(s -> !s.isEmpty())
            .collect(Collectors.toCollection(LinkedHashSet::new));
    final String[] filtered =
        Arrays.stream(MAIN_CLASSES).filter(allowList::contains).toArray(String[]::new);
    if (filtered.length == 0) {
      throw new IllegalArgumentException(
          "android.test.mains filter produced no matches: " + overrides);
    }
    return filtered;
  }

  private static void invokeMain(final String target) throws Exception {
    final Class<?> clazz = Class.forName(target);
    final Method main = clazz.getMethod("main", String[].class);
    try {
      main.invoke(null, (Object) new String[0]);
    } catch (final InvocationTargetException ex) {
      final Throwable cause = ex.getCause();
      if (cause instanceof Exception exception) {
        throw exception;
      }
      if (cause instanceof Error error) {
        throw error;
      }
      throw new RuntimeException(cause);
    }
  }
}
