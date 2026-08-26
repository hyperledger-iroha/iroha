// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.ExecutableBatchItem
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.core.model.instructions.InstructionKind
import org.hyperledger.iroha.sdk.crypto.Blake3
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.sccp.SccpV1
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

/** Rust-owned wire parity checks for the complete typed Musubi V1 mutation surface. */
class MusubiInstructionsV1FixtureTest {
    @Test
    fun `Musubi hashing supports payloads above the legacy helper limit`() {
        assertEquals(
            "b1fc3c3bf473596bc8ac1f5c86f77c2fc0e0186a872b88adf841716fe9140a50",
            Blake3.hash(ByteArray(100_000)).toHex(),
        )
    }

    @Test
    fun `Name ordering uses unsigned UTF-8 bytes`() {
        val packageId = MusubiPackageIdV1(
            BigInteger.ONE,
            MusubiPackageScopeV1.dataspaceRoot(),
            MusubiPackageNameV1("ordering"),
        )
        val requirement = MusubiVersionReqV1.parse("^1.0.0")
        val ascii = MusubiDependencyReqV1("z", packageId, requirement)
        val nonAscii = MusubiDependencyReqV1("é", packageId, requirement)

        assertTrue(ascii < nonAscii)
    }

    @Test
    fun `parent-local dependency aliases are unique across published graphs`() {
        val publication = cases(fixture())
            .first { it.string("id") == "publish-delegated-domain-release" }
            .objectValue("semantic")
            .let { parsePublication(it.objectValue("publication")) }
        val manifest = publication.manifest
        val lock = publication.resolution.lock
        val firstRequirement = manifest.dependencies.single()
        val firstEdge = lock.rootDependencies.single()
        val firstNode = lock.nodes.single()
        val secondPackage = MusubiPackageIdV1(
            firstRequirement.packageId.homeDataspace,
            firstRequirement.packageId.scope,
            MusubiPackageNameV1("vector-next"),
        )
        val secondRequirement = MusubiDependencyReqV1(
            firstRequirement.alias,
            secondPackage,
            firstRequirement.requirement,
        )
        val secondRelease = MusubiReleaseIdV1(secondPackage, firstEdge.selected.version)
        val secondEdge = MusubiExactDependencyEdgeV1(
            firstEdge.alias,
            firstEdge.kind,
            secondRelease.packageId,
            firstEdge.requirement,
            secondRelease,
        )
        val requirements = listOf(firstRequirement, secondRequirement).sorted()
        val edges = listOf(firstEdge, secondEdge).sorted()

        fun assertUniqueAliasFailure(block: () -> Unit) {
            val error = assertFailsWith<IllegalArgumentException>(block = block)
            assertTrue(error.message.orEmpty().contains("unique parent-local aliases"))
        }

        assertUniqueAliasFailure {
            MusubiReleaseManifestV1(
                manifest.release,
                manifest.edition,
                manifest.abi,
                requirements,
                manifest.exports,
                manifest.interfaceDigest,
                manifest.metadata,
                manifest.archiveId,
                manifest.verificationLockDigest,
            )
        }
        assertUniqueAliasFailure {
            MusubiVerificationNodeV1(
                firstNode.release,
                firstNode.releaseDigest,
                firstNode.archiveId,
                firstNode.sourceDigest,
                firstNode.interfaceDigest,
                firstNode.abi,
                edges,
            )
        }
        val secondNode = MusubiVerificationNodeV1(
            secondRelease,
            firstNode.releaseDigest,
            firstNode.archiveId,
            firstNode.sourceDigest,
            firstNode.interfaceDigest,
            firstNode.abi,
            emptyList(),
        )
        assertUniqueAliasFailure {
            MusubiVerificationLockV1(
                lock.root,
                edges,
                listOf(firstNode, secondNode).sorted(),
            )
        }
    }

    @Test
    fun `new mutation compare-and-set revisions reject zero`() {
        val byId = cases(fixture()).associateBy { it.string("id") }

        byId.getValue("register-archive-max-bounds-signed-receipt")
            .objectValue("semantic")
            .let { semantic ->
                assertFailsWith<IllegalArgumentException> {
                    MusubiInstructionsV1.RegisterMusubiArchiveV1(
                        parseArchiveCommitment(semantic.objectValue("commitment")),
                        parseSeedIngressReceipt(semantic.objectValue("staging_receipt")),
                        BigInteger.ZERO,
                    )
                }
            }
        byId.getValue("register-provider-bundle-attestation")
            .objectValue("semantic")
            .let { semantic ->
                assertFailsWith<IllegalArgumentException> {
                    MusubiInstructionsV1.RegisterMusubiProviderBundleAttestationV1(
                        parseProviderAttestation(semantic.objectValue("attestation")),
                        BigInteger.ZERO,
                    )
                }
            }
        byId.getValue("add-location-three-signed-providers")
            .objectValue("semantic")
            .let { semantic ->
                assertFailsWith<IllegalArgumentException> {
                    MusubiInstructionsV1.AddMusubiArchiveLocationV1(
                        parseDigest(semantic["archive_id"]),
                        parseDigest(semantic["location_id"]),
                        parseDigest(semantic["pin_manifest"]),
                        parseDigest(semantic["replication_order"]),
                        parseProviderAttestationSetDigest(
                            semantic["provider_attestation_set_digest"],
                        ),
                        semantic.bigInteger("renew_after_epoch"),
                        semantic.bigInteger("expires_at_epoch"),
                        BigInteger.ZERO,
                    )
                }
            }
        byId.getValue("publish-delegated-domain-release")
            .objectValue("semantic")
            .let { semantic ->
                assertFailsWith<IllegalArgumentException> {
                    MusubiInstructionsV1.PublishMusubiReleaseV1(
                        MusubiNamespaceV1(newtypeText(semantic["namespace"])),
                        parsePublication(semantic.objectValue("publication")),
                        semantic["namespace_delegation"]?.let {
                            parseNamespaceDelegation(it.objectValue())
                        },
                        BigInteger.ZERO,
                        semantic.optionalBigInteger("expected_governance_revision"),
                    )
                }
            }
        byId.getValue("replace-domain-metadata-high-revision")
            .objectValue("semantic")
            .let { semantic ->
                assertFailsWith<IllegalArgumentException> {
                    MusubiInstructionsV1.SetMusubiPackageMetadataV1(
                        parsePackage(semantic.objectValue("package")),
                        parseMetadata(semantic.objectValue("metadata")),
                        BigInteger.ZERO,
                    )
                }
            }
        byId.getValue("set-allowlisted-policy-repriced-aliases")
            .objectValue("semantic")
            .let { semantic ->
                assertFailsWith<IllegalArgumentException> {
                    MusubiInstructionsV1.SetMusubiRegistryPolicyV1(
                        parseGovernanceDecision(semantic.objectValue("decision")),
                        parseRegistryPolicy(semantic.objectValue("policy")),
                        BigInteger.ZERO,
                    )
                }
            }
    }

