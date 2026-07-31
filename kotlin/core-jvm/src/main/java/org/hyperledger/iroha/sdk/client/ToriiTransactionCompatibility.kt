package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** First-release compatibility values required before submitting a transaction to Torii. */
object ToriiTransactionCompatibility {
    /** Current data-model version encoded by this SDK. */
    const val EXPECTED_DATA_MODEL_VERSION: Int = 4

    /** Current `SignedTransaction` Norito schema hash encoded by this SDK. */
    const val EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX: String =
        "7ab5ff9c572efb316deac478f19209c5"

    internal fun requireCompatible(payload: ByteArray) {
        val json = String(payload, StandardCharsets.UTF_8)
        require(json.toByteArray(StandardCharsets.UTF_8).contentEquals(payload)) {
            "node capabilities response must be valid UTF-8 JSON"
        }
        val parsed = JsonParser.parse(json)
        val fields = parsed as? Map<*, *>
            ?: throw IllegalArgumentException("node capabilities response must be a JSON object")

        val actualVersion = JsonNumbers.asInt(
            fields["data_model_version"],
            "node capabilities response.data_model_version",
        )
        if (actualVersion != EXPECTED_DATA_MODEL_VERSION) {
            throw ToriiDataModelMismatchException(
                EXPECTED_DATA_MODEL_VERSION,
                actualVersion,
            )
        }

        val actualSchemaHash = fields["signed_transaction_schema_hash_hex"] as? String
        if (actualSchemaHash != EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX) {
            throw ToriiTransactionSchemaMismatchException(
                EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX,
                actualSchemaHash,
            )
        }
    }
}

/** Base class for transaction-submission capability guard failures. */
open class ToriiTransactionCompatibilityException(
    message: String,
    cause: Throwable? = null,
) : IllegalStateException(message, cause)

/** Raised when Torii advertises a data-model version this SDK cannot encode. */
class ToriiDataModelMismatchException(
    val expected: Int,
    val actual: Int,
) : ToriiTransactionCompatibilityException(
    "Torii node data_model_version $actual does not match client version $expected",
)

/** Raised when Torii advertises a different signed-transaction schema. */
class ToriiTransactionSchemaMismatchException(
    val expected: String,
    val actual: String?,
) : ToriiTransactionCompatibilityException(
    "Torii node signed_transaction_schema_hash_hex " +
        "${actual ?: "<missing-or-invalid>"} does not match client schema $expected",
)

/** Raised when the fresh Torii capability advert cannot be fetched or decoded exactly. */
class ToriiTransactionCompatibilityProbeException(
    cause: Throwable,
) : ToriiTransactionCompatibilityException(
    "Failed to verify Torii transaction submission compatibility",
    cause,
)
