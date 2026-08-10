// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.crypto.Blake3
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/**
 * Typed Kotlin constructors for the complete Musubi V1 registry mutation surface.
 *
 * Every constructor produces the registered dynamic [InstructionBox] form: the exact Rust wire
 * identifier paired with a concrete, schema-bound Norito frame. Semantic values are encoded
 * directly; no JSON map participates in the mutation wire path.
 *
 */
object MusubiInstructionsV1 {
    /** Register one immutable namespace-to-home-dataspace binding. */
    class RegisterMusubiNamespaceBindingV1(
        @JvmField val binding: MusubiNamespaceBindingV1,
        @JvmField val expectedPolicyRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedPolicyRevision,
                "registerNamespaceBinding.expectedPolicyRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeNamespaceBinding(field, binding) }
            encodeField(it) { field -> encodeU64(field, expectedPolicyRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.namespace_binding.register"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RegisterMusubiNamespaceBindingV1"
        }
    }

    /** Register one immutable canonical source archive accepted by authenticated seed ingress. */
    class RegisterMusubiArchiveV1(
        @JvmField val commitment: MusubiArchiveCommitmentV1,
        @JvmField val stagingReceipt: MusubiSeedIngressReceiptV1,
        @JvmField val expectedPolicyRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedPolicyRevision,
                "registerArchive.expectedPolicyRevision",
            )
            require(expectedPolicyRevision > BigInteger.ZERO) {
                "Registry policy revision must be non-zero"
            }
            val binding = stagingReceipt.payload.binding
            require(binding.archiveId.bytes().contentEquals(archiveIdFor(commitment))) {
                "Musubi staging receipt must bind the derived archive ID"
            }
            require(binding.carBodyDigest == commitment.carDigest &&
                binding.carBodyLength == commitment.carSize) {
                "Musubi staging receipt must bind the committed CAR body"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeArchiveCommitment(field, commitment) }
            encodeField(it) { field -> encodeSeedIngressReceipt(field, stagingReceipt) }
            encodeField(it) { field -> encodeU64(field, expectedPolicyRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.archive.register"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RegisterMusubiArchiveV1"
        }
    }

    /** Register one immutable provider proof for later location-set commitments. */
    class RegisterMusubiProviderBundleAttestationV1(
        @JvmField val attestation: MusubiProviderBundleVerificationAttestationV1,
        @JvmField val expectedLocationRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedLocationRevision,
                "registerProviderAttestation.expectedLocationRevision",
            )
            require(expectedLocationRevision > BigInteger.ZERO) {
                "Provider-attestation location revision must be non-zero"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeProviderAttestation(field, attestation) }
            encodeField(it) { field -> encodeU64(field, expectedLocationRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String =
                "iroha.musubi.v1.provider_bundle_attestation.register"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RegisterMusubiProviderBundleAttestationV1"
        }
    }

    /** Add or renew one SoraFS location bound to registered provider attestations. */
    class AddMusubiArchiveLocationV1(
        @JvmField val archiveId: MusubiDigest32V1,
        @JvmField val locationId: MusubiDigest32V1,
        @JvmField val pinManifest: MusubiDigest32V1,
        @JvmField val replicationOrder: MusubiDigest32V1,
        @JvmField val providerAttestationSetDigest: MusubiProviderBundleAttestationSetDigestV1,
        @JvmField val renewAfterEpoch: BigInteger,
        @JvmField val expiresAtEpoch: BigInteger,
        @JvmField val expectedLocationRevision: BigInteger,
    ) {
        init {
            listOf(archiveId, locationId, pinManifest, replicationOrder).forEach {
                MusubiValidationV1.requireNonZeroDigest(it, "Musubi archive-location identity")
            }
            listOf(
                "renewAfterEpoch" to renewAfterEpoch,
                "expiresAtEpoch" to expiresAtEpoch,
                "expectedLocationRevision" to expectedLocationRevision,
            ).forEach { (field, value) ->
                MusubiValidationV1.requireU64(value, "addArchiveLocation.$field")
            }
            require(renewAfterEpoch < expiresAtEpoch) {
                "Musubi archive-location renewal must precede expiry"
            }
            require(expectedLocationRevision > BigInteger.ZERO) {
                "Archive-location revision must be non-zero"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeDigestNewtype(field, archiveId) }
            encodeField(it) { field -> encodeDigestNewtype(field, locationId) }
            encodeField(it) { field -> encodeDigestNewtype(field, pinManifest) }
            encodeField(it) { field -> encodeDigestNewtype(field, replicationOrder) }
            encodeField(it) { field ->
                encodeDigestNewtype(field, providerAttestationSetDigest)
            }
            encodeField(it) { field -> encodeU64(field, renewAfterEpoch) }
            encodeField(it) { field -> encodeU64(field, expiresAtEpoch) }
            encodeField(it) { field -> encodeU64(field, expectedLocationRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.archive_location.add"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::AddMusubiArchiveLocationV1"
        }
    }

    /** Invite an account to an owner or independently permissioned maintainer role. */
    class InviteMusubiPackageMaintainerV1(
        @JvmField val packageId: MusubiPackageIdV1,
        @JvmField val inviteId: MusubiDigest32V1,
        @JvmField val invitedAccount: String,
        @JvmField val role: MusubiPackageRoleV1,
        @JvmField val expiresAtHeight: BigInteger,
        @JvmField val expectedGovernanceRevision: BigInteger,
    ) {
        private val invitedAccountPayload = TransferWirePayloadEncoder.encodeAccountIdPayload(
            requireCanonicalI105Address(
                invitedAccount,
                "invitePackageMaintainer.invitedAccount",
            ),
        )

        init {
            require(inviteId.bytes().any { it.toInt() != 0 }) {
                "Musubi maintainer invitation ID must be non-zero"
            }
            MusubiValidationV1.requireU64(
                expiresAtHeight,
                "invitePackageMaintainer.expiresAtHeight",
            )
            require(expiresAtHeight > BigInteger.ZERO) {
                "Musubi maintainer invitation expiry must be positive"
            }
            MusubiValidationV1.requireU64(
                expectedGovernanceRevision,
                "invitePackageMaintainer.expectedGovernanceRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodePackageId(field, packageId) }
            encodeField(it) { field -> encodeDigestNewtype(field, inviteId) }
            encodeField(it) { field -> field.writeBytes(invitedAccountPayload) }
            encodeField(it) { field -> encodePackageRole(field, role) }
            encodeField(it) { field -> encodeU64(field, expiresAtHeight) }
            encodeField(it) { field -> encodeU64(field, expectedGovernanceRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.package_member.invite"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::InviteMusubiPackageMaintainerV1"
        }
    }

    /** Accept a pending package-maintainer invitation. */
    class AcceptMusubiPackageMaintainerV1(
        @JvmField val packageId: MusubiPackageIdV1,
        @JvmField val inviteId: MusubiDigest32V1,
        @JvmField val expectedGovernanceRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedGovernanceRevision,
                "accept.expectedGovernanceRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodePackageId(field, packageId) }
            encodeField(it) { field -> encodeDigestNewtype(field, inviteId) }
            encodeField(it) { field -> encodeU64(field, expectedGovernanceRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.package_member.accept"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::AcceptMusubiPackageMaintainerV1"
        }
    }

    /** Revoke a pending package-maintainer invitation. */
    class RevokeMusubiPackageMaintainerInvitationV1(
        @JvmField val packageId: MusubiPackageIdV1,
        @JvmField val inviteId: MusubiDigest32V1,
        @JvmField val expectedGovernanceRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedGovernanceRevision,
                "revoke.expectedGovernanceRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodePackageId(field, packageId) }
            encodeField(it) { field -> encodeDigestNewtype(field, inviteId) }
            encodeField(it) { field -> encodeU64(field, expectedGovernanceRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String =
                "iroha.musubi.v1.package_member.invitation.revoke"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RevokeMusubiPackageMaintainerInvitationV1"
        }
    }

    /** Replace an accepted package member's owner or maintainer role. */
    class SetMusubiPackageMaintainerRoleV1(
        @JvmField val packageId: MusubiPackageIdV1,
        @JvmField val account: String,
        @JvmField val role: MusubiPackageRoleV1,
        @JvmField val expectedGovernanceRevision: BigInteger,
    ) {
        private val accountPayload = TransferWirePayloadEncoder.encodeAccountIdPayload(
            requireCanonicalI105Address(account, "setPackageMaintainerRole.account"),
        )

        init {
            MusubiValidationV1.requireU64(
                expectedGovernanceRevision,
                "setPackageMaintainerRole.expectedGovernanceRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodePackageId(field, packageId) }
            encodeField(it) { field -> field.writeBytes(accountPayload) }
            encodeField(it) { field -> encodePackageRole(field, role) }
            encodeField(it) { field -> encodeU64(field, expectedGovernanceRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.package_member.set_role"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::SetMusubiPackageMaintainerRoleV1"
        }
    }

    /** Retire one archive location without changing its archive or release identity. */
    class RetireMusubiArchiveLocationV1(
        @JvmField val archiveId: MusubiDigest32V1,
        @JvmField val locationId: MusubiDigest32V1,
        @JvmField val expectedLocationRevision: BigInteger,
        @JvmField val reason: MusubiReasonV1,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedLocationRevision,
                "retireArchiveLocation.expectedLocationRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeDigestNewtype(field, archiveId) }
            encodeField(it) { field -> encodeDigestNewtype(field, locationId) }
            encodeField(it) { field -> encodeU64(field, expectedLocationRevision) }
            encodeField(it) { field -> encodeReason(field, reason) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.archive_location.retire"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RetireMusubiArchiveLocationV1"
        }
    }

    /** Claim an absent package when authorized and publish one immutable release. */
    class PublishMusubiReleaseV1(
        @JvmField val namespace: MusubiNamespaceV1,
        @JvmField val publication: MusubiPublicationV1,
        @JvmField val namespaceDelegation: MusubiNamespaceDelegationV1?,
        @JvmField val expectedPolicyRevision: BigInteger,
        @JvmField val expectedGovernanceRevision: BigInteger?,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedPolicyRevision,
                "publishRelease.expectedPolicyRevision",
            )
            require(expectedPolicyRevision > BigInteger.ZERO) {
                "Registry policy revision must be non-zero"
            }
            expectedGovernanceRevision?.let {
                MusubiValidationV1.requireU64(it, "publishRelease.expectedGovernanceRevision")
                require(it > BigInteger.ZERO) {
                    "Existing-package governance revision must be non-zero"
                }
            }
            require(
                MusubiValidationV1.namespaceMatchesScope(
                    publication.manifest.release.packageId,
                    namespace,
                ),
            ) { "Musubi publication namespace does not match the structural package scope" }
            val lockDigest = verificationLockDigestFor(publication.resolution.lock)
            require(
                lockDigest.contentEquals(publication.manifest.verificationLockDigest.bytes()),
            ) {
                "Musubi publication verification-lock digest does not bind the exact lock"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeNamespace(field, namespace) }
            encodeField(it) { field -> encodePublication(field, publication) }
            encodeField(it) { field ->
                encodeOption(field, namespaceDelegation) { value, delegation ->
                    encodeNamespaceDelegation(value, delegation)
                }
            }
            encodeField(it) { field -> encodeU64(field, expectedPolicyRevision) }
            encodeField(it) { field ->
                encodeOption(field, expectedGovernanceRevision) { value, revision ->
                    encodeU64(value, revision)
                }
            }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.release.publish"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::PublishMusubiReleaseV1"
        }
    }

    /** Yank or unyank an immutable release. */
    class SetMusubiReleaseYankV1(
        @JvmField val release: MusubiReleaseIdV1,
        @JvmField val yanked: Boolean,
        @JvmField val reason: MusubiReasonV1,
        @JvmField val expectedYankRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedYankRevision,
                "setReleaseYank.expectedYankRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeReleaseId(field, release) }
            encodeField(it) { field -> encodeBool(field, yanked) }
            encodeField(it) { field -> encodeReason(field, reason) }
            encodeField(it) { field -> encodeU64(field, expectedYankRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.release_yank.set"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::SetMusubiReleaseYankV1"
        }
    }

    /** Replace the complete mutable metadata projection for one package. */
    class SetMusubiPackageMetadataV1(
        @JvmField val packageId: MusubiPackageIdV1,
        @JvmField val metadata: MusubiReleaseMetadataV1,
        @JvmField val expectedMetadataRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedMetadataRevision,
                "setPackageMetadata.expectedMetadataRevision",
            )
            require(expectedMetadataRevision > BigInteger.ZERO) {
                "Package metadata revision must be non-zero"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodePackageId(field, packageId) }
            encodeField(it) { field -> encodeReleaseMetadata(field, metadata) }
            encodeField(it) { field -> encodeU64(field, expectedMetadataRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.package_metadata.set"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::SetMusubiPackageMetadataV1"
        }
    }

    /** Remove an accepted package member using compare-and-set governance. */
    class RemoveMusubiPackageMaintainerV1(
        @JvmField val packageId: MusubiPackageIdV1,
        @JvmField val account: String,
        @JvmField val expectedGovernanceRevision: BigInteger,
    ) {
        private val accountPayload = TransferWirePayloadEncoder.encodeAccountIdPayload(
            requireCanonicalI105Address(account, "removePackageMaintainer.account"),
        )

        init {
            MusubiValidationV1.requireU64(
                expectedGovernanceRevision,
                "removePackageMaintainer.expectedGovernanceRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodePackageId(field, packageId) }
            encodeField(it) { field -> field.writeBytes(accountPayload) }
            encodeField(it) { field -> encodeU64(field, expectedGovernanceRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.package_member.remove"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RemoveMusubiPackageMaintainerV1"
        }
    }

    /** Register a paid, permanent global alias for a canonical package identity. */
    class RegisterMusubiAliasV1(
        @JvmField val alias: MusubiAliasNameV1,
        @JvmField val target: MusubiPackageIdV1,
        @JvmField val expectedPricingRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedPricingRevision,
                "registerAlias.expectedPricingRevision",
            )
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeAliasName(field, alias) }
            encodeField(it) { field -> encodePackageId(field, target) }
            encodeField(it) { field -> encodeU64(field, expectedPricingRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.alias.register"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RegisterMusubiAliasV1"
        }
    }

    /** Assert the immutable digest of one exact package release. */
    class AssertMusubiReleaseDigestV1(
        @JvmField val release: MusubiReleaseIdV1,
        @JvmField val expectedDigest: MusubiDigest32V1,
    ) {
        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeReleaseId(field, release) }
            encodeField(it) { field -> encodeDigestNewtype(field, expectedDigest) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.release_digest.assert"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::AssertMusubiReleaseDigestV1"
        }
    }

    /** Recover package ownership under one enacted, delayed Parliament decision. */
    class RecoverMusubiPackageV1(
        @JvmField val decision: MusubiGovernanceDecisionV1,
        @JvmField val packageId: MusubiPackageIdV1,
        owners: List<String>,
        @JvmField val expectedGovernanceRevision: BigInteger,
    ) {
        /** Canonical owners in Rust [AccountId] ordering. */
        @JvmField val owners: List<String>
        private val ownerPayloads: List<ByteArray>

        init {
            require(owners.size in 1..64) {
                "Musubi package recovery must carry between 1 and 64 owners"
            }
            MusubiValidationV1.requireU64(
                expectedGovernanceRevision,
                "recoverPackage.expectedGovernanceRevision",
            )
            require(expectedGovernanceRevision > BigInteger.ZERO) {
                "Musubi package recovery governance revision must be non-zero"
            }

            val payloads = ArrayList<ByteArray>(owners.size)
            val orderKeys = ArrayList<MusubiAccountOrderKeyV1>(owners.size)
            owners.forEachIndexed { index, owner ->
                val canonical = requireCanonicalI105Address(
                    owner,
                    "recoverPackage.owners[$index]",
                )
                payloads += TransferWirePayloadEncoder.encodeAccountIdPayload(canonical)
                orderKeys += musubiAccountOrderKeyV1(canonical)
            }
            require(orderKeys.zipWithNext().all { (left, right) ->
                compareMusubiAccountOrderKeysV1(left, right) < 0
            }) {
                "Musubi package recovery owners must be sorted and distinct in AccountId wire order"
            }
            this.owners = owners.toList()
            ownerPayloads = payloads
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeGovernanceDecision(field, decision) }
            encodeField(it) { field -> encodePackageId(field, packageId) }
            encodeField(it) { field -> encodeAccountIds(field, ownerPayloads) }
            encodeField(it) { field -> encodeU64(field, expectedGovernanceRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.parliament.package_recover"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RecoverMusubiPackageV1"
        }
    }

    /** Retarget one permanent alias under an enacted Parliament recovery decision. */
    class RetargetMusubiAliasV1(
        @JvmField val decision: MusubiGovernanceDecisionV1,
        @JvmField val alias: MusubiAliasNameV1,
        @JvmField val target: MusubiPackageIdV1,
        @JvmField val expectedHistoryRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedHistoryRevision,
                "retargetAlias.expectedHistoryRevision",
            )
            require(expectedHistoryRevision > BigInteger.ZERO) {
                "Musubi alias history revision must be non-zero"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeGovernanceDecision(field, decision) }
            encodeField(it) { field -> encodeAliasName(field, alias) }
            encodeField(it) { field -> encodePackageId(field, target) }
            encodeField(it) { field -> encodeU64(field, expectedHistoryRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.parliament.alias_retarget"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::RetargetMusubiAliasV1"
        }
    }

    /** Mark one exact release artifact unavailable under an enacted Parliament decision. */
    class SetMusubiArtifactTakedownV1(
        @JvmField val decision: MusubiGovernanceDecisionV1,
        @JvmField val release: MusubiReleaseIdV1,
        @JvmField val reason: MusubiReasonV1,
        @JvmField val expectedArtifactGovernanceRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedArtifactGovernanceRevision,
                "setArtifactTakedown.expectedArtifactGovernanceRevision",
            )
            require(expectedArtifactGovernanceRevision > BigInteger.ZERO) {
                "Musubi artifact governance revision must be non-zero"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeGovernanceDecision(field, decision) }
            encodeField(it) { field -> encodeReleaseId(field, release) }
            encodeField(it) { field -> encodeReason(field, reason) }
            encodeField(it) { field ->
                encodeU64(field, expectedArtifactGovernanceRevision)
            }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.parliament.artifact_takedown"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::SetMusubiArtifactTakedownV1"
        }
    }

    /** Replace prospective registry admission and alias pricing under Parliament authority. */
    class SetMusubiRegistryPolicyV1(
        @JvmField val decision: MusubiGovernanceDecisionV1,
        @JvmField val policy: MusubiRegistryPolicyV1,
        @JvmField val expectedPolicyRevision: BigInteger,
    ) {
        init {
            MusubiValidationV1.requireU64(
                expectedPolicyRevision,
                "setRegistryPolicy.expectedPolicyRevision",
            )
            require(expectedPolicyRevision > BigInteger.ZERO &&
                expectedPolicyRevision < MusubiValidationV1.U64_MAX &&
                policy.revision == expectedPolicyRevision.add(BigInteger.ONE)) {
                "Musubi replacement policy must be the exact revision successor"
            }
            val actionDigest = domainHash(
                PARLIAMENT_ACTION_DOMAIN,
                encodeSetRegistryPolicyParliamentAction(policy, expectedPolicyRevision),
            )
            require(actionDigest.contentEquals(decision.actionDigest.bytes())) {
                "Musubi Parliament decision action digest does not bind this policy replacement"
            }
        }

        /** Return the canonical headerless Rust payload. */
        fun barePayload(): ByteArray = encodeBare {
            encodeField(it) { field -> encodeGovernanceDecision(field, decision) }
            encodeField(it) { field -> encodeRegistryPolicy(field, policy) }
            encodeField(it) { field -> encodeU64(field, expectedPolicyRevision) }
        }

        /** Return the concrete schema-bound Norito frame registered by core. */
        fun concreteFrame(): ByteArray = frame(SCHEMA_NAME, barePayload())

        /** Return the dynamic V1 instruction box submitted in a transaction. */
        fun toInstructionBox(): InstructionBox =
            InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

        companion object {
            /** Stable dynamic instruction registry identifier. */
            const val WIRE_ID: String = "iroha.musubi.v1.parliament.registry_policy.set"

            /** Exact Rust concrete type name used for the Norito schema hash. */
            const val SCHEMA_NAME: String =
                "iroha_data_model::isi::musubi::SetMusubiRegistryPolicyV1"
        }
    }
}

private object RawPayloadAdapter : TypeAdapter<ByteArray> {
    override fun encode(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeBytes(value)
    }

    override fun decode(decoder: NoritoDecoder): ByteArray =
        decoder.readBytes(decoder.remaining())
}

private fun encodeBare(block: (NoritoEncoder) -> Unit): ByteArray =
    NoritoEncoder(NoritoHeader.COMPACT_LEN).also(block).toByteArray()

internal fun musubiProviderBundleAttestationDigestV1(
    attestation: MusubiProviderBundleVerificationAttestationV1,
): ByteArray = domainHash(
    PROVIDER_BUNDLE_ATTESTATION_DIGEST_DOMAIN,
    encodeBare { encodeProviderAttestation(it, attestation) },
)

internal fun musubiReleaseManifestDigestV1(
    manifest: MusubiReleaseManifestV1,
): ByteArray = domainHash(
    RELEASE_DIGEST_DOMAIN,
    encodeBare { encodeReleaseManifest(it, manifest) },
)

private fun frame(schemaName: String, barePayload: ByteArray): ByteArray =
    NoritoCodec.encode(
        barePayload,
        schemaName,
        RawPayloadAdapter,
        NoritoHeader.COMPACT_LEN,
    )

private fun encodeField(
    encoder: NoritoEncoder,
    block: (NoritoEncoder) -> Unit,
) {
    val child = encoder.childEncoder().also(block)
    val payload = child.toByteArray()
    encoder.writeLength(
        payload.size.toLong(),
        (encoder.flags and NoritoHeader.COMPACT_LEN) != 0,
    )
    encoder.writeBytes(payload)
}

private fun encodePackageId(encoder: NoritoEncoder, packageId: MusubiPackageIdV1) {
    encodeField(encoder) { field -> encodeDataSpaceId(field, packageId.homeDataspace) }
    encodeField(encoder) { field -> encodePackageScope(field, packageId.scope) }
    encodeField(encoder) { field -> encodePackageName(field, packageId.name) }
}

private fun encodeNamespaceBinding(
    encoder: NoritoEncoder,
    binding: MusubiNamespaceBindingV1,
) {
    encodeField(encoder) { field -> encodeNamespace(field, binding.namespace) }
    encodeField(encoder) { field -> encodeDataSpaceId(field, binding.homeDataspace) }
    encodeField(encoder) { field -> encodePackageScope(field, binding.scope) }
    encodeField(encoder) { field -> encodeU64(field, binding.generation) }
}

private fun encodeNamespace(encoder: NoritoEncoder, namespace: MusubiNamespaceV1) {
    encodeField(encoder) { field -> encodeString(field, namespace.value) }
}

private fun encodeDataSpaceId(encoder: NoritoEncoder, value: BigInteger) {
    encodeField(encoder) { field -> encodeU64(field, value) }
}

private fun encodePackageScope(encoder: NoritoEncoder, scope: MusubiPackageScopeV1) {
    when (scope.kind) {
        MusubiPackageScopeV1.Kind.DATASPACE_ROOT -> encoder.writeUInt(0, 32)
        MusubiPackageScopeV1.Kind.DOMAIN -> {
            encoder.writeUInt(1, 32)
            encodeField(encoder) { field -> encodeString(field, requireNotNull(scope.domain)) }
        }
    }
}

private fun encodePackageRole(encoder: NoritoEncoder, role: MusubiPackageRoleV1) {
    when (role.kind) {
        MusubiPackageRoleV1.Kind.OWNER -> encoder.writeUInt(0, 32)
        MusubiPackageRoleV1.Kind.MAINTAINER -> {
            encoder.writeUInt(1, 32)
            encodeField(encoder) { field ->
                encodeMaintainerPermissions(field, requireNotNull(role.permissions))
            }
        }
    }
}

private fun encodeMaintainerPermissions(
    encoder: NoritoEncoder,
    permissions: MusubiMaintainerPermissionsV1,
) {
    encodeField(encoder) { field -> encodeBool(field, permissions.publish) }
    encodeField(encoder) { field -> encodeBool(field, permissions.yank) }
    encodeField(encoder) { field -> encodeBool(field, permissions.metadata) }
    encodeField(encoder) { field -> encodeBool(field, permissions.archiveLocations) }
}

private fun encodePackageName(encoder: NoritoEncoder, name: MusubiPackageNameV1) {
    encodeField(encoder) { field -> encodeString(field, name.value) }
}

private fun encodeAliasName(encoder: NoritoEncoder, alias: MusubiAliasNameV1) {
    encodeField(encoder) { field -> encodeString(field, alias.value) }
}

private fun encodeReason(encoder: NoritoEncoder, reason: MusubiReasonV1) {
    encodeField(encoder) { field -> encodeString(field, reason.value) }
}

private fun encodeGovernanceDecision(
    encoder: NoritoEncoder,
    decision: MusubiGovernanceDecisionV1,
) {
    encodeField(encoder) { field -> field.writeBytes(decision.decisionId()) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, decision.actionDigest) }
    encodeField(encoder) { field -> encodeU64(field, decision.enactedAtHeight) }
    encodeField(encoder) { field -> encodeU64(field, decision.executeAfterHeight) }
}

private fun encodeAccountIds(encoder: NoritoEncoder, payloads: List<ByteArray>) {
    encoder.writeUInt(payloads.size.toLong(), 64)
    payloads.forEach { payload ->
        encodeField(encoder) { field -> field.writeBytes(payload) }
    }
}

private fun encodeReleaseId(encoder: NoritoEncoder, release: MusubiReleaseIdV1) {
    encodeField(encoder) { field -> encodePackageId(field, release.packageId) }
    encodeField(encoder) { field -> encodeVersion(field, release.version) }
}

private fun encodeVersion(encoder: NoritoEncoder, version: MusubiVersionV1) {
    encodeField(encoder) { field -> encodeU64(field, version.major) }
    encodeField(encoder) { field -> encodeU64(field, version.minor) }
    encodeField(encoder) { field -> encodeU64(field, version.patch) }
    encodeField(encoder) { field -> encodePrerelease(field, version.prerelease) }
}

private fun encodePrerelease(
    encoder: NoritoEncoder,
    identifiers: List<MusubiPrereleaseIdentifierV1>,
) {
    encoder.writeUInt(identifiers.size.toLong(), 64)
    identifiers.forEach { identifier ->
        encodeField(encoder) { field ->
            val numeric = identifier.numeric
            if (numeric != null) {
                field.writeUInt(0, 32)
                encodeField(field) { value -> encodeU64(value, numeric) }
            } else {
                field.writeUInt(1, 32)
                encodeField(field) { value ->
                    encodeString(value, requireNotNull(identifier.alphaNumeric))
                }
            }
        }
    }
}

private fun encodeDigestNewtype(encoder: NoritoEncoder, digest: MusubiDigest32V1) {
    encodeField(encoder) { field -> field.writeBytes(digest.bytes()) }
}

private fun encodeArchiveCommitment(
    encoder: NoritoEncoder,
    commitment: MusubiArchiveCommitmentV1,
) {
    encodeField(encoder) { field -> encodeManifestRootCid(field, commitment.rootCid()) }
    encodeField(encoder) { field -> encodeChunkerProfile(field, commitment.chunker) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, commitment.chunkPlanDigest) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, commitment.porRoot) }
    encodeField(encoder) { field -> encodeU64(field, commitment.contentLength) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, commitment.carDigest) }
    encodeField(encoder) { field -> encodeU64(field, commitment.carSize) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, commitment.bundleDigest) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, commitment.sourceTreeDigest) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, commitment.descriptorDigest) }
    encodeField(encoder) { field -> field.writeUInt(commitment.fileCount, 32) }
    encodeField(encoder) { field -> field.writeUInt(commitment.chunkCount, 32) }
}

private fun encodeChunkerProfile(
    encoder: NoritoEncoder,
    chunker: MusubiChunkerProfileHandleV1,
) {
    encodeField(encoder) { field -> field.writeUInt(chunker.profileId, 32) }
    encodeField(encoder) { field -> encodeString(field, chunker.namespace) }
    encodeField(encoder) { field -> encodeString(field, chunker.name) }
    encodeField(encoder) { field -> encodeString(field, chunker.semver) }
    encodeField(encoder) { field -> encodeU64(field, chunker.multihashCode) }
}

private fun encodeSeedIngressReceipt(
    encoder: NoritoEncoder,
    receipt: MusubiSeedIngressReceiptV1,
) {
    encodeField(encoder) { field -> encodeSeedIngressPayload(field, receipt.payload) }
    encodeField(encoder) { field ->
        encodeSequence(field, receipt.approvals) { element, approval ->
            encodeSeedIngressApproval(element, approval)
        }
    }
}

private fun encodeSeedIngressPayload(
    encoder: NoritoEncoder,
    payload: MusubiSeedIngressReceiptPayloadV1,
) {
    encodeField(encoder) { field -> field.writeByte(1) }
    encodeField(encoder) { field -> encodeSeedIngressBinding(field, payload.binding) }
    encodeField(encoder) { field -> encodeU64(field, payload.issuedAtMs) }
    encodeField(encoder) { field -> encodeU64(field, payload.expiresAtMs) }
}

private fun encodeSeedIngressBinding(
    encoder: NoritoEncoder,
    binding: MusubiSeedIngressReceiptBindingV1,
) {
    encodeField(encoder) { field -> field.writeBytes(binding.networkId.bytes()) }
    encodeField(encoder) { field -> field.writeBytes(binding.publisherPayload) }
    encodeField(encoder) { field -> field.writeBytes(binding.ingressBrokerPayload) }
    encodeField(encoder) { field -> encodeFixed32Newtype(field, binding.seedProviderPayload) }
    encodeField(encoder) { field ->
        encodeDigestNewtype(field, binding.semanticReleaseManifestDigest)
    }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.archiveId) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.carBodyDigest) }
    encodeField(encoder) { field -> encodeU64(field, binding.carBodyLength) }
    encodeField(encoder) { field -> field.writeBytes(binding.nonce()) }
}

