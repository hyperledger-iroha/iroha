package org.hyperledger.iroha.sdk.sorafs

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Assumptions.assumeTrue
import org.junit.jupiter.api.Test

class SorafsReferenceValidatorsTest {
    private val maxScaledXor =
        "6703903964971298549787012499102923063739682910296196688861780721860882015" +
            "036773488400937149083451713845015929093243025426876941405973284973216824" +
            ".503042047"

    @Test
    fun exposesBridgeSelectors() {
        assertEquals(1, SorafsOrderbookPayloadKind.ORDER_REQUEST.bridgeCode)
        assertEquals(6, SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.bridgeCode)
        assertTrue(SorafsOrderbookPayloadKind.ORDER_REQUEST.isUserSignedPayload)
        assertTrue(!SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.isUserSignedPayload)
        assertEquals(1, SorafsPdpPayloadKind.COMMITMENT.bridgeCode)
        assertEquals(3, SorafsPdpPayloadKind.PROOF.bridgeCode)
        assertEquals(1, SorafsPopPayloadKind.CREDENTIAL.bridgeCode)
        assertEquals(6, SorafsPopPayloadKind.MEMBERSHIP_PROOF.bridgeCode)
        assertEquals(7, SorafsPopPayloadKind.ISSUED_CREDENTIAL_BUNDLE.bridgeCode)
        assertEquals(1, SorafsHedgingPayloadKind.PRICE_FEED.bridgeCode)
        assertEquals(4, SorafsHedgingPayloadKind.BILLING_STATEMENT.bridgeCode)
        assertEquals(1, SorafsOrderbookSide.BID.bridgeCode)
        assertEquals(3, SorafsOrderbookTier.ARCHIVE.bridgeCode)
        assertEquals(4, SorafsOrderbookCancelReason.REPLACED.bridgeCode)
        assertEquals(21, SorafsReferenceValidators.REQUIRED_BRIDGE_ABI_VERSION)
        assertTrue(!SorafsReferenceValidators.isBridgeAbiSupported(20))
        assertTrue(SorafsReferenceValidators.isBridgeAbiSupported(21))
        assertTrue(!SorafsReferenceValidators.isGovernanceDagBridgeSupported(21, false))
        assertTrue(SorafsReferenceValidators.isGovernanceDagBridgeSupported(21, true))
        assertEquals(64, SorafsReferenceValidators.GOVERNANCE_DAG_MAX_BLOCKS_V1)
        assertEquals(32, SorafsReferenceValidators.GOVERNANCE_DAG_CID_BYTES_V1)
        assertEquals(67_108_864, SorafsReferenceValidators.REFERENCE_MAX_INPUT_BYTES_V1)
        assertEquals(1_024, SorafsReferenceValidators.REFERENCE_MAX_LABEL_BYTES_V1)
    }

