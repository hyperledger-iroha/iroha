// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/**
 * Detached commitments to the exact Native-published initial State and paired ordinary Guard.
 * Each digest is SHA256 of the complete retained canonical original, including the Bootstrap
 * approval's signature and evidence. These bytes are acknowledgement data; they cannot reopen
 * an owner, authorize money, or stand in for Native verification of a wallet operation.
 * TODO: wire monetary work through a separate authenticated ordinary-current Native owner.
 */
class KagemushaOrdinaryInitialStatePublicationOriginalsV1 internal constructor(fields: List<ByteArray>) {
    private val originals = fields.drop(1).map(ByteArray::copyOf)
    fun enrollmentId(): ByteArray = originals[0].copyOf()
    fun publicationOriginalDigest(): ByteArray = originals[1].copyOf()
    fun retailCertificateOriginalDigest(): ByteArray = originals[2].copyOf()
    fun appCredentialOriginalDigest(): ByteArray = originals[3].copyOf()
    fun bootstrapApprovalOriginalDigest(): ByteArray = originals[4].copyOf()
    fun stateOriginalDigest(): ByteArray = originals[5].copyOf()
    fun pairedStateProofOriginalDigest(): ByteArray = originals[6].copyOf()
    fun pairedOrdinaryGuardOriginalDigest(): ByteArray = originals[7].copyOf()
}
