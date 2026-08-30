package org.hyperledger.iroha.sdk.client

import kotlin.test.Test
import kotlin.test.assertFailsWith

class AuthenticatedCommittedRejectionV1Test {
    @Test
    fun rejectionCodeRemainsAClosedAbi22Union() {
        fixture("validation")
        assertFailsWith<IllegalArgumentException> {
            fixture("server_error")
        }
    }

    private fun fixture(rejectionCode: String): AuthenticatedCommittedRejectionV1 =
        AuthenticatedCommittedRejectionV1(
            transactionHashHex = "ab".repeat(32),
            transactionAuthorityAccountId = "canonical-authority-from-native",
            blockHashHex = "cd".repeat(32),
            resultHashHex = "ef".repeat(32),
            rejectionCode = rejectionCode,
            rejectionMessage = "permission denied",
            committedBlockHeight = 7L,
        )
}