private fun encodeSeedIngressApproval(
    encoder: NoritoEncoder,
    approval: MusubiSeedIngressReceiptApprovalV1,
) {
    encodeField(encoder) { field -> encodeByteVector(field, approval.publicKeyPayload) }
    encodeField(encoder) { field -> encodeByteVector(field, approval.signaturePayload) }
}

private fun encodeProviderAttestation(
    encoder: NoritoEncoder,
    attestation: MusubiProviderBundleVerificationAttestationV1,
) {
    encodeField(encoder) { field -> encodeProviderPayload(field, attestation.payload) }
    encodeField(encoder) { field ->
        encodeSequence(field, attestation.approvals) { element, approval ->
            encodeProviderApproval(element, approval)
        }
    }
}

private fun encodeProviderPayload(
    encoder: NoritoEncoder,
    payload: MusubiProviderBundleVerificationPayloadV1,
) {
    encodeField(encoder) { field -> field.writeByte(1) }
    encodeField(encoder) { field -> encodeProviderBinding(field, payload.binding) }
}

private fun encodeProviderBinding(
    encoder: NoritoEncoder,
    binding: MusubiProviderBundleVerificationBindingV1,
) {
    encodeField(encoder) { field -> field.writeBytes(binding.networkId.bytes()) }
    encodeField(encoder) { field -> encodeFixed32Newtype(field, binding.providerIdPayload) }
    encodeField(encoder) { field -> field.writeBytes(binding.completedByPayload) }
    encodeField(encoder) { field ->
        encodeProviderCompletionAuthority(field, binding.completionAuthority)
    }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.replicationOrder) }
    encodeField(encoder) { field -> encodeU64(field, binding.assignmentRevision) }
    encodeField(encoder) { field -> encodeU64(field, binding.completionEpoch) }
    encodeField(encoder) { field -> encodeProviderFinalizedAnchor(field, binding.finalizedAnchor) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.archiveId) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.bundleDigest) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.descriptorDigest) }
    encodeField(encoder) { field ->
        encodeDigestNewtype(field, binding.semanticReleaseManifestDigest)
    }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.verificationLockDigest) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, binding.sourceTreeDigest) }
}

