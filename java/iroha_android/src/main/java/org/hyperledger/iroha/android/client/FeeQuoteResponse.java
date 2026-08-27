package org.hyperledger.iroha.android.client;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.TreeMap;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.model.FeeChargeKind;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.numeric.NumericV1;

/** Successful deterministic fee quote preserving the exact payer and gas bound. */
public final class FeeQuoteResponse {
  private static final BigInteger U64_MAX = new BigInteger("18446744073709551615");

  private final FeePaymentIntent intent;
  private final Map<String, Object> observation;
  private final List<Map<String, Object>> components;
  private final List<Map<String, Object>> capacities;
  private final Map<String, Object> decision;
  private final ParsedObservation parsedObservation;
  private final List<ParsedComponent> parsedComponents;
  private final List<ParsedCapacity> parsedCapacities;
  private final ParsedDecision parsedDecision;

  FeeQuoteResponse(
      final FeePaymentIntent intent,
      final Map<String, Object> observation,
      final List<Map<String, Object>> components,
      final List<Map<String, Object>> capacities,
      final Map<String, Object> decision) {
    this.intent = Objects.requireNonNull(intent, "intent");
    this.observation = snapshot(observation);
    this.components = snapshotList(components);
    this.capacities = snapshotList(capacities);
    this.decision = snapshot(decision);
    this.parsedObservation = parseObservation(this.observation);
    this.parsedComponents = new ArrayList<>();
    for (int index = 0; index < this.components.size(); index++) {
      this.parsedComponents.add(parseComponent(index, this.components.get(index)));
    }
    this.parsedCapacities = new ArrayList<>();
    for (int index = 0; index < this.capacities.size(); index++) {
      this.parsedCapacities.add(parseCapacity(index, this.capacities.get(index)));
    }
    this.parsedDecision = parseDecision(this.decision);
  }

  public FeePaymentIntent intent() {
    return intent;
  }

  public Map<String, Object> observation() {
    return observation;
  }

  public List<Map<String, Object>> components() {
    return components;
  }

  public List<Map<String, Object>> capacities() {
    return capacities;
  }

  public Map<String, Object> decision() {
    return decision;
  }

  /** Validate this quote against the unsigned transaction payload used to request it. */
  public void validateForDraft(final TransactionPayload payload) {
    Objects.requireNonNull(payload, "payload");
    validateForDraft(payload.feePayment(), payload.authority());
  }

  /** Validate this quote against a payload containing the exact quoted fee intent. */
  public void validateForSignedPayload(final TransactionPayload payload) {
    Objects.requireNonNull(payload, "payload");
    if (!sameExactFeeQuoteIntent(intent, payload.feePayment())) {
      throw new IllegalArgumentException("fee quote intent differs from the signed payload");
    }
    validateSemantics(payload.authority());
  }

  void validateForDraft(final FeePaymentIntent draftIntent, final String authority) {
    Objects.requireNonNull(draftIntent, "draftIntent");
    if (!sameFeeQuotePayerAndGasBound(intent, draftIntent)) {
      throw new IllegalArgumentException(
          "fee quote changed the draft payer, sponsor revision, or gas bound");
    }
    validateSemantics(authority);
  }

  private void validateSemantics(final String authority) {
    if (parsedObservation.nextBlockHeight.signum() == 0) {
      throw new IllegalArgumentException("fee quote next_block_height must be non-zero");
    }
    final List<FeeChargeLimit> limits = intent.chargeLimits();
    if (parsedComponents.size() != limits.size()) {
      throw new IllegalArgumentException("fee quote components differ from the quoted intent");
    }
    for (int index = 0; index < parsedComponents.size(); index++) {
      final ParsedComponent component = parsedComponents.get(index);
      final FeeChargeLimit limit = limits.get(index);
      if (component.kind != limit.kind()
          || !component.assetDefinitionId.equals(limit.assetDefinitionId())
          || !component.maxAmount.toString().equals(limit.maxAmount())) {
        throw new IllegalArgumentException("fee quote components differ from the quoted intent");
      }
    }

    if (intent instanceof FeePaymentIntent.Authority) {
      if (!(parsedDecision.debitSource instanceof AccountDebitSource)
          || !sameFeeQuoteAccountIdentity(
              ((AccountDebitSource) parsedDecision.debitSource).accountId, authority)
          || parsedDecision.programRevision != null) {
        throw new IllegalArgumentException(
            "authority-paid fee quote has an inconsistent admission decision");
      }
      if (!parsedCapacities.isEmpty()) {
        throw new IllegalArgumentException(
            "authority-paid fee quote must not contain capacities");
      }
      return;
    }

    final FeePaymentIntent.Sponsor payment = (FeePaymentIntent.Sponsor) intent;
    if (!(parsedDecision.debitSource instanceof SponsorProgramDebitSource)
        || !sameFeeQuoteProgramIdentity(
            ((SponsorProgramDebitSource) parsedDecision.debitSource).programId,
            payment.programId())
        || !BigInteger.valueOf(payment.programRevision()).equals(parsedDecision.programRevision)) {
      throw new IllegalArgumentException(
          "sponsored fee quote has an inconsistent admission decision");
    }
    validateSponsorCapacities();
  }

