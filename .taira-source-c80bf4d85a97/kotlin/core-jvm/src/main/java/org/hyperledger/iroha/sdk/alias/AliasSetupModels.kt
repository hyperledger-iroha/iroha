package org.hyperledger.iroha.sdk.alias

import java.math.BigInteger
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

/** Account provisioning behavior requested by an account-alias intent. */
enum class AccountProvisionV1(@JvmField val wireValue: String) {
    EXISTING("existing"),
    CREATE("create");

    /** Returns the tagged Norito JSON representation. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to wireValue, "value" to null)
}

/** Whether an account alias is primary or additional. */
enum class AccountAliasRoleV1(@JvmField val wireValue: String) {
    PRIMARY("primary"),
    ADDITIONAL("additional");

    /** Returns the tagged Norito JSON representation. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to wireValue, "value" to null)
}

/** Lease terms used only when setup classifies a resource as absent. */
class AliasLeaseAcquisitionV1(
    termYears: Int,
    pricingClassHint: Int? = null,
) : AliasJsonValue() {
    /** Requested lease term in whole years. */
    @JvmField
    val termYears: Int = requireU8(termYears, "termYears", allowZero = false)

    /** Optional expected pricing class. */
    @JvmField
    val pricingClassHint: Int? = pricingClassHint?.let {
        requireU8(it, "pricingClassHint", allowZero = true)
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "term_years" to termYears,
        "pricing_class_hint" to pricingClassHint,
    )
}

/** Policy, payment asset, cap, and deadline guard for one lease operation. */
class AliasQuoteGuardV1(
    expectedPolicyVersion: Int,
    expectedPaymentAsset: String,
    maxAmount: String,
    validUntilMs: Long,
) : AliasJsonValue() {
    /** Policy version consensus must observe. */
    @JvmField
    val expectedPolicyVersion: Int = requireU16(expectedPolicyVersion, "expectedPolicyVersion")

    /** Canonical payment asset identifier consensus must observe. */
    @JvmField
    val expectedPaymentAsset: String = requireCanonicalAsset(expectedPaymentAsset, "expectedPaymentAsset")

    /** Canonical maximum quantity authorized by the payer. */
    @JvmField
    val maxAmount: String = requireCanonicalQuantity(maxAmount, "maxAmount")

    /** Last block timestamp at which the quote may be used. */
    @JvmField
    val validUntilMs: Long = requireNonNegative(validUntilMs, "validUntilMs")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "expected_policy_version" to expectedPolicyVersion,
        "expected_payment_asset" to expectedPaymentAsset,
        "max_amount" to maxAmount,
        "valid_until_ms" to validUntilMs,
    )
}

/** Desired state for one dataspace alias. */
class AliasDataSpaceIntentV1(
    /** Resolved dataspace name. */
    @JvmField val dataspace: ResolvedDataSpaceV1,
    owner: String,
) : AliasJsonValue() {
    /** Exact canonical owner. */
    @JvmField
    val owner: String = requireCanonicalI105Address(owner, "owner")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "dataspace" to dataspace.toJsonMap(),
        "owner" to owner,
    )
}

/** Desired state for one domain. */
class AliasDomainIntentV1(
    /** Resolved domain name. */
    @JvmField val domain: ResolvedDomainV1,
    owner: String,
) : AliasJsonValue() {
    /** Exact canonical owner. */
    @JvmField
    val owner: String = requireCanonicalI105Address(owner, "owner")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "domain" to domain.toJsonMap(),
        "owner" to owner,
    )
}

/** Desired state for one account alias. */
class AliasAccountIntentV1(
    /** Resolved account alias. */
    @JvmField val alias: ResolvedAccountAliasV1,
    targetAccount: String,
    /** Whether the target must already exist. */
    @JvmField val provision: AccountProvisionV1,
    /** Whether the alias is primary or additional. */
    @JvmField val role: AccountAliasRoleV1,
) : AliasJsonValue() {
    /** Exact canonical target account. */
    @JvmField
    val targetAccount: String = requireCanonicalI105Address(targetAccount, "targetAccount")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "alias" to alias.toJsonMap(),
        "target_account" to targetAccount,
        "provision" to provision.toJsonMap(),
        "role" to role.toJsonMap(),
    )
}

/** Declarative desired state for one alias/SNS resource. */
sealed class AliasIntentV1 : AliasJsonValue() {
    /** Stable JSON variant name. */
    abstract val kind: String

