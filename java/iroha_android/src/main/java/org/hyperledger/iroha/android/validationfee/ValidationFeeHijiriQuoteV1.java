package org.hyperledger.iroha.android.validationfee;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonParser;

/**
 * Native-verified V1 Hijiri validation-fee quote.
 *
 * <p>The assurance marker is intentionally explicit: the live projection is authenticated by the
 * account-signed transport, but is not an independently witness-verified state proof. Admission
 * later binds the policy and Hijiri hashes and rejects a stale quote.
 */
public final class ValidationFeeHijiriQuoteV1 {
  /** Maximum canonical V1 response size accepted by clients. */
  public static final int MAX_RESPONSE_BYTES = 64 * 1024;

  /** Stable schema marker returned by the native verifier. */
  public static final String SCHEMA =
      "iroha.torii.v1.validation_fee.hijiri_quote.response";

  /** Honest assurance marker for a live state-evaluated quote. */
  public static final String ASSURANCE =
      "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED";

  private static final long U32_MAX = 0xffff_ffffL;
  private static final Set<String> EXACT_FIELDS =
      Set.of(
          "schema",
          "version",
          "assurance",
          "evaluatedStateHeight",
          "quotedExecutionHeight",
          "accountId",
          "activePolicyVersion",
          "activePolicyHash",
          "feeAssetDefinitionId",
          "treasuryAccountId",
          "feeScale",
          "hijiriParametersVersion",
          "hijiriParametersRevision",
          "hijiriParametersDigest",
          "defaultAccountRiskQ16",
          "effectiveAccountRiskQ16",
          "accountRiskRevision",
          "accountRiskDigest",
          "feeMultiplierQ16",
          "hijiriFeeQuoteHash",
          "basePerTransferFeeMinorUnits",
          "adjustedPerTransferFeeMinorUnits",
          "qualifyingTransferCount",
          "aggregateBaseFeeMinorUnits",
          "aggregateAdjustedFeeMinorUnits");

  private final String schema;
  private final int version;
  private final String assurance;
  private final String evaluatedStateHeight;
  private final String quotedExecutionHeight;
  private final String accountId;
  private final String activePolicyVersion;
  private final String activePolicyHash;
  private final String feeAssetDefinitionId;
  private final String treasuryAccountId;
  private final int feeScale;
  private final int hijiriParametersVersion;
  private final String hijiriParametersRevision;
  private final String hijiriParametersDigest;
  private final long defaultAccountRiskQ16;
  private final long effectiveAccountRiskQ16;
  private final String accountRiskRevision;
  private final String accountRiskDigest;
  private final long feeMultiplierQ16;
  private final String hijiriFeeQuoteHash;
  private final String basePerTransferFeeMinorUnits;
  private final String adjustedPerTransferFeeMinorUnits;
  private final int qualifyingTransferCount;
  private final String aggregateBaseFeeMinorUnits;
  private final String aggregateAdjustedFeeMinorUnits;

  private ValidationFeeHijiriQuoteV1(
      final String schema,
      final int version,
      final String assurance,
      final String evaluatedStateHeight,
      final String quotedExecutionHeight,
      final String accountId,
      final String activePolicyVersion,
      final String activePolicyHash,
      final String feeAssetDefinitionId,
      final String treasuryAccountId,
      final int feeScale,
      final int hijiriParametersVersion,
      final String hijiriParametersRevision,
      final String hijiriParametersDigest,
      final long defaultAccountRiskQ16,
      final long effectiveAccountRiskQ16,
      final String accountRiskRevision,
      final String accountRiskDigest,
      final long feeMultiplierQ16,
      final String hijiriFeeQuoteHash,
      final String basePerTransferFeeMinorUnits,
      final String adjustedPerTransferFeeMinorUnits,
      final int qualifyingTransferCount,
      final String aggregateBaseFeeMinorUnits,
      final String aggregateAdjustedFeeMinorUnits) {
    this.schema = schema;
    this.version = version;
    this.assurance = assurance;
    this.evaluatedStateHeight = evaluatedStateHeight;
    this.quotedExecutionHeight = quotedExecutionHeight;
    this.accountId = accountId;
    this.activePolicyVersion = activePolicyVersion;
    this.activePolicyHash = activePolicyHash;
    this.feeAssetDefinitionId = feeAssetDefinitionId;
    this.treasuryAccountId = treasuryAccountId;
    this.feeScale = feeScale;
    this.hijiriParametersVersion = hijiriParametersVersion;
    this.hijiriParametersRevision = hijiriParametersRevision;
    this.hijiriParametersDigest = hijiriParametersDigest;
    this.defaultAccountRiskQ16 = defaultAccountRiskQ16;
    this.effectiveAccountRiskQ16 = effectiveAccountRiskQ16;
    this.accountRiskRevision = accountRiskRevision;
    this.accountRiskDigest = accountRiskDigest;
    this.feeMultiplierQ16 = feeMultiplierQ16;
    this.hijiriFeeQuoteHash = hijiriFeeQuoteHash;
    this.basePerTransferFeeMinorUnits = basePerTransferFeeMinorUnits;
    this.adjustedPerTransferFeeMinorUnits = adjustedPerTransferFeeMinorUnits;
    this.qualifyingTransferCount = qualifyingTransferCount;
    this.aggregateBaseFeeMinorUnits = aggregateBaseFeeMinorUnits;
    this.aggregateAdjustedFeeMinorUnits = aggregateAdjustedFeeMinorUnits;
  }