private fun encodeProviderCompletionAuthority(
    encoder: NoritoEncoder,
    authority: MusubiProviderIngestCompletionAuthorityV1,
) {
    encodeField(encoder) { field -> field.writeBytes(authority.providerOwnerPayload) }
    encodeField(encoder) { field -> encodeProviderSignerPolicy(field, authority.signerPolicy) }
}

private fun encodeProviderSignerPolicy(
    encoder: NoritoEncoder,
    policy: MusubiProviderIngestCompletionSignerPolicyV1,
) {
    encodeField(encoder) { field -> field.writeBytes(policy.policyId()) }
    encodeField(encoder) { field -> encodeU64(field, policy.revision) }
    encodeField(encoder) { field ->
        encodeOption(field, policy.predecessorDigest()) { value, digest ->
            encodeFixedByteArrayElements(value, digest)
        }
    }
    encodeField(encoder) { field -> field.writeBytes(policy.policyDigest()) }
}

private fun encodeProviderFinalizedAnchor(
    encoder: NoritoEncoder,
    anchor: MusubiProviderIngestFinalizedAnchorV1,
) {
    encodeField(encoder) { field -> encodeU64(field, anchor.height) }
    encodeField(encoder) { field -> field.writeBytes(anchor.blockHash()) }
}

private fun encodeProviderApproval(
    encoder: NoritoEncoder,
    approval: MusubiProviderBundleVerificationApprovalV1,
) {
    encodeField(encoder) { field -> encodeByteVector(field, approval.publicKeyPayload) }
    encodeField(encoder) { field -> encodeByteVector(field, approval.signaturePayload) }
}

