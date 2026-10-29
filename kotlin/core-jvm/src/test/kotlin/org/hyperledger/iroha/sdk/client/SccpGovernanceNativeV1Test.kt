package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Release acceptance uses the real mandatory native account-admission owner. */
class SccpGovernanceNativeV1Test {
    @Test
    fun strandedReleaseAdmitsCanonicalRecipientAndExactAmountAndMemoBounds() {
        val proposal = SccpProposalFixtures.proposal("release_stranded")
        val body = SccpProposalFixtures.body(proposal)
        body["recipient"] =
            AccountAddress.fromAccount(TestEd25519Keys.publicKey(1), "ed25519")
                .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        for (amount in listOf("1", BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE).toString())) {
            for (memo in listOf("", "a".repeat(256), "é".repeat(128), "🦊".repeat(64), " \n\u0000 ")) {
                body["amount"] = amount
                body["memo"] = memo
                assertEquals("SccpRouteGovernance", read(proposal).kind)
            }
        }
        for (field in body.keys.toList()) {
            val saved = body.remove(field)
            assertFailsWith<IllegalArgumentException> { read(proposal) }
            body[field] = saved
        }
        body["extra"] = null
        assertFailsWith<IllegalArgumentException> { read(proposal) }
        body.remove("extra")
        val recipient = body["recipient"]
        for (invalid in listOf("alice@wonderland", "", "invalid", " $recipient")) {
            body["recipient"] = invalid
            assertFailsWith<IllegalArgumentException> { read(proposal) }
        }
        body["recipient"] = recipient
        SccpProposalFixtures.inner(proposal)["base_revisions"] =
            listOf(
                SccpProposalFixtures.base("route_control", "bsc_mainnet"),
            )
        assertFailsWith<IllegalArgumentException> { read(proposal) }
    }

    private fun read(value: Map<String, Any?>): ParliamentApiV1.Proposal =
        ParliamentApiV1.Proposal.fromJson(SccpProposalFixtures.encode(value))
}
