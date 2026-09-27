// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

/** Export raw Pixel 6 testnet evidence for inspection, without a monetary authority claim. */
fun pixel6TestnetObservationJsonV1(evidence: Pixel6TestnetObservationResultV1.Evidence): String =
    buildString {
        append("{\"profile\":\"")
        append(evidence.profile)
        append("\",\"hardware_one_use_qualified\":false")
        append(",\"attestation_root_trusted\":false,\"monetary_authority\":false")
        append(",\"recovered\":")
        append(evidence.recovered)
        fun field(name: String, value: ByteArray) {
            append(",\"")
            append(name)
            append("\":\"")
            value.forEach { byte -> append(HEX[(byte.toInt() ushr 4) and 15]); append(HEX[byte.toInt() and 15]) }
            append('"')
        }
        field("network_id_hex", evidence.networkId())
        field("release_id_hex", evidence.releaseId())
        field("canonical_selection_frame_hex", evidence.canonicalSelectionFrame())
        field("lane_commitment_hex", evidence.laneCommitment())
        field("secure_index_before_le_hex", evidence.secureIndexBeforeLittleEndian())
        field("secure_index_after_le_hex", evidence.secureIndexAfterLittleEndian())
        field("attestation_nonce_hex", evidence.attestationNonce())
        field("attestation_challenge_hex", evidence.attestationChallenge())
        field("public_key_sec1_hex", evidence.publicKey())
        field("signature_der_hex", evidence.signatureDer())
        field("signed_message_hex", evidence.signedMessage())
        append(",\"certificate_chain_der_hex\":[")
        evidence.certificateChain().forEachIndexed { index, certificate ->
            if (index != 0) append(',')
            append('"')
            certificate.forEach { byte ->
                append(HEX[(byte.toInt() ushr 4) and 15])
                append(HEX[byte.toInt() and 15])
            }
            append('"')
        }
        append("]}")
    }

private const val HEX = "0123456789abcdef"
