package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address

private const val COMPLETE_REPLICATION_ACTION = "CompleteReplicationOrder"

/** Exact governed signer-policy identity expected at completion commit. */
class ProviderIngestCompletionSignerPolicyV1(
    val policyId: String,
    val revision: Long,
    val predecessorDigest: String?,
    val policyDigest: String,
) {
    init {
        ReplicationOrderInstructionValidation.requireDigest(policyId, "policyId")
        ReplicationOrderInstructionValidation.requirePositiveRevision(revision, "revision")
        ReplicationOrderInstructionValidation.requireDigest(policyDigest, "policyDigest")
        if (revision == 1L) {
            require(predecessorDigest == null) {
                "predecessorDigest must be absent at revision one"
            }
        } else {
            require(predecessorDigest != null) {
                "predecessorDigest is required after revision one"
            }
            ReplicationOrderInstructionValidation.requireDigest(
                predecessorDigest,
                "predecessorDigest",
            )
        }
    }

    override fun equals(other: Any?): Boolean = other is ProviderIngestCompletionSignerPolicyV1 &&
        policyId == other.policyId && revision == other.revision &&
        predecessorDigest == other.predecessorDigest && policyDigest == other.policyDigest

    override fun hashCode(): Int {
        var result = policyId.hashCode()
        result = 31 * result + revision.hashCode()
        result = 31 * result + (predecessorDigest?.hashCode() ?: 0)
        return 31 * result + policyDigest.hashCode()
    }

    internal fun canonicalJson(): String {
        val predecessor = predecessorDigest?.let { "\"$it\"" } ?: "null"
        return "{\"policy_id\":\"$policyId\",\"revision\":$revision," +
            "\"predecessor_digest\":$predecessor,\"policy_digest\":\"$policyDigest\"}"
    }
}

/** Exact provider owner, completion signer and signer policy expected at completion commit. */
class ProviderIngestCompletionAuthorityV1(
    val providerOwner: String,
    val completionSigner: String,
    val signerPolicy: ProviderIngestCompletionSignerPolicyV1,
) {
    init {
        require(
            ReplicationOrderInstructionValidation.requireProviderOwner(providerOwner) ==
                providerOwner,
        ) {
            "providerOwner must be an exact canonical I105 account id"
        }
        require(requireCanonicalI105Address(completionSigner, "completionSigner") == completionSigner) {
            "completionSigner must be an exact canonical I105 account id"
        }
    }

    override fun equals(other: Any?): Boolean = other is ProviderIngestCompletionAuthorityV1 &&
        providerOwner == other.providerOwner && completionSigner == other.completionSigner &&
        signerPolicy == other.signerPolicy

    override fun hashCode(): Int =
        31 * (31 * providerOwner.hashCode() + completionSigner.hashCode()) + signerPolicy.hashCode()

    internal fun canonicalJson(): String =
        "{\"provider_owner\":\"$providerOwner\",\"completion_signer\":\"$completionSigner\"," +
            "\"signer_policy\":" +
            "${signerPolicy.canonicalJson()}}"
}

/** Exact finalized committed-chain prefix used to prepare a completion. */
class ProviderIngestFinalizedAnchorV1(
    val height: Long,
    val blockHash: String,
) {
    init {
        ReplicationOrderInstructionValidation.requirePositiveRevision(height, "height")
        ReplicationOrderInstructionValidation.requireDigest(blockHash, "blockHash")
    }

    override fun equals(other: Any?): Boolean = other is ProviderIngestFinalizedAnchorV1 &&
        height == other.height && blockHash == other.blockHash

    override fun hashCode(): Int = 31 * height.hashCode() + blockHash.hashCode()

    internal fun canonicalJson(): String =
        "{\"height\":$height,\"block_hash\":\"$blockHash\"}"
}