    /** Dependency-order rank: dataspace, domain, account alias. */
    abstract val dependencyRank: Int

    /** Exact resource text used in diagnostics. */
    abstract fun resourceText(): String

    /** Dataspace desired state. */
    class Dataspace(/** Exact intent payload. */ @JvmField val intent: AliasDataSpaceIntentV1) : AliasIntentV1() {
        override val kind: String = "dataspace"
        override val dependencyRank: Int = 0
        override fun resourceText(): String = intent.dataspace.canonicalName
        override fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to kind, "intent" to intent.toJsonMap())
    }

    /** Domain desired state. */
    class Domain(/** Exact intent payload. */ @JvmField val intent: AliasDomainIntentV1) : AliasIntentV1() {
        override val kind: String = "domain"
        override val dependencyRank: Int = 1
        override fun resourceText(): String = intent.domain.canonicalName
        override fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to kind, "intent" to intent.toJsonMap())
    }

    /** Account-alias desired state. */
    class AccountAlias(/** Exact intent payload. */ @JvmField val intent: AliasAccountIntentV1) : AliasIntentV1() {
        override val kind: String = "account_alias"
        override val dependencyRank: Int = 2
        override fun resourceText(): String = intent.alias.canonicalName.canonicalText()
        override fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to kind, "intent" to intent.toJsonMap())
    }
}

/** Exact resolved resource supported by setup and lifecycle operations. */
sealed class AliasTargetV1 : AliasJsonValue() {
    /** Stable JSON variant name. */
    abstract val kind: String

    /** Dataspace target. */
    class Dataspace(/** Resolved target. */ @JvmField val resource: ResolvedDataSpaceV1) : AliasTargetV1() {
        override val kind: String = "dataspace"
        override fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to kind, "resource" to resource.toJsonMap())
    }

    /** Domain target. */
    class Domain(/** Resolved target. */ @JvmField val resource: ResolvedDomainV1) : AliasTargetV1() {
        override val kind: String = "domain"
        override fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to kind, "resource" to resource.toJsonMap())
    }

    /** Account-alias target. */
    class AccountAlias(/** Resolved target. */ @JvmField val resource: ResolvedAccountAliasV1) : AliasTargetV1() {
        override val kind: String = "account_alias"
        override fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to kind, "resource" to resource.toJsonMap())
    }
}

/** Exact scope carried by account-alias manage, delegate, and resolve permissions. */
sealed class AccountAliasPermissionScope : AliasJsonValue() {
    /** Permission scoped to one canonical domain. */
    class Domain(domain: String) : AccountAliasPermissionScope() {
        /** Canonical `domain.dataspace` literal. */
        @JvmField
        val domain: String = ResolvedDomainV1(domain, BigInteger.ZERO).canonicalName

        override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
            "scope" to "domain",
            "value" to domain,
        )
    }

    /** Permission scoped to one numeric dataspace. */
    class Dataspace(dataspaceId: BigInteger) : AccountAliasPermissionScope() {
        /** Full unsigned dataspace identifier. */
        @JvmField
        val dataspaceId: BigInteger = requireU64(dataspaceId, "dataspaceId")

        override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
            "scope" to "dataspace",
            "value" to dataspaceId,
        )
    }

    /** Permission scoped to one exact resolved account alias. */
    class Alias(@JvmField val alias: ResolvedAccountAliasV1) : AccountAliasPermissionScope() {
        override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
            "scope" to "alias",
            "value" to alias.toJsonMap(),
        )
    }
}

/** One `iroha.alias.ensure` instruction. */
class EnsureAlias(
    /** Exact desired resource state. */
    @JvmField val intent: AliasIntentV1,
    /** Terms used only for acquisition. */
    @JvmField val acquisition: AliasLeaseAcquisitionV1,
    /** Quote guard recomputed by consensus. */
    @JvmField val quoteGuard: AliasQuoteGuardV1,
) : AliasJsonValue() {
    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "intent" to intent.toJsonMap(),
        "acquisition" to acquisition.toJsonMap(),
        "quote_guard" to quoteGuard.toJsonMap(),
    )

    companion object {
        /** Stable instruction registry identifier. */
        const val WIRE_ID: String = "iroha.alias.ensure"
    }
}

