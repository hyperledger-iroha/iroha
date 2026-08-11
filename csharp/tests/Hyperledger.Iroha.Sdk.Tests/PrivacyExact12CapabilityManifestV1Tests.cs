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
    public void CanonicalManifestValidationPreservesTheCommittedProjection()
    {
        var fixture = BuildFixture();
        var decoded = PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
            fixture.Manifest,
            fixture.Catalog);

        Assert.Equal(1U, decoded.Version);
        Assert.Equal(2UL, decoded.CommittedHeight);
        Assert.Equal(12, decoded.Protocols.Count);
        Assert.True(decoded.Protocols[0].IsNetworkAvailable);
        Assert.All(decoded.Protocols, row => Assert.True(row.LocalCompiledTupleMatches));
        Assert.Equal(
            PrivacyCapabilityReadinessV1.AvailableExperimental,
            decoded.Protocols[(int)PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0].Readiness);
        Assert.Equal(
            PrivacyCapabilityLimitationV1.MissingDistributionWideKnowledgeSoundnessEvidence,
            decoded.Protocols[(int)PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0].Limitation);
        Assert.Equal(ConsensusPolicy(), decoded.ConsensusPolicy);
        Assert.Equal(
            Option(ActivationForProfileZero(), present: true),
            decoded.Protocols[0].ActivationCanonicalBytes);
        var activationCopy = decoded.Protocols[0].ActivationCanonicalBytes;
        activationCopy[0] ^= 0xff;
        Assert.False(activationCopy.SequenceEqual(decoded.Protocols[0].ActivationCanonicalBytes));
        Assert.Equal(fixture.Manifest, decoded.CanonicalArchive);
    }

    [Fact]
    public void LocalCatalogTupleSubstitutionFailsClosed()
    {
        var fixture = BuildFixture();
        var mismatchedProfiles = fixture.CompiledProfiles.Select(static value => (byte[])value.Clone()).ToArray();
        mismatchedProfiles[0][^1] ^= 1;
        var mismatchedCatalog = BuildCatalog(mismatchedProfiles);

        Assert.Throws<PrivacyExact12CapabilityManifestCodecV1.LocalTupleMismatchException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                fixture.Manifest,
                mismatchedCatalog));
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
            rowZeroActivationState: PrivacyCapabilityActivationStateV1.Suspended);

        Assert.Throws<PrivacyExact12CapabilityManifestException>(() =>
            PrivacyExact12CapabilityManifestCodecV1.DecodeValidated(
                fixture.Manifest,
                fixture.Catalog));
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
        PrivacyCapabilityActivationStateV1 rowZeroActivationState =
            PrivacyCapabilityActivationStateV1.Active,
        uint maxActionsPerTransaction = 1,
        byte activationDigestByte = 0x31)
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
                profiles[index] = EnumValue(1, EnumValue(0));
            }
        }

        var rows = new byte[12][];
        for (var index = 0; index < rows.Length; index++)
        {
            var available = index is 0 or 6;
            var readiness = available
                ? EnumValue(index == 6 ? 1U : 0U)
                : EnumValue(2, EnumValue(0));
            var activation = index == 0
                ? Option(ActivationForProfileZero(activationDigestByte), present: true)
                : Option(Array.Empty<byte>(), present: false);
            var activationState = index == 0
                ? rowZeroActivationState
                : PrivacyCapabilityActivationStateV1.NotRegistered;
            var limitation = index == 6
                ? Option(EnumValue(0), present: true)
                : Option(Array.Empty<byte>(), present: false);
            rows[index] = Struct(
                EnumValue(checked((uint)index)),
                EnumValue(checked((uint)index)),
                EnumValue((uint)ExecutionModes[index]),
                Struct(new[] { FeatureMasks[index] }),
                profiles[index],
                readiness,
                EnumValue((uint)activationState),
                activation,
                limitation);
        }

        var catalog = BuildCatalog(profiles);
        var manifestWithZeroDigest = BuildManifestArchive(
            rows,
            new byte[32],
            maxActionsPerTransaction);
        var digest = ComputeManifestDigest(manifestWithZeroDigest);
        return new Fixture(
            BuildManifestArchive(rows, digest, maxActionsPerTransaction),
            catalog,
            profiles);
    }

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

    private static byte[] ActivationForProfileZero(byte digestByte = 0x31)
    {
        var digest = Struct(Enumerable.Repeat(digestByte, 32).ToArray());
        var lifecycle = EnumValue(1, Struct(U64(1), U64(2), U64(2)));
        return Struct(
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
            EnumValue(0));
    }

    private static byte[] BuildCatalog(IReadOnlyList<byte[]> profiles)
    {
        var rows = profiles.Select((profile, index) =>
            Struct(EnumValue(checked((uint)index)), profile)).ToArray();
        return NoritoCodec.Encode(
            PrivacyExact12CapabilityManifestCodecV1.CatalogSchemaName,
            Struct(U32(1), Sequence(rows)),
            NoritoCodec.CanonicalLayoutFlags);
    }

    private static byte[] ConsensusPolicy(uint maxActionsPerTransaction = 1)
    {
        var consensusLimits = Struct(
            U32(maxActionsPerTransaction),
            U32(2),
            U32(9 * 1024 * 1024),
            U32(9 * 1024 * 1024),
            U32(9 * 1024 * 1024),
            U32(18 * 1024 * 1024),
            U32(256 * 1024),
            U32(8),
            U32(8),
            U32(2_048));
        return Struct(
            consensusLimits,
            Option(Array.Empty<byte>(), present: false));
    }

    private static byte[] BuildManifestArchive(
        IReadOnlyList<byte[]> rows,
        byte[] digest,
        uint maxActionsPerTransaction)
    {
        return NoritoCodec.Encode(
            PrivacyExact12CapabilityManifestCodecV1.ManifestSchemaName,
            Struct(
                U32(1),
                U64(2),
                ConsensusPolicy(maxActionsPerTransaction),
                Sequence(rows),
                Struct(digest)),
            NoritoCodec.CanonicalLayoutFlags);
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

    private static byte[] U64(ulong value)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt64LittleEndian(value);
        return writer.ToArray();
    }

    private sealed record Fixture(byte[] Manifest, byte[] Catalog, byte[][] CompiledProfiles);
}
