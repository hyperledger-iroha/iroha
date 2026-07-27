using System;
using System.Collections.Generic;
using Hyperledger.Iroha.Zk;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class VerifyingKeyBackendTagTests
{
    [Fact]
    public void NoritoDiscriminantsMatchRustOrderExactly()
    {
        var expected = new[]
        {
            (VerifyingKeyBackendTag.Halo2IpaPasta, 0U),
            (VerifyingKeyBackendTag.Stark, 1U),
        };

        Assert.Equal(expected.Length, Enum.GetValues<VerifyingKeyBackendTag>().Length);
        foreach (var (tag, discriminant) in expected)
        {
            Assert.Equal(discriminant, tag.NoritoDiscriminant());
            Assert.Equal(tag, (VerifyingKeyBackendTag)discriminant);
        }
    }

    [Theory]
    [InlineData(VerifyingKeyBackendTag.Halo2IpaPasta, "halo2-ipa-pasta")]
    [InlineData(VerifyingKeyBackendTag.Stark, "stark")]
    public void CanonicalLabelsRoundTripExactly(
        VerifyingKeyBackendTag expected,
        string label)
    {
        Assert.Equal(label, expected.CanonicalLabel());
        Assert.True(VerifyingKeyBackendTags.TryFromCanonicalLabel(label, out var parsed));
        Assert.Equal(expected, parsed);
        Assert.Equal(expected, VerifyingKeyBackendTags.FromCanonicalLabel(label));
        Assert.Equal(label, VerifyingKeyBackendTags.RequireCanonicalLabel(label));
    }

    [Fact]
    public void UnknownEnumValuesCannotAcquireAStringOrWireDiscriminant()
    {
        var unknown = (VerifyingKeyBackendTag)2U;

        Assert.Throws<ArgumentOutOfRangeException>(() => unknown.CanonicalLabel());
        Assert.Throws<ArgumentOutOfRangeException>(() => unknown.NoritoDiscriminant());
    }

    [Theory]
    [MemberData(nameof(NonCanonicalLabels))]
    public void NonCanonicalAndRetiredLabelsAreRejected(string? label)
    {
        Assert.False(VerifyingKeyBackendTags.TryFromCanonicalLabel(label, out _));

        var parseError = Assert.Throws<ArgumentException>(
            () => VerifyingKeyBackendTags.FromCanonicalLabel(label, "proofBackend"));
        Assert.Equal("proofBackend", parseError.ParamName);

        var requireError = Assert.Throws<ArgumentException>(
            () => VerifyingKeyBackendTags.RequireCanonicalLabel(label, "proofBackend"));
        Assert.Equal("proofBackend", requireError.ParamName);
    }

    public static IEnumerable<object?[]> NonCanonicalLabels()
    {
        string?[] labels =
        [
            null,
            "",
            " ",
            "\t",
            "\n",
            " halo2-ipa-pasta",
            "halo2-ipa-pasta ",
            "HALO2-IPA-PASTA",
            "Halo2-Ipa-Pasta",
            "halo2/ipa",
            "halo2/pasta",
            "halo2/bn254",
            "groth16",
            "groth16/bls12-377",
            "stark ",
            "STARK",
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "halo2-ipa-orchard",
            "anonymous-pgc",
            "verange",
            "zkat",
            "silent-threshold-anoncred",
            "aztec-plonkish-private-kernel",
            "penumbra-masp",
            "unknown",
            "stark\0",
            "st\u0430rk",
            "halo2\uFF0Fipa",
            "stark\u200B",
        ];

        foreach (var label in labels)
        {
            yield return [label];
        }
    }

    [Theory]
    [InlineData("halo2/ipa")]
    [InlineData("halo2/pasta/kaigi-roster-v1")]
    [InlineData("halo2/pasta/kaigi-usage-v1")]
    [InlineData("halo2/pasta/ivm-overlay-bind")]
    [InlineData("halo2/pasta/ivm-execution-v1")]
    [InlineData("halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3")]
    [InlineData("halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2")]
    [InlineData("halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2")]
    [InlineData("halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3")]
    [InlineData("halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3")]
    [InlineData("halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4")]
    [InlineData("stark/fri")]
    [InlineData("stark/fri/sha256-goldilocks")]
    [InlineData("stark/fri/poseidon2-goldilocks")]
    [InlineData("stark/fri/sha256_goldilocks.v1")]
    public void VerifierRegistryAcceptsOnlyPinnedProfiles(string label)
    {
        Assert.True(VerifierBackendRegistryLabels.IsSupportedLabel(label));
        Assert.Equal(label, VerifierBackendRegistryLabels.RequireSupportedLabel(label));
    }

    [Theory]
    [MemberData(nameof(UnsupportedRegistryLabels))]
    public void VerifierRegistryRejectsAliasesAndRetiredProfiles(string? label)
    {
        Assert.False(VerifierBackendRegistryLabels.IsSupportedLabel(label));
        var error = Assert.Throws<ArgumentException>(
            () => VerifierBackendRegistryLabels.RequireSupportedLabel(
                label,
                "registryBackend"));
        Assert.Equal("registryBackend", error.ParamName);
    }

    public static IEnumerable<object?[]> UnsupportedRegistryLabels()
    {
        string?[] labels =
        [
            null,
            "",
            " ",
            " halo2/ipa",
            "halo2/ipa ",
            "HALO2/IPA",
            "halo2-ipa-pasta",
            "halo2/pasta",
            "halo2/ipa-pasta-cycle-v1",
            "halo2/pasta/ipa/ivm-execution-v1",
            "halo2/ipa:ivm-execution-v1",
            "halo2/ipa::ivm-execution-v1",
            "stark",
            "STARK/FRI",
            "stark/fri/",
            "stark/fri/latest",
            "stark/fri/sha512-goldilocks",
            "halo2/bn254",
            "groth16",
            "groth16/bls12-377",
            "halo2-ipa-orchard",
            "anonymous-pgc",
            "verange",
            "zkat",
            "silent-threshold-anoncred",
            "aztec-plonkish-private-kernel",
            "penumbra-masp",
            "stark/fri/sha256-goldilocks\0",
            "st\u0430rk/fri",
            "stark\uFF0Ffri",
            "stark/fri/\u200Bsha256-goldilocks",
        ];

        foreach (var label in labels)
        {
            yield return [label];
        }
    }
}