private fun encodePublication(encoder: NoritoEncoder, publication: MusubiPublicationV1) {
    encodeField(encoder) { field -> encodeReleaseManifest(field, publication.manifest) }
    encodeField(encoder) { field -> encodeResolutionProof(field, publication.resolution) }
}

private fun encodeReleaseManifest(
    encoder: NoritoEncoder,
    manifest: MusubiReleaseManifestV1,
) {
    encodeField(encoder) { field -> encodeReleaseId(field, manifest.release) }
    encodeField(encoder) { field -> encodeKotodamaEdition(field, manifest.edition) }
    encodeField(encoder) { field -> encodeAbiBinding(field, manifest.abi) }
    encodeField(encoder) { field ->
        encodeSequence(field, manifest.dependencies) { element, dependency ->
            encodeDependencyReq(element, dependency)
        }
    }
    encodeField(encoder) { field ->
        encodeSequence(field, manifest.exports) { element, export ->
            encodeName(element, export)
        }
    }
    encodeField(encoder) { field -> encodeDigestNewtype(field, manifest.interfaceDigest) }
    encodeField(encoder) { field -> encodeReleaseMetadata(field, manifest.metadata) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, manifest.archiveId) }
    encodeField(encoder) { field ->
        encodeDigestNewtype(field, manifest.verificationLockDigest)
    }
}

