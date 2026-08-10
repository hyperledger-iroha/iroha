package org.hyperledger.iroha.android.alias;

import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.arrayField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.exactKeys;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.intField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.longField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.objectField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.objectValue;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.optionalObject;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.optionalString;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.set;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.stringField;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonParser;

/** Strict sponsored-onboarding response parser. */
public final class AccountOnboardingJsonParser {
  private AccountOnboardingJsonParser() {}

  /** Parses the stateless receipt returned by {@code /v1/accounts/onboard/plan}. */
  public static AccountOnboardingPlanReceiptV1 parseReceipt(final byte[] payload) {
    final Map<String, Object> root = root(payload, "account onboarding receipt");
    exactKeys(root, set("body", "plan_hash", "signature"), "account onboarding receipt");
    return new AccountOnboardingPlanReceiptV1(
        parseBody(objectField(root, "body", "account onboarding receipt.body")),
        stringField(root, "plan_hash", "account onboarding receipt.plan_hash"),
        stringField(root, "signature", "account onboarding receipt.signature"));
  }

  /** Parses queued, repaired, or unchanged apply results. */
  public static AccountOnboardingResponseV1 parseResponse(final byte[] payload) {
    final Map<String, Object> root = root(payload, "account onboarding response");
    final Set<String> allowed = set("account_id", "alias", "tx_hash_hex", "status", "disposition");
    final Set<String> unknown = new HashSet<>(root.keySet());
    unknown.removeAll(allowed);
    if (!unknown.isEmpty()
        || !root.containsKey("account_id")
        || !root.containsKey("alias")
        || !root.containsKey("status")
        || !root.containsKey("disposition")) {
      throw new IllegalStateException("account onboarding response has invalid fields");
    }
    final String statusValue = stringField(root, "status", "account onboarding response.status");
    final AccountOnboardingStatusV1 status;
    if ("Queued".equals(statusValue)) status = AccountOnboardingStatusV1.QUEUED;
    else if ("Repaired".equals(statusValue)) status = AccountOnboardingStatusV1.REPAIRED;
    else if ("Unchanged".equals(statusValue)) status = AccountOnboardingStatusV1.UNCHANGED;
    else throw new IllegalStateException("account onboarding response.status is unsupported");

    final String dispositionValue =
        AliasTransactionPlanJsonParser.parseTaggedVariant(
            objectField(root, "disposition", "account onboarding response.disposition"),
            "kind",
            "account onboarding response.disposition");
    final AliasSetupModels.AliasPlanDispositionV1 disposition;
    if ("no_op".equals(dispositionValue)) disposition = AliasSetupModels.AliasPlanDispositionV1.NO_OP;
    else if ("repair".equals(dispositionValue)) disposition = AliasSetupModels.AliasPlanDispositionV1.REPAIR;
    else if ("create".equals(dispositionValue)) disposition = AliasSetupModels.AliasPlanDispositionV1.CREATE;
    else if ("conflict".equals(dispositionValue)) disposition = AliasSetupModels.AliasPlanDispositionV1.CONFLICT;
    else throw new IllegalStateException("account onboarding response.disposition is unsupported");

    return new AccountOnboardingResponseV1(
        stringField(root, "account_id", "account onboarding response.account_id"),
        stringField(root, "alias", "account onboarding response.alias"),
        optionalString(root, "tx_hash_hex", "account onboarding response.tx_hash_hex"),
        status,
        disposition);
  }