/** Canonical signed request body for one indivisible alias setup plan. */
class AliasSetupPlanRequestV1(
    intents: List<EnsureAlias>,
    schemaVersion: Int = VERSION,
) : AliasJsonValue() {
    /** Request layout version. */
    @JvmField
    val schemaVersion: Int = schemaVersion.also {
        require(it == VERSION) { "schemaVersion must be $VERSION" }
    }

    /** Exact setup instructions that the planner must preserve as one transaction. */
    @JvmField
    val intents: List<EnsureAlias> = intents.toList().also { values ->
        require(values.isNotEmpty()) { "intents must not be empty" }
        val resources = values.map { it.intent.kind + "\u0000" + it.intent.resourceText() }
        require(resources.toSet().size == resources.size) {
            "intents must not contain the same resource more than once"
        }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema_version" to schemaVersion,
        "intents" to intents.map { it.toJsonMap() },
    )

    companion object {
        /** Current planner request layout. */
        const val VERSION: Int = 1
    }
}

/** Expiry-CAS lease renewal; the transaction authority is the payer. */
class RenewAliasLease(
    /** Exact resolved lease target. */
    @JvmField val target: AliasTargetV1,
    expectedCurrentExpiryMs: Long,
    targetExpiryMs: Long,
    /** Policy, asset, cap, and deadline guard. */
    @JvmField val quoteGuard: AliasQuoteGuardV1,
) : AliasJsonValue() {
    /** Expiry that must still be current at execution. */
    @JvmField
    val expectedCurrentExpiryMs: Long = requireNonNegative(expectedCurrentExpiryMs, "expectedCurrentExpiryMs")

    /** Absolute expiry to install after charging the exact recomputed quote. */
    @JvmField
    val targetExpiryMs: Long = requireNonNegative(targetExpiryMs, "targetExpiryMs").also {
        require(it > this.expectedCurrentExpiryMs) { "targetExpiryMs must be later than expectedCurrentExpiryMs" }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "target" to target.toJsonMap(),
        "expected_current_expiry_ms" to expectedCurrentExpiryMs,
        "target_expiry_ms" to targetExpiryMs,
        "quote_guard" to quoteGuard.toJsonMap(),
    )

    companion object {
        /** Stable instruction registry identifier. */
        const val WIRE_ID: String = "iroha.alias.lease.renew"
    }
}

/** Owner-configured deterministic native auto-renew policy. */
class AliasAutoRenewConfigV1(
    termYears: Int,
    policyVersion: Int,
    paymentAsset: String,
    maxAmount: String,
    renewBeforeExpiryMs: Long,
    retryBackoffMs: Long,
    maxFailures: Long,
) : AliasJsonValue() {
    /** Renewal term in whole years. */
    @JvmField
    val termYears: Int = requireU8(termYears, "termYears", allowZero = false)

    /** SNS policy version accepted by the owner. */
    @JvmField
    val policyVersion: Int = requireU16(policyVersion, "policyVersion")

    /** Payment asset accepted by the owner. */
    @JvmField
    val paymentAsset: String = requireCanonicalAsset(paymentAsset, "paymentAsset")

    /** Maximum exact renewal charge authorized by the owner. */
    @JvmField
    val maxAmount: String = requireCanonicalQuantity(maxAmount, "maxAmount")

    /** Time before expiry at which attempts begin. */
    @JvmField
    val renewBeforeExpiryMs: Long = requireNonNegative(renewBeforeExpiryMs, "renewBeforeExpiryMs").also {
        require(it > 0) { "renewBeforeExpiryMs must be positive" }
    }

    /** Deterministic delay between failed attempts. */
    @JvmField
    val retryBackoffMs: Long = requireNonNegative(retryBackoffMs, "retryBackoffMs").also {
        require(it > 0) { "retryBackoffMs must be positive" }
    }

    /** Failure limit after which renewal is suspended. */
    @JvmField
    val maxFailures: Long = maxFailures.also { require(it in 1..0xffff_ffffL) { "maxFailures must fit in u32" } }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "term_years" to termYears,
        "policy_version" to policyVersion,
        "payment_asset" to paymentAsset,
        "max_amount" to maxAmount,
        "renew_before_expiry_ms" to renewBeforeExpiryMs,
        "retry_backoff_ms" to retryBackoffMs,
        "max_failures" to maxFailures,
    )
}

