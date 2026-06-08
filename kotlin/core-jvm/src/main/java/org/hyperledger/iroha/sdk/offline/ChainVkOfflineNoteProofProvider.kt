package org.hyperledger.iroha.sdk.offline

/** Offline Note proof provider that proves against a chain-supplied `VerifyingKeyBox`. */
class ChainVkOfflineNoteProofProvider(vkBoxNorito: ByteArray) : OfflineNoteProofProvider {
    private val vkBoxNorito = vkBoxNorito.copyOf()

    init {
        require(this.vkBoxNorito.isNotEmpty()) { "vkBoxNorito must not be empty" }
    }

    override fun proveAudit(audit: OfflineNote.AuditBundle): OfflineNote.RecursiveProof {
        val proofNorito = NativeOfflineNoteProver.proveAudit(OfflineNote.encodeAudit(audit), vkBoxNorito)
        return OfflineNote.decodeRecursiveProof(proofNorito)
    }

    override fun proveRedeem(redemption: OfflineNote.Redeem): OfflineNote.RecursiveProof {
        val proofNorito = NativeOfflineNoteProver.proveRedeem(OfflineNote.encodeRedeem(redemption), vkBoxNorito)
        return OfflineNote.decodeRecursiveProof(proofNorito)
    }
}