  public String schema() { return schema; }

  public int version() { return version; }

  public String assurance() { return assurance; }

  public String evaluatedStateHeight() { return evaluatedStateHeight; }

  public String quotedExecutionHeight() { return quotedExecutionHeight; }

  public String accountId() { return accountId; }

  public String activePolicyVersion() { return activePolicyVersion; }

  public String activePolicyHash() { return activePolicyHash; }

  public String feeAssetDefinitionId() { return feeAssetDefinitionId; }

  public String treasuryAccountId() { return treasuryAccountId; }

  public int feeScale() { return feeScale; }

  public int hijiriParametersVersion() { return hijiriParametersVersion; }

  public String hijiriParametersRevision() { return hijiriParametersRevision; }

  public String hijiriParametersDigest() { return hijiriParametersDigest; }

  public long defaultAccountRiskQ16() { return defaultAccountRiskQ16; }

  public long effectiveAccountRiskQ16() { return effectiveAccountRiskQ16; }

  public String accountRiskRevision() { return accountRiskRevision; }

  public String accountRiskDigest() { return accountRiskDigest; }

  public long feeMultiplierQ16() { return feeMultiplierQ16; }

  public String hijiriFeeQuoteHash() { return hijiriFeeQuoteHash; }

  public String basePerTransferFeeMinorUnits() { return basePerTransferFeeMinorUnits; }

  public String adjustedPerTransferFeeMinorUnits() { return adjustedPerTransferFeeMinorUnits; }

  public int qualifyingTransferCount() { return qualifyingTransferCount; }

  public String aggregateBaseFeeMinorUnits() { return aggregateBaseFeeMinorUnits; }

  public String aggregateAdjustedFeeMinorUnits() { return aggregateAdjustedFeeMinorUnits; }