private fun encodeKotodamaEdition(
    encoder: NoritoEncoder,
    edition: MusubiKotodamaEditionV1,
) {
    require(edition == MusubiKotodamaEditionV1.V1)
    encoder.writeUInt(0, 32)
}

private fun encodeAbiBinding(encoder: NoritoEncoder, abi: MusubiAbiBindingV1) {
    encodeField(encoder) { field -> field.writeUInt(abi.abiVersion.toLong(), 16) }
    encodeField(encoder) { field -> field.writeBytes(abi.abiHash()) }
}

private fun encodeDependencyReq(encoder: NoritoEncoder, dependency: MusubiDependencyReqV1) {
    encodeField(encoder) { field -> encodeName(field, dependency.alias) }
    encodeField(encoder) { field -> encodePackageId(field, dependency.packageId) }
    encodeField(encoder) { field -> encodeVersionReq(field, dependency.requirement) }
}

private fun encodeVersionReq(encoder: NoritoEncoder, requirement: MusubiVersionReqV1) {
    encoder.writeUInt(requirement.kind.ordinal.toLong(), 32)
    when (requirement.kind) {
        MusubiVersionReqV1.Kind.ANY -> Unit
        MusubiVersionReqV1.Kind.CARET,
        MusubiVersionReqV1.Kind.TILDE,
        MusubiVersionReqV1.Kind.EXACT,
        -> encodeField(encoder) { field -> encodeVersion(field, requireNotNull(requirement.version)) }
        MusubiVersionReqV1.Kind.MAJOR_WILDCARD ->
            encodeField(encoder) { field -> encodeU64(field, requireNotNull(requirement.major)) }
        MusubiVersionReqV1.Kind.MINOR_WILDCARD -> encodeField(encoder) { field ->
            encodeField(field) { value -> encodeU64(value, requireNotNull(requirement.major)) }
            encodeField(field) { value -> encodeU64(value, requireNotNull(requirement.minor)) }
        }
        MusubiVersionReqV1.Kind.COMPARATORS -> encodeField(encoder) { field ->
            encodeSequence(field, requirement.comparators) { element, comparator ->
                encodeVersionComparator(element, comparator)
            }
        }
    }
}