    @Test
    fun `new mutation nested commitments ordering and canonicality are enforced`() {
        val byId = cases(fixture()).associateBy { it.string("id") }

        val location = byId.getValue("add-location-three-signed-providers")
            .objectValue("semantic")
        fun addLocation(
            renewAfterEpoch: BigInteger = location.bigInteger("renew_after_epoch"),
            expiresAtEpoch: BigInteger = location.bigInteger("expires_at_epoch"),
        ) = MusubiInstructionsV1.AddMusubiArchiveLocationV1(
            parseDigest(location["archive_id"]),
            parseDigest(location["location_id"]),
            parseDigest(location["pin_manifest"]),
            parseDigest(location["replication_order"]),
            parseProviderAttestationSetDigest(location["provider_attestation_set_digest"]),
            renewAfterEpoch,
            expiresAtEpoch,
            location.bigInteger("expected_location_revision"),
        )
        assertFailsWith<IllegalArgumentException> {
            addLocation(
                renewAfterEpoch = location.bigInteger("expires_at_epoch"),
            )
        }
        addLocation(renewAfterEpoch = BigInteger.ZERO)

        val metadata = byId.getValue("replace-domain-metadata-high-revision")
            .objectValue("semantic")
            .let { parseMetadata(it.objectValue("metadata")) }
        assertFailsWith<IllegalArgumentException> {
            MusubiReleaseMetadataV1(
                metadata.description,
                metadata.readme,
                metadata.license,
                metadata.repository,
                metadata.keywords.reversed(),
            )
        }

        assertFailsWith<IllegalArgumentException> {
            MusubiVersionReqV1.parse("=1.0.0,=2.0.0")
        }

        val publish = byId.getValue("publish-delegated-domain-release")
            .objectValue("semantic")
        val publication = parsePublication(publish.objectValue("publication"))
        val manifest = publication.manifest
        val tamperedManifest = MusubiReleaseManifestV1(
            manifest.release,
            manifest.edition,
            manifest.abi,
            manifest.dependencies,
            manifest.exports,
            manifest.interfaceDigest,
            manifest.metadata,
            manifest.archiveId,
            MusubiDigest32V1.fromBytes(ByteArray(32) { 0x5a.toByte() }),
        )
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.PublishMusubiReleaseV1(
                MusubiNamespaceV1(newtypeText(publish["namespace"])),
                MusubiPublicationV1(tamperedManifest, publication.resolution),
                publish["namespace_delegation"]?.let {
                    parseNamespaceDelegation(it.objectValue())
                },
                publish.bigInteger("expected_policy_revision"),
                publish.optionalBigInteger("expected_governance_revision"),
            )
        }

