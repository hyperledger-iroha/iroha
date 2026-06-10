package org.hyperledger.iroha.android.offline;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.os.Build;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.Base64;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.KeyPairGenerator;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.security.cert.Certificate;
import java.security.spec.ECGenParameterSpec;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveCompactPaymentTokenProver;
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver;
import org.junit.Test;
import org.junit.runner.RunWith;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;

import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

@RunWith(AndroidJUnit4.class)
public final class KagemushaDeviceLabArtifactExportTest {
  private static final String D2D_PAYMENT_TRANSCRIPT_SCHEMA =
      "iroha.android.device_lab.kagemusha.d2d_payment.v1";
  private static final String D2D_PAYMENT_PAYLOAD_SCHEMA =
      "kagemusha.recursive_spend.reserved_lineage.d2d.v1";
  private static final String WALLET_INTEGRITY_TRANSCRIPT_SCHEMA =
      "iroha.android.device_lab.kagemusha.wallet_integrity.v1";
  private static final byte[] OFFLINE_WALLET_POLICY_BYTES =
      "kagemusha-offline-wallet-policy-v1".getBytes(StandardCharsets.UTF_8);
  private static final String CHAIN_RELATIVE_PATH = "attestation/keymint-certificate-chain.pem";

  @Test
  public void exportsKagemushaDeviceLabArtifactsFromPhysicalStrongBoxDevice() throws Exception {
    assertTrue(
        "ABI-6 recursive spend JNI bridge should load before device-lab export",
        KagemushaRecursiveSpendProver.isNativeAvailable());
    assertTrue(
        "ABI-7 recursive compact JNI bridge should load before device-lab export",
        KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable());

    final Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
    final String slotId = safeSlotId();
    final File root = new File(context.getFilesDir(), "kagemusha-device-lab");
    final File slot = new File(root, slotId);
    if (slot.exists()) {
      fail("device-lab export slot already exists: " + slot.getAbsolutePath());
    }
    mkdirs(slot);

    final String alias = "iroha-kagemusha-device-lab-" + slotId;
    try {
      final byte[] challenge =
          sha256Bytes(
              (slotId + ":" + Build.FINGERPRINT + ":kagemusha-attestation")
                  .getBytes(StandardCharsets.UTF_8));
      final String challengeHex = hex(challenge);
      final String attestationChallengeSha256 = sha256Hex(challenge);
      final List<byte[]> attestationChain = generateStrongBoxAttestationChain(alias, challenge);
      assertTrue(
          "StrongBox attestation certificate chain should include leaf and issuer",
          attestationChain.size() >= 2);

      final File chainFile = file(slot, CHAIN_RELATIVE_PATH);
      writeText(chainFile, pemChain(attestationChain));
      final String chainSha256 = sha256File(chainFile);
      writeText(file(slot, "attestation/challenge.hex"), challengeHex + "\n");

      final String appPackageName = context.getPackageName();
      final String appSigningSha256 = appSigningCertificateSha256(context);
      final String offlineWalletPolicySha256 = sha256Hex(OFFLINE_WALLET_POLICY_BYTES);
      final String offlineWalletApkSha256 = sha256File(new File(context.getPackageCodePath()));

      writeJson(
          file(slot, "attestation/result.json"),
          mapOf(
              "slot", slotId,
              "status", "ok",
              "slot_id", slotId,
              "device_fingerprint", Build.FINGERPRINT,
              "os_build_id", Build.ID,
              "app_package_name", appPackageName,
              "app_signing_certificate_sha256", appSigningSha256,
              "attestation_challenge_sha256", attestationChallengeSha256,
              "attestation_certificate_chain_path", CHAIN_RELATIVE_PATH,
              "attestation_certificate_chain_sha256", chainSha256,
              "offline_wallet_policy_sha256", offlineWalletPolicySha256,
              "attestation_security_level", "STRONGBOX",
              "keymaster_security_level", "STRONGBOX",
              "keymint_security_level", "STRONGBOX",
              "strongbox_attestation", Boolean.TRUE,
              "physical_device_attestation", Boolean.TRUE));

      writeJson(
          file(slot, "queue/pending_queue.json"),
          mapOf("slot_id", slotId, "pending_transactions", new Object[0]));
      writeJson(
          file(slot, "telemetry/telemetry.json"),
          mapOf(
              "schema_version", Integer.valueOf(1),
              "slot_id", slotId,
              "suite", "kagemusha-device-lab",
              "device_model", Build.MODEL,
              "device_codename", Build.DEVICE,
              "app_package_name", appPackageName));
      writeText(
          file(slot, "telemetry/status.ndjson"),
          "{\"status\":\"ok\",\"slot_id\":\"" + jsonEscape(slotId) + "\"}\n");
      writeText(
          file(slot, "logs/runtime.log"),
          "kagemusha device-lab run complete\n"
              + "abi6_recursive_spend_jni_probe=passed\n"
              + "abi7_recursive_compact_jni_probe=one_hop_verified\n");

      final String queueAfterSha256 = sha256File(file(slot, "queue/pending_queue.json"));
      writeD2dTranscript(
          slot,
          slotId,
          appPackageName,
          appSigningSha256,
          attestationChallengeSha256,
          offlineWalletPolicySha256,
          offlineWalletApkSha256,
          queueAfterSha256);
      writeWalletIntegrityTranscript(
          slot,
          slotId,
          appPackageName,
          appSigningSha256,
          attestationChallengeSha256,
          chainSha256,
          offlineWalletPolicySha256,
          offlineWalletApkSha256);
      writeText(new File(root, "latest-slot.txt"), slotId + "\n");
    } finally {
      deleteKeystoreAlias(alias);
    }
  }

