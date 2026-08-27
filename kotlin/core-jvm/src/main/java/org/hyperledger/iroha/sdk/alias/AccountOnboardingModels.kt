package org.hyperledger.iroha.sdk.alias

import java.io.ByteArrayOutputStream
import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.client.FeePaymentJson
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

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
    /** Exact genesis-derived network identity. */ @JvmField val networkId: NetworkId,
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
        "network_id" to networkId.literal,
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

/** Exact public-reset mutation identity authenticated by every prepared result. */
class TairaPublicResetMutationBindingV1(
    schema: String = SCHEMA,
    authorizationSha256: String,
    authorizationNonce: String,
    kind: String,
    phase: String,
    idempotencyKey: String,
    executionExpiresAtUnixMs: Long,
) : AliasJsonValue() {
    @JvmField val schema: String = schema.also { require(it == SCHEMA) { "unsupported binding schema" } }
    @JvmField val authorizationSha256: String = requireLowerHex32(authorizationSha256, "authorizationSha256")
    @JvmField val authorizationNonce: String = authorizationNonce.also {
        require(it.length == 32 && it.all(::isBindingTokenChar)) {
            "authorizationNonce must contain exactly 32 lowercase token characters"
        }
    }
    @JvmField val kind: String = kind.also { require(it == ONBOARDING || it == FAUCET) { "unsupported binding kind" } }
    @JvmField val phase: String = phase.also {
        require(it.length in 1..128 && it.all(::isBindingTokenChar)) {
            "phase must contain 1..128 lowercase token characters"
        }
    }
    @JvmField val idempotencyKey: String = requireLowerHex32(idempotencyKey, "idempotencyKey")
    @JvmField val executionExpiresAtUnixMs: Long = executionExpiresAtUnixMs.also {
        require(it >= 0) { "executionExpiresAtUnixMs must not be negative" }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema" to schema,
        "authorization_sha256" to authorizationSha256,
        "authorization_nonce" to authorizationNonce,
        "kind" to kind,
        "phase" to phase,
        "idempotency_key" to idempotencyKey,
        "execution_expires_at_unix_ms" to executionExpiresAtUnixMs,
    )

    companion object {
        const val SCHEMA: String = "iroha.taira.public-reset.mutation-binding.v1"
        const val ONBOARDING: String = "onboarding"
        const val FAUCET: String = "faucet"
    }
}

/** Non-mutating prepare body consuming one signed onboarding plan receipt. */
class AccountOnboardingPrepareRequestV1(
    /** Exact reset binding. */ @JvmField val binding: TairaPublicResetMutationBindingV1,
    /** Exact signed plan receipt. */ @JvmField val receipt: AccountOnboardingPlanReceiptV1,
    /** Exact payer, sponsor revision, and gas bound Torii may quote. */
    @JvmField val feePayment: FeePaymentIntent,
    schema: String = SCHEMA,
) : AliasJsonValue() {
    @JvmField val schema: String = schema.also { require(it == SCHEMA) { "unsupported onboarding prepare schema" } }

    init {
        require(binding.kind == TairaPublicResetMutationBindingV1.ONBOARDING) {
            "onboarding prepare requires an onboarding binding"
        }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema" to schema,
        "binding" to binding.toJsonMap(),
        "receipt" to receipt.toJsonMap(),
        "fee_payment" to feePayment.toJsonMap(),
    )

    companion object {
        const val SCHEMA: String = "iroha.accounts.onboard.prepare.v1"
    }
}

