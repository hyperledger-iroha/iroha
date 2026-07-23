package org.hyperledger.iroha.sdk.alias

import java.math.BigDecimal
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.crypto.IrohaHash

/** Decodes and re-encodes a framed instruction using the SDK instruction registry. */
fun interface AliasInstructionFrameRoundTripper {
    /** Returns the re-encoded frame for the supplied stable wire identifier and exact payload. */
    fun decodeAndReencode(wireId: String, framedPayload: ByteArray): ByteArray
}

/** A decoded EnsureAlias frame together with its canonical re-encoding. */
class DecodedEnsureAliasFrame(
    /** Typed instruction decoded from the planner frame. */
    @JvmField val instruction: EnsureAlias,
    reencodedFrame: ByteArray,
) {
    private val encoded: ByteArray = reencodedFrame.copyOf()

    /** Defensive copy of the re-encoded frame. */
    val reencodedFrame: ByteArray
        get() = encoded.copyOf()
}

/** Registry hook that decodes an EnsureAlias frame and re-encodes the same typed value. */
fun interface AliasEnsureInstructionFrameCodec {
    /** Decodes and canonically re-encodes one exact planner frame. */
    fun decodeAndReencode(wireId: String, framedPayload: ByteArray): DecodedEnsureAliasFrame
}

/** A decoded lifecycle frame together with its canonical re-encoding. */
class DecodedAliasLifecycleFrame(
    /** Typed lifecycle operation decoded from the planner frame. */
    @JvmField val operation: AliasLifecycleOperationV1,
    reencodedFrame: ByteArray,
) {
    private val encoded: ByteArray = reencodedFrame.copyOf()

    /** Defensive copy of the canonical re-encoding. */
    val reencodedFrame: ByteArray
        get() = encoded.copyOf()
}

/** Registry hook that round-trips a typed renewal or auto-renew frame. */
fun interface AliasLifecycleInstructionFrameCodec {
    /** Decodes and canonically re-encodes one exact lifecycle frame. */
    fun decodeAndReencode(wireId: String, framedPayload: ByteArray): DecodedAliasLifecycleFrame
}

/** Verification helpers used before locally signing an alias transaction plan. */
object AliasPlanVerifier {
    private val HASH_DOMAIN = "iroha:alias-transaction-plan-body:v1\u0000"
        .toByteArray(StandardCharsets.UTF_8)
    private val LIFECYCLE_HASH_DOMAIN = "iroha:alias-lifecycle-transaction-plan-body:v1\u0000"
        .toByteArray(StandardCharsets.UTF_8)

    /** Computes the canonical plan hash from the exact Norito encoding of the plan body. */
    @JvmStatic
    fun canonicalHash(canonicalBodyNorito: ByteArray): ByteArray =
        IrohaHash.prehash(HASH_DOMAIN + canonicalBodyNorito)

    /** Computes a lifecycle-plan commitment from the exact Norito body bytes. */
    @JvmStatic
    fun canonicalLifecycleHash(canonicalBodyNorito: ByteArray): ByteArray =
        IrohaHash.prehash(LIFECYCLE_HASH_DOMAIN + canonicalBodyNorito)

    /**
     * Verifies the carried plan hash against exact Norito body bytes.
     *
     * Hash text may be plain hexadecimal or carry an `0x` or `blake2b:` prefix.
     */
    @JvmStatic
    fun verifyHash(planHash: String, canonicalBodyNorito: ByteArray): Boolean {
        val expected = decodeHash(planHash) ?: return false
        return MessageDigest.isEqual(expected, canonicalHash(canonicalBodyNorito))
    }

    /** Verifies the hash carried by `plan`. */
    @JvmStatic
    fun verifyHash(plan: AliasTransactionPlanV1, canonicalBodyNorito: ByteArray): Boolean =
        verifyHash(plan.planHash, canonicalBodyNorito)

    /** Verifies the hash carried by a lifecycle plan against exact canonical body bytes. */
    @JvmStatic
    fun verifyLifecycleHash(
        plan: AliasLifecycleTransactionPlanV1,
        canonicalBodyNorito: ByteArray,
    ): Boolean {
        val expected = decodeHash(plan.planHash) ?: return false
        return MessageDigest.isEqual(expected, canonicalLifecycleHash(canonicalBodyNorito))
    }