/** Revision-CAS instruction that enables or disables native auto-renew. */
class ConfigureAliasAutoRenew(
    /** Exact resolved lease target. */
    @JvmField val target: AliasTargetV1,
    expectedRevision: Long,
    /** New configuration, or null to disable. */
    @JvmField val config: AliasAutoRenewConfigV1?,
) : AliasJsonValue() {
    /** Revision that must still be current at execution. */
    @JvmField
    val expectedRevision: Long = requireNonNegative(expectedRevision, "expectedRevision")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "target" to target.toJsonMap(),
        "expected_revision" to expectedRevision,
        "config" to config?.toJsonMap(),
    )

    companion object {
        /** Stable instruction registry identifier. */
        const val WIRE_ID: String = "iroha.alias.auto_renew.configure"
    }
}

/** Versioned canonical request accepted by an alias lifecycle planner. */
sealed class AliasLifecyclePlanRequestV1 : AliasJsonValue() {
    /** Exact lifecycle operation that the planner must preserve. */
    abstract val operation: AliasLifecycleOperationV1
}

/** Canonical signed request body for one lease-renewal plan. */
class AliasLeaseRenewPlanRequestV1(
    /** Exact absolute-expiry compare-and-set renewal. */
    @JvmField val renewal: RenewAliasLease,
    schemaVersion: Int = VERSION,
) : AliasLifecyclePlanRequestV1() {
    /** Request layout version. */
    @JvmField
    val schemaVersion: Int = schemaVersion.also {
        require(it == VERSION) { "schemaVersion must be $VERSION" }
    }

    override val operation: AliasLifecycleOperationV1 = AliasLifecycleOperationV1.RenewLease(renewal)

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema_version" to schemaVersion,
        "renewal" to renewal.toJsonMap(),
    )

    companion object {
        /** Current renewal planner request layout. */
        const val VERSION: Int = 1
    }
}

/** Canonical signed request body for one native auto-renew configuration plan. */
class AliasAutoRenewPlanRequestV1(
    /** Exact revision-CAS configuration change. */
    @JvmField val configuration: ConfigureAliasAutoRenew,
    schemaVersion: Int = VERSION,
) : AliasLifecyclePlanRequestV1() {
    /** Request layout version. */
    @JvmField
    val schemaVersion: Int = schemaVersion.also {
        require(it == VERSION) { "schemaVersion must be $VERSION" }
    }

    override val operation: AliasLifecycleOperationV1 =
        AliasLifecycleOperationV1.ConfigureAutoRenew(configuration)

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema_version" to schemaVersion,
        "configuration" to configuration.toJsonMap(),
    )

    companion object {
        /** Current auto-renew planner request layout. */
        const val VERSION: Int = 1
    }
}

/** Exact lifecycle operation committed by a lifecycle transaction plan. */
sealed class AliasLifecycleOperationV1 : AliasJsonValue() {
    /** Stable JSON variant name. */
    abstract val kind: String

    /** Exact resource targeted by this operation. */
    abstract val target: AliasTargetV1

    /** Absolute-expiry lease renewal. */
    class RenewLease(/** Exact renewal. */ @JvmField val renewal: RenewAliasLease) :
        AliasLifecycleOperationV1() {
        override val kind: String = "renew_lease"
        override val target: AliasTargetV1 = renewal.target
        override fun toJsonMap(): Map<String, Any?> =
            linkedMapOf("kind" to kind, "operation" to renewal.toJsonMap())
    }

    /** Enable, replace, or disable deterministic native auto-renew. */
    class ConfigureAutoRenew(
        /** Exact configuration CAS. */ @JvmField val configuration: ConfigureAliasAutoRenew,
    ) : AliasLifecycleOperationV1() {
        override val kind: String = "configure_auto_renew"
        override val target: AliasTargetV1 = configuration.target
        override fun toJsonMap(): Map<String, Any?> =
            linkedMapOf("kind" to kind, "operation" to configuration.toJsonMap())
    }
}

/** Whether a lifecycle plan requires a transaction or is an exact no-op. */
enum class AliasLifecyclePlanDispositionV1(@JvmField val wireValue: String) {
    NO_OP("no_op"),
    APPLY("apply");

    /** Returns the tagged Norito JSON representation. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to wireValue, "value" to null)
}

/** Target-CAS account-alias rebind instruction; lease state is not accepted. */
class RebindAccountAlias(
    /** Exact resolved alias being rebound. */
    @JvmField val alias: ResolvedAccountAliasV1,
    expectedTargetAccount: String,
    newTargetAccount: String,
) : AliasJsonValue() {
    /** Account that must currently be bound. */
    @JvmField
    val expectedTargetAccount: String = requireCanonicalI105Address(expectedTargetAccount, "expectedTargetAccount")

    /** Account to bind after the compare-and-set succeeds. */
    @JvmField
    val newTargetAccount: String = requireCanonicalI105Address(newTargetAccount, "newTargetAccount")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "alias" to alias.toJsonMap(),
        "expected_target_account" to expectedTargetAccount,
        "new_target_account" to newTargetAccount,
    )