/** Canonical account-faucet claim prepared into one exact transaction. */
class AccountFaucetClaimV1(
    accountId: String,
    powAnchorHeight: BigInteger,
    powNonceHex: String,
) : AliasJsonValue() {
    /** Canonical domainless destination account. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Positive committed block height anchoring proof-of-work. */
    @JvmField
    val powAnchorHeight: BigInteger = requireU64(powAnchorHeight, "powAnchorHeight").also {
        require(it.signum() > 0) { "powAnchorHeight must be positive" }
    }

    /** Non-empty, even-length canonical lowercase hexadecimal nonce. */
    @JvmField
    val powNonceHex: String = requireLowerHex(powNonceHex, "powNonceHex").also {
        require(it.length <= 64) { "powNonceHex must not exceed 32 bytes" }
    }

    /** Domain-separated semantic hash committed by a prepared faucet transaction. */
    fun semanticHashHex(): String {
        val encoded = ByteArrayOutputStream()
        writeNoritoField(encoded, encodeNoritoString(accountId))
        writeNoritoField(encoded, encodeU64LittleEndian(powAnchorHeight))
        writeNoritoField(encoded, encodeNoritoString(powNonceHex))
        return lowerHex(IrohaHash.prehash(FAUCET_CLAIM_HASH_DOMAIN + encoded.toByteArray()))
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "account_id" to accountId,
        "pow_anchor_height" to powAnchorHeight,
        "pow_nonce_hex" to powNonceHex,
    )

    companion object {
        private val FAUCET_CLAIM_HASH_DOMAIN =
            "iroha:accounts:faucet:claim:v1\u0000".toByteArray(StandardCharsets.UTF_8)

        private fun encodeNoritoString(value: String): ByteArray = ByteArrayOutputStream().also { output ->
            val bytes = value.toByteArray(StandardCharsets.UTF_8)
            writeCompactLength(output, bytes.size.toLong())
            output.write(bytes)
        }.toByteArray()

        private fun encodeU64LittleEndian(value: BigInteger): ByteArray = ByteArray(8) { index ->
            value.shiftRight(index * 8).and(BigInteger.valueOf(0xffL)).toByte()
        }

        private fun writeNoritoField(output: ByteArrayOutputStream, value: ByteArray) {
            writeCompactLength(output, value.size.toLong())
            output.write(value)
        }

        private fun writeCompactLength(output: ByteArrayOutputStream, raw: Long) {
            var value = raw
            do {
                var next = (value and 0x7fL).toInt()
                value = value ushr 7
                if (value != 0L) next = next or 0x80
                output.write(next)
            } while (value != 0L)
        }

        private fun lowerHex(bytes: ByteArray): String {
            val digits = "0123456789abcdef"
            return buildString(bytes.size * 2) {
                bytes.forEach { byte ->
                    val value = byte.toInt() and 0xff
                    append(digits[value ushr 4])
                    append(digits[value and 0x0f])
                }
            }
        }
    }
}

/** Non-mutating prepare body consuming one exact faucet claim. */
class AccountFaucetPrepareRequestV1(
    /** Exact reset binding. */ @JvmField val binding: TairaPublicResetMutationBindingV1,
    /** Exact solved faucet claim. */ @JvmField val claim: AccountFaucetClaimV1,
    /** Exact payer, sponsor revision, and gas bound Torii may quote. */
    @JvmField val feePayment: FeePaymentIntent,
    schema: String = SCHEMA,
) : AliasJsonValue() {
    /** Exact immutable request schema. */
    @JvmField
    val schema: String = schema.also { require(it == SCHEMA) { "unsupported faucet prepare schema" } }

    init {
        require(binding.kind == TairaPublicResetMutationBindingV1.FAUCET) {
            "faucet prepare requires a faucet binding"
        }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema" to schema,
        "binding" to binding.toJsonMap(),
        "claim" to claim.toJsonMap(),
        "fee_payment" to feePayment.toJsonMap(),
    )

    companion object {
        /** Current and only first-release faucet prepare schema. */
        const val SCHEMA: String = "iroha.accounts.faucet.prepare.v1"
    }
}

/** Independently trusted first-release faucet identity and exact issuance policy. */
class AccountFaucetPolicyV1(
    faucetAuthority: String,
    assetDefinitionId: String,
    /** Exact positive quantity one accepted claim may receive. */ @JvmField val amount: KotodamaQuantity,
) {
    /** Trusted single-signatory faucet authority. */
    @JvmField
    val faucetAuthority: String = requireCanonicalI105Address(faucetAuthority, "faucetAuthority").also {
        val parsed = try {
            AccountAddress.parseEncoded(it, null)
        } catch (error: AccountAddressException) {
            throw IllegalArgumentException("faucetAuthority must be a canonical single-signatory account", error)
        }
        require(parsed.singleKeyPayloadIgnoringCurveSupport() != null) {
            "faucetAuthority must be a single-signatory account"
        }
    }

    /** Exact canonical asset definition the faucet may issue. */
    @JvmField
    val assetDefinitionId: String = assetDefinitionId.also {
        require(AssetDefinitionIdEncoder.isCanonicalAddress(it)) {
            "assetDefinitionId must be a canonical asset-definition address"
        }
    }

    init {
        require(amount.mantissa.signum() > 0) { "faucet policy amount must be positive" }
    }
}

