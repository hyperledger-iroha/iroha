package org.hyperledger.iroha.android.alias;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/** Verification helpers used before locally signing an alias transaction plan. */
public final class AliasPlanVerifier {
  private static final byte[] HASH_DOMAIN =
      "iroha:alias-transaction-plan-body:v1\0".getBytes(StandardCharsets.UTF_8);
  private static final byte[] LIFECYCLE_HASH_DOMAIN =
      "iroha:alias-lifecycle-transaction-plan-body:v1\0".getBytes(StandardCharsets.UTF_8);

  private AliasPlanVerifier() {}

  /** Computes the canonical plan hash from exact Norito plan-body bytes. */
  public static byte[] canonicalHash(final byte[] canonicalBodyNorito) {
    return canonicalHashWithDomain(HASH_DOMAIN, canonicalBodyNorito);
  }

  /** Computes a lifecycle-plan commitment from exact Norito plan-body bytes. */
  public static byte[] canonicalLifecycleHash(final byte[] canonicalBodyNorito) {
    return canonicalHashWithDomain(LIFECYCLE_HASH_DOMAIN, canonicalBodyNorito);
  }

  private static byte[] canonicalHashWithDomain(
      final byte[] domain, final byte[] canonicalBodyNorito) {
    if (canonicalBodyNorito == null) {
      throw new IllegalArgumentException("canonicalBodyNorito must not be null");
    }
    final byte[] preimage = new byte[domain.length + canonicalBodyNorito.length];
    System.arraycopy(domain, 0, preimage, 0, domain.length);
    System.arraycopy(
        canonicalBodyNorito, 0, preimage, domain.length, canonicalBodyNorito.length);
    return IrohaHash.prehash(preimage);
  }

  /** Verifies hash text against exact Norito plan-body bytes. */
  public static boolean verifyHash(final String planHash, final byte[] canonicalBodyNorito) {
    final byte[] expected = decodeHash(planHash);
    return expected != null && MessageDigest.isEqual(expected, canonicalHash(canonicalBodyNorito));
  }

  /** Verifies the hash carried by a typed plan. */
  public static boolean verifyHash(
      final AliasTransactionPlanV1 plan, final byte[] canonicalBodyNorito) {
    return verifyHash(plan.planHash(), canonicalBodyNorito);
  }

  /** Verifies the hash carried by a lifecycle plan. */
  public static boolean verifyLifecycleHash(
      final AliasLifecycleTransactionPlanV1 plan, final byte[] canonicalBodyNorito) {
    final byte[] expected = decodeHash(plan.planHash());
    return expected != null
        && MessageDigest.isEqual(expected, canonicalLifecycleHash(canonicalBodyNorito));
  }

