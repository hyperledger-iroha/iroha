package org.hyperledger.iroha.sdk.client

/** Request body for `POST /v1/zk/vk/register`. */
class VerifyingKeyRegisterRequest(
    val authority: String,
    val privateKey: String,
    val backend: String,
    val name: String,
    val version: Long,
    val circuitId: String,
    val publicInputsSchemaHashHex: String,
    val gasScheduleId: String,
    val curve: String? = null,
    val maxProofBytes: Long? = null,
    val metadataUriCid: String? = null,
    val verifyingKeyBytesCid: String? = null,
    val activationHeight: Long? = null,
    val withdrawHeight: Long? = null,
    val commitmentHex: String? = null,
    verifyingKeyBytes: ByteArray? = null,
    val verifyingKeyLength: Long? = null,
    val status: String? = null,
) {
    private val verifyingKeyBytesCopy: ByteArray? = verifyingKeyBytes?.copyOf()

    val verifyingKeyBytes: ByteArray?
        get() = verifyingKeyBytesCopy?.copyOf()
}
