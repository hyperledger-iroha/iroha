package org.hyperledger.iroha.sdk.alias

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.client.JsonParser

/** Secret-free intent accepted by the sponsored account-onboarding planner. */
class AccountOnboardingPlanRequestV1(
    alias: String,
    accountId: String,
    permissions: List<String> = emptyList(),
    version: Int = VERSION,
) : AliasJsonValue() {
    /** Request layout version. */
    @JvmField
    val version: Int = version.also { require(it == VERSION) { "version must be $VERSION" } }

    /** Canonical catalog-free account alias. */
    @JvmField
    val alias: String = AccountAliasName.parse(alias).canonicalText()

    /** Canonical domainless account to create or repair. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Deterministically sorted permission names selected from the server allowlist. */
    @JvmField
    val permissions: List<String> = permissions.map { requireOnboardingToken(it, "permission") }
        .toSortedSet()
        .toList()

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "version" to version,
        "alias" to alias,
        "account_id" to accountId,
        "permissions" to permissions,
    )

    companion object {
        /** Current sponsored onboarding request layout. */
        const val VERSION: Int = 1
    }
}

/** Canonical body signed by a stateless sponsored-onboarding receipt. */
class AccountOnboardingPlanBodyV1(
    version: Int,
    /** Canonical embedded request. */ @JvmField val request: AccountOnboardingPlanRequestV1,
    authority: String,
    chainId: String,
    /** World-state planning anchor. */ @JvmField val anchor: AliasPlanAnchorV1,
    /** Canonically resolved account-alias disposition. */ @JvmField val resource: AliasPlanResourceV1,
    /** Acquisition terms used only if the alias remains absent. */
    @JvmField val acquisition: AliasLeaseAcquisitionV1,
    /** Guard revalidated immediately before server submission. */ @JvmField val quoteGuard: AliasQuoteGuardV1,
    instructions: List<AliasFramedInstructionV1>,
    /** Optional owner-signed native auto-renew follow-up frame. */
    @JvmField val ownerAutoRenewInstruction: AliasFramedInstructionV1?,
    validUntilMs: Long,
) : AliasJsonValue() {
    /** Receipt body layout version. */
    @JvmField
    val version: Int = version.also { require(it == VERSION) { "version must be $VERSION" } }

    /** Configured Torii onboarding authority. */
    @JvmField
    val authority: String = requireCanonicalI105Address(authority, "authority")

    /** Chain to which the receipt is bound. */
    @JvmField
    val chainId: String = chainId.also { requireOnboardingToken(it, "chainId") }

    /** Exact ordered server-signed transaction frames. */
    @JvmField
    val instructions: List<AliasFramedInstructionV1> = instructions.toList()

    /** Last block timestamp at which the receipt may be applied. */
    @JvmField
    val validUntilMs: Long = validUntilMs.also { require(it >= 0) { "validUntilMs must not be negative" } }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "version" to version,
        "request" to request.toJsonMap(),
        "authority" to authority,
        "chain_id" to chainId,
        "anchor" to anchor.toJsonMap(),
        "resource" to resource.toJsonMap(),
        "acquisition" to acquisition.toJsonMap(),
        "quote_guard" to quoteGuard.toJsonMap(),
        "instructions" to instructions.map { it.toJsonMap() },
        "owner_auto_renew_instruction" to ownerAutoRenewInstruction?.toJsonMap(),
        "valid_until_ms" to validUntilMs,
    )

    companion object {
        /** Current receipt body layout. */
        const val VERSION: Int = 1
    }
}

/** Stateless signer-authenticated sponsored-onboarding receipt. */
class AccountOnboardingPlanReceiptV1(
    /** Canonical signed body. */ @JvmField val body: AccountOnboardingPlanBodyV1,
    planHash: String,
    signature: String,
) : AliasJsonValue() {
    /** Domain-separated hash of the canonical body. */
    @JvmField
    val planHash: String = planHash.also {
        require(AliasHashText.decode(it) != null) { "planHash must be a canonical 32-byte hash" }
    }

    /** Onboarding-authority signature over `planHash`, preserved byte-exactly as hex. */
    @JvmField
    val signature: String = signature.also { value ->
        require(value.isNotEmpty() && value.length % 2 == 0 && value.all { it in '0'..'9' || it.lowercaseChar() in 'a'..'f' }) {
            "signature must be non-empty even-length hexadecimal"
        }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "body" to body.toJsonMap(),
        "plan_hash" to planHash,
        "signature" to signature,
    )
}

/** Apply body containing only a previously issued stateless receipt. */
class AccountOnboardingApplyRequestV1(
    /** Exact receipt returned by the planner. */ @JvmField val receipt: AccountOnboardingPlanReceiptV1,
) : AliasJsonValue() {
    override fun toJsonMap(): Map<String, Any?> = linkedMapOf("receipt" to receipt.toJsonMap())
}