  /** Returns stable validation codes for a plan that is not safe to submit. */
  public static List<String> validateExecutable(final AliasTransactionPlanV1 plan) {
    if (plan == null) throw new IllegalArgumentException("plan must not be null");
    final AliasSetupModels.AliasTransactionPlanBodyV1 body = plan.body();
    final Set<String> errors = new LinkedHashSet<>();
    if (body.version() != AliasSetupModels.AliasTransactionPlanBodyV1.VERSION) {
      errors.add("alias.plan.version_unsupported");
    }
    if (!body.blockers().isEmpty()) errors.add("alias.plan.blocked");
    if (body.resources().isEmpty()) errors.add("alias.plan.resources_empty");
    if (body.instructions().size() != body.resources().size()) {
      errors.add("alias.plan.instruction_count_mismatch");
    }
    if (decodeHash(plan.planHash()) == null) errors.add("alias.plan.hash_invalid");
    if (!dependencyOrdered(body.resources())) errors.add("alias.plan.resource_order_invalid");

    final List<String> totalKeys = new ArrayList<>();
    for (final AliasSetupModels.AliasAssetTotalV1 total : body.totalsByAsset()) {
      totalKeys.add(assetSortKey(total.paymentAsset()) + "\0" + total.amount());
    }
    if (!sorted(totalKeys)) errors.add("alias.plan.totals_not_canonical");
    final List<String> warningKeys = new ArrayList<>();
    for (final AliasSetupModels.AliasSetupDiagnosticV1 warning : body.warnings()) {
      warningKeys.add(warning.sortKey());
    }
    final List<String> blockerKeys = new ArrayList<>();
    for (final AliasSetupModels.AliasSetupDiagnosticV1 blocker : body.blockers()) {
      blockerKeys.add(blocker.sortKey());
    }
    if (!sorted(warningKeys) || !sorted(blockerKeys)) {
      errors.add("alias.plan.diagnostics_not_canonical");
    }
    for (final AliasSetupModels.AliasFramedInstructionV1 instruction : body.instructions()) {
      if (!EnsureAlias.WIRE_ID.equals(instruction.wireId())
          || instruction.framedPayload().length == 0) {
        errors.add("alias.plan.instruction_invalid");
      }
    }

    final Set<Long> claimedIndices = new HashSet<>();
    Long previousInstructionIndex = null;
    for (final AliasSetupModels.AliasPlanResourceV1 resource : body.resources()) {
      final Long index = resource.instructionIndex();
      if (index != null) {
        if (index.longValue() >= body.instructions().size()) {
          errors.add("alias.plan.instruction_index_invalid");
        } else {
          if (!claimedIndices.add(index)) errors.add("alias.plan.instruction_index_duplicate");
          if (previousInstructionIndex != null && index.longValue() <= previousInstructionIndex) {
            errors.add("alias.plan.instruction_indexes_not_ordered");
          }
          previousInstructionIndex = index;
          if (!EnsureAlias.WIRE_ID.equals(body.instructions().get(index.intValue()).wireId())) {
            errors.add("alias.plan.instruction_wire_id_invalid");
          }
        }
      }
      switch (resource.disposition()) {
        case NO_OP -> {
          if (resource.quote() != null || index == null) {
            errors.add("alias.plan.no_op_shape_invalid");
          }
        }
        case REPAIR -> {
          if (resource.quote() != null || index == null) {
            errors.add("alias.plan.repair_shape_invalid");
          }
        }
        case CREATE -> {
          if (resource.quote() == null || index == null) {
            errors.add("alias.plan.create_shape_invalid");
          }
        }
        case CONFLICT -> {
          errors.add("alias.plan.conflict");
          if (resource.quote() != null || index != null) {
            errors.add("alias.plan.conflict_not_empty");
          }
        }
      }
      final AliasSetupModels.AliasLeaseQuoteV1 quote = resource.quote();
      if (quote != null) {
        if (!quote.target().equals(AliasSetupModels.targetFor(resource.intent()))) {
          errors.add("alias.plan.quote_target_mismatch");
        }
        if (!AliasSetupModels.amountWithinCap(quote.exactAmount(), quote.guard().maxAmount())) {
          errors.add("alias.plan.quote_cap_invalid");
        }
        if (quote.expiresAtMs() > quote.graceExpiresAtMs()
            || quote.graceExpiresAtMs() > quote.redemptionExpiresAtMs()) {
          errors.add("alias.plan.quote_expiry_order_invalid");
        }
      }
    }
    if (claimedIndices.size() != body.instructions().size()) {
      errors.add("alias.plan.instruction_unreferenced");
    }
    final List<String> result = new ArrayList<>(errors);
    Collections.sort(result);
    return Collections.unmodifiableList(result);
  }

  /** Decodes and re-encodes every exact frame, rejecting byte-level changes. */
  public static boolean verifyExactFrames(
      final AliasTransactionPlanV1 plan,
      final AliasInstructionFrameRoundTripper roundTripper) {
    if (roundTripper == null) throw new IllegalArgumentException("roundTripper must not be null");
    for (final AliasSetupModels.AliasFramedInstructionV1 instruction :
        plan.body().instructions()) {
      final byte[] original = instruction.framedPayload();
      final byte[] encoded;
      try {
        encoded = roundTripper.decodeAndReencode(instruction.wireId(), original.clone());
      } catch (final RuntimeException exception) {
        return false;
      }
      if (encoded == null || !MessageDigest.isEqual(original, encoded)) return false;
    }
    return true;
  }

  /** Requires canonical shape, the exact plan hash, and exact instruction round trips. */
  public static void requireExecutable(
      final AliasTransactionPlanV1 plan,
      final byte[] canonicalBodyNorito,
      final AliasInstructionFrameRoundTripper roundTripper) {
    final List<String> errors = new ArrayList<>(validateExecutable(plan));
    if (!verifyHash(plan, canonicalBodyNorito)) errors.add("alias.plan.hash_mismatch");
    if (!verifyExactFrames(plan, roundTripper)) {
      errors.add("alias.plan.instruction_roundtrip_mismatch");
    }
    if (!errors.isEmpty()) {
      Collections.sort(errors);
      throw new IllegalArgumentException(String.join(",", new LinkedHashSet<>(errors)));
    }
  }