    @Test
    fun rejectsGeneratedAtBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateOrderbookPayloadJson(
                SorafsOrderbookPayloadKind.ORDER_REQUEST,
                ByteArray(0),
                generatedAtUnix = -1,
            )
        }
        assertTrue(error.message.orEmpty().contains("generatedAtUnix"))
    }

    @Test
    fun rejectsBlankLabelBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateHedgingPayloadJson(
                SorafsHedgingPayloadKind.PRICE_FEED,
                ByteArray(0),
                label = " ",
                generatedAtUnix = 1,
            )
        }
        assertTrue(error.message.orEmpty().contains("label"))
    }

    @Test
    fun boundsGovernanceDagInputsBeforeNativeDispatch() {
        val emptyChain = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateGovernanceDagHeadChainJson(
                head = ByteArray(0),
                blocks = emptyList(),
                generatedAtUnix = 1,
            )
        }
        assertTrue(emptyChain.message.orEmpty().contains("1..64"))

        val tooManyBlocks = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateGovernanceDagHeadChainJson(
                head = ByteArray(0),
                blocks = List(65) { ByteArray(0) },
                generatedAtUnix = 1,
            )
        }
        assertTrue(tooManyBlocks.message.orEmpty().contains("1..64"))

        val mismatchedLabels = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateGovernanceDagHeadChainJson(
                head = ByteArray(0),
                blocks = listOf(ByteArray(0)),
                blockLabels = emptyList(),
                generatedAtUnix = 1,
            )
        }
        assertTrue(mismatchedLabels.message.orEmpty().contains("blockLabels"))

        val oversizedLabel = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateGovernanceDagBlockJson(
                noritoBytes = ByteArray(0),
                label = "x".repeat(1_025),
                generatedAtUnix = 1,
            )
        }
        assertTrue(oversizedLabel.message.orEmpty().contains("1024"))

        val controlLabel = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateGovernanceDagBlockJson(
                noritoBytes = ByteArray(0),
                label = "bad\u0001label",
                generatedAtUnix = 1,
            )
        }
        assertTrue(controlLabel.message.orEmpty().contains("control characters"))

        for (invalidCid in listOf(ByteArray(0), ByteArray(31), ByteArray(33))) {
            val invalidExpectedCid = assertThrows(IllegalArgumentException::class.java) {
                SorafsReferenceValidators.validateGovernanceDagBlockJson(
                    noritoBytes = ByteArray(0),
                    expectedBlockCid = invalidCid,
                    generatedAtUnix = 1,
                )
            }
            assertTrue(invalidExpectedCid.message.orEmpty().contains("exactly 32 bytes"))
        }
    }

    @Test
    fun rejectsRuntimeSnapshotSigningBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.signOrderbookPayload(
                SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT,
                ByteArray(0),
                ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(error.message.orEmpty().contains("cannot be signed"))
    }

    @Test
    fun rejectsBadSigningKeyBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.signOrderbookPayload(
                SorafsOrderbookPayloadKind.ORDER_REQUEST,
                ByteArray(0),
                ByteArray(32),
            )
        }
        assertTrue(error.message.orEmpty().contains("privateKey"))
    }

    @Test
    fun rejectsInvalidOrderIdDerivationInputsBeforeNativeDispatch() {
        val emptyOwner = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.deriveOrderbookOrderId(ByteArray(0), 7)
        }
        assertTrue(emptyOwner.message.orEmpty().contains("ownerAccount"))

        val zeroNonce = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.deriveOrderbookOrderId(byteArrayOf(1), 0)
        }
        assertTrue(zeroNonce.message.orEmpty().contains("nonce"))
    }

    @Test
    fun rejectsOversizedOrderbookOwnerAccountsBeforeNativeDispatch() {
        assertEquals(256, SorafsReferenceValidators.ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1)
        val oversized = ByteArray(
            SorafsReferenceValidators.ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1,
        ) { 0x45 }
        val deriveError = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.deriveOrderbookOrderId(oversized, 7)
        }
        assertTrue(deriveError.message.orEmpty().contains("at most 256 bytes"))

        val requestError = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                side = SorafsOrderbookSide.BID,
                tier = SorafsOrderbookTier.HOT,
                pricePerGib = "1",
                quantityGib = 1,
                ownerAccount = oversized,
                providerId = null,
                expiryUnix = 1,
                nonce = 7,
                makerFeeBps = 0,
                takerFeeBps = 0,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(requestError.message.orEmpty().contains("at most 256 bytes"))

        val cancelError = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderCancel(
                orderId = ByteArray(32) { 0x11 },
                ownerAccount = oversized,
                reason = SorafsOrderbookCancelReason.OWNER_REQUESTED,
                nonce = 8,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(cancelError.message.orEmpty().contains("at most 256 bytes"))
    }

    @Test
    fun rejectsOrderbookOrderRequestFieldsBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                orderId = ByteArray(31) { 0x11.toByte() },
                side = SorafsOrderbookSide.BID,
                tier = SorafsOrderbookTier.HOT,
                pricePerGib = "42",
                quantityGib = 7,
                ownerAccount = byteArrayOf(0x01),
                providerId = null,
                expiryUnix = 123,
                nonce = 1,
                makerFeeBps = 0,
                takerFeeBps = 25,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(error.message.orEmpty().contains("orderId"))
    }

    @Test
    fun rejectsOrderbookSettlementReceiptFieldsBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookSettlementReceipt(
                receiptId = ByteArray(32) { 0x21.toByte() },
                channelId = ByteArray(32) { 0x22.toByte() },
                tradeId = ByteArray(32) { 0x23.toByte() },
                rangeStart = 0,
                rangeEnd = 64,
                chunkHash = ByteArray(32) { 0x24.toByte() },
                bytesDelivered = 64,
                xorDebited = "not-a-decimal",
                providerCredit = "10",
                feeAmount = "1",
                issuedAtUnix = 123,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(error.message.orEmpty().contains("xorDebited"))
    }

    @Test
    fun rejectsNoncanonicalOrOverprecisionXorQuantitiesBeforeNativeDispatch() {
        assertEquals(155, maxScaledXor.length)
        for (value in listOf("1.0", "0.0000000001", "1".repeat(156))) {
            val error = assertThrows(IllegalArgumentException::class.java) {
                SorafsReferenceValidators.buildSignedOrderbookSettlementReceipt(
                    receiptId = ByteArray(32) { 0x21.toByte() },
                    channelId = ByteArray(32) { 0x22.toByte() },
                    tradeId = ByteArray(32) { 0x23.toByte() },
                    rangeStart = 0,
                    rangeEnd = 64,
                    chunkHash = ByteArray(32) { 0x24.toByte() },
                    bytesDelivered = 64,
                    xorDebited = value,
                    providerCredit = "0",
                    feeAmount = "0",
                    issuedAtUnix = 123,
                    privateKey = ByteArray(32) { 0xB7.toByte() },
                )
            }
            assertTrue(error.message.orEmpty().contains("xorDebited"))
        }
    }

    @Test
    fun validatesOrderbookFixtureWhenNativeBridgeIsAvailable() {
        assumeTrue(SorafsReferenceValidators.isNativeAvailable(), "connect_norito_bridge not available")
        val payload = fixture("sorafs_manifest", "orderbook", "order_request_v1.to")
        val json = SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST,
            payload,
            label = "order_request_v1.to",
            generatedAtUnix = 123,
        )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "orderbook",
                "order_request_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            json,
        )

        for (name in listOf("order_request_bad_signature", "order_request_trailing_bytes")) {
            val outcome = SorafsReferenceValidators.validateOrderbookPayloadJson(
                SorafsOrderbookPayloadKind.ORDER_REQUEST,
                fixture("sorafs_manifest", "orderbook", "negative", "${name}_v1.to"),
                label = "${name}_v1.to",
                generatedAtUnix = 123,
            )
            assertEquals(
                fixture(
                    "sorafs_manifest",
                    "orderbook",
                    "negative",
                    "${name}_validation_outcome_v1.json",
                ).toString(Charsets.UTF_8),
                outcome,
                name,
            )
        }
    }

    @Test
    fun validatesEveryPdpOutcomeFixtureWhenNativeBridgeIsAvailable() {
        assumeTrue(SorafsReferenceValidators.isNativeAvailable(), "connect_norito_bridge not available")
        val commitment = fixture("sorafs_manifest", "pdp", "commitment_v1.to")
        val challenge = fixture("sorafs_manifest", "pdp", "challenge_v1.to")
        val proof = fixture("sorafs_manifest", "pdp", "proof_v1.to")
        val bundle = SorafsReferenceValidators.validatePdpBundleJson(
            commitment,
            challenge,
            proof,
            commitmentLabel = "commitment_v1.to",
            challengeLabel = "challenge_v1.to",
            proofLabel = "proof_v1.to",
            generatedAtUnix = 123,
        )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "pdp",
                "bundle_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            bundle,
        )

        for ((name, kind) in listOf(
            "duplicate_hot_leaf_challenge" to SorafsPdpPayloadKind.CHALLENGE,
            "missing_signature_proof" to SorafsPdpPayloadKind.PROOF,
        )) {
            val outcome = SorafsReferenceValidators.validatePdpPayloadJson(
                kind,
                fixture("sorafs_manifest", "pdp", "negative", "${name}_v1.to"),
                label = "${name}_v1.to",
                generatedAtUnix = 123,
            )
            assertPdpOutcome(name, outcome)
        }

        for (name in listOf("late_proof", "wrong_manifest_proof", "wrong_provider_proof")) {
            val outcome = SorafsReferenceValidators.validatePdpChallengeProofJson(
                challenge,
                fixture("sorafs_manifest", "pdp", "negative", "${name}_v1.to"),
                challengeLabel = "challenge_v1.to",
                proofLabel = "${name}_v1.to",
                generatedAtUnix = 123,
            )
            assertPdpOutcome(name, outcome)
        }

        for (name in listOf(
            "missing_hot_leaf_path_proof",
            "missing_segment_path_proof",
            "wrong_path_proof",
        )) {
            val outcome = SorafsReferenceValidators.validatePdpBundleJson(
                commitment,
                challenge,
                fixture("sorafs_manifest", "pdp", "negative", "${name}_v1.to"),
                commitmentLabel = "commitment_v1.to",
                challengeLabel = "challenge_v1.to",
                proofLabel = "${name}_v1.to",
                generatedAtUnix = 123,
            )
            assertPdpOutcome(name, outcome)
        }
    }

    @Test
    fun validatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable() {
        requireGovernanceDagNativeBridge()
        val first = fixture("sorafs_manifest", "governance", "dag_block_0_v1.to")
        val second = fixture("sorafs_manifest", "governance", "dag_block_1_v1.to")
        val head = fixture("sorafs_manifest", "governance", "dag_head_v1.to")

        val blockOutcome = SorafsReferenceValidators.validateGovernanceDagBlockJson(
            first,
            label = "dag_block_0_v1.to",
            generatedAtUnix = 123,
        )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "governance",
                "dag_block_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            blockOutcome,
        )

        val cidMismatch = SorafsReferenceValidators.validateGovernanceDagBlockJson(
            first,
            expectedBlockCid = ByteArray(32) { 0x7f },
            generatedAtUnix = 123,
        )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "governance",
                "dag_block_cid_mismatch_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            cidMismatch,
        )

        val headOutcome = SorafsReferenceValidators.validateGovernanceDagHeadChainJson(
            head = head,
            blocks = listOf(first, second),
            headLabel = "dag_head_v1.to",
            blockLabels = listOf("dag_block_0_v1.to", "dag_block_1_v1.to"),
            generatedAtUnix = 123,
        )
        val goldenOutcome = fixture(
            "sorafs_manifest",
            "governance",
            "dag_head_validation_outcome_v1.json",
        ).toString(Charsets.UTF_8)
        assertEquals(goldenOutcome, headOutcome)

        val reordered = SorafsReferenceValidators.validateGovernanceDagHeadChainJson(
            head,
            listOf(second, first),
            generatedAtUnix = 123,
        )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "governance",
                "dag_head_reordered_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            reordered,
        )

        val blockSignatureOutcome =
            SorafsReferenceValidators.validateGovernanceDagBlockJson(
                fixture(
                    "sorafs_manifest",
                    "governance",
                    "dag_block_bad_signature_v1.to",
                ),
                label = "dag_block_bad_signature_v1.to",
                generatedAtUnix = 123,
            )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "governance",
                "dag_block_bad_signature_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            blockSignatureOutcome,
        )

        val trailingBytesOutcome =
            SorafsReferenceValidators.validateGovernanceDagBlockJson(
                fixture(
                    "sorafs_manifest",
                    "governance",
                    "dag_block_trailing_bytes_v1.to",
                ),
                label = "dag_block_trailing_bytes_v1.to",
                generatedAtUnix = 123,
            )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "governance",
                "dag_block_trailing_bytes_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            trailingBytesOutcome,
        )

        val headSignatureOutcome =
            SorafsReferenceValidators.validateGovernanceDagHeadChainJson(
                head = fixture(
                    "sorafs_manifest",
                    "governance",
                    "dag_head_bad_signature_v1.to",
                ),
                blocks = listOf(first, second),
                headLabel = "dag_head_bad_signature_v1.to",
                blockLabels = listOf("dag_block_0_v1.to", "dag_block_1_v1.to"),
                generatedAtUnix = 123,
            )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "governance",
                "dag_head_bad_signature_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            headSignatureOutcome,
        )

        val predecessorOutcome =
            SorafsReferenceValidators.validateGovernanceDagHeadChainJson(
                head = fixture(
                    "sorafs_manifest",
                    "governance",
                    "dag_head_bad_predecessor_v1.to",
                ),
                blocks = listOf(
                    first,
                    fixture(
                        "sorafs_manifest",
                        "governance",
                        "dag_block_1_bad_predecessor_v1.to",
                    ),
                ),
                headLabel = "dag_head_bad_predecessor_v1.to",
                blockLabels = listOf(
                    "dag_block_0_v1.to",
                    "dag_block_1_bad_predecessor_v1.to",
                ),
                generatedAtUnix = 123,
            )
        assertEquals(
            fixture(
                "sorafs_manifest",
                "governance",
                "dag_head_bad_predecessor_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            predecessorOutcome,
        )
    }

    private fun requireGovernanceDagNativeBridge() {
        if (SorafsReferenceValidators.isNativeAvailable()) {
            return
        }
        if (System.getenv("IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION") == "1") {
            throw AssertionError(
                "ABI-21 connect_norito_bridge with Governance DAG symbols is required.",
            )
        }
        assumeTrue(false, "connect_norito_bridge not available")
    }

    @Test
    fun signsOrderbookFixtureWhenNativeBridgeIsAvailable() {
        assumeTrue(SorafsReferenceValidators.isNativeAvailable(), "connect_norito_bridge not available")
        val payload = fixture("sorafs_manifest", "orderbook", "order_request_v1.to")
        val signed = SorafsReferenceValidators.signOrderbookPayload(
            SorafsOrderbookPayloadKind.ORDER_REQUEST,
            payload,
            ByteArray(32) { 0xB7.toByte() },
        )
        assertTrue(signed.isNotEmpty())
        assertTrue(!signed.contentEquals(payload))
    }

    @Test
    fun derivesCanonicalOrderIdAndRejectsExplicitMismatchWhenNativeBridgeIsAvailable() {
        assumeTrue(SorafsReferenceValidators.isNativeAvailable(), "connect_norito_bridge not available")
        val owner = "buyer@sora".toByteArray(Charsets.UTF_8)
        val orderId = SorafsReferenceValidators.deriveOrderbookOrderId(owner, 7)
        assertEquals(
            "9d91ad7700ca0c4762e031f9231aa38dd4502c6048c6ffa31d365e3c4e080b69",
            orderId.toHex(),
        )
        assertTrue(!orderId.contentEquals(SorafsReferenceValidators.deriveOrderbookOrderId(owner, 8)))
        assertTrue(
            !orderId.contentEquals(
                SorafsReferenceValidators.deriveOrderbookOrderId(
                    "provider@sora".toByteArray(Charsets.UTF_8),
                    7,
                ),
            ),
        )

        val maximumOwner = ByteArray(
            SorafsReferenceValidators.ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
        ) { 0x45 }
        val maximumOwnerOrderId =
            SorafsReferenceValidators.deriveOrderbookOrderId(maximumOwner, 9)
        val maximumOwnerOrder = SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            side = SorafsOrderbookSide.BID,
            tier = SorafsOrderbookTier.HOT,
            pricePerGib = "1",
            quantityGib = 1,
            ownerAccount = maximumOwner,
            providerId = null,
            expiryUnix = 1_800_000_000,
            nonce = 9,
            makerFeeBps = 0,
            takerFeeBps = 0,
            privateKey = ByteArray(32) { 0xB7.toByte() },
        )
        assertTrue(
            SorafsReferenceValidators.validateOrderbookPayloadJson(
                SorafsOrderbookPayloadKind.ORDER_REQUEST,
                maximumOwnerOrder,
                generatedAtUnix = 123,
            ).contains("\"status\": \"Ok\""),
        )
        val maximumOwnerCancel = SorafsReferenceValidators.buildSignedOrderbookOrderCancel(
            orderId = maximumOwnerOrderId,
            ownerAccount = maximumOwner,
            reason = SorafsOrderbookCancelReason.OWNER_REQUESTED,
            nonce = 10,
            privateKey = ByteArray(32) { 0xB7.toByte() },
        )
        assertTrue(
            SorafsReferenceValidators.validateOrderbookPayloadJson(
                SorafsOrderbookPayloadKind.ORDER_CANCEL,
                maximumOwnerCancel,
                generatedAtUnix = 123,
            ).contains("\"status\": \"Ok\""),
        )

        val signed = SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            side = SorafsOrderbookSide.BID,
            tier = SorafsOrderbookTier.HOT,
            pricePerGib = maxScaledXor,
            quantityGib = 64,
            ownerAccount = owner,
            providerId = null,
            expiryUnix = 1_800_000_000,
            nonce = 7,
            makerFeeBps = 10,
            takerFeeBps = 15,
            privateKey = ByteArray(32) { 0xB7.toByte() },
        )
        val outcome = SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST,
            signed,
            generatedAtUnix = 123,
        )
        assertTrue(outcome.contains("\"status\": \"Ok\""), outcome)

        val ask = SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            side = SorafsOrderbookSide.ASK,
            tier = SorafsOrderbookTier.HOT,
            pricePerGib = "1.25",
            quantityGib = 4,
            ownerAccount = owner,
            providerId = ByteArray(32) { 0x72 },
            expiryUnix = 1_800_000_000,
            nonce = 8,
            makerFeeBps = 10,
            takerFeeBps = 15,
            privateKey = ByteArray(32) { 0xB7.toByte() },
        )
        val askOutcome = SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST,
            ask,
            generatedAtUnix = 123,
        )
        assertTrue(askOutcome.contains("\"status\": \"Ok\""), askOutcome)

        val bidProviderError = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                side = SorafsOrderbookSide.BID,
                tier = SorafsOrderbookTier.HOT,
                pricePerGib = "1",
                quantityGib = 1,
                ownerAccount = owner,
                providerId = ByteArray(32) { 0x72 },
                expiryUnix = 1_800_000_000,
                nonce = 17,
                makerFeeBps = 0,
                takerFeeBps = 0,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(bidProviderError.message.orEmpty().contains("absent or empty"))

        val askProviderError = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                side = SorafsOrderbookSide.ASK,
                tier = SorafsOrderbookTier.HOT,
                pricePerGib = "1",
                quantityGib = 1,
                ownerAccount = owner,
                providerId = null,
                expiryUnix = 1_800_000_000,
                nonce = 17,
                makerFeeBps = 0,
                takerFeeBps = 0,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(askProviderError.message.orEmpty().contains("providerId"))

        assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                orderId = ByteArray(32) { 0x11.toByte() },
                side = SorafsOrderbookSide.BID,
                tier = SorafsOrderbookTier.HOT,
                pricePerGib = "0.000000001",
                quantityGib = 64,
                ownerAccount = owner,
                providerId = null,
                expiryUnix = 1_800_000_000,
                nonce = 7,
                makerFeeBps = 10,
                takerFeeBps = 15,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
    }

    private fun assertPdpOutcome(name: String, actual: String) {
        assertEquals(
            fixture(
                "sorafs_manifest",
                "pdp",
                "negative",
                "${name}_validation_outcome_v1.json",
            ).toString(Charsets.UTF_8),
            actual,
            name,
        )
    }

    private fun fixture(vararg parts: String): ByteArray {
        val cwd = Paths.get(System.getProperty("user.dir")).toAbsolutePath()
        val relative = parts.fold(Paths.get("fixtures")) { path, part -> path.resolve(part) }
        val candidates = listOf(
            cwd.resolve(relative),
            cwd.resolve("..").resolve(relative),
            cwd.resolve("..").resolve("..").resolve(relative),
        )
        val path = candidates.firstOrNull { Files.exists(it) }
            ?: throw IllegalStateException("missing fixture ${relative.joinToString("/")}")
        return Files.readAllBytes(path.normalizeAbsolute())
    }

    private fun ByteArray.toHex(): String {
        val alphabet = "0123456789abcdef"
        val output = StringBuilder(size * 2)
        for (byte in this) {
            val value = byte.toInt() and 0xff
            output.append(alphabet[value ushr 4])
            output.append(alphabet[value and 0x0f])
        }
        return output.toString()
    }

    private fun Path.normalizeAbsolute(): Path = toAbsolutePath().normalize()
}