    /**
     * Returns stable validation codes for a plan that is not safe to submit.
     *
     * The checks are independent of local clock time; transaction admission revalidates the block-time
     * deadline. Callers may separately compare `validUntilMs` for user-facing expiry warnings.
     */
    @JvmStatic
    fun validateExecutable(plan: AliasTransactionPlanV1): List<String> {
        val body = plan.body
        val errors = linkedSetOf<String>()
        if (body.version != AliasTransactionPlanBodyV1.VERSION) {
            errors += "alias.plan.version_unsupported"
        }
        if (body.blockers.isNotEmpty()) {
            errors += "alias.plan.blocked"
        }
        if (body.resources.isEmpty()) {
            errors += "alias.plan.resources_empty"
        }
        if (body.instructions.size != body.resources.size) {
            errors += "alias.plan.instruction_count_mismatch"
        }
        if (decodeHash(plan.planHash) == null) {
            errors += "alias.plan.hash_invalid"
        }
        if (!isDependencyOrdered(body.resources)) {
            errors += "alias.plan.resource_order_invalid"
        }
        if (!isSorted(body.totalsByAsset.map { assetSortKey(it.paymentAsset) + "\u0000" + it.amount })) {
            errors += "alias.plan.totals_not_canonical"
        }
        if (!isSorted(body.warnings.map { it.sortKey() }) || !isSorted(body.blockers.map { it.sortKey() })) {
            errors += "alias.plan.diagnostics_not_canonical"
        }
        if (body.instructions.any {
                it.wireId != EnsureAlias.WIRE_ID || it.framedPayload.isEmpty()
            }
        ) {
            errors += "alias.plan.instruction_invalid"
        }

        val claimedIndices = linkedSetOf<Long>()
        var previousInstructionIndex: Long? = null
        body.resources.forEach { resource ->
            val index = resource.instructionIndex
            if (index != null) {
                if (index >= body.instructions.size.toLong()) {
                    errors += "alias.plan.instruction_index_invalid"
                } else {
                    if (!claimedIndices.add(index)) {
                        errors += "alias.plan.instruction_index_duplicate"
                    }
                    previousInstructionIndex?.let { previous ->
                        if (index <= previous) errors += "alias.plan.instruction_indexes_not_ordered"
                    }
                    previousInstructionIndex = index
                    if (body.instructions[index.toInt()].wireId != EnsureAlias.WIRE_ID) {
                        errors += "alias.plan.instruction_wire_id_invalid"
                    }
                }
            }
            when (resource.disposition) {
                AliasPlanDispositionV1.NO_OP -> {
                    if (resource.quote != null || index == null) {
                        errors += "alias.plan.no_op_shape_invalid"
                    }
                }
                AliasPlanDispositionV1.REPAIR -> {
                    if (resource.quote != null || index == null) {
                        errors += "alias.plan.repair_shape_invalid"
                    }
                }
                AliasPlanDispositionV1.CREATE -> {
                    if (resource.quote == null || index == null) {
                        errors += "alias.plan.create_shape_invalid"
                    }
                }
                AliasPlanDispositionV1.CONFLICT -> {
                    errors += "alias.plan.conflict"
                    if (resource.quote != null || index != null) {
                        errors += "alias.plan.conflict_not_empty"
                    }
                }
            }
            val quote = resource.quote
            if (quote != null) {
                if (quote.target.toJsonMap() != targetFor(resource.intent).toJsonMap()) {
                    errors += "alias.plan.quote_target_mismatch"
                }
                if (!amountWithinCap(quote.exactAmount, quote.guard.maxAmount)) {
                    errors += "alias.plan.quote_cap_invalid"
                }
                if (quote.expiresAtMs > quote.graceExpiresAtMs ||
                    quote.graceExpiresAtMs > quote.redemptionExpiresAtMs
                ) {
                    errors += "alias.plan.quote_expiry_order_invalid"
                }
            }
        }
        if (claimedIndices.size != body.instructions.size) {
            errors += "alias.plan.instruction_unreferenced"
        }
        return errors.toList().sorted()
    }