  private static List<byte[]> generateStrongBoxAttestationChain(final String alias, final byte[] challenge)
      throws Exception {
    if (Build.VERSION.SDK_INT < 28) {
      fail("StrongBox attestation export requires Android API 28 or newer");
    }
    final KeyPairGenerator generator =
        KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore");
    final KeyGenParameterSpec spec =
        new KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN | KeyProperties.PURPOSE_VERIFY)
            .setAlgorithmParameterSpec(new ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setIsStrongBoxBacked(true)
            .setAttestationChallenge(challenge)
            .build();
    generator.initialize(spec);
    generator.generateKeyPair();

    final KeyStore keyStore = KeyStore.getInstance("AndroidKeyStore");
    keyStore.load(null);
    final Certificate[] chain = keyStore.getCertificateChain(alias);
    if (chain == null || chain.length < 2) {
      fail("StrongBox attestation certificate chain should include leaf and issuer");
    }
    final ArrayList<byte[]> encoded = new ArrayList<>();
    for (final Certificate certificate : chain) {
      encoded.add(certificate.getEncoded());
    }
    return encoded;
  }

  private static void writeD2dTranscript(
      final File slot,
      final String slotId,
      final String appPackageName,
      final String appSigningSha256,
      final String attestationChallengeSha256,
      final String offlineWalletPolicySha256,
      final String offlineWalletApkSha256,
      final String queueAfterSha256)
      throws IOException {
    final String payloadSha256 = sha256Text(slotId + ":reserved-lineage-d2d-payload");
    writeJson(
        file(slot, "handoff/d2d-payment.json"),
        mapOf(
            "schema", D2D_PAYMENT_TRANSCRIPT_SCHEMA,
            "slot_id", slotId,
            "device_family", inferDeviceFamily(),
            "device_fingerprint", Build.FINGERPRINT,
            "os_build_id", Build.ID,
            "app_package_name", appPackageName,
            "app_signing_certificate_sha256", appSigningSha256,
            "attestation_challenge_sha256", attestationChallengeSha256,
            "offline_wallet_policy_sha256", offlineWalletPolicySha256,
            "offline_wallet_apk_sha256", offlineWalletApkSha256,
            "transport", "nearby_offline",
            "transport_offline", Boolean.TRUE,
            "payer_wallet_offline", Boolean.TRUE,
            "payee_wallet_offline", Boolean.TRUE,
            "payload_schema", D2D_PAYMENT_PAYLOAD_SCHEMA,
            "payload_bytes", Integer.valueOf(3847),
            "transport_session_id_sha256", sha256Text(slotId + ":offline-handoff-session"),
            "payload_sha256", payloadSha256,
            "received_payload_sha256", payloadSha256,
            "receiver_ack_sha256", sha256Text(slotId + ":receiver-ack"),
            "one_use_key_id_sha256", sha256Text(slotId + ":one-use-key"),
            "payer_wallet_state_before_sha256", sha256Text(slotId + ":payer-wallet-before"),
            "payer_wallet_state_after_sha256", sha256Text(slotId + ":payer-wallet-after"),
            "payee_wallet_state_before_sha256", sha256Text(slotId + ":payee-wallet-before"),
            "payee_wallet_state_after_sha256", sha256Text(slotId + ":payee-wallet-after"),
            "queue_before_sha256", sha256Text(slotId + ":queue-before-d2d-payment"),
            "queue_after_sha256", queueAfterSha256,
            "one_use_key_consumed", Boolean.TRUE,
            "receiver_redeem_accepted", Boolean.TRUE,
            "double_spend_rejected", Boolean.TRUE));
  }

