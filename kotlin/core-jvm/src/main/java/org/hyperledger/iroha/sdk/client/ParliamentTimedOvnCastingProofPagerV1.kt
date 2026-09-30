package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.util.concurrent.CompletableFuture

/** Fetch one independently authenticated, bounded proof page at the supplied checkpoint height. */
fun interface ParliamentTimedOvnCastingProofPageFetcherV1 {
    fun fetch(trustedCheckpointHeight: BigInteger): CompletableFuture<ParliamentTimedOvnCastingProofResponseV1>
}

/** Shared Kotlin/Java owner for complete-checkpoint promotion and durable bounded paging. */
object ParliamentTimedOvnCastingProofPagerV1 {
    /** Verify and persist each full checkpoint before fetching the next independently signed page. */
    @JvmStatic
    fun synchronize(
        initialTrustedCheckpointHeight: BigInteger,
        initialTrustedCheckpointNorito: ByteArray,
        pageFetcher: ParliamentTimedOvnCastingProofPageFetcherV1,
        pageVerifier: ParliamentTimedOvnCastingProofPageVerifierV1,
        checkpointPersister: ParliamentTimedOvnCastingCheckpointPersisterV1,
    ): CompletableFuture<ParliamentTimedOvnCastingProofTerminalV1> {
        val initialHeight =
            ParliamentApiV1.requireTimedOvnCastingCheckpointHeight(initialTrustedCheckpointHeight)
        require(initialTrustedCheckpointNorito.size in 1..(68 * 1024 * 1024)) {
            "initialTrustedCheckpointNorito must contain a bounded complete canonical checkpoint"
        }
        return page(
            initialHeight,
            initialTrustedCheckpointNorito.copyOf(),
            initialHeight,
            pageFetcher,
            pageVerifier,
            checkpointPersister,
            0,
        )
    }

    private fun page(
        currentHeight: BigInteger,
        currentCheckpoint: ByteArray,
        initialHeight: BigInteger,
        pageFetcher: ParliamentTimedOvnCastingProofPageFetcherV1,
        pageVerifier: ParliamentTimedOvnCastingProofPageVerifierV1,
        checkpointPersister: ParliamentTimedOvnCastingCheckpointPersisterV1,
        verifiedPages: Int,
    ): CompletableFuture<ParliamentTimedOvnCastingProofTerminalV1> {
        if (verifiedPages >= ParliamentApiV1.MAX_TIMED_OVN_CASTING_PROOF_PAGES) {
            return CompletableFuture<ParliamentTimedOvnCastingProofTerminalV1>().also {
                it.completeExceptionally(
                    IllegalStateException("Parliament casting-proof page limit was reached"),
                )
            }
        }
        return pageFetcher.fetch(currentHeight).thenCompose { response ->
            val verification = pageVerifier.verify(response, currentHeight, currentCheckpoint.copyOf())
            validatePromotion(initialHeight, currentHeight, verification)
            checkpointPersister.persist(verification).thenCompose {
                val nextPageCount = verifiedPages + 1
                if (!verification.moreAvailable) {
                    CompletableFuture.completedFuture(
                        ParliamentTimedOvnCastingProofTerminalV1(
                            response,
                            currentHeight,
                            currentCheckpoint,
                            verification,
                            nextPageCount,
                        ),
                    )
                } else {
                    page(
                        verification.evaluatedBlockHeight,
                        verification.promotedCheckpointNorito(),
                        initialHeight,
                        pageFetcher,
                        pageVerifier,
                        checkpointPersister,
                        nextPageCount,
                    )
                }
            }
        }
    }

    private fun validatePromotion(
        initialHeight: BigInteger,
        currentHeight: BigInteger,
        verification: ParliamentTimedOvnCastingProofPageVerificationV1,
    ) {
        val evaluatedHeight = verification.evaluatedBlockHeight
        require(evaluatedHeight >= currentHeight) {
            "native casting-proof verification regressed the checkpoint height"
        }
        val pageAdvance = evaluatedHeight.subtract(currentHeight)
        require(
            pageAdvance <= BigInteger.valueOf(ParliamentApiV1.MAX_TIMED_OVN_CASTING_PROOF_PAGE_HEIGHT_ADVANCE.toLong()),
        ) { "native casting-proof verification exceeded the page height bound" }
        require(
            evaluatedHeight.subtract(initialHeight) <=
                BigInteger.valueOf(ParliamentApiV1.MAX_TIMED_OVN_CASTING_PROOF_HEIGHT_ADVANCE.toLong()),
        ) { "native casting-proof verification exceeded the aggregate height bound" }
        if (verification.moreAvailable) {
            require(pageAdvance.signum() > 0) {
                "nonterminal casting-proof page did not advance its checkpoint"
            }
        }
        // Native verification authenticates the complete retained decision. Same-height alternate
        // certificate witnesses need not have byte-identical checkpoint encodings.
    }
}