/** Sponsored onboarding apply status. */
enum class AccountOnboardingStatusV1(@JvmField val wireValue: String) {
    QUEUED("Queued"),
    REPAIRED("Repaired"),
    UNCHANGED("Unchanged"),
}

/** Typed result returned by sponsored onboarding apply. */
class AccountOnboardingResponseV1(
    accountId: String,
    alias: String,
    transactionHashHex: String?,
    /** Queue/repair/no-op result. */ @JvmField val status: AccountOnboardingStatusV1,
    /** Live disposition observed immediately before apply. */ @JvmField val disposition: AliasPlanDispositionV1,
) {
    @JvmField val accountId: String = requireCanonicalI105Address(accountId, "accountId")
    @JvmField val alias: String = requireCanonicalResponseAlias(alias)
    @JvmField val transactionHashHex: String? = transactionHashHex?.also {
        require(it.length == 64 && it.all { value -> value in '0'..'9' || value in 'a'..'f' }) {
            "transactionHashHex must contain 64 lowercase hex characters"
        }
    }

    init {
        when (status) {
            AccountOnboardingStatusV1.UNCHANGED -> require(
                transactionHashHex == null && disposition == AliasPlanDispositionV1.NO_OP,
            ) {
                "Unchanged onboarding must omit transactionHashHex and report no-op"
            }
            AccountOnboardingStatusV1.QUEUED -> require(
                transactionHashHex != null && disposition == AliasPlanDispositionV1.CREATE,
            ) {
                "Queued onboarding must carry transactionHashHex and report create"
            }
            AccountOnboardingStatusV1.REPAIRED -> require(
                transactionHashHex != null &&
                    (disposition == AliasPlanDispositionV1.REPAIR ||
                        disposition == AliasPlanDispositionV1.NO_OP),
            ) {
                "Repaired onboarding must carry transactionHashHex and report repair or no-op"
            }
        }
    }
}

/** Strict sponsored-onboarding response parser. */
object AccountOnboardingJsonParser {
    private val parser = AliasTransactionPlanJsonParser

    /** Parses the stateless receipt returned by `/v1/accounts/onboard/plan`. */
    @JvmStatic
    fun parseReceipt(payload: ByteArray): AccountOnboardingPlanReceiptV1 {
        val root = root(payload, "account onboarding receipt")
        parser.exactKeys(root, setOf("body", "plan_hash", "signature"), "account onboarding receipt")
        return AccountOnboardingPlanReceiptV1(
            parseBody(parser.objectField(root, "body", "account onboarding receipt.body")),
            parser.stringField(root, "plan_hash", "account onboarding receipt.plan_hash"),
            parser.stringField(root, "signature", "account onboarding receipt.signature"),
        )
    }

    /** Parses queued, repaired, or unchanged apply results. */
    @JvmStatic
    fun parseResponse(payload: ByteArray): AccountOnboardingResponseV1 {
        val root = root(payload, "account onboarding response")
        val allowed = setOf("account_id", "alias", "tx_hash_hex", "status", "disposition")
        check(root.keys.all { it in allowed } &&
            listOf("account_id", "alias", "status", "disposition").all { root.containsKey(it) }
        ) { "account onboarding response has invalid fields" }
        val status = when (parser.stringField(root, "status", "account onboarding response.status")) {
            "Queued" -> AccountOnboardingStatusV1.QUEUED
            "Repaired" -> AccountOnboardingStatusV1.REPAIRED
            "Unchanged" -> AccountOnboardingStatusV1.UNCHANGED
            else -> error("account onboarding response.status is unsupported")
        }
        val disposition = when (
            parser.parseTaggedVariant(
                parser.objectField(root, "disposition", "account onboarding response.disposition"),
                "kind",
                "account onboarding response.disposition",
            )
        ) {
            "no_op" -> AliasPlanDispositionV1.NO_OP
            "repair" -> AliasPlanDispositionV1.REPAIR
            "create" -> AliasPlanDispositionV1.CREATE
            "conflict" -> AliasPlanDispositionV1.CONFLICT
            else -> error("account onboarding response.disposition is unsupported")
        }
        return AccountOnboardingResponseV1(
            parser.stringField(root, "account_id", "account onboarding response.account_id"),
            parser.stringField(root, "alias", "account onboarding response.alias"),
            parser.optionalString(root, "tx_hash_hex", "account onboarding response.tx_hash_hex"),
            status,
            disposition,
        )
    }