    companion object {
        /** Stable instruction registry identifier. */
        const val WIRE_ID: String = "iroha.account.alias.rebind"
    }
}

/** Primary-alias compare-and-set instruction; lease state is not accepted. */
class CompareAndSetPrimaryAccountAlias(
    account: String,
    /** Alias that must currently be primary, or null if none is expected. */
    @JvmField val expectedAlias: ResolvedAccountAliasV1?,
    /** New primary alias, or null to clear it. */
    @JvmField val newAlias: ResolvedAccountAliasV1?,
) : AliasJsonValue() {
    /** Account whose primary alias is changing. */
    @JvmField
    val account: String = requireCanonicalI105Address(account, "account")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "account" to account,
        "expected_alias" to expectedAlias?.toJsonMap(),
        "new_alias" to newAlias?.toJsonMap(),
    )

    companion object {
        /** Stable instruction registry identifier. */
        const val WIRE_ID: String = "iroha.account.alias.primary.compare_and_set"
    }
}

/** Planner classification for one resource. */
enum class AliasPlanDispositionV1(@JvmField val wireValue: String) {
    NO_OP("no_op"),
    REPAIR("repair"),
    CREATE("create"),
    CONFLICT("conflict");

    /** Returns the tagged Norito JSON representation. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf("kind" to wireValue, "value" to null)
}

/** Exact lease quote attached to a create or renewal plan resource. */
class AliasLeaseQuoteV1(
    /** Exact resolved target. */
    @JvmField val target: AliasTargetV1,
    pricingClass: Int,
    exactAmount: String,
    /** Policy, asset, cap, and deadline guard. */
    @JvmField val guard: AliasQuoteGuardV1,
    expiresAtMs: Long,
    graceExpiresAtMs: Long,
    redemptionExpiresAtMs: Long,
) : AliasJsonValue() {
    /** Pricing class selected by policy. */
    @JvmField
    val pricingClass: Int = requireU8(pricingClass, "pricingClass", allowZero = true)

    /** Exact amount consensus will charge. */
    @JvmField
    val exactAmount: String = requireCanonicalQuantity(exactAmount, "exactAmount")

    /** Resulting paid-term expiry. */
    @JvmField
    val expiresAtMs: Long = requireNonNegative(expiresAtMs, "expiresAtMs")

    /** Resulting grace-period expiry. */
    @JvmField
    val graceExpiresAtMs: Long = requireNonNegative(graceExpiresAtMs, "graceExpiresAtMs")

    /** Resulting redemption-period expiry. */
    @JvmField
    val redemptionExpiresAtMs: Long = requireNonNegative(redemptionExpiresAtMs, "redemptionExpiresAtMs")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "target" to target.toJsonMap(),
        "pricing_class" to pricingClass,
        "exact_amount" to exactAmount,
        "guard" to guard.toJsonMap(),
        "expires_at_ms" to expiresAtMs,
        "grace_expires_at_ms" to graceExpiresAtMs,
        "redemption_expires_at_ms" to redemptionExpiresAtMs,
    )
}

/** Planner result for one ordered resource intent. */
class AliasPlanResourceV1(
    /** Canonically resolved desired state. */
    @JvmField val intent: AliasIntentV1,
    /** Fixed idempotency classification. */
    @JvmField val disposition: AliasPlanDispositionV1,
    /** Exact quote for acquisition or renewal. */
    @JvmField val quote: AliasLeaseQuoteV1?,
    instructionIndex: Long?,
) : AliasJsonValue() {
    /** Index of the matching executable instruction, if any. */
    @JvmField
    val instructionIndex: Long? = instructionIndex?.let {
        require(it in 0..0xffff_ffffL) { "instructionIndex must be an unsigned 32-bit integer" }
        it
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "intent" to intent.toJsonMap(),
        "disposition" to disposition.toJsonMap(),
        "quote" to quote?.toJsonMap(),
        "instruction_index" to instructionIndex,
    )
}

