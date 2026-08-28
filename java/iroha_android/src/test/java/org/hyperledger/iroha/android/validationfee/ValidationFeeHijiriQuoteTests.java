package org.hyperledger.iroha.android.validationfee;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.hyperledger.iroha.android.testing.TestAccountIds;

/** Focused managed-boundary tests for native Hijiri validation-fee quotes. */
public final class ValidationFeeHijiriQuoteTests {
  private ValidationFeeHijiriQuoteTests() {}

  public static void main(final String[] args) {
    requestRejectsNoncanonicalAccountAndUnboundedCount();
    projectionParserExposesEveryExactVerifiedField();
    projectionParserRejectsFieldDriftAndIncompleteRiskBinding();
    bridgeRejectsInvalidByteBoundsBeforeNativeLoading();
    missingAdditiveJniMethodsFailWithStableCapabilityError();
    freshNativeBridgeEncodesAndVerifiesThroughJavaJniNames();
  }

  private static void requestRejectsNoncanonicalAccountAndUnboundedCount() {
    final String accountId = TestAccountIds.ed25519Authority(0x51);
    final ValidationFeeHijiriQuoteRequestV1 request =
        new ValidationFeeHijiriQuoteRequestV1(accountId, 2);
    assert request.version() == 1 : "request version mismatch";
    assert accountId.equals(request.accountId()) : "request account mismatch";
    assert request.qualifyingTransferCount() == 2 : "request count mismatch";

    expectIllegalArgument(
        () -> new ValidationFeeHijiriQuoteRequestV1(" " + accountId, 1),
        "padded account must be rejected");
    expectIllegalArgument(
        () -> new ValidationFeeHijiriQuoteRequestV1("alice@wonderland", 1),
        "domain-qualified alias must be rejected");
    expectIllegalArgument(
        () -> new ValidationFeeHijiriQuoteRequestV1(accountId, 0),
        "zero transfer count must be rejected");
    expectIllegalArgument(
        () ->
            new ValidationFeeHijiriQuoteRequestV1(
                accountId, ValidationFeeHijiriQuoteRequestV1.MAX_QUALIFYING_TRANSFERS + 1),
        "excessive transfer count must be rejected");
  }

  private static void projectionParserExposesEveryExactVerifiedField() {
    final String accountId = TestAccountIds.ed25519Authority(0x51);
    final ValidationFeeHijiriQuoteV1 quote =
        ValidationFeeHijiriQuoteV1.parseVerifiedProjection(projectionJson(accountId));

    assert ValidationFeeHijiriQuoteV1.SCHEMA.equals(quote.schema()) : "schema mismatch";
    assert ValidationFeeHijiriQuoteV1.ASSURANCE.equals(quote.assurance())
        : "assurance mismatch";
    assert "42".equals(quote.evaluatedStateHeight()) : "state height mismatch";
    assert "43".equals(quote.quotedExecutionHeight()) : "execution height mismatch";
    assert accountId.equals(quote.accountId()) : "account mismatch";
    assert quote.feeMultiplierQ16() == 65_536L : "multiplier mismatch";
    assert quote.qualifyingTransferCount() == 2 : "transfer count mismatch";
    assert "20".equals(quote.aggregateAdjustedFeeMinorUnits()) : "aggregate mismatch";
    assert quote.accountRiskRevision() == null : "risk revision must be absent";
    assert quote.accountRiskDigest() == null : "risk digest must be absent";
  }

  private static void projectionParserRejectsFieldDriftAndIncompleteRiskBinding() {
    final String canonical =
        new String(
            projectionJson(TestAccountIds.ed25519Authority(0x51)),
            StandardCharsets.UTF_8);
    final String unknown = canonical.substring(0, canonical.length() - 1)
        + ",\"unexpected\":true}";
    expectIllegalArgument(
        () ->
            ValidationFeeHijiriQuoteV1.parseVerifiedProjection(
                unknown.getBytes(StandardCharsets.UTF_8)),
        "unknown projection fields must be rejected");
    final String incomplete =
        canonical.replace("\"accountRiskRevision\":null", "\"accountRiskRevision\":\"1\"");
    expectIllegalArgument(
        () ->
            ValidationFeeHijiriQuoteV1.parseVerifiedProjection(
                incomplete.getBytes(StandardCharsets.UTF_8)),
        "incomplete account-risk binding must be rejected");
    final String excessiveCount =
        canonical.replace(
            "\"qualifyingTransferCount\":2", "\"qualifyingTransferCount\":100001");
    expectIllegalArgument(
        () ->
            ValidationFeeHijiriQuoteV1.parseVerifiedProjection(
                excessiveCount.getBytes(StandardCharsets.UTF_8)),
        "excessive echoed transfer count must be rejected");
  }