  static boolean sameFeeQuoteAccountIdentity(final String left, final String right) {
    try {
      final byte[] leftBytes =
          AccountAddress.parseEncodedIgnoringCurveSupport(left, null).canonicalBytes();
      final byte[] rightBytes =
          AccountAddress.parseEncodedIgnoringCurveSupport(right, null).canonicalBytes();
      return Arrays.equals(leftBytes, rightBytes);
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException(
          "fee quote account identity must use canonical I105", error);
    }
  }

  private static boolean sameFeeQuoteProgramIdentity(
      final FeeSponsorProgramId left, final FeeSponsorProgramId right) {
    return left.name().equals(right.name())
        && sameFeeQuoteAccountIdentity(left.sponsor(), right.sponsor());
  }

  private static boolean sameFeeQuotePayerAndGasBound(
      final FeePaymentIntent left, final FeePaymentIntent right) {
    if (!Objects.equals(left.gasLimit(), right.gasLimit())) {
      return false;
    }
    if (left instanceof FeePaymentIntent.Authority
        && right instanceof FeePaymentIntent.Authority) {
      return true;
    }
    if (left instanceof FeePaymentIntent.Sponsor
        && right instanceof FeePaymentIntent.Sponsor) {
      final FeePaymentIntent.Sponsor leftSponsor = (FeePaymentIntent.Sponsor) left;
      final FeePaymentIntent.Sponsor rightSponsor = (FeePaymentIntent.Sponsor) right;
      return leftSponsor.programRevision() == rightSponsor.programRevision()
          && sameFeeQuoteProgramIdentity(leftSponsor.programId(), rightSponsor.programId());
    }
    return false;
  }

  private static boolean sameExactFeeQuoteIntent(
      final FeePaymentIntent left, final FeePaymentIntent right) {
    return sameFeeQuotePayerAndGasBound(left, right)
        && left.chargeLimits().equals(right.chargeLimits());
  }

  private void validateSponsorCapacities() {
    if (parsedCapacities.isEmpty() != parsedComponents.isEmpty()) {
      throw new IllegalArgumentException(
          "sponsored fee quote capacities must be empty exactly when components are empty");
    }
    final TreeMap<String, NumericV1.QuantityValue> aggregateByAsset =
        new TreeMap<>(FeeQuoteResponse::compareAssetIds);
    for (final ParsedComponent component : parsedComponents) {
      final NumericV1.QuantityValue previous = aggregateByAsset.get(component.assetDefinitionId);
      aggregateByAsset.put(
          component.assetDefinitionId,
          previous == null
              ? component.maxAmount
              : addQuantities(
                  previous,
                  component.maxAmount,
                  "fee quote component aggregate for "
                      + component.assetDefinitionId
                      + " is invalid"));
    }
    if (parsedCapacities.size() != aggregateByAsset.size()) {
      throw new IllegalArgumentException(
          "sponsored fee quote must contain one capacity per component asset");
    }
    int index = 0;
    for (final Map.Entry<String, NumericV1.QuantityValue> entry : aggregateByAsset.entrySet()) {
      final ParsedCapacity capacity = parsedCapacities.get(index++);
      if (!capacity.assetDefinitionId.equals(entry.getKey())) {
        throw new IllegalArgumentException(
            "sponsored fee quote capacities are duplicated, unrelated, or not in canonical asset order");
      }
      final NumericV1.QuantityValue requiredVaultBalance =
          addQuantities(
              capacity.reserveFloor,
              entry.getValue(),
              "fee quote required vault balance for " + entry.getKey() + " is invalid");
      if (compareQuantities(capacity.vaultBalance, requiredVaultBalance) < 0) {
        throw new IllegalArgumentException(
            "fee quote vault capacity for "
                + entry.getKey()
                + " does not cover its reserve and aggregate charge");
      }
      requireCapacity(capacity.blockRemaining, entry, "block");
      requireCapacity(capacity.programEpochRemaining, entry, "program epoch");
      requireCapacity(capacity.beneficiaryEpochRemaining, entry, "beneficiary epoch");
    }
  }