/** Authenticated exact faucet transaction prepared by Torii. */
class AccountFaucetPreparedTransactionV1(
    /** Exact reset binding. */ @JvmField val binding: TairaPublicResetMutationBindingV1,
    /** Exact solved claim consumed during preparation. */ @JvmField val claim: AccountFaucetClaimV1,
    semanticHashHex: String,
    accountId: String,
    assetDefinitionId: String,
    assetId: String,
    /** Exact issuance quantity. */ @JvmField val amount: KotodamaQuantity,
    transactionHashHex: String,
    signedTransactionWireHex: String,
    signedTransactionWireSha256: String,
    /** Exact signature-bound fee intent. */ @JvmField val feePayment: FeePaymentIntent,
    serverSignature: String,
    schema: String = SCHEMA,
    operation: String = OPERATION,
) : AliasJsonValue() {
    /** Exact immutable prepared-envelope schema. */
    @JvmField
    val schema: String = schema.also { require(it == SCHEMA) { "unsupported prepared transaction schema" } }

    /** Exact immutable operation label. */
    @JvmField
    val operation: String = operation.also { require(it == OPERATION) { "prepared faucet operation must be faucet" } }

    /** Domain-separated claim hash. */
    @JvmField
    val semanticHashHex: String = requireLowerHex32(semanticHashHex, "semanticHashHex")

    /** Canonical destination account. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Canonical issued asset definition. */
    @JvmField
    val assetDefinitionId: String = assetDefinitionId.also {
        require(AssetDefinitionIdEncoder.isCanonicalAddress(it)) {
            "assetDefinitionId must be a canonical asset-definition address"
        }
    }

    /** Canonical destination asset balance. */
    @JvmField
    val assetId: String = assetId

    /** Hash of the exact signed transaction. */
    @JvmField
    val transactionHashHex: String = requireTransactionHash(transactionHashHex, "transactionHashHex")

    /** Canonical fixed-V1 signed transaction wire as lowercase hexadecimal. */
    @JvmField
    val signedTransactionWireHex: String = requireLowerHex(signedTransactionWireHex, "signedTransactionWireHex")

    /** SHA-256 of the exact canonical transaction wire. */
    @JvmField
    val signedTransactionWireSha256: String = requireLowerHex32(
        signedTransactionWireSha256,
        "signedTransactionWireSha256",
    )

    /** Faucet-authority signature authenticating every preceding prepared field. */
    @JvmField
    val serverSignature: String = requireHex(serverSignature, "serverSignature")

    init {
        require(binding.kind == TairaPublicResetMutationBindingV1.FAUCET) {
            "prepared faucet requires a faucet binding"
        }
        require(claim.accountId == this.accountId) {
            "prepared faucet account must equal the claim account"
        }
        require(assetId == "$assetDefinitionId#${this.accountId}") {
            "prepared faucet asset must be the exact destination balance"
        }
        require(amount.mantissa.signum() > 0) { "prepared faucet amount must be positive" }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema" to schema,
        "binding" to binding.toJsonMap(),
        "operation" to operation,
        "claim" to claim.toJsonMap(),
        "semantic_hash_hex" to semanticHashHex,
        "account_id" to accountId,
        "asset_definition_id" to assetDefinitionId,
        "asset_id" to assetId,
        "amount" to amount.toString(),
        "transaction_hash_hex" to transactionHashHex,
        "signed_transaction_wire_hex" to signedTransactionWireHex,
        "signed_transaction_wire_sha256" to signedTransactionWireSha256,
        "fee_payment" to feePayment.toJsonMap(),
        "server_signature" to serverSignature,
    )

    companion object {
        /** Current and only first-release prepared-envelope schema. */
        const val SCHEMA: String = "iroha.taira.prepared-transaction.v1"

        /** Exact faucet operation label. */
        const val OPERATION: String = "faucet"
    }
}

/** Closed result of non-mutating onboarding preparation. */
sealed interface AccountOnboardingPrepareResponseV1