/** Exact framed Norito instruction returned by the planner. */
class AliasFramedInstructionV1(
    wireId: String,
    framedPayload: ByteArray,
) : AliasJsonValue() {
    /** Stable instruction wire identifier. */
    @JvmField
    val wireId: String = requireCanonicalToken(wireId, "wireId")

    private val payload: ByteArray = framedPayload.copyOf()

    /** Defensive copy of the exact planner frame. */
    val framedPayload: ByteArray
        get() = payload.copyOf()

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "wire_id" to wireId,
        "framed_payload" to payload.map { it.toInt() and 0xff },
    )
}

/** Exact total charge for one payment asset. */
class AliasAssetTotalV1(
    paymentAsset: String,
    amount: String,
) : AliasJsonValue() {
    /** Canonical payment asset. */
    @JvmField
    val paymentAsset: String = requireCanonicalAsset(paymentAsset, "paymentAsset")

    /** Exact aggregate quantity. */
    @JvmField
    val amount: String = requireCanonicalQuantity(amount, "amount")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "payment_asset" to paymentAsset,
        "amount" to amount,
    )
}

/** Overall setup/readiness state. */
enum class AliasSetupStatusV1(@JvmField val wireValue: String) {
    READY("ready"),
    PENDING("pending"),
    BLOCKED("blocked");

    /** Returns the tagged Norito JSON representation. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf("status" to wireValue, "value" to null)
}

/** Setup/readiness validation phase. */
enum class AliasSetupValidationPhaseV1(@JvmField val wireValue: String) {
    CONFIG("config"),
    CATALOG("catalog"),
    BOOTSTRAP("bootstrap"),
    WORLD_STATE("world_state"),
    PLANNING("planning");

    /** Returns the tagged Norito JSON representation. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf("phase" to wireValue, "value" to null)
}

/** Setup/readiness diagnostic severity. */
enum class AliasSetupSeverityV1(@JvmField val wireValue: String) {
    INFO("info"),
    WARNING("warning"),
    ERROR("error");

    /** Returns the tagged Norito JSON representation. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf("severity" to wireValue, "value" to null)
}

/** One stable, secret-free setup/readiness diagnostic. */
class AliasSetupDiagnosticV1(
    /** Validation phase. */
    @JvmField val phase: AliasSetupValidationPhaseV1,
    code: String,
    /** Diagnostic severity. */
    @JvmField val severity: AliasSetupSeverityV1,
    resource: String? = null,
    configPath: String? = null,
    expected: String? = null,
    actual: String? = null,
    remediation: String,
) : AliasJsonValue(), Comparable<AliasSetupDiagnosticV1> {
    /** Stable machine-readable code. */
    @JvmField
    val code: String = requireCanonicalToken(code, "code")

    /** Canonical resource text, if known. */
    @JvmField
    val resource: String? = resource?.let { requireNonBlank(it, "resource") }

    /** Relevant configuration path. */
    @JvmField
    val configPath: String? = configPath?.let { requireNonBlank(it, "configPath") }

    /** Redacted expected value. */
    @JvmField
    val expected: String? = expected?.let { requireNonBlank(it, "expected") }

    /** Redacted actual value. */
    @JvmField
    val actual: String? = actual?.let { requireNonBlank(it, "actual") }

    /** Human-readable corrective action. */
    @JvmField
    val remediation: String = requireNonBlank(remediation, "remediation")

    internal fun sortKey(): String = listOf(
        phase.ordinal.toString(), code, severity.ordinal.toString(), resource.orEmpty(), configPath.orEmpty(),
        expected.orEmpty(), actual.orEmpty(), remediation,
    ).joinToString("\u0000")

    override fun compareTo(other: AliasSetupDiagnosticV1): Int = sortKey().compareTo(other.sortKey())

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "phase" to phase.toJsonMap(),
        "code" to code,
        "severity" to severity.toJsonMap(),
        "resource" to resource,
        "config_path" to configPath,
        "expected" to expected,
        "actual" to actual,
        "remediation" to remediation,
    )
}

/** Deterministically ordered setup/readiness diagnostics. */
class AliasSetupReportV1(
    /** Overall readiness state. */
    @JvmField val status: AliasSetupStatusV1,
    diagnostics: List<AliasSetupDiagnosticV1>,
) : AliasJsonValue() {
    /** Report layout version. */
    @JvmField
    val version: Int = VERSION

    /** Stable diagnostics in canonical field order. */
    @JvmField
    val diagnostics: List<AliasSetupDiagnosticV1> = diagnostics.sorted()

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "version" to version,
        "status" to status.toJsonMap(),
        "diagnostics" to diagnostics.map { it.toJsonMap() },
    )