private fun encodeVersionComparator(
    encoder: NoritoEncoder,
    comparator: MusubiVersionComparatorV1,
) {
    encodeField(encoder) { field -> field.writeUInt(comparator.op.ordinal.toLong(), 32) }
    encodeField(encoder) { field -> encodeVersion(field, comparator.version) }
}

private fun encodeReleaseMetadata(
    encoder: NoritoEncoder,
    metadata: MusubiReleaseMetadataV1,
) {
    encodeField(encoder) { field ->
        encodeOption(field, metadata.description) { value, text ->
            encodeTextNewtype(value, text.value)
        }
    }
    encodeField(encoder) { field ->
        encodeOption(field, metadata.readme) { value, text ->
            encodeTextNewtype(value, text.value)
        }
    }
    encodeField(encoder) { field ->
        encodeOption(field, metadata.license) { value, text ->
            encodeTextNewtype(value, text.value)
        }
    }
    encodeField(encoder) { field ->
        encodeOption(field, metadata.repository) { value, text ->
            encodeTextNewtype(value, text.value)
        }
    }
    encodeField(encoder) { field ->
        encodeSequence(field, metadata.keywords) { element, keyword ->
            encodeTextNewtype(element, keyword.value)
        }
    }
}

private fun encodeResolutionProof(encoder: NoritoEncoder, proof: MusubiResolutionProofV1) {
    encodeField(encoder) { field -> encodeRegistrySnapshot(field, proof.snapshot) }
    encodeField(encoder) { field -> encodeVerificationLock(field, proof.lock) }
}

private fun encodeRegistrySnapshot(encoder: NoritoEncoder, snapshot: MusubiRegistrySnapshotV1) {
    encodeField(encoder) { field -> encodeU64(field, snapshot.finalizedHeight) }
    encodeField(encoder) { field -> field.writeBytes(snapshot.finalizedBlockHash()) }
    encodeField(encoder) { field -> encodeU64(field, snapshot.indexRevision) }
}

private fun encodeVerificationLock(encoder: NoritoEncoder, lock: MusubiVerificationLockV1) {
    encodeField(encoder) { field -> encodeString(field, lock.schema) }
    encodeField(encoder) { field -> field.writeByte(lock.version) }
    encodeField(encoder) { field -> encodeReleaseId(field, lock.root) }
    encodeField(encoder) { field ->
        encodeSequence(field, lock.rootDependencies) { element, dependency ->
            encodeExactDependencyEdge(element, dependency)
        }
    }
    encodeField(encoder) { field ->
        encodeSequence(field, lock.nodes) { element, node ->
            encodeVerificationNode(element, node)
        }
    }
}

private fun encodeExactDependencyEdge(
    encoder: NoritoEncoder,
    edge: MusubiExactDependencyEdgeV1,
) {
    encodeField(encoder) { field -> encodeName(field, edge.alias) }
    encodeField(encoder) { field -> field.writeUInt(edge.kind.ordinal.toLong(), 32) }
    encodeField(encoder) { field -> encodePackageId(field, edge.packageId) }
    encodeField(encoder) { field -> encodeVersionReq(field, edge.requirement) }
    encodeField(encoder) { field -> encodeReleaseId(field, edge.selected) }
}

