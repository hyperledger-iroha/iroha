package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Immutable metadata describing a cached attestation verdict for an allowance. */
public final class OfflineVerdictMetadata {

  private final String certificateIdHex;
  private final String controllerId;
  private final String controllerDisplay;
  private final String verdictIdHex;
  private final String attestationNonceHex;
  private final long certificateExpiresAtMs;
  private final long policyExpiresAtMs;
  private final Long refreshAtMs;
  private final String remainingAmount;
  private final long recordedAtMs;
  private final String integrityPolicy;
  private final PlayIntegrityMetadata playIntegrity;
  private final PlayIntegrityTokenSnapshot playIntegrityToken;
  private final SafetyDetectMetadata hmsSafetyDetect;
  private final SafetyDetectTokenSnapshot hmsSafetyDetectToken;
  private final ProvisionedMetadata provisionedMetadata;

  public OfflineVerdictMetadata(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final String verdictIdHex,
      final String attestationNonceHex,
      final long certificateExpiresAtMs,
      final long policyExpiresAtMs,
      final Long refreshAtMs,
      final String remainingAmount,
      final long recordedAtMs,
      final String integrityPolicy,
      final PlayIntegrityMetadata playIntegrity,
      final PlayIntegrityTokenSnapshot playIntegrityToken,
      final SafetyDetectMetadata hmsSafetyDetect,
      final SafetyDetectTokenSnapshot hmsSafetyDetectToken,
      final ProvisionedMetadata provisionedMetadata) {
    this.certificateIdHex = Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    this.controllerId = Objects.requireNonNull(controllerId, "controllerId");
    this.controllerDisplay = Objects.requireNonNull(controllerDisplay, "controllerDisplay");
    this.verdictIdHex = verdictIdHex;
    this.attestationNonceHex = attestationNonceHex;
    this.certificateExpiresAtMs = certificateExpiresAtMs;
    this.policyExpiresAtMs = policyExpiresAtMs;
    this.refreshAtMs = refreshAtMs;
    this.remainingAmount =
        remainingAmount == null || remainingAmount.isBlank() ? "0" : remainingAmount.trim();
    this.recordedAtMs = recordedAtMs;
    this.integrityPolicy = integrityPolicy;
    this.playIntegrity = playIntegrity;
    this.playIntegrityToken = playIntegrityToken;
    this.hmsSafetyDetect = hmsSafetyDetect;
    this.hmsSafetyDetectToken = hmsSafetyDetectToken;
    this.provisionedMetadata = provisionedMetadata;
  }

  public OfflineVerdictMetadata(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final String verdictIdHex,
      final String attestationNonceHex,
      final long certificateExpiresAtMs,
      final long policyExpiresAtMs,
      final Long refreshAtMs,
      final String remainingAmount,
      final long recordedAtMs) {
    this(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        verdictIdHex,
        attestationNonceHex,
        certificateExpiresAtMs,
        policyExpiresAtMs,
        refreshAtMs,
        remainingAmount,
        recordedAtMs,
        null,
        null,
        null,
        null,
        null,
        null);
  }

  public String certificateIdHex() {
    return certificateIdHex;
  }

  public String controllerId() {
    return controllerId;
  }

  public String controllerDisplay() {
    return controllerDisplay;
  }

  public String verdictIdHex() {
    return verdictIdHex;
  }

  public String attestationNonceHex() {
    return attestationNonceHex;
  }

  public long certificateExpiresAtMs() {
    return certificateExpiresAtMs;
  }

  public long policyExpiresAtMs() {
    return policyExpiresAtMs;
  }

  public Long refreshAtMs() {
    return refreshAtMs;
  }

  public String remainingAmount() {
    return remainingAmount;
  }

  public long recordedAtMs() {
    return recordedAtMs;
  }

  public String integrityPolicy() {
    return integrityPolicy;
  }

  public PlayIntegrityMetadata playIntegrity() {
    return playIntegrity;
  }

  public PlayIntegrityTokenSnapshot playIntegrityToken() {
    return playIntegrityToken;
  }

  public SafetyDetectMetadata hmsSafetyDetect() {
    return hmsSafetyDetect;
  }

  public SafetyDetectTokenSnapshot hmsSafetyDetectToken() {
    return hmsSafetyDetectToken;
  }