    /** Decodes and re-encodes every frame, rejecting any byte-level change. */
    @JvmStatic
    fun verifyExactFrames(
        plan: AliasTransactionPlanV1,
        roundTripper: AliasInstructionFrameRoundTripper,
    ): Boolean = plan.body.instructions.all { instruction ->
        val original = instruction.framedPayload
        val encoded = try {
            roundTripper.decodeAndReencode(instruction.wireId, original.copyOf())
        } catch (_: RuntimeException) {
            return@all false
        }
        MessageDigest.isEqual(original, encoded)
    }

    /** Requires canonical shape, the exact plan hash, and exact instruction round trips. */
    @JvmStatic
    fun requireExecutable(
        plan: AliasTransactionPlanV1,
        canonicalBodyNorito: ByteArray,
        roundTripper: AliasInstructionFrameRoundTripper,
    ) {
        val errors = validateExecutable(plan).toMutableList()
        if (!verifyHash(plan, canonicalBodyNorito)) errors += "alias.plan.hash_mismatch"
        if (!verifyExactFrames(plan, roundTripper)) errors += "alias.plan.instruction_roundtrip_mismatch"
        require(errors.isEmpty()) { errors.distinct().sorted().joinToString(",") }
    }

    /**
     * Requires an executable plan to be the complete canonical rendering of the signed request.
     *
     * The typed codec closes the acquisition/guard substitution gap: every frame must decode to
     * the exact requested [EnsureAlias], in canonical dependency/resource order, before the frame
     * is handed to the generic transaction builder.
     */
    @JvmStatic
    fun requireExecutableForRequest(
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        canonicalBodyNorito: ByteArray,
        frameCodec: AliasEnsureInstructionFrameCodec,
    ) {
        val decoded = mutableListOf<EnsureAlias>()
        requireExecutable(
            plan,
            canonicalBodyNorito,
            AliasInstructionFrameRoundTripper { wireId, framedPayload ->
                val result = frameCodec.decodeAndReencode(wireId, framedPayload)
                decoded += result.instruction
                result.reencodedFrame
            },
        )
        val expected = request.intents.sortedWith(
            compareBy<EnsureAlias>({ it.intent.dependencyRank }, { it.intent.resourceText() }),
        )
        val errors = linkedSetOf<String>()
        if (decoded != expected) errors += "alias.plan.signed_request_mismatch"
        if (plan.body.resources.map { it.intent } != expected.map { it.intent }) {
            errors += "alias.plan.resource_request_mismatch"
        }
        require(errors.isEmpty()) { errors.sorted().joinToString(",") }
    }

    /** Returns stable validation codes when a lifecycle plan is unsafe to submit. */
    @JvmStatic
    fun validateLifecycleExecutable(plan: AliasLifecycleTransactionPlanV1): List<String> {
        val body = plan.body
        val errors = linkedSetOf<String>()
        if (body.version != AliasLifecycleTransactionPlanBodyV1.VERSION) {
            errors += "alias.lifecycle.plan.version_unsupported"
        }
        if (body.blockers.isNotEmpty()) errors += "alias.lifecycle.plan.blocked"
        if (decodeHash(plan.planHash) == null) errors += "alias.lifecycle.plan.hash_invalid"
        if (!isSorted(body.totalsByAsset.map { assetSortKey(it.paymentAsset) + "\u0000" + it.amount })) {
            errors += "alias.lifecycle.plan.totals_not_canonical"
        }
        if (!isSorted(body.warnings.map { it.sortKey() }) ||
            !isSorted(body.blockers.map { it.sortKey() })
        ) {
            errors += "alias.lifecycle.plan.diagnostics_not_canonical"
        }

        val expectedWireId = when (body.operation) {
            is AliasLifecycleOperationV1.RenewLease -> RenewAliasLease.WIRE_ID
            is AliasLifecycleOperationV1.ConfigureAutoRenew -> ConfigureAliasAutoRenew.WIRE_ID
        }
        when (body.disposition) {
            AliasLifecyclePlanDispositionV1.NO_OP -> {
                if (body.operation !is AliasLifecycleOperationV1.ConfigureAutoRenew ||
                    body.instruction != null || body.quote != null || body.totalsByAsset.isNotEmpty()
                ) {
                    errors += "alias.lifecycle.plan.no_op_shape_invalid"
                }
            }
            AliasLifecyclePlanDispositionV1.APPLY -> {
                val instruction = body.instruction
                if (instruction == null || instruction.wireId != expectedWireId || instruction.framedPayload.isEmpty()) {
                    errors += "alias.lifecycle.plan.instruction_invalid"
                }
                when (val operation = body.operation) {
                    is AliasLifecycleOperationV1.RenewLease -> {
                        val quote = body.quote
                        if (quote == null || body.totalsByAsset.size != 1) {
                            errors += "alias.lifecycle.plan.renewal_quote_invalid"
                        } else {
                            if (quote.target != operation.target ||
                                quote.guard != operation.renewal.quoteGuard ||
                                quote.expiresAtMs != operation.renewal.targetExpiryMs ||
                                body.validUntilMs != operation.renewal.quoteGuard.validUntilMs ||
                                !amountWithinCap(quote.exactAmount, quote.guard.maxAmount)
                            ) {
                                errors += "alias.lifecycle.plan.renewal_quote_mismatch"
                            }
                            val total = body.totalsByAsset[0]
                            if (total.paymentAsset != quote.guard.expectedPaymentAsset ||
                                total.amount != quote.exactAmount
                            ) {
                                errors += "alias.lifecycle.plan.renewal_total_mismatch"
                            }
                            if (quote.expiresAtMs > quote.graceExpiresAtMs ||
                                quote.graceExpiresAtMs > quote.redemptionExpiresAtMs
                            ) {
                                errors += "alias.lifecycle.plan.quote_expiry_order_invalid"
                            }
                        }
                    }
                    is AliasLifecycleOperationV1.ConfigureAutoRenew -> {
                        if (body.quote != null || body.totalsByAsset.isNotEmpty()) {
                            errors += "alias.lifecycle.plan.auto_renew_charge_invalid"
                        }
                    }
                }
            }
        }
        return errors.toList().sorted()
    }

