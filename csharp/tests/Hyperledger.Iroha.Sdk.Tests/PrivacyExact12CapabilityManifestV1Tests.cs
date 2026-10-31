using System.Buffers.Binary;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Privacy;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyExact12CapabilityManifestV1Tests
{
    private static readonly PrivacyExecutionModeV1[] ExecutionModes =
    [
        PrivacyExecutionModeV1.AuthorizationAction,
        PrivacyExecutionModeV1.PaymentAction,
        PrivacyExecutionModeV1.Component,
        PrivacyExecutionModeV1.AdmissionAction,
        PrivacyExecutionModeV1.PresentationAction,
        PrivacyExecutionModeV1.PresentationAction,
        PrivacyExecutionModeV1.Component,
        PrivacyExecutionModeV1.PresentationAction,
        PrivacyExecutionModeV1.NoteAction,
        PrivacyExecutionModeV1.PaymentAction,
        PrivacyExecutionModeV1.NoteAction,
        PrivacyExecutionModeV1.NoteAction,
    ];

    private static readonly byte[] FeatureMasks =
        [0, 6, 1, 2, 2, 2, 0, 2, 7, 2, 7, 31];


    [Fact]
    public void RetiredQualificationFieldCannotReenterRuntimeManifest()
    {
        var fixture = BuildFixture(retiredQualificationField: true);
        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
    }

    [Fact]
    public void ManagedProjectionPreservesFieldsWithoutGrantingAuthenticatedOrigin()
    {
        var fixture = BuildFixture();
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
            fixture.Manifest,
            fixture.Catalog);

        Assert.Equal(1U, decoded.Version);
        Assert.Equal(3UL, decoded.CommittedHeight);
        Assert.Equal(NetworkId.FromBytes(Enumerable.Repeat((byte)0xd1, 32).ToArray()), decoded.NetworkId);
        Assert.Equal(12, decoded.Protocols.Count);
        Assert.True(decoded.Protocols[0].IsNetworkAvailable);
        Assert.All(decoded.Protocols, row => Assert.True(row.LocalCompiledTupleMatches));
        Assert.Equal(
            PrivacyCapabilityReadinessV1.Available,
            decoded.Protocols[(int)PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV1].Readiness);
        Assert.Null(decoded.Protocols[(int)PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV1].UnavailableReason);
        Assert.Equal(
            PrivacyCapabilityUnavailableReasonV1.CompiledProfile,
            decoded.Protocols[1].UnavailableReason);
        Assert.Equal(
            PrivacyCompiledProfileUnavailableReasonV1.EngineUnavailable,
            decoded.Protocols[1].CompiledProfileUnavailableReason);
        Assert.Equal(ConsensusPolicy(), decoded.ConsensusPolicy);
        Assert.Equal(
            Option(Array.Empty<byte>(), present: false),
            decoded.Protocols[0].PolicyCanonicalBytes);
        var policyCopy = decoded.Protocols[0].PolicyCanonicalBytes;
        policyCopy[0] ^= 0xff;
        Assert.False(policyCopy.SequenceEqual(decoded.Protocols[0].PolicyCanonicalBytes));
        Assert.Equal(fixture.Manifest, decoded.CanonicalArchive);
    }

    [Fact]
    public void ManifestRetainsExactNetworkAndRejectsOtherNetworkAdmission()
    {
        var fixture = BuildFixture();
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog);
        var expected = NetworkId.FromBytes(Enumerable.Repeat((byte)0xd1, 32).ToArray());
        Assert.Equal(expected, decoded.NetworkId);
        PrivacyExact12CapabilityManifestCodecV1.RequireManifestNetwork(decoded.NetworkId, expected);
        var other = NetworkId.FromBytes(Enumerable.Repeat((byte)0xd3, 32).ToArray());
        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.RequireManifestNetwork(decoded.NetworkId, other));
        var copy = decoded.NetworkId.ToBytes(); copy[0] ^= 1;
        Assert.Equal(expected, decoded.NetworkId);
    }

    [Fact]
    public void MissingManifestNetworkCannotAuthorizeConstruction()
    {
        var fixture = BuildFixture(networkId: Array.Empty<byte>());
        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
    }

    [Fact]
    public void RecomputedManifestDigestCannotHideMalformedOrMismatchedNetworkBytes()
    {
        var valid = Enumerable.Repeat((byte)0xd1, 32).ToArray();
        foreach (var network in new[] { Array.Empty<byte>(), valid[..31], valid.Concat(new byte[] { 0 }).ToArray(), Struct(valid), Enumerable.Repeat((byte)0xd0, 32).ToArray() })
        {
            var fixture = BuildFixture(networkId: network);
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
        }
        var foreign = BuildFixture(networkId: Enumerable.Repeat((byte)0xd3, 32).ToArray());
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(foreign.Manifest, foreign.Catalog);
        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.RequireManifestNetwork(decoded.NetworkId, NetworkId.FromBytes(valid)));
    }

    [Fact]
    public void PublicValidatorChecksCanonicalRuntimeDataWithoutMintingAnAuthenticatedOrigin()
    {
        // Structural model fixtures are not native catalog or Torii authority.
        var fixture = BuildFixture();
        Assert.Equal(PrivacyNative.IsAvailable()
            ? PrivacyExact12CapabilityManifestValidationStatusV1.Valid
            : PrivacyExact12CapabilityManifestValidationStatusV1.NativeUnavailable,
            PrivacyNative.ValidateExact12CapabilityManifestV1(fixture.Manifest));
        Assert.DoesNotContain(typeof(PrivacyExact12CapabilityManifestV1).GetConstructors(),
            constructor => constructor.IsPublic);
    }

    [Fact]
    public void OnlyTheSelectedRuntimeTupleControlsAdmission()
    {
        var fixture = BuildFixture();
        var profiles = fixture.CompiledProfiles.Select(static value => (byte[])value.Clone()).ToArray();
        // A different local X509 construction must not block the active ACE row.
        profiles[5] = EnumValue(1, EnumValue(0));
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, BuildCatalog(profiles));
        Assert.False(decoded.Protocols[5].LocalCompiledTupleMatches);
        Assert.True(decoded.Protocols[0].LocalCompiledTupleMatches);
        PrivacyExact12CapabilityManifestCodecV1.RequireSelectedRuntimeProfile(decoded.Protocols[0]);
        Assert.Throws<PrivacyCapabilityUnavailableException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.RequireSelectedRuntimeProfile(decoded.Protocols[5]));

        profiles[0] = AvailableProfile(0, 0, 0, 0x35, EnumValue(0));
        decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, BuildCatalog(profiles));
        Assert.False(decoded.Protocols[0].LocalCompiledTupleMatches);
        Assert.Throws<PrivacyExact12CapabilityManifestCodecV1.LocalTupleMismatchException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.RequireSelectedRuntimeProfile(decoded.Protocols[0]));
    }

    [Fact]
    public void LegacySnapshotSchemaCannotAuthorize()
    {
        var fixture = BuildFixture();
        var legacy = NoritoCodec.Encode(
            "iroha.privacy.capability-snapshot.v1",
            NoritoCodec.Decode(
                PrivacyExact12CapabilityManifestCodecV1.ManifestSchemaName,
                fixture.Manifest).Payload,
            NoritoCodec.CanonicalLayoutFlags);

        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                legacy,
                fixture.Catalog));
    }

    [Fact]
    public void RecomputedDigestCannotHideActivationProjectionDrift()
    {
        var fixture = BuildFixture(
            rowZeroReadiness: UnavailableReadiness(3));

        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                fixture.Manifest,
                fixture.Catalog));
    }

    [Fact]
    public void CompiledProtocolIsAvailableWithoutReleaseEvidenceOrRegistration()
    {
        var fixture = BuildFixture();
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog);
        Assert.Equal(PrivacyCapabilityReadinessV1.Available, decoded.Protocols[0].Readiness);
        Assert.Null(decoded.Protocols[0].UnavailableReason);
        Assert.True(decoded.Protocols[0].IsNetworkAvailable);
    }

    [Fact]
    public void RetiredQualificationReadinessReasonsAreRejected()
    {
        foreach (var tag in new[] { 1U, 2U, 3U, 4U, 5U, 6U })
        {
            var fixture = BuildFixture(rowZeroReadiness: UnavailableReadiness(tag));
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
        }
    }

    [Fact]
    public void RetiredGovernanceLifecycleRecordsCannotReenterOrdinaryPolicy()
    {
        foreach (var lifecycle in new[] {
            EnumValue(0, Struct(U64(1))),
            EnumValue(2, Struct(U64(1), U64(2), U64(3))),
            EnumValue(3, Struct(U64(1), Option(U64(2), true), U64(3))) })
        {
            var fixture = BuildFixture(rowZeroLifecycle: lifecycle);
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
        }
    }

    [Fact]
    public void PendingUpdatesAcceptNextBlockAndRejectInvalidSnapshotHeights()
    {
        (ulong Scheduled, ulong Effective, ulong Committed, bool Accepted)[] cases =
        [
            (3, 4, 3, true), (3, 5, 3, true),
            (ulong.MaxValue - 1, ulong.MaxValue, ulong.MaxValue - 1, true),
            (0, 4, 3, false), (3, 3, 3, false), (3, 2, 3, false),
            (4, 5, 3, false), (3, 4, 4, false),
            (ulong.MaxValue, ulong.MaxValue, ulong.MaxValue, false),
        ];
        foreach (var consensus in new[] { true, false })
        {
            foreach (var item in cases)
            {
                var next = consensus
                    ? Struct(U32(1), U32(2), U32(9 * 1024 * 1024), U32(9_699_840),
                        U32(9_699_840), U32(16_711_680), U32(256 * 1024),
                        U32(8), U32(8), U32(1_024))
                    : EnumValue(6, Struct(U32(3)));
                var pending = Struct(U64(item.Scheduled), U64(item.Effective), next);
                var fixture = BuildFixture(
                    committedHeight: item.Committed,
                    pendingConsensus: consensus ? pending : null,
                    pendingJindo: consensus ? null : pending);
                if (item.Accepted)
                {
                    var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                        fixture.Manifest, fixture.Catalog);
                    Assert.Equal(fixture.Manifest, decoded.CanonicalArchive);
                    Assert.True(decoded.Protocols[0].IsNetworkAvailable);
                }
                else
                {
                    Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                        PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                            fixture.Manifest, fixture.Catalog));
                }
            }
        }
    }

    [Fact]
    public void AvailableCompiledProfilesNeedNoPolicyRegistrationAtAnyCommittedHeight()
    {
        foreach (var height in new[] { 1UL, 3UL, ulong.MaxValue })
        {
            var fixture = BuildFixture(committedHeight: height);
            var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog);
            foreach (var index in new[] { 0, 6 })
            {
                Assert.True(decoded.Protocols[index].IsNetworkAvailable);
                Assert.Equal(Option(Array.Empty<byte>(), false), decoded.Protocols[index].PolicyCanonicalBytes);
                PrivacyExact12CapabilityManifestCodecV1.RequireSelectedRuntimeProfile(decoded.Protocols[index]);
            }
        }
    }

    [Fact]
    public void EveryRetiredLifecycleIncludingSameHeightActivationIsRejected()
    {
        foreach (var lifecycle in new[] {
            EnumValue(1, Struct(U64(1), U64(1), U64(1))),
            EnumValue(1, Struct(U64(1), U64(1), U64(3))),
            EnumValue(2, Struct(U64(1), U64(1), U64(2))),
            EnumValue(3, Struct(U64(1), Option(U64(1), true), U64(2))),
            EnumValue(3, Struct(U64(1), Option(Array.Empty<byte>(), false), U64(2))) })
        {
            var fixture = BuildFixture(rowZeroLifecycle: lifecycle);
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
        }
    }

    [Fact]
    public void ExplicitActivationRejectsRemovedScheduleAndInvalidHistory()
    {
        var cases = new[]
        {
            (Lifecycle: EnumValue(0, Struct(U64(1), U64(4))), Tag: 2U),
            (Lifecycle: EnumValue(0, Struct(U64(1), U64(301))), Tag: 2U),
            (Lifecycle: EnumValue(0, Struct()), Tag: 2U),
            (Lifecycle: EnumValue(0, Struct(U64(0))), Tag: 2U),
            (Lifecycle: EnumValue(0, Struct(U64(4))), Tag: 2U),
            (Lifecycle: EnumValue(1, Struct(U64(2), U64(1), U64(2))), Tag: 0U),
            (Lifecycle: EnumValue(1, Struct(U64(1), U64(2), U64(1))), Tag: 0U),
            (Lifecycle: EnumValue(1, Struct(U64(1), U64(4), U64(4))), Tag: 0U),
            (Lifecycle: EnumValue(1, Struct(U64(1), U64(1), U64(4))), Tag: 0U),
            (Lifecycle: EnumValue(2, Struct(U64(1), U64(1), U64(1))), Tag: 3U),
            (Lifecycle: EnumValue(3, Struct(U64(1), Option(U64(1), present: true), U64(1))), Tag: 4U),
            (Lifecycle: EnumValue(3, Struct(U64(1), Option(Array.Empty<byte>(), present: false), U64(1))), Tag: 4U),
        };
        foreach (var item in cases)
        {
            var fixture = BuildFixture(
                rowZeroReadiness: item.Tag == 0 ? EnumValue(0) : UnavailableReadiness(item.Tag),
                rowZeroLifecycle: item.Lifecycle);
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                    fixture.Manifest, fixture.Catalog));
        }
    }

    [Fact]
    public void LegacyNineFieldRowAndExperimentalAssuranceArchivesAreRejected()
    {
        var legacyRows = BuildFixture(useLegacyNineFieldRows: true);
        var legacyAssurance = BuildFixture(useLegacyAssurance: true);

        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                legacyRows.Manifest,
                legacyRows.Catalog));
        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                legacyAssurance.Manifest,
                legacyAssurance.Catalog));
    }

    [Fact]
    public void SelectedUnavailableExceptionPreservesJointSecurityReason()
    {
        var fixture = BuildFixture();
        var row = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog).Protocols[5];
        var error = Assert.Throws<PrivacyCapabilityUnavailableException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.RequireSelectedRuntimeProfile(row));
        Assert.Equal(PrivacyProtocolIdV1.IrohaZkX509StarkP256V1, error.ProtocolId);
        Assert.Equal(PrivacyCapabilityUnavailableReasonV1.CompiledProfile, error.Reason);
        Assert.Equal(PrivacyCompiledProfileUnavailableReasonV1.JointSecurityReductionIncomplete, error.CompiledProfileReason);
        Assert.Null(error.SchemaError);
        Assert.Contains("JointSecurityReductionIncomplete", error.Message);
    }

    [Fact]
    public void JointSecurityReductionFailureIsTypedUnitAndCannotClaimAvailability()
    {
        var fixture = BuildFixture();
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog);
        Assert.Equal(PrivacyCompiledProfileUnavailableReasonV1.JointSecurityReductionIncomplete,
            decoded.Protocols[5].CompiledProfileUnavailableReason);
        Assert.False(decoded.Protocols[5].IsNetworkAvailable);
        foreach (var invalid in new[] { EnumValue(3, new byte[] { 0 }), EnumValue(4) })
        {
            var malformed = BuildFixture(x509UnavailableReason: invalid);
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(malformed.Manifest, malformed.Catalog));
        }
        var forged = BuildFixture(x509AvailableReadiness: true);
        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(forged.Manifest, forged.Catalog));
    }

    [Fact]
    public void ReadinessSurfaceContainsOnlyTheFinalHardCutStates()
    {
        Assert.Equal(
            new[] { "Available", "Unavailable" },
            Enum.GetNames<PrivacyCapabilityReadinessV1>());
    }

    [Fact]
    public void RecomputedDigestCannotHideConsensusPolicyDrift()
    {
        var fixture = BuildFixture(maxActionsPerTransaction: 2);

        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                fixture.Manifest,
                fixture.Catalog));
    }

    [Fact]
    public void RecomputedDigestCannotHideActivationBindingDrift()
    {
        var fixture = BuildFixture(activationDigestByte: 0x32);

        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                fixture.Manifest,
                fixture.Catalog));
    }

    [Fact]
    public void OrdinaryPolicyIsOptionalEvenForUnavailableProfilesAndCannotCreateReadiness()
    {
        var policy = Struct(EnumValue(5), EnumValue(5), Option(Array.Empty<byte>(), false));
        var fixture = BuildFixture(x509Policy: policy);
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog);
        Assert.Equal(Option(policy, true), decoded.Protocols[5].PolicyCanonicalBytes);
        Assert.False(decoded.Protocols[5].IsNetworkAvailable);
        Assert.Equal(PrivacyCompiledProfileUnavailableReasonV1.JointSecurityReductionIncomplete, decoded.Protocols[5].CompiledProfileUnavailableReason);
        foreach (var malformed in new[] {
            Struct(EnumValue(6), EnumValue(5), Option(Array.Empty<byte>(), false)),
            Struct(EnumValue(5), EnumValue(6, Struct(U32(4))), Option(Array.Empty<byte>(), false)),
            Struct(EnumValue(5), EnumValue(5), Option(Array.Empty<byte>(), false), U64(1)) })
        {
            var invalid = BuildFixture(x509Policy: malformed);
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(invalid.Manifest, invalid.Catalog));
        }
    }

    [Theory]
    [InlineData(0U, false)]
    [InlineData(8U, true)]
    [InlineData(9U, false)]
    [InlineData(16U, false)]
    public void VeRangeOrdinaryPolicyUsesCanonicalEightCommitmentCeiling(uint count, bool accepted)
    {
        var policy = Struct(EnumValue(2), EnumValue(2, Struct(U32(count))), Option(Array.Empty<byte>(), false));
        var fixture = BuildFixture(verangePolicy: policy);
        if (accepted)
        {
            var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog);
            Assert.Equal(Option(policy, true), decoded.Protocols[2].PolicyCanonicalBytes);
            Assert.False(decoded.Protocols[2].IsNetworkAvailable);
        }
        else
        {
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
        }
    }

    [Fact]
    public void ProtocolUpdatesCanLoosenWithinHardBoundsButCannotBeNoopOrExceedBounds()
    {
        foreach (var next in new[] { 1U, 3U, 4U })
        {
            var fixture = BuildFixture(jindoCurrentLimit: 2, pendingJindo: Struct(U64(3), U64(4), EnumValue(6, Struct(U32(next)))));
            var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog);
            Assert.True(decoded.Protocols[6].IsNetworkAvailable);
        }
        foreach (var next in new[] { 0U, 2U, 5U })
        {
            var fixture = BuildFixture(jindoCurrentLimit: 2, pendingJindo: Struct(U64(3), U64(4), EnumValue(6, Struct(U32(next)))));
            Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
        }
    }

    [Fact]
    public void ConsensusUpdatesCanLoosenWithinHardBoundsButCannotBeNoopOrExceedBounds()
    {
        foreach (var next in new[] { 512U, 1536U, 2048U, 0U, 1024U, 2049U })
        {
            var limits = Struct(U32(1), U32(2), U32(9 * 1024 * 1024), U32(9_699_840), U32(9_699_840),
                U32(16_711_680), U32(256 * 1024), U32(8), U32(8), U32(next));
            var fixture = BuildFixture(retainedRootCount: 1024, pendingConsensus: Struct(U64(3), U64(4), limits));
            if (next is > 0 and <= 2048 && next != 1024)
                Assert.Equal(fixture.Manifest, PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog).CanonicalArchive);
            else Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
                PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(fixture.Manifest, fixture.Catalog));
        }
        Assert.Equal(new[] { "CompiledProfile" }, Enum.GetNames<PrivacyCapabilityUnavailableReasonV1>());
    }

    [Fact]
    public void PublicManifestAndAdmissionConstructorsAreClosed()
    {
        var manifestConstructor = Assert.Single(
            typeof(PrivacyExact12CapabilityManifestV1).GetConstructors(
                BindingFlags.NonPublic | BindingFlags.Instance));
        Assert.True(manifestConstructor.IsPrivate);
        var admissionConstructor = Assert.Single(
            typeof(PrivacyExact12CapabilityTupleAdmissionV1).GetConstructors(
                BindingFlags.NonPublic | BindingFlags.Instance));
        Assert.True(admissionConstructor.IsPrivate);
        Assert.DoesNotContain(
            typeof(PrivacyExact12CapabilityManifestCodecV1).GetMethods(
                BindingFlags.NonPublic | BindingFlags.Static),
            method => method.ReturnType == typeof(PrivacyExact12CapabilityManifestV1));

        var authenticatedFetch = Assert.Single(
            typeof(PrivacyExact12CapabilityManifestV1).GetMethods(
                BindingFlags.NonPublic | BindingFlags.Static),
            method => method.Name == "FetchAuthenticatedToriiAsync");
        Assert.Equal(
            new[] { typeof(ToriiClient), typeof(CancellationToken) },
            authenticatedFetch.GetParameters().Select(parameter => parameter.ParameterType));
        Assert.DoesNotContain(
            typeof(PrivacyExact12CapabilityAdmissionV1).GetMethods(
                BindingFlags.Public | BindingFlags.Static),
            method => method.GetParameters().Any(parameter =>
                parameter.ParameterType.Name.Contains(
                    "PrivacyCapabilitySnapshot",
                    StringComparison.Ordinal)));
    }

    private static Fixture BuildFixture(
        byte[]? rowZeroReadiness = null,
        byte[]? rowZeroLifecycle = null,
        ulong committedHeight = 3,
        uint maxActionsPerTransaction = 1,
        byte activationDigestByte = 0x31,
        bool useLegacyNineFieldRows = false,
        bool useLegacyAssurance = false,
        byte[]? networkId = null,
        bool retiredQualificationField = false,
        byte[]? x509UnavailableReason = null,
        bool x509AvailableReadiness = false,
        byte[]? pendingConsensus = null,
        byte[]? pendingJindo = null,
        byte[]? rowZeroPolicy = null, byte[]? x509Policy = null, uint jindoCurrentLimit = 4, uint retainedRootCount = 2048, byte[]? verangePolicy = null)
    {
        var profiles = new byte[12][];
        profiles[0] = AvailableProfile(
            protocol: 0,
            proofSystem: 0,
            engine: 0,
            digestByte: 0x31,
            limits: EnumValue(0));
        for (var index = 1; index < profiles.Length; index++)
        {
            if (index == 6)
            {
                profiles[index] = AvailableProfile(
                    protocol: 6,
                    proofSystem: 5,
                    engine: 5,
                    digestByte: 0x61,
                    limits: EnumValue(6, Struct(U32(4))));
            }
            else
            {
                profiles[index] = EnumValue(1, index == 5 ? x509UnavailableReason ?? EnumValue(3) : EnumValue(0));
            }
        }

        var rows = new byte[12][];
        for (var index = 0; index < rows.Length; index++)
        {
            var readiness = useLegacyNineFieldRows
                ? index switch
                {
                    0 => EnumValue(0),
                    6 => EnumValue(1),
                    _ => EnumValue(2, EnumValue(0)),
                }
                : index switch
                {
                    0 => rowZeroReadiness ?? EnumValue(0),
                    5 => x509AvailableReadiness ? EnumValue(0) : UnavailableReadiness(0, x509UnavailableReason ?? EnumValue(3)),
                    6 => EnumValue(0),
                    _ => UnavailableReadiness(0, EnumValue(0)),
                };
            var policy = index == 0
                ? rowZeroLifecycle is not null || activationDigestByte != 0x31 || useLegacyAssurance || useLegacyNineFieldRows
                    ? Option(RetiredActivationForProfileZero(activationDigestByte, useLegacyAssurance || useLegacyNineFieldRows, rowZeroLifecycle), true)
                    : rowZeroPolicy is null ? Option(Array.Empty<byte>(), false) : Option(rowZeroPolicy, true)
                : index == 6 && (pendingJindo is not null || jindoCurrentLimit != 4)
                    ? Option(Struct(EnumValue(6), EnumValue(6, Struct(U32(jindoCurrentLimit))), Option(pendingJindo ?? Array.Empty<byte>(), pendingJindo is not null)), true)
                : index == 5 && x509Policy is not null ? Option(x509Policy, true)
                : index == 2 && verangePolicy is not null ? Option(verangePolicy, true)
                : Option(Array.Empty<byte>(), false);
            var fields = new List<byte[]>
            {
                EnumValue(checked((uint)index)),
                EnumValue(checked((uint)index)),
                EnumValue((uint)ExecutionModes[index]),
                Struct(new[] { FeatureMasks[index] }),
                profiles[index],
                readiness,
            };
            if (useLegacyNineFieldRows)
            {
                fields.Add(EnumValue(index == 0 ? 2U : 0U));
                fields.Add(policy);
                fields.Add(index == 6
                    ? Option(EnumValue(0), present: true)
                    : Option(Array.Empty<byte>(), present: false));
            }
            else
            {
                fields.Add(policy);
            }
            rows[index] = Struct(fields.ToArray());
        }

        var catalog = BuildCatalog(profiles);
        var manifestWithZeroDigest = BuildManifestArchive(
            rows,
            networkId ?? Enumerable.Repeat((byte)0xd1, 32).ToArray(),
            retiredQualificationField,
            new byte[32],
            maxActionsPerTransaction,
            committedHeight,
            pendingConsensus, retainedRootCount);
        var digest = ComputeManifestDigest(manifestWithZeroDigest);
        return new Fixture(
            BuildManifestArchive(
                rows,
                networkId ?? Enumerable.Repeat((byte)0xd1, 32).ToArray(),
                retiredQualificationField,
                digest,
                maxActionsPerTransaction,
                committedHeight,
                pendingConsensus, retainedRootCount),
            catalog,
            profiles);
    }

    private static byte[] BuildCatalog(IReadOnlyList<byte[]> profiles) => NoritoCodec.Encode(
        PrivacyExact12CapabilityManifestCodecV1.CatalogSchemaName,
        Struct(U32(1), Sequence(profiles.Select((profile, index) => Struct(EnumValue((uint)index), profile)).ToArray())),
        NoritoCodec.CanonicalLayoutFlags);

    private static byte[] AvailableProfile(
        uint protocol,
        uint proofSystem,
        uint engine,
        byte digestByte,
        byte[] limits)
    {
        var digest = Struct(Enumerable.Repeat(digestByte, 32).Select(static value => (byte)value).ToArray());
        return EnumValue(
            0,
            Struct(
                EnumValue(protocol),
                EnumValue(proofSystem),
                EnumValue(engine),
                digest,
                digest,
                digest,
                digest,
                digest,
                limits));
    }

    private static byte[] RetiredActivationForProfileZero(
        byte digestByte = 0x31,
        bool useLegacyAssurance = false,
        byte[]? lifecycleOverride = null)
    {
        var digest = Struct(Enumerable.Repeat(digestByte, 32).ToArray());
        var lifecycle = lifecycleOverride ?? EnumValue(1, Struct(U64(1), U64(2), U64(2)));
        var fields = new List<byte[]>
        {
            EnumValue(0),
            EnumValue(0),
            EnumValue(0),
            digest,
            digest,
            digest,
            digest,
            digest,
            lifecycle,
            EnumValue(0),
            Option(Array.Empty<byte>(), present: false),
        };
        if (useLegacyAssurance)
        {
            fields.Add(EnumValue(0));
        }
        return Struct(fields.ToArray());
    }

    private static byte[] ConsensusPolicy(
        uint maxActionsPerTransaction = 1, byte[]? pending = null, uint retainedRootCount = 2048, byte[]? verangePolicy = null)
    {
        var consensusLimits = Struct(
            U32(maxActionsPerTransaction),
            U32(2),
            U32(9 * 1024 * 1024),
            U32(9_699_840),
            U32(9_699_840),
            U32(16_711_680),
            U32(256 * 1024),
            U32(8),
            U32(8),
            U32(retainedRootCount));
        return Struct(
            consensusLimits,
            Option(pending ?? Array.Empty<byte>(), present: pending is not null));
    }

    private static byte[] BuildManifestArchive(
        IReadOnlyList<byte[]> rows,
        byte[] networkId,
        bool retiredQualificationField,
        byte[] digest,
        uint maxActionsPerTransaction,
        ulong committedHeight,
        byte[]? pendingConsensus = null,
        uint retainedRootCount = 2048, byte[]? verangePolicy = null)
    {
        var fields = new List<byte[]> { U32(1), networkId, U64(committedHeight), ConsensusPolicy(maxActionsPerTransaction, pendingConsensus, retainedRootCount) };
        if (retiredQualificationField) fields.Add(Option(Array.Empty<byte>(), false));
        fields.Add(Sequence(rows)); fields.Add(Struct(digest));
        return NoritoCodec.Encode(PrivacyExact12CapabilityManifestCodecV1.ManifestSchemaName,
            Struct(fields.ToArray()), NoritoCodec.CanonicalLayoutFlags);
    }

    private static byte[] ComputeManifestDigest(byte[] normalizedArchive)
    {
        var domain = Encoding.UTF8.GetBytes(
            "iroha:privacy:exact12-capability-manifest:v1");
        Span<byte> length = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(length, checked((ulong)normalizedArchive.Length));
        var input = new byte[domain.Length + length.Length + normalizedArchive.Length];
        domain.CopyTo(input, 0);
        length.CopyTo(input.AsSpan(domain.Length));
        normalizedArchive.CopyTo(input, domain.Length + length.Length);
        return SHA256.HashData(input);
    }

    private static byte[] Struct(params byte[][] fields)
    {
        var writer = new CanonicalNoritoWriter();
        foreach (var field in fields)
        {
            writer.WriteField(field);
        }
        return writer.ToArray();
    }

    private static byte[] Sequence(IReadOnlyList<byte[]> values)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteSequenceLength(checked((ulong)values.Count));
        foreach (var value in values)
        {
            writer.WriteField(value);
        }
        return writer.ToArray();
    }

    private static byte[] EnumValue(uint tag, params byte[][] fields)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(tag);
        foreach (var field in fields)
        {
            writer.WriteField(field);
        }
        return writer.ToArray();
    }

    private static byte[] UnavailableReadiness(uint reasonTag, byte[]? detail = null) =>
        EnumValue(
            1,
            detail is null
                ? EnumValue(reasonTag)
                : EnumValue(reasonTag, detail));

    private static byte[] Option(byte[] value, bool present)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteByte(present ? (byte)1 : (byte)0);
        if (present)
        {
            writer.WriteField(value);
        }
        return writer.ToArray();
    }

    private static byte[] U32(uint value)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(value);
        return writer.ToArray();
    }

    private static byte[] U16(ushort value)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt16LittleEndian(value);
        return writer.ToArray();
    }

    private static byte[] U64(ulong value)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt64LittleEndian(value);
        return writer.ToArray();
    }

    private static byte[] CompactString(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        var writer = new CanonicalNoritoWriter();
        writer.WriteCompactLength(checked((ulong)bytes.Length));
        writer.WriteBytes(bytes);
        return writer.ToArray();
    }

    private sealed record Fixture(byte[] Manifest, byte[] Catalog, byte[][] CompiledProfiles);
}