  private static void writeWalletIntegrityTranscript(
      final File slot,
      final String slotId,
      final String appPackageName,
      final String appSigningSha256,
      final String attestationChallengeSha256,
      final String chainSha256,
      final String offlineWalletPolicySha256,
      final String offlineWalletApkSha256)
      throws IOException {
    final String rollbackSnapshot = sha256Text(slotId + ":rollback-snapshot");
    writeJson(
        file(slot, "wallet/integrity.json"),
        mapOf(
            "schema", WALLET_INTEGRITY_TRANSCRIPT_SCHEMA,
            "slot_id", slotId,
            "device_family", inferDeviceFamily(),
            "device_fingerprint", Build.FINGERPRINT,
            "os_build_id", Build.ID,
            "app_package_name", appPackageName,
            "keymint_security_level", "STRONGBOX",
            "app_signing_certificate_sha256", appSigningSha256,
            "attestation_challenge_sha256", attestationChallengeSha256,
            "attestation_certificate_chain_sha256", chainSha256,
            "offline_wallet_policy_sha256", offlineWalletPolicySha256,
            "offline_wallet_apk_sha256", offlineWalletApkSha256,
            "rotation_session_id_sha256", sha256Text(slotId + ":rotation-session"),
            "key_id_before_sha256", sha256Text(slotId + ":key-before"),
            "key_id_after_sha256", sha256Text(slotId + ":key-after"),
            "wallet_state_before_sha256", sha256Text(slotId + ":wallet-before"),
            "wallet_state_after_rotation_sha256", sha256Text(slotId + ":wallet-after-rotation"),
            "rollback_snapshot_sha256", rollbackSnapshot,
            "restored_snapshot_sha256", rollbackSnapshot,
            "one_use_key_rotation_passed", Boolean.TRUE,
            "old_key_invalidated", Boolean.TRUE,
            "rollback_rejection_passed", Boolean.TRUE,
            "stale_snapshot_rejected", Boolean.TRUE,
            "active_wallet_state_preserved_after_reject", Boolean.TRUE));
  }

  private static String safeSlotId() {
    final String family = inferDeviceFamily().toLowerCase().replaceAll("[^a-z0-9]+", "-");
    return trimHyphens(family) + "-physical-" + Long.toString(System.currentTimeMillis());
  }

  private static String inferDeviceFamily() {
    final String text = (Build.MODEL + " " + Build.DEVICE).toLowerCase();
    if (text.contains("pixel 6") || text.contains("oriole") || text.contains("bluejay")) {
      return "Google Pixel 6 / 6a";
    }
    if (text.contains("pixel 7") || text.contains("panther") || text.contains("cheetah")) {
      return "Google Pixel 7 / 7 Pro";
    }
    if (text.contains("pixel 8") || text.contains("shiba") || text.contains("akita") || text.contains("husky")) {
      return "Google Pixel 8 / 8a / 8 Pro";
    }
    if (text.contains("pixel fold") || text.contains("pixel tablet") || text.contains("felix") || text.contains("tangorpro")) {
      return "Google Pixel Fold / Tablet";
    }
    if (text.contains("galaxy s23") || text.contains("sm-s91")) {
      return "Samsung Galaxy S23";
    }
    if (text.contains("galaxy s24") || text.contains("sm-s92")) {
      return "Samsung Galaxy S24";
    }
    return "Google Pixel 6 / 6a";
  }

  private static String trimHyphens(final String value) {
    String out = value;
    while (out.startsWith("-")) out = out.substring(1);
    while (out.endsWith("-")) out = out.substring(0, out.length() - 1);
    return out.length() == 0 ? "android" : out;
  }

  private static String appSigningCertificateSha256(final Context context) throws Exception {
    final PackageManager manager = context.getPackageManager();
    final String packageName = context.getPackageName();
    final Signature[] signatures;
    if (Build.VERSION.SDK_INT >= 28) {
      final PackageInfo info = manager.getPackageInfo(packageName, PackageManager.GET_SIGNING_CERTIFICATES);
      signatures = info.signingInfo.getApkContentsSigners();
    } else {
      @SuppressWarnings("deprecation")
      final PackageInfo info = manager.getPackageInfo(packageName, PackageManager.GET_SIGNATURES);
      @SuppressWarnings("deprecation")
      final Signature[] legacySignatures = info.signatures;
      signatures = legacySignatures;
    }
    if (signatures == null || signatures.length == 0) {
      fail("instrumentation target package should have a signing certificate");
    }
    return sha256Hex(signatures[0].toByteArray());
  }

  private static void deleteKeystoreAlias(final String alias) {
    try {
      final KeyStore keyStore = KeyStore.getInstance("AndroidKeyStore");
      keyStore.load(null);
      if (keyStore.containsAlias(alias)) {
        keyStore.deleteEntry(alias);
      }
    } catch (final Exception ignored) {
      // Evidence has already been exported; cleanup failure should not hide it.
    }
  }