    /** Requires a lifecycle plan to preserve the signed request, hash, and exact typed frame. */
    @JvmStatic
    fun requireLifecycleExecutableForRequest(
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        canonicalBodyNorito: ByteArray,
        frameCodec: AliasLifecycleInstructionFrameCodec,
    ) {
        val errors = validateLifecycleExecutable(plan).toMutableList()
        if (!verifyLifecycleHash(plan, canonicalBodyNorito)) {
            errors += "alias.lifecycle.plan.hash_mismatch"
        }
        if (plan.body.operation != request.operation) {
            errors += "alias.lifecycle.plan.signed_request_mismatch"
        }
        val instruction = plan.body.instruction
        if (instruction != null) {
            val decoded = try {
                frameCodec.decodeAndReencode(instruction.wireId, instruction.framedPayload)
            } catch (_: RuntimeException) {
                null
            }
            if (decoded == null ||
                decoded.operation != plan.body.operation ||
                !MessageDigest.isEqual(instruction.framedPayload, decoded.reencodedFrame)
            ) {
                errors += "alias.lifecycle.plan.instruction_roundtrip_mismatch"
            }
        }
        require(errors.isEmpty()) { errors.distinct().sorted().joinToString(",") }
    }

    private fun targetFor(intent: AliasIntentV1): AliasTargetV1 = when (intent) {
        is AliasIntentV1.Dataspace -> AliasTargetV1.Dataspace(intent.intent.dataspace)
        is AliasIntentV1.Domain -> AliasTargetV1.Domain(intent.intent.domain)
        is AliasIntentV1.AccountAlias -> AliasTargetV1.AccountAlias(intent.intent.alias)
    }

    private fun isDependencyOrdered(resources: List<AliasPlanResourceV1>): Boolean {
        var previous = -1
        for (resource in resources) {
            if (resource.intent.dependencyRank < previous) return false
            previous = resource.intent.dependencyRank
        }
        return true
    }

    private fun isSorted(values: List<String>): Boolean {
        for (index in 1 until values.size) {
            if (values[index - 1] > values[index]) return false
        }
        return true
    }

    private fun amountWithinCap(exact: String, cap: String): Boolean = try {
        val exactValue = BigDecimal(exact)
        val capValue = BigDecimal(cap)
        exactValue.signum() >= 0 && capValue.signum() >= 0 && exactValue <= capValue
    } catch (_: NumberFormatException) {
        false
    }

    private fun assetSortKey(asset: String): String =
        AssetDefinitionIdEncoder.parseAddressBytes(asset)
            .joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun decodeHash(value: String): ByteArray? = AliasHashText.decode(value)
}