        val setPolicy = byId.getValue("set-allowlisted-policy-repriced-aliases")
            .objectValue("semantic")
        val decision = parseGovernanceDecision(setPolicy.objectValue("decision"))
        val wrongActionDigest = decision.actionDigest.bytes().also { it[0] = (it[0].toInt() xor 1).toByte() }
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.SetMusubiRegistryPolicyV1(
                MusubiGovernanceDecisionV1(
                    decision.decisionId(),
                    MusubiDigest32V1.fromBytes(wrongActionDigest),
                    decision.enactedAtHeight,
                    decision.executeAfterHeight,
                ),
                parseRegistryPolicy(setPolicy.objectValue("policy")),
                setPolicy.bigInteger("expected_policy_revision"),
            )
        }

        val register = byId.getValue("register-archive-max-bounds-signed-receipt")
            .objectValue("semantic")
        val commitment = parseArchiveCommitment(register.objectValue("commitment"))
        val receipt = parseSeedIngressReceipt(register.objectValue("staging_receipt"))
        val binding = receipt.payload.binding
        val wrongCarDigest = binding.carBodyDigest.bytes().also {
            it[0] = (it[0].toInt() xor 1).toByte()
        }
        val wrongBinding = MusubiSeedIngressReceiptBindingV1(
            binding.networkId,
            binding.publisher,
            binding.ingressBroker,
            binding.seedProvider,
            binding.semanticReleaseManifestDigest,
            binding.archiveId,
            MusubiDigest32V1.fromBytes(wrongCarDigest),
            binding.carBodyLength,
            binding.nonce(),
        )
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RegisterMusubiArchiveV1(
                commitment,
                MusubiSeedIngressReceiptV1(
                    MusubiSeedIngressReceiptPayloadV1(
                        wrongBinding,
                        receipt.payload.issuedAtMs,
                        receipt.payload.expiresAtMs,
                    ),
                    receipt.approvals,
                ),
                register.bigInteger("expected_policy_revision"),
            )
        }
    }

    @Test
    fun `typed constructors match every Rust-owned wire layer`() {
        val fixture = fixture()
        assertEquals("iroha-musubi-instructions-v1", fixture["format"])
        assertEquals(1L, fixture["fixture_version"])
        assertEquals("iroha_data_model::isi::musubi", fixture["rust_owner"])
        assertEquals(INSTRUCTION_BOX_SCHEMA, fixture["instruction_box_schema_name"])
        assertEquals(
            fixture["instruction_box_schema_hash"],
            SchemaHash.hash16(INSTRUCTION_BOX_SCHEMA).toHex(),
        )
        assertFailsWith<IllegalArgumentException> {
            parseDigest(listOf(List(32) { index -> if (index == 0) 256L else 0L }))
        }

        val fixtureCases = cases(fixture)
        assertEquals(EXPECTED_CASE_IDS, fixtureCases.map { it["id"] })
        fixtureCases.forEach { case ->
            val mutation = mutation(case)
            assertEquals(case["wire_id"], mutation.wireId)
            assertEquals(case["concrete_schema_name"], mutation.schemaName)
            assertEquals(
                case["concrete_schema_hash"],
                SchemaHash.hash16(mutation.schemaName).toHex(),
            )
            assertEquals(NoritoHeader.COMPACT_LEN.toLong(), case["header_flags"])
            assertContentEquals(hex(case.string("bare_payload_hex")), mutation.barePayload)
            assertContentEquals(hex(case.string("concrete_frame_hex")), mutation.concreteFrame)

            val concrete = NoritoHeader.decode(
                mutation.concreteFrame,
                SchemaHash.hash16(mutation.schemaName),
            )
            concrete.header.validateChecksum(concrete.payload)
            assertEquals(NoritoHeader.COMPACT_LEN, concrete.header.flags)
            assertContentEquals(mutation.barePayload, concrete.payload)

            assertEquals(InstructionKind.CUSTOM, mutation.box.kind)
            val wire = assertIs<WirePayload>(mutation.box.payload)
            assertEquals(mutation.wireId, wire.wireName)
            assertContentEquals(mutation.concreteFrame, wire.payloadBytes)

            val standalone = NoritoJavaCodecAdapter.encodeInstructionBox(mutation.box)
            assertContentEquals(
                hex(case.string("standalone_instruction_box_frame_hex")),
                standalone,
            )
            val boxed = NoritoHeader.decode(
                standalone,
                SchemaHash.hash16(INSTRUCTION_BOX_SCHEMA),
            )
            boxed.header.validateChecksum(boxed.payload)
            assertEquals(NoritoHeader.COMPACT_LEN, boxed.header.flags)
            assertContentEquals(hex(case.string("instruction_box_pair_hex")), boxed.payload)
        }
    }

    @Test
    fun `one transaction batch preserves every fixture instruction pair inline`() {
        val fixtureCases = cases(fixture())
        val mutations = fixtureCases.map(::mutation)
        val transaction = TransactionPayload(
            networkId = TestNetworkIds.canonical(),
            authority = AccountAddress
                .fromAccount(TestEd25519Keys.publicKey(0x5a), "ed25519")
                .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1),
            creationTimeMs = 1_735_555_000_000L,
            executable = Executable.batch(
                mutations.map { ExecutableBatchItem.instruction(it.box) },
            ),
            feePayment = FeePaymentIntent.authority(emptyList()),
        )
        val encoded = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
            .encodeTransaction(transaction)
        val transactionDecoder = canonicalDecoder(encoded)
        readField(transactionDecoder, "domain")
        readField(transactionDecoder, "authority")
        readField(transactionDecoder, "creation_time_ms")
        val executablePayload = readField(transactionDecoder, "executable")
        repeat(5) { index -> readField(transactionDecoder, "tail[$index]") }
        assertEquals(0, transactionDecoder.remaining())

        val executableDecoder = canonicalDecoder(executablePayload)
        assertEquals(BATCH_EXECUTABLE_TAG, executableDecoder.readUInt(32))
        val batchPayload = readField(executableDecoder, "batch")
        assertEquals(0, executableDecoder.remaining())

        val batchDecoder = canonicalDecoder(batchPayload)
        assertEquals(fixtureCases.size.toLong(), batchDecoder.readUInt(64))
        fixtureCases.forEachIndexed { index, case ->
            val item = canonicalDecoder(readSequenceElement(batchDecoder, "batch[$index]"))
            assertEquals(INSTRUCTION_BATCH_ITEM_TAG, item.readUInt(32))
            assertContentEquals(
                hex(case.string("instruction_box_pair_hex")),
                readField(item, "batch[$index].instruction"),
            )
            assertEquals(0, item.remaining())
        }
        assertEquals(0, batchDecoder.remaining())
    }

    @Test
    fun `alias and unsigned fields reject noncanonical values before encoding`() {
        listOf("Alias", "-alias", "alias-", "alias--name", "a".repeat(33)).forEach {
            assertFailsWith<IllegalArgumentException> { MusubiAliasNameV1(it) }
        }

        val fixtureCase = cases(fixture()).first { it["id"] == "accept-root-max-revision" }
        val semantic = fixtureCase.objectValue("semantic")
        assertEquals(U64_MAX, semantic.bigInteger("expected_governance_revision"))
        val packageId = parsePackage(semantic.objectValue("package"))
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.AcceptMusubiPackageMaintainerV1(
                packageId,
                parseDigest(semantic["invite_id"]),
                U64_MAX.add(BigInteger.ONE),
            )
        }
    }

    @Test
    fun `reason account and new revisions reject noncanonical values before encoding`() {
        val maximumReasonText = "é".repeat(512)
        assertEquals(
            1_024,
            maximumReasonText.toByteArray(StandardCharsets.UTF_8).size,
        )
        assertEquals(maximumReasonText, MusubiReasonV1(maximumReasonText).value)
        listOf(
            "",
            " leading whitespace",
            "trailing whitespace ",
            "embedded\u0000control",
            "a".repeat(1_025),
        ).forEach { invalid ->
            assertFailsWith<IllegalArgumentException> { MusubiReasonV1(invalid) }
        }

        val fixtureCases = cases(fixture())
        val retire = fixtureCases
            .first { it["id"] == "retire-location-max-revision" }
            .objectValue("semantic")
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RetireMusubiArchiveLocationV1(
                parseDigest(retire["archive_id"]),
                parseDigest(retire["location_id"]),
                U64_MAX.add(BigInteger.ONE),
                MusubiReasonV1(newtypeText(retire["reason"])),
            )
        }

        val unyank = fixtureCases
            .first { it["id"] == "unyank-domain-release-high-revision" }
            .objectValue("semantic")
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.SetMusubiReleaseYankV1(
                parseRelease(unyank.objectValue("release")),
                unyank.boolean("yanked"),
                MusubiReasonV1(newtypeText(unyank["reason"])),
                U64_MAX.add(BigInteger.ONE),
            )
        }

        val remove = fixtureCases
            .first { it["id"] == "remove-root-maintainer-high-revision" }
            .objectValue("semantic")
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RemoveMusubiPackageMaintainerV1(
                parsePackage(remove.objectValue("package")),
                remove.string("account"),
                U64_MAX.add(BigInteger.ONE),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RemoveMusubiPackageMaintainerV1(
                parsePackage(remove.objectValue("package")),
                "not-a-canonical-account",
                remove.bigInteger("expected_governance_revision"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RemoveMusubiPackageMaintainerV1(
                parsePackage(remove.objectValue("package")),
                " ${remove.string("account")}",
                remove.bigInteger("expected_governance_revision"),
            )
        }
    }

    @Test
    fun `namespace roles and invitations reject noncanonical values before encoding`() {
        val fixtureCases = cases(fixture())
        val register = fixtureCases
            .first { it["id"] == "register-domain-namespace-max-generation" }
            .objectValue("semantic")
        val binding = parseNamespaceBinding(register.objectValue("binding"))
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RegisterMusubiNamespaceBindingV1(
                binding,
                U64_MAX.add(BigInteger.ONE),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiNamespaceBindingV1(
                binding.namespace,
                binding.homeDataspace,
                binding.scope,
                BigInteger.ZERO,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiNamespaceBindingV1(
                binding.namespace,
                binding.homeDataspace,
                MusubiPackageScopeV1.dataspaceRoot(),
                binding.generation,
            )
        }

        assertFailsWith<IllegalArgumentException> {
            MusubiMaintainerPermissionsV1(
                publish = false,
                yank = false,
                metadata = false,
                archiveLocations = false,
            )
        }

        val invite = fixtureCases
            .first { it["id"] == "invite-domain-maintainer-max-expiry" }
            .objectValue("semantic")
        val invitePackage = parsePackage(invite.objectValue("package"))
        val invitedAccount = invite.string("invited_account")
        val inviteRole = parsePackageRole(invite["role"])
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.InviteMusubiPackageMaintainerV1(
                invitePackage,
                MusubiDigest32V1.fromBytes(ByteArray(32)),
                invitedAccount,
                inviteRole,
                invite.bigInteger("expires_at_height"),
                invite.bigInteger("expected_governance_revision"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.InviteMusubiPackageMaintainerV1(
                invitePackage,
                parseDigest(invite["invite_id"]),
                "not-a-canonical-account",
                inviteRole,
                invite.bigInteger("expires_at_height"),
                invite.bigInteger("expected_governance_revision"),
            )
        }
        listOf(BigInteger.ZERO, U64_MAX.add(BigInteger.ONE)).forEach { invalidExpiry ->
            assertFailsWith<IllegalArgumentException> {
                MusubiInstructionsV1.InviteMusubiPackageMaintainerV1(
                    invitePackage,
                    parseDigest(invite["invite_id"]),
                    invitedAccount,
                    inviteRole,
                    invalidExpiry,
                    invite.bigInteger("expected_governance_revision"),
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.InviteMusubiPackageMaintainerV1(
                invitePackage,
                parseDigest(invite["invite_id"]),
                invitedAccount,
                inviteRole,
                invite.bigInteger("expires_at_height"),
                U64_MAX.add(BigInteger.ONE),
            )
        }

        val setRole = fixtureCases
            .first { it["id"] == "promote-root-member-to-owner-high-revision" }
            .objectValue("semantic")
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.SetMusubiPackageMaintainerRoleV1(
                parsePackage(setRole.objectValue("package")),
                "not-a-canonical-account",
                parsePackageRole(setRole["role"]),
                setRole.bigInteger("expected_governance_revision"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.SetMusubiPackageMaintainerRoleV1(
                parsePackage(setRole.objectValue("package")),
                setRole.string("account"),
                parsePackageRole(setRole["role"]),
                U64_MAX.add(BigInteger.ONE),
            )
        }
    }

    @Test
    fun `Parliament mutations reject invalid decisions owners and revisions`() {
        val fixtureCases = cases(fixture())
        val recover = fixtureCases
            .first { it["id"] == "recover-domain-package-three-owners" }
            .objectValue("semantic")
        val decision = parseGovernanceDecision(recover.objectValue("decision"))
        val packageId = parsePackage(recover.objectValue("package"))
        val owners = recover.arrayValue("owners").map {
            it as? String ?: error("fixture recovery owner must be a string")
        }

        assertFailsWith<IllegalArgumentException> {
            MusubiGovernanceDecisionV1(
                ByteArray(32),
                decision.actionDigest,
                BigInteger.ONE,
                BigInteger.valueOf(2L),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiGovernanceDecisionV1(
                decision.decisionId(),
                MusubiDigest32V1.fromBytes(ByteArray(32)),
                BigInteger.ONE,
                BigInteger.valueOf(2L),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiGovernanceDecisionV1(
                decision.decisionId(),
                decision.actionDigest,
                BigInteger.ZERO,
                BigInteger.valueOf(2L),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiGovernanceDecisionV1(
                decision.decisionId(),
                decision.actionDigest,
                BigInteger.valueOf(2L),
                BigInteger.valueOf(2L),
            )
        }

        val rejectedOwnerSets: List<List<String>> = listOf(
            emptyList(),
            owners.reversed(),
            listOf(owners.first(), owners.first()),
            List(65) { owners.first() },
        )
        rejectedOwnerSets.forEach { rejectedOwners ->
            assertFailsWith<IllegalArgumentException> {
                MusubiInstructionsV1.RecoverMusubiPackageV1(
                    decision,
                    packageId,
                    rejectedOwners,
                    BigInteger.ONE,
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RecoverMusubiPackageV1(
                decision,
                packageId,
                owners,
                BigInteger.ZERO,
            )
        }

        val retarget = fixtureCases
            .first { it["id"] == "retarget-one-character-alias-high-revision" }
            .objectValue("semantic")
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RetargetMusubiAliasV1(
                parseGovernanceDecision(retarget.objectValue("decision")),
                MusubiAliasNameV1(newtypeText(retarget["alias"])),
                parsePackage(retarget.objectValue("target")),
                BigInteger.ZERO,
            )
        }

        val takedown = fixtureCases
            .first { it["id"] == "takedown-max-major-prerelease" }
            .objectValue("semantic")
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.SetMusubiArtifactTakedownV1(
                parseGovernanceDecision(takedown.objectValue("decision")),
                parseRelease(takedown.objectValue("release")),
                MusubiReasonV1(newtypeText(takedown["reason"])),
                BigInteger.ZERO,
            )
        }
    }

    @Test
    fun `recovery normalizes multisig owners before distinctness and encoding`() {
        val recover = cases(fixture())
            .first { it["id"] == "recover-domain-package-three-owners" }
            .objectValue("semantic")
        val decision = parseGovernanceDecision(recover.objectValue("decision"))
        val packageId = parsePackage(recover.objectValue("package"))
        val sortedBytes = hex(
            "0a010100020002" +
                "01000100205c9c6df261c9cb840475776aaefcd944b405328fab28f9b3a95ef40490d3de84" +
                "0100020020d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737",
        )
        val reversedBytes = hex(
            "0a010100020002" +
                "0100020020d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737" +
                "01000100205c9c6df261c9cb840475776aaefcd944b405328fab28f9b3a95ef40490d3de84",
        )
        val sortedOwner = AccountAddress.fromCanonicalBytes(sortedBytes)
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
        val reversedOwner = AccountAddress.fromCanonicalBytes(reversedBytes)
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
        assertNotEquals(sortedOwner, reversedOwner)

        val sorted = MusubiInstructionsV1.RecoverMusubiPackageV1(
            decision,
            packageId,
            listOf(sortedOwner),
            BigInteger.ONE,
        )
        val reversed = MusubiInstructionsV1.RecoverMusubiPackageV1(
            decision,
            packageId,
            listOf(reversedOwner),
            BigInteger.ONE,
        )
        assertContentEquals(sorted.barePayload(), reversed.barePayload())
        assertFailsWith<IllegalArgumentException> {
            MusubiInstructionsV1.RecoverMusubiPackageV1(
                decision,
                packageId,
                listOf(sortedOwner, reversedOwner),
                BigInteger.ONE,
            )
        }
    }

    private fun mutation(case: MutableMap<String, Any?>): MutationEncoding {
        val semantic = case.objectValue("semantic")
        return when (case.string("id")) {
            "accept-root-max-revision" -> {
                semantic.requireKeys(
                    "package",
                    "invite_id",
                    "expected_governance_revision",
                )
                val value = MusubiInstructionsV1.AcceptMusubiPackageMaintainerV1(
                    parsePackage(semantic.objectValue("package")),
                    parseDigest(semantic["invite_id"]),
                    semantic.bigInteger("expected_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.AcceptMusubiPackageMaintainerV1.WIRE_ID,
                    MusubiInstructionsV1.AcceptMusubiPackageMaintainerV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "revoke-domain-invitation" -> {
                semantic.requireKeys(
                    "package",
                    "invite_id",
                    "expected_governance_revision",
                )
                val value = MusubiInstructionsV1.RevokeMusubiPackageMaintainerInvitationV1(
                    parsePackage(semantic.objectValue("package")),
                    parseDigest(semantic["invite_id"]),
                    semantic.bigInteger("expected_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RevokeMusubiPackageMaintainerInvitationV1.WIRE_ID,
                    MusubiInstructionsV1.RevokeMusubiPackageMaintainerInvitationV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "register-alias-domain-target" -> {
                semantic.requireKeys("alias", "target", "expected_pricing_revision")
                val value = MusubiInstructionsV1.RegisterMusubiAliasV1(
                    MusubiAliasNameV1(newtypeText(semantic["alias"])),
                    parsePackage(semantic.objectValue("target")),
                    semantic.bigInteger("expected_pricing_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RegisterMusubiAliasV1.WIRE_ID,
                    MusubiInstructionsV1.RegisterMusubiAliasV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "assert-prerelease-digest" -> {
                semantic.requireKeys("release", "expected_digest")
                val value = MusubiInstructionsV1.AssertMusubiReleaseDigestV1(
                    parseRelease(semantic.objectValue("release")),
                    parseDigest(semantic["expected_digest"]),
                )
                MutationEncoding(
                    MusubiInstructionsV1.AssertMusubiReleaseDigestV1.WIRE_ID,
                    MusubiInstructionsV1.AssertMusubiReleaseDigestV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "retire-location-max-revision" -> {
                semantic.requireKeys(
                    "archive_id",
                    "location_id",
                    "expected_location_revision",
                    "reason",
                )
                val value = MusubiInstructionsV1.RetireMusubiArchiveLocationV1(
                    parseDigest(semantic["archive_id"]),
                    parseDigest(semantic["location_id"]),
                    semantic.bigInteger("expected_location_revision"),
                    MusubiReasonV1(newtypeText(semantic["reason"])),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RetireMusubiArchiveLocationV1.WIRE_ID,
                    MusubiInstructionsV1.RetireMusubiArchiveLocationV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "unyank-domain-release-high-revision" -> {
                semantic.requireKeys(
                    "release",
                    "yanked",
                    "reason",
                    "expected_yank_revision",
                )
                val value = MusubiInstructionsV1.SetMusubiReleaseYankV1(
                    parseRelease(semantic.objectValue("release")),
                    semantic.boolean("yanked"),
                    MusubiReasonV1(newtypeText(semantic["reason"])),
                    semantic.bigInteger("expected_yank_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.SetMusubiReleaseYankV1.WIRE_ID,
                    MusubiInstructionsV1.SetMusubiReleaseYankV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "remove-root-maintainer-high-revision" -> {
                semantic.requireKeys("package", "account", "expected_governance_revision")
                val value = MusubiInstructionsV1.RemoveMusubiPackageMaintainerV1(
                    parsePackage(semantic.objectValue("package")),
                    semantic.string("account"),
                    semantic.bigInteger("expected_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RemoveMusubiPackageMaintainerV1.WIRE_ID,
                    MusubiInstructionsV1.RemoveMusubiPackageMaintainerV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "register-domain-namespace-max-generation" -> {
                semantic.requireKeys("binding", "expected_policy_revision")
                val value = MusubiInstructionsV1.RegisterMusubiNamespaceBindingV1(
                    parseNamespaceBinding(semantic.objectValue("binding")),
                    semantic.bigInteger("expected_policy_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RegisterMusubiNamespaceBindingV1.WIRE_ID,
                    MusubiInstructionsV1.RegisterMusubiNamespaceBindingV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "invite-domain-maintainer-max-expiry" -> {
                semantic.requireKeys(
                    "package",
                    "invite_id",
                    "invited_account",
                    "role",
                    "expires_at_height",
                    "expected_governance_revision",
                )
                val value = MusubiInstructionsV1.InviteMusubiPackageMaintainerV1(
                    parsePackage(semantic.objectValue("package")),
                    parseDigest(semantic["invite_id"]),
                    semantic.string("invited_account"),
                    parsePackageRole(semantic["role"]),
                    semantic.bigInteger("expires_at_height"),
                    semantic.bigInteger("expected_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.InviteMusubiPackageMaintainerV1.WIRE_ID,
                    MusubiInstructionsV1.InviteMusubiPackageMaintainerV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "promote-root-member-to-owner-high-revision" -> {
                semantic.requireKeys("package", "account", "role", "expected_governance_revision")
                val value = MusubiInstructionsV1.SetMusubiPackageMaintainerRoleV1(
                    parsePackage(semantic.objectValue("package")),
                    semantic.string("account"),
                    parsePackageRole(semantic["role"]),
                    semantic.bigInteger("expected_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.SetMusubiPackageMaintainerRoleV1.WIRE_ID,
                    MusubiInstructionsV1.SetMusubiPackageMaintainerRoleV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "recover-domain-package-three-owners" -> {
                semantic.requireKeys(
                    "decision",
                    "package",
                    "owners",
                    "expected_governance_revision",
                )
                val value = MusubiInstructionsV1.RecoverMusubiPackageV1(
                    parseGovernanceDecision(semantic.objectValue("decision")),
                    parsePackage(semantic.objectValue("package")),
                    semantic.arrayValue("owners").map {
                        it as? String ?: error("fixture recovery owner must be a string")
                    },
                    semantic.bigInteger("expected_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RecoverMusubiPackageV1.WIRE_ID,
                    MusubiInstructionsV1.RecoverMusubiPackageV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "retarget-one-character-alias-high-revision" -> {
                semantic.requireKeys(
                    "decision",
                    "alias",
                    "target",
                    "expected_history_revision",
                )
                val value = MusubiInstructionsV1.RetargetMusubiAliasV1(
                    parseGovernanceDecision(semantic.objectValue("decision")),
                    MusubiAliasNameV1(newtypeText(semantic["alias"])),
                    parsePackage(semantic.objectValue("target")),
                    semantic.bigInteger("expected_history_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RetargetMusubiAliasV1.WIRE_ID,
                    MusubiInstructionsV1.RetargetMusubiAliasV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "takedown-max-major-prerelease" -> {
                semantic.requireKeys(
                    "decision",
                    "release",
                    "reason",
                    "expected_artifact_governance_revision",
                )
                val value = MusubiInstructionsV1.SetMusubiArtifactTakedownV1(
                    parseGovernanceDecision(semantic.objectValue("decision")),
                    parseRelease(semantic.objectValue("release")),
                    MusubiReasonV1(newtypeText(semantic["reason"])),
                    semantic.bigInteger("expected_artifact_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.SetMusubiArtifactTakedownV1.WIRE_ID,
                    MusubiInstructionsV1.SetMusubiArtifactTakedownV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "register-archive-max-bounds-signed-receipt" -> {
                semantic.requireKeys("commitment", "staging_receipt", "expected_policy_revision")
                val value = MusubiInstructionsV1.RegisterMusubiArchiveV1(
                    parseArchiveCommitment(semantic.objectValue("commitment")),
                    parseSeedIngressReceipt(semantic.objectValue("staging_receipt")),
                    semantic.bigInteger("expected_policy_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RegisterMusubiArchiveV1.WIRE_ID,
                    MusubiInstructionsV1.RegisterMusubiArchiveV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "register-provider-bundle-attestation" -> {
                semantic.requireKeys("attestation", "expected_location_revision")
                val value = MusubiInstructionsV1.RegisterMusubiProviderBundleAttestationV1(
                    parseProviderAttestation(semantic.objectValue("attestation")),
                    semantic.bigInteger("expected_location_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.RegisterMusubiProviderBundleAttestationV1.WIRE_ID,
                    MusubiInstructionsV1.RegisterMusubiProviderBundleAttestationV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "add-location-three-signed-providers" -> {
                semantic.requireKeys(
                    "archive_id",
                    "location_id",
                    "pin_manifest",
                    "replication_order",
                    "provider_attestation_set_digest",
                    "renew_after_epoch",
                    "expires_at_epoch",
                    "expected_location_revision",
                )
                val value = MusubiInstructionsV1.AddMusubiArchiveLocationV1(
                    parseDigest(semantic["archive_id"]),
                    parseDigest(semantic["location_id"]),
                    parseDigest(semantic["pin_manifest"]),
                    parseDigest(semantic["replication_order"]),
                    parseProviderAttestationSetDigest(
                        semantic["provider_attestation_set_digest"],
                    ),
                    semantic.bigInteger("renew_after_epoch"),
                    semantic.bigInteger("expires_at_epoch"),
                    semantic.bigInteger("expected_location_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.AddMusubiArchiveLocationV1.WIRE_ID,
                    MusubiInstructionsV1.AddMusubiArchiveLocationV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "publish-delegated-domain-release" -> {
                semantic.requireKeys(
                    "namespace",
                    "publication",
                    "namespace_delegation",
                    "expected_policy_revision",
                    "expected_governance_revision",
                )
                val value = MusubiInstructionsV1.PublishMusubiReleaseV1(
                    MusubiNamespaceV1(newtypeText(semantic["namespace"])),
                    parsePublication(semantic.objectValue("publication")),
                    semantic["namespace_delegation"]?.let {
                        parseNamespaceDelegation(it.objectValue())
                    },
                    semantic.bigInteger("expected_policy_revision"),
                    semantic.optionalBigInteger("expected_governance_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.PublishMusubiReleaseV1.WIRE_ID,
                    MusubiInstructionsV1.PublishMusubiReleaseV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "replace-domain-metadata-high-revision" -> {
                semantic.requireKeys("package", "metadata", "expected_metadata_revision")
                val value = MusubiInstructionsV1.SetMusubiPackageMetadataV1(
                    parsePackage(semantic.objectValue("package")),
                    parseMetadata(semantic.objectValue("metadata")),
                    semantic.bigInteger("expected_metadata_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.SetMusubiPackageMetadataV1.WIRE_ID,
                    MusubiInstructionsV1.SetMusubiPackageMetadataV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            "set-allowlisted-policy-repriced-aliases" -> {
                semantic.requireKeys("decision", "policy", "expected_policy_revision")
                val value = MusubiInstructionsV1.SetMusubiRegistryPolicyV1(
                    parseGovernanceDecision(semantic.objectValue("decision")),
                    parseRegistryPolicy(semantic.objectValue("policy")),
                    semantic.bigInteger("expected_policy_revision"),
                )
                MutationEncoding(
                    MusubiInstructionsV1.SetMusubiRegistryPolicyV1.WIRE_ID,
                    MusubiInstructionsV1.SetMusubiRegistryPolicyV1.SCHEMA_NAME,
                    value.barePayload(),
                    value.concreteFrame(),
                    value.toInstructionBox(),
                )
            }
            else -> error("unknown Musubi instruction fixture case ${case["id"]}")
        }
    }

    private fun parseArchiveCommitment(
        value: MutableMap<String, Any?>,
    ): MusubiArchiveCommitmentV1 {
        value.requireKeys(
            "root_cid",
            "chunker",
            "chunk_plan_digest",
            "por_root",
            "content_length",
            "car_digest",
            "car_size",
            "bundle_digest",
            "source_tree_digest",
            "descriptor_digest",
            "file_count",
            "chunk_count",
        )
        val chunker = value.objectValue("chunker")
        chunker.requireKeys("profile_id", "namespace", "name", "semver", "multihash_code")
        return MusubiArchiveCommitmentV1(
            fixedBytes(value["root_cid"], 36),
            MusubiChunkerProfileHandleV1(
                chunker.bigInteger("profile_id").longValueExact(),
                chunker.string("namespace"),
                chunker.string("name"),
                chunker.string("semver"),
                chunker.bigInteger("multihash_code"),
            ),
            parseDigest(value["chunk_plan_digest"]),
            parseDigest(value["por_root"]),
            value.bigInteger("content_length"),
            parseDigest(value["car_digest"]),
            value.bigInteger("car_size"),
            parseDigest(value["bundle_digest"]),
            parseDigest(value["source_tree_digest"]),
            parseDigest(value["descriptor_digest"]),
            value.bigInteger("file_count").longValueExact(),
            value.bigInteger("chunk_count").longValueExact(),
        )
    }

    private fun parseSeedIngressReceipt(
        value: MutableMap<String, Any?>,
    ): MusubiSeedIngressReceiptV1 {
        value.requireKeys("payload", "approvals")
        val payload = value.objectValue("payload")
        payload.requireKeys("version", "binding", "issued_at_ms", "expires_at_ms")
        assertEquals(BigInteger.ONE, payload.bigInteger("version"))
        val binding = payload.objectValue("binding")
        binding.requireKeys(
            "network_id",
            "publisher",
            "ingress_broker",
            "seed_provider",
            "semantic_release_manifest_digest",
            "archive_id",
            "car_body_digest",
            "car_body_length",
            "nonce",
        )
        val typedBinding = MusubiSeedIngressReceiptBindingV1(
            NetworkId.parse(binding.string("network_id")),
            binding.string("publisher"),
            binding.string("ingress_broker"),
            newtypeText(binding["seed_provider"]),
            parseDigest(binding["semantic_release_manifest_digest"]),
            parseDigest(binding["archive_id"]),
            parseDigest(binding["car_body_digest"]),
            binding.bigInteger("car_body_length"),
            fixedBytes32(binding["nonce"]),
        )
        return MusubiSeedIngressReceiptV1(
            MusubiSeedIngressReceiptPayloadV1(
                typedBinding,
                payload.bigInteger("issued_at_ms"),
                payload.bigInteger("expires_at_ms"),
            ),
            value.arrayValue("approvals").map { raw ->
                val approval = raw.objectValue()
                approval.requireKeys("public_key", "signature")
                MusubiSeedIngressReceiptApprovalV1(
                    approval.string("public_key"),
                    approval.string("signature"),
                )
            },
        )
    }

    private fun parseProviderAttestation(
        value: MutableMap<String, Any?>,
    ): MusubiProviderBundleVerificationAttestationV1 {
        value.requireKeys("payload", "approvals")
        val payload = value.objectValue("payload")
        payload.requireKeys("version", "binding")
        assertEquals(BigInteger.ONE, payload.bigInteger("version"))
        val binding = payload.objectValue("binding")
        binding.requireKeys(
            "network_id",
            "provider_id",
            "completed_by",
            "completion_authority",
            "replication_order",
            "assignment_revision",
            "completion_epoch",
            "finalized_anchor",
            "archive_id",
            "bundle_digest",
            "descriptor_digest",
            "semantic_release_manifest_digest",
            "verification_lock_digest",
            "source_tree_digest",
        )
        val authority = binding.objectValue("completion_authority")
        authority.requireKeys("provider_owner", "signer_policy")
        val signer = authority.objectValue("signer_policy")
        signer.requireKeys("policy_id", "revision", "predecessor_digest", "policy_digest")
        val anchor = binding.objectValue("finalized_anchor")
        anchor.requireKeys("height", "block_hash")
        val typedBinding = MusubiProviderBundleVerificationBindingV1(
            NetworkId.parse(binding.string("network_id")),
            newtypeText(binding["provider_id"]),
            binding.string("completed_by"),
            MusubiProviderIngestCompletionAuthorityV1(
                authority.string("provider_owner"),
                MusubiProviderIngestCompletionSignerPolicyV1(
                    fixedBytes32(signer["policy_id"]),
                    signer.bigInteger("revision"),
                    signer["predecessor_digest"]?.let { fixedBytes32(it) },
                    fixedBytes32(signer["policy_digest"]),
                ),
            ),
            parseDigest(binding["replication_order"]),
            binding.bigInteger("assignment_revision"),
            binding.bigInteger("completion_epoch"),
            MusubiProviderIngestFinalizedAnchorV1(
                anchor.bigInteger("height"),
                fixedBytes32(anchor["block_hash"]),
            ),
            parseDigest(binding["archive_id"]),
            parseDigest(binding["bundle_digest"]),
            parseDigest(binding["descriptor_digest"]),
            parseDigest(binding["semantic_release_manifest_digest"]),
            parseDigest(binding["verification_lock_digest"]),
            parseDigest(binding["source_tree_digest"]),
        )
        return MusubiProviderBundleVerificationAttestationV1(
            MusubiProviderBundleVerificationPayloadV1(typedBinding),
            value.arrayValue("approvals").map { raw ->
                val approval = raw.objectValue()
                approval.requireKeys("public_key", "signature")
                MusubiProviderBundleVerificationApprovalV1(
                    approval.string("public_key"),
                    approval.string("signature"),
                )
            },
        )
    }

    private fun parsePublication(value: MutableMap<String, Any?>): MusubiPublicationV1 {
        value.requireKeys("manifest", "resolution")
        return MusubiPublicationV1(
            parseReleaseManifest(value.objectValue("manifest")),
            parseResolutionProof(value.objectValue("resolution")),
        )
    }

    private fun parseReleaseManifest(
        value: MutableMap<String, Any?>,
    ): MusubiReleaseManifestV1 {
        value.requireKeys(
            "release",
            "edition",
            "abi",
            "dependencies",
            "exports",
            "interface_digest",
            "metadata",
            "archive_id",
            "verification_lock_digest",
        )
        val edition = value.objectValue("edition")
        edition.requireKeys("kind", "value")
        assertEquals("V1", edition.string("kind"))
        require(edition["value"] == null)
        return MusubiReleaseManifestV1(
            parseRelease(value.objectValue("release")),
            MusubiKotodamaEditionV1.V1,
            parseAbi(value.objectValue("abi")),
            value.arrayValue("dependencies").map { parseDependencyReq(it.objectValue()) },
            value.arrayValue("exports").map {
                it as? String ?: error("fixture export must be a string")
            },
            parseDigest(value["interface_digest"]),
            parseMetadata(value.objectValue("metadata")),
            parseDigest(value["archive_id"]),
            parseDigest(value["verification_lock_digest"]),
        )
    }

    private fun parseAbi(value: MutableMap<String, Any?>): MusubiAbiBindingV1 {
        value.requireKeys("abi_version", "abi_hash")
        assertEquals(BigInteger.ONE, value.bigInteger("abi_version"))
        return MusubiAbiBindingV1(fixedBytes32(value["abi_hash"]))
    }

    private fun parseDependencyReq(value: MutableMap<String, Any?>): MusubiDependencyReqV1 {
        value.requireKeys("alias", "package", "requirement")
        return MusubiDependencyReqV1(
            value.string("alias"),
            parsePackage(value.objectValue("package")),
            parseRequirement(value.objectValue("requirement")),
        )
    }

    private fun parseResolutionProof(value: MutableMap<String, Any?>): MusubiResolutionProofV1 {
        value.requireKeys("snapshot", "lock")
        val snapshot = value.objectValue("snapshot")
        snapshot.requireKeys("finalized_height", "finalized_block_hash", "index_revision")
        return MusubiResolutionProofV1(
            MusubiRegistrySnapshotV1(
                snapshot.bigInteger("finalized_height"),
                fixedBytes32(snapshot["finalized_block_hash"]),
                snapshot.bigInteger("index_revision"),
            ),
            parseVerificationLock(value.objectValue("lock")),
        )
    }

    private fun parseVerificationLock(
        value: MutableMap<String, Any?>,
    ): MusubiVerificationLockV1 {
        value.requireKeys("schema", "version", "root", "root_dependencies", "nodes")
        assertEquals(MusubiVerificationLockV1.SCHEMA, value.string("schema"))
        assertEquals(BigInteger.ONE, value.bigInteger("version"))
        return MusubiVerificationLockV1(
            parseRelease(value.objectValue("root")),
            value.arrayValue("root_dependencies").map {
                parseExactDependencyEdge(it.objectValue())
            },
            value.arrayValue("nodes").map { parseVerificationNode(it.objectValue()) },
        )
    }

    private fun parseExactDependencyEdge(
        value: MutableMap<String, Any?>,
    ): MusubiExactDependencyEdgeV1 {
        value.requireKeys("alias", "kind", "package", "requirement", "selected")
        val kind = value.objectValue("kind")
        kind.requireKeys("kind", "value")
        require(kind["value"] == null)
        return MusubiExactDependencyEdgeV1(
            value.string("alias"),
            when (kind.string("kind")) {
                "Normal" -> MusubiDependencyKindV1.NORMAL
                "Development" -> MusubiDependencyKindV1.DEVELOPMENT
                else -> error("unknown dependency kind ${kind["kind"]}")
            },
            parsePackage(value.objectValue("package")),
            parseRequirement(value.objectValue("requirement")),
            parseRelease(value.objectValue("selected")),
        )
    }

    private fun parseVerificationNode(
        value: MutableMap<String, Any?>,
    ): MusubiVerificationNodeV1 {
        value.requireKeys(
            "release",
            "release_digest",
            "archive_id",
            "source_digest",
            "interface_digest",
            "abi",
            "dependencies",
        )
        return MusubiVerificationNodeV1(
            parseRelease(value.objectValue("release")),
            parseDigest(value["release_digest"]),
            parseDigest(value["archive_id"]),
            parseDigest(value["source_digest"]),
            parseDigest(value["interface_digest"]),
            parseAbi(value.objectValue("abi")),
            value.arrayValue("dependencies").map {
                parseExactDependencyEdge(it.objectValue())
            },
        )
    }

    private fun parseRequirement(value: MutableMap<String, Any?>): MusubiVersionReqV1 {
        value.requireKeys("kind", "value")
        return when (value.string("kind")) {
            "Any" -> MusubiVersionReqV1.fromWire(MusubiVersionReqV1.Kind.ANY)
            "Caret" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.CARET,
                version = parseVersion(value["value"].objectValue()),
            )
            "Tilde" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.TILDE,
                version = parseVersion(value["value"].objectValue()),
            )
            "MajorWildcard" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.MAJOR_WILDCARD,
                major = value.bigInteger("value"),
            )
            "MinorWildcard" -> {
                val wildcard = value["value"].objectValue()
                wildcard.requireKeys("major", "minor")
                MusubiVersionReqV1.fromWire(
                    MusubiVersionReqV1.Kind.MINOR_WILDCARD,
                    major = wildcard.bigInteger("major"),
                    minor = wildcard.bigInteger("minor"),
                )
            }
            "Exact" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.EXACT,
                version = parseVersion(value["value"].objectValue()),
            )
            "Comparators" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.COMPARATORS,
                comparators = value["value"].arrayValue().map { raw ->
                    val comparator = raw.objectValue()
                    comparator.requireKeys("op", "version")
                    val op = comparator.objectValue("op")
                    op.requireKeys("kind", "value")
                    require(op["value"] == null)
                    MusubiVersionComparatorV1(
                        when (op.string("kind")) {
                            "Greater" -> MusubiComparatorOpV1.GREATER
                            "GreaterOrEqual" -> MusubiComparatorOpV1.GREATER_OR_EQUAL
                            "Less" -> MusubiComparatorOpV1.LESS
                            "LessOrEqual" -> MusubiComparatorOpV1.LESS_OR_EQUAL
                            "Equal" -> MusubiComparatorOpV1.EQUAL
                            else -> error("unknown comparator op ${op["kind"]}")
                        },
                        parseVersion(comparator.objectValue("version")),
                    )
                },
            )
            else -> error("unknown requirement kind ${value["kind"]}")
        }
    }

    private fun parseMetadata(value: MutableMap<String, Any?>): MusubiReleaseMetadataV1 {
        value.requireKeys("description", "readme", "license", "repository", "keywords")
        return MusubiReleaseMetadataV1(
            value["description"]?.let { MusubiDescriptionV1(newtypeText(it)) },
            value["readme"]?.let { MusubiDocumentRefV1(newtypeText(it)) },
            value["license"]?.let { MusubiDocumentRefV1(newtypeText(it)) },
            value["repository"]?.let { MusubiDocumentRefV1(newtypeText(it)) },
            value.arrayValue("keywords").map { MusubiKeywordV1(newtypeText(it)) },
        )
    }

    private fun parseNamespaceDelegation(
        value: MutableMap<String, Any?>,
    ): MusubiNamespaceDelegationV1 {
        value.requireKeys("payload", "approvals")
        val payload = value.objectValue("payload")
        payload.requireKeys(
            "version",
            "namespace_binding",
            "owner_generation",
            "owner",
            "delegate",
            "expires_at_height",
        )
        assertEquals(BigInteger.ONE, payload.bigInteger("version"))
        return MusubiNamespaceDelegationV1(
            MusubiNamespaceDelegationPayloadV1(
                parseDigest(payload["namespace_binding"]),
                payload.bigInteger("owner_generation"),
                payload.string("owner"),
                payload.string("delegate"),
                payload.bigInteger("expires_at_height"),
            ),
            value.arrayValue("approvals").map { raw ->
                val approval = raw.objectValue()
                approval.requireKeys("public_key", "signature")
                MusubiNamespaceDelegationApprovalV1(
                    approval.string("public_key"),
                    approval.string("signature"),
                )
            },
        )
    }

    private fun parseRegistryPolicy(
        value: MutableMap<String, Any?>,
    ): MusubiRegistryPolicyV1 {
        value.requireKeys("version", "revision", "mode", "allowlisted_dataspaces", "alias_pricing")
        assertEquals(BigInteger.ONE, value.bigInteger("version"))
        val mode = value.objectValue("mode")
        mode.requireKeys("kind", "value")
        require(mode["value"] == null)
        val pricing = value.objectValue("alias_pricing")
        pricing.requireKeys(
            "revision",
            "length_1_xor",
            "length_2_xor",
            "length_3_xor",
            "length_4_xor",
            "length_5_to_32_xor",
        )
        return MusubiRegistryPolicyV1(
            value.bigInteger("revision"),
            when (mode.string("kind")) {
                "Closed" -> MusubiRegistryAdmissionModeV1.CLOSED
                "Allowlisted" -> MusubiRegistryAdmissionModeV1.ALLOWLISTED
                "Open" -> MusubiRegistryAdmissionModeV1.OPEN
                else -> error("unknown registry mode ${mode["kind"]}")
            },
            value.arrayValue("allowlisted_dataspaces").map {
                (it as? Number)?.toString()?.let(::BigInteger)
                    ?: error("fixture allowlisted dataspace must be an integer")
            },
            MusubiAliasPricingPolicyV1(
                pricing.bigInteger("revision"),
                pricing.bigInteger("length_1_xor"),
                pricing.bigInteger("length_2_xor"),
                pricing.bigInteger("length_3_xor"),
                pricing.bigInteger("length_4_xor"),
                pricing.bigInteger("length_5_to_32_xor"),
            ),
        )
    }

    private fun parseNamespaceBinding(
        value: MutableMap<String, Any?>,
    ): MusubiNamespaceBindingV1 {
        value.requireKeys("namespace", "home_dataspace", "scope", "generation")
        val scope = value.objectValue("scope")
        scope.requireKeys("kind", "value")
        return MusubiNamespaceBindingV1(
            MusubiNamespaceV1(newtypeText(value["namespace"])),
            value.bigInteger("home_dataspace"),
            when (scope.string("kind")) {
                "DataspaceRoot" -> {
                    require(scope["value"] == null)
                    MusubiPackageScopeV1.dataspaceRoot()
                }
                "Domain" -> MusubiPackageScopeV1.domain(scope.string("value"))
                else -> error("unknown package scope ${scope["kind"]}")
            },
            value.bigInteger("generation"),
        )
    }

    private fun parsePackageRole(value: Any?): MusubiPackageRoleV1 {
        val role = value.objectValue()
        role.requireKeys("kind", "value")
        return when (role.string("kind")) {
            "Owner" -> {
                require(role["value"] == null)
                MusubiPackageRoleV1.owner()
            }
            "Maintainer" -> {
                val permissions = role["value"].objectValue()
                permissions.requireKeys("publish", "yank", "metadata", "archive_locations")
                MusubiPackageRoleV1.maintainer(
                    MusubiMaintainerPermissionsV1(
                        permissions.boolean("publish"),
                        permissions.boolean("yank"),
                        permissions.boolean("metadata"),
                        permissions.boolean("archive_locations"),
                    ),
                )
            }
            else -> error("unknown package role ${role["kind"]}")
        }
    }

    private fun parsePackage(value: MutableMap<String, Any?>): MusubiPackageIdV1 {
        value.requireKeys("home_dataspace", "scope", "name")
        val scope = value.objectValue("scope")
        scope.requireKeys("kind", "value")
        return MusubiPackageIdV1(
            value.bigInteger("home_dataspace"),
            when (scope.string("kind")) {
                "DataspaceRoot" -> {
                    require(scope["value"] == null)
                    MusubiPackageScopeV1.dataspaceRoot()
                }
                "Domain" -> MusubiPackageScopeV1.domain(scope.string("value"))
                else -> error("unknown package scope ${scope["kind"]}")
            },
            MusubiPackageNameV1(newtypeText(value["name"])),
        )
    }

    private fun parseRelease(value: MutableMap<String, Any?>): MusubiReleaseIdV1 {
        value.requireKeys("package", "version")
        return MusubiReleaseIdV1(
            parsePackage(value.objectValue("package")),
            parseVersion(value.objectValue("version")),
        )
    }

    private fun parseVersion(version: MutableMap<String, Any?>): MusubiVersionV1 {
        version.requireKeys("major", "minor", "patch", "prerelease")
        val prerelease = version.arrayValue("prerelease").map { raw ->
            val identifier = raw.objectValue()
            identifier.requireKeys("kind", "value")
            when (identifier.string("kind")) {
                "Numeric" -> MusubiPrereleaseIdentifierV1.numeric(
                    identifier.bigInteger("value"),
                )
                "AlphaNumeric" -> MusubiPrereleaseIdentifierV1.alphaNumeric(
                    identifier.string("value"),
                )
                else -> error("unknown prerelease kind ${identifier["kind"]}")
            }
        }
        return MusubiVersionV1(
            version.bigInteger("major"),
            version.bigInteger("minor"),
            version.bigInteger("patch"),
            prerelease,
        )
    }

    private fun parseDigest(value: Any?): MusubiDigest32V1 {
        val bytes = value.arrayValue().single().arrayValue()
        require(bytes.size == 32)
        return MusubiDigest32V1.fromBytes(
            ByteArray(bytes.size) { index ->
                val number = bytes[index] as? Number
                    ?: error("fixture digest octet must be an integer")
                val octet = BigInteger(number.toString())
                require(octet >= BigInteger.ZERO && octet <= BYTE_MAX) {
                    "fixture digest octet must be in 0..255"
                }
                octet.toInt().toByte()
            },
        )
    }

    private fun parseProviderAttestationSetDigest(
        value: Any?,
    ): MusubiProviderBundleAttestationSetDigestV1 =
        MusubiProviderBundleAttestationSetDigestV1(parseDigest(value).bytes())

    private fun parseGovernanceDecision(
        value: MutableMap<String, Any?>,
    ): MusubiGovernanceDecisionV1 {
        value.requireKeys(
            "decision_id",
            "action_digest",
            "enacted_at_height",
            "execute_after_height",
        )
        return MusubiGovernanceDecisionV1(
            fixedBytes32(value["decision_id"]),
            parseDigest(value["action_digest"]),
            value.bigInteger("enacted_at_height"),
            value.bigInteger("execute_after_height"),
        )
    }

    private fun fixedBytes32(value: Any?): ByteArray = fixedBytes(value, 32)

    private fun fixedBytes(value: Any?, size: Int): ByteArray {
        val bytes = value.arrayValue()
        require(bytes.size == size)
        return ByteArray(bytes.size) { index ->
            val number = bytes[index] as? Number
                ?: error("fixture byte octet must be an integer")
            val octet = BigInteger(number.toString())
            require(octet >= BigInteger.ZERO && octet <= BYTE_MAX) {
                "fixture byte octet must be in 0..255"
            }
            octet.toInt().toByte()
        }
    }

    private fun fixture(): MutableMap<String, Any?> = JsonParser.parse(
        String(Files.readAllBytes(findFixture()), StandardCharsets.UTF_8),
    ).objectValue()

    private fun cases(fixture: MutableMap<String, Any?>): List<MutableMap<String, Any?>> =
        fixture.arrayValue("cases").map { it.objectValue() }

    private fun findFixture(): Path {
        var current = Paths.get("").toAbsolutePath().normalize()
        repeat(8) {
            val candidate = current.resolve("fixtures/musubi/instructions_v1.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent ?: return@repeat
        }
        error("fixtures/musubi/instructions_v1.json was not found from the test working directory")
    }

    private fun canonicalDecoder(payload: ByteArray): NoritoDecoder =
        NoritoDecoder(payload, NoritoCodec.DEFAULT_FLAGS)

    private fun readField(decoder: NoritoDecoder, field: String): ByteArray {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$field is too large" }
        return decoder.readBytes(length.toInt())
    }

    private fun readSequenceElement(decoder: NoritoDecoder, field: String): ByteArray {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$field is too large" }
        return decoder.readBytes(length.toInt())
    }

    private fun hex(value: String): ByteArray {
        require(value.length % 2 == 0 && value.all { it in "0123456789abcdef" })
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun MutableMap<String, Any?>.string(key: String): String =
        this[key] as? String ?: error("fixture field $key must be a string")

    private fun MutableMap<String, Any?>.bigInteger(key: String): BigInteger =
        (this[key] as? Number)?.toString()?.let(::BigInteger)
            ?: error("fixture field $key must be an integer")

    private fun MutableMap<String, Any?>.optionalBigInteger(key: String): BigInteger? =
        this[key]?.let {
            (it as? Number)?.toString()?.let(::BigInteger)
                ?: error("fixture field $key must be an integer or null")
        }

    private fun MutableMap<String, Any?>.boolean(key: String): Boolean =
        this[key] as? Boolean ?: error("fixture field $key must be a boolean")

    private fun MutableMap<String, Any?>.requireKeys(vararg expected: String) {
        assertEquals(expected.toSet(), keys)
    }

    private fun MutableMap<String, Any?>.objectValue(key: String): MutableMap<String, Any?> =
        this[key].objectValue()

    private fun MutableMap<String, Any?>.arrayValue(key: String): List<Any?> =
        this[key].arrayValue()

    @Suppress("UNCHECKED_CAST")
    private fun Any?.objectValue(): MutableMap<String, Any?> =
        this as? MutableMap<String, Any?> ?: error("fixture value must be an object")

    @Suppress("UNCHECKED_CAST")
    private fun Any?.arrayValue(): List<Any?> =
        this as? List<Any?> ?: error("fixture value must be an array")

    private fun newtypeText(value: Any?): String =
        value.arrayValue().single() as? String ?: error("fixture newtype must contain text")

    private class MutationEncoding(
        val wireId: String,
        val schemaName: String,
        val barePayload: ByteArray,
        val concreteFrame: ByteArray,
        val box: InstructionBox,
    )

    companion object {
        private const val INSTRUCTION_BOX_SCHEMA =
            "(alloc::string::String, alloc::vec::Vec<u8>)"
        private const val BATCH_EXECUTABLE_TAG = 4L
        private const val INSTRUCTION_BATCH_ITEM_TAG = 0L
        private val U64_MAX = BigInteger("18446744073709551615")
        private val BYTE_MAX = BigInteger.valueOf(255)
        private val EXPECTED_CASE_IDS = listOf(
            "accept-root-max-revision",
            "revoke-domain-invitation",
            "register-alias-domain-target",
            "assert-prerelease-digest",
            "retire-location-max-revision",
            "unyank-domain-release-high-revision",
            "remove-root-maintainer-high-revision",
            "register-domain-namespace-max-generation",
            "invite-domain-maintainer-max-expiry",
            "promote-root-member-to-owner-high-revision",
            "recover-domain-package-three-owners",
            "retarget-one-character-alias-high-revision",
            "takedown-max-major-prerelease",
            "register-archive-max-bounds-signed-receipt",
            "register-provider-bundle-attestation",
            "add-location-three-signed-providers",
            "publish-delegated-domain-release",
            "replace-domain-metadata-high-revision",
            "set-allowlisted-policy-repriced-aliases",
        )
    }
}
