package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineVerdictJournalTest {

  private OfflineVerdictJournalTest() {}

  public static void main(final String[] args) throws Exception {
    writesAndReloadsMetadata();
    emitsWarningsForApproachingDeadlines();
    storesSafetyDetectToken();
    storesPlayIntegrityToken();
    parsesPlayIntegrityMetadata();
    parsesProvisionedMetadata();
    rejectsFractionalRequiredMetadataFields();
    dropsFractionalTokenTimestamps();
    dropsFractionalProvisionedVersion();
    dropsFractionalPlayIntegrityMaxAge();
    dropsFractionalIntegrityMetadataFields();
    System.out.println("[IrohaAndroid] OfflineVerdictJournalTest passed.");
  }

  private static void writesAndReloadsMetadata() throws IOException {
    final Path tempFile = Files.createTempFile("offline-verdict-journal", ".json");
    final OfflineVerdictJournal journal = new OfflineVerdictJournal(tempFile);
    final List<OfflineAllowanceList.OfflineAllowanceItem> allowances = List.of(sampleAllowance());
    journal.upsert(allowances, Instant.ofEpochMilli(1_700_000_000_000L));
    assert journal.entries().size() == 1 : "metadata entry missing";
    final OfflineVerdictMetadata metadata = journal.entries().get(0);
    assert "deadbeef".equals(metadata.certificateIdHex()) : "certificate mismatch";
    assert metadata.refreshAtMs() == 1_700_100_000_000L : "refresh mismatch";

    final OfflineVerdictJournal reloaded = new OfflineVerdictJournal(tempFile);
    assert reloaded.entries().size() == 1 : "failed to reload persisted metadata";
    Files.deleteIfExists(tempFile);
  }

  private static void emitsWarningsForApproachingDeadlines() throws IOException {
    final Path tempFile = Files.createTempFile("offline-verdict-journal", ".json");
    final OfflineVerdictJournal journal = new OfflineVerdictJournal(tempFile);
    journal.upsert(List.of(sampleAllowance()), Instant.now());
    final List<OfflineVerdictWarning> warnings =
        journal.warnings(60_000L, Instant.ofEpochMilli(1_700_100_000_000L));
    assert warnings.size() == 1 : "warning missing";
    assert warnings.get(0).state() == OfflineVerdictWarning.State.EXPIRED
        : "expected expired warning";
    Files.deleteIfExists(tempFile);
  }

  private static void storesSafetyDetectToken() throws IOException {
    final Path tempFile = Files.createTempFile("offline-verdict-journal", ".json");
    final OfflineVerdictJournal journal = new OfflineVerdictJournal(tempFile);
    journal.upsert(List.of(sampleAllowance()), Instant.now());
    final OfflineVerdictMetadata.SafetyDetectTokenSnapshot snapshot =
        new OfflineVerdictMetadata.SafetyDetectTokenSnapshot("token-123", 1_700_000_123_000L);
    journal.updateSafetyDetectToken("deadbeef", snapshot);
    final OfflineVerdictMetadata metadata =
        journal.find("deadbeef").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert metadata.hmsSafetyDetectToken() != null : "expected cached token";
    assert "token-123".equals(metadata.hmsSafetyDetectToken().token()) : "token mismatch";
    final OfflineVerdictJournal reloaded = new OfflineVerdictJournal(tempFile);
    final OfflineVerdictMetadata persisted =
        reloaded.find("deadbeef").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert persisted.hmsSafetyDetectToken() != null : "token not persisted";
    assert 1_700_000_123_000L == persisted.hmsSafetyDetectToken().fetchedAtMs()
        : "timestamp mismatch";
    Files.deleteIfExists(tempFile);
  }

  private static void storesPlayIntegrityToken() throws IOException {
    final Path tempFile = Files.createTempFile("offline-verdict-journal", ".json");
    final OfflineVerdictJournal journal = new OfflineVerdictJournal(tempFile);
    journal.upsert(List.of(playIntegrityAllowance()), Instant.now());
    final OfflineVerdictMetadata.PlayIntegrityTokenSnapshot snapshot =
        new OfflineVerdictMetadata.PlayIntegrityTokenSnapshot("play-token", 1_700_000_456_000L);
    journal.updatePlayIntegrityToken("baddcafe", snapshot);
    final OfflineVerdictMetadata metadata =
        journal.find("baddcafe").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert metadata.playIntegrityToken() != null : "expected cached play integrity token";
    assert "play-token".equals(metadata.playIntegrityToken().token()) : "token mismatch";
    final OfflineVerdictJournal reloaded = new OfflineVerdictJournal(tempFile);
    final OfflineVerdictMetadata persisted =
        reloaded.find("baddcafe").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert persisted.playIntegrityToken() != null : "token not persisted";
    assert 1_700_000_456_000L == persisted.playIntegrityToken().fetchedAtMs()
        : "timestamp mismatch";
    Files.deleteIfExists(tempFile);
  }

  private static void parsesPlayIntegrityMetadata() throws IOException {
    final Path tempFile = Files.createTempFile("offline-verdict-journal", ".json");
    final OfflineVerdictJournal journal = new OfflineVerdictJournal(tempFile);
    journal.upsert(List.of(playIntegrityAllowance()), Instant.now());
    final OfflineVerdictMetadata metadata =
        journal.find("baddcafe").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert "play_integrity".equals(metadata.integrityPolicy()) : "policy mismatch";
    final OfflineVerdictMetadata.PlayIntegrityMetadata play = metadata.playIntegrity();
    assert play != null : "expected play integrity metadata";
    assert play.cloudProjectNumber() == 4242L : "project number mismatch";
    assert "testing".equals(play.environment()) : "environment mismatch";
    assert play.packageNames().contains("com.example.pos") : "package missing";
    assert play.signingDigestsSha256().contains("ab12") : "digest missing";
    assert play.allowedAppVerdicts().contains("PLAY_RECOGNIZED") : "app verdict missing";
    assert play.allowedDeviceVerdicts().contains("MEETS_DEVICE_INTEGRITY")
        : "device verdict missing";
    assert play.maxTokenAgeMs() != null && play.maxTokenAgeMs() == 3_600_000L
        : "max age mismatch";
    final OfflineVerdictJournal reloaded = new OfflineVerdictJournal(tempFile);
    final OfflineVerdictMetadata persisted =
        reloaded.find("baddcafe").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert persisted.playIntegrity() != null : "play integrity metadata not persisted";
    Files.deleteIfExists(tempFile);
  }

  private static void parsesProvisionedMetadata() throws IOException {
    final Path tempFile = Files.createTempFile("offline-verdict-journal", ".json");
    final OfflineVerdictJournal journal = new OfflineVerdictJournal(tempFile);
    journal.upsert(List.of(provisionedAllowance()), Instant.ofEpochMilli(1_800_000_000_000L));
    final OfflineVerdictMetadata metadata =
        journal.find("cafebabe").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert "provisioned".equals(metadata.integrityPolicy()) : "policy mismatch";
    final OfflineVerdictMetadata.ProvisionedMetadata provisioned =
        metadata.provisionedMetadata();
    assert provisioned != null : "expected provisioned metadata";
    assert "ed0120C0FFEE".equals(provisioned.inspectorPublicKey()) : "inspector mismatch";
    assert "offline_provisioning_v1".equals(provisioned.manifestSchema()) : "schema mismatch";
    assert provisioned.manifestVersion() != null && provisioned.manifestVersion() == 3
        : "version mismatch";
    assert provisioned.maxManifestAgeMs() != null && provisioned.maxManifestAgeMs() == 604_800_000L
        : "max age mismatch";
    assert "00ff".equals(provisioned.manifestDigest()) : "digest mismatch";
    final OfflineVerdictJournal reloaded = new OfflineVerdictJournal(tempFile);
    final OfflineVerdictMetadata persisted =
        reloaded.find("cafebabe").orElseThrow(() -> new IllegalStateException("missing metadata"));
    assert persisted.provisionedMetadata() != null : "provisioned metadata not persisted";
    Files.deleteIfExists(tempFile);
  }

  private static void rejectsFractionalRequiredMetadataFields() {
    final String json =
        """
        {
          "controller_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "controller_display": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "certificate_expires_at_ms": 1700000000000.5,
          "policy_expires_at_ms": 1700000000000,
          "recorded_at_ms": 1700000000000
        }
        """;
    boolean thrown = false;
    try {
      OfflineVerdictMetadata.fromJson("deadbeef", parseObject(json));
    } catch (final IllegalStateException ex) {
      thrown = true;
    }
    assert thrown : "expected fractional timestamps to be rejected";
  }

  private static void dropsFractionalTokenTimestamps() {
    final String json =
        """
        {
          "controller_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "controller_display": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "certificate_expires_at_ms": 1700000000000,
          "policy_expires_at_ms": 1700000000000,
          "recorded_at_ms": 1700000000000,
          "play_integrity_token": {
            "token": "play-token",
            "fetched_at_ms": 1700000000000.25
          },
          "hms_safety_detect_token": {
            "token": "hms-token",
            "fetched_at_ms": 1700000000000.5
          }
        }
        """;
    final OfflineVerdictMetadata metadata =
        OfflineVerdictMetadata.fromJson("deadbeef", parseObject(json));
    assert metadata.playIntegrityToken() == null : "expected play integrity token to be dropped";
    assert metadata.hmsSafetyDetectToken() == null : "expected HMS token to be dropped";
  }

  private static void dropsFractionalProvisionedVersion() {
    final String json =
        """
        {
          "controller_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "controller_display": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "certificate_expires_at_ms": 1700000000000,
          "policy_expires_at_ms": 1700000000000,
          "recorded_at_ms": 1700000000000,
          "android_provisioned": {
            "inspector_public_key": "ed0120C0FFEE",
            "manifest_schema": "offline_provisioning_v1",
            "manifest_version": 1.5
          }
        }
        """;
    final OfflineVerdictMetadata metadata =
        OfflineVerdictMetadata.fromJson("deadbeef", parseObject(json));
    final OfflineVerdictMetadata.ProvisionedMetadata provisioned =
        metadata.provisionedMetadata();
    assert provisioned != null : "expected provisioned metadata to parse";
    assert provisioned.manifestVersion() == null : "expected fractional version to be dropped";
  }

  private static void dropsFractionalPlayIntegrityMaxAge() {
    final String json =
        """
        {
          "controller_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "controller_display": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
          "certificate_expires_at_ms": 1700000000000,
          "policy_expires_at_ms": 1700000000000,
          "recorded_at_ms": 1700000000000,
          "play_integrity": {
            "cloud_project_number": 4242,
            "environment": "testing",
            "package_names": ["com.example.pos"],
            "signing_digests_sha256": ["ab12"],
            "allowed_app_verdicts": ["PLAY_RECOGNIZED"],
            "allowed_device_verdicts": ["MEETS_DEVICE_INTEGRITY"],
            "max_token_age_ms": 3600.5
          }
        }
        """;
    final OfflineVerdictMetadata metadata =
        OfflineVerdictMetadata.fromJson("deadbeef", parseObject(json));
    final OfflineVerdictMetadata.PlayIntegrityMetadata play = metadata.playIntegrity();
    assert play != null : "expected play integrity metadata";
    assert play.maxTokenAgeMs() == null : "expected fractional max age to be dropped";
  }

  private static void dropsFractionalIntegrityMetadataFields() throws IOException {
    final Path tempFile = Files.createTempFile("offline-verdict-journal", ".json");
    try {
      final OfflineVerdictJournal journal = new OfflineVerdictJournal(tempFile);
      journal.upsert(
          List.of(fractionalPlayIntegrityAllowance(), fractionalProvisionedAllowance()),
          Instant.now());
      final OfflineVerdictMetadata play =
          journal.find("fractional-play")
              .orElseThrow(() -> new IllegalStateException("missing play metadata"));
      assert play.playIntegrity() != null : "expected play integrity metadata";
      assert play.playIntegrity().maxTokenAgeMs() == null
          : "expected fractional play integrity max age to be dropped";
      final OfflineVerdictMetadata provisioned =
          journal.find("fractional-prov")
              .orElseThrow(() -> new IllegalStateException("missing provisioned metadata"));
      assert provisioned.provisionedMetadata() != null : "expected provisioned metadata";
      assert provisioned.provisionedMetadata().manifestVersion() == null
          : "expected fractional manifest version to be dropped";
    } finally {
      Files.deleteIfExists(tempFile);
    }
  }

  private static OfflineAllowanceList.OfflineAllowanceItem sampleAllowance() {
    return new OfflineAllowanceList.OfflineAllowanceItem(
        "deadbeef",
        "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
        "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "USD",
        null,
        1_700_000_000_000L,
        1_700_200_000_000L,
        1_700_300_000_000L,
        1_700_100_000_000L,
        "feedface",
        "abcd",
        "10",
        "{}");
  }

  private static OfflineAllowanceList.OfflineAllowanceItem provisionedAllowance() {
    return new OfflineAllowanceList.OfflineAllowanceItem(
        "cafebabe",
        "inspector@nexus",
        "inspector@nexus",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "USD",
        null,
        1_800_000_000_000L,
        1_800_400_000_000L,
        1_800_500_000_000L,
        1_800_050_000_000L,
        null,
        null,
        "0",
        provisionedRecordJson());
  }

  private static OfflineAllowanceList.OfflineAllowanceItem playIntegrityAllowance() {
    return new OfflineAllowanceList.OfflineAllowanceItem(
        "baddcafe",
        "play@integrity",
        "Play Integrity",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "USD",
        null,
        1_700_000_000_000L,
        1_700_200_000_000L,
        1_700_300_000_000L,
        1_700_100_000_000L,
        "feedbead",
        "nonce",
        "50",
        playIntegrityRecordJson());
  }

  private static OfflineAllowanceList.OfflineAllowanceItem fractionalPlayIntegrityAllowance() {
    return new OfflineAllowanceList.OfflineAllowanceItem(
        "fractional-play",
        "play@integrity",
        "Play Integrity",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "USD",
        null,
        1_700_000_000_000L,
        1_700_200_000_000L,
        1_700_300_000_000L,
        1_700_100_000_000L,
        "feedbead",
        "nonce",
        "50",
        fractionalPlayIntegrityRecordJson());
  }

  private static OfflineAllowanceList.OfflineAllowanceItem fractionalProvisionedAllowance() {
    return new OfflineAllowanceList.OfflineAllowanceItem(
        "fractional-prov",
        "inspector@nexus",
        "inspector@nexus",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        "USD",
        null,
        1_800_000_000_000L,
        1_800_400_000_000L,
        1_800_500_000_000L,
        1_800_050_000_000L,
        null,
        null,
        "0",
        fractionalProvisionedRecordJson());
  }

  private static String playIntegrityRecordJson() {
    return """
        {
          "certificate": {
            "metadata": {
              "android.integrity.policy": "play_integrity",
              "android.play_integrity.cloud_project_number": 4242,
              "android.play_integrity.environment": "testing",
              "android.play_integrity.package_names": ["com.example.pos"],
              "android.play_integrity.signing_digests_sha256": ["ab12"],
              "android.play_integrity.allowed_app_verdicts": ["PLAY_RECOGNIZED"],
              "android.play_integrity.allowed_device_verdicts": ["MEETS_DEVICE_INTEGRITY"],
              "android.play_integrity.max_token_age_ms": 3600000
            }
          }
        }
        """;
  }

  private static String provisionedRecordJson() {
    return """
        {
          "certificate": {
            "metadata": {
              "android.integrity.policy": "provisioned",
              "android.provisioned.inspector_public_key": "ed0120C0FFEE",
              "android.provisioned.manifest_schema": "offline_provisioning_v1",
              "android.provisioned.manifest_version": 3,
              "android.provisioned.max_manifest_age_ms": 604800000,
              "android.provisioned.manifest_digest": "00ff"
            }
          }
        }
        """;
  }

  private static String fractionalPlayIntegrityRecordJson() {
    return """
        {
          "certificate": {
            "metadata": {
              "android.integrity.policy": "play_integrity",
              "android.play_integrity.cloud_project_number": 4242,
              "android.play_integrity.environment": "testing",
              "android.play_integrity.package_names": ["com.example.pos"],
              "android.play_integrity.signing_digests_sha256": ["ab12"],
              "android.play_integrity.allowed_app_verdicts": ["PLAY_RECOGNIZED"],
              "android.play_integrity.allowed_device_verdicts": ["MEETS_DEVICE_INTEGRITY"],
              "android.play_integrity.max_token_age_ms": 3600.5
            }
          }
        }
        """;
  }

  private static String fractionalProvisionedRecordJson() {
    return """
        {
          "certificate": {
            "metadata": {
              "android.integrity.policy": "provisioned",
              "android.provisioned.inspector_public_key": "ed0120C0FFEE",
              "android.provisioned.manifest_schema": "offline_provisioning_v1",
              "android.provisioned.manifest_version": 1.5,
              "android.provisioned.max_manifest_age_ms": 604800000,
              "android.provisioned.manifest_digest": "00ff"
            }
          }
        }
        """;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parseObject(final String json) {
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof Map<?, ?> map)) {
      throw new IllegalStateException("expected JSON object");
    }
    return (Map<String, Object>) map;
  }
}