    /** Parses authenticated onboarding readiness diagnostics. */
    @JvmStatic
    fun parseReadiness(payload: ByteArray): AliasSetupReportV1 {
        val root = root(payload, "account onboarding readiness")
        parser.exactKeys(root, setOf("version", "status", "diagnostics"), "account onboarding readiness")
        val version = parser.intField(root, "version", "account onboarding readiness.version")
        check(version == AliasSetupReportV1.VERSION) { "account onboarding readiness.version is unsupported" }
        val status = when (
            parser.parseTaggedVariant(
                parser.objectField(root, "status", "account onboarding readiness.status"),
                "status",
                "account onboarding readiness.status",
            )
        ) {
            "ready" -> AliasSetupStatusV1.READY
            "pending" -> AliasSetupStatusV1.PENDING
            "blocked" -> AliasSetupStatusV1.BLOCKED
            else -> error("account onboarding readiness.status is unsupported")
        }
        val diagnostics = parser.arrayField(root, "diagnostics", "account onboarding readiness.diagnostics")
            .mapIndexed { index, value ->
                parser.parseDiagnostic(
                    parser.objectValue(value, "account onboarding readiness.diagnostics[$index]"),
                    "account onboarding readiness.diagnostics[$index]",
                )
            }
        return AliasSetupReportV1(status, diagnostics)
    }

    private fun parseBody(root: Map<String, Any?>): AccountOnboardingPlanBodyV1 {
        parser.exactKeys(
            root,
            setOf(
                "version", "request", "authority", "chain_id", "anchor", "resource",
                "acquisition", "quote_guard", "instructions", "owner_auto_renew_instruction",
                "valid_until_ms",
            ),
            "account onboarding receipt.body",
        )
        val requestRoot = parser.objectField(root, "request", "body.request")
        parser.exactKeys(requestRoot, setOf("version", "alias", "account_id", "permissions"), "body.request")
        val permissions = parser.arrayField(requestRoot, "permissions", "body.request.permissions")
            .mapIndexed { index, value ->
                value as? String ?: error("body.request.permissions[$index] must be a string")
            }
        val acquisition = parser.objectField(root, "acquisition", "body.acquisition")
        parser.exactKeys(acquisition, setOf("term_years", "pricing_class_hint"), "body.acquisition")
        return AccountOnboardingPlanBodyV1(
            parser.intField(root, "version", "body.version"),
            AccountOnboardingPlanRequestV1(
                parser.stringField(requestRoot, "alias", "body.request.alias"),
                parser.stringField(requestRoot, "account_id", "body.request.account_id"),
                permissions,
                parser.intField(requestRoot, "version", "body.request.version"),
            ),
            parser.stringField(root, "authority", "body.authority"),
            parser.stringField(root, "chain_id", "body.chain_id"),
            parser.parseAnchor(parser.objectField(root, "anchor", "body.anchor")),
            parser.parseResource(parser.objectField(root, "resource", "body.resource"), "body.resource"),
            AliasLeaseAcquisitionV1(
                parser.intField(acquisition, "term_years", "body.acquisition.term_years"),
                if (acquisition["pricing_class_hint"] == null) null else
                    parser.intField(acquisition, "pricing_class_hint", "body.acquisition.pricing_class_hint"),
            ),
            parser.parseGuard(parser.objectField(root, "quote_guard", "body.quote_guard"), "body.quote_guard"),
            parser.arrayField(root, "instructions", "body.instructions").mapIndexed { index, value ->
                parser.parseFrame(parser.objectValue(value, "body.instructions[$index]"), "body.instructions[$index]")
            },
            parser.optionalObject(root, "owner_auto_renew_instruction", "body.owner_auto_renew_instruction")
                ?.let { parser.parseFrame(it, "body.owner_auto_renew_instruction") },
            parser.longField(root, "valid_until_ms", "body.valid_until_ms"),
        )
    }

    private fun root(payload: ByteArray, path: String): Map<String, Any?> {
        require(payload.isNotEmpty()) { "$path returned an empty payload" }
        return parser.objectValue(JsonParser.parse(String(payload, StandardCharsets.UTF_8)), path)
    }
}

/** Validates a raw onboarding token before placing it in an HTTP header. */
internal fun requireOnboardingCredential(value: String): String {
    require(value.length in 32..256 && value.all { it in '!'..'~' }) {
        "onboarding token must contain 32..256 printable non-whitespace ASCII bytes"
    }
    return value
}

private fun requireOnboardingToken(value: String, field: String): String {
    require(value.isNotBlank() && value == value.trim() && value.none { it.isWhitespace() || it.isISOControl() }) {
        "$field must be non-blank without whitespace or controls"
    }
    return value
}

private fun requireCanonicalResponseAlias(value: String): String {
    val canonical = AccountAliasName.parse(value).canonicalText()
    require(canonical == value) { "alias must be canonical" }
    return canonical
}