  /** Requires a plan to be the complete canonical rendering of the signed request. */
  public static void requireExecutableForRequest(
      final AliasSetupPlanRequestV1 request,
      final AliasTransactionPlanV1 plan,
      final byte[] canonicalBodyNorito,
      final AliasEnsureInstructionFrameCodec frameCodec,
      final int chainDiscriminant) {
    if (request == null || frameCodec == null) {
      throw new IllegalArgumentException("request and frameCodec must not be null");
    }
    final List<EnsureAlias> decoded = new ArrayList<>();
    requireExecutable(
        plan,
        canonicalBodyNorito,
        (wireId, framedPayload) -> {
          final DecodedEnsureAliasFrame result =
              frameCodec.decodeAndReencode(wireId, framedPayload, chainDiscriminant);
          if (result == null) throw new IllegalArgumentException("frame codec returned null");
          decoded.add(result.instruction());
          return result.reencodedFrame();
        });

    final List<EnsureAlias> expected = new ArrayList<>(request.intents());
    expected.sort(
        java.util.Comparator.comparingInt(
                (EnsureAlias value) -> value.intent().dependencyRank())
            .thenComparing(value -> value.intent().resourceText()));
    final Set<String> errors = new LinkedHashSet<>();
    if (!decoded.equals(expected)) errors.add("alias.plan.signed_request_mismatch");
    final List<AliasSetupModels.AliasIntentV1> plannedIntents = new ArrayList<>();
    for (final AliasSetupModels.AliasPlanResourceV1 resource : plan.body().resources()) {
      plannedIntents.add(resource.intent());
    }
    final List<AliasSetupModels.AliasIntentV1> expectedIntents = new ArrayList<>();
    for (final EnsureAlias value : expected) expectedIntents.add(value.intent());
    if (!plannedIntents.equals(expectedIntents)) {
      errors.add("alias.plan.resource_request_mismatch");
    }
    if (!errors.isEmpty()) {
      final List<String> sorted = new ArrayList<>(errors);
      Collections.sort(sorted);
      throw new IllegalArgumentException(String.join(",", sorted));
    }
  }

  /** Returns stable validation codes when a lifecycle plan is unsafe to submit. */
  public static List<String> validateLifecycleExecutable(
      final AliasLifecycleTransactionPlanV1 plan) {
    if (plan == null) throw new IllegalArgumentException("plan must not be null");
    final AliasLifecycleTransactionPlanBodyV1 body = plan.body();
    final Set<String> errors = new LinkedHashSet<>();
    if (body.version() != AliasLifecycleTransactionPlanBodyV1.VERSION) {
      errors.add("alias.lifecycle.plan.version_unsupported");
    }
    if (!body.blockers().isEmpty()) errors.add("alias.lifecycle.plan.blocked");
    if (decodeHash(plan.planHash()) == null) errors.add("alias.lifecycle.plan.hash_invalid");

    final List<String> totalKeys = new ArrayList<>();
    for (final AliasSetupModels.AliasAssetTotalV1 total : body.totalsByAsset()) {
      totalKeys.add(assetSortKey(total.paymentAsset()) + "\0" + total.amount());
    }
    if (!sorted(totalKeys)) errors.add("alias.lifecycle.plan.totals_not_canonical");
    final List<String> warningKeys = new ArrayList<>();
    for (final AliasSetupModels.AliasSetupDiagnosticV1 warning : body.warnings()) {
      warningKeys.add(warning.sortKey());
    }
    final List<String> blockerKeys = new ArrayList<>();
    for (final AliasSetupModels.AliasSetupDiagnosticV1 blocker : body.blockers()) {
      blockerKeys.add(blocker.sortKey());
    }
    if (!sorted(warningKeys) || !sorted(blockerKeys)) {
      errors.add("alias.lifecycle.plan.diagnostics_not_canonical");
    }

    final String expectedWireId =
        body.operation() instanceof AliasLifecycleOperationV1.RenewLease
            ? RenewAliasLease.WIRE_ID
            : ConfigureAliasAutoRenew.WIRE_ID;
    if (body.disposition() == AliasLifecyclePlanDispositionV1.NO_OP) {
      if (!(body.operation() instanceof AliasLifecycleOperationV1.ConfigureAutoRenew)
          || body.instruction() != null
          || body.quote() != null
          || !body.totalsByAsset().isEmpty()) {
        errors.add("alias.lifecycle.plan.no_op_shape_invalid");
      }
    } else {
      final AliasSetupModels.AliasFramedInstructionV1 instruction = body.instruction();
      if (instruction == null
          || !expectedWireId.equals(instruction.wireId())
          || instruction.framedPayload().length == 0) {
        errors.add("alias.lifecycle.plan.instruction_invalid");
      }
      if (body.operation() instanceof AliasLifecycleOperationV1.RenewLease) {
        final AliasLifecycleOperationV1.RenewLease operation =
            (AliasLifecycleOperationV1.RenewLease) body.operation();
        final AliasSetupModels.AliasLeaseQuoteV1 quote = body.quote();
        if (quote == null || body.totalsByAsset().size() != 1) {
          errors.add("alias.lifecycle.plan.renewal_quote_invalid");
        } else {
          if (!quote.target().equals(operation.target())
              || !quote.guard().equals(operation.renewal().quoteGuard())
              || quote.expiresAtMs() != operation.renewal().targetExpiryMs()
              || body.validUntilMs() != operation.renewal().quoteGuard().validUntilMs()
              || !AliasSetupModels.amountWithinCap(
                  quote.exactAmount(), quote.guard().maxAmount())) {
            errors.add("alias.lifecycle.plan.renewal_quote_mismatch");
          }
          final AliasSetupModels.AliasAssetTotalV1 total = body.totalsByAsset().get(0);
          if (!total.paymentAsset().equals(quote.guard().expectedPaymentAsset())
              || !total.amount().equals(quote.exactAmount())) {
            errors.add("alias.lifecycle.plan.renewal_total_mismatch");
          }
          if (quote.expiresAtMs() > quote.graceExpiresAtMs()
              || quote.graceExpiresAtMs() > quote.redemptionExpiresAtMs()) {
            errors.add("alias.lifecycle.plan.quote_expiry_order_invalid");
          }
        }
      } else if (body.quote() != null || !body.totalsByAsset().isEmpty()) {
        errors.add("alias.lifecycle.plan.auto_renew_charge_invalid");
      }
    }

    final List<String> result = new ArrayList<>(errors);
    Collections.sort(result);
    return Collections.unmodifiableList(result);
  }

