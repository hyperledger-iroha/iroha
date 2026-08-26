package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasName;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasPricingPolicy;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveCommitment;
import org.hyperledger.iroha.android.client.MusubiModelsV1.DependencyKind;
import org.hyperledger.iroha.android.client.MusubiModelsV1.DependencyRequirement;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Digest32;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactDependencyEdge;
import org.hyperledger.iroha.android.client.MusubiModelsV1.GovernanceDecision;
import org.hyperledger.iroha.android.client.MusubiModelsV1.MaintainerPermissions;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceDelegation;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceDelegationApproval;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceDelegationPayload;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageRole;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageScope;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PrereleaseIdentifier;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationSetDigest;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationApproval;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationAttestation;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationPayload;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderCompletionAuthority;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderCompletionSignerPolicy;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderFinalizedAnchor;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Publication;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Reason;
import org.hyperledger.iroha.android.client.MusubiModelsV1.RegistryAdmissionMode;
import org.hyperledger.iroha.android.client.MusubiModelsV1.RegistryPolicy;
import org.hyperledger.iroha.android.client.MusubiModelsV1.RegistrySnapshot;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseManifest;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseMetadata;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolutionProof;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceipt;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceiptApproval;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceiptBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceiptPayload;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Version;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionComparator;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionReq;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VerificationLock;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VerificationNode;
import org.hyperledger.iroha.android.crypto.Blake3;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Typed Java builders for the first fixture-backed Musubi V1 mutation instructions. */
public final class MusubiInstructionsV1 {
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();

  private MusubiInstructionsV1() {}

  /** Shared typed mutation surface that emits the existing dynamic instruction container. */
  public abstract static class TypedInstructionV1 {
    private final String wireId;
    private final String concreteSchemaName;

    private TypedInstructionV1(final String wireId, final String concreteSchemaName) {
      this.wireId = wireId;
      this.concreteSchemaName = concreteSchemaName;
    }

    /** Stable registry wire identifier carried by {@link InstructionBox}. */
    public final String wireId() {
      return wireId;
    }

    /** Concrete Rust type name whose Norito schema hash frames the payload. */
    public final String concreteSchemaName() {
      return concreteSchemaName;
    }

    /** Canonical headerless Norito payload encoded with {@code COMPACT_LEN}. */
    public final byte[] barePayload() {
      final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
      encodeBare(encoder);
      return encoder.toByteArray();
    }

    /** Canonical concrete-type Norito frame embedded inside the dynamic instruction pair. */
    public final byte[] concreteFrame() {
      return NoritoCodec.encode(
          barePayload(), concreteSchemaName, BarePayloadAdapter.INSTANCE, NoritoHeader.COMPACT_LEN);
    }

    /** Return the existing dynamic instruction representation with a CUSTOM payload kind. */
    public final InstructionBox toInstructionBox() {
      return InstructionBox.fromWirePayload(wireId, concreteFrame());
    }

    abstract void encodeBare(NoritoEncoder encoder);
  }

  /** Register one immutable namespace-to-home-dataspace binding. */
  public static final class RegisterMusubiNamespaceBindingV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.namespace_binding.register";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RegisterMusubiNamespaceBindingV1";

    private final NamespaceBinding binding;
    private final BigInteger expectedPolicyRevision;

    public RegisterMusubiNamespaceBindingV1(
        final NamespaceBinding binding, final BigInteger expectedPolicyRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.binding = Objects.requireNonNull(binding, "binding");
      MusubiModelsV1.requireU64(expectedPolicyRevision, "expectedPolicyRevision");
      this.expectedPolicyRevision = expectedPolicyRevision;
    }

    public NamespaceBinding binding() {
      return binding;
    }