  private static void requireCapacity(
      final NumericV1.QuantityValue remaining,
      final Map.Entry<String, NumericV1.QuantityValue> required,
      final String window) {
    if (compareQuantities(remaining, required.getValue()) < 0) {
      throw new IllegalArgumentException(
          "fee quote "
              + window
              + " capacity for "
              + required.getKey()
              + " does not cover its aggregate charge");
    }
  }

  private static ParsedObservation parseObservation(final Map<String, Object> value) {
    final String path = "fee quote response.observation";
    requireExactKeys(
        value, keys("ledger_time_ms", "next_block_height", "route_dataspace_id"), path);
    return new ParsedObservation(
        unsignedInteger(value.get("ledger_time_ms"), path + ".ledger_time_ms"),
        unsignedInteger(value.get("next_block_height"), path + ".next_block_height"),
        unsignedInteger(value.get("route_dataspace_id"), path + ".route_dataspace_id"));
  }

  private static ParsedComponent parseComponent(
      final int index, final Map<String, Object> value) {
    final String path = "fee quote response.components[" + index + "]";
    requireExactKeys(value, keys("kind", "asset_definition_id", "max_amount"), path);
    final Map<String, Object> kindValue = objectValue(value.get("kind"), path + ".kind");
    requireExactKeys(kindValue, keys("kind", "value"), path + ".kind");
    if (kindValue.get("value") != null) {
      throw new IllegalArgumentException(path + ".kind.value must be null");
    }
    final FeeChargeKind kind;
    if ("nexus".equals(kindValue.get("kind"))) {
      kind = FeeChargeKind.NEXUS;
    } else if ("pipeline_gas".equals(kindValue.get("kind"))) {
      kind = FeeChargeKind.PIPELINE_GAS;
    } else {
      throw new IllegalArgumentException(path + ".kind.kind must be nexus or pipeline_gas");
    }
    return new ParsedComponent(
        kind,
        canonicalAssetId(value.get("asset_definition_id"), path + ".asset_definition_id"),
        quantity(value.get("max_amount"), path + ".max_amount"));
  }

  private static ParsedCapacity parseCapacity(
      final int index, final Map<String, Object> value) {
    final String path = "fee quote response.capacities[" + index + "]";
    requireExactKeys(
        value,
        keys(
            "asset_definition_id",
            "vault_balance",
            "reserve_floor",
            "block_remaining",
            "program_epoch_remaining",
            "beneficiary_epoch_remaining"),
        path);
    return new ParsedCapacity(
        canonicalAssetId(value.get("asset_definition_id"), path + ".asset_definition_id"),
        quantity(value.get("vault_balance"), path + ".vault_balance"),
        quantity(value.get("reserve_floor"), path + ".reserve_floor"),
        quantity(value.get("block_remaining"), path + ".block_remaining"),
        quantity(value.get("program_epoch_remaining"), path + ".program_epoch_remaining"),
        quantity(value.get("beneficiary_epoch_remaining"), path + ".beneficiary_epoch_remaining"));
  }

