using System;
using System.Buffers.Binary;
using System.Linq;
using System.Reflection;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyNativeTests
{
    private static readonly string[] Expected =
    {
        "zk-ace-pq-authorization-v0",
        "anonymous-pgc-k-out-of-n-v1",
        "verange-transparent-range-v1",
        "iroha-zk-ams-v1",
        "vega-existing-credential-zk-v0",
        "iroha-zk-x509-stark-p256-v0",
        "iroha-jindo-polynomial-commitment-v0",
        "iroha-bootle-lantern-anoncred-v1",
        "orchard-halo2-actions-v1",
        "monero-fcmp-plus-plus-v1",
        "iroha-ivm-private-note-stark-v1",
        "pq-masp-stark-v0",
    };

    [Fact]
    public void ExactClosedRegistryIsStable()
    {
        Assert.Equal(21U, PrivacyNative.RequiredBridgeAbiVersion);
        Assert.Equal(12, PrivacyProtocolsV1.All.Count);
        Assert.Equal(Expected, PrivacyProtocolsV1.All.Select(value => value.CanonicalLabel()));
        for (var index = 0; index < Expected.Length; index++)
        {
            Assert.Equal(
                PrivacyProtocolsV1.All[index],
                PrivacyProtocolsV1.ParseCanonicalLabel(Expected[index]));
        }
    }

    [Theory]
    [InlineData("jindo-lattice-pcs-zk-v0")]
    [InlineData("sis-hints-anoncred-pq-v0")]
    [InlineData("silent-threshold-anoncred-v0")]
    [InlineData("zk-ams-recursive-admission-v0")]
    [InlineData("iroha-zk-ams-v1 ")]
    [InlineData("Iroha-Zk-Ams-V1")]
    [InlineData("")]
    [InlineData("unknown-privacy-protocol-v1")]
    public void AliasesAndNonCanonicalSpellingsAreRejected(string rejected)
    {
        Assert.Throws<ArgumentException>(
            () => PrivacyProtocolsV1.ParseCanonicalLabel(rejected));
    }

    [Fact]
    public void CapabilityArchiveValidationFailsClosed()
    {
        Assert.Throws<ArgumentNullException>(() => new PrivacyCapabilitiesArchive(null!));
        Assert.Throws<ArgumentException>(() => new PrivacyCapabilitiesArchive(new byte[39]));

        var badMagic = CapabilityArchive();
        badMagic[0] = (byte)'X';
        Assert.Throws<ArgumentException>(() => new PrivacyCapabilitiesArchive(badMagic));

        var badSchema = CapabilityArchive();
        badSchema[13] = 0x51;
        Assert.Throws<ArgumentException>(() => new PrivacyCapabilitiesArchive(badSchema));

        var badCrc = CapabilityArchive();
        badCrc[40] ^= 0x01;
        Assert.Throws<ArgumentException>(() => new PrivacyCapabilitiesArchive(badCrc));

        var badFlags = CapabilityArchive();
        badFlags[39] = 0x80;
        Assert.Throws<ArgumentException>(() => new PrivacyCapabilitiesArchive(badFlags));
    }

    [Fact]
    public void CapabilityArchiveIsDefensivelyCopied()
    {
        var source = CapabilityArchive();
        var archive = new PrivacyCapabilitiesArchive(source);
        source[0] = (byte)'X';
        Assert.Equal((byte)'N', archive.NoritoBytes[0]);
        var exposed = archive.NoritoBytes;
        exposed[0] = (byte)'X';
        Assert.Equal((byte)'N', archive.NoritoBytes[0]);
    }

    [Fact]
    public void RetiredGenericProofSurfaceIsAbsent()
    {
        var names = typeof(PrivacyNative)
            .GetMethods(BindingFlags.Public | BindingFlags.Static)
            .Select(method => method.Name)
            .ToArray();
        Assert.DoesNotContain(names, name => name.Contains("ProofRequest", StringComparison.Ordinal));
        Assert.DoesNotContain(names, name => name.Contains("BuildProof", StringComparison.Ordinal));
        Assert.DoesNotContain(names, name => name.Contains("VerifyProof", StringComparison.Ordinal));
    }

    private static byte[] CapabilityArchive()
    {
        var archive = new byte[41];
        "NRT0"u8.CopyTo(archive);
        Array.Fill(archive, (byte)0x50, 6, 16);
        BinaryPrimitives.WriteUInt64LittleEndian(archive.AsSpan(23, 8), 1);
        archive[40] = 1;
        BinaryPrimitives.WriteUInt64LittleEndian(
            archive.AsSpan(31, 8),
            PrivacyCapabilitiesArchive.Crc64(archive.AsSpan(40)));
        return archive;
    }
}