/** Authenticated exact transaction returned by onboarding preparation. */
class AccountOnboardingPreparedTransactionV1(
    /** Exact reset binding. */ @JvmField val binding: TairaPublicResetMutationBindingV1,
    /** Exact plan receipt consumed at prepare. */ @JvmField val receipt: AccountOnboardingPlanReceiptV1,
    semanticHashHex: String,
    accountId: String,
    alias: String,
    /** Live disposition used to build the transaction. */ @JvmField val disposition: AliasPlanDispositionV1,
    transactionHashHex: String,
    signedTransactionWireHex: String,
    signedTransactionWireSha256: String,
    /** Exact signature-bound fee intent. */ @JvmField val feePayment: FeePaymentIntent,
    serverSignature: String,
    schema: String = SCHEMA,
    operation: String = OPERATION,
) : AliasJsonValue(), AccountOnboardingPrepareResponseV1 {
    @JvmField val schema: String = schema.also { require(it == SCHEMA) { "unsupported prepared transaction schema" } }
    @JvmField val operation: String = operation.also { require(it == OPERATION) { "prepared onboarding operation must be onboarding" } }
    @JvmField val semanticHashHex: String = requireLowerHex32(semanticHashHex, "semanticHashHex")
    @JvmField val accountId: String = requireCanonicalI105Address(accountId, "accountId")
    @JvmField val alias: String = requireCanonicalResponseAlias(alias)
    @JvmField val transactionHashHex: String = requireTransactionHash(transactionHashHex, "transactionHashHex")
    @JvmField val signedTransactionWireHex: String = requireLowerHex(signedTransactionWireHex, "signedTransactionWireHex")
    @JvmField val signedTransactionWireSha256: String = requireLowerHex32(
        signedTransactionWireSha256,
        "signedTransactionWireSha256",
    )
    @JvmField val serverSignature: String = requireHex(serverSignature, "serverSignature")

    init {
        require(binding.kind == TairaPublicResetMutationBindingV1.ONBOARDING) {
            "prepared onboarding requires an onboarding binding"
        }
        require(disposition != AliasPlanDispositionV1.CONFLICT && disposition != AliasPlanDispositionV1.NO_OP) {
            "prepared onboarding requires create or repair disposition"
        }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema" to schema,
        "binding" to binding.toJsonMap(),
        "operation" to operation,
        "receipt" to receipt.toJsonMap(),
        "semantic_hash_hex" to semanticHashHex,
        "account_id" to accountId,
        "alias" to alias,
        "disposition" to disposition.toJsonMap(),
        "transaction_hash_hex" to transactionHashHex,
        "signed_transaction_wire_hex" to signedTransactionWireHex,
        "signed_transaction_wire_sha256" to signedTransactionWireSha256,
        "fee_payment" to feePayment.toJsonMap(),
        "server_signature" to serverSignature,
    )

    companion object {
        const val SCHEMA: String = "iroha.taira.prepared-transaction.v1"
        const val OPERATION: String = "onboarding"
    }
}

/** Authenticated nonterminal result requiring one fresh atomic account-and-alias observation. */
class AccountOnboardingProofRequiredPrepareResponseV1(
    /** Exact reset binding. */ @JvmField val binding: TairaPublicResetMutationBindingV1,
    semanticHashHex: String,
    accountId: String,
    alias: String,
    /** Observed no-op disposition; this does not prove current state. */ @JvmField val disposition: AliasPlanDispositionV1,
    serverSignature: String,
    schema: String = SCHEMA,
    operation: String = AccountOnboardingPreparedTransactionV1.OPERATION,
    outcome: String = OUTCOME,
    proofKind: String = PROOF_KIND,
) : AliasJsonValue(), AccountOnboardingPrepareResponseV1 {
    @JvmField val schema: String = schema.also { require(it == SCHEMA) { "unsupported onboarding proof-required schema" } }
    @JvmField val operation: String = operation.also {
        require(it == AccountOnboardingPreparedTransactionV1.OPERATION) { "proof-required operation must be onboarding" }
    }
    @JvmField val outcome: String = outcome.also { require(it == OUTCOME) { "outcome must be ProofRequired" } }
    @JvmField val proofKind: String = proofKind.also {
        require(it == PROOF_KIND) { "proofKind must require current account and alias state" }
    }
    @JvmField val semanticHashHex: String = requireLowerHex32(semanticHashHex, "semanticHashHex")
    @JvmField val accountId: String = requireCanonicalI105Address(accountId, "accountId")
    @JvmField val alias: String = requireCanonicalResponseAlias(alias)
    @JvmField val serverSignature: String = requireHex(serverSignature, "serverSignature")

    init {
        require(binding.kind == TairaPublicResetMutationBindingV1.ONBOARDING) {
            "proof-required onboarding requires an onboarding binding"
        }
        require(disposition == AliasPlanDispositionV1.NO_OP) {
            "proof-required onboarding must report no-op"
        }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema" to schema,
        "binding" to binding.toJsonMap(),
        "operation" to operation,
        "outcome" to outcome,
        "proof_kind" to proofKind,
        "semantic_hash_hex" to semanticHashHex,
        "account_id" to accountId,
        "alias" to alias,
        "disposition" to disposition.toJsonMap(),
        "server_signature" to serverSignature,
    )

    companion object {
        const val SCHEMA: String = "iroha.accounts.onboard.prepare-proof-required.v1"
        const val OUTCOME: String = "ProofRequired"
        const val PROOF_KIND: String = "account_alias_current_state"
    }
}

/** Typed exact canonical committed-block hash for an atomic onboarding observation. */
class AccountOnboardingBlockHashV1(literal: String) {
    /** Canonical checksummed Iroha hash literal. */
    @JvmField
    val literal: String = NetworkId.parse(literal).literal

    override fun equals(other: Any?): Boolean =
        this === other || other is AccountOnboardingBlockHashV1 && literal == other.literal

    override fun hashCode(): Int = literal.hashCode()

    override fun toString(): String = literal
}

