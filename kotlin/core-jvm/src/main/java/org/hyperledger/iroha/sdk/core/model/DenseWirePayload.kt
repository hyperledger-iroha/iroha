package org.hyperledger.iroha.sdk.core.model

/**
 * An opaque instruction envelope of the dense Norito layout (`norito.md`, "Dense layout (header
 * minor 1)").
 *
 * In a dense transaction an instruction travels as its wire ID and its bare dense payload: there
 * is no nested Norito header, checksum or padding, and the wire ID alone identifies the type.
 * A wire ID the native bridge registers decodes into the instruction's typed [WirePayload] frame
 * instead. This envelope holds only a wire ID the bridge does not register: its payload stays
 * uninterpreted and re-encodes only in the dense layout it was decoded from, as the data model's
 * opaque envelope does. It is deliberately not a [WirePayload]: code that expects a framed minor-0
 * instruction payload never receives these bytes.
 */
interface DenseWirePayload : InstructionPayload {
    /** Registry wire ID of the instruction. */
    val wireName: String

    /** The instruction's bare dense payload; empty for an instruction without fields. */
    val payloadBytes: ByteArray
}