  private static void bridgeRejectsInvalidByteBoundsBeforeNativeLoading() {
    expectIllegalArgument(
        () -> ValidationFeeHijiriQuoteBridge.verifyResponseV1(new byte[0], new byte[] {1}),
        "empty response must be rejected before native loading");
    expectIllegalArgument(
        () ->
            ValidationFeeHijiriQuoteBridge.verifyResponseV1(
                new byte[] {1},
                new byte[ValidationFeeHijiriQuoteRequestV1.MAX_REQUEST_BYTES + 1]),
        "oversized request must be rejected before native loading");
  }

  private static void missingAdditiveJniMethodsFailWithStableCapabilityError() {
    for (final String method :
        new String[] {"nativeEncodeRequestV1", "nativeVerifyResponseV1"}) {
      try {
        ValidationFeeHijiriQuoteBridge.invokeRequiredQuoteNative(
            method,
            () -> {
              throw new UnsatisfiedLinkError("missing JNI sentinel");
            });
        throw new AssertionError("missing JNI method must fail closed: " + method);
      } catch (final IllegalStateException failure) {
        final String expected =
            "native Hijiri validation-fee quote bridge is unavailable: required ABI-23 method "
                + method
                + " is missing";
        assert expected.equals(failure.getMessage()) : "unstable missing-JNI capability error";
        assert failure.getCause() instanceof UnsatisfiedLinkError
            : "missing-JNI capability cause must be retained";
      }
    }
  }

  private static void freshNativeBridgeEncodesAndVerifiesThroughJavaJniNames() {
    final ValidationFeeHijiriQuoteRequestV1 request =
        new ValidationFeeHijiriQuoteRequestV1(TestAccountIds.ed25519Authority(0x51), 2);
    final byte[] first = ValidationFeeHijiriQuoteBridge.encodeRequestV1(request);
    final byte[] second = ValidationFeeHijiriQuoteBridge.encodeRequestV1(request);

    assert first.length > 0 : "native quote request must be non-empty";
    assert first.length <= ValidationFeeHijiriQuoteRequestV1.MAX_REQUEST_BYTES
        : "native quote request exceeds its bound";
    assert Arrays.equals(first, second) : "native quote request encoding must be deterministic";
    expectIllegalArgument(
        () -> ValidationFeeHijiriQuoteBridge.verifyResponseV1(new byte[] {0}, first),
        "malformed native quote response must be rejected");
  }

  private static byte[] projectionJson(final String accountId) {
    final String json =
        "{\"schema\":\""
            + ValidationFeeHijiriQuoteV1.SCHEMA
            + "\",\"version\":1,\"assurance\":\""
            + ValidationFeeHijiriQuoteV1.ASSURANCE
            + "\",\"evaluatedStateHeight\":\"42\",\"quotedExecutionHeight\":\"43\","
            + "\"accountId\":\""
            + accountId
            + "\",\"activePolicyVersion\":\"1\",\"activePolicyHash\":\""
            + "03".repeat(32)
            + "\",\"feeAssetDefinitionId\":\"asset\",\"treasuryAccountId\":\""
            + accountId
            + "\",\"feeScale\":2,\"hijiriParametersVersion\":1,"
            + "\"hijiriParametersRevision\":\"1\",\"hijiriParametersDigest\":\""
            + "05".repeat(32)
            + "\",\"defaultAccountRiskQ16\":0,\"effectiveAccountRiskQ16\":0,"
            + "\"accountRiskRevision\":null,\"accountRiskDigest\":null,"
            + "\"feeMultiplierQ16\":65536,\"hijiriFeeQuoteHash\":\""
            + "07".repeat(32)
            + "\",\"basePerTransferFeeMinorUnits\":\"10\","
            + "\"adjustedPerTransferFeeMinorUnits\":\"10\","
            + "\"qualifyingTransferCount\":2,\"aggregateBaseFeeMinorUnits\":\"20\","
            + "\"aggregateAdjustedFeeMinorUnits\":\"20\"}";
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static void expectIllegalArgument(final Runnable action, final String message) {
    boolean rejected = false;
    try {
      action.run();
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : message;
  }
}