    public BigInteger expectedPolicyRevision() {
      return expectedPolicyRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeNamespaceBinding(child, binding));
      writeSized(encoder, child -> encodeU64(child, expectedPolicyRevision));
    }
  }

  /** Register one immutable source archive admitted through authenticated seed ingress. */
  public static final class RegisterMusubiArchiveV1 extends TypedInstructionV1 {
    public static final String WIRE_ID = "iroha.musubi.v1.archive.register";
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RegisterMusubiArchiveV1";

    private final ArchiveCommitment commitment;
    private final SeedIngressReceipt stagingReceipt;
    private final BigInteger expectedPolicyRevision;

    public RegisterMusubiArchiveV1(
        final ArchiveCommitment commitment,
        final SeedIngressReceipt stagingReceipt,
        final BigInteger expectedPolicyRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.commitment = Objects.requireNonNull(commitment, "commitment");
      this.stagingReceipt = Objects.requireNonNull(stagingReceipt, "stagingReceipt");
      this.expectedPolicyRevision =
          requirePositiveRevision(expectedPolicyRevision, "expectedPolicyRevision");
      final SeedIngressReceiptBinding binding = stagingReceipt.payload().binding();
      final Digest32 computedArchiveId = Digest32.fromBytes(domainHash(
          "iroha.musubi.archive-id.v1", encoded(encoder -> encodeArchiveCommitment(encoder, commitment))));
      if (!computedArchiveId.equals(binding.archiveId())
          || !commitment.carDigest().equals(binding.carBodyDigest())
          || !commitment.carSize().equals(binding.carBodyLength())) {
        throw new IllegalArgumentException(
            "Musubi archive commitment does not match its seed-ingress receipt");
      }
    }

    public ArchiveCommitment commitment() { return commitment; }
    public SeedIngressReceipt stagingReceipt() { return stagingReceipt; }
    public BigInteger expectedPolicyRevision() { return expectedPolicyRevision; }

    @Override void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeArchiveCommitment(child, commitment));
      writeSized(encoder, child -> encodeSeedIngressReceipt(child, stagingReceipt));
      writeSized(encoder, child -> encodeU64(child, expectedPolicyRevision));
    }
  }

  /** Register one immutable provider proof for later location-set commitments. */
  public static final class RegisterMusubiProviderBundleAttestationV1
      extends TypedInstructionV1 {
    public static final String WIRE_ID =
        "iroha.musubi.v1.provider_bundle_attestation.register";
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RegisterMusubiProviderBundleAttestationV1";

    private final ProviderBundleVerificationAttestation attestation;
    private final BigInteger expectedLocationRevision;

    public RegisterMusubiProviderBundleAttestationV1(
        final ProviderBundleVerificationAttestation attestation,
        final BigInteger expectedLocationRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.attestation = Objects.requireNonNull(attestation, "attestation");
      this.expectedLocationRevision =
          requirePositiveRevision(expectedLocationRevision, "expectedLocationRevision");
    }

    public ProviderBundleVerificationAttestation attestation() { return attestation; }
    public BigInteger expectedLocationRevision() { return expectedLocationRevision; }

    @Override void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeProviderAttestation(child, attestation));
      writeSized(encoder, child -> encodeU64(child, expectedLocationRevision));
    }
  }

  /** Add or renew one archive location bound to registered provider attestations. */
  public static final class AddMusubiArchiveLocationV1 extends TypedInstructionV1 {
    public static final String WIRE_ID = "iroha.musubi.v1.archive_location.add";
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::AddMusubiArchiveLocationV1";

    private final Digest32 archiveId;
    private final Digest32 locationId;
    private final Digest32 pinManifest;
    private final Digest32 replicationOrder;
    private final ProviderBundleAttestationSetDigest providerAttestationSetDigest;
    private final BigInteger renewAfterEpoch;
    private final BigInteger expiresAtEpoch;
    private final BigInteger expectedLocationRevision;

    public AddMusubiArchiveLocationV1(
        final Digest32 archiveId,
        final Digest32 locationId,
        final Digest32 pinManifest,
        final Digest32 replicationOrder,
        final ProviderBundleAttestationSetDigest providerAttestationSetDigest,
        final BigInteger renewAfterEpoch,
        final BigInteger expiresAtEpoch,
        final BigInteger expectedLocationRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.archiveId = requireNonZeroDigest(archiveId, "archiveId");
      this.locationId = requireNonZeroDigest(locationId, "locationId");
      this.pinManifest = requireNonZeroDigest(pinManifest, "pinManifest");
      this.replicationOrder = requireNonZeroDigest(replicationOrder, "replicationOrder");
      this.providerAttestationSetDigest =
          Objects.requireNonNull(providerAttestationSetDigest, "providerAttestationSetDigest");
      MusubiModelsV1.requireU64(renewAfterEpoch, "renewAfterEpoch");
      MusubiModelsV1.requireU64(expiresAtEpoch, "expiresAtEpoch");
      requirePositiveRevision(expectedLocationRevision, "expectedLocationRevision");
      if (renewAfterEpoch.compareTo(expiresAtEpoch) >= 0) {
        throw new IllegalArgumentException("Musubi archive renewal must precede expiry");
      }
      this.renewAfterEpoch = renewAfterEpoch;
      this.expiresAtEpoch = expiresAtEpoch;
      this.expectedLocationRevision = expectedLocationRevision;
    }

    public Digest32 archiveId() { return archiveId; }
    public Digest32 locationId() { return locationId; }
    public Digest32 pinManifest() { return pinManifest; }
    public Digest32 replicationOrder() { return replicationOrder; }
    public ProviderBundleAttestationSetDigest providerAttestationSetDigest() {
      return providerAttestationSetDigest;
    }
    public BigInteger renewAfterEpoch() { return renewAfterEpoch; }
    public BigInteger expiresAtEpoch() { return expiresAtEpoch; }
    public BigInteger expectedLocationRevision() { return expectedLocationRevision; }

    @Override void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeDigest32(child, archiveId));
      writeSized(encoder, child -> encodeDigest32(child, locationId));
      writeSized(encoder, child -> encodeDigest32(child, pinManifest));
      writeSized(encoder, child -> encodeDigest32(child, replicationOrder));
      writeSized(encoder, child -> encodeDigest32(child, providerAttestationSetDigest));
      writeSized(encoder, child -> encodeU64(child, renewAfterEpoch));
      writeSized(encoder, child -> encodeU64(child, expiresAtEpoch));
      writeSized(encoder, child -> encodeU64(child, expectedLocationRevision));
    }
  }

  /** Claim an absent package if authorized and publish one immutable release. */
  public static final class PublishMusubiReleaseV1 extends TypedInstructionV1 {
    public static final String WIRE_ID = "iroha.musubi.v1.release.publish";
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::PublishMusubiReleaseV1";

    private final MusubiModelsV1.Namespace namespace;
    private final Publication publication;
    private final NamespaceDelegation namespaceDelegation;
    private final BigInteger expectedPolicyRevision;
    private final BigInteger expectedGovernanceRevision;

    public PublishMusubiReleaseV1(
        final MusubiModelsV1.Namespace namespace,
        final Publication publication,
        final NamespaceDelegation namespaceDelegation,
        final BigInteger expectedPolicyRevision,
        final BigInteger expectedGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.namespace = Objects.requireNonNull(namespace, "namespace");
      this.publication = Objects.requireNonNull(publication, "publication");
      this.namespaceDelegation = namespaceDelegation;
      this.expectedPolicyRevision =
          requirePositiveRevision(expectedPolicyRevision, "expectedPolicyRevision");
      if (expectedGovernanceRevision != null) {
        requirePositiveRevision(expectedGovernanceRevision, "expectedGovernanceRevision");
      }
      this.expectedGovernanceRevision = expectedGovernanceRevision;
      if (!namespaceMatchesPackage(namespace.value(), publication.manifest().release().packageId())) {
        throw new IllegalArgumentException("Musubi publication namespace and package scope disagree");
      }
      final byte[] encodedLock = encoded(
          encoder -> encodeVerificationLock(encoder, publication.resolution().lock()));
      final Digest32 computedLockDigest = Digest32.fromBytes(
          domainHash("iroha.musubi.verification-lock.v1", encodedLock));
      if (!computedLockDigest.equals(publication.manifest().verificationLockDigest())) {
        throw new IllegalArgumentException(
            "Musubi publication verification-lock digest does not match its manifest");
      }
    }

    public MusubiModelsV1.Namespace namespace() { return namespace; }
    public Publication publication() { return publication; }
    public NamespaceDelegation namespaceDelegation() { return namespaceDelegation; }
    public BigInteger expectedPolicyRevision() { return expectedPolicyRevision; }
    public BigInteger expectedGovernanceRevision() { return expectedGovernanceRevision; }

    @Override void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeStringNewtype(child, namespace.value()));
      writeSized(encoder, child -> encodePublication(child, publication));
      writeSized(encoder, child -> encodeOption(
          child, namespaceDelegation, MusubiInstructionsV1::encodeNamespaceDelegation));
      writeSized(encoder, child -> encodeU64(child, expectedPolicyRevision));
      writeSized(encoder, child -> encodeOption(
          child, expectedGovernanceRevision, MusubiInstructionsV1::encodeU64));
    }
  }

  /** Replace the complete mutable package metadata projection. */
  public static final class SetMusubiPackageMetadataV1 extends TypedInstructionV1 {
    public static final String WIRE_ID = "iroha.musubi.v1.package_metadata.set";
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::SetMusubiPackageMetadataV1";

    private final PackageId packageId;
    private final ReleaseMetadata metadata;
    private final BigInteger expectedMetadataRevision;

    public SetMusubiPackageMetadataV1(
        final PackageId packageId,
        final ReleaseMetadata metadata,
        final BigInteger expectedMetadataRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.metadata = Objects.requireNonNull(metadata, "metadata");
      this.expectedMetadataRevision =
          requirePositiveRevision(expectedMetadataRevision, "expectedMetadataRevision");
    }

    public PackageId packageId() { return packageId; }
    public ReleaseMetadata metadata() { return metadata; }
    public BigInteger expectedMetadataRevision() { return expectedMetadataRevision; }

    @Override void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodePackageId(child, packageId));
      writeSized(encoder, child -> encodeReleaseMetadata(child, metadata));
      writeSized(encoder, child -> encodeU64(child, expectedMetadataRevision));
    }
  }

  /** Execute an enacted Parliament replacement of the complete registry policy. */
  public static final class SetMusubiRegistryPolicyV1 extends TypedInstructionV1 {
    public static final String WIRE_ID = "iroha.musubi.v1.parliament.registry_policy.set";
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::SetMusubiRegistryPolicyV1";

    private final GovernanceDecision decision;
    private final RegistryPolicy policy;
    private final BigInteger expectedPolicyRevision;

    public SetMusubiRegistryPolicyV1(
        final GovernanceDecision decision,
        final RegistryPolicy policy,
        final BigInteger expectedPolicyRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.decision = Objects.requireNonNull(decision, "decision");
      this.policy = Objects.requireNonNull(policy, "policy");
      this.expectedPolicyRevision =
          requirePositiveRevision(expectedPolicyRevision, "expectedPolicyRevision");
      if (!policy.revision().equals(expectedPolicyRevision.add(BigInteger.ONE))) {
        throw new IllegalArgumentException(
            "Musubi replacement policy must use the exact successor revision");
      }
      final byte[] action = encoded(encoder -> {
        encoder.writeUInt(3L, 32);
        writeSized(encoder, replacement -> {
          writeSized(replacement, child -> encodeRegistryPolicy(child, policy));
          writeSized(replacement, child -> encodeU64(child, expectedPolicyRevision));
        });
      });
      final byte[] expectedActionDigest =
          domainHash("iroha.musubi.parliament-action.v1", action);
      if (!Arrays.equals(expectedActionDigest, decision.actionDigest().bytes())) {
        throw new IllegalArgumentException(
            "Musubi governance decision does not authorize this registry policy action");
      }
    }

    public GovernanceDecision decision() { return decision; }
    public RegistryPolicy policy() { return policy; }
    public BigInteger expectedPolicyRevision() { return expectedPolicyRevision; }

    @Override void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeGovernanceDecision(child, decision));
      writeSized(encoder, child -> encodeRegistryPolicy(child, policy));
      writeSized(encoder, child -> encodeU64(child, expectedPolicyRevision));
    }
  }

  /** Invite one canonical account to an owner or explicitly permissioned maintainer role. */
  public static final class InviteMusubiPackageMaintainerV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.package_member.invite";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::InviteMusubiPackageMaintainerV1";

    private final PackageId packageId;
    private final Digest32 inviteId;
    private final String invitedAccount;
    private final byte[] invitedAccountPayload;
    private final PackageRole role;
    private final BigInteger expiresAtHeight;
    private final BigInteger expectedGovernanceRevision;

    public InviteMusubiPackageMaintainerV1(
        final PackageId packageId,
        final Digest32 inviteId,
        final String invitedAccount,
        final PackageRole role,
        final BigInteger expiresAtHeight,
        final BigInteger expectedGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.inviteId = requireNonZeroDigest(inviteId, "inviteId");
      this.invitedAccount =
          AccountIdLiteral.requireCanonicalI105Address(invitedAccount, "invitedAccount");
      this.invitedAccountPayload =
          TransferWirePayloadEncoder.encodeAccountIdPayload(this.invitedAccount);
      this.role = Objects.requireNonNull(role, "role");
      MusubiModelsV1.requireU64(expiresAtHeight, "expiresAtHeight");
      if (expiresAtHeight.signum() == 0) {
        throw new IllegalArgumentException("Musubi invitation expiry height must be positive");
      }
      this.expiresAtHeight = expiresAtHeight;
      MusubiModelsV1.requireU64(expectedGovernanceRevision, "expectedGovernanceRevision");
      this.expectedGovernanceRevision = expectedGovernanceRevision;
    }

    public PackageId packageId() {
      return packageId;
    }

    public Digest32 inviteId() {
      return inviteId;
    }

    public String invitedAccount() {
      return invitedAccount;
    }

    public PackageRole role() {
      return role;
    }

    public BigInteger expiresAtHeight() {
      return expiresAtHeight;
    }

    public BigInteger expectedGovernanceRevision() {
      return expectedGovernanceRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodePackageId(child, packageId));
      writeSized(encoder, child -> encodeDigest32(child, inviteId));
      writeSized(encoder, child -> child.writeBytes(invitedAccountPayload));
      writeSized(encoder, child -> encodePackageRole(child, role));
      writeSized(encoder, child -> encodeU64(child, expiresAtHeight));
      writeSized(encoder, child -> encodeU64(child, expectedGovernanceRevision));
    }
  }

  /** Replace one accepted package member's role under compare-and-set governance. */
  public static final class SetMusubiPackageMaintainerRoleV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.package_member.set_role";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::SetMusubiPackageMaintainerRoleV1";

    private final PackageId packageId;
    private final String account;
    private final byte[] accountPayload;
    private final PackageRole role;
    private final BigInteger expectedGovernanceRevision;

    public SetMusubiPackageMaintainerRoleV1(
        final PackageId packageId,
        final String account,
        final PackageRole role,
        final BigInteger expectedGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.account = AccountIdLiteral.requireCanonicalI105Address(account, "account");
      this.accountPayload = TransferWirePayloadEncoder.encodeAccountIdPayload(this.account);
      this.role = Objects.requireNonNull(role, "role");
      MusubiModelsV1.requireU64(expectedGovernanceRevision, "expectedGovernanceRevision");
      this.expectedGovernanceRevision = expectedGovernanceRevision;
    }

    public PackageId packageId() {
      return packageId;
    }

    public String account() {
      return account;
    }

    public PackageRole role() {
      return role;
    }

    public BigInteger expectedGovernanceRevision() {
      return expectedGovernanceRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodePackageId(child, packageId));
      writeSized(encoder, child -> child.writeBytes(accountPayload));
      writeSized(encoder, child -> encodePackageRole(child, role));
      writeSized(encoder, child -> encodeU64(child, expectedGovernanceRevision));
    }
  }

  /** Accept a pending package role invitation as the invited account. */
  public static final class AcceptMusubiPackageMaintainerV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.package_member.accept";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::AcceptMusubiPackageMaintainerV1";

    private final PackageId packageId;
    private final Digest32 inviteId;
    private final BigInteger expectedGovernanceRevision;

    public AcceptMusubiPackageMaintainerV1(
        final PackageId packageId,
        final Digest32 inviteId,
        final BigInteger expectedGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.inviteId = Objects.requireNonNull(inviteId, "inviteId");
      MusubiModelsV1.requireU64(expectedGovernanceRevision, "expectedGovernanceRevision");
      this.expectedGovernanceRevision = expectedGovernanceRevision;
    }

    public PackageId packageId() {
      return packageId;
    }

    public Digest32 inviteId() {
      return inviteId;
    }

    public BigInteger expectedGovernanceRevision() {
      return expectedGovernanceRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodePackageId(child, packageId));
      writeSized(encoder, child -> encodeDigest32(child, inviteId));
      writeSized(encoder, child -> encodeU64(child, expectedGovernanceRevision));
    }
  }

  /** Revoke an unaccepted package role invitation as a current owner. */
  public static final class RevokeMusubiPackageMaintainerInvitationV1
      extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID =
        "iroha.musubi.v1.package_member.invitation.revoke";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RevokeMusubiPackageMaintainerInvitationV1";

    private final PackageId packageId;
    private final Digest32 inviteId;
    private final BigInteger expectedGovernanceRevision;

    public RevokeMusubiPackageMaintainerInvitationV1(
        final PackageId packageId,
        final Digest32 inviteId,
        final BigInteger expectedGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.inviteId = Objects.requireNonNull(inviteId, "inviteId");
      MusubiModelsV1.requireU64(expectedGovernanceRevision, "expectedGovernanceRevision");
      this.expectedGovernanceRevision = expectedGovernanceRevision;
    }

    public PackageId packageId() {
      return packageId;
    }

    public Digest32 inviteId() {
      return inviteId;
    }

    public BigInteger expectedGovernanceRevision() {
      return expectedGovernanceRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodePackageId(child, packageId));
      writeSized(encoder, child -> encodeDigest32(child, inviteId));
      writeSized(encoder, child -> encodeU64(child, expectedGovernanceRevision));
    }
  }

  /** Register a paid permanent global alias for an exact structural package. */
  public static final class RegisterMusubiAliasV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.alias.register";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RegisterMusubiAliasV1";

    private final AliasName alias;
    private final PackageId target;
    private final BigInteger expectedPricingRevision;

    public RegisterMusubiAliasV1(
        final AliasName alias,
        final PackageId target,
        final BigInteger expectedPricingRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.alias = Objects.requireNonNull(alias, "alias");
      this.target = Objects.requireNonNull(target, "target");
      MusubiModelsV1.requireU64(expectedPricingRevision, "expectedPricingRevision");
      this.expectedPricingRevision = expectedPricingRevision;
    }

    public AliasName alias() {
      return alias;
    }

    public PackageId target() {
      return target;
    }

    public BigInteger expectedPricingRevision() {
      return expectedPricingRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeStringNewtype(child, alias.value()));
      writeSized(encoder, child -> encodePackageId(child, target));
      writeSized(encoder, child -> encodeU64(child, expectedPricingRevision));
    }
  }

  /** Assert the immutable digest of an exact Musubi release. */
  public static final class AssertMusubiReleaseDigestV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.release_digest.assert";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::AssertMusubiReleaseDigestV1";

    private final ReleaseId release;
    private final Digest32 expectedDigest;

    public AssertMusubiReleaseDigestV1(
        final ReleaseId release, final Digest32 expectedDigest) {
      super(WIRE_ID, SCHEMA_NAME);
      this.release = Objects.requireNonNull(release, "release");
      this.expectedDigest = Objects.requireNonNull(expectedDigest, "expectedDigest");
    }

    public ReleaseId release() {
      return release;
    }

    public Digest32 expectedDigest() {
      return expectedDigest;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeReleaseId(child, release));
      writeSized(encoder, child -> encodeDigest32(child, expectedDigest));
    }
  }

  /** Retire one exact archive location while preserving the immutable archive identity. */
  public static final class RetireMusubiArchiveLocationV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.archive_location.retire";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RetireMusubiArchiveLocationV1";

    private final Digest32 archiveId;
    private final Digest32 locationId;
    private final BigInteger expectedLocationRevision;
    private final Reason reason;

    public RetireMusubiArchiveLocationV1(
        final Digest32 archiveId,
        final Digest32 locationId,
        final BigInteger expectedLocationRevision,
        final Reason reason) {
      super(WIRE_ID, SCHEMA_NAME);
      this.archiveId = Objects.requireNonNull(archiveId, "archiveId");
      this.locationId = Objects.requireNonNull(locationId, "locationId");
      MusubiModelsV1.requireU64(expectedLocationRevision, "expectedLocationRevision");
      this.expectedLocationRevision = expectedLocationRevision;
      this.reason = Objects.requireNonNull(reason, "reason");
    }

    public Digest32 archiveId() {
      return archiveId;
    }

    public Digest32 locationId() {
      return locationId;
    }

    public BigInteger expectedLocationRevision() {
      return expectedLocationRevision;
    }

    public Reason reason() {
      return reason;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeDigest32(child, archiveId));
      writeSized(encoder, child -> encodeDigest32(child, locationId));
      writeSized(encoder, child -> encodeU64(child, expectedLocationRevision));
      writeSized(encoder, child -> encodeStringNewtype(child, reason.value()));
    }
  }

  /** Set the reversible yank state for one immutable Musubi release. */
  public static final class SetMusubiReleaseYankV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.release_yank.set";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::SetMusubiReleaseYankV1";

    private final ReleaseId release;
    private final boolean yanked;
    private final Reason reason;
    private final BigInteger expectedYankRevision;

    public SetMusubiReleaseYankV1(
        final ReleaseId release,
        final boolean yanked,
        final Reason reason,
        final BigInteger expectedYankRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.release = Objects.requireNonNull(release, "release");
      this.yanked = yanked;
      this.reason = Objects.requireNonNull(reason, "reason");
      MusubiModelsV1.requireU64(expectedYankRevision, "expectedYankRevision");
      this.expectedYankRevision = expectedYankRevision;
    }

    public ReleaseId release() {
      return release;
    }

    public boolean yanked() {
      return yanked;
    }

    public Reason reason() {
      return reason;
    }

    public BigInteger expectedYankRevision() {
      return expectedYankRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeReleaseId(child, release));
      writeSized(encoder, child -> child.writeByte(yanked ? 1 : 0));
      writeSized(encoder, child -> encodeStringNewtype(child, reason.value()));
      writeSized(encoder, child -> encodeU64(child, expectedYankRevision));
    }
  }

  /** Remove one accepted package member subject to the on-chain last-owner invariant. */
  public static final class RemoveMusubiPackageMaintainerV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID = "iroha.musubi.v1.package_member.remove";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RemoveMusubiPackageMaintainerV1";

    private final PackageId packageId;
    private final String account;
    private final byte[] accountPayload;
    private final BigInteger expectedGovernanceRevision;

    public RemoveMusubiPackageMaintainerV1(
        final PackageId packageId,
        final String account,
        final BigInteger expectedGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.account = AccountIdLiteral.requireCanonicalI105Address(account, "account");
      this.accountPayload = TransferWirePayloadEncoder.encodeAccountIdPayload(this.account);
      MusubiModelsV1.requireU64(expectedGovernanceRevision, "expectedGovernanceRevision");
      this.expectedGovernanceRevision = expectedGovernanceRevision;
    }

    public PackageId packageId() {
      return packageId;
    }

    public String account() {
      return account;
    }

    public BigInteger expectedGovernanceRevision() {
      return expectedGovernanceRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodePackageId(child, packageId));
      writeSized(encoder, child -> child.writeBytes(accountPayload));
      writeSized(encoder, child -> encodeU64(child, expectedGovernanceRevision));
    }
  }

  /** Apply an enacted Parliament replacement of one package's complete owner set. */
  public static final class RecoverMusubiPackageV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID =
        "iroha.musubi.v1.parliament.package_recover";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RecoverMusubiPackageV1";

    private final GovernanceDecision decision;
    private final PackageId packageId;
    private final List<String> owners;
    private final List<byte[]> ownerPayloads;
    private final BigInteger expectedGovernanceRevision;

    public RecoverMusubiPackageV1(
        final GovernanceDecision decision,
        final PackageId packageId,
        final List<String> owners,
        final BigInteger expectedGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.decision = Objects.requireNonNull(decision, "decision");
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      Objects.requireNonNull(owners, "owners");
      if (owners.isEmpty() || owners.size() > 64) {
        throw new IllegalArgumentException("Musubi recovery must provide 1 to 64 owners");
      }
      final List<String> canonicalOwners = new ArrayList<>(owners.size());
      final List<byte[]> canonicalPayloads = new ArrayList<>(owners.size());
      for (final String owner : owners) {
        final String canonical = AccountIdLiteral.requireCanonicalI105Address(owner, "owner");
        if (!canonicalOwners.isEmpty()
            && compareAccountIds(canonicalOwners.get(canonicalOwners.size() - 1), canonical) >= 0) {
          throw new IllegalArgumentException(
              "Musubi recovery owners must be strictly sorted and distinct by AccountId");
        }
        canonicalOwners.add(canonical);
        canonicalPayloads.add(TransferWirePayloadEncoder.encodeAccountIdPayload(canonical));
      }
      this.owners = Collections.unmodifiableList(canonicalOwners);
      this.ownerPayloads = Collections.unmodifiableList(canonicalPayloads);
      this.expectedGovernanceRevision =
          requirePositiveRevision(expectedGovernanceRevision, "expectedGovernanceRevision");
    }

    public GovernanceDecision decision() { return decision; }
    public PackageId packageId() { return packageId; }
    public List<String> owners() { return owners; }
    public BigInteger expectedGovernanceRevision() { return expectedGovernanceRevision; }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeGovernanceDecision(child, decision));
      writeSized(encoder, child -> encodePackageId(child, packageId));
      writeSized(encoder, child -> encodeAccountIds(child, ownerPayloads));
      writeSized(encoder, child -> encodeU64(child, expectedGovernanceRevision));
    }
  }

  /** Apply an enacted Parliament retarget of one permanent global alias. */
  public static final class RetargetMusubiAliasV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID =
        "iroha.musubi.v1.parliament.alias_retarget";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::RetargetMusubiAliasV1";

    private final GovernanceDecision decision;
    private final AliasName alias;
    private final PackageId target;
    private final BigInteger expectedHistoryRevision;

    public RetargetMusubiAliasV1(
        final GovernanceDecision decision,
        final AliasName alias,
        final PackageId target,
        final BigInteger expectedHistoryRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.decision = Objects.requireNonNull(decision, "decision");
      this.alias = Objects.requireNonNull(alias, "alias");
      this.target = Objects.requireNonNull(target, "target");
      this.expectedHistoryRevision =
          requirePositiveRevision(expectedHistoryRevision, "expectedHistoryRevision");
    }

    public GovernanceDecision decision() { return decision; }
    public AliasName alias() { return alias; }
    public PackageId target() { return target; }
    public BigInteger expectedHistoryRevision() { return expectedHistoryRevision; }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeGovernanceDecision(child, decision));
      writeSized(encoder, child -> encodeStringNewtype(child, alias.value()));
      writeSized(encoder, child -> encodePackageId(child, target));
      writeSized(encoder, child -> encodeU64(child, expectedHistoryRevision));
    }
  }

  /** Apply an enacted Parliament takedown of one immutable release artifact. */
  public static final class SetMusubiArtifactTakedownV1 extends TypedInstructionV1 {
    /** Stable dynamic instruction registry identifier. */
    public static final String WIRE_ID =
        "iroha.musubi.v1.parliament.artifact_takedown";

    /** Exact Rust concrete type name used for the Norito schema hash. */
    public static final String SCHEMA_NAME =
        "iroha_data_model::isi::musubi::SetMusubiArtifactTakedownV1";

    private final GovernanceDecision decision;
    private final ReleaseId release;
    private final Reason reason;
    private final BigInteger expectedArtifactGovernanceRevision;

    public SetMusubiArtifactTakedownV1(
        final GovernanceDecision decision,
        final ReleaseId release,
        final Reason reason,
        final BigInteger expectedArtifactGovernanceRevision) {
      super(WIRE_ID, SCHEMA_NAME);
      this.decision = Objects.requireNonNull(decision, "decision");
      this.release = Objects.requireNonNull(release, "release");
      this.reason = Objects.requireNonNull(reason, "reason");
      this.expectedArtifactGovernanceRevision =
          requirePositiveRevision(
              expectedArtifactGovernanceRevision,
              "expectedArtifactGovernanceRevision");
    }

    public GovernanceDecision decision() { return decision; }
    public ReleaseId release() { return release; }
    public Reason reason() { return reason; }
    public BigInteger expectedArtifactGovernanceRevision() {
      return expectedArtifactGovernanceRevision;
    }

    @Override
    void encodeBare(final NoritoEncoder encoder) {
      writeSized(encoder, child -> encodeGovernanceDecision(child, decision));
      writeSized(encoder, child -> encodeReleaseId(child, release));
      writeSized(encoder, child -> encodeStringNewtype(child, reason.value()));
      writeSized(encoder, child -> encodeU64(child, expectedArtifactGovernanceRevision));
    }
  }

  private interface EncoderAction {
    void encode(NoritoEncoder encoder);
  }

  private interface ValueEncoder<T> {
    void encode(NoritoEncoder encoder, T value);
  }

  private static byte[] encoded(final EncoderAction action) {
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
    action.encode(encoder);
    return encoder.toByteArray();
  }

  private static void writeSized(final NoritoEncoder encoder, final EncoderAction action) {
    final NoritoEncoder child = encoder.childEncoder();
    action.encode(child);
    final byte[] payload = child.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private static <T> void encodeOption(
      final NoritoEncoder encoder, final T value, final ValueEncoder<T> valueEncoder) {
    if (value == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    writeSized(encoder, child -> valueEncoder.encode(child, value));
  }

  private static void encodeGovernanceDecision(
      final NoritoEncoder encoder, final GovernanceDecision value) {
    writeSized(encoder, child -> child.writeBytes(value.decisionId().bytes()));
    writeSized(encoder, child -> encodeDigest32(child, value.actionDigest()));
    writeSized(encoder, child -> encodeU64(child, value.enactedAtHeight()));
    writeSized(encoder, child -> encodeU64(child, value.executeAfterHeight()));
  }

  private static void encodeArchiveCommitment(
      final NoritoEncoder encoder, final ArchiveCommitment value) {
    writeSized(encoder, child -> encodeGenericFixedBytes(child, value.rootCid()));
    writeSized(encoder, child -> encodeChunkerProfile(child, value.chunker()));
    writeSized(encoder, child -> encodeDigest32(child, value.chunkPlanDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.porRoot()));
    writeSized(encoder, child -> encodeU64(child, value.contentLength()));
    writeSized(encoder, child -> encodeDigest32(child, value.carDigest()));
    writeSized(encoder, child -> encodeU64(child, value.carSize()));
    writeSized(encoder, child -> encodeDigest32(child, value.bundleDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.sourceTreeDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.descriptorDigest()));
    writeSized(encoder, child -> child.writeUInt(value.fileCount(), 32));
    writeSized(encoder, child -> child.writeUInt(value.chunkCount(), 32));
  }

  private static void encodeGenericFixedBytes(
      final NoritoEncoder encoder, final byte[] value) {
    for (final byte octet : value) {
      writeSized(encoder, child -> child.writeByte(octet & 0xff));
    }
  }

  private static void encodeChunkerProfile(
      final NoritoEncoder encoder, final MusubiModelsV1.ChunkerProfileHandle value) {
    writeSized(encoder, child -> child.writeUInt(value.profileId(), 32));
    writeSized(encoder, child -> encodeString(child, value.namespace()));
    writeSized(encoder, child -> encodeString(child, value.name()));
    writeSized(encoder, child -> encodeString(child, value.semver()));
    writeSized(encoder, child -> encodeU64(child, value.multihashCode()));
  }

  private static void encodeSeedIngressReceipt(
      final NoritoEncoder encoder, final SeedIngressReceipt value) {
    writeSized(encoder, child -> encodeSeedIngressPayload(child, value.payload()));
    writeSized(encoder, child -> {
      child.writeUInt(value.approvals().size(), 64);
      for (final SeedIngressReceiptApproval approval : value.approvals()) {
        writeSized(child, item -> encodeSeedIngressApproval(item, approval));
      }
    });
  }

  private static void encodeSeedIngressPayload(
      final NoritoEncoder encoder, final SeedIngressReceiptPayload value) {
    writeSized(encoder, child -> child.writeByte(1));
    writeSized(encoder, child -> encodeSeedIngressBinding(child, value.binding()));
    writeSized(encoder, child -> encodeU64(child, value.issuedAtMs()));
    writeSized(encoder, child -> encodeU64(child, value.expiresAtMs()));
  }

  private static void encodeSeedIngressBinding(
      final NoritoEncoder encoder, final SeedIngressReceiptBinding value) {
    writeSized(encoder, child -> child.writeBytes(value.networkId().bytes()));
    writeSized(encoder, child -> encodeAccountId(child, value.publisher()));
    writeSized(encoder, child -> encodeAccountId(child, value.ingressBroker()));
    writeSized(encoder, child -> encodeHexDigestNewtype(child, value.seedProvider()));
    writeSized(encoder, child -> encodeDigest32(child, value.semanticReleaseManifestDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.archiveId()));
    writeSized(encoder, child -> encodeDigest32(child, value.carBodyDigest()));
    writeSized(encoder, child -> encodeU64(child, value.carBodyLength()));
    writeSized(encoder, child -> child.writeBytes(value.nonce()));
  }

  private static void encodeSeedIngressApproval(
      final NoritoEncoder encoder, final SeedIngressReceiptApproval value) {
    writeSized(encoder, child -> encodePublicKey(child, value.publicKey()));
    writeSized(encoder, child -> encodeSignature(child, value.signature()));
  }

  private static void encodeAccountId(final NoritoEncoder encoder, final String value) {
    encoder.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value));
  }

  private static void encodePublicKey(final NoritoEncoder encoder, final String value) {
    final PublicKeyCodec.PublicKeyPayload key = PublicKeyCodec.decodePublicKeyLiteral(value);
    if (key == null) throw new IllegalArgumentException("invalid Musubi public key");
    BYTE_VECTOR_ADAPTER.encode(
        encoder, PublicKeyCodec.compactPublicKeyPayload(key.curveId(), key.keyBytes()));
  }

  private static void encodeSignature(final NoritoEncoder encoder, final String value) {
    BYTE_VECTOR_ADAPTER.encode(encoder, hexBytes(value));
  }

  private static void encodeHexDigestNewtype(
      final NoritoEncoder encoder, final String value) {
    final byte[] bytes = hexBytes(value);
    encoder.writeLength(bytes.length, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    encoder.writeBytes(bytes);
  }

  private static void encodeAccountIds(
      final NoritoEncoder encoder, final List<byte[]> payloads) {
    encoder.writeUInt(payloads.size(), 64);
    for (final byte[] payload : payloads) {
      writeSized(encoder, child -> child.writeBytes(payload));
    }
  }

  private static void encodeProviderAttestation(
      final NoritoEncoder encoder, final ProviderBundleVerificationAttestation value) {
    writeSized(encoder, child -> encodeProviderPayload(child, value.payload()));
    writeSized(encoder, child -> {
      child.writeUInt(value.approvals().size(), 64);
      for (final ProviderBundleVerificationApproval approval : value.approvals()) {
        writeSized(child, item -> encodeProviderApproval(item, approval));
      }
    });
  }

  private static void encodeProviderPayload(
      final NoritoEncoder encoder, final ProviderBundleVerificationPayload value) {
    writeSized(encoder, child -> child.writeByte(1));
    writeSized(encoder, child -> encodeProviderBinding(child, value.binding()));
  }

  private static void encodeProviderBinding(
      final NoritoEncoder encoder, final ProviderBundleVerificationBinding value) {
    writeSized(encoder, child -> child.writeBytes(value.networkId().bytes()));
    writeSized(encoder, child -> encodeHexDigestNewtype(child, value.providerId()));
    writeSized(encoder, child -> encodeAccountId(child, value.completedBy()));
    writeSized(encoder, child -> encodeCompletionAuthority(child, value.completionAuthority()));
    writeSized(encoder, child -> encodeDigest32(child, value.replicationOrder()));
    writeSized(encoder, child -> encodeU64(child, value.assignmentRevision()));
    writeSized(encoder, child -> encodeU64(child, value.completionEpoch()));
    writeSized(encoder, child -> encodeFinalizedAnchor(child, value.finalizedAnchor()));
    writeSized(encoder, child -> encodeDigest32(child, value.archiveId()));
    writeSized(encoder, child -> encodeDigest32(child, value.bundleDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.descriptorDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.semanticReleaseManifestDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.verificationLockDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.sourceTreeDigest()));
  }

  private static void encodeCompletionAuthority(
      final NoritoEncoder encoder, final ProviderCompletionAuthority value) {
    writeSized(encoder, child -> encodeAccountId(child, value.providerOwner()));
    writeSized(encoder, child -> encodeCompletionSignerPolicy(child, value.signerPolicy()));
  }

  private static void encodeCompletionSignerPolicy(
      final NoritoEncoder encoder, final ProviderCompletionSignerPolicy value) {
    writeSized(encoder, child -> child.writeBytes(value.policyId()));
    writeSized(encoder, child -> encodeU64(child, value.revision()));
    writeSized(encoder, child -> encodeOption(
        child,
        value.predecessorDigest(),
        MusubiInstructionsV1::encodeGenericFixedBytes));
    writeSized(encoder, child -> child.writeBytes(value.policyDigest()));
  }

  private static void encodeFinalizedAnchor(
      final NoritoEncoder encoder, final ProviderFinalizedAnchor value) {
    writeSized(encoder, child -> encodeU64(child, value.height()));
    writeSized(encoder, child -> child.writeBytes(value.blockHash()));
  }

  private static void encodeProviderApproval(
      final NoritoEncoder encoder, final ProviderBundleVerificationApproval value) {
    writeSized(encoder, child -> encodePublicKey(child, value.publicKey()));
    writeSized(encoder, child -> encodeSignature(child, value.signature()));
  }

  private static void encodeNamespaceBinding(
      final NoritoEncoder encoder, final NamespaceBinding value) {
    writeSized(encoder, child -> encodeStringNewtype(child, value.namespace().value()));
    writeSized(encoder, child -> encodeU64Newtype(child, value.homeDataspace()));
    writeSized(encoder, child -> encodePackageScope(child, value.scope()));
    writeSized(encoder, child -> encodeU64(child, value.generation()));
  }

  private static void encodePublication(
      final NoritoEncoder encoder, final Publication value) {
    writeSized(encoder, child -> encodeReleaseManifest(child, value.manifest()));
    writeSized(encoder, child -> encodeResolutionProof(child, value.resolution()));
  }

  private static void encodeReleaseManifest(
      final NoritoEncoder encoder, final ReleaseManifest value) {
    writeSized(encoder, child -> encodeReleaseId(child, value.release()));
    writeSized(encoder, child -> child.writeUInt(0L, 32));
    writeSized(encoder, child -> encodeAbiBinding(child, value.abi()));
    writeSized(encoder, child -> encodeDependencyRequirements(child, value.dependencies()));
    writeSized(encoder, child -> encodeNames(child, value.exports()));
    writeSized(encoder, child -> encodeDigest32(child, value.interfaceDigest()));
    writeSized(encoder, child -> encodeReleaseMetadata(child, value.metadata()));
    writeSized(encoder, child -> encodeDigest32(child, value.archiveId()));
    writeSized(encoder, child -> encodeDigest32(child, value.verificationLockDigest()));
  }

  private static void encodeAbiBinding(
      final NoritoEncoder encoder, final MusubiModelsV1.AbiBinding value) {
    writeSized(encoder, child -> child.writeUInt(1L, 16));
    writeSized(encoder, child -> child.writeBytes(value.abiHash()));
  }

  private static void encodeDependencyRequirements(
      final NoritoEncoder encoder, final List<DependencyRequirement> values) {
    encoder.writeUInt(values.size(), 64);
    for (final DependencyRequirement value : values) {
      writeSized(encoder, child -> encodeDependencyRequirement(child, value));
    }
  }

  private static void encodeDependencyRequirement(
      final NoritoEncoder encoder, final DependencyRequirement value) {
    writeSized(encoder, child -> encodeString(child, value.alias()));
    writeSized(encoder, child -> encodePackageId(child, value.packageId()));
    writeSized(encoder, child -> encodeVersionRequirement(child, value.requirement()));
  }

  private static void encodeNames(final NoritoEncoder encoder, final List<String> values) {
    encoder.writeUInt(values.size(), 64);
    for (final String value : values) {
      writeSized(encoder, child -> encodeString(child, value));
    }
  }

  private static void encodeReleaseMetadata(
      final NoritoEncoder encoder, final ReleaseMetadata value) {
    writeSized(encoder, child -> encodeOption(
        child, value.description(), MusubiInstructionsV1::encodeStringNewtype));
    writeSized(encoder, child -> encodeOption(
        child, value.readme(), MusubiInstructionsV1::encodeStringNewtype));
    writeSized(encoder, child -> encodeOption(
        child, value.license(), MusubiInstructionsV1::encodeStringNewtype));
    writeSized(encoder, child -> encodeOption(
        child, value.repository(), MusubiInstructionsV1::encodeStringNewtype));
    writeSized(encoder, child -> {
      child.writeUInt(value.keywords().size(), 64);
      for (final String keyword : value.keywords()) {
        writeSized(child, item -> encodeStringNewtype(item, keyword));
      }
    });
  }

  private static void encodeResolutionProof(
      final NoritoEncoder encoder, final ResolutionProof value) {
    writeSized(encoder, child -> encodeRegistrySnapshot(child, value.snapshot()));
    writeSized(encoder, child -> encodeVerificationLock(child, value.lock()));
  }

  private static void encodeRegistrySnapshot(
      final NoritoEncoder encoder, final RegistrySnapshot value) {
    writeSized(encoder, child -> encodeU64(child, value.finalizedHeight()));
    writeSized(encoder, child -> child.writeBytes(value.finalizedBlockHash()));
    writeSized(encoder, child -> encodeU64(child, value.indexRevision()));
  }

  private static void encodeVerificationLock(
      final NoritoEncoder encoder, final VerificationLock value) {
    writeSized(encoder, child -> encodeString(child, value.schema()));
    writeSized(encoder, child -> child.writeByte(value.version()));
    writeSized(encoder, child -> encodeReleaseId(child, value.root()));
    writeSized(encoder, child -> encodeExactEdges(child, value.rootDependencies()));
    writeSized(encoder, child -> {
      child.writeUInt(value.nodes().size(), 64);
      for (final VerificationNode node : value.nodes()) {
        writeSized(child, item -> encodeVerificationNode(item, node));
      }
    });
  }

  private static void encodeExactEdges(
      final NoritoEncoder encoder, final List<ExactDependencyEdge> values) {
    encoder.writeUInt(values.size(), 64);
    for (final ExactDependencyEdge value : values) {
      writeSized(encoder, child -> encodeExactEdge(child, value));
    }
  }

  private static void encodeExactEdge(
      final NoritoEncoder encoder, final ExactDependencyEdge value) {
    writeSized(encoder, child -> encodeString(child, value.alias()));
    writeSized(encoder, child -> child.writeUInt(
        value.kind() == DependencyKind.NORMAL ? 0L : 1L, 32));
    writeSized(encoder, child -> encodePackageId(child, value.packageId()));
    writeSized(encoder, child -> encodeVersionRequirement(child, value.requirement()));
    writeSized(encoder, child -> encodeReleaseId(child, value.selected()));
  }

  private static void encodeVerificationNode(
      final NoritoEncoder encoder, final VerificationNode value) {
    writeSized(encoder, child -> encodeReleaseId(child, value.release()));
    writeSized(encoder, child -> encodeDigest32(child, value.releaseDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.archiveId()));
    writeSized(encoder, child -> encodeDigest32(child, value.sourceDigest()));
    writeSized(encoder, child -> encodeDigest32(child, value.interfaceDigest()));
    writeSized(encoder, child -> encodeAbiBinding(child, value.abi()));
    writeSized(encoder, child -> encodeExactEdges(child, value.dependencies()));
  }

  private static void encodeNamespaceDelegation(
      final NoritoEncoder encoder, final NamespaceDelegation value) {
    writeSized(encoder, child -> encodeNamespaceDelegationPayload(child, value.payload()));
    writeSized(encoder, child -> {
      child.writeUInt(value.approvals().size(), 64);
      for (final NamespaceDelegationApproval approval : value.approvals()) {
        writeSized(child, item -> encodeNamespaceDelegationApproval(item, approval));
      }
    });
  }

  private static void encodeNamespaceDelegationPayload(
      final NoritoEncoder encoder, final NamespaceDelegationPayload value) {
    writeSized(encoder, child -> child.writeByte(value.version()));
    writeSized(encoder, child -> encodeDigest32(child, value.namespaceBinding()));
    writeSized(encoder, child -> encodeU64(child, value.ownerGeneration()));
    writeSized(encoder, child -> encodeAccountId(child, value.owner()));
    writeSized(encoder, child -> encodeAccountId(child, value.delegate()));
    writeSized(encoder, child -> encodeU64(child, value.expiresAtHeight()));
  }

  private static void encodeNamespaceDelegationApproval(
      final NoritoEncoder encoder, final NamespaceDelegationApproval value) {
    writeSized(encoder, child -> encodePublicKey(child, value.publicKey()));
    writeSized(encoder, child -> encodeSignature(child, value.signature()));
  }

  private static void encodeRegistryPolicy(
      final NoritoEncoder encoder, final RegistryPolicy value) {
    writeSized(encoder, child -> child.writeByte(value.version()));
    writeSized(encoder, child -> encodeU64(child, value.revision()));
    writeSized(encoder, child -> child.writeUInt(value.mode().ordinal(), 32));
    writeSized(encoder, child -> {
      child.writeUInt(value.allowlistedDataspaces().size(), 64);
      for (final BigInteger dataspace : value.allowlistedDataspaces()) {
        writeSized(child, item -> encodeU64Newtype(item, dataspace));
      }
    });
    writeSized(encoder, child -> encodeAliasPricing(child, value.aliasPricing()));
  }

  private static void encodeAliasPricing(
      final NoritoEncoder encoder, final AliasPricingPolicy value) {
    writeSized(encoder, child -> encodeU64(child, value.revision()));
    writeSized(encoder, child -> encodeU64(child, value.length1Xor()));
    writeSized(encoder, child -> encodeU64(child, value.length2Xor()));
    writeSized(encoder, child -> encodeU64(child, value.length3Xor()));
    writeSized(encoder, child -> encodeU64(child, value.length4Xor()));
    writeSized(encoder, child -> encodeU64(child, value.length5To32Xor()));
  }

  private static void encodePackageId(final NoritoEncoder encoder, final PackageId value) {
    writeSized(encoder, child -> encodeU64Newtype(child, value.homeDataspace()));
    writeSized(encoder, child -> encodePackageScope(child, value.scope()));
    writeSized(encoder, child -> encodeStringNewtype(child, value.name().value()));
  }

  private static void encodePackageScope(
      final NoritoEncoder encoder, final PackageScope value) {
    if (value.kind() == PackageScope.Kind.DATASPACE_ROOT) {
      encoder.writeUInt(0L, 32);
      return;
    }
    encoder.writeUInt(1L, 32);
    writeSized(encoder, child -> encodeString(child, value.domain()));
  }

  private static void encodePackageRole(final NoritoEncoder encoder, final PackageRole value) {
    if (value.kind() == PackageRole.Kind.OWNER) {
      encoder.writeUInt(0L, 32);
      return;
    }
    encoder.writeUInt(1L, 32);
    writeSized(encoder, child -> encodeMaintainerPermissions(child, value.permissions()));
  }

  private static void encodeMaintainerPermissions(
      final NoritoEncoder encoder, final MaintainerPermissions value) {
    writeSized(encoder, child -> encodeBool(child, value.publish()));
    writeSized(encoder, child -> encodeBool(child, value.yank()));
    writeSized(encoder, child -> encodeBool(child, value.metadata()));
    writeSized(encoder, child -> encodeBool(child, value.archiveLocations()));
  }

  private static void encodeReleaseId(final NoritoEncoder encoder, final ReleaseId value) {
    writeSized(encoder, child -> encodePackageId(child, value.packageId()));
    writeSized(encoder, child -> encodeVersion(child, value.version()));
  }

  private static void encodeVersion(final NoritoEncoder encoder, final Version value) {
    writeSized(encoder, child -> encodeU64(child, value.major()));
    writeSized(encoder, child -> encodeU64(child, value.minor()));
    writeSized(encoder, child -> encodeU64(child, value.patch()));
    writeSized(encoder, child -> encodePrerelease(child, value));
  }

  private static void encodePrerelease(final NoritoEncoder encoder, final Version version) {
    encoder.writeUInt(version.prerelease().size(), 64);
    for (final PrereleaseIdentifier identifier : version.prerelease()) {
      writeSized(encoder, child -> encodePrereleaseIdentifier(child, identifier));
    }
  }

  private static void encodePrereleaseIdentifier(
      final NoritoEncoder encoder, final PrereleaseIdentifier value) {
    if (value.numeric() != null) {
      encoder.writeUInt(0L, 32);
      writeSized(encoder, child -> encodeU64(child, value.numeric()));
      return;
    }
    encoder.writeUInt(1L, 32);
    writeSized(encoder, child -> encodeString(child, value.alphaNumeric()));
  }

  private static void encodeVersionRequirement(
      final NoritoEncoder encoder, final VersionReq value) {
    switch (value.kind()) {
      case ANY:
        encoder.writeUInt(0L, 32);
        return;
      case CARET:
        encoder.writeUInt(1L, 32);
        writeSized(encoder, child -> encodeVersion(child, value.version()));
        return;
      case TILDE:
        encoder.writeUInt(2L, 32);
        writeSized(encoder, child -> encodeVersion(child, value.version()));
        return;
      case MAJOR_WILDCARD:
        encoder.writeUInt(3L, 32);
        writeSized(encoder, child -> encodeU64(child, value.major()));
        return;
      case MINOR_WILDCARD:
        encoder.writeUInt(4L, 32);
        writeSized(encoder, child -> {
          writeSized(child, item -> encodeU64(item, value.major()));
          writeSized(child, item -> encodeU64(item, value.minor()));
        });
        return;
      case EXACT:
        encoder.writeUInt(5L, 32);
        writeSized(encoder, child -> encodeVersion(child, value.version()));
        return;
      case COMPARATORS:
        encoder.writeUInt(6L, 32);
        writeSized(encoder, child -> {
          child.writeUInt(value.comparators().size(), 64);
          for (final VersionComparator comparator : value.comparators()) {
            writeSized(child, item -> encodeVersionComparator(item, comparator));
          }
        });
        return;
      default:
        throw new IllegalStateException("unhandled Musubi version requirement");
    }
  }

  private static void encodeVersionComparator(
      final NoritoEncoder encoder, final VersionComparator value) {
    writeSized(encoder, child -> child.writeUInt(value.op().ordinal(), 32));
    writeSized(encoder, child -> encodeVersion(child, value.version()));
  }

  private static void encodeU64Newtype(final NoritoEncoder encoder, final BigInteger value) {
    writeSized(encoder, child -> encodeU64(child, value));
  }

  private static void encodeStringNewtype(final NoritoEncoder encoder, final String value) {
    writeSized(encoder, child -> encodeString(child, value));
  }

  private static void encodeDigest32(final NoritoEncoder encoder, final Digest32 value) {
    final byte[] bytes = value.bytes();
    encoder.writeLength(bytes.length, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    encoder.writeBytes(bytes);
  }

  private static void encodeString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    encoder.writeBytes(bytes);
  }

  private static void encodeBool(final NoritoEncoder encoder, final boolean value) {
    encoder.writeByte(value ? 1 : 0);
  }

  private static void encodeU64(final NoritoEncoder encoder, final BigInteger value) {
    MusubiModelsV1.requireU64(value, "u64");
    encoder.writeUInt(value.longValue(), 64);
  }

  private static BigInteger requirePositiveRevision(
      final BigInteger value, final String field) {
    MusubiModelsV1.requireU64(value, field);
    if (value.signum() == 0) {
      throw new IllegalArgumentException("Musubi " + field + " must be non-zero");
    }
    return value;
  }

  private static int compareAccountIds(final String left, final String right) {
    try {
      final AccountAddress leftAddress = AccountAddress.parseEncoded(left, null);
      final AccountAddress rightAddress = AccountAddress.parseEncoded(right, null);
      final AccountAddress.SingleKeyPayload leftSingle =
          leftAddress.singleKeyPayload().orElse(null);
      final AccountAddress.SingleKeyPayload rightSingle =
          rightAddress.singleKeyPayload().orElse(null);
      if (leftSingle != null && rightSingle == null) return -1;
      if (leftSingle == null && rightSingle != null) return 1;
      if (leftSingle != null) return comparePublicKeys(leftSingle, rightSingle);

      final AccountAddress.MultisigPolicyPayload leftPolicy =
          leftAddress.multisigPolicyPayload().orElse(null);
      final AccountAddress.MultisigPolicyPayload rightPolicy =
          rightAddress.multisigPolicyPayload().orElse(null);
      if (leftPolicy == null || rightPolicy == null) {
        throw new IllegalArgumentException("Musubi owner has an unsupported account controller");
      }
      int comparison = Integer.compare(leftPolicy.version(), rightPolicy.version());
      if (comparison != 0) return comparison;
      comparison = Integer.compare(leftPolicy.threshold(), rightPolicy.threshold());
      if (comparison != 0) return comparison;
      final List<AccountAddress.MultisigMemberPayload> leftMembers =
          canonicalMultisigMembers(leftPolicy.members());
      final List<AccountAddress.MultisigMemberPayload> rightMembers =
          canonicalMultisigMembers(rightPolicy.members());
      final int common = Math.min(leftMembers.size(), rightMembers.size());
      for (int index = 0; index < common; index++) {
        comparison = compareMultisigMembers(leftMembers.get(index), rightMembers.get(index));
        if (comparison != 0) return comparison;
      }
      return Integer.compare(leftMembers.size(), rightMembers.size());
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException("Musubi owner is not a canonical AccountId", error);
    }
  }

  private static List<AccountAddress.MultisigMemberPayload> canonicalMultisigMembers(
      final List<AccountAddress.MultisigMemberPayload> members) {
    final List<AccountAddress.MultisigMemberPayload> canonical = new ArrayList<>(members);
    Collections.sort(
        canonical,
        (left, right) ->
            compareUnsignedBytes(
                canonicalMultisigMemberSortKey(left), canonicalMultisigMemberSortKey(right)));
    return canonical;
  }

  private static byte[] canonicalMultisigMemberSortKey(
      final AccountAddress.MultisigMemberPayload member) {
    final String algorithm = PublicKeyCodec.algorithmForCurveId(member.curveId());
    if (algorithm == null) {
      throw new IllegalArgumentException("Musubi owner uses an unsupported multisig curve");
    }
    final byte[] algorithmBytes = algorithm.getBytes(StandardCharsets.UTF_8);
    final byte[] publicKey = member.publicKey();
    final byte[] sortKey = new byte[algorithmBytes.length + 1 + publicKey.length];
    System.arraycopy(algorithmBytes, 0, sortKey, 0, algorithmBytes.length);
    sortKey[algorithmBytes.length] = 0;
    System.arraycopy(publicKey, 0, sortKey, algorithmBytes.length + 1, publicKey.length);
    return sortKey;
  }

  private static int comparePublicKeys(
      final AccountAddress.SingleKeyPayload left,
      final AccountAddress.SingleKeyPayload right) {
    return compareUnsignedBytes(
        PublicKeyCodec.compactPublicKeyPayload(left.curveId(), left.publicKey()),
        PublicKeyCodec.compactPublicKeyPayload(right.curveId(), right.publicKey()));
  }

  private static int compareMultisigMembers(
      final AccountAddress.MultisigMemberPayload left,
      final AccountAddress.MultisigMemberPayload right) {
    int comparison = compareUnsignedBytes(
        PublicKeyCodec.compactPublicKeyPayload(left.curveId(), left.publicKey()),
        PublicKeyCodec.compactPublicKeyPayload(right.curveId(), right.publicKey()));
    if (comparison != 0) return comparison;
    return Integer.compare(left.weight(), right.weight());
  }

  private static int compareUnsignedBytes(final byte[] left, final byte[] right) {
    final int common = Math.min(left.length, right.length);
    for (int index = 0; index < common; index++) {
      final int comparison = Integer.compare(left[index] & 0xff, right[index] & 0xff);
      if (comparison != 0) return comparison;
    }
    return Integer.compare(left.length, right.length);
  }

  static byte[] providerBundleAttestationDigest(
      final ProviderBundleVerificationAttestation attestation) {
    return domainHash(
        "iroha.musubi.provider-bundle-attestation.digest.v1",
        encoded(encoder -> encodeProviderAttestation(encoder, attestation)));
  }

  static byte[] releaseManifestDigest(final ReleaseManifest manifest) {
    return domainHash(
        "iroha.musubi.release-digest.v1",
        encoded(encoder -> encodeReleaseManifest(encoder, manifest)));
  }

  private static boolean namespaceMatchesPackage(
      final String namespace, final PackageId packageId) {
    final int separator = namespace.indexOf('.');
    if (packageId.scope().kind() == PackageScope.Kind.DATASPACE_ROOT) {
      return separator < 0;
    }
    return separator > 0
        && namespace.substring(0, separator).equals(packageId.scope().domain());
  }

  private static byte[] domainHash(final String domain, final byte[] encoded) {
    final byte[] domainBytes = domain.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[8 + domainBytes.length + 8 + encoded.length];
    writeLittleEndianU64(preimage, 0, domainBytes.length);
    System.arraycopy(domainBytes, 0, preimage, 8, domainBytes.length);
    final int encodedLengthOffset = 8 + domainBytes.length;
    writeLittleEndianU64(preimage, encodedLengthOffset, encoded.length);
    System.arraycopy(encoded, 0, preimage, encodedLengthOffset + 8, encoded.length);
    return Blake3.hashUnbounded(preimage);
  }

  private static void writeLittleEndianU64(
      final byte[] target, final int offset, final long value) {
    for (int index = 0; index < 8; index++) {
      target[offset + index] = (byte) (value >>> (index * 8));
    }
  }

  private static byte[] hexBytes(final String value) {
    if (value == null || (value.length() & 1) != 0 || !value.matches("[0-9A-Fa-f]+")) {
      throw new IllegalArgumentException("Musubi value is not hexadecimal");
    }
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static Digest32 requireNonZeroDigest(final Digest32 value, final String field) {
    Objects.requireNonNull(value, field);
    for (final byte octet : value.bytes()) {
      if (octet != 0) {
        return value;
      }
    }
    throw new IllegalArgumentException("Musubi " + field + " must be non-zero");
  }

  private static final class BarePayloadAdapter implements TypeAdapter<byte[]> {
    private static final BarePayloadAdapter INSTANCE = new BarePayloadAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      encoder.writeBytes(value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      return decoder.readBytes(decoder.remaining());
    }
  }
}