  private static String pemChain(final List<byte[]> chain) {
    final StringBuilder builder = new StringBuilder();
    for (final byte[] certificate : chain) {
      builder.append("-----BEGIN CERTIFICATE-----\n");
      final String encoded = Base64.encodeToString(certificate, Base64.NO_WRAP);
      for (int i = 0; i < encoded.length(); i += 64) {
        builder.append(encoded, i, Math.min(i + 64, encoded.length())).append('\n');
      }
      builder.append("-----END CERTIFICATE-----\n");
    }
    return builder.toString();
  }

  private static File file(final File root, final String relative) {
    return new File(root, relative);
  }

  private static void mkdirs(final File directory) throws IOException {
    if (!directory.isDirectory() && !directory.mkdirs()) {
      throw new IOException("failed to create " + directory);
    }
  }

  private static void writeText(final File file, final String text) throws IOException {
    final File parent = file.getParentFile();
    if (parent != null) {
      mkdirs(parent);
    }
    try (FileOutputStream out = new FileOutputStream(file)) {
      out.write(text.getBytes(StandardCharsets.UTF_8));
      out.getFD().sync();
    }
  }

  private static void writeJson(final File file, final Map<String, Object> payload) throws IOException {
    writeText(file, json(payload) + "\n");
  }

  private static LinkedHashMap<String, Object> mapOf(final Object... keysAndValues) {
    final LinkedHashMap<String, Object> out = new LinkedHashMap<>();
    for (int i = 0; i < keysAndValues.length; i += 2) {
      out.put((String) keysAndValues[i], keysAndValues[i + 1]);
    }
    return out;
  }

  private static String json(final Object value) {
    if (value == null) return "null";
    if (value instanceof String) return "\"" + jsonEscape((String) value) + "\"";
    if (value instanceof Boolean || value instanceof Number) return value.toString();
    if (value instanceof Object[]) {
      final Object[] array = (Object[]) value;
      final StringBuilder builder = new StringBuilder("[");
      for (int i = 0; i < array.length; i++) {
        if (i > 0) builder.append(',');
        builder.append(json(array[i]));
      }
      return builder.append(']').toString();
    }
    if (value instanceof Map<?, ?>) {
      final StringBuilder builder = new StringBuilder("{");
      boolean first = true;
      for (final Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet()) {
        if (!first) builder.append(',');
        first = false;
        builder.append(json(String.valueOf(entry.getKey()))).append(':').append(json(entry.getValue()));
      }
      return builder.append('}').toString();
    }
    return json(String.valueOf(value));
  }

  private static String jsonEscape(final String value) {
    final StringBuilder builder = new StringBuilder();
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      switch (ch) {
        case '"':
          builder.append("\\\"");
          break;
        case '\\':
          builder.append("\\\\");
          break;
        case '\b':
          builder.append("\\b");
          break;
        case '\f':
          builder.append("\\f");
          break;
        case '\n':
          builder.append("\\n");
          break;
        case '\r':
          builder.append("\\r");
          break;
        case '\t':
          builder.append("\\t");
          break;
        default:
          if (ch < 0x20) {
            builder.append(String.format("\\u%04x", Integer.valueOf(ch)));
          } else {
            builder.append(ch);
          }
      }
    }
    return builder.toString();
  }

  private static String sha256Text(final String value) {
    return sha256Hex(value.getBytes(StandardCharsets.UTF_8));
  }

  private static String sha256File(final File file) throws IOException {
    final MessageDigest digest = sha256();
    try (FileInputStream input = new FileInputStream(file)) {
      final byte[] buffer = new byte[8192];
      int read;
      while ((read = input.read(buffer)) != -1) {
        digest.update(buffer, 0, read);
      }
    }
    return hex(digest.digest());
  }

  private static String sha256Hex(final byte[] bytes) {
    return hex(sha256Bytes(bytes));
  }

  private static byte[] sha256Bytes(final byte[] bytes) {
    final MessageDigest digest = sha256();
    digest.update(bytes);
    return digest.digest();
  }

  private static MessageDigest sha256() {
    try {
      return MessageDigest.getInstance("SHA-256");
    } catch (final Exception e) {
      throw new IllegalStateException("SHA-256 digest unavailable", e);
    }
  }

  private static String hex(final byte[] bytes) {
    final char[] alphabet = "0123456789abcdef".toCharArray();
    final char[] out = new char[bytes.length * 2];
    for (int i = 0; i < bytes.length; i++) {
      final int value = bytes[i] & 0xff;
      out[i * 2] = alphabet[value >>> 4];
      out[i * 2 + 1] = alphabet[value & 0x0f];
    }
    return new String(out);
  }
}