    companion object {
        /** Current report layout version. */
        const val VERSION: Int = 1
    }
}

/** World-state anchor used to classify an alias plan. */
class AliasPlanAnchorV1(
    blockHeight: Long,
    blockHash: String,
) : AliasJsonValue() {
    /** Height of the anchored block. */
    @JvmField
    val blockHeight: Long = requireNonNegative(blockHeight, "blockHeight")

    /** Hash of the anchored block. */
    @JvmField
    val blockHash: String = requireHashText(blockHash, "blockHash")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "block_height" to blockHeight,
        "block_hash" to blockHash,
    )
}

/** Canonical body committed by an alias transaction plan hash. */
class AliasTransactionPlanBodyV1(
    version: Int,
    authority: String,
    chainId: String,
    /** World-state classification anchor. */
    @JvmField val anchor: AliasPlanAnchorV1,
    resources: List<AliasPlanResourceV1>,
    instructions: List<AliasFramedInstructionV1>,
    totalsByAsset: List<AliasAssetTotalV1>,
    warnings: List<AliasSetupDiagnosticV1>,
    blockers: List<AliasSetupDiagnosticV1>,
    validUntilMs: Long,
) : AliasJsonValue() {
    /** Layout version; currently `1`. */
    @JvmField
    val version: Int = version.also { require(it in 0..255) { "version must be an unsigned byte" } }

    /** Transaction authority and lease payer. */
    @JvmField
    val authority: String = requireCanonicalI105Address(authority, "authority")

    /** Target chain identifier. */
    @JvmField
    val chainId: String = requireNonBlank(chainId, "chainId")

    /** Ordered resources in dependency order. */
    @JvmField
    val resources: List<AliasPlanResourceV1> = resources.toList()

    /** Ordered exact framed instructions for one transaction. */
    @JvmField
    val instructions: List<AliasFramedInstructionV1> = instructions.toList()

    /** Exact totals sorted by canonical payment asset ID. */
    @JvmField
    val totalsByAsset: List<AliasAssetTotalV1> = totalsByAsset.toList()

    /** Non-blocking planner diagnostics. */
    @JvmField
    val warnings: List<AliasSetupDiagnosticV1> = warnings.toList()

    /** Blocking planner diagnostics. */
    @JvmField
    val blockers: List<AliasSetupDiagnosticV1> = blockers.toList()

    /** Last block timestamp at which the plan may be submitted. */
    @JvmField
    val validUntilMs: Long = requireNonNegative(validUntilMs, "validUntilMs")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "version" to version,
        "authority" to authority,
        "chain_id" to chainId,
        "anchor" to anchor.toJsonMap(),
        "resources" to resources.map { it.toJsonMap() },
        "instructions" to instructions.map { it.toJsonMap() },
        "totals_by_asset" to totalsByAsset.map { it.toJsonMap() },
        "warnings" to warnings.map { it.toJsonMap() },
        "blockers" to blockers.map { it.toJsonMap() },
        "valid_until_ms" to validUntilMs,
    )

    companion object {
        /** Current canonical plan-body version. */
        const val VERSION: Int = 1
    }
}

/** Alias transaction plan and its canonical body commitment. */
class AliasTransactionPlanV1(
    /** Canonical plan body. */
    @JvmField val body: AliasTransactionPlanBodyV1,
    planHash: String,
) : AliasJsonValue() {
    /** Domain-separated Iroha hash commitment to the Norito-encoded body. */
    @JvmField
    val planHash: String = requireHashText(planHash, "planHash")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "body" to body.toJsonMap(),
        "plan_hash" to planHash,
    )
}