  /** Requires a lifecycle plan to preserve the signed request, hash, and exact typed frame. */
  public static void requireLifecycleExecutableForRequest(
      final AliasLifecyclePlanRequestV1 request,
      final AliasLifecycleTransactionPlanV1 plan,
      final byte[] canonicalBodyNorito,
      final AliasLifecycleInstructionFrameCodec frameCodec,
      final int chainDiscriminant) {
    if (request == null || frameCodec == null) {
      throw new IllegalArgumentException("request and frameCodec must not be null");
    }
    final List<String> errors = new ArrayList<>(validateLifecycleExecutable(plan));
    if (!verifyLifecycleHash(plan, canonicalBodyNorito)) {
      errors.add("alias.lifecycle.plan.hash_mismatch");
    }
    if (!plan.body().operation().equals(request.operation())) {
      errors.add("alias.lifecycle.plan.signed_request_mismatch");
    }
    final AliasSetupModels.AliasFramedInstructionV1 instruction = plan.body().instruction();
    if (instruction != null) {
      DecodedAliasLifecycleFrame decoded = null;
      try {
        decoded =
            frameCodec.decodeAndReencode(
                instruction.wireId(), instruction.framedPayload(), chainDiscriminant);
      } catch (final RuntimeException ignored) {
        // Converted into the stable validation error below.
      }
      if (decoded == null
          || !decoded.operation().equals(plan.body().operation())
          || !MessageDigest.isEqual(instruction.framedPayload(), decoded.reencodedFrame())) {
        errors.add("alias.lifecycle.plan.instruction_roundtrip_mismatch");
      }
    }
    if (!errors.isEmpty()) {
      Collections.sort(errors);
      throw new IllegalArgumentException(String.join(",", new LinkedHashSet<>(errors)));
    }
  }

  private static boolean dependencyOrdered(
      final List<AliasSetupModels.AliasPlanResourceV1> resources) {
    int previous = -1;
    for (final AliasSetupModels.AliasPlanResourceV1 resource : resources) {
      if (resource.intent().dependencyRank() < previous) return false;
      previous = resource.intent().dependencyRank();
    }
    return true;
  }

  private static boolean sorted(final List<String> values) {
    for (int index = 1; index < values.size(); index++) {
      if (values.get(index - 1).compareTo(values.get(index)) > 0) return false;
    }
    return true;
  }

  private static String assetSortKey(final String asset) {
    final byte[] bytes = AssetDefinitionIdEncoder.parseAddressBytes(asset);
    final StringBuilder result = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) result.append(String.format("%02x", value & 0xff));
    return result.toString();
  }

  private static byte[] decodeHash(final String value) {
    return AliasNameSupport.decodeHash(value);
  }
}