private fun encodeVerificationNode(encoder: NoritoEncoder, node: MusubiVerificationNodeV1) {
    encodeField(encoder) { field -> encodeReleaseId(field, node.release) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, node.releaseDigest) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, node.archiveId) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, node.sourceDigest) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, node.interfaceDigest) }
    encodeField(encoder) { field -> encodeAbiBinding(field, node.abi) }
    encodeField(encoder) { field ->
        encodeSequence(field, node.dependencies) { element, dependency ->
            encodeExactDependencyEdge(element, dependency)
        }
    }
}

private fun encodeNamespaceDelegation(
    encoder: NoritoEncoder,
    delegation: MusubiNamespaceDelegationV1,
) {
    encodeField(encoder) { field -> encodeNamespaceDelegationPayload(field, delegation.payload) }
    encodeField(encoder) { field ->
        encodeSequence(field, delegation.approvals) { element, approval ->
            encodeNamespaceDelegationApproval(element, approval)
        }
    }
}

private fun encodeNamespaceDelegationPayload(
    encoder: NoritoEncoder,
    payload: MusubiNamespaceDelegationPayloadV1,
) {
    encodeField(encoder) { field -> field.writeByte(payload.version) }
    encodeField(encoder) { field -> encodeDigestNewtype(field, payload.namespaceBinding) }
    encodeField(encoder) { field -> encodeU64(field, payload.ownerGeneration) }
    encodeField(encoder) { field -> field.writeBytes(payload.ownerPayload) }
    encodeField(encoder) { field -> field.writeBytes(payload.delegatePayload) }
    encodeField(encoder) { field -> encodeU64(field, payload.expiresAtHeight) }
}

private fun encodeNamespaceDelegationApproval(
    encoder: NoritoEncoder,
    approval: MusubiNamespaceDelegationApprovalV1,
) {
    encodeField(encoder) { field -> encodeByteVector(field, approval.publicKeyPayload) }
    encodeField(encoder) { field -> encodeByteVector(field, approval.signaturePayload) }
}

private fun encodeRegistryPolicy(encoder: NoritoEncoder, policy: MusubiRegistryPolicyV1) {
    encodeField(encoder) { field -> field.writeByte(policy.version) }
    encodeField(encoder) { field -> encodeU64(field, policy.revision) }
    encodeField(encoder) { field -> field.writeUInt(policy.mode.ordinal.toLong(), 32) }
    encodeField(encoder) { field ->
        encodeSequence(field, policy.allowlistedDataspaces) { element, dataspace ->
            encodeDataSpaceId(element, dataspace)
        }
    }
    encodeField(encoder) { field -> encodeAliasPricingPolicy(field, policy.aliasPricing) }
}

private fun encodeAliasPricingPolicy(
    encoder: NoritoEncoder,
    policy: MusubiAliasPricingPolicyV1,
) {
    encodeField(encoder) { field -> encodeU64(field, policy.revision) }
    encodeField(encoder) { field -> encodeU64(field, policy.length1Xor) }
    encodeField(encoder) { field -> encodeU64(field, policy.length2Xor) }
    encodeField(encoder) { field -> encodeU64(field, policy.length3Xor) }
    encodeField(encoder) { field -> encodeU64(field, policy.length4Xor) }
    encodeField(encoder) { field -> encodeU64(field, policy.length5To32Xor) }
}

private fun encodeName(encoder: NoritoEncoder, name: String) {
    encodeString(encoder, name)
}

private fun encodeTextNewtype(encoder: NoritoEncoder, value: String) {
    encodeField(encoder) { field -> encodeString(field, value) }
}

private fun encodeFixed32Newtype(encoder: NoritoEncoder, bytes: ByteArray) {
    require(bytes.size == 32)
    encodeField(encoder) { field -> field.writeBytes(bytes) }
}

private fun encodeManifestRootCid(encoder: NoritoEncoder, bytes: ByteArray) {
    require(bytes.size == 36)
    encodeFixedByteArrayElements(encoder, bytes)
}

private fun encodeFixedByteArrayElements(encoder: NoritoEncoder, bytes: ByteArray) {
    bytes.forEach { byte -> encodeField(encoder) { field -> field.writeByte(byte.toInt()) } }
}

private fun encodeByteVector(encoder: NoritoEncoder, bytes: ByteArray) {
    encoder.writeUInt(bytes.size.toLong(), 64)
    bytes.forEach { byte -> encodeField(encoder) { field -> field.writeByte(byte.toInt()) } }
}

private fun <T> encodeSequence(
    encoder: NoritoEncoder,
    values: List<T>,
    encodeElement: (NoritoEncoder, T) -> Unit,
) {
    encoder.writeUInt(values.size.toLong(), 64)
    values.forEach { value ->
        encodeField(encoder) { element -> encodeElement(element, value) }
    }
}

private fun <T : Any> encodeOption(
    encoder: NoritoEncoder,
    value: T?,
    encodeValue: (NoritoEncoder, T) -> Unit,
) {
    if (value == null) {
        encoder.writeByte(0)
    } else {
        encoder.writeByte(1)
        encodeField(encoder) { payload -> encodeValue(payload, value) }
    }
}

private fun archiveIdFor(commitment: MusubiArchiveCommitmentV1): ByteArray = domainHash(
    ARCHIVE_ID_DOMAIN,
    encodeBare { encodeArchiveCommitment(it, commitment) },
)

private fun verificationLockDigestFor(lock: MusubiVerificationLockV1): ByteArray = domainHash(
    VERIFICATION_LOCK_DOMAIN,
    encodeBare { encodeVerificationLock(it, lock) },
)

private fun encodeSetRegistryPolicyParliamentAction(
    policy: MusubiRegistryPolicyV1,
    expectedRevision: BigInteger,
): ByteArray = encodeBare { encoder ->
    encoder.writeUInt(3, 32)
    encodeField(encoder) { action ->
        encodeField(action) { field -> encodeRegistryPolicy(field, policy) }
        encodeField(action) { field -> encodeU64(field, expectedRevision) }
    }
}

private fun domainHash(domain: ByteArray, payload: ByteArray): ByteArray {
    val preimage = ByteArray(16 + domain.size + payload.size)
    writeLittleEndianU64(preimage, 0, domain.size.toLong())
    System.arraycopy(domain, 0, preimage, 8, domain.size)
    writeLittleEndianU64(preimage, 8 + domain.size, payload.size.toLong())
    System.arraycopy(payload, 0, preimage, 16 + domain.size, payload.size)
    return Blake3.hash(preimage)
}

private fun writeLittleEndianU64(target: ByteArray, offset: Int, value: Long) {
    var remaining = value
    repeat(java.lang.Long.BYTES) { index ->
        target[offset + index] = remaining.toByte()
        remaining = remaining ushr 8
    }
}

private fun encodeBool(encoder: NoritoEncoder, value: Boolean) {
    encoder.writeByte(if (value) 1 else 0)
}

