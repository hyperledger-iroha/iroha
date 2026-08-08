package org.hyperledger.iroha.sdk.alias

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class AliasSetupModelsTest {
    @Test
    fun parsesCatalogFreeAccountAliasForms() {
        val fixture = sharedAliasFixture()
        @Suppress("UNCHECKED_CAST")
        val cases = fixture["account_alias_cases"] as List<Map<String, Any?>>
        cases.forEach { case ->
            val parsed = AccountAliasName.parse(case.getValue("input") as String)
            assertEquals(case["canonical"], parsed.canonicalText())
            assertEquals(case["label"], parsed.label)
            assertEquals(case["domain"], parsed.domain)
            assertEquals(case["dataspace"], parsed.dataspace)
        }

        val qualified = AccountAliasName.parse("Merchant@Banka.Paynet")
        assertEquals("merchant", qualified.label)
        assertEquals("banka", qualified.domain)
        assertEquals("paynet", qualified.dataspace)
        assertEquals("merchant@banka.paynet", qualified.canonicalText())

        val root = AccountAliasName.parse("Merchant@Paynet")
        assertEquals(null, root.domain)
        assertEquals("merchant@paynet", root.canonicalText())

        val idn = AccountAliasName.parse("merchant@例え")
        assertEquals("merchant@xn--r8jz45g", idn.canonicalText())
        assertEquals(
            "{\"dataspace\":\"paynet\",\"domain\":\"banka\",\"label\":\"merchant\"}",
            JsonEncoder.encode(qualified.toJsonMap()),
        )
    }

    @Test
    fun rejectsAmbiguousOrInvalidAccountAliasForms() {
        listOf(
            "",
            " merchant@paynet",
            "merchant",
            "merchant@",
            "@paynet",
            "merchant@@paynet",
            "merchant@a.b.c",
            "merchant@.paynet",
            "merchant@paynet.",
        ).forEach { input ->
            assertFailsWith<IllegalArgumentException>(input) { AccountAliasName.parse(input) }
        }
    }

    @Test
    fun resolvedNamesPinFullUnsignedDataspaceIds() {
        val max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val dataspace = ResolvedDataSpaceV1("Paynet", max)
        val domain = ResolvedDomainV1("Banka.Paynet", max)
        val alias = ResolvedAccountAliasV1("Merchant@Banka.Paynet", max)

        assertEquals("paynet", dataspace.canonicalName)
        assertEquals("banka.paynet", domain.canonicalName)
        assertEquals(dataspace, domain.parentDataspace())
        assertEquals(domain, alias.parentDomain())
        assertEquals(max, alias.toJsonMap()["dataspace_id"])

        assertFailsWith<IllegalArgumentException> {
            ResolvedDataSpaceV1("paynet", max.add(BigInteger.ONE))
        }
    }

    @Test
    fun ensureAliasUsesVersionedNoritoJsonShapes() {
        val alias = resolvedAlias()
        val ensure = EnsureAlias(
            AliasIntentV1.AccountAlias(
                AliasAccountIntentV1(
                    alias,
                    account(0x22),
                    AccountProvisionV1.CREATE,
                    AccountAliasRoleV1.PRIMARY,
                ),
            ),
            AliasLeaseAcquisitionV1(1),
            guard(),
        )
        val json = JsonEncoder.encode(ensure.toJsonMap())
        assertTrue(json.contains("\"kind\":\"account_alias\""))
        assertTrue(json.contains("\"provision\":{\"kind\":\"create\",\"value\":null}"))
        assertTrue(json.contains("\"quote_guard\""))
        assertEquals("iroha.alias.ensure", EnsureAlias.WIRE_ID)
    }

    @Test
    fun lifecycleBuildersUseCasAndNeverCarryBindingLeaseExpiry() {
        val alias = resolvedAlias()
        val target = AliasTargetV1.AccountAlias(alias)
        val renewal = RenewAliasLease(target, 1_000, 2_000, guard())
        assertEquals("iroha.alias.lease.renew", RenewAliasLease.WIRE_ID)
        assertEquals(1_000L, renewal.toJsonMap()["expected_current_expiry_ms"])
        assertEquals(2_000L, renewal.toJsonMap()["target_expiry_ms"])

        val config = AliasAutoRenewConfigV1(
            1,
            3,
            asset(),
            "5",
            86_400_000,
            60_000,
            5,
        )
        val enabled = ConfigureAliasAutoRenew(target, 4, config)
        val disabled = ConfigureAliasAutoRenew(target, 5, null)
        assertEquals("iroha.alias.auto_renew.configure", ConfigureAliasAutoRenew.WIRE_ID)
        assertEquals(config.toJsonMap(), enabled.toJsonMap()["config"])
        assertEquals(null, disabled.toJsonMap()["config"])

        val rebind = RebindAccountAlias(alias, account(0x22), account(0x33))
        val primary = CompareAndSetPrimaryAccountAlias(account(0x33), alias, null)
        assertFalse(rebind.toJsonMap().containsKey("lease_expiry_ms"))
        assertFalse(primary.toJsonMap().containsKey("lease_expiry_ms"))
        assertEquals("iroha.account.alias.rebind", RebindAccountAlias.WIRE_ID)
        assertEquals(
            "iroha.account.alias.primary.compare_and_set",
            CompareAndSetPrimaryAccountAlias.WIRE_ID,
        )
    }

    @Test
    fun verifiesPlanHashAndExactInstructionFrames() {
        val fixture = sharedAliasFixture()
        @Suppress("UNCHECKED_CAST")
        val hashVectors = fixture.getValue("plan_hash_vectors") as List<Map<String, Any?>>
        val hashVector = hashVectors.first { it["name"] == "setup_account_alias_create" }
        val bodyBytes = decodeHex(hashVector.getValue("canonical_body_norito_hex") as String)
        val plan = createPlan(bodyBytes)

        assertEquals(
            hashVector["canonical_plan_hash_hex"],
            hex(AliasPlanVerifier.canonicalHash(bodyBytes)),
        )
        assertTrue(AliasPlanVerifier.verifyHash(plan, bodyBytes))
        assertFalse(AliasPlanVerifier.verifyHash(plan, bodyBytes + 1))
        assertEquals(emptyList(), AliasPlanVerifier.validateExecutable(plan))
        assertTrue(
            AliasPlanVerifier.verifyExactFrames(plan) { _, payload -> payload.copyOf() },
        )
        assertFalse(
            AliasPlanVerifier.verifyExactFrames(plan) { _, payload ->
                payload.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }
            },
        )
        AliasPlanVerifier.requireExecutable(plan, bodyBytes) { _, payload -> payload.copyOf() }
    }

    @Test
    fun validatesEverySharedRustFrameAndLifecycleHash() {
        val fixture = sharedAliasFixture()
        @Suppress("UNCHECKED_CAST")
        val frames = fixture.getValue("instruction_frame_vectors") as List<Map<String, Any?>>
        val expectedWireIds = mapOf(
            "ensure_account_alias" to EnsureAlias.WIRE_ID,
            "renew_account_alias" to RenewAliasLease.WIRE_ID,
            "configure_auto_renew_enable" to ConfigureAliasAutoRenew.WIRE_ID,
            "configure_auto_renew_disable" to ConfigureAliasAutoRenew.WIRE_ID,
            "rebind_account_alias" to RebindAccountAlias.WIRE_ID,
            "compare_and_set_primary_account_alias" to CompareAndSetPrimaryAccountAlias.WIRE_ID,
        )
        assertEquals(expectedWireIds.keys, frames.map { it.getValue("name") as String }.toSet())
        frames.forEach { vector ->
            val name = vector.getValue("name") as String
            assertEquals(expectedWireIds.getValue(name), vector["wire_id"])
            val original = decodeHex(vector.getValue("framed_payload_hex") as String)
            val decoded = NoritoHeader.decode(original, null)
            decoded.header.validateChecksum(decoded.payload)
            val paddingLength = original.size - NoritoHeader.HEADER_LENGTH - decoded.payload.size
            assertTrue(paddingLength >= 0)
            val reencoded = decoded.header.encode() + ByteArray(paddingLength) + decoded.payload
            assertContentEquals(original, reencoded)
        }

        @Suppress("UNCHECKED_CAST")
        val hashes = fixture.getValue("plan_hash_vectors") as List<Map<String, Any?>>
        val lifecycle = hashes.first { it["name"] == "renew_account_alias" }
        val lifecycleBody = decodeHex(lifecycle.getValue("canonical_body_norito_hex") as String)
        assertEquals(
            lifecycle["canonical_plan_hash_hex"],
            hex(AliasPlanVerifier.canonicalLifecycleHash(lifecycleBody)),
        )
    }

    @Test
    fun sharedRustPlanBodiesAndTypedFramesDecodeAndReencodeCanonically() {
        val fixture = sharedAliasFixture()
        @Suppress("UNCHECKED_CAST")
        val hashes = fixture.getValue("plan_hash_vectors") as List<Map<String, Any?>>

        val setup = hashes.first { it["name"] == "setup_account_alias_create" }
        val setupBytes = decodeHex(setup.getValue("canonical_body_norito_hex") as String)
        val setupBody =
            AliasNoritoCodec.decodePlanBody(
                setupBytes,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        assertContentEquals(setupBytes, AliasNoritoCodec.encodePlanBody(setupBody))
        assertEquals(setup["canonical_plan_hash_hex"], hex(AliasPlanVerifier.canonicalHash(setupBytes)))

        val lifecycle = hashes.first { it["name"] == "renew_account_alias" }
        val lifecycleBytes = decodeHex(lifecycle.getValue("canonical_body_norito_hex") as String)
        val lifecycleBody =
            AliasNoritoCodec.decodeLifecyclePlanBody(
                lifecycleBytes,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        assertContentEquals(lifecycleBytes, AliasNoritoCodec.encodeLifecyclePlanBody(lifecycleBody))
        assertEquals(
            lifecycle["canonical_plan_hash_hex"],
            hex(AliasPlanVerifier.canonicalLifecycleHash(lifecycleBytes)),
        )

        @Suppress("UNCHECKED_CAST")
        val frames = fixture.getValue("instruction_frame_vectors") as List<Map<String, Any?>>
        frames.forEach { vector ->
            val original = decodeHex(vector.getValue("framed_payload_hex") as String)
            val reencoded = when (vector.getValue("name")) {
                "ensure_account_alias" -> AliasNoritoCodec.encodeEnsureAliasFrame(
                    AliasNoritoCodec.decodeEnsureAliasFrame(
                        original,
                        AccountAddress.DEFAULT_I105_DISCRIMINANT,
                    ),
                )
                "renew_account_alias" -> AliasNoritoCodec.encodeRenewAliasLeaseFrame(
                    AliasNoritoCodec.decodeRenewAliasLeaseFrame(
                        original,
                        AccountAddress.DEFAULT_I105_DISCRIMINANT,
                    ),
                )
                "configure_auto_renew_enable", "configure_auto_renew_disable" ->
                    AliasNoritoCodec.encodeConfigureAutoRenewFrame(
                        AliasNoritoCodec.decodeConfigureAutoRenewFrame(
                            original,
                            AccountAddress.DEFAULT_I105_DISCRIMINANT,
                        ),
                    )
                "rebind_account_alias" -> AliasNoritoCodec.encodeRebindAccountAliasFrame(
                    AliasNoritoCodec.decodeRebindAccountAliasFrame(
                        original,
                        AccountAddress.DEFAULT_I105_DISCRIMINANT,
                    ),
                )
                "compare_and_set_primary_account_alias" ->
                    AliasNoritoCodec.encodeCompareAndSetPrimaryAliasFrame(
                        AliasNoritoCodec.decodeCompareAndSetPrimaryAliasFrame(
                            original,
                            AccountAddress.DEFAULT_I105_DISCRIMINANT,
                        ),
                    )
                else -> error("unexpected shared alias frame")
            }
            assertContentEquals(original, reencoded, vector.getValue("name") as String)
        }

        val setupPlan = AliasTransactionPlanV1(
            setupBody,
            setup.getValue("canonical_plan_hash_hex") as String,
        )
        val ensure = AliasNoritoCodec.decodeEnsureAliasFrame(
            setupBody.instructions.single().framedPayload,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
        )
        val setupTransaction = AliasPlanApply.buildTransactionPayload(
            AliasSetupPlanRequestV1(listOf(ensure)),
            setupPlan,
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(emptyList()),
            creationTimeMs = 40_000,
        )
        assertEquals(setupBody.instructions.size, (setupTransaction.executable as Executable.Instructions).instructions.size)

        val lifecyclePlan = AliasLifecycleTransactionPlanV1(
            lifecycleBody,
            lifecycle.getValue("canonical_plan_hash_hex") as String,
        )
        val renewal = (lifecycleBody.operation as AliasLifecycleOperationV1.RenewLease).renewal
        val lifecycleTransaction = AliasLifecyclePlanApply.buildTransactionPayload(
            AliasLeaseRenewPlanRequestV1(renewal),
            lifecyclePlan,
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(emptyList()),
            creationTimeMs = 40_000,
        )
        assertEquals(1, (lifecycleTransaction.executable as Executable.Instructions).instructions.size)
    }

    @Test
    fun sharedBlockedReportUsesTheTypedSecretFreeShape() {
        val fixture = sharedAliasFixture()
        @Suppress("UNCHECKED_CAST")
        val expected = fixture.getValue("report_json_vector") as Map<String, Any?>
        val report = AliasSetupReportV1(
            AliasSetupStatusV1.BLOCKED,
            listOf(
                AliasSetupDiagnosticV1(
                    AliasSetupValidationPhaseV1.CATALOG,
                    "alias.catalog.mapping_conflict",
                    AliasSetupSeverityV1.ERROR,
                    resource = "dataspace:paynet",
                    expected = "7",
                    actual = "9",
                    remediation = "Make the static catalog and active SNS record map paynet to the same dataspace ID.",
                ),
            ),
        )
        assertEquals(JsonEncoder.encode(expected), JsonEncoder.encode(report.toJsonMap()))
    }

    @Test
    fun sharedResolvedNamesQuoteGuardAndExactPermissionMatchTypedJson() {
        val fixture = sharedAliasFixture()
        @Suppress("UNCHECKED_CAST")
        val names = fixture.getValue("resolved_name_json_vectors") as Map<String, Any?>
        val alias = resolvedAlias()
        val typedNames = linkedMapOf(
            "dataspace" to ResolvedDataSpaceV1("paynet", 7L).toJsonMap(),
            "domain" to ResolvedDomainV1("banka.paynet", 7L).toJsonMap(),
            "account_alias" to alias.toJsonMap(),
        )
        assertEquals(JsonEncoder.encode(names), JsonEncoder.encode(typedNames))

        @Suppress("UNCHECKED_CAST")
        val quote = fixture.getValue("quote_guard_json_vector") as Map<String, Any?>
        val typedQuote = AliasQuoteGuardV1(
            2,
            "4rPeAP6jAjiLVZThZYwwPRBuQagt",
            "10",
            50_000,
        )
        assertEquals(JsonEncoder.encode(quote), JsonEncoder.encode(typedQuote.toJsonMap()))

        @Suppress("UNCHECKED_CAST")
        val permission = fixture.getValue("permission_scope_json_vector") as Map<String, Any?>
        val typedPermission = AccountAliasPermissionScope.Alias(alias)
        assertEquals(JsonEncoder.encode(permission), JsonEncoder.encode(typedPermission.toJsonMap()))
    }

    @Test
    fun parsesTypedPlanAndBuildsOneOrdinaryTransactionAfterRequestBoundVerification() {
        val bodyBytes = byteArrayOf(1, 3, 3, 7)
        val original = createPlan(bodyBytes)
        val checksummed = AliasTransactionPlanV1(
            original.body,
            HashLiteral.canonicalize(AliasPlanVerifier.canonicalHash(bodyBytes)),
        )
        val canonicalJson = JsonEncoder.encode(checksummed.toJsonMap())
        assertTrue("\"network_id\":\"${TEST_NETWORK_ID.literal}\"" in canonicalJson)
        assertFalse("\"chain_id\"" in canonicalJson)
        val parsed = AliasTransactionPlanJsonParser.parse(
            canonicalJson.toByteArray(StandardCharsets.UTF_8),
        )
        assertFailsWith<IllegalStateException> {
            AliasTransactionPlanJsonParser.parse(
                canonicalJson.replace("\"network_id\"", "\"chain_id\"")
                    .toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertEquals(checksummed, parsed)
        assertTrue(AliasPlanVerifier.verifyHash(parsed, bodyBytes))

        val ensure = EnsureAlias(
            parsed.body.resources.single().intent,
            AliasLeaseAcquisitionV1(1),
            guard(),
        )
        val request = AliasSetupPlanRequestV1(listOf(ensure))
        val payload = AliasPlanApply.buildTransactionPayload(
            request,
            parsed,
            AliasPlanBodyNoritoEncoder { bodyBytes.copyOf() },
            AliasEnsureInstructionFrameCodec { wireId, frame, chainDiscriminant ->
                assertEquals(EnsureAlias.WIRE_ID, wireId)
                assertEquals(AccountAddress.DEFAULT_I105_DISCRIMINANT, chainDiscriminant)
                DecodedEnsureAliasFrame(ensure, frame.copyOf())
            },
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(emptyList()),
            creationTimeMs = 40_000,
            nonce = 7,
        )

        assertEquals(TEST_NETWORK_ID, payload.networkId)
        assertEquals(TEST_NETWORK_ID, parsed.body.networkId)
        assertEquals(parsed.body.authority, payload.authority)
        assertEquals(9_000L, payload.timeToLiveMs)
        val executable = payload.executable as Executable.Instructions
        assertEquals(1, executable.instructions.size)
        assertEquals(EnsureAlias.WIRE_ID, executable.instructions.single().name)
        assertFailsWith<IllegalArgumentException> {
            AliasPlanApply.buildTransactionPayload(
                request,
                parsed,
                AliasPlanBodyNoritoEncoder { bodyBytes.copyOf() },
                AliasEnsureInstructionFrameCodec { _, frame, _ ->
                    DecodedEnsureAliasFrame(ensure, frame.copyOf())
                },
                OTHER_NETWORK_ID,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
                FeePaymentIntent.authority(emptyList()),
                creationTimeMs = 40_000,
            )
        }
    }

    @Test
    fun requestBoundVerificationRejectsSubstitutedAcquisitionTerms() {
        val bodyBytes = byteArrayOf(4, 2)
        val plan = createPlan(bodyBytes)
        val intended = EnsureAlias(
            plan.body.resources.single().intent,
            AliasLeaseAcquisitionV1(1),
            guard(),
        )
        val substituted = EnsureAlias(
            plan.body.resources.single().intent,
            AliasLeaseAcquisitionV1(2),
            guard(),
        )
        assertFailsWith<IllegalArgumentException> {
            AliasPlanVerifier.requireExecutableForRequest(
                AliasSetupPlanRequestV1(listOf(intended)),
                plan,
                bodyBytes,
                AliasEnsureInstructionFrameCodec { _, frame, _ ->
                    DecodedEnsureAliasFrame(substituted, frame)
                },
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        }
    }

    @Test
    fun parsesVerifiesAndBuildsLifecyclePlansWithoutMutationRoutes() {
        val bodyBytes = byteArrayOf(7, 1, 9, 4)
        val target = AliasTargetV1.AccountAlias(resolvedAlias())
        val renewal = RenewAliasLease(target, 1_000, 2_000, guard())
        val request = AliasLeaseRenewPlanRequestV1(renewal)
        val frame = AliasFramedInstructionV1(RenewAliasLease.WIRE_ID, byteArrayOf(1, 2, 3))
        val quote = AliasLeaseQuoteV1(target, 1, "3", guard(), 2_000, 2_100, 2_200)
        val body = AliasLifecycleTransactionPlanBodyV1(
            AliasLifecycleTransactionPlanBodyV1.VERSION,
            account(0x11),
            TEST_NETWORK_ID,
            AliasPlanAnchorV1(9, "01".repeat(32)),
            request.operation,
            AliasLifecyclePlanDispositionV1.APPLY,
            frame,
            quote,
            listOf(AliasAssetTotalV1(asset(), "3")),
            emptyList(),
            emptyList(),
            50_000,
        )
        val plan = AliasLifecycleTransactionPlanV1(
            body,
            hex(AliasPlanVerifier.canonicalLifecycleHash(bodyBytes)),
        )
        val canonicalJson = JsonEncoder.encode(plan.toJsonMap())
        assertTrue("\"network_id\":\"${TEST_NETWORK_ID.literal}\"" in canonicalJson)
        assertFalse("\"chain_id\"" in canonicalJson)
        val parsed = AliasLifecycleTransactionPlanJsonParser.parse(
            canonicalJson.toByteArray(StandardCharsets.UTF_8),
        )
        assertFailsWith<IllegalStateException> {
            AliasLifecycleTransactionPlanJsonParser.parse(
                canonicalJson.replace("\"network_id\"", "\"chain_id\"")
                    .toByteArray(StandardCharsets.UTF_8),
            )
        }

        assertEquals(plan, parsed)
        assertEquals(emptyList(), AliasPlanVerifier.validateLifecycleExecutable(parsed))
        AliasPlanVerifier.requireLifecycleExecutableForRequest(
            request,
            parsed,
            bodyBytes,
            AliasLifecycleInstructionFrameCodec { wireId, payload, chainDiscriminant ->
                assertEquals(RenewAliasLease.WIRE_ID, wireId)
                assertEquals(AccountAddress.DEFAULT_I105_DISCRIMINANT, chainDiscriminant)
                DecodedAliasLifecycleFrame(request.operation, payload.copyOf())
            },
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
        )
        val transaction = AliasLifecyclePlanApply.buildTransactionPayload(
            request,
            parsed,
            AliasLifecyclePlanBodyNoritoEncoder { bodyBytes.copyOf() },
            AliasLifecycleInstructionFrameCodec { _, payload, _ ->
                DecodedAliasLifecycleFrame(request.operation, payload.copyOf())
            },
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(emptyList()),
            creationTimeMs = 40_000,
        )
        val executable = transaction.executable as Executable.Instructions
        assertEquals(RenewAliasLease.WIRE_ID, executable.instructions.single().name)
        assertFailsWith<IllegalArgumentException> {
            AliasLifecyclePlanApply.buildTransactionPayload(
                request,
                parsed,
                AliasLifecyclePlanBodyNoritoEncoder { bodyBytes.copyOf() },
                AliasLifecycleInstructionFrameCodec { _, payload, _ ->
                    DecodedAliasLifecycleFrame(request.operation, payload.copyOf())
                },
                OTHER_NETWORK_ID,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
                FeePaymentIntent.authority(emptyList()),
                creationTimeMs = 40_000,
            )
        }
    }

    @Test
    fun autoRenewNoOpHasNoFrameChargeOrSubmission() {
        val bodyBytes = byteArrayOf(2, 4, 6)
        val request = AliasAutoRenewPlanRequestV1(
            ConfigureAliasAutoRenew(AliasTargetV1.AccountAlias(resolvedAlias()), 5, null),
        )
        val body = AliasLifecycleTransactionPlanBodyV1(
            1,
            account(0x11),
            TEST_NETWORK_ID,
            AliasPlanAnchorV1(9, "01".repeat(32)),
            request.operation,
            AliasLifecyclePlanDispositionV1.NO_OP,
            null,
            null,
            emptyList(),
            emptyList(),
            emptyList(),
            50_000,
        )
        val plan = AliasLifecycleTransactionPlanV1(
            body,
            hex(AliasPlanVerifier.canonicalLifecycleHash(bodyBytes)),
        )
        assertEquals(emptyList(), AliasPlanVerifier.validateLifecycleExecutable(plan))
        AliasPlanVerifier.requireLifecycleExecutableForRequest(
            request,
            plan,
            bodyBytes,
            AliasLifecycleInstructionFrameCodec { _, _, _ ->
                error("no-op must not decode an instruction")
            },
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
        )
        assertFailsWith<IllegalArgumentException> {
            AliasLifecyclePlanApply.buildTransactionPayload(
                request,
                plan,
                AliasLifecyclePlanBodyNoritoEncoder { bodyBytes },
                AliasLifecycleInstructionFrameCodec { _, _, _ -> error("unreachable") },
                TEST_NETWORK_ID,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
                FeePaymentIntent.authority(emptyList()),
                creationTimeMs = 40_000,
            )
        }
    }

    @Test
    fun sponsoredOnboardingReceiptIsTypedSecretFreeAndRoundTripsForApply() {
        val intent = AliasIntentV1.AccountAlias(
            AliasAccountIntentV1(
                resolvedAlias(),
                account(0x22),
                AccountProvisionV1.CREATE,
                AccountAliasRoleV1.PRIMARY,
            ),
        )
        val request = AccountOnboardingPlanRequestV1(
            "Merchant@Banka.Paynet",
            account(0x22),
            listOf("CanSetMetadata", "CanSetMetadata"),
        )
        val body = AccountOnboardingPlanBodyV1(
            1,
            request,
            account(0x11),
            "test-chain",
            AliasPlanAnchorV1(9, "01".repeat(32)),
            AliasPlanResourceV1(intent, AliasPlanDispositionV1.CREATE, null, 0),
            AliasLeaseAcquisitionV1(1),
            guard(),
            listOf(AliasFramedInstructionV1(EnsureAlias.WIRE_ID, byteArrayOf(1, 2, 3))),
            null,
            50_000,
        )
        val receipt = AccountOnboardingPlanReceiptV1(body, "03".repeat(32), "AA")
        val encoded = JsonEncoder.encode(receipt.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        assertEquals(receipt, AccountOnboardingJsonParser.parseReceipt(encoded))
        val apply = AccountOnboardingApplyRequestV1(receipt).toJsonMap()
        assertFalse(JsonEncoder.encode(apply).contains("token"))
        assertFalse(JsonEncoder.encode(apply).contains("private_key"))

        val response = AccountOnboardingJsonParser.parseResponse(
            """{"account_id":"${account(0x22)}","alias":"merchant@banka.paynet","status":"Unchanged","disposition":{"kind":"no_op","value":null}}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        assertEquals(AccountOnboardingStatusV1.UNCHANGED, response.status)
        assertEquals(null, response.transactionHashHex)

        val readiness = AccountOnboardingJsonParser.parseReadiness(
            """{"version":1,"status":{"status":"ready","value":null},"diagnostics":[]}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        assertEquals(AliasSetupStatusV1.READY, readiness.status)
        assertTrue(readiness.diagnostics.isEmpty())
    }

    @Test
    fun onboardingResponseRequiresExactStatusHashAndDispositionSemantics() {
        val hash = "ab".repeat(32)
        val account = account(0x22)

        AccountOnboardingResponseV1(
            account,
            "merchant@banka.paynet",
            hash,
            AccountOnboardingStatusV1.QUEUED,
            AliasPlanDispositionV1.CREATE,
        )
        AccountOnboardingResponseV1(
            account,
            "merchant@banka.paynet",
            hash,
            AccountOnboardingStatusV1.REPAIRED,
            AliasPlanDispositionV1.REPAIR,
        )
        AccountOnboardingResponseV1(
            account,
            "merchant@banka.paynet",
            hash,
            AccountOnboardingStatusV1.REPAIRED,
            AliasPlanDispositionV1.NO_OP,
        )
        AccountOnboardingResponseV1(
            account,
            "merchant@banka.paynet",
            null,
            AccountOnboardingStatusV1.UNCHANGED,
            AliasPlanDispositionV1.NO_OP,
        )

        listOf(
            Triple(AccountOnboardingStatusV1.QUEUED, AliasPlanDispositionV1.REPAIR, hash),
            Triple(AccountOnboardingStatusV1.REPAIRED, AliasPlanDispositionV1.CREATE, hash),
            Triple(AccountOnboardingStatusV1.UNCHANGED, AliasPlanDispositionV1.NO_OP, hash),
            Triple(AccountOnboardingStatusV1.QUEUED, AliasPlanDispositionV1.CREATE, null),
        ).forEach { (status, disposition, transactionHash) ->
            assertFailsWith<IllegalArgumentException> {
                AccountOnboardingResponseV1(
                    account,
                    "merchant@banka.paynet",
                    transactionHash,
                    status,
                    disposition,
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingResponseV1(
                account,
                "Merchant@Banka.Paynet",
                null,
                AccountOnboardingStatusV1.UNCHANGED,
                AliasPlanDispositionV1.NO_OP,
            )
        }
    }

    @Test
    fun onboardingResponseVerifierBindsReceiptAndHttpStatus() {
        val createReceipt = AccountOnboardingPlanReceiptV1(
            onboardingBody(account(0x11)),
            "03".repeat(32),
            "AA",
        )
        val unchanged = AccountOnboardingResponseV1(
            account(0x22),
            "merchant@banka.paynet",
            null,
            AccountOnboardingStatusV1.UNCHANGED,
            AliasPlanDispositionV1.NO_OP,
        )
        assertEquals(
            unchanged,
            AccountOnboardingResponseVerifier.requireValidForReceipt(
                createReceipt,
                unchanged,
                200,
            ),
        )

        val queued = AccountOnboardingResponseV1(
            account(0x22),
            "merchant@banka.paynet",
            "ab".repeat(32),
            AccountOnboardingStatusV1.QUEUED,
            AliasPlanDispositionV1.CREATE,
        )
        assertEquals(
            queued,
            AccountOnboardingResponseVerifier.requireValidForReceipt(createReceipt, queued, 202),
        )

        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingResponseVerifier.requireValidForReceipt(createReceipt, queued, 200)
        }
        val substituted = AccountOnboardingResponseV1(
            account(0x23),
            "merchant@banka.paynet",
            null,
            AccountOnboardingStatusV1.UNCHANGED,
            AliasPlanDispositionV1.NO_OP,
        )
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingResponseVerifier.requireValidForReceipt(
                createReceipt,
                substituted,
                200,
            )
        }

        val noOpReceipt = AccountOnboardingPlanReceiptV1(
            onboardingBody(account(0x11), disposition = AliasPlanDispositionV1.NO_OP),
            "04".repeat(32),
            "AA",
        )
        val ancillaryRepair = AccountOnboardingResponseV1(
            account(0x22),
            "merchant@banka.paynet",
            "cd".repeat(32),
            AccountOnboardingStatusV1.REPAIRED,
            AliasPlanDispositionV1.NO_OP,
        )
        assertEquals(
            ancillaryRepair,
            AccountOnboardingResponseVerifier.requireValidForReceipt(
                noOpReceipt,
                ancillaryRepair,
                202,
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingResponseVerifier.requireValidForReceipt(noOpReceipt, queued, 202)
        }
    }

    @Test
    fun onboardingReceiptVerifiesCanonicalBodyAndRejectsTamperOrWrongAuthority() {
        val signer = Ed25519PrivateKeyParameters(ByteArray(32) { 0x51.toByte() }, 0)
        val authority = AccountAddress.fromAccount(signer.generatePublicKey().encoded, "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val body = onboardingBody(authority)
        val encoded = AliasNoritoCodec.encodeOnboardingPlanBody(body)
        assertContentEquals(
            encoded,
            AliasNoritoCodec.encodeOnboardingPlanBody(
                AliasNoritoCodec.decodeOnboardingPlanBody(
                    encoded,
                    AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            ),
        )

        val receipt = signedOnboardingReceipt(body, signer)
        assertTrue(AccountOnboardingReceiptVerifier.verify(receipt))
        assertEquals(receipt, AccountOnboardingReceiptVerifier.requireValidForRequest(body.request, receipt))

        val tampered = AccountOnboardingPlanReceiptV1(
            onboardingBody(authority, "other-chain"),
            receipt.planHash,
            receipt.signature,
        )
        assertFalse(AccountOnboardingReceiptVerifier.verify(tampered))

        val wrongSigner = Ed25519PrivateKeyParameters(ByteArray(32) { 0x52.toByte() }, 0)
        val wrongAuthority = AccountAddress.fromAccount(
            wrongSigner.generatePublicKey().encoded,
            "ed25519",
        ).toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val wrongAuthorityReceipt = signedOnboardingReceipt(onboardingBody(wrongAuthority), signer)
        assertFalse(AccountOnboardingReceiptVerifier.verify(wrongAuthorityReceipt))

        val substitutedSelfSignedReceipt = signedOnboardingReceipt(
            onboardingBody(wrongAuthority),
            wrongSigner,
        )
        assertTrue(AccountOnboardingReceiptVerifier.verify(substitutedSelfSignedReceipt))
        assertFalse(
            AccountOnboardingReceiptVerifier.verify(substitutedSelfSignedReceipt, authority),
        )
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingReceiptVerifier.requireValid(
                substitutedSelfSignedReceipt,
                authority,
            )
        }
    }

    @Test
    fun sharedRustOnboardingReceiptReencodesAndVerifiesExactly() {
        val fixture = sharedAliasFixture()
        @Suppress("UNCHECKED_CAST")
        val vector = fixture.getValue("account_onboarding_receipt_vector") as Map<String, Any?>
        val bodyBytes = decodeHex(vector.getValue("canonical_body_norito_hex") as String)
        val body =
            AliasNoritoCodec.decodeOnboardingPlanBody(
                bodyBytes,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        assertContentEquals(bodyBytes, AliasNoritoCodec.encodeOnboardingPlanBody(body))
        assertEquals(
            vector.getValue("canonical_plan_hash_hex"),
            hex(AccountOnboardingReceiptVerifier.canonicalHash(body)),
        )

        @Suppress("UNCHECKED_CAST")
        val receiptJson = vector.getValue("receipt_json") as Map<String, Any?>
        val receipt = AccountOnboardingJsonParser.parseReceipt(
            JsonEncoder.encode(receiptJson).toByteArray(StandardCharsets.UTF_8),
        )
        assertContentEquals(
            bodyBytes,
            AliasNoritoCodec.encodeOnboardingPlanBody(receipt.body),
        )
        assertEquals(vector.getValue("authority"), receipt.body.authority)
        assertEquals(vector.getValue("signature_hex"), receipt.signature)
        assertTrue(
            AccountOnboardingReceiptVerifier.verify(
                receipt,
                vector.getValue("authority") as String,
            ),
        )

        val tamperedSignature = receipt.signature.toCharArray().also {
            it[0] = if (it[0] == '0') '1' else '0'
        }.concatToString()
        assertFalse(
            AccountOnboardingReceiptVerifier.verify(
                AccountOnboardingPlanReceiptV1(receipt.body, receipt.planHash, tamperedSignature),
                receipt.body.authority,
            ),
        )
    }

    @Test
    fun validationRejectsConflictAndNonCanonicalOrdering() {
        val bodyBytes = byteArrayOf(9, 8, 7)
        val valid = createPlan(bodyBytes)
        val conflictResource = AliasPlanResourceV1(
            valid.body.resources.single().intent,
            AliasPlanDispositionV1.CONFLICT,
            null,
            null,
        )
        val dataspaceResource = AliasPlanResourceV1(
            AliasIntentV1.Dataspace(
                AliasDataSpaceIntentV1(ResolvedDataSpaceV1("paynet", 7L), account(0x11)),
            ),
            AliasPlanDispositionV1.NO_OP,
            null,
            null,
        )
        val body = AliasTransactionPlanBodyV1(
            1,
            valid.body.authority,
            valid.body.networkId,
            valid.body.anchor,
            listOf(conflictResource, dataspaceResource),
            emptyList(),
            emptyList(),
            emptyList(),
            emptyList(),
            49_000,
        )
        val plan = AliasTransactionPlanV1(body, hex(AliasPlanVerifier.canonicalHash(bodyBytes)))
        val errors = AliasPlanVerifier.validateExecutable(plan)
        assertTrue("alias.plan.conflict" in errors)
        assertTrue("alias.plan.resource_order_invalid" in errors)
    }

    @Test
    fun framedPayloadIsDefensivelyCopied() {
        val source = byteArrayOf(1, 2, 3)
        val frame = AliasFramedInstructionV1(EnsureAlias.WIRE_ID, source)
        source[0] = 9
        assertContentEquals(byteArrayOf(1, 2, 3), frame.framedPayload)
        val read = frame.framedPayload
        read[1] = 8
        assertContentEquals(byteArrayOf(1, 2, 3), frame.framedPayload)
    }

    private fun createPlan(canonicalBodyBytes: ByteArray): AliasTransactionPlanV1 {
        val alias = resolvedAlias()
        val intent = AliasIntentV1.AccountAlias(
            AliasAccountIntentV1(
                alias,
                account(0x22),
                AccountProvisionV1.CREATE,
                AccountAliasRoleV1.PRIMARY,
            ),
        )
        val frame = AliasFramedInstructionV1(EnsureAlias.WIRE_ID, sharedEnsureFrame())
        val quote = AliasLeaseQuoteV1(
            AliasTargetV1.AccountAlias(alias),
            1,
            "3",
            guard(),
            1_000,
            2_000,
            3_000,
        )
        val body = AliasTransactionPlanBodyV1(
            AliasTransactionPlanBodyV1.VERSION,
            account(0x11),
            TEST_NETWORK_ID,
            AliasPlanAnchorV1(9, "01".repeat(32)),
            listOf(AliasPlanResourceV1(intent, AliasPlanDispositionV1.CREATE, quote, 0L)),
            listOf(frame),
            listOf(AliasAssetTotalV1(asset(), "3")),
            emptyList(),
            emptyList(),
            49_000,
        )
        return AliasTransactionPlanV1(body, hex(AliasPlanVerifier.canonicalHash(canonicalBodyBytes)))
    }

    private fun onboardingBody(
        authority: String,
        chainId: String = "test-chain",
        disposition: AliasPlanDispositionV1 = AliasPlanDispositionV1.CREATE,
    ): AccountOnboardingPlanBodyV1 {
        val intent = AliasIntentV1.AccountAlias(
            AliasAccountIntentV1(
                resolvedAlias(),
                account(0x22),
                AccountProvisionV1.CREATE,
                AccountAliasRoleV1.PRIMARY,
            ),
        )
        return AccountOnboardingPlanBodyV1(
            1,
            AccountOnboardingPlanRequestV1(
                resolvedAlias().canonicalName.canonicalText(),
                account(0x22),
            ),
            authority,
            chainId,
            AliasPlanAnchorV1(9, "01".repeat(32)),
            AliasPlanResourceV1(
                intent,
                disposition,
                null,
                if (disposition == AliasPlanDispositionV1.NO_OP) null else 0,
            ),
            AliasLeaseAcquisitionV1(1),
            guard(),
            if (disposition == AliasPlanDispositionV1.NO_OP) {
                emptyList()
            } else {
                listOf(AliasFramedInstructionV1(EnsureAlias.WIRE_ID, sharedEnsureFrame()))
            },
            null,
            50_000,
        )
    }

    private fun signedOnboardingReceipt(
        body: AccountOnboardingPlanBodyV1,
        privateKey: Ed25519PrivateKeyParameters,
    ): AccountOnboardingPlanReceiptV1 {
        val hash = AccountOnboardingReceiptVerifier.canonicalHash(body)
        val signer = Ed25519Signer()
        signer.init(true, privateKey)
        signer.update(hash, 0, hash.size)
        return AccountOnboardingPlanReceiptV1(body, hex(hash), hex(signer.generateSignature()))
    }

    private fun guard(): AliasQuoteGuardV1 = AliasQuoteGuardV1(
        3,
        asset(),
        "5",
        50_000,
    )

    private fun resolvedAlias(): ResolvedAccountAliasV1 =
        ResolvedAccountAliasV1(AccountAliasName.parse("merchant@banka.paynet"), 7L)

    private fun account(fill: Int): String = AccountAddress
        .fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    private fun asset(): String {
        val bytes = ByteArray(16) { it.toByte() }
        bytes[6] = 0x46
        bytes[8] = 0x88.toByte()
        return AssetDefinitionIdEncoder.encodeFromBytes(bytes)
    }

    private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun decodeHex(value: String): ByteArray {
        require(value.length % 2 == 0) { "fixture hex must contain whole bytes" }
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun sharedEnsureFrame(): ByteArray {
        val frames = sharedAliasFixture().getValue("instruction_frame_vectors") as List<Map<String, Any?>>
        val ensure = frames.first { it["name"] == "ensure_account_alias" }
        return decodeHex(ensure.getValue("framed_payload_hex") as String)
    }

    @Suppress("UNCHECKED_CAST")
    private fun sharedAliasFixture(): Map<String, Any?> {
        var current: Path? = Paths.get(System.getProperty("user.dir")).toAbsolutePath()
        repeat(8) {
            val directory = current ?: error("repository root not found")
            val candidate = directory.resolve("fixtures/norito_rpc/alias_setup_v1/alias_setup_v1.json")
            if (Files.isRegularFile(candidate)) {
                val json = String(Files.readAllBytes(candidate), StandardCharsets.UTF_8)
                return JsonParser.parse(json) as Map<String, Any?>
            }
            current = directory.parent
        }
        error("shared alias setup fixture not found")
    }

    private companion object {
        private val TEST_NETWORK_ID = NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
        )
        private val OTHER_NETWORK_ID = NetworkId.fromBytes(ByteArray(NetworkId.BYTE_LENGTH) { 0xA5.toByte() })
    }
}