/** Exact closed request for one atomic account-onboarding state observation. */
class AccountOnboardingCurrentStateRequestV1(
    accountId: String,
    alias: String,
) : AliasJsonValue() {
    /** Request layout version. */
    @JvmField
    val version: Int = VERSION

    /** Exact canonical domainless account whose existence is observed. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Exact canonical alias whose active target is observed. */
    @JvmField
    val alias: String = requireCanonicalResponseAlias(alias)

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "version" to version,
        "account_id" to accountId,
        "alias" to alias,
    )

    companion object {
        /** Current and only first-release layout. */
        const val VERSION: Int = 1
    }
}

/** One internally consistent atomic account-onboarding state observation. */
class AccountOnboardingCurrentStateResponseV1(
    version: Int,
    /** Exact network that owns the observed state. */ @JvmField val networkId: NetworkId,
    accountId: String,
    alias: String,
    /** Whether the exact requested account exists. */ @JvmField val accountExists: Boolean,
    aliasTargetAccountId: String?,
    observedBlockHeight: BigInteger,
    /** Typed block hash anchoring the observation. */
    @JvmField val observedBlockHash: AccountOnboardingBlockHashV1,
) : AliasJsonValue() {
    /** Response layout version. */
    @JvmField
    val version: Int = version.also { require(it == VERSION) { "version must be $VERSION" } }

    /** Exact echoed account id. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Exact echoed account alias. */
    @JvmField
    val alias: String = requireCanonicalResponseAlias(alias)

    /** Active alias target, or null when no active target exists. */
    @JvmField
    val aliasTargetAccountId: String? = aliasTargetAccountId?.let {
        requireCanonicalI105Address(it, "aliasTargetAccountId")
    }

    /** Positive unsigned committed block height anchoring the observation. */
    @JvmField
    val observedBlockHeight: BigInteger = observedBlockHeight.also {
        require(it.signum() > 0 && it.bitLength() <= 64) {
            "observedBlockHeight must be a positive unsigned 64-bit integer"
        }
    }

    init {
        val target = this.aliasTargetAccountId
        require(
            accountExists || target == null ||
                !AccountOnboardingReceiptVerifier.sameAccountIdentity(accountId, target),
        ) { "alias target cannot equal an account reported absent in the same snapshot" }
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "version" to version,
        "network_id" to networkId.literal,
        "account_id" to accountId,
        "alias" to alias,
        "account_exists" to accountExists,
        "alias_target_account_id" to aliasTargetAccountId,
        "observed_block_height" to observedBlockHeight,
        "observed_block_hash" to observedBlockHash.literal,
    )

    /** Validates all trust pins and returns the exact closed current-state classification. */
    fun classify(
        request: AccountOnboardingCurrentStateRequestV1,
        expectedNetworkId: NetworkId,
    ): AccountOnboardingCurrentStateV1 {
        require(networkId == expectedNetworkId) {
            "account onboarding current-state response changed networkId"
        }
        require(accountId == request.accountId && alias == request.alias) {
            "account onboarding current-state response did not echo the exact request"
        }
        require(accountExists) {
            "account onboarding current-state response reports the expected account absent"
        }
        val target = aliasTargetAccountId
        val outcome = when {
            target == null -> AccountOnboardingCurrentStateV1.Outcome.ALIAS_ABSENT
            AccountOnboardingReceiptVerifier.sameAccountIdentity(
                request.accountId,
                target,
            ) -> AccountOnboardingCurrentStateV1.Outcome.APPLIED
            else -> AccountOnboardingCurrentStateV1.Outcome.ALIAS_CONFLICT
        }
        return AccountOnboardingCurrentStateV1(outcome, observedBlockHeight, observedBlockHash)
    }

    companion object {
        /** Current and only first-release layout. */
        const val VERSION: Int = 1
    }
}

/** Closed classification derived from one committed state snapshot. */
class AccountOnboardingCurrentStateV1(
    /** Exact state classification. */ @JvmField val outcome: Outcome,
    /** Nonzero committed height. */ @JvmField val blockHeight: BigInteger,
    /** Typed committed block hash. */ @JvmField val blockHash: AccountOnboardingBlockHashV1,
) {
    /** First-release atomic onboarding-state outcomes. */
    enum class Outcome {
        APPLIED,
        ALIAS_ABSENT,
        ALIAS_CONFLICT,
    }

    init {
        require(blockHeight.signum() > 0 && blockHeight.bitLength() <= 64) {
            "blockHeight must be a positive unsigned 64-bit integer"
        }
    }

    override fun equals(other: Any?): Boolean =
        this === other || other is AccountOnboardingCurrentStateV1 &&
            outcome == other.outcome && blockHeight == other.blockHeight && blockHash == other.blockHash

    override fun hashCode(): Int = 31 * (31 * outcome.hashCode() + blockHeight.hashCode()) + blockHash.hashCode()
}

