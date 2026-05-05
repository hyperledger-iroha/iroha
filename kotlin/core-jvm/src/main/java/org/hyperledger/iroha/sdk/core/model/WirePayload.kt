package org.hyperledger.iroha.sdk.core.model

interface WirePayload : InstructionPayload {
    val wireName: String
    val payloadBytes: ByteArray
}