private fun encodeString(encoder: NoritoEncoder, value: String) {
    val bytes = value.toByteArray(StandardCharsets.UTF_8)
    encoder.writeLength(
        bytes.size.toLong(),
        (encoder.flags and NoritoHeader.COMPACT_LEN) != 0,
    )
    encoder.writeBytes(bytes)
}

private fun encodeU64(encoder: NoritoEncoder, value: BigInteger) {
    MusubiValidationV1.requireU64(value, "Musubi u64 wire value")
    var remaining = value
    repeat(java.lang.Long.BYTES) {
        encoder.writeByte(remaining.and(U8_MASK).toInt())
        remaining = remaining.shiftRight(8)
    }
}

private val U8_MASK = BigInteger.valueOf(0xffL)
private val ARCHIVE_ID_DOMAIN = "iroha.musubi.archive-id.v1".toByteArray(StandardCharsets.UTF_8)
private val VERIFICATION_LOCK_DOMAIN =
    "iroha.musubi.verification-lock.v1".toByteArray(StandardCharsets.UTF_8)
private val RELEASE_DIGEST_DOMAIN =
    "iroha.musubi.release-digest.v1".toByteArray(StandardCharsets.UTF_8)
private val PARLIAMENT_ACTION_DOMAIN =
    "iroha.musubi.parliament-action.v1".toByteArray(StandardCharsets.UTF_8)
private val PROVIDER_BUNDLE_ATTESTATION_DIGEST_DOMAIN =
    "iroha.musubi.provider-bundle-attestation.digest.v1"
        .toByteArray(StandardCharsets.UTF_8)

private class MusubiPublicKeyOrderKeyV1(
    val algorithm: Int,
    val payload: ByteArray,
)

private class MusubiMultisigMemberOrderKeyV1(
    val publicKey: MusubiPublicKeyOrderKeyV1,
    val weight: Int,
    val canonicalSortKey: ByteArray,
)

private class MusubiAccountOrderKeyV1(
    val variant: Int,
    val publicKey: MusubiPublicKeyOrderKeyV1?,
    val version: Int,
    val threshold: Int,
    val members: List<MusubiMultisigMemberOrderKeyV1>,
)

private fun musubiAccountOrderKeyV1(canonicalOwner: String): MusubiAccountOrderKeyV1 {
    val address = AccountAddress.parseAny(canonicalOwner, null).address
    val single = address.singleKeyPayload()
    if (single != null) {
        return MusubiAccountOrderKeyV1(
            variant = 0,
            publicKey = MusubiPublicKeyOrderKeyV1(
                musubiAlgorithmOrderV1(single.curveId),
                single.publicKey,
            ),
            version = 0,
            threshold = 0,
            members = emptyList(),
        )
    }
    val policy = requireNotNull(address.multisigPolicyPayload()) {
        "Musubi package recovery owner has no canonical account controller"
    }
    val members = policy.members.map { member ->
        val publicKey = MusubiPublicKeyOrderKeyV1(
            musubiAlgorithmOrderV1(member.curveId),
            member.publicKey,
        )
        MusubiMultisigMemberOrderKeyV1(
            publicKey,
            member.weight,
            musubiMultisigMemberSortKeyV1(member.curveId, publicKey.payload),
        )
    }.sortedWith(Comparator { left, right ->
        MusubiValidationV1.compareUnsignedBytes(
            left.canonicalSortKey,
            right.canonicalSortKey,
        )
    })
    for (index in 1 until members.size) {
        require(!members[index - 1].canonicalSortKey.contentEquals(members[index].canonicalSortKey)) {
            "Musubi package recovery owner contains duplicate multisig members"
        }
    }
    return MusubiAccountOrderKeyV1(
        variant = 1,
        publicKey = null,
        version = policy.version,
        threshold = policy.threshold,
        members = members,
    )
}

private fun musubiAlgorithmOrderV1(curveId: Int): Int = when (curveId and 0xff) {
    0x01 -> 0
    0x04 -> 1
    0x03 -> 2
    0x05 -> 3
    0x02 -> 4
    0x0a -> 5
    0x0b -> 6
    0x0c -> 7
    0x0d -> 8
    0x0e -> 9
    0x0f -> 10
    else -> throw IllegalArgumentException(
        "Musubi package recovery owner uses an unsupported signing algorithm",
    )
}

private fun musubiAlgorithmStaticNameV1(curveId: Int): String = when (curveId and 0xff) {
    0x01 -> "ed25519"
    0x04 -> "secp256k1"
    0x03 -> "bls_normal"
    0x05 -> "bls_small"
    0x02 -> "ml-dsa"
    0x0a -> "gost3410-2012-256-paramset-a"
    0x0b -> "gost3410-2012-256-paramset-b"
    0x0c -> "gost3410-2012-256-paramset-c"
    0x0d -> "gost3410-2012-512-paramset-a"
    0x0e -> "gost3410-2012-512-paramset-b"
    0x0f -> "sm2"
    else -> throw IllegalArgumentException(
        "Musubi package recovery owner uses an unsupported signing algorithm",
    )
}

private fun musubiMultisigMemberSortKeyV1(curveId: Int, publicKey: ByteArray): ByteArray {
    val algorithm = musubiAlgorithmStaticNameV1(curveId).toByteArray(StandardCharsets.UTF_8)
    return ByteArray(algorithm.size + 1 + publicKey.size).also { key ->
        System.arraycopy(algorithm, 0, key, 0, algorithm.size)
        key[algorithm.size] = 0
        System.arraycopy(publicKey, 0, key, algorithm.size + 1, publicKey.size)
    }
}

private fun compareMusubiAccountOrderKeysV1(
    left: MusubiAccountOrderKeyV1,
    right: MusubiAccountOrderKeyV1,
): Int {
    left.variant.compareTo(right.variant).let { if (it != 0) return it }
    if (left.variant == 0) {
        return compareMusubiPublicKeyOrderKeysV1(
            requireNotNull(left.publicKey),
            requireNotNull(right.publicKey),
        )
    }
    left.version.compareTo(right.version).let { if (it != 0) return it }
    left.threshold.compareTo(right.threshold).let { if (it != 0) return it }
    for (index in 0 until minOf(left.members.size, right.members.size)) {
        compareMusubiPublicKeyOrderKeysV1(
            left.members[index].publicKey,
            right.members[index].publicKey,
        ).let { if (it != 0) return it }
        left.members[index].weight.compareTo(right.members[index].weight).let {
            if (it != 0) return it
        }
    }
    return left.members.size.compareTo(right.members.size)
}

private fun compareMusubiPublicKeyOrderKeysV1(
    left: MusubiPublicKeyOrderKeyV1,
    right: MusubiPublicKeyOrderKeyV1,
): Int {
    left.algorithm.compareTo(right.algorithm).let { if (it != 0) return it }
    return MusubiValidationV1.compareUnsignedBytes(left.payload, right.payload)
}