/** Canonical terminal or nonterminal state for an exact prepared hash. */
enum class PreparedTransactionOutcomeV1(@JvmField val wireValue: String) {
    APPLIED("Applied"),
    PENDING("Pending"),
    REJECTED("Rejected"),
}

/** Response bound to one exact submitted prepared transaction. */
class PreparedTransactionSubmitResponseV1(
    /** Exact binding copied from the submitted envelope. */ @JvmField val binding: TairaPublicResetMutationBindingV1,
    operation: String,
    transactionHashHex: String,
    /** Canonical reconciliation result. */ @JvmField val outcome: PreparedTransactionOutcomeV1,
    schema: String = SCHEMA,
) : AliasJsonValue() {
    @JvmField val schema: String = schema.also { require(it == SCHEMA) { "unsupported prepared submit schema" } }
    @JvmField val operation: String = operation.also {
        require(it == AccountOnboardingPreparedTransactionV1.OPERATION || it == TairaPublicResetMutationBindingV1.FAUCET) {
            "unsupported prepared submit operation"
        }
    }
    @JvmField val transactionHashHex: String = requireTransactionHash(transactionHashHex, "transactionHashHex")

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "schema" to schema,
        "binding" to binding.toJsonMap(),
        "operation" to operation,
        "transaction_hash_hex" to transactionHashHex,
        "outcome" to outcome.wireValue,
    )

    companion object {
        const val SCHEMA: String = "iroha.taira.prepared-transaction-submit.v1"
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

    /** Parses the closed prepared-or-authenticated-proof-required response. */
    @JvmStatic
    fun parsePrepareResponse(payload: ByteArray): AccountOnboardingPrepareResponseV1 {
        val root = root(payload, "account onboarding prepare response")
        return when (parser.stringField(root, "schema", "account onboarding prepare response.schema")) {
            AccountOnboardingPreparedTransactionV1.SCHEMA -> parsePrepared(root)
            AccountOnboardingProofRequiredPrepareResponseV1.SCHEMA -> parseProofRequired(root)
            else -> error("account onboarding prepare response.schema is unsupported")
        }
    }

    /** Parses one exact authenticated faucet prepared transaction. */
    @JvmStatic
    fun parseFaucetPrepareResponse(payload: ByteArray): AccountFaucetPreparedTransactionV1 {
        val root = root(payload, "account faucet prepare response")
        return parseFaucetPrepared(root)
    }

    /** Parses one closed atomic account-onboarding current-state response. */
    @JvmStatic
    fun parseCurrentStateResponse(payload: ByteArray): AccountOnboardingCurrentStateResponseV1 {
        val path = "account onboarding current-state response"
        val root = root(payload, path)
        parser.exactKeys(
            root,
            setOf(
                "version", "network_id", "account_id", "alias", "account_exists",
                "alias_target_account_id", "observed_block_height", "observed_block_hash",
            ),
            path,
        )
        val accountExists = root["account_exists"]
        check(accountExists is Boolean) { "$path.account_exists must be a boolean" }
        val aliasTarget = root["alias_target_account_id"]?.let {
            check(it is String) { "$path.alias_target_account_id must be a string or null" }
            it
        }
        return AccountOnboardingCurrentStateResponseV1(
            parser.intField(root, "version", "$path.version"),
            NetworkId.parse(parser.stringField(root, "network_id", "$path.network_id")),
            parser.stringField(root, "account_id", "$path.account_id"),
            parser.stringField(root, "alias", "$path.alias"),
            accountExists,
            aliasTarget,
            positiveU64(root["observed_block_height"], "$path.observed_block_height"),
            AccountOnboardingBlockHashV1(
                parser.stringField(root, "observed_block_hash", "$path.observed_block_hash"),
            ),
        )
    }

    /** Parses a submit response bound to one exact prepared transaction. */
    @JvmStatic
    fun parseSubmitResponse(payload: ByteArray): PreparedTransactionSubmitResponseV1 {
        val root = root(payload, "prepared transaction submit response")
        parser.exactKeys(
            root,
            setOf("schema", "binding", "operation", "transaction_hash_hex", "outcome"),
            "prepared transaction submit response",
        )
        val outcome = when (parser.stringField(root, "outcome", "prepared transaction submit response.outcome")) {
            "Applied" -> PreparedTransactionOutcomeV1.APPLIED
            "Pending" -> PreparedTransactionOutcomeV1.PENDING
            "Rejected" -> PreparedTransactionOutcomeV1.REJECTED
            else -> error("prepared transaction submit response.outcome is unsupported")
        }
        return PreparedTransactionSubmitResponseV1(
            parseBinding(parser.objectField(root, "binding", "prepared transaction submit response.binding")),
            parser.stringField(root, "operation", "prepared transaction submit response.operation"),
            parser.stringField(root, "transaction_hash_hex", "prepared transaction submit response.transaction_hash_hex"),
            outcome,
            parser.stringField(root, "schema", "prepared transaction submit response.schema"),
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
                "version", "request", "authority", "network_id", "anchor", "resource",
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
            NetworkId.parse(parser.stringField(root, "network_id", "body.network_id")),
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

    private fun parsePrepared(root: Map<String, Any?>): AccountOnboardingPreparedTransactionV1 {
        val path = "prepared onboarding transaction"
        parser.exactKeys(
            root,
            setOf(
                "schema", "binding", "operation", "receipt", "semantic_hash_hex", "account_id",
                "alias", "disposition", "transaction_hash_hex", "signed_transaction_wire_hex",
                "signed_transaction_wire_sha256", "fee_payment", "server_signature",
            ),
            path,
        )
        return AccountOnboardingPreparedTransactionV1(
            parseBinding(parser.objectField(root, "binding", "$path.binding")),
            parseReceiptValue(parser.objectField(root, "receipt", "$path.receipt")),
            parser.stringField(root, "semantic_hash_hex", "$path.semantic_hash_hex"),
            parser.stringField(root, "account_id", "$path.account_id"),
            parser.stringField(root, "alias", "$path.alias"),
            parseDisposition(parser.objectField(root, "disposition", "$path.disposition"), "$path.disposition"),
            parser.stringField(root, "transaction_hash_hex", "$path.transaction_hash_hex"),
            parser.stringField(root, "signed_transaction_wire_hex", "$path.signed_transaction_wire_hex"),
            parser.stringField(root, "signed_transaction_wire_sha256", "$path.signed_transaction_wire_sha256"),
            FeePaymentJson.parse(root["fee_payment"], "$path.fee_payment"),
            parser.stringField(root, "server_signature", "$path.server_signature"),
            parser.stringField(root, "schema", "$path.schema"),
            parser.stringField(root, "operation", "$path.operation"),
        )
    }

    private fun parseFaucetPrepared(root: Map<String, Any?>): AccountFaucetPreparedTransactionV1 {
        val path = "prepared faucet transaction"
        parser.exactKeys(
            root,
            setOf(
                "schema", "binding", "operation", "claim", "semantic_hash_hex", "account_id",
                "asset_definition_id", "asset_id", "amount", "transaction_hash_hex",
                "signed_transaction_wire_hex", "signed_transaction_wire_sha256", "fee_payment",
                "server_signature",
            ),
            path,
        )
        val claimRoot = parser.objectField(root, "claim", "$path.claim")
        parser.exactKeys(
            claimRoot,
            setOf("account_id", "pow_anchor_height", "pow_nonce_hex"),
            "$path.claim",
        )
        return AccountFaucetPreparedTransactionV1(
            parseBinding(parser.objectField(root, "binding", "$path.binding")),
            AccountFaucetClaimV1(
                parser.stringField(claimRoot, "account_id", "$path.claim.account_id"),
                positiveU64(claimRoot["pow_anchor_height"], "$path.claim.pow_anchor_height"),
                parser.stringField(claimRoot, "pow_nonce_hex", "$path.claim.pow_nonce_hex"),
            ),
            parser.stringField(root, "semantic_hash_hex", "$path.semantic_hash_hex"),
            parser.stringField(root, "account_id", "$path.account_id"),
            parser.stringField(root, "asset_definition_id", "$path.asset_definition_id"),
            parser.stringField(root, "asset_id", "$path.asset_id"),
            KotodamaQuantity.parseCanonical(parser.stringField(root, "amount", "$path.amount")),
            parser.stringField(root, "transaction_hash_hex", "$path.transaction_hash_hex"),
            parser.stringField(root, "signed_transaction_wire_hex", "$path.signed_transaction_wire_hex"),
            parser.stringField(root, "signed_transaction_wire_sha256", "$path.signed_transaction_wire_sha256"),
            FeePaymentJson.parse(root["fee_payment"], "$path.fee_payment"),
            parser.stringField(root, "server_signature", "$path.server_signature"),
            parser.stringField(root, "schema", "$path.schema"),
            parser.stringField(root, "operation", "$path.operation"),
        )
    }

    private fun parseProofRequired(root: Map<String, Any?>): AccountOnboardingProofRequiredPrepareResponseV1 {
        val path = "proof-required onboarding prepare response"
        parser.exactKeys(
            root,
            setOf(
                "schema", "binding", "operation", "outcome", "proof_kind", "semantic_hash_hex", "account_id",
                "alias", "disposition", "server_signature",
            ),
            path,
        )
        return AccountOnboardingProofRequiredPrepareResponseV1(
            parseBinding(parser.objectField(root, "binding", "$path.binding")),
            parser.stringField(root, "semantic_hash_hex", "$path.semantic_hash_hex"),
            parser.stringField(root, "account_id", "$path.account_id"),
            parser.stringField(root, "alias", "$path.alias"),
            parseDisposition(parser.objectField(root, "disposition", "$path.disposition"), "$path.disposition"),
            parser.stringField(root, "server_signature", "$path.server_signature"),
            parser.stringField(root, "schema", "$path.schema"),
            parser.stringField(root, "operation", "$path.operation"),
            parser.stringField(root, "outcome", "$path.outcome"),
            parser.stringField(root, "proof_kind", "$path.proof_kind"),
        )
    }

    private fun parseReceiptValue(root: Map<String, Any?>): AccountOnboardingPlanReceiptV1 {
        parser.exactKeys(root, setOf("body", "plan_hash", "signature"), "account onboarding receipt")
        return AccountOnboardingPlanReceiptV1(
            parseBody(parser.objectField(root, "body", "account onboarding receipt.body")),
            parser.stringField(root, "plan_hash", "account onboarding receipt.plan_hash"),
            parser.stringField(root, "signature", "account onboarding receipt.signature"),
        )
    }

    private fun parseBinding(root: Map<String, Any?>): TairaPublicResetMutationBindingV1 {
        val path = "public reset mutation binding"
        parser.exactKeys(
            root,
            setOf(
                "schema", "authorization_sha256", "authorization_nonce", "kind", "phase",
                "idempotency_key", "execution_expires_at_unix_ms",
            ),
            path,
        )
        return TairaPublicResetMutationBindingV1(
            parser.stringField(root, "schema", "$path.schema"),
            parser.stringField(root, "authorization_sha256", "$path.authorization_sha256"),
            parser.stringField(root, "authorization_nonce", "$path.authorization_nonce"),
            parser.stringField(root, "kind", "$path.kind"),
            parser.stringField(root, "phase", "$path.phase"),
            parser.stringField(root, "idempotency_key", "$path.idempotency_key"),
            parser.longField(root, "execution_expires_at_unix_ms", "$path.execution_expires_at_unix_ms"),
        )
    }

    private fun parseDisposition(root: Map<String, Any?>, path: String): AliasPlanDispositionV1 = when (
        parser.parseTaggedVariant(root, "kind", path)
    ) {
        "no_op" -> AliasPlanDispositionV1.NO_OP
        "repair" -> AliasPlanDispositionV1.REPAIR
        "create" -> AliasPlanDispositionV1.CREATE
        "conflict" -> AliasPlanDispositionV1.CONFLICT
        else -> error("$path is unsupported")
    }

    private fun root(payload: ByteArray, path: String): Map<String, Any?> {
        require(payload.isNotEmpty()) { "$path returned an empty payload" }
        return parser.objectValue(JsonParser.parse(String(payload, StandardCharsets.UTF_8)), path)
    }

    private fun positiveU64(value: Any?, path: String): BigInteger {
        val exact = when (value) {
            is BigInteger -> value
            is BigDecimal -> try {
                value.toBigIntegerExact()
            } catch (error: ArithmeticException) {
                throw IllegalStateException("$path must be an integer", error)
            }
            is Byte, is Short, is Int, is Long -> BigInteger.valueOf((value as Number).toLong())
            else -> throw IllegalStateException("$path must be an exact integer")
        }
        check(exact.signum() > 0 && exact.bitLength() <= 64) {
            "$path must be a positive unsigned 64-bit integer"
        }
        return exact
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

private fun isBindingTokenChar(value: Char): Boolean =
    value in 'a'..'z' || value in '0'..'9' || value == '-' || value == '_'

private fun requireLowerHex32(value: String, field: String): String {
    require(value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
        "$field must contain exactly 64 lowercase hexadecimal characters"
    }
    return value
}

private fun requireTransactionHash(value: String, field: String): String {
    require(
        value.length == 64 &&
            value.all { it in '0'..'9' || it in 'a'..'f' } &&
            value.last() in "13579bdf",
    ) {
        "$field must match the canonical Iroha HashOf marker pattern [0-9a-f]{63}[13579bdf]"
    }
    return value
}

private fun requireLowerHex(value: String, field: String): String {
    require(value.isNotEmpty() && value.length % 2 == 0 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
        "$field must contain non-empty even-length lowercase hexadecimal"
    }
    return value
}

private fun requireHex(value: String, field: String): String {
    require(value.isNotEmpty() && value.length % 2 == 0 && value.all { Character.digit(it, 16) >= 0 }) {
        "$field must contain non-empty even-length hexadecimal"
    }
    return value
}