/** Canonical body committed by an alias lifecycle transaction plan hash. */
class AliasLifecycleTransactionPlanBodyV1(
    version: Int,
    authority: String,
    chainId: String,
    /** World-state classification anchor. */
    @JvmField val anchor: AliasPlanAnchorV1,
    /** Exact signed lifecycle operation. */
    @JvmField val operation: AliasLifecycleOperationV1,
    /** Whether applying the exact operation requires a transaction. */
    @JvmField val disposition: AliasLifecyclePlanDispositionV1,
    /** Exact framed instruction for APPLY, absent for NO_OP. */
    @JvmField val instruction: AliasFramedInstructionV1?,
    /** Exact renewal quote, present only for lease renewal. */
    @JvmField val quote: AliasLeaseQuoteV1?,
    totalsByAsset: List<AliasAssetTotalV1>,
    warnings: List<AliasSetupDiagnosticV1>,
    blockers: List<AliasSetupDiagnosticV1>,
    validUntilMs: Long,
) : AliasJsonValue() {
    /** Layout version; currently `1`. */
    @JvmField
    val version: Int = version.also { require(it == VERSION) { "version must be $VERSION" } }

    /** Transaction authority and renewal payer. */
    @JvmField
    val authority: String = requireCanonicalI105Address(authority, "authority")

    /** Target chain identifier. */
    @JvmField
    val chainId: String = requireNonBlank(chainId, "chainId")

    /** Exact totals in canonical payment-asset order. */
    @JvmField
    val totalsByAsset: List<AliasAssetTotalV1> = totalsByAsset.toList()

    /** Non-blocking planner diagnostics. */
    @JvmField
    val warnings: List<AliasSetupDiagnosticV1> = warnings.toList()

    /** Blocking planner diagnostics. */
    @JvmField
    val blockers: List<AliasSetupDiagnosticV1> = blockers.toList()

    /** Last block timestamp at which the plan may be submitted. */
    @JvmField
    val validUntilMs: Long = requireNonNegative(validUntilMs, "validUntilMs")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "version" to version,
        "authority" to authority,
        "chain_id" to chainId,
        "anchor" to anchor.toJsonMap(),
        "operation" to operation.toJsonMap(),
        "disposition" to disposition.toJsonMap(),
        "instruction" to instruction?.toJsonMap(),
        "quote" to quote?.toJsonMap(),
        "totals_by_asset" to totalsByAsset.map { it.toJsonMap() },
        "warnings" to warnings.map { it.toJsonMap() },
        "blockers" to blockers.map { it.toJsonMap() },
        "valid_until_ms" to validUntilMs,
    )

    companion object {
        /** Current lifecycle plan-body layout. */
        const val VERSION: Int = 1
    }
}

/** Alias lifecycle transaction plan and canonical body commitment. */
class AliasLifecycleTransactionPlanV1(
    /** Canonical lifecycle plan body. */
    @JvmField val body: AliasLifecycleTransactionPlanBodyV1,
    planHash: String,
) : AliasJsonValue() {
    /** Domain-separated hash of the canonical Norito body. */
    @JvmField
    val planHash: String = requireHashText(planHash, "planHash")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "body" to body.toJsonMap(),
        "plan_hash" to planHash,
    )
}

private fun requireU8(value: Int, field: String, allowZero: Boolean): Int {
    require(value in (if (allowZero) 0 else 1)..255) { "$field must fit in an unsigned byte" }
    return value
}

private fun requireU16(value: Int, field: String): Int {
    require(value in 0..65535) { "$field must fit in an unsigned 16-bit integer" }
    return value
}

private fun requireNonNegative(value: Long, field: String): Long {
    require(value >= 0) { "$field must not be negative" }
    return value
}

private fun requireNonBlank(value: String, field: String): String {
    require(value.isNotBlank() && value == value.trim()) { "$field must be non-blank without surrounding whitespace" }
    require(value.none { it.isISOControl() }) { "$field must not contain control characters" }
    return value
}

private fun requireCanonicalToken(value: String, field: String): String {
    requireNonBlank(value, field)
    require(value.none { it.isWhitespace() }) { "$field must not contain whitespace" }
    return value
}

private fun requireCanonicalAsset(value: String, field: String): String {
    require(AssetDefinitionIdEncoder.isCanonicalAddress(value)) {
        "$field must use a canonical unprefixed Base58 asset-definition address"
    }
    return value
}

private fun requireCanonicalQuantity(value: String, field: String): String {
    try {
        KotodamaQuantity.parseCanonical(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must use a canonical non-negative quantity", ex)
    }
    return value
}

private fun requireHashText(value: String, field: String): String {
    require(AliasHashText.decode(value) != null) { "$field must be a canonical 32-byte hash" }
    return value
}

/** Converts a non-negative signed value to an unsigned dataspace identifier. */
fun dataspaceId(value: Long): BigInteger {
    require(value >= 0) { "dataspaceId must not be negative" }
    return BigInteger.valueOf(value)
}
