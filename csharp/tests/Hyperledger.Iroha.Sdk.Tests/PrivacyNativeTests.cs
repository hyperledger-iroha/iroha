using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyNativeTests
{
    private static readonly IReadOnlyList<string[]> Matrix = LoadExact12Matrix();
    private static readonly IReadOnlyList<string[]> ProtocolRows = Rows("protocol");
    private static readonly IReadOnlyList<string[]> TypedEnvelopeRows = Rows("typed-envelope");
    private static readonly string[] Retired = Rows("retired").Select(row => row[1]).ToArray();
    private static readonly string[] Expected = ProtocolRows.Select(row => row[2]).ToArray();

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

    [Fact]
    public void SharedExact12MatrixBindsRoutesAndTypedEnvelopeDigests()
    {
        Assert.True(
            new HashSet<string>(
                new[] { "matrix-version", "registry-sha256", "protocol", "typed-envelope", "retired" },
                StringComparer.Ordinal).SetEquals(Matrix.Select(row => row[0])));
        Assert.Equal(new[] { new[] { "matrix-version", "1" } }, Rows("matrix-version"));
        Assert.Equal(
            Enumerable.Range(0, 12).Select(index => index.ToString(CultureInfo.InvariantCulture)),
            ProtocolRows.Select(row => row[1]));
        Assert.Equal(12, Expected.Distinct(StringComparer.Ordinal).Count());
        var registryPreimage = string.Concat(Expected.Select(value => $"{value}\n"));
        var registryDigest = Convert.ToHexString(
            SHA256.HashData(Encoding.UTF8.GetBytes(registryPreimage))).ToLowerInvariant();
        Assert.Equal(
            new[] { new[] { "registry-sha256", registryDigest } },
            Rows("registry-sha256"));
        Assert.Equal(
            ProtocolRows.Select(row => row[2..5]),
            TypedEnvelopeRows.Select(row => row[1..4]));
        Assert.Equal(12, TypedEnvelopeRows.Count);
        foreach (var row in TypedEnvelopeRows)
        {
            Assert.Equal(6, row.Length);
            foreach (var digest in row[4..])
            {
                Assert.Matches("^[0-9a-f]{64}$", digest);
                Assert.NotEqual(new string('0', 64), digest);
            }
        }
        Assert.Equal(Retired.Length, Retired.Distinct(StringComparer.Ordinal).Count());
        Assert.Empty(Retired.Intersect(Expected, StringComparer.Ordinal));
    }

    [Theory]
    [InlineData("jindo-lattice-pcs-zk-v0")]
    [InlineData("sis-hints-anoncred-pq-v0")]
    [InlineData("sis-with-hints")]
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
    public void EveryRetiredMatrixProtocolIsRejected()
    {
        foreach (var retired in Retired)
        {
            Assert.Throws<ArgumentException>(
                () => PrivacyProtocolsV1.ParseCanonicalLabel(retired));
        }
    }

    [Fact]
    public void SharedTypedValidatorStatusContractIsStable()
    {
        Assert.Equal(256 * 1024, PrivacyNative.PrivacyNativeArchiveMaxBytes);
        Assert.Equal(
            Enumerable.Range(0, 9),
            Enum.GetValues<PrivacyCapabilityValidationStatusV1>().Select(value => (int)value));
        Assert.Empty(
            typeof(PrivacyCapabilitiesArchive).GetConstructors(
                BindingFlags.Public | BindingFlags.Instance));
        var validator = typeof(PrivacyNative).GetMethod(
            "NativeValidateCapabilities",
            BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(validator);
        Assert.NotNull(validator!.GetCustomAttribute<System.Runtime.InteropServices.DllImportAttribute>());
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

    private static IReadOnlyList<string[]> Rows(string kind)
    {
        return Matrix.Where(row => row[0] == kind).ToArray();
    }

    private static IReadOnlyList<string[]> LoadExact12Matrix()
    {
        var directory = new DirectoryInfo(Directory.GetCurrentDirectory());
        FileInfo? fixture = null;
        while (directory is not null)
        {
            var candidate = new FileInfo(
                Path.Combine(directory.FullName, "fixtures", "privacy", "exact12_v1.tsv"));
            if (candidate.Exists)
            {
                fixture = candidate;
                break;
            }
            directory = directory.Parent;
        }
        if (fixture is null)
        {
            throw new InvalidOperationException("cannot locate fixtures/privacy/exact12_v1.tsv");
        }
        var text = File.ReadAllText(fixture.FullName, Encoding.UTF8);
        if (!text.EndsWith('\n') || text.Contains('\r'))
        {
            throw new InvalidOperationException("exact12 fixture is not canonical LF text");
        }
        if (text[..^1].Split('\n', StringSplitOptions.None).Any(string.IsNullOrEmpty))
        {
            throw new InvalidOperationException("exact12 fixture contains an empty row");
        }
        return text
            .Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Where(line => !line.StartsWith('#'))
            .Select(line => line.Split('\t'))
            .ToArray();
    }
}
