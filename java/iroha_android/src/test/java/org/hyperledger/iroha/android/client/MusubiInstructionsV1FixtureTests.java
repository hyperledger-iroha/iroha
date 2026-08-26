package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.AcceptMusubiPackageMaintainerV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.AddMusubiArchiveLocationV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.AssertMusubiReleaseDigestV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.InviteMusubiPackageMaintainerV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.PublishMusubiReleaseV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RecoverMusubiPackageV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RegisterMusubiArchiveV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RegisterMusubiAliasV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RegisterMusubiNamespaceBindingV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RegisterMusubiProviderBundleAttestationV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RemoveMusubiPackageMaintainerV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RetargetMusubiAliasV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RetireMusubiArchiveLocationV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.RevokeMusubiPackageMaintainerInvitationV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.SetMusubiArtifactTakedownV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.SetMusubiPackageMaintainerRoleV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.SetMusubiPackageMetadataV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.SetMusubiRegistryPolicyV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.SetMusubiReleaseYankV1;
import org.hyperledger.iroha.android.client.MusubiInstructionsV1.TypedInstructionV1;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasName;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasPricingPolicy;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveCommitment;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ChunkerProfileHandle;
import org.hyperledger.iroha.android.client.MusubiModelsV1.DependencyKind;
import org.hyperledger.iroha.android.client.MusubiModelsV1.DependencyRequirement;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Digest32;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactDependencyEdge;
import org.hyperledger.iroha.android.client.MusubiModelsV1.GovernanceDecision;
import org.hyperledger.iroha.android.client.MusubiModelsV1.MaintainerPermissions;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Namespace;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceDelegation;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceDelegationApproval;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceDelegationPayload;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageName;
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
import org.hyperledger.iroha.android.model.ExecutableBatchItem;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.junit.Test;

/** Cross-language fixture checks for typed Musubi mutation construction and framing. */
public final class MusubiInstructionsV1FixtureTests {
  private static final String FIXTURE_PATH = "fixtures/musubi/instructions_v1.json";

  @Test
  public void musubiHashingSupportsPayloadsAboveTheLegacyBlake3HelperLimit() {
    final byte[] input = new byte[100_000];
    assertThrows(IllegalArgumentException.class, () -> Blake3.hash(input));
    assertArrayEquals(
        decodeHex("b1fc3c3bf473596bc8ac1f5c86f77c2fc0e0186a872b88adf841716fe9140a50"),
        Blake3.hashUnbounded(input));
  }

  @Test
  public void publicationAndPolicyRejectMismatchedConsensusDigests() throws Exception {
    final Map<String, Object> publishSemantic =
        object(fixtureCase("publish-delegated-domain-release").get("semantic"));
    final Publication publication = publication(publishSemantic.get("publication"));
    final ReleaseManifest manifest = publication.manifest();
    final byte[] wrongLockDigest = manifest.verificationLockDigest().bytes();
    wrongLockDigest[0] ^= 1;
    final Publication mismatchedPublication =
        new Publication(
            new ReleaseManifest(
                manifest.release(),
                manifest.abi(),
                manifest.dependencies(),
                manifest.exports(),
                manifest.interfaceDigest(),
                manifest.metadata(),
                manifest.archiveId(),
                Digest32.fromBytes(wrongLockDigest)),
            publication.resolution());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new PublishMusubiReleaseV1(
                new Namespace(newtypeText(publishSemantic.get("namespace"))),
                mismatchedPublication,
                namespaceDelegation(publishSemantic.get("namespace_delegation")),
                unsigned(publishSemantic.get("expected_policy_revision")),
                optionalUnsigned(publishSemantic.get("expected_governance_revision"))));