  public ProvisionedMetadata provisionedMetadata() {
    return provisionedMetadata;
  }

  public OfflineVerdictMetadata withSafetyDetectToken(
      final SafetyDetectTokenSnapshot tokenSnapshot) {
    return new OfflineVerdictMetadata(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        verdictIdHex,
        attestationNonceHex,
        certificateExpiresAtMs,
        policyExpiresAtMs,
        refreshAtMs,
        remainingAmount,
        recordedAtMs,
        integrityPolicy,
        playIntegrity,
        playIntegrityToken,
        hmsSafetyDetect,
        tokenSnapshot,
        provisionedMetadata);
  }

  public OfflineVerdictMetadata withPlayIntegrityToken(
      final PlayIntegrityTokenSnapshot tokenSnapshot) {
    return new OfflineVerdictMetadata(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        verdictIdHex,
        attestationNonceHex,
        certificateExpiresAtMs,
        policyExpiresAtMs,
        refreshAtMs,
        remainingAmount,
        recordedAtMs,
        integrityPolicy,
        playIntegrity,
        tokenSnapshot,
        hmsSafetyDetect,
        hmsSafetyDetectToken,
        provisionedMetadata);
  }

  Map<String, Object> toJson() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("controller_id", controllerId);
    map.put("controller_display", controllerDisplay);
    map.put("verdict_id_hex", verdictIdHex);
    map.put("attestation_nonce_hex", attestationNonceHex);
    map.put("certificate_expires_at_ms", certificateExpiresAtMs);
    map.put("policy_expires_at_ms", policyExpiresAtMs);
    map.put("refresh_at_ms", refreshAtMs);
    map.put("remaining_amount", remainingAmount);
    map.put("recorded_at_ms", recordedAtMs);
    if (integrityPolicy != null && !integrityPolicy.isBlank()) {
      map.put("integrity_policy", integrityPolicy);
    }
    if (playIntegrity != null) {
      map.put("play_integrity", playIntegrity.toJson());
    }
    if (playIntegrityToken != null) {
      map.put("play_integrity_token", playIntegrityToken.toJson());
    }
    if (hmsSafetyDetect != null) {
      map.put("hms_safety_detect", hmsSafetyDetect.toJson());
    }
    if (hmsSafetyDetectToken != null) {
      map.put("hms_safety_detect_token", hmsSafetyDetectToken.toJson());
    }
    if (provisionedMetadata != null) {
      map.put("android_provisioned", provisionedMetadata.toJson());
    }
    return map;
  }

  @SuppressWarnings("unchecked")
  static OfflineVerdictMetadata fromJson(final String certificateIdHex, final Object value) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException("verdict entry for " + certificateIdHex + " is not an object");
    }
    final Map<String, Object> map = (Map<String, Object>) value;
    final String controllerId = asString(map.get("controller_id"), certificateIdHex + ".controller");
    final String controllerDisplay =
        asString(map.get("controller_display"), certificateIdHex + ".controller_display");
    final String verdictIdHex = asOptionalString(map.get("verdict_id_hex"));
    final String attestationNonceHex = asOptionalString(map.get("attestation_nonce_hex"));
    final long certificateExpiresAtMs =
        asLong(map.get("certificate_expires_at_ms"), certificateIdHex + ".certificate_expires_at_ms");
    final long policyExpiresAtMs =
        asLong(map.get("policy_expires_at_ms"), certificateIdHex + ".policy_expires_at_ms");
    final Long refreshAtMs = asOptionalLong(map.get("refresh_at_ms"));
    final String remainingAmount =
        asOptionalString(map.get("remaining_amount")) == null
            ? "0"
            : asOptionalString(map.get("remaining_amount"));
    final long recordedAtMs = asLong(map.get("recorded_at_ms"), certificateIdHex + ".recorded_at_ms");
    final String integrityPolicy = asOptionalString(map.get("integrity_policy"));
    final PlayIntegrityMetadata playIntegrity =
        map.containsKey("play_integrity")
            ? PlayIntegrityMetadata.fromJson(map.get("play_integrity"))
            : null;
    final PlayIntegrityTokenSnapshot playIntegrityToken =
        map.containsKey("play_integrity_token")
            ? PlayIntegrityTokenSnapshot.fromJson(map.get("play_integrity_token"))
            : null;
    final SafetyDetectMetadata hms =
        map.containsKey("hms_safety_detect")
            ? SafetyDetectMetadata.fromJson(map.get("hms_safety_detect"))
            : null;
    final SafetyDetectTokenSnapshot token =
        map.containsKey("hms_safety_detect_token")
            ? SafetyDetectTokenSnapshot.fromJson(map.get("hms_safety_detect_token"))
            : null;
    final ProvisionedMetadata provisioned =
        map.containsKey("android_provisioned")
            ? ProvisionedMetadata.fromJson(map.get("android_provisioned"))
            : null;
    return new OfflineVerdictMetadata(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        verdictIdHex,
        attestationNonceHex,
        certificateExpiresAtMs,
        policyExpiresAtMs,
        refreshAtMs,
        remainingAmount,
        recordedAtMs,
        integrityPolicy,
        playIntegrity,
        playIntegrityToken,
        hms,
        token,
        provisioned);
  }

  private static long asLong(final Object value, final String field) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(field + " is not a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(field + " must be an integer");
    }
    return number.longValue();
  }

  private static Long asOptionalLong(final Object value) {
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        return null;
      }
      return number.longValue();
    }
    return null;
  }

  private static String asString(final Object value, final String field) {
    if (value == null) {
      throw new IllegalStateException(field + " is missing");
    }
    if (value instanceof String string) {
      return string;
    }
    return String.valueOf(value);
  }

  private static String asOptionalString(final Object value) {
    if (value == null) {
      return null;
    }
    return value instanceof String string ? string : String.valueOf(value);
  }

  /** Subset of Play Integrity metadata tracked for offline policy enforcement. */
  public static final class PlayIntegrityMetadata {
    private final long cloudProjectNumber;
    private final String environment;
    private final List<String> packageNames;
    private final List<String> signingDigestsSha256;
    private final List<String> allowedAppVerdicts;
    private final List<String> allowedDeviceVerdicts;
    private final Long maxTokenAgeMs;

    public PlayIntegrityMetadata(
        final long cloudProjectNumber,
        final String environment,
        final List<String> packageNames,
        final List<String> signingDigestsSha256,
        final List<String> allowedAppVerdicts,
        final List<String> allowedDeviceVerdicts,
        final Long maxTokenAgeMs) {
      this.cloudProjectNumber = cloudProjectNumber;
      this.environment = environment;
      this.packageNames =
          packageNames == null ? List.of() : List.copyOf(packageNames);
      this.signingDigestsSha256 =
          signingDigestsSha256 == null ? List.of() : List.copyOf(signingDigestsSha256);
      this.allowedAppVerdicts =
          allowedAppVerdicts == null ? List.of() : List.copyOf(allowedAppVerdicts);
      this.allowedDeviceVerdicts =
          allowedDeviceVerdicts == null ? List.of() : List.copyOf(allowedDeviceVerdicts);
      this.maxTokenAgeMs = maxTokenAgeMs;
    }

    public long cloudProjectNumber() {
      return cloudProjectNumber;
    }

    public String environment() {
      return environment;
    }

    public List<String> packageNames() {
      return packageNames;
    }

    public List<String> signingDigestsSha256() {
      return signingDigestsSha256;
    }

    public List<String> allowedAppVerdicts() {
      return allowedAppVerdicts;
    }

    public List<String> allowedDeviceVerdicts() {
      return allowedDeviceVerdicts;
    }

    public Long maxTokenAgeMs() {
      return maxTokenAgeMs;
    }

    Map<String, Object> toJson() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("cloud_project_number", cloudProjectNumber);
      map.put("environment", environment);
      map.put("package_names", packageNames);
      map.put("signing_digests_sha256", signingDigestsSha256);
      map.put("allowed_app_verdicts", allowedAppVerdicts);
      map.put("allowed_device_verdicts", allowedDeviceVerdicts);
      map.put("max_token_age_ms", maxTokenAgeMs);
      return map;
    }

    static PlayIntegrityMetadata fromJson(final Object value) {
      if (!(value instanceof Map<?, ?> raw)) {
        return null;
      }
      final Long project = asOptionalLong(raw.get("cloud_project_number"));
      final String environment = asOptionalString(raw.get("environment"));
      if (project == null || project <= 0 || environment == null || environment.isBlank()) {
        return null;
      }
      final List<String> packages = asStringList(raw.get("package_names"));
      final List<String> digests = asStringList(raw.get("signing_digests_sha256"));
      final List<String> appVerdicts = asStringList(raw.get("allowed_app_verdicts"));
      final List<String> deviceVerdicts = asStringList(raw.get("allowed_device_verdicts"));
      if (packages.isEmpty() || digests.isEmpty() || appVerdicts.isEmpty() || deviceVerdicts.isEmpty()) {
        return null;
      }
      final Long maxAge = asOptionalLong(raw.get("max_token_age_ms"));
      return new PlayIntegrityMetadata(
          project,
          environment.trim(),
          packages,
          digests,
          appVerdicts,
          deviceVerdicts,
          maxAge);
    }
  }

  /** Snapshot of the cached Play Integrity attestation token. */
  public static final class PlayIntegrityTokenSnapshot {
    private final String token;
    private final long fetchedAtMs;

    public PlayIntegrityTokenSnapshot(final String token, final long fetchedAtMs) {
      this.token = Objects.requireNonNull(token, "token");
      this.fetchedAtMs = fetchedAtMs;
    }

    public String token() {
      return token;
    }

    public long fetchedAtMs() {
      return fetchedAtMs;
    }

    Map<String, Object> toJson() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("token", token);
      map.put("fetched_at_ms", fetchedAtMs);
      return map;
    }

    static PlayIntegrityTokenSnapshot fromJson(final Object value) {
      if (!(value instanceof Map<?, ?> raw)) {
        return null;
      }
      final Object tokenValue = raw.get("token");
      final Object fetchedValue = raw.get("fetched_at_ms");
      if (!(tokenValue instanceof String tokenString) || tokenString.isBlank()) {
        return null;
      }
      if (!(fetchedValue instanceof Number number)) {
        return null;
      }
      if (number instanceof Float || number instanceof Double) {
        return null;
      }
      return new PlayIntegrityTokenSnapshot(tokenString, number.longValue());
    }
  }

  /** Subset of HMS Safety Detect metadata tracked for offline policy enforcement. */
  public static final class SafetyDetectMetadata {
    private final String appId;
    private final List<String> packageNames;
    private final List<String> signingDigestsSha256;
    private final List<String> requiredEvaluations;
    private final Long maxTokenAgeMs;

    public SafetyDetectMetadata(
        final String appId,
        final List<String> packageNames,
        final List<String> signingDigestsSha256,
        final List<String> requiredEvaluations,
        final Long maxTokenAgeMs) {
      this.appId = appId;
      this.packageNames =
          packageNames == null ? List.of() : List.copyOf(packageNames);
      this.signingDigestsSha256 =
          signingDigestsSha256 == null ? List.of() : List.copyOf(signingDigestsSha256);
      this.requiredEvaluations =
          requiredEvaluations == null ? List.of() : List.copyOf(requiredEvaluations);
      this.maxTokenAgeMs = maxTokenAgeMs;
    }

    public String appId() {
      return appId;
    }

    public List<String> packageNames() {
      return packageNames;
    }

    public List<String> signingDigestsSha256() {
      return signingDigestsSha256;
    }

    public List<String> requiredEvaluations() {
      return requiredEvaluations;
    }

    public Long maxTokenAgeMs() {
      return maxTokenAgeMs;
    }

    Map<String, Object> toJson() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("app_id", appId);
      map.put("package_names", packageNames);
      map.put("signing_digests_sha256", signingDigestsSha256);
      map.put("required_evaluations", requiredEvaluations);
      map.put("max_token_age_ms", maxTokenAgeMs);
      return map;
    }

    static SafetyDetectMetadata fromJson(final Object value) {
      if (!(value instanceof Map<?, ?> raw)) {
        return null;
      }
      final String appId = asOptionalString(raw.get("app_id"));
      final List<String> packages = asStringList(raw.get("package_names"));
      final List<String> digests = asStringList(raw.get("signing_digests_sha256"));
      final List<String> evaluations = asStringList(raw.get("required_evaluations"));
      final Long maxAge = asOptionalLong(raw.get("max_token_age_ms"));
      return new SafetyDetectMetadata(appId, packages, digests, evaluations, maxAge);
    }
  }

  /** Snapshot of the cached HMS Safety Detect attestation token. */
  public static final class SafetyDetectTokenSnapshot {
    private final String token;
    private final long fetchedAtMs;

    public SafetyDetectTokenSnapshot(final String token, final long fetchedAtMs) {
      this.token = Objects.requireNonNull(token, "token");
      this.fetchedAtMs = fetchedAtMs;
    }

    public String token() {
      return token;
    }

    public long fetchedAtMs() {
      return fetchedAtMs;
    }

    Map<String, Object> toJson() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("token", token);
      map.put("fetched_at_ms", fetchedAtMs);
      return map;
    }

    static SafetyDetectTokenSnapshot fromJson(final Object value) {
      if (!(value instanceof Map<?, ?> raw)) {
        return null;
      }
      final Object tokenValue = raw.get("token");
      final Object fetchedValue = raw.get("fetched_at_ms");
      if (!(tokenValue instanceof String tokenString) || tokenString.isBlank()) {
        return null;
      }
      if (!(fetchedValue instanceof Number number)) {
        return null;
      }
      if (number instanceof Float || number instanceof Double) {
        return null;
      }
      return new SafetyDetectTokenSnapshot(tokenString, number.longValue());
    }
  }

  /** Subset of provisioned inspector metadata tracked for policy enforcement. */
  public static final class ProvisionedMetadata {
    private final String inspectorPublicKey;
    private final String manifestSchema;
    private final Integer manifestVersion;
    private final Long maxManifestAgeMs;
    private final String manifestDigest;

    public ProvisionedMetadata(
        final String inspectorPublicKey,
        final String manifestSchema,
        final Integer manifestVersion,
        final Long maxManifestAgeMs,
        final String manifestDigest) {
      this.inspectorPublicKey =
          inspectorPublicKey == null ? null : inspectorPublicKey.trim();
      this.manifestSchema = manifestSchema == null ? null : manifestSchema.trim();
      this.manifestVersion = manifestVersion;
      this.maxManifestAgeMs = maxManifestAgeMs;
      this.manifestDigest =
          manifestDigest == null || manifestDigest.isBlank() ? null : manifestDigest.trim();
    }

    public String inspectorPublicKey() {
      return inspectorPublicKey;
    }

    public String manifestSchema() {
      return manifestSchema;
    }

    public Integer manifestVersion() {
      return manifestVersion;
    }

    public Long maxManifestAgeMs() {
      return maxManifestAgeMs;
    }

    public String manifestDigest() {
      return manifestDigest;
    }

    Map<String, Object> toJson() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("inspector_public_key", inspectorPublicKey);
      map.put("manifest_schema", manifestSchema);
      map.put("manifest_version", manifestVersion);
      map.put("max_manifest_age_ms", maxManifestAgeMs);
      map.put("manifest_digest", manifestDigest);
      return map;
    }

    static ProvisionedMetadata fromJson(final Object value) {
      if (!(value instanceof Map<?, ?> raw)) {
        return null;
      }
      final String inspector = asOptionalString(raw.get("inspector_public_key"));
      final String schema = asOptionalString(raw.get("manifest_schema"));
      if (inspector == null || inspector.isBlank() || schema == null || schema.isBlank()) {
        return null;
      }
      final Integer version = asOptionalInteger(raw.get("manifest_version"));
      final Long maxAge = asOptionalLong(raw.get("max_manifest_age_ms"));
      final String digest = asOptionalString(raw.get("manifest_digest"));
      return new ProvisionedMetadata(inspector, schema, version, maxAge, digest);
    }
  }

  private static List<String> asStringList(final Object value) {
    if (!(value instanceof List<?> list)) {
      return List.of();
    }
    final List<String> out = new ArrayList<>(list.size());
    for (final Object entry : list) {
      if (entry != null) {
        out.add(String.valueOf(entry));
      }
    }
    return Collections.unmodifiableList(out);
  }

  private static Integer asOptionalInteger(final Object value) {
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        return null;
      }
      return number.intValue();
    }
    if (value == null) {
      return null;
    }
    try {
      return Integer.parseInt(String.valueOf(value));
    } catch (final NumberFormatException ignored) {
      return null;
    }
  }
}