  /** Parses authenticated onboarding readiness diagnostics. */
  public static AliasSetupModels.AliasSetupReportV1 parseReadiness(final byte[] payload) {
    final Map<String, Object> root = root(payload, "account onboarding readiness");
    exactKeys(root, set("version", "status", "diagnostics"), "account onboarding readiness");
    if (intField(root, "version", "account onboarding readiness.version")
        != AliasSetupModels.AliasSetupReportV1.VERSION) {
      throw new IllegalStateException("account onboarding readiness.version is unsupported");
    }
    final String statusValue =
        AliasTransactionPlanJsonParser.parseTaggedVariant(
            objectField(root, "status", "account onboarding readiness.status"),
            "status",
            "account onboarding readiness.status");
    final AliasSetupModels.AliasSetupStatusV1 status;
    if ("ready".equals(statusValue)) status = AliasSetupModels.AliasSetupStatusV1.READY;
    else if ("pending".equals(statusValue)) status = AliasSetupModels.AliasSetupStatusV1.PENDING;
    else if ("blocked".equals(statusValue)) status = AliasSetupModels.AliasSetupStatusV1.BLOCKED;
    else throw new IllegalStateException("account onboarding readiness.status is unsupported");

    final List<Object> values = arrayField(root, "diagnostics", "account onboarding readiness.diagnostics");
    final List<AliasSetupModels.AliasSetupDiagnosticV1> diagnostics =
        new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      final String path = "account onboarding readiness.diagnostics[" + index + "]";
      diagnostics.add(
          AliasTransactionPlanJsonParser.parseDiagnostic(objectValue(values.get(index), path), path));
    }
    return new AliasSetupModels.AliasSetupReportV1(status, diagnostics);
  }

  private static AccountOnboardingPlanBodyV1 parseBody(final Map<String, Object> root) {
    exactKeys(
        root,
        set(
            "version",
            "request",
            "authority",
            "network_id",
            "anchor",
            "resource",
            "acquisition",
            "quote_guard",
            "instructions",
            "owner_auto_renew_instruction",
            "valid_until_ms"),
        "account onboarding receipt.body");
    final Map<String, Object> request = objectField(root, "request", "body.request");
    exactKeys(request, set("version", "alias", "account_id", "permissions"), "body.request");
    final List<Object> permissionValues = arrayField(request, "permissions", "body.request.permissions");
    final List<String> permissions = new ArrayList<>(permissionValues.size());
    for (int index = 0; index < permissionValues.size(); index++) {
      final Object value = permissionValues.get(index);
      if (!(value instanceof String)) {
        throw new IllegalStateException("body.request.permissions[" + index + "] must be a string");
      }
      permissions.add((String) value);
    }
    final Map<String, Object> acquisition = objectField(root, "acquisition", "body.acquisition");
    exactKeys(acquisition, set("term_years", "pricing_class_hint"), "body.acquisition");
    final Object pricingHint = acquisition.get("pricing_class_hint");
    final Map<String, Object> ownerFrame =
        optionalObject(root, "owner_auto_renew_instruction", "body.owner_auto_renew_instruction");
    return new AccountOnboardingPlanBodyV1(
        intField(root, "version", "body.version"),
        new AccountOnboardingPlanRequestV1(
            intField(request, "version", "body.request.version"),
            stringField(request, "alias", "body.request.alias"),
            stringField(request, "account_id", "body.request.account_id"),
            permissions),
        stringField(root, "authority", "body.authority"),
        org.hyperledger.iroha.android.model.NetworkId.parse(
            stringField(root, "network_id", "body.network_id")),
        AliasTransactionPlanJsonParser.parseAnchor(objectField(root, "anchor", "body.anchor")),
        AliasTransactionPlanJsonParser.parseResource(
            objectField(root, "resource", "body.resource"), "body.resource"),
        new AliasSetupModels.AliasLeaseAcquisitionV1(
            intField(acquisition, "term_years", "body.acquisition.term_years"),
            pricingHint == null
                ? null
                : Integer.valueOf(
                    intField(
                        acquisition,
                        "pricing_class_hint",
                        "body.acquisition.pricing_class_hint"))),
        AliasTransactionPlanJsonParser.parseGuard(
            objectField(root, "quote_guard", "body.quote_guard"), "body.quote_guard"),
        parseFrames(root),
        ownerFrame == null
            ? null
            : AliasTransactionPlanJsonParser.parseFrame(
                ownerFrame, "body.owner_auto_renew_instruction"),
        longField(root, "valid_until_ms", "body.valid_until_ms"));
  }

  private static List<AliasSetupModels.AliasFramedInstructionV1> parseFrames(
      final Map<String, Object> root) {
    final List<Object> values = arrayField(root, "instructions", "body.instructions");
    final List<AliasSetupModels.AliasFramedInstructionV1> result = new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      final String path = "body.instructions[" + index + "]";
      result.add(AliasTransactionPlanJsonParser.parseFrame(objectValue(values.get(index), path), path));
    }
    return result;
  }

  private static Map<String, Object> root(final byte[] payload, final String path) {
    if (payload == null || payload.length == 0) {
      throw new IllegalStateException(path + " returned an empty payload");
    }
    return objectValue(JsonParser.parse(new String(payload, StandardCharsets.UTF_8)), path);
  }
}