    final Map<String, Object> policySemantic =
        object(fixtureCase("set-allowlisted-policy-repriced-aliases").get("semantic"));
    final GovernanceDecision decision = governanceDecision(policySemantic.get("decision"));
    final byte[] wrongActionDigest = decision.actionDigest().bytes();
    wrongActionDigest[0] ^= 1;
    final GovernanceDecision mismatchedDecision =
        new GovernanceDecision(
            decision.decisionId(),
            Digest32.fromBytes(wrongActionDigest),
            decision.enactedAtHeight(),
            decision.executeAfterHeight());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SetMusubiRegistryPolicyV1(
                mismatchedDecision,
                registryPolicy(policySemantic.get("policy")),
                unsigned(policySemantic.get("expected_policy_revision"))));
  }

  @Test
  public void newMutationBuildersRejectZeroCasRevisions() throws Exception {
    final RegisterMusubiArchiveV1 register =
        (RegisterMusubiArchiveV1)
            instruction(fixtureCase("register-archive-max-bounds-signed-receipt"));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterMusubiArchiveV1(
                register.commitment(), register.stagingReceipt(), BigInteger.ZERO));

    final RegisterMusubiProviderBundleAttestationV1 registerProviderAttestation =
        (RegisterMusubiProviderBundleAttestationV1)
            instruction(fixtureCase("register-provider-bundle-attestation"));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterMusubiProviderBundleAttestationV1(
                registerProviderAttestation.attestation(), BigInteger.ZERO));

    final AddMusubiArchiveLocationV1 add =
        (AddMusubiArchiveLocationV1)
            instruction(fixtureCase("add-location-three-signed-providers"));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new AddMusubiArchiveLocationV1(
                add.archiveId(),
                add.locationId(),
                add.pinManifest(),
                add.replicationOrder(),
                add.providerAttestationSetDigest(),
                add.renewAfterEpoch(),
                add.expiresAtEpoch(),
                BigInteger.ZERO));

    final PublishMusubiReleaseV1 publish =
        (PublishMusubiReleaseV1)
            instruction(fixtureCase("publish-delegated-domain-release"));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new PublishMusubiReleaseV1(
                publish.namespace(),
                publish.publication(),
                publish.namespaceDelegation(),
                BigInteger.ZERO,
                publish.expectedGovernanceRevision()));

    final SetMusubiPackageMetadataV1 metadata =
        (SetMusubiPackageMetadataV1)
            instruction(fixtureCase("replace-domain-metadata-high-revision"));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SetMusubiPackageMetadataV1(
                metadata.packageId(), metadata.metadata(), BigInteger.ZERO));

    final SetMusubiRegistryPolicyV1 policy =
        (SetMusubiRegistryPolicyV1)
            instruction(fixtureCase("set-allowlisted-policy-repriced-aliases"));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SetMusubiRegistryPolicyV1(
                policy.decision(), policy.policy(), BigInteger.ZERO));
  }

  @Test
  public void protocolNameOrderingUsesUnsignedUtf8Bytes() throws Exception {
    final String bmpPrivateUse = "\ue000";
    final String supplementary = "\ud800\udc00";
    assertTrue(bmpPrivateUse.compareTo(supplementary) > 0);

    final Publication publication =
        ((PublishMusubiReleaseV1)
                instruction(fixtureCase("publish-delegated-domain-release")))
            .publication();
    final DependencyRequirement dependency = publication.manifest().dependencies().get(0);
    final DependencyRequirement bmpDependency =
        new DependencyRequirement(
            bmpPrivateUse, dependency.packageId(), dependency.requirement());
    final DependencyRequirement supplementaryDependency =
        new DependencyRequirement(
            supplementary, dependency.packageId(), dependency.requirement());
    assertTrue(bmpDependency.compareTo(supplementaryDependency) < 0);

    final ExactDependencyEdge edge =
        publication.resolution().lock().rootDependencies().get(0);
    final ExactDependencyEdge bmpEdge =
        new ExactDependencyEdge(
            bmpPrivateUse,
            edge.kind(),
            edge.packageId(),
            edge.requirement(),
            edge.selected());
    final ExactDependencyEdge supplementaryEdge =
        new ExactDependencyEdge(
            supplementary,
            edge.kind(),
            edge.packageId(),
            edge.requirement(),
            edge.selected());
    assertTrue(bmpEdge.compareTo(supplementaryEdge) < 0);

    final ReleaseManifest manifest = publication.manifest();
    final ReleaseManifest orderedExports =
        new ReleaseManifest(
            manifest.release(),
            manifest.abi(),
            manifest.dependencies(),
            Arrays.asList(bmpPrivateUse, supplementary),
            manifest.interfaceDigest(),
            manifest.metadata(),
            manifest.archiveId(),
            manifest.verificationLockDigest());
    assertEquals(Arrays.asList(bmpPrivateUse, supplementary), orderedExports.exports());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new ReleaseManifest(
                manifest.release(),
                manifest.abi(),
                manifest.dependencies(),
                Arrays.asList(supplementary, bmpPrivateUse),
                manifest.interfaceDigest(),
                manifest.metadata(),
                manifest.archiveId(),
                manifest.verificationLockDigest()));
  }

  @Test
  public void parentLocalDependencyAliasesAreUniqueAcrossPublishedGraphs() throws Exception {
    final Publication publication =
        ((PublishMusubiReleaseV1)
                instruction(fixtureCase("publish-delegated-domain-release")))
            .publication();
    final ReleaseManifest manifest = publication.manifest();
    final VerificationLock lock = publication.resolution().lock();
    final DependencyRequirement firstRequirement = manifest.dependencies().get(0);
    final ExactDependencyEdge firstEdge = lock.rootDependencies().get(0);
    final VerificationNode firstNode = lock.nodes().get(0);
    final PackageId secondPackage =
        new PackageId(
            firstRequirement.packageId().homeDataspace(),
            firstRequirement.packageId().scope(),
            new PackageName("vector-next"));
    final DependencyRequirement secondRequirement =
        new DependencyRequirement(
            firstRequirement.alias(), secondPackage, firstRequirement.requirement());
    final ReleaseId secondRelease = new ReleaseId(secondPackage, firstEdge.selected().version());
    final ExactDependencyEdge secondEdge =
        new ExactDependencyEdge(
            firstEdge.alias(),
            firstEdge.kind(),
            secondPackage,
            firstEdge.requirement(),
            secondRelease);
    final List<DependencyRequirement> requirements =
        Arrays.asList(firstRequirement, secondRequirement);
    final List<ExactDependencyEdge> edges = Arrays.asList(firstEdge, secondEdge);

    assertUniqueAliasFailure(
        () ->
            new ReleaseManifest(
                manifest.release(),
                manifest.abi(),
                requirements,
                manifest.exports(),
                manifest.interfaceDigest(),
                manifest.metadata(),
                manifest.archiveId(),
                manifest.verificationLockDigest()));
    assertUniqueAliasFailure(
        () ->
            new VerificationNode(
                firstNode.release(),
                firstNode.releaseDigest(),
                firstNode.archiveId(),
                firstNode.sourceDigest(),
                firstNode.interfaceDigest(),
                firstNode.abi(),
                edges));
    final VerificationNode secondNode =
        new VerificationNode(
            secondRelease,
            firstNode.releaseDigest(),
            firstNode.archiveId(),
            firstNode.sourceDigest(),
            firstNode.interfaceDigest(),
            firstNode.abi(),
            Collections.emptyList());
    assertUniqueAliasFailure(
        () ->
            new VerificationLock(
                lock.root(), edges, Arrays.asList(firstNode, secondNode)));
  }

  @Test
  public void canonicalRustFixtureMatchesEveryJavaEncodingLayer() throws Exception {
    final Map<String, Object> fixture = fixture();
    assertEquals("iroha-musubi-instructions-v1", fixture.get("format"));
    assertEquals(1, number(fixture.get("fixture_version")).intValue());
    assertEquals("iroha_data_model::isi::musubi", fixture.get("rust_owner"));
    assertEquals(
        "(alloc::string::String, alloc::vec::Vec<u8>)",
        fixture.get("instruction_box_schema_name"));
    final byte[] instructionBoxSchemaHash =
        decodeHex(string(fixture.get("instruction_box_schema_hash")));
    assertArrayEquals(
        instructionBoxSchemaHash,
        SchemaHash.hash16(string(fixture.get("instruction_box_schema_name"))));
    final List<Object> invalidDigestOctets =
        new ArrayList<Object>(Collections.nCopies(32, 0L));
    invalidDigestOctets.set(0, 256L);
    assertThrows(
        AssertionError.class,
        () -> digest(Collections.<Object>singletonList(invalidDigestOctets)));

    final List<Object> cases = array(fixture.get("cases"));
    assertEquals(19, cases.size());
    final List<String> caseIds = new ArrayList<>();
    for (final Object rawCase : cases) {
      caseIds.add(string(object(rawCase).get("id")));
    }
    assertEquals(
        Arrays.asList(
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
            "set-allowlisted-policy-repriced-aliases"),
        caseIds);
    for (final Object rawCase : cases) {
      final Map<String, Object> fixtureCase = object(rawCase);
      final TypedInstructionV1 instruction = instruction(fixtureCase);

      assertEquals(fixtureCase.get("wire_id"), instruction.wireId());
      assertEquals(fixtureCase.get("concrete_schema_name"), instruction.concreteSchemaName());
      assertEquals(2, number(fixtureCase.get("header_flags")).intValue());
      assertArrayEquals(
          decodeHex(string(fixtureCase.get("bare_payload_hex"))), instruction.barePayload());
      assertArrayEquals(
          decodeHex(string(fixtureCase.get("concrete_frame_hex"))),
          instruction.concreteFrame());

      final byte[] concreteSchemaHash =
          decodeHex(string(fixtureCase.get("concrete_schema_hash")));
      assertArrayEquals(
          concreteSchemaHash, SchemaHash.hash16(instruction.concreteSchemaName()));
      final NoritoHeader.DecodeResult concrete =
          NoritoHeader.decode(instruction.concreteFrame(), concreteSchemaHash);
      concrete.header().validateChecksum(concrete.payload());
      assertEquals(2, concrete.header().flags());
      assertArrayEquals(instruction.barePayload(), concrete.payload());

      final InstructionBox box = instruction.toInstructionBox();
      assertEquals(InstructionKind.CUSTOM, box.kind());
      assertTrue(box.payload() instanceof InstructionBox.WirePayload);
      final InstructionBox.WirePayload wire = (InstructionBox.WirePayload) box.payload();
      assertEquals(instruction.wireId(), wire.wireName());
      assertArrayEquals(instruction.concreteFrame(), wire.payloadBytes());

      final byte[] standalone = NoritoJavaCodecAdapter.encodeInstructionBox(box);
      assertArrayEquals(
          decodeHex(string(fixtureCase.get("standalone_instruction_box_frame_hex"))), standalone);
      final NoritoHeader.DecodeResult instructionBox =
          NoritoHeader.decode(standalone, instructionBoxSchemaHash);
      instructionBox.header().validateChecksum(instructionBox.payload());
      assertEquals(2, instructionBox.header().flags());
      assertArrayEquals(
          decodeHex(string(fixtureCase.get("instruction_box_pair_hex"))),
          instructionBox.payload());
      assertCanonicalBoundedInstructionPair(
          instructionBox.payload(), instruction.wireId(), instruction.concreteFrame());
    }
  }

  @Test
  public void transactionBatchPreservesEveryFixtureInstructionPairInline() throws Exception {
    final List<Object> fixtureCases = array(fixture().get("cases"));
    final List<ExecutableBatchItem> items = new ArrayList<>();
    for (final Object rawCase : fixtureCases) {
      items.add(ExecutableBatchItem.instruction(instruction(object(rawCase)).toInstructionBox()));
    }
    final String authority =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x5a), "ed25519")
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final TransactionPayload transaction =
        TransactionPayload.builder()
            .setNetworkId(
                org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
            .setAuthority(authority)
            .setCreationTimeMs(1_735_555_000_000L)
            .setBatch(items)
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
            .build();
    final byte[] encoded =
        new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
            .encodeTransaction(transaction);

    final NoritoDecoder transactionDecoder = canonicalDecoder(encoded);
    readField(transactionDecoder, "network_id");
    readField(transactionDecoder, "authority");
    readField(transactionDecoder, "creation_time_ms");
    final byte[] executablePayload = readField(transactionDecoder, "executable");
    for (final String field :
        Arrays.asList(
            "time_to_live_ms",
            "nonce",
            "fee_payment",
            "admission_intent",
            "metadata",
            "attachments")) {
      readField(transactionDecoder, field);
    }
    assertEquals(0, transactionDecoder.remaining());

    final NoritoDecoder executableDecoder = canonicalDecoder(executablePayload);
    assertEquals(4L, executableDecoder.readUInt(32));
    final byte[] batchPayload = readField(executableDecoder, "batch");
    assertEquals(0, executableDecoder.remaining());

    final NoritoDecoder batchDecoder = canonicalDecoder(batchPayload);
    assertEquals(fixtureCases.size(), batchDecoder.readUInt(64));
    for (int index = 0; index < fixtureCases.size(); index++) {
      final NoritoDecoder item =
          canonicalDecoder(readField(batchDecoder, "batch[" + index + "]"));
      assertEquals(0L, item.readUInt(32));
      assertArrayEquals(
          decodeHex(string(object(fixtureCases.get(index)).get("instruction_box_pair_hex"))),
          readField(item, "batch[" + index + "].instruction"));
      assertEquals(0, item.remaining());
    }
    assertEquals(0, batchDecoder.remaining());
  }

  @Test
  public void aliasNameEnforcesThePermanentAliasGrammar() {
    assertEquals("a", new AliasName("a").value());
    assertEquals(repeat('z', 32), new AliasName(repeat('z', 32)).value());
    for (final String invalid :
        new String[] {"", "Oracle", "-oracle", "oracle-", "oracle--tools", "orácle",
            repeat('z', 33)}) {
      assertThrows(IllegalArgumentException.class, () -> new AliasName(invalid));
    }
  }

  @Test
  public void mutationInputsRejectInvalidRevisionsAndAccounts() throws Exception {
    final PackageId packageId =
        new PackageId(
            BigInteger.ZERO, PackageScope.dataspaceRoot(), new PackageName("boundary"));
    final Digest32 inviteId = Digest32.fromBytes(new byte[32]);
    final String canonicalAccount =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x31), "ed25519")
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final PackageRole owner = PackageRole.owner();
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new AcceptMusubiPackageMaintainerV1(
                packageId, inviteId, MusubiModelsV1.U64_MAX.add(BigInteger.ONE)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterMusubiAliasV1(
                new AliasName("boundary"), packageId, BigInteger.valueOf(-1L)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RetireMusubiArchiveLocationV1(
                inviteId,
                inviteId,
                MusubiModelsV1.U64_MAX.add(BigInteger.ONE),
                new Reason("retired")));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SetMusubiReleaseYankV1(
                new ReleaseId(packageId, Version.parse("1.0.0")),
                true,
                new Reason("withdrawn"),
                BigInteger.valueOf(-1L)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RemoveMusubiPackageMaintainerV1(
                packageId, "not-a-canonical-account", BigInteger.ZERO));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterMusubiNamespaceBindingV1(
                new NamespaceBinding(
                    new Namespace("boundary"),
                    BigInteger.ZERO,
                    PackageScope.dataspaceRoot(),
                    BigInteger.ONE),
                MusubiModelsV1.U64_MAX.add(BigInteger.ONE)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new InviteMusubiPackageMaintainerV1(
                packageId,
                inviteId,
                canonicalAccount,
                owner,
                BigInteger.ONE,
                BigInteger.ZERO));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new InviteMusubiPackageMaintainerV1(
                packageId,
                Digest32.fromBytes(repeatedBytes((byte) 1)),
                canonicalAccount,
                owner,
                BigInteger.ZERO,
                BigInteger.ZERO));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new InviteMusubiPackageMaintainerV1(
                packageId,
                Digest32.fromBytes(repeatedBytes((byte) 1)),
                "not-a-canonical-account",
                owner,
                BigInteger.ONE,
                BigInteger.ZERO));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SetMusubiPackageMaintainerRoleV1(
                packageId,
                canonicalAccount,
                owner,
                MusubiModelsV1.U64_MAX.add(BigInteger.ONE)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SetMusubiPackageMaintainerRoleV1(
                packageId, "not-a-canonical-account", owner, BigInteger.ZERO));

    final Digest32 nonZeroDigest = Digest32.fromBytes(repeatedBytes((byte) 0x5a));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new GovernanceDecision(
                Digest32.fromBytes(new byte[32]),
                nonZeroDigest,
                BigInteger.ONE,
                BigInteger.valueOf(2L)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new GovernanceDecision(
                nonZeroDigest,
                nonZeroDigest,
                BigInteger.valueOf(2L),
                BigInteger.valueOf(2L)));
    final GovernanceDecision decision =
        new GovernanceDecision(
            nonZeroDigest, nonZeroDigest, BigInteger.ONE, BigInteger.valueOf(2L));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RecoverMusubiPackageV1(
                decision,
                packageId,
                Arrays.asList(canonicalAccount, canonicalAccount),
                BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RecoverMusubiPackageV1(
                decision, packageId, Collections.<String>emptyList(), BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RetargetMusubiAliasV1(
                decision, new AliasName("boundary"), packageId, BigInteger.ZERO));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SetMusubiArtifactTakedownV1(
                decision,
                new ReleaseId(packageId, Version.parse("1.0.0")),
                new Reason("takedown"),
                BigInteger.ZERO));
  }

  @Test
  public void recoveryRejectsAlternateMultisigSpellingsOfTheSameAccount() throws Exception {
    final AccountAddress.MultisigMemberPayload firstMember =
        AccountAddress.MultisigMemberPayload.of(
            1, 1, TestEd25519Keys.publicKey(0x41));
    final AccountAddress.MultisigMemberPayload secondMember =
        AccountAddress.MultisigMemberPayload.of(
            1, 2, TestEd25519Keys.publicKey(0x42));
    final String forward =
        AccountAddress.fromMultisigPolicy(
                AccountAddress.MultisigPolicyPayload.of(
                    1, 2, Arrays.asList(firstMember, secondMember)))
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final String reverse =
        AccountAddress.fromMultisigPolicy(
                AccountAddress.MultisigPolicyPayload.of(
                    1, 2, Arrays.asList(secondMember, firstMember)))
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    assertTrue(!forward.equals(reverse));
    assertArrayEquals(
        TransferWirePayloadEncoder.encodeAccountIdPayload(forward),
        TransferWirePayloadEncoder.encodeAccountIdPayload(reverse));

    final Digest32 digest = Digest32.fromBytes(repeatedBytes((byte) 0x5a));
    final GovernanceDecision decision =
        new GovernanceDecision(
            digest, digest, BigInteger.ONE, BigInteger.valueOf(2L));
    final PackageId packageId =
        new PackageId(
            BigInteger.ZERO, PackageScope.dataspaceRoot(), new PackageName("multisig-recovery"));

    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RecoverMusubiPackageV1(
                decision, packageId, Arrays.asList(forward, reverse), BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RecoverMusubiPackageV1(
                decision, packageId, Arrays.asList(reverse, forward), BigInteger.ONE));
  }

  @Test
  public void providerCompletionAuthorityUsesCanonicalMultisigIdentity() throws Exception {
    final AccountAddress.MultisigMemberPayload firstMember =
        AccountAddress.MultisigMemberPayload.of(
            1, 1, TestEd25519Keys.publicKey(0x61));
    final AccountAddress.MultisigMemberPayload secondMember =
        AccountAddress.MultisigMemberPayload.of(
            1, 2, TestEd25519Keys.publicKey(0x62));
    final String forward =
        AccountAddress.fromMultisigPolicy(
                AccountAddress.MultisigPolicyPayload.of(
                    1, 2, Arrays.asList(firstMember, secondMember)))
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final String reverse =
        AccountAddress.fromMultisigPolicy(
                AccountAddress.MultisigPolicyPayload.of(
                    1, 2, Arrays.asList(secondMember, firstMember)))
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    assertTrue(!forward.equals(reverse));
    assertArrayEquals(
        TransferWirePayloadEncoder.encodeAccountIdPayload(forward),
        TransferWirePayloadEncoder.encodeAccountIdPayload(reverse));

    final Map<String, Object> semantic =
        object(fixtureCase("register-provider-bundle-attestation").get("semantic"));
    final ProviderBundleVerificationBinding template =
        providerAttestation(semantic.get("attestation"))
            .payload()
            .binding();
    final ProviderCompletionAuthority equivalentAuthority =
        new ProviderCompletionAuthority(
            reverse, template.completionAuthority().signerPolicy());
    final ProviderBundleVerificationBinding binding =
        providerBindingWithAccounts(template, forward, equivalentAuthority);
    assertEquals(forward, binding.completedBy());
    assertEquals(reverse, binding.completionAuthority().providerOwner());

    final String unrelated =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x63), "ed25519")
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    assertThrows(
        IllegalArgumentException.class,
        () ->
            providerBindingWithAccounts(
                template,
                forward,
                new ProviderCompletionAuthority(
                    unrelated, template.completionAuthority().signerPolicy())));
  }

  @Test
  public void namespaceBindingsAndPackageRolesEnforceTheirStructuralContracts() {
    final NamespaceBinding root =
        new NamespaceBinding(
            new Namespace("payments"),
            MusubiModelsV1.U64_MAX,
            PackageScope.dataspaceRoot(),
            BigInteger.ONE);
    assertEquals("payments", root.namespace().value());
    assertEquals(MusubiModelsV1.U64_MAX, root.homeDataspace());
    assertEquals(PackageRole.Kind.OWNER, PackageRole.owner().kind());

    final MaintainerPermissions permissions =
        new MaintainerPermissions(true, false, true, false);
    final PackageRole maintainer = PackageRole.maintainer(permissions);
    assertEquals(PackageRole.Kind.MAINTAINER, maintainer.kind());
    assertEquals(permissions, maintainer.permissions());
    assertThrows(
        IllegalArgumentException.class,
        () -> new MaintainerPermissions(false, false, false, false));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new NamespaceBinding(
                new Namespace("finance.payments"),
                BigInteger.ONE,
                PackageScope.dataspaceRoot(),
                BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new NamespaceBinding(
                new Namespace("payments"),
                BigInteger.ONE,
                PackageScope.dataspaceRoot(),
                BigInteger.ZERO));
  }

  @Test
  public void reasonEnforcesTheRustBoundedCleanTextContract() {
    assertEquals(repeat('r', 1_024), new Reason(repeat('r', 1_024)).value());
    assertEquals(repeat('\u00e9', 512), new Reason(repeat('\u00e9', 512)).value());
    for (final String invalid :
        new String[] {
          "",
          " leading",
          "trailing ",
          "line\nbreak",
          "\u00a0leading",
          "\ud800",
          repeat('r', 1_025),
          repeat('\u00e9', 513)
        }) {
      assertThrows(IllegalArgumentException.class, () -> new Reason(invalid));
    }
  }

  private static TypedInstructionV1 instruction(final Map<String, Object> fixtureCase) {
    final Map<String, Object> semantic = object(fixtureCase.get("semantic"));
    final String id = string(fixtureCase.get("id"));
    if ("accept-root-max-revision".equals(id)) {
      assertSemanticKeys(
          semantic, "package", "invite_id", "expected_governance_revision");
      return new AcceptMusubiPackageMaintainerV1(
          packageId(semantic.get("package")),
          digest(semantic.get("invite_id")),
          unsigned(semantic.get("expected_governance_revision")));
    }
    if ("revoke-domain-invitation".equals(id)) {
      assertSemanticKeys(
          semantic, "package", "invite_id", "expected_governance_revision");
      return new RevokeMusubiPackageMaintainerInvitationV1(
          packageId(semantic.get("package")),
          digest(semantic.get("invite_id")),
          unsigned(semantic.get("expected_governance_revision")));
    }
    if ("register-alias-domain-target".equals(id)) {
      assertSemanticKeys(semantic, "alias", "target", "expected_pricing_revision");
      return new RegisterMusubiAliasV1(
          new AliasName(newtypeText(semantic.get("alias"))),
          packageId(semantic.get("target")),
          unsigned(semantic.get("expected_pricing_revision")));
    }
    if ("assert-prerelease-digest".equals(id)) {
      assertSemanticKeys(semantic, "release", "expected_digest");
      return new AssertMusubiReleaseDigestV1(
          releaseId(semantic.get("release")), digest(semantic.get("expected_digest")));
    }
    if ("retire-location-max-revision".equals(id)) {
      assertSemanticKeys(
          semantic,
          "archive_id",
          "location_id",
          "expected_location_revision",
          "reason");
      return new RetireMusubiArchiveLocationV1(
          digest(semantic.get("archive_id")),
          digest(semantic.get("location_id")),
          unsigned(semantic.get("expected_location_revision")),
          new Reason(newtypeText(semantic.get("reason"))));
    }
    if ("unyank-domain-release-high-revision".equals(id)) {
      assertSemanticKeys(
          semantic, "release", "yanked", "reason", "expected_yank_revision");
      return new SetMusubiReleaseYankV1(
          releaseId(semantic.get("release")),
          bool(semantic.get("yanked")),
          new Reason(newtypeText(semantic.get("reason"))),
          unsigned(semantic.get("expected_yank_revision")));
    }
    if ("remove-root-maintainer-high-revision".equals(id)) {
      assertSemanticKeys(
          semantic, "package", "account", "expected_governance_revision");
      return new RemoveMusubiPackageMaintainerV1(
          packageId(semantic.get("package")),
          string(semantic.get("account")),
          unsigned(semantic.get("expected_governance_revision")));
    }
    if ("register-domain-namespace-max-generation".equals(id)) {
      assertSemanticKeys(semantic, "binding", "expected_policy_revision");
      return new RegisterMusubiNamespaceBindingV1(
          namespaceBinding(semantic.get("binding")),
          unsigned(semantic.get("expected_policy_revision")));
    }
    if ("invite-domain-maintainer-max-expiry".equals(id)) {
      assertSemanticKeys(
          semantic,
          "package",
          "invite_id",
          "invited_account",
          "role",
          "expires_at_height",
          "expected_governance_revision");
      return new InviteMusubiPackageMaintainerV1(
          packageId(semantic.get("package")),
          digest(semantic.get("invite_id")),
          string(semantic.get("invited_account")),
          packageRole(semantic.get("role")),
          unsigned(semantic.get("expires_at_height")),
          unsigned(semantic.get("expected_governance_revision")));
    }
    if ("promote-root-member-to-owner-high-revision".equals(id)) {
      assertSemanticKeys(
          semantic, "package", "account", "role", "expected_governance_revision");
      return new SetMusubiPackageMaintainerRoleV1(
          packageId(semantic.get("package")),
          string(semantic.get("account")),
          packageRole(semantic.get("role")),
          unsigned(semantic.get("expected_governance_revision")));
    }
    if ("recover-domain-package-three-owners".equals(id)) {
      assertSemanticKeys(
          semantic, "decision", "package", "owners", "expected_governance_revision");
      final List<String> owners = new ArrayList<>();
      for (final Object owner : array(semantic.get("owners"))) owners.add(string(owner));
      return new RecoverMusubiPackageV1(
          governanceDecision(semantic.get("decision")),
          packageId(semantic.get("package")),
          owners,
          unsigned(semantic.get("expected_governance_revision")));
    }
    if ("retarget-one-character-alias-high-revision".equals(id)) {
      assertSemanticKeys(semantic, "decision", "alias", "target", "expected_history_revision");
      return new RetargetMusubiAliasV1(
          governanceDecision(semantic.get("decision")),
          new AliasName(newtypeText(semantic.get("alias"))),
          packageId(semantic.get("target")),
          unsigned(semantic.get("expected_history_revision")));
    }
    if ("takedown-max-major-prerelease".equals(id)) {
      assertSemanticKeys(
          semantic,
          "decision",
          "release",
          "reason",
          "expected_artifact_governance_revision");
      return new SetMusubiArtifactTakedownV1(
          governanceDecision(semantic.get("decision")),
          releaseId(semantic.get("release")),
          new Reason(newtypeText(semantic.get("reason"))),
          unsigned(semantic.get("expected_artifact_governance_revision")));
    }
    if ("register-archive-max-bounds-signed-receipt".equals(id)) {
      assertSemanticKeys(
          semantic, "commitment", "staging_receipt", "expected_policy_revision");
      return new RegisterMusubiArchiveV1(
          archiveCommitment(semantic.get("commitment")),
          seedIngressReceipt(semantic.get("staging_receipt")),
          unsigned(semantic.get("expected_policy_revision")));
    }
    if ("register-provider-bundle-attestation".equals(id)) {
      assertSemanticKeys(semantic, "attestation", "expected_location_revision");
      return new RegisterMusubiProviderBundleAttestationV1(
          providerAttestation(semantic.get("attestation")),
          unsigned(semantic.get("expected_location_revision")));
    }
    if ("add-location-three-signed-providers".equals(id)) {
      assertSemanticKeys(
          semantic,
          "archive_id",
          "location_id",
          "pin_manifest",
          "replication_order",
          "provider_attestation_set_digest",
          "renew_after_epoch",
          "expires_at_epoch",
          "expected_location_revision");
      return new AddMusubiArchiveLocationV1(
          digest(semantic.get("archive_id")),
          digest(semantic.get("location_id")),
          digest(semantic.get("pin_manifest")),
          digest(semantic.get("replication_order")),
          ProviderBundleAttestationSetDigest.fromBytes(
              digest(semantic.get("provider_attestation_set_digest")).bytes()),
          unsigned(semantic.get("renew_after_epoch")),
          unsigned(semantic.get("expires_at_epoch")),
          unsigned(semantic.get("expected_location_revision")));
    }
    if ("publish-delegated-domain-release".equals(id)) {
      assertSemanticKeys(
          semantic,
          "namespace",
          "publication",
          "namespace_delegation",
          "expected_policy_revision",
          "expected_governance_revision");
      return new PublishMusubiReleaseV1(
          new Namespace(newtypeText(semantic.get("namespace"))),
          publication(semantic.get("publication")),
          namespaceDelegation(semantic.get("namespace_delegation")),
          unsigned(semantic.get("expected_policy_revision")),
          optionalUnsigned(semantic.get("expected_governance_revision")));
    }
    if ("replace-domain-metadata-high-revision".equals(id)) {
      assertSemanticKeys(semantic, "package", "metadata", "expected_metadata_revision");
      return new SetMusubiPackageMetadataV1(
          packageId(semantic.get("package")),
          releaseMetadata(semantic.get("metadata")),
          unsigned(semantic.get("expected_metadata_revision")));
    }
    if ("set-allowlisted-policy-repriced-aliases".equals(id)) {
      assertSemanticKeys(semantic, "decision", "policy", "expected_policy_revision");
      return new SetMusubiRegistryPolicyV1(
          governanceDecision(semantic.get("decision")),
          registryPolicy(semantic.get("policy")),
          unsigned(semantic.get("expected_policy_revision")));
    }
    throw new AssertionError("unhandled Musubi instruction fixture: " + id);
  }

  private static void assertSemanticKeys(
      final Map<String, Object> semantic, final String... expectedKeys) {
    assertEquals(new HashSet<>(Arrays.asList(expectedKeys)), semantic.keySet());
  }

  private static GovernanceDecision governanceDecision(final Object value) {
    final Map<String, Object> decision = object(value);
    assertSemanticKeys(
        decision,
        "decision_id",
        "action_digest",
        "enacted_at_height",
        "execute_after_height");
    return new GovernanceDecision(
        rawDigest(array(decision.get("decision_id"))),
        digest(decision.get("action_digest")),
        unsigned(decision.get("enacted_at_height")),
        unsigned(decision.get("execute_after_height")));
  }

  private static ArchiveCommitment archiveCommitment(final Object value) {
    final Map<String, Object> commitment = object(value);
    assertSemanticKeys(
        commitment,
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
        "chunk_count");
    final Map<String, Object> chunker = object(commitment.get("chunker"));
    assertSemanticKeys(chunker, "profile_id", "namespace", "name", "semver", "multihash_code");
    return new ArchiveCommitment(
        rawBytes(array(commitment.get("root_cid")), 36),
        new ChunkerProfileHandle(
            exactInteger(chunker.get("profile_id")).longValueExact(),
            string(chunker.get("namespace")),
            string(chunker.get("name")),
            string(chunker.get("semver")),
            unsigned(chunker.get("multihash_code"))),
        digest(commitment.get("chunk_plan_digest")),
        digest(commitment.get("por_root")),
        unsigned(commitment.get("content_length")),
        digest(commitment.get("car_digest")),
        unsigned(commitment.get("car_size")),
        digest(commitment.get("bundle_digest")),
        digest(commitment.get("source_tree_digest")),
        digest(commitment.get("descriptor_digest")),
        exactInteger(commitment.get("file_count")).longValueExact(),
        exactInteger(commitment.get("chunk_count")).longValueExact());
  }

  private static SeedIngressReceipt seedIngressReceipt(final Object value) {
    final Map<String, Object> receipt = object(value);
    assertSemanticKeys(receipt, "payload", "approvals");
    final Map<String, Object> payload = object(receipt.get("payload"));
    assertSemanticKeys(payload, "version", "binding", "issued_at_ms", "expires_at_ms");
    assertEquals(1, exactInteger(payload.get("version")).intValueExact());
    final Map<String, Object> binding = object(payload.get("binding"));
    assertSemanticKeys(
        binding,
        "network_id",
        "publisher",
        "ingress_broker",
        "seed_provider",
        "semantic_release_manifest_digest",
        "archive_id",
        "car_body_digest",
        "car_body_length",
        "nonce");
    final SeedIngressReceiptBinding bindingValue = new SeedIngressReceiptBinding(
        NetworkId.parse(string(binding.get("network_id"))),
        string(binding.get("publisher")),
        string(binding.get("ingress_broker")),
        newtypeText(binding.get("seed_provider")),
        digest(binding.get("semantic_release_manifest_digest")),
        digest(binding.get("archive_id")),
        digest(binding.get("car_body_digest")),
        unsigned(binding.get("car_body_length")),
        fixed32(binding.get("nonce")));
    final List<SeedIngressReceiptApproval> approvals = new ArrayList<>();
    for (final Object approvalValue : array(receipt.get("approvals"))) {
      final Map<String, Object> approval = object(approvalValue);
      assertSemanticKeys(approval, "public_key", "signature");
      approvals.add(new SeedIngressReceiptApproval(
          string(approval.get("public_key")), string(approval.get("signature"))));
    }
    return new SeedIngressReceipt(
        new SeedIngressReceiptPayload(
            bindingValue,
            unsigned(payload.get("issued_at_ms")),
            unsigned(payload.get("expires_at_ms"))),
        approvals);
  }

  private static ProviderBundleVerificationAttestation providerAttestation(final Object value) {
    final Map<String, Object> attestation = object(value);
    assertSemanticKeys(attestation, "payload", "approvals");
    final Map<String, Object> payload = object(attestation.get("payload"));
    assertSemanticKeys(payload, "version", "binding");
    assertEquals(1, exactInteger(payload.get("version")).intValueExact());
    final Map<String, Object> binding = object(payload.get("binding"));
    assertSemanticKeys(
        binding,
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
        "source_tree_digest");
    final Map<String, Object> authority = object(binding.get("completion_authority"));
    assertSemanticKeys(authority, "provider_owner", "signer_policy");
    final Map<String, Object> signerPolicy = object(authority.get("signer_policy"));
    assertSemanticKeys(
        signerPolicy, "policy_id", "revision", "predecessor_digest", "policy_digest");
    final Object predecessor = signerPolicy.get("predecessor_digest");
    final ProviderCompletionSignerPolicy policy = new ProviderCompletionSignerPolicy(
        fixed32(signerPolicy.get("policy_id")),
        unsigned(signerPolicy.get("revision")),
        predecessor == null ? null : fixed32(predecessor),
        fixed32(signerPolicy.get("policy_digest")));
    final Map<String, Object> anchor = object(binding.get("finalized_anchor"));
    assertSemanticKeys(anchor, "height", "block_hash");
    final ProviderBundleVerificationBinding bindingValue =
        new ProviderBundleVerificationBinding(
            NetworkId.parse(string(binding.get("network_id"))),
            newtypeText(binding.get("provider_id")),
            string(binding.get("completed_by")),
            new ProviderCompletionAuthority(string(authority.get("provider_owner")), policy),
            digest(binding.get("replication_order")),
            unsigned(binding.get("assignment_revision")),
            unsigned(binding.get("completion_epoch")),
            new ProviderFinalizedAnchor(
                unsigned(anchor.get("height")), fixed32(anchor.get("block_hash"))),
            digest(binding.get("archive_id")),
            digest(binding.get("bundle_digest")),
            digest(binding.get("descriptor_digest")),
            digest(binding.get("semantic_release_manifest_digest")),
            digest(binding.get("verification_lock_digest")),
            digest(binding.get("source_tree_digest")));
    final List<ProviderBundleVerificationApproval> approvals = new ArrayList<>();
    for (final Object approvalValue : array(attestation.get("approvals"))) {
      final Map<String, Object> approval = object(approvalValue);
      assertSemanticKeys(approval, "public_key", "signature");
      approvals.add(new ProviderBundleVerificationApproval(
          string(approval.get("public_key")), string(approval.get("signature"))));
    }
    return new ProviderBundleVerificationAttestation(
        new ProviderBundleVerificationPayload(bindingValue), approvals);
  }

  private static ProviderBundleVerificationBinding providerBindingWithAccounts(
      final ProviderBundleVerificationBinding template,
      final String completedBy,
      final ProviderCompletionAuthority completionAuthority) {
    return new ProviderBundleVerificationBinding(
        template.networkId(),
        template.providerId(),
        completedBy,
        completionAuthority,
        template.replicationOrder(),
        template.assignmentRevision(),
        template.completionEpoch(),
        template.finalizedAnchor(),
        template.archiveId(),
        template.bundleDigest(),
        template.descriptorDigest(),
        template.semanticReleaseManifestDigest(),
        template.verificationLockDigest(),
        template.sourceTreeDigest());
  }

  private static PackageId packageId(final Object value) {
    final Map<String, Object> packageValue = object(value);
    assertSemanticKeys(packageValue, "home_dataspace", "scope", "name");
    return new PackageId(
        unsigned(packageValue.get("home_dataspace")),
        packageScope(packageValue.get("scope")),
        new PackageName(newtypeText(packageValue.get("name"))));
  }

  private static NamespaceBinding namespaceBinding(final Object value) {
    final Map<String, Object> binding = object(value);
    assertSemanticKeys(binding, "namespace", "home_dataspace", "scope", "generation");
    return new NamespaceBinding(
        new Namespace(newtypeText(binding.get("namespace"))),
        unsigned(binding.get("home_dataspace")),
        packageScope(binding.get("scope")),
        unsigned(binding.get("generation")));
  }

  private static PackageScope packageScope(final Object value) {
    final Map<String, Object> scopeValue = object(value);
    assertSemanticKeys(scopeValue, "kind", "value");
    final String scopeKind = string(scopeValue.get("kind"));
    if ("DataspaceRoot".equals(scopeKind)) {
      assertEquals(null, scopeValue.get("value"));
      return PackageScope.dataspaceRoot();
    }
    if ("Domain".equals(scopeKind)) {
      return PackageScope.domain(string(scopeValue.get("value")));
    }
    throw new AssertionError("unsupported Musubi package scope: " + scopeKind);
  }

  private static PackageRole packageRole(final Object value) {
    final Map<String, Object> role = object(value);
    assertSemanticKeys(role, "kind", "value");
    final String kind = string(role.get("kind"));
    if ("Owner".equals(kind)) {
      assertEquals(null, role.get("value"));
      return PackageRole.owner();
    }
    if ("Maintainer".equals(kind)) {
      final Map<String, Object> permissions = object(role.get("value"));
      assertSemanticKeys(
          permissions, "publish", "yank", "metadata", "archive_locations");
      return PackageRole.maintainer(
          new MaintainerPermissions(
              bool(permissions.get("publish")),
              bool(permissions.get("yank")),
              bool(permissions.get("metadata")),
              bool(permissions.get("archive_locations"))));
    }
    throw new AssertionError("unsupported Musubi package role: " + kind);
  }

  private static ReleaseId releaseId(final Object value) {
    final Map<String, Object> release = object(value);
    assertSemanticKeys(release, "package", "version");
    return new ReleaseId(packageId(release.get("package")), version(release.get("version")));
  }

  private static Version version(final Object value) {
    final Map<String, Object> version = object(value);
    assertSemanticKeys(version, "major", "minor", "patch", "prerelease");
    final List<PrereleaseIdentifier> prerelease = new ArrayList<>();
    for (final Object rawIdentifier : array(version.get("prerelease"))) {
      final Map<String, Object> identifier = object(rawIdentifier);
      assertSemanticKeys(identifier, "kind", "value");
      if ("Numeric".equals(identifier.get("kind"))) {
        prerelease.add(PrereleaseIdentifier.numeric(unsigned(identifier.get("value"))));
      } else if ("AlphaNumeric".equals(identifier.get("kind"))) {
        prerelease.add(PrereleaseIdentifier.alphaNumeric(string(identifier.get("value"))));
      } else {
        throw new AssertionError("unsupported Musubi prerelease fixture variant");
      }
    }
    return new Version(
        unsigned(version.get("major")),
        unsigned(version.get("minor")),
        unsigned(version.get("patch")),
        prerelease);
  }

  private static Publication publication(final Object value) {
    final Map<String, Object> publication = object(value);
    assertSemanticKeys(publication, "manifest", "resolution");
    return new Publication(
        releaseManifest(publication.get("manifest")),
        resolutionProof(publication.get("resolution")));
  }

  private static ReleaseManifest releaseManifest(final Object value) {
    final Map<String, Object> manifest = object(value);
    assertSemanticKeys(
        manifest,
        "release",
        "edition",
        "abi",
        "dependencies",
        "exports",
        "interface_digest",
        "metadata",
        "archive_id",
        "verification_lock_digest");
    final Map<String, Object> edition = object(manifest.get("edition"));
    assertSemanticKeys(edition, "kind", "value");
    assertEquals("V1", edition.get("kind"));
    assertEquals(null, edition.get("value"));
    final List<DependencyRequirement> dependencies = new ArrayList<>();
    for (final Object dependency : array(manifest.get("dependencies"))) {
      dependencies.add(dependencyRequirement(dependency));
    }
    final List<String> exports = new ArrayList<>();
    for (final Object export : array(manifest.get("exports"))) exports.add(string(export));
    return new ReleaseManifest(
        releaseId(manifest.get("release")),
        abiBinding(manifest.get("abi")),
        dependencies,
        exports,
        digest(manifest.get("interface_digest")),
        releaseMetadata(manifest.get("metadata")),
        digest(manifest.get("archive_id")),
        digest(manifest.get("verification_lock_digest")));
  }

  private static MusubiModelsV1.AbiBinding abiBinding(final Object value) {
    final Map<String, Object> abi = object(value);
    assertSemanticKeys(abi, "abi_version", "abi_hash");
    assertEquals(1, exactInteger(abi.get("abi_version")).intValueExact());
    return new MusubiModelsV1.AbiBinding(fixed32(abi.get("abi_hash")));
  }

  private static DependencyRequirement dependencyRequirement(final Object value) {
    final Map<String, Object> dependency = object(value);
    assertSemanticKeys(dependency, "alias", "package", "requirement");
    return new DependencyRequirement(
        string(dependency.get("alias")),
        packageId(dependency.get("package")),
        versionRequirement(dependency.get("requirement")));
  }

  private static VersionReq versionRequirement(final Object value) {
    final Map<String, Object> requirement = object(value);
    assertSemanticKeys(requirement, "kind", "value");
    final String kind = string(requirement.get("kind"));
    if ("Any".equals(kind)) return VersionReq.parse("*");
    if ("Caret".equals(kind)) return VersionReq.parse("^" + versionText(requirement.get("value")));
    if ("Tilde".equals(kind)) return VersionReq.parse("~" + versionText(requirement.get("value")));
    if ("MajorWildcard".equals(kind)) {
      return VersionReq.parse(unsigned(requirement.get("value")) + ".*");
    }
    if ("MinorWildcard".equals(kind)) {
      final Map<String, Object> wildcard = object(requirement.get("value"));
      assertSemanticKeys(wildcard, "major", "minor");
      return VersionReq.parse(
          unsigned(wildcard.get("major")) + "." + unsigned(wildcard.get("minor")) + ".*");
    }
    if ("Exact".equals(kind)) return VersionReq.parse("=" + versionText(requirement.get("value")));
    if ("Comparators".equals(kind)) {
      final StringBuilder text = new StringBuilder();
      for (final Object comparatorValue : array(requirement.get("value"))) {
        final Map<String, Object> comparator = object(comparatorValue);
        assertSemanticKeys(comparator, "op", "version");
        final Map<String, Object> op = object(comparator.get("op"));
        assertSemanticKeys(op, "kind", "value");
        assertEquals(null, op.get("value"));
        if (text.length() > 0) text.append(',');
        final String operator = string(op.get("kind"));
        if ("Greater".equals(operator)) text.append('>');
        else if ("GreaterOrEqual".equals(operator)) text.append(">=");
        else if ("Less".equals(operator)) text.append('<');
        else if ("LessOrEqual".equals(operator)) text.append("<=");
        else if ("Equal".equals(operator)) text.append('=');
        else throw new AssertionError("unsupported comparator operator: " + operator);
        text.append(versionText(comparator.get("version")));
      }
      return VersionReq.parse(text.toString());
    }
    throw new AssertionError("unsupported Musubi requirement: " + kind);
  }

  private static String versionText(final Object value) {
    return version(value).canonicalText();
  }

  private static ResolutionProof resolutionProof(final Object value) {
    final Map<String, Object> resolution = object(value);
    assertSemanticKeys(resolution, "snapshot", "lock");
    final Map<String, Object> snapshot = object(resolution.get("snapshot"));
    assertSemanticKeys(snapshot, "finalized_height", "finalized_block_hash", "index_revision");
    return new ResolutionProof(
        new RegistrySnapshot(
            unsigned(snapshot.get("finalized_height")),
            fixed32(snapshot.get("finalized_block_hash")),
            unsigned(snapshot.get("index_revision"))),
        verificationLock(resolution.get("lock")));
  }

  private static VerificationLock verificationLock(final Object value) {
    final Map<String, Object> lock = object(value);
    assertSemanticKeys(lock, "schema", "version", "root", "root_dependencies", "nodes");
    assertEquals("musubi-verification-lock", lock.get("schema"));
    assertEquals(1, exactInteger(lock.get("version")).intValueExact());
    final List<ExactDependencyEdge> rootDependencies = new ArrayList<>();
    for (final Object edge : array(lock.get("root_dependencies"))) {
      rootDependencies.add(exactDependencyEdge(edge));
    }
    final List<VerificationNode> nodes = new ArrayList<>();
    for (final Object node : array(lock.get("nodes"))) nodes.add(verificationNode(node));
    return new VerificationLock(releaseId(lock.get("root")), rootDependencies, nodes);
  }

  private static ExactDependencyEdge exactDependencyEdge(final Object value) {
    final Map<String, Object> edge = object(value);
    assertSemanticKeys(edge, "alias", "kind", "package", "requirement", "selected");
    final Map<String, Object> kind = object(edge.get("kind"));
    assertSemanticKeys(kind, "kind", "value");
    assertEquals(null, kind.get("value"));
    final DependencyKind dependencyKind;
    if ("Normal".equals(kind.get("kind"))) dependencyKind = DependencyKind.NORMAL;
    else if ("Development".equals(kind.get("kind"))) dependencyKind = DependencyKind.DEVELOPMENT;
    else throw new AssertionError("unsupported Musubi dependency kind");
    return new ExactDependencyEdge(
        string(edge.get("alias")),
        dependencyKind,
        packageId(edge.get("package")),
        versionRequirement(edge.get("requirement")),
        releaseId(edge.get("selected")));
  }

  private static VerificationNode verificationNode(final Object value) {
    final Map<String, Object> node = object(value);
    assertSemanticKeys(
        node,
        "release",
        "release_digest",
        "archive_id",
        "source_digest",
        "interface_digest",
        "abi",
        "dependencies");
    final List<ExactDependencyEdge> dependencies = new ArrayList<>();
    for (final Object dependency : array(node.get("dependencies"))) {
      dependencies.add(exactDependencyEdge(dependency));
    }
    return new VerificationNode(
        releaseId(node.get("release")),
        digest(node.get("release_digest")),
        digest(node.get("archive_id")),
        digest(node.get("source_digest")),
        digest(node.get("interface_digest")),
        abiBinding(node.get("abi")),
        dependencies);
  }

  private static ReleaseMetadata releaseMetadata(final Object value) {
    final Map<String, Object> metadata = object(value);
    assertSemanticKeys(metadata, "description", "readme", "license", "repository", "keywords");
    final List<String> keywords = new ArrayList<>();
    for (final Object keyword : array(metadata.get("keywords"))) {
      keywords.add(newtypeText(keyword));
    }
    return new ReleaseMetadata(
        optionalNewtypeText(metadata.get("description")),
        optionalNewtypeText(metadata.get("readme")),
        optionalNewtypeText(metadata.get("license")),
        optionalNewtypeText(metadata.get("repository")),
        keywords);
  }

  private static NamespaceDelegation namespaceDelegation(final Object value) {
    if (value == null) return null;
    final Map<String, Object> delegation = object(value);
    assertSemanticKeys(delegation, "payload", "approvals");
    final Map<String, Object> payload = object(delegation.get("payload"));
    assertSemanticKeys(
        payload,
        "version",
        "namespace_binding",
        "owner_generation",
        "owner",
        "delegate",
        "expires_at_height");
    assertEquals(1, exactInteger(payload.get("version")).intValueExact());
    final List<NamespaceDelegationApproval> approvals = new ArrayList<>();
    for (final Object approvalValue : array(delegation.get("approvals"))) {
      final Map<String, Object> approval = object(approvalValue);
      assertSemanticKeys(approval, "public_key", "signature");
      approvals.add(new NamespaceDelegationApproval(
          string(approval.get("public_key")), string(approval.get("signature"))));
    }
    return new NamespaceDelegation(
        new NamespaceDelegationPayload(
            digest(payload.get("namespace_binding")),
            unsigned(payload.get("owner_generation")),
            string(payload.get("owner")),
            string(payload.get("delegate")),
            unsigned(payload.get("expires_at_height"))),
        approvals);
  }

  private static RegistryPolicy registryPolicy(final Object value) {
    final Map<String, Object> policy = object(value);
    assertSemanticKeys(policy, "version", "revision", "mode", "allowlisted_dataspaces", "alias_pricing");
    assertEquals(1, exactInteger(policy.get("version")).intValueExact());
    final Map<String, Object> mode = object(policy.get("mode"));
    assertSemanticKeys(mode, "kind", "value");
    assertEquals(null, mode.get("value"));
    final RegistryAdmissionMode admissionMode;
    if ("Closed".equals(mode.get("kind"))) admissionMode = RegistryAdmissionMode.CLOSED;
    else if ("Allowlisted".equals(mode.get("kind"))) {
      admissionMode = RegistryAdmissionMode.ALLOWLISTED;
    } else if ("Open".equals(mode.get("kind"))) admissionMode = RegistryAdmissionMode.OPEN;
    else throw new AssertionError("unsupported Musubi admission mode");
    final List<BigInteger> dataspaces = new ArrayList<>();
    for (final Object dataspace : array(policy.get("allowlisted_dataspaces"))) {
      dataspaces.add(unsigned(dataspace));
    }
    final Map<String, Object> pricing = object(policy.get("alias_pricing"));
    assertSemanticKeys(
        pricing,
        "revision",
        "length_1_xor",
        "length_2_xor",
        "length_3_xor",
        "length_4_xor",
        "length_5_to_32_xor");
    return new RegistryPolicy(
        unsigned(policy.get("revision")),
        admissionMode,
        dataspaces,
        new AliasPricingPolicy(
            unsigned(pricing.get("revision")),
            unsigned(pricing.get("length_1_xor")),
            unsigned(pricing.get("length_2_xor")),
            unsigned(pricing.get("length_3_xor")),
            unsigned(pricing.get("length_4_xor")),
            unsigned(pricing.get("length_5_to_32_xor"))));
  }

  private static Digest32 digest(final Object value) {
    final List<Object> wrapper = array(value);
    assertEquals(1, wrapper.size());
    return rawDigest(array(wrapper.get(0)));
  }

  private static Digest32 rawDigest(final List<Object> octets) {
    assertEquals(32, octets.size());
    final byte[] bytes = new byte[octets.size()];
    for (int index = 0; index < bytes.length; index++) {
      final BigInteger octet = exactInteger(octets.get(index));
      if (octet.signum() < 0 || octet.compareTo(BigInteger.valueOf(255)) > 0) {
        throw new AssertionError("digest fixture octet is outside u8");
      }
      bytes[index] = octet.byteValue();
    }
    return Digest32.fromBytes(bytes);
  }

  private static byte[] fixed32(final Object value) {
    return rawBytes(array(value), 32);
  }

  private static byte[] rawBytes(final List<Object> octets, final int expectedLength) {
    assertEquals(expectedLength, octets.size());
    final byte[] bytes = new byte[octets.size()];
    for (int index = 0; index < bytes.length; index++) {
      final BigInteger octet = exactInteger(octets.get(index));
      if (octet.signum() < 0 || octet.compareTo(BigInteger.valueOf(255)) > 0) {
        throw new AssertionError("fixture octet is outside u8");
      }
      bytes[index] = octet.byteValue();
    }
    return bytes;
  }

  private static void assertCanonicalBoundedInstructionPair(
      final byte[] pair, final String expectedWireId, final byte[] expectedFrame) {
    final Cursor cursor = new Cursor(pair);
    final int nameFieldLength = cursor.readCompactLength();
    final Cursor nameField = cursor.readBounded(nameFieldLength);
    final int nameLength = nameField.readCompactLength();
    assertEquals(expectedWireId, new String(nameField.readBytes(nameLength), StandardCharsets.UTF_8));
    assertEquals(0, nameField.remaining());

    final int payloadFieldLength = cursor.readCompactLength();
    final Cursor payloadField = cursor.readBounded(payloadFieldLength);
    final long payloadLength = payloadField.readUnsignedU64AsLong();
    assertTrue(
        "fixture frame length must fit the Java array bound",
        payloadLength >= 0 && payloadLength <= Integer.MAX_VALUE);
    assertArrayEquals(expectedFrame, payloadField.readBytes((int) payloadLength));
    assertEquals(0, payloadField.remaining());
    assertEquals(0, cursor.remaining());
  }

  private static Map<String, Object> fixture() throws Exception {
    final Path path = findFixture();
    final String json = new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
    return object(JsonParser.parse(json));
  }

  private static Map<String, Object> fixtureCase(final String id) throws Exception {
    for (final Object value : array(fixture().get("cases"))) {
      final Map<String, Object> fixtureCase = object(value);
      if (id.equals(fixtureCase.get("id"))) return fixtureCase;
    }
    throw new AssertionError("missing Musubi fixture case: " + id);
  }

  private static NoritoDecoder canonicalDecoder(final byte[] payload) {
    return new NoritoDecoder(
        payload, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
  }

  private static byte[] readField(final NoritoDecoder decoder, final String field) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length < 0 || length > Integer.MAX_VALUE) {
      throw new AssertionError(field + " exceeds the Java fixture bound");
    }
    return decoder.readBytes((int) length);
  }

  private static Path findFixture() {
    Path current = Paths.get("").toAbsolutePath().normalize();
    for (int index = 0; index < 8 && current != null; index++) {
      final Path candidate = current.resolve(FIXTURE_PATH);
      if (Files.isRegularFile(candidate)) {
        return candidate;
      }
      current = current.getParent();
    }
    throw new AssertionError(FIXTURE_PATH + " was not found");
  }

  private static BigInteger unsigned(final Object value) {
    final BigInteger integer = exactInteger(value);
    if (integer.signum() < 0 || integer.compareTo(MusubiModelsV1.U64_MAX) > 0) {
      throw new AssertionError("fixture unsigned integer is outside u64");
    }
    return integer;
  }

  private static BigInteger optionalUnsigned(final Object value) {
    return value == null ? null : unsigned(value);
  }

  private static BigInteger exactInteger(final Object value) {
    final Number number = number(value);
    try {
      return new BigInteger(number.toString());
    } catch (final NumberFormatException error) {
      throw new AssertionError("fixture value must be an exact integer", error);
    }
  }

  private static Number number(final Object value) {
    if (!(value instanceof Number)) {
      throw new AssertionError("fixture value must be numeric");
    }
    return (Number) value;
  }

  private static boolean bool(final Object value) {
    if (!(value instanceof Boolean)) {
      throw new AssertionError("fixture value must be boolean");
    }
    return (Boolean) value;
  }

  private static String newtypeText(final Object value) {
    final List<Object> wrapper = array(value);
    assertEquals(1, wrapper.size());
    return string(wrapper.get(0));
  }

  private static String optionalNewtypeText(final Object value) {
    return value == null ? null : newtypeText(value);
  }

  private static String string(final Object value) {
    if (!(value instanceof String)) {
      throw new AssertionError("fixture value must be a string");
    }
    return (String) value;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value) {
    if (!(value instanceof Map)) {
      throw new AssertionError("fixture value must be an object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> array(final Object value) {
    if (!(value instanceof List)) {
      throw new AssertionError("fixture value must be an array");
    }
    return (List<Object>) value;
  }

  private static byte[] decodeHex(final String value) {
    if ((value.length() & 1) != 0) {
      throw new AssertionError("fixture hex has odd length");
    }
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] =
          (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static String repeat(final char value, final int count) {
    return new String(new char[count]).replace('\0', value);
  }

  private static byte[] repeatedBytes(final byte value) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, value);
    return bytes;
  }

  private static void assertUniqueAliasFailure(final Runnable constructor) {
    final IllegalArgumentException error =
        assertThrows(IllegalArgumentException.class, constructor::run);
    assertTrue(error.getMessage().contains("unique parent-local aliases"));
  }

  private static final class Cursor {
    private final byte[] bytes;
    private int offset;

    private Cursor(final byte[] bytes) {
      this.bytes = bytes;
    }

    private int remaining() {
      return bytes.length - offset;
    }

    private int readCompactLength() {
      long value = 0L;
      int shift = 0;
      for (int index = 0; index < 10; index++) {
        final int octet = readByte();
        if (index == 9 && (octet & 0xfe) != 0) {
          throw new AssertionError("fixture compact length overflows u64");
        }
        value |= (long) (octet & 0x7f) << shift;
        if ((octet & 0x80) == 0) {
          if (value > Integer.MAX_VALUE) {
            throw new AssertionError("fixture compact length exceeds Java bounds");
          }
          return (int) value;
        }
        shift += 7;
      }
      throw new AssertionError("fixture compact length is unterminated");
    }

    private long readUnsignedU64AsLong() {
      if (remaining() < 8) {
        throw new AssertionError("fixture u64 is truncated");
      }
      long value = 0L;
      for (int index = 0; index < 8; index++) {
        value |= (long) readByte() << (index * 8);
      }
      return value;
    }

    private Cursor readBounded(final int length) {
      return new Cursor(readBytes(length));
    }

    private byte[] readBytes(final int length) {
      if (length < 0 || length > remaining()) {
        throw new AssertionError("fixture field exceeds its bounded pair payload");
      }
      final byte[] value = new byte[length];
      System.arraycopy(bytes, offset, value, 0, length);
      offset += length;
      return value;
    }

    private int readByte() {
      if (remaining() == 0) {
        throw new AssertionError("fixture pair is truncated");
      }
      return bytes[offset++] & 0xff;
    }
  }
}