  private static ParsedDecision parseDecision(final Map<String, Object> value) {
    final String path = "fee quote response.decision";
    requireExactKeys(value, keys("status", "value"), path);
    if (!"accepted".equals(value.get("status"))) {
      throw new IllegalArgumentException(path + ".status must be accepted");
    }
    final Map<String, Object> accepted = objectValue(value.get("value"), path + ".value");
    requireExactKeys(accepted, keys("debit_source", "program_revision"), path + ".value");
    final Map<String, Object> debitSource =
        objectValue(accepted.get("debit_source"), path + ".value.debit_source");
    requireExactKeys(debitSource, keys("kind", "value"), path + ".value.debit_source");
    if ("account".equals(debitSource.get("kind"))) {
      if (!(debitSource.get("value") instanceof String)) {
        throw new IllegalArgumentException(
            path + ".value.debit_source.value must be a string");
      }
      final String accountId = (String) debitSource.get("value");
      AccountIdLiteral.requireCanonicalI105Address(
          accountId, path + ".value.debit_source.value");
      if (accepted.get("program_revision") != null) {
        throw new IllegalArgumentException(
            path + ".value.program_revision must be null for an account debit");
      }
      return new ParsedDecision(new AccountDebitSource(accountId), null);
    }
    if (!"sponsor_program".equals(debitSource.get("kind"))) {
      throw new IllegalArgumentException(
          path + ".value.debit_source.kind must be account or sponsor_program");
    }
    final Map<String, Object> program =
        objectValue(debitSource.get("value"), path + ".value.debit_source.value");
    requireExactKeys(program, keys("sponsor", "name"), path + ".value.debit_source.value");
    if (!(program.get("sponsor") instanceof String)
        || !(program.get("name") instanceof String)) {
      throw new IllegalArgumentException(
          path + ".value.debit_source.value must contain string sponsor and name fields");
    }
    return new ParsedDecision(
        new SponsorProgramDebitSource(
            new FeeSponsorProgramId(
                (String) program.get("sponsor"), (String) program.get("name"))),
        positiveInteger(accepted.get("program_revision"), path + ".value.program_revision"));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(path + " must be an object");
    }
    final Map<?, ?> map = (Map<?, ?>) value;
    for (final Object key : map.keySet()) {
      if (!(key instanceof String)) {
        throw new IllegalArgumentException(path + " keys must be strings");
      }
    }
    return (Map<String, Object>) map;
  }

  private static void requireExactKeys(
      final Map<String, Object> value, final Set<String> expected, final String path) {
    final Set<String> unknown = new HashSet<>(value.keySet());
    unknown.removeAll(expected);
    if (!unknown.isEmpty()) {
      throw new IllegalArgumentException(path + " contains unknown fields: " + unknown);
    }
    final Set<String> missing = new HashSet<>(expected);
    missing.removeAll(value.keySet());
    if (!missing.isEmpty()) {
      throw new IllegalArgumentException(path + " is missing required fields: " + missing);
    }
  }

  private static String canonicalAssetId(final Object value, final String path) {
    if (!(value instanceof String)
        || !AssetDefinitionIdEncoder.isCanonicalAddress((String) value)) {
      throw new IllegalArgumentException(path + " must be a canonical asset definition id");
    }
    return (String) value;
  }

  private static NumericV1.QuantityValue quantity(final Object value, final String path) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(path + " must be a string");
    }
    try {
      return NumericV1.QuantityValue.parseCanonical((String) value);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(path + " must be a canonical quantity", error);
    }
  }

  private static BigInteger unsignedInteger(final Object value, final String path) {
    final BigInteger integer = exactInteger(value, path);
    if (integer.signum() < 0 || integer.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(path + " must fit in u64");
    }
    return integer;
  }

  private static BigInteger positiveInteger(final Object value, final String path) {
    final BigInteger integer = unsignedInteger(value, path);
    if (integer.signum() == 0) {
      throw new IllegalArgumentException(path + " must be positive");
    }
    return integer;
  }

  private static BigInteger exactInteger(final Object value, final String path) {
    try {
      if (value instanceof BigInteger) {
        return (BigInteger) value;
      }
      if (value instanceof BigDecimal) {
        return ((BigDecimal) value).toBigIntegerExact();
      }
      if (value instanceof Byte
          || value instanceof Short
          || value instanceof Integer
          || value instanceof Long) {
        return BigInteger.valueOf(((Number) value).longValue());
      }
    } catch (final ArithmeticException error) {
      throw new IllegalArgumentException(path + " must be an integer", error);
    }
    throw new IllegalArgumentException(path + " must be an integer");
  }

  private static NumericV1.QuantityValue addQuantities(
      final NumericV1.QuantityValue left,
      final NumericV1.QuantityValue right,
      final String path) {
    final int scale = Math.max(left.scale(), right.scale());
    final BigInteger sum =
        left.mantissa()
            .multiply(BigInteger.TEN.pow(scale - left.scale()))
            .add(right.mantissa().multiply(BigInteger.TEN.pow(scale - right.scale())));
    try {
      return NumericV1.QuantityValue.of(sum, scale);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(path, error);
    }
  }

  private static int compareQuantities(
      final NumericV1.QuantityValue left, final NumericV1.QuantityValue right) {
    final int scale = Math.max(left.scale(), right.scale());
    final BigInteger leftMantissa =
        left.mantissa().multiply(BigInteger.TEN.pow(scale - left.scale()));
    final BigInteger rightMantissa =
        right.mantissa().multiply(BigInteger.TEN.pow(scale - right.scale()));
    return leftMantissa.compareTo(rightMantissa);
  }

  private static int compareAssetIds(final String left, final String right) {
    final byte[] leftBytes = AssetDefinitionIdEncoder.parseAddressBytes(left);
    final byte[] rightBytes = AssetDefinitionIdEncoder.parseAddressBytes(right);
    for (int index = 0; index < leftBytes.length; index++) {
      final int comparison = Integer.compare(leftBytes[index] & 0xff, rightBytes[index] & 0xff);
      if (comparison != 0) {
        return comparison;
      }
    }
    return 0;
  }

  private static Map<String, Object> snapshot(final Map<String, Object> value) {
    return Collections.unmodifiableMap(new LinkedHashMap<>(value));
  }

  private static List<Map<String, Object>> snapshotList(final List<Map<String, Object>> values) {
    final List<Map<String, Object>> out = new ArrayList<>();
    for (final Map<String, Object> value : values) {
      out.add(snapshot(value));
    }
    return Collections.unmodifiableList(out);
  }

  private static Set<String> keys(final String... values) {
    return new HashSet<>(Arrays.asList(values));
  }

  private static final class ParsedObservation {
    final BigInteger ledgerTimeMs;
    final BigInteger nextBlockHeight;
    final BigInteger routeDataspaceId;

    ParsedObservation(
        final BigInteger ledgerTimeMs,
        final BigInteger nextBlockHeight,
        final BigInteger routeDataspaceId) {
      this.ledgerTimeMs = ledgerTimeMs;
      this.nextBlockHeight = nextBlockHeight;
      this.routeDataspaceId = routeDataspaceId;
    }
  }

  private static final class ParsedComponent {
    final FeeChargeKind kind;
    final String assetDefinitionId;
    final NumericV1.QuantityValue maxAmount;

    ParsedComponent(
        final FeeChargeKind kind,
        final String assetDefinitionId,
        final NumericV1.QuantityValue maxAmount) {
      this.kind = kind;
      this.assetDefinitionId = assetDefinitionId;
      this.maxAmount = maxAmount;
    }
  }

  private static final class ParsedCapacity {
    final String assetDefinitionId;
    final NumericV1.QuantityValue vaultBalance;
    final NumericV1.QuantityValue reserveFloor;
    final NumericV1.QuantityValue blockRemaining;
    final NumericV1.QuantityValue programEpochRemaining;
    final NumericV1.QuantityValue beneficiaryEpochRemaining;

    ParsedCapacity(
        final String assetDefinitionId,
        final NumericV1.QuantityValue vaultBalance,
        final NumericV1.QuantityValue reserveFloor,
        final NumericV1.QuantityValue blockRemaining,
        final NumericV1.QuantityValue programEpochRemaining,
        final NumericV1.QuantityValue beneficiaryEpochRemaining) {
      this.assetDefinitionId = assetDefinitionId;
      this.vaultBalance = vaultBalance;
      this.reserveFloor = reserveFloor;
      this.blockRemaining = blockRemaining;
      this.programEpochRemaining = programEpochRemaining;
      this.beneficiaryEpochRemaining = beneficiaryEpochRemaining;
    }
  }

  private abstract static class ParsedDebitSource {}

  private static final class AccountDebitSource extends ParsedDebitSource {
    final String accountId;

    AccountDebitSource(final String accountId) {
      this.accountId = accountId;
    }
  }

  private static final class SponsorProgramDebitSource extends ParsedDebitSource {
    final FeeSponsorProgramId programId;

    SponsorProgramDebitSource(final FeeSponsorProgramId programId) {
      this.programId = programId;
    }
  }

  private static final class ParsedDecision {
    final ParsedDebitSource debitSource;
    final BigInteger programRevision;

    ParsedDecision(final ParsedDebitSource debitSource, final BigInteger programRevision) {
      this.debitSource = debitSource;
      this.programRevision = programRevision;
    }
  }
}