/** Typed representation of the six-field `CompleteReplicationOrder` hard cut. */
class CompleteReplicationOrderInstruction(
    orderId: String,
    providerId: String,
    completionEpoch: Long,
    expectedAuthority: ProviderIngestCompletionAuthorityV1,
    expectedAssignmentRevision: Long,
    finalizedAnchor: ProviderIngestFinalizedAnchorV1,
) : InstructionTemplate {
    val orderId: String = ReplicationOrderInstructionValidation.requireOrderId(orderId)
    val providerId: String = ReplicationOrderInstructionValidation.requireProviderId(providerId)
    val completionEpoch: Long =
        ReplicationOrderInstructionValidation.requireEpoch(completionEpoch, "completionEpoch")
    val expectedAuthority: ProviderIngestCompletionAuthorityV1 = expectedAuthority
    val expectedAssignmentRevision: Long =
        ReplicationOrderInstructionValidation.requirePositiveRevision(
            expectedAssignmentRevision,
            "expectedAssignmentRevision",
        )
    val finalizedAnchor: ProviderIngestFinalizedAnchorV1 = finalizedAnchor

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = linkedMapOf(
        "action" to COMPLETE_REPLICATION_ACTION,
        "order_id" to this.orderId,
        "provider_id" to this.providerId,
        "completion_epoch" to this.completionEpoch.toString(),
        "expected_authority" to this.expectedAuthority.canonicalJson(),
        "expected_assignment_revision" to this.expectedAssignmentRevision.toString(),
        "finalized_anchor" to this.finalizedAnchor.canonicalJson(),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CompleteReplicationOrderInstruction) return false
        return orderId == other.orderId &&
            providerId == other.providerId &&
            completionEpoch == other.completionEpoch &&
            expectedAuthority == other.expectedAuthority &&
            expectedAssignmentRevision == other.expectedAssignmentRevision &&
            finalizedAnchor == other.finalizedAnchor
    }

    override fun hashCode(): Int {
        var result = orderId.hashCode()
        result = 31 * result + providerId.hashCode()
        result = 31 * result + completionEpoch.hashCode()
        result = 31 * result + expectedAuthority.hashCode()
        result = 31 * result + expectedAssignmentRevision.hashCode()
        result = 31 * result + finalizedAnchor.hashCode()
        return result
    }

    companion object {
        private val authorityPattern = Regex(
            """^\{"provider_owner":"([^"\\]+)","completion_signer":"([^"\\]+)","signer_policy":\{"policy_id":"([0-9a-f]{64})","revision":([1-9][0-9]*),"predecessor_digest":(null|"([0-9a-f]{64})"),"policy_digest":"([0-9a-f]{64})"\}\}$""",
        )
        private val anchorPattern = Regex(
            """^\{"height":([1-9][0-9]*),"block_hash":"([0-9a-f]{64})"\}$""",
        )

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CompleteReplicationOrderInstruction {
            ReplicationOrderInstructionValidation.requireArguments(
                arguments,
                COMPLETE_REPLICATION_ACTION,
                setOf(
                    "order_id",
                    "provider_id",
                    "completion_epoch",
                    "expected_authority",
                    "expected_assignment_revision",
                    "finalized_anchor",
                ),
            )
            return CompleteReplicationOrderInstruction(
                orderId = require(arguments, "order_id"),
                providerId = require(arguments, "provider_id"),
                completionEpoch = requireLong(arguments, "completion_epoch"),
                expectedAuthority = parseAuthority(require(arguments, "expected_authority")),
                expectedAssignmentRevision =
                    requireLong(arguments, "expected_assignment_revision"),
                finalizedAnchor = parseAnchor(require(arguments, "finalized_anchor")),
            )
        }

        private fun parseAuthority(value: String): ProviderIngestCompletionAuthorityV1 {
            val match = authorityPattern.matchEntire(value)
                ?: throw IllegalArgumentException(
                    "Instruction argument 'expected_authority' must use canonical JSON",
                )
            val policy = ProviderIngestCompletionSignerPolicyV1(
                policyId = match.groupValues[3],
                revision = requireLongLiteral(match.groupValues[4], "signer policy revision"),
                predecessorDigest = match.groupValues[6].ifEmpty { null },
                policyDigest = match.groupValues[7],
            )
            val authority = ProviderIngestCompletionAuthorityV1(
                providerOwner = match.groupValues[1],
                completionSigner = match.groupValues[2],
                signerPolicy = policy,
            )
            require(authority.canonicalJson() == value) {
                "Instruction argument 'expected_authority' must use canonical JSON"
            }
            return authority
        }

        private fun parseAnchor(value: String): ProviderIngestFinalizedAnchorV1 {
            val match = anchorPattern.matchEntire(value)
                ?: throw IllegalArgumentException(
                    "Instruction argument 'finalized_anchor' must use canonical JSON",
                )
            val anchor = ProviderIngestFinalizedAnchorV1(
                height = requireLongLiteral(match.groupValues[1], "finalized anchor height"),
                blockHash = match.groupValues[2],
            )
            require(anchor.canonicalJson() == value) {
                "Instruction argument 'finalized_anchor' must use canonical JSON"
            }
            return anchor
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long =
            requireLongLiteral(require(arguments, key), "Instruction argument '$key'")

        private fun requireLongLiteral(value: String, context: String): Long {
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("$context must be a number: $value", ex)
            }
        }
    }
}