  static ValidationFeeHijiriQuoteV1 parseVerifiedProjection(final byte[] canonicalJsonUtf8) {
    Objects.requireNonNull(canonicalJsonUtf8, "canonicalJsonUtf8");
    if (canonicalJsonUtf8.length == 0 || canonicalJsonUtf8.length > MAX_RESPONSE_BYTES) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection must contain 1.." + MAX_RESPONSE_BYTES + " bytes");
    }
    final String text = new String(canonicalJsonUtf8, StandardCharsets.UTF_8);
    if (!Arrays.equals(text.getBytes(StandardCharsets.UTF_8), canonicalJsonUtf8)) {
      throw new IllegalArgumentException("native Hijiri quote projection is not valid UTF-8");
    }
    final Object parsed = JsonParser.parse(text);
    if (!(parsed instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("native Hijiri quote projection must be an object");
    }
    @SuppressWarnings("unchecked")
    final Map<String, Object> root = (Map<String, Object>) parsed;
    if (!root.keySet().equals(EXACT_FIELDS)) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection fields differ from the frozen V1 schema");
    }
    final ValidationFeeHijiriQuoteV1 quote =
        new ValidationFeeHijiriQuoteV1(
            requiredString(root, "schema"),
            (int) requiredUnsigned(root, "version", 0xffffL),
            requiredString(root, "assurance"),
            requiredString(root, "evaluatedStateHeight"),
            requiredString(root, "quotedExecutionHeight"),
            ValidationFeeHijiriQuoteRequestV1.requireCanonicalAccountId(
                requiredString(root, "accountId"), "accountId"),
            requiredString(root, "activePolicyVersion"),
            requiredString(root, "activePolicyHash"),
            requiredString(root, "feeAssetDefinitionId"),
            requiredString(root, "treasuryAccountId"),
            (int) requiredUnsigned(root, "feeScale", 0xffL),
            (int) requiredUnsigned(root, "hijiriParametersVersion", 0xffffL),
            requiredString(root, "hijiriParametersRevision"),
            requiredString(root, "hijiriParametersDigest"),
            requiredUnsigned(root, "defaultAccountRiskQ16", U32_MAX),
            requiredUnsigned(root, "effectiveAccountRiskQ16", U32_MAX),
            optionalString(root, "accountRiskRevision"),
            optionalString(root, "accountRiskDigest"),
            requiredUnsigned(root, "feeMultiplierQ16", U32_MAX),
            requiredString(root, "hijiriFeeQuoteHash"),
            requiredString(root, "basePerTransferFeeMinorUnits"),
            requiredString(root, "adjustedPerTransferFeeMinorUnits"),
            (int)
                requiredUnsigned(
                    root,
                    "qualifyingTransferCount",
                    ValidationFeeHijiriQuoteRequestV1.MAX_QUALIFYING_TRANSFERS),
            requiredString(root, "aggregateBaseFeeMinorUnits"),
            requiredString(root, "aggregateAdjustedFeeMinorUnits"));
    if (!SCHEMA.equals(quote.schema)) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection has an unsupported schema");
    }
    if (quote.version != ValidationFeeHijiriQuoteRequestV1.VERSION) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection has an unsupported version");
    }
    if (!ASSURANCE.equals(quote.assurance)) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection has an unsupported assurance marker");
    }
    if ((quote.accountRiskRevision == null) != (quote.accountRiskDigest == null)) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection has an incomplete account-risk binding");
    }
    return quote;
  }

  private static String requiredString(final Map<String, Object> root, final String field) {
    final Object value = root.get(field);
    if (!(value instanceof String) || ((String) value).isEmpty()) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection." + field + " must be a non-empty string");
    }
    return (String) value;
  }

  private static String optionalString(final Map<String, Object> root, final String field) {
    if (!root.containsKey(field)) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection." + field + " is missing");
    }
    final Object value = root.get(field);
    if (value == null) {
      return null;
    }
    if (!(value instanceof String) || ((String) value).isEmpty()) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection."
              + field
              + " must be null or a non-empty string");
    }
    return (String) value;
  }

  private static long requiredUnsigned(
      final Map<String, Object> root, final String field, final long maximum) {
    final Object value = root.get(field);
    final BigInteger integer;
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      integer = BigInteger.valueOf(((Number) value).longValue());
    } else if (value instanceof BigInteger) {
      integer = (BigInteger) value;
    } else {
      throw new IllegalArgumentException(
          "native Hijiri quote projection." + field + " must be an integer");
    }
    if (integer.signum() < 0 || integer.compareTo(BigInteger.valueOf(maximum)) > 0) {
      throw new IllegalArgumentException(
          "native Hijiri quote projection." + field + " is outside its unsigned range");
    }
    return integer.longValue();
  }

  @Override
  public boolean equals(final Object other) {
    if (!(other instanceof ValidationFeeHijiriQuoteV1)) {
      return false;
    }
    final ValidationFeeHijiriQuoteV1 quote = (ValidationFeeHijiriQuoteV1) other;
    return version == quote.version
        && feeScale == quote.feeScale
        && hijiriParametersVersion == quote.hijiriParametersVersion
        && defaultAccountRiskQ16 == quote.defaultAccountRiskQ16
        && effectiveAccountRiskQ16 == quote.effectiveAccountRiskQ16
        && feeMultiplierQ16 == quote.feeMultiplierQ16
        && qualifyingTransferCount == quote.qualifyingTransferCount
        && schema.equals(quote.schema)
        && assurance.equals(quote.assurance)
        && evaluatedStateHeight.equals(quote.evaluatedStateHeight)
        && quotedExecutionHeight.equals(quote.quotedExecutionHeight)
        && accountId.equals(quote.accountId)
        && activePolicyVersion.equals(quote.activePolicyVersion)
        && activePolicyHash.equals(quote.activePolicyHash)
        && feeAssetDefinitionId.equals(quote.feeAssetDefinitionId)
        && treasuryAccountId.equals(quote.treasuryAccountId)
        && hijiriParametersRevision.equals(quote.hijiriParametersRevision)
        && hijiriParametersDigest.equals(quote.hijiriParametersDigest)
        && Objects.equals(accountRiskRevision, quote.accountRiskRevision)
        && Objects.equals(accountRiskDigest, quote.accountRiskDigest)
        && hijiriFeeQuoteHash.equals(quote.hijiriFeeQuoteHash)
        && basePerTransferFeeMinorUnits.equals(quote.basePerTransferFeeMinorUnits)
        && adjustedPerTransferFeeMinorUnits.equals(quote.adjustedPerTransferFeeMinorUnits)
        && aggregateBaseFeeMinorUnits.equals(quote.aggregateBaseFeeMinorUnits)
        && aggregateAdjustedFeeMinorUnits.equals(quote.aggregateAdjustedFeeMinorUnits);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        schema,
        version,
        assurance,
        evaluatedStateHeight,
        quotedExecutionHeight,
        accountId,
        activePolicyVersion,
        activePolicyHash,
        feeAssetDefinitionId,
        treasuryAccountId,
        feeScale,
        hijiriParametersVersion,
        hijiriParametersRevision,
        hijiriParametersDigest,
        defaultAccountRiskQ16,
        effectiveAccountRiskQ16,
        accountRiskRevision,
        accountRiskDigest,
        feeMultiplierQ16,
        hijiriFeeQuoteHash,
        basePerTransferFeeMinorUnits,
        adjustedPerTransferFeeMinorUnits,
        qualifyingTransferCount,
        aggregateBaseFeeMinorUnits,
        aggregateAdjustedFeeMinorUnits);
  }
}
