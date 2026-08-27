package org.hyperledger.iroha.android.alias;

import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.arrayField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.exactKeys;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.intField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.longField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.objectField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.objectValue;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.optionalObject;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.set;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.stringField;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.FeePaymentJson;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.numeric.NumericV1;

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

  /** Parses the closed prepared-or-authenticated-proof-required response. */
  public static AccountOnboardingPrepareResponseV1 parsePrepareResponse(final byte[] payload) {
    final Map<String, Object> root = root(payload, "account onboarding prepare response");
    final String schema =
        stringField(root, "schema", "account onboarding prepare response.schema");
    if (AccountOnboardingPreparedTransactionV1.SCHEMA.equals(schema)) return parsePrepared(root);
    if (AccountOnboardingProofRequiredPrepareResponseV1.SCHEMA.equals(schema)) {
      return parseProofRequired(root);
    }
    throw new IllegalStateException("account onboarding prepare response.schema is unsupported");
  }

  /** Parses one exact authenticated faucet prepared transaction. */
  public static AccountFaucetPreparedTransactionV1 parseFaucetPrepareResponse(
      final byte[] payload) {
    return parseFaucetPrepared(root(payload, "account faucet prepare response"));
  }

  /** Parses one closed atomic account-onboarding current-state response. */
  public static AccountOnboardingCurrentStateResponseV1 parseCurrentStateResponse(
      final byte[] payload) {
    final String path = "account onboarding current-state response";
    final Map<String, Object> root = root(payload, path);
    exactKeys(
        root,
        set(
            "version",
            "network_id",
            "account_id",
            "alias",
            "account_exists",
            "alias_target_account_id",
            "observed_block_height",
            "observed_block_hash"),
        path);
    final Object accountExistsValue = root.get("account_exists");
    if (!(accountExistsValue instanceof Boolean)) {
      throw new IllegalStateException(path + ".account_exists must be a boolean");
    }
    final Object aliasTargetValue = root.get("alias_target_account_id");
    if (aliasTargetValue != null && !(aliasTargetValue instanceof String)) {
      throw new IllegalStateException(
          path + ".alias_target_account_id must be a string or null");
    }
    return new AccountOnboardingCurrentStateResponseV1(
        intField(root, "version", path + ".version"),
        NetworkId.parse(stringField(root, "network_id", path + ".network_id")),
        stringField(root, "account_id", path + ".account_id"),
        stringField(root, "alias", path + ".alias"),
        ((Boolean) accountExistsValue).booleanValue(),
        (String) aliasTargetValue,
        positiveU64(root.get("observed_block_height"), path + ".observed_block_height"),
        new AccountOnboardingBlockHashV1(
            stringField(root, "observed_block_hash", path + ".observed_block_hash")));
  }

  /** Parses a submit response bound to one exact prepared transaction. */
  public static PreparedTransactionSubmitResponseV1 parseSubmitResponse(final byte[] payload) {
    final String path = "prepared transaction submit response";
    final Map<String, Object> root = root(payload, path);
    exactKeys(root, set("schema", "binding", "operation", "transaction_hash_hex", "outcome"), path);
    final String outcomeValue = stringField(root, "outcome", path + ".outcome");
    final PreparedTransactionOutcomeV1 outcome;
    if ("Applied".equals(outcomeValue)) outcome = PreparedTransactionOutcomeV1.APPLIED;
    else if ("Pending".equals(outcomeValue)) outcome = PreparedTransactionOutcomeV1.PENDING;
    else if ("Rejected".equals(outcomeValue)) outcome = PreparedTransactionOutcomeV1.REJECTED;
    else throw new IllegalStateException(path + ".outcome is unsupported");
    return new PreparedTransactionSubmitResponseV1(
        stringField(root, "schema", path + ".schema"),
        parseBinding(objectField(root, "binding", path + ".binding")),
        stringField(root, "operation", path + ".operation"),
        stringField(root, "transaction_hash_hex", path + ".transaction_hash_hex"),
        outcome);
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

  private static AccountOnboardingPreparedTransactionV1 parsePrepared(
      final Map<String, Object> root) {
    final String path = "prepared onboarding transaction";
    exactKeys(
        root,
        set(
            "schema", "binding", "operation", "receipt", "semantic_hash_hex", "account_id",
            "alias", "disposition", "transaction_hash_hex", "signed_transaction_wire_hex",
            "signed_transaction_wire_sha256", "fee_payment", "server_signature"),
        path);
    return new AccountOnboardingPreparedTransactionV1(
        stringField(root, "schema", path + ".schema"),
        parseBinding(objectField(root, "binding", path + ".binding")),
        stringField(root, "operation", path + ".operation"),
        parseReceiptValue(objectField(root, "receipt", path + ".receipt")),
        stringField(root, "semantic_hash_hex", path + ".semantic_hash_hex"),
        stringField(root, "account_id", path + ".account_id"),
        stringField(root, "alias", path + ".alias"),
        parseDisposition(objectField(root, "disposition", path + ".disposition"), path + ".disposition"),
        stringField(root, "transaction_hash_hex", path + ".transaction_hash_hex"),
        stringField(root, "signed_transaction_wire_hex", path + ".signed_transaction_wire_hex"),
        stringField(root, "signed_transaction_wire_sha256", path + ".signed_transaction_wire_sha256"),
        FeePaymentJson.parse(root.get("fee_payment"), path + ".fee_payment"),
        stringField(root, "server_signature", path + ".server_signature"));
  }

  private static AccountFaucetPreparedTransactionV1 parseFaucetPrepared(
      final Map<String, Object> root) {
    final String path = "prepared faucet transaction";
    exactKeys(
        root,
        set(
            "schema", "binding", "operation", "claim", "semantic_hash_hex", "account_id",
            "asset_definition_id", "asset_id", "amount", "transaction_hash_hex",
            "signed_transaction_wire_hex", "signed_transaction_wire_sha256", "fee_payment",
            "server_signature"),
        path);
    final Map<String, Object> claim = objectField(root, "claim", path + ".claim");
    exactKeys(
        claim,
        set("account_id", "pow_anchor_height", "pow_nonce_hex"),
        path + ".claim");
    return new AccountFaucetPreparedTransactionV1(
        stringField(root, "schema", path + ".schema"),
        parseBinding(objectField(root, "binding", path + ".binding")),
        stringField(root, "operation", path + ".operation"),
        new AccountFaucetClaimV1(
            stringField(claim, "account_id", path + ".claim.account_id"),
            positiveU64(claim.get("pow_anchor_height"), path + ".claim.pow_anchor_height"),
            stringField(claim, "pow_nonce_hex", path + ".claim.pow_nonce_hex")),
        stringField(root, "semantic_hash_hex", path + ".semantic_hash_hex"),
        stringField(root, "account_id", path + ".account_id"),
        stringField(root, "asset_definition_id", path + ".asset_definition_id"),
        stringField(root, "asset_id", path + ".asset_id"),
        NumericV1.QuantityValue.parseCanonical(stringField(root, "amount", path + ".amount")),
        stringField(root, "transaction_hash_hex", path + ".transaction_hash_hex"),
        stringField(root, "signed_transaction_wire_hex", path + ".signed_transaction_wire_hex"),
        stringField(
            root, "signed_transaction_wire_sha256", path + ".signed_transaction_wire_sha256"),
        FeePaymentJson.parse(root.get("fee_payment"), path + ".fee_payment"),
        stringField(root, "server_signature", path + ".server_signature"));
  }

  private static AccountOnboardingProofRequiredPrepareResponseV1 parseProofRequired(
      final Map<String, Object> root) {
    final String path = "proof-required onboarding prepare response";
    exactKeys(
        root,
        set(
            "schema", "binding", "operation", "outcome", "proof_kind", "semantic_hash_hex", "account_id",
            "alias", "disposition", "server_signature"),
        path);
    return new AccountOnboardingProofRequiredPrepareResponseV1(
        stringField(root, "schema", path + ".schema"),
        parseBinding(objectField(root, "binding", path + ".binding")),
        stringField(root, "operation", path + ".operation"),
        stringField(root, "outcome", path + ".outcome"),
        stringField(root, "proof_kind", path + ".proof_kind"),
        stringField(root, "semantic_hash_hex", path + ".semantic_hash_hex"),
        stringField(root, "account_id", path + ".account_id"),
        stringField(root, "alias", path + ".alias"),
        parseDisposition(objectField(root, "disposition", path + ".disposition"), path + ".disposition"),
        stringField(root, "server_signature", path + ".server_signature"));
  }

  private static AccountOnboardingPlanReceiptV1 parseReceiptValue(
      final Map<String, Object> root) {
    exactKeys(root, set("body", "plan_hash", "signature"), "account onboarding receipt");
    return new AccountOnboardingPlanReceiptV1(
        parseBody(objectField(root, "body", "account onboarding receipt.body")),
        stringField(root, "plan_hash", "account onboarding receipt.plan_hash"),
        stringField(root, "signature", "account onboarding receipt.signature"));
  }

  private static TairaPublicResetMutationBindingV1 parseBinding(
      final Map<String, Object> root) {
    final String path = "public reset mutation binding";
    exactKeys(
        root,
        set(
            "schema", "authorization_sha256", "authorization_nonce", "kind", "phase",
            "idempotency_key", "execution_expires_at_unix_ms"),
        path);
    return new TairaPublicResetMutationBindingV1(
        stringField(root, "schema", path + ".schema"),
        stringField(root, "authorization_sha256", path + ".authorization_sha256"),
        stringField(root, "authorization_nonce", path + ".authorization_nonce"),
        stringField(root, "kind", path + ".kind"),
        stringField(root, "phase", path + ".phase"),
        stringField(root, "idempotency_key", path + ".idempotency_key"),
        longField(root, "execution_expires_at_unix_ms", path + ".execution_expires_at_unix_ms"));
  }

  private static AliasSetupModels.AliasPlanDispositionV1 parseDisposition(
      final Map<String, Object> root, final String path) {
    final String value = AliasTransactionPlanJsonParser.parseTaggedVariant(root, "kind", path);
    if ("no_op".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.NO_OP;
    if ("repair".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.REPAIR;
    if ("create".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.CREATE;
    if ("conflict".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.CONFLICT;
    throw new IllegalStateException(path + " is unsupported");
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

  private static BigInteger positiveU64(final Object value, final String path) {
    final BigInteger exact;
    if (value instanceof BigInteger) {
      exact = (BigInteger) value;
    } else if (value instanceof BigDecimal) {
      try {
        exact = ((BigDecimal) value).toBigIntegerExact();
      } catch (final ArithmeticException error) {
        throw new IllegalStateException(path + " must be an integer", error);
      }
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      exact = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw new IllegalStateException(path + " must be an exact integer");
    }
    if (exact.signum() <= 0 || exact.bitLength() > 64) {
      throw new IllegalStateException(path + " must be a positive unsigned 64-bit integer");
    }
    return exact;
  }
}
