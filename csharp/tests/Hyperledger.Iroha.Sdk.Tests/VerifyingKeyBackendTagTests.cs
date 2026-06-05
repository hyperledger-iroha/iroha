using Hyperledger.Iroha.Zk;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class VerifyingKeyBackendTagTests
{
    [Fact]
    public void NoritoDiscriminantsPreserveRustOrder()
    {
        (VerifyingKeyBackendTag Tag, uint Discriminant)[] expected =
        [
            (VerifyingKeyBackendTag.Halo2IpaPasta, 0),
            (VerifyingKeyBackendTag.Halo2Bn254, 1),
            (VerifyingKeyBackendTag.Groth16, 2),
            (VerifyingKeyBackendTag.Stark, 3),
            (VerifyingKeyBackendTag.Unsupported, 4),
            (VerifyingKeyBackendTag.Halo2IpaOrchard, 5),
            (VerifyingKeyBackendTag.Groth16Bls12377, 6),
            (VerifyingKeyBackendTag.FcmpPlusPlusCurveTree, 7),
            (VerifyingKeyBackendTag.LatticePcsSis, 8),
            (VerifyingKeyBackendTag.MidenStark, 9),
            (VerifyingKeyBackendTag.AztecPlonkishPrivateKernel, 10),
            (VerifyingKeyBackendTag.PqMaspStarkFri, 11),
            (VerifyingKeyBackendTag.AnonymousPgc, 12),
            (VerifyingKeyBackendTag.VeRange, 13),
            (VerifyingKeyBackendTag.ZkAt, 14),
            (VerifyingKeyBackendTag.RecursiveAnonymousAdmission, 15),
            (VerifyingKeyBackendTag.VegaExistingCredentialZk, 16),
            (VerifyingKeyBackendTag.SilentThresholdAnoncred, 17),
            (VerifyingKeyBackendTag.ZkX509, 18),
            (VerifyingKeyBackendTag.SisWithHints, 19),
        ];

        foreach (var (tag, discriminant) in expected)
        {
            Assert.Equal(discriminant, tag.NoritoDiscriminant());
            Assert.Equal(tag, (VerifyingKeyBackendTag)discriminant);
        }
    }

    [Theory]
    [InlineData(VerifyingKeyBackendTag.Halo2IpaPasta, "halo2-ipa-pasta")]
    [InlineData(VerifyingKeyBackendTag.Halo2Bn254, "halo2-bn254")]
    [InlineData(VerifyingKeyBackendTag.Groth16, "groth16")]
    [InlineData(VerifyingKeyBackendTag.Stark, "stark")]
    [InlineData(VerifyingKeyBackendTag.Unsupported, "unsupported")]
    [InlineData(VerifyingKeyBackendTag.Halo2IpaOrchard, "halo2-ipa-orchard")]
    [InlineData(VerifyingKeyBackendTag.Groth16Bls12377, "groth16-bls12-377")]
    [InlineData(VerifyingKeyBackendTag.FcmpPlusPlusCurveTree, "fcmp-plus-plus-curve-tree")]
    [InlineData(VerifyingKeyBackendTag.LatticePcsSis, "lattice-pcs-sis")]
    [InlineData(VerifyingKeyBackendTag.MidenStark, "miden-stark")]
    [InlineData(VerifyingKeyBackendTag.AztecPlonkishPrivateKernel, "aztec-plonkish-private-kernel")]
    [InlineData(VerifyingKeyBackendTag.PqMaspStarkFri, "pq-masp-stark-fri")]
    [InlineData(VerifyingKeyBackendTag.AnonymousPgc, "anonymous-pgc")]
    [InlineData(VerifyingKeyBackendTag.VeRange, "verange")]
    [InlineData(VerifyingKeyBackendTag.ZkAt, "zkat")]
    [InlineData(VerifyingKeyBackendTag.RecursiveAnonymousAdmission, "recursive-anonymous-admission")]
    [InlineData(VerifyingKeyBackendTag.VegaExistingCredentialZk, "vega-existing-credential-zk")]
    [InlineData(VerifyingKeyBackendTag.SilentThresholdAnoncred, "silent-threshold-anoncred")]
    [InlineData(VerifyingKeyBackendTag.ZkX509, "zk-x509")]
    [InlineData(VerifyingKeyBackendTag.SisWithHints, "sis-with-hints")]
    public void CanonicalLabelsMatchRustCatalog(VerifyingKeyBackendTag tag, string label)
    {
        Assert.Equal(label, tag.CanonicalLabel());
    }

    [Theory]
    [InlineData("halo2-ipa-orchard", VerifyingKeyBackendTag.Halo2IpaOrchard)]
    [InlineData("halo2/ipa/orchard", VerifyingKeyBackendTag.Halo2IpaOrchard)]
    [InlineData("orchard", VerifyingKeyBackendTag.Halo2IpaOrchard)]
    [InlineData("anonymous-pgc", VerifyingKeyBackendTag.AnonymousPgc)]
    [InlineData("anonymous-pgc-k-out-of-n", VerifyingKeyBackendTag.AnonymousPgc)]
    [InlineData("verange-transparent-range", VerifyingKeyBackendTag.VeRange)]
    [InlineData("zkAt policy-private authenticator", VerifyingKeyBackendTag.ZkAt)]
    [InlineData("recursive-anonymous-admission", VerifyingKeyBackendTag.RecursiveAnonymousAdmission)]
    [InlineData("zk-ams-recursive-admission-v0", VerifyingKeyBackendTag.RecursiveAnonymousAdmission)]
    [InlineData("vega-existing-credential-zk", VerifyingKeyBackendTag.VegaExistingCredentialZk)]
    [InlineData("threshold-anonymous-credentials", VerifyingKeyBackendTag.SilentThresholdAnoncred)]
    [InlineData("silent-threshold-anoncred", VerifyingKeyBackendTag.SilentThresholdAnoncred)]
    [InlineData("zkvm-x509-identity", VerifyingKeyBackendTag.ZkX509)]
    [InlineData("zk-x509-onchain-identity-v0", VerifyingKeyBackendTag.ZkX509)]
    [InlineData("sis-with-hints", VerifyingKeyBackendTag.SisWithHints)]
    [InlineData("lattice-anonymous-credentials", VerifyingKeyBackendTag.SisWithHints)]
    [InlineData("groth16-bls12-377", VerifyingKeyBackendTag.Groth16Bls12377)]
    [InlineData("groth16/bls12-377", VerifyingKeyBackendTag.Groth16Bls12377)]
    [InlineData("penumbra-masp", VerifyingKeyBackendTag.Groth16Bls12377)]
    [InlineData("halo2/ipa/penumbra", VerifyingKeyBackendTag.Groth16Bls12377)]
    [InlineData("halo2/ipa/masp", VerifyingKeyBackendTag.Groth16Bls12377)]
    [InlineData("monero-fcmp++", VerifyingKeyBackendTag.FcmpPlusPlusCurveTree)]
    [InlineData("fcmp-plus-plus-curve-tree", VerifyingKeyBackendTag.FcmpPlusPlusCurveTree)]
    [InlineData("halo2/ipa/monero", VerifyingKeyBackendTag.FcmpPlusPlusCurveTree)]
    [InlineData("halo2/ipa/curve-tree", VerifyingKeyBackendTag.FcmpPlusPlusCurveTree)]
    [InlineData("lattice-pcs-sis", VerifyingKeyBackendTag.LatticePcsSis)]
    [InlineData("jindo-lattice-pcs-zk", VerifyingKeyBackendTag.LatticePcsSis)]
    [InlineData("miden-stark", VerifyingKeyBackendTag.MidenStark)]
    [InlineData("aztec-plonkish-private-kernel", VerifyingKeyBackendTag.AztecPlonkishPrivateKernel)]
    [InlineData("pq-masp-stark-fri", VerifyingKeyBackendTag.PqMaspStarkFri)]
    [InlineData("post-quantum-masp", VerifyingKeyBackendTag.PqMaspStarkFri)]
    public void PendingProductionAliasesRemainFailClosed(string label, VerifyingKeyBackendTag expected)
    {
        var parsed = VerifyingKeyBackendTags.FromCatalogLabel(label);

        Assert.Equal(expected, parsed);
        Assert.True(parsed.IsPendingProductionBackend());
        Assert.True(VerifyingKeyBackendTags.IsPendingProductionBackendLabel(label));
    }

    [Theory]
    [InlineData("halo2-ipa-pasta", VerifyingKeyBackendTag.Halo2IpaPasta)]
    [InlineData("halo2/ipa", VerifyingKeyBackendTag.Halo2IpaPasta)]
    [InlineData("halo2/pasta/ipa/vote-bool", VerifyingKeyBackendTag.Halo2IpaPasta)]
    [InlineData("halo2-bn254", VerifyingKeyBackendTag.Halo2Bn254)]
    [InlineData("halo2/bn254", VerifyingKeyBackendTag.Halo2Bn254)]
    [InlineData("groth16", VerifyingKeyBackendTag.Groth16)]
    [InlineData("groth16/bn254", VerifyingKeyBackendTag.Groth16)]
    [InlineData("stark", VerifyingKeyBackendTag.Stark)]
    [InlineData("stark/fri/sha256-goldilocks", VerifyingKeyBackendTag.Stark)]
    [InlineData("unknown/privacy/backend", VerifyingKeyBackendTag.Unsupported)]
    public void SupportedLegacyFamiliesDoNotBecomePending(string label, VerifyingKeyBackendTag expected)
    {
        var parsed = VerifyingKeyBackendTags.FromCatalogLabel(label);

        Assert.Equal(expected, parsed);
        Assert.False(parsed.IsPendingProductionBackend());
    }

    [Theory]
    [InlineData("halo2\uFF0Fipa")]
    [InlineData("halo2/\u200Bipa")]
    [InlineData("h\u0430lo2/ipa")]
    [InlineData("stark\uFF0Ffri/sha256-goldilocks")]
    [InlineData("stark/fri/\u200Bsha256-goldilocks")]
    [InlineData("st\u0430rk/fri/sha256-goldilocks")]
    public void CatalogAliasesRejectNonAsciiConfusablesBeforeCompaction(string label)
    {
        var parsed = VerifyingKeyBackendTags.FromCatalogLabel(label);

        Assert.Equal(VerifyingKeyBackendTag.Unsupported, parsed);
        Assert.False(VerifyingKeyBackendTags.IsPendingProductionBackendLabel(label));
    }

    [Theory]
    [InlineData("halo2/ipa/orchard/dev-fixture")]
    [InlineData("stark/fri/miden/claimed-production")]
    [InlineData("anonymous-pgc-k-out-of-n-v1-production")]
    [InlineData("sis-hints-anoncred-pq-v0-devfixture")]
    [InlineData("groth16/bls12-377/../../prod")]
    [InlineData("post-quantum-masp/audit-claimed")]
    public void AdversarialPendingAliasSplicesStayUnsupported(string label)
    {
        var parsed = VerifyingKeyBackendTags.FromCatalogLabel(label);

        Assert.Equal(VerifyingKeyBackendTag.Unsupported, parsed);
        Assert.False(parsed.IsPendingProductionBackend());
        Assert.False(VerifyingKeyBackendTags.IsPendingProductionBackendLabel(label));
    }

    [Theory]
    [InlineData("halo2/ipa")]
    [InlineData("halo2/ipa:ivm-execution-v1")]
    [InlineData("halo2/pasta/ivm-execution-v1")]
    [InlineData("halo2/pasta/kagemusha-folded-v1")]
    [InlineData("halo2/pasta/kaigi-roster-v1")]
    [InlineData("halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified")]
    [InlineData("stark/fri")]
    [InlineData("stark/fri/sha256-goldilocks")]
    public void ProductionVerifierBackendClassifierMirrorsNativeAllowlist(string backend)
    {
        Assert.True(VerifyingKeyBackendTags.IsProductionVerifyBackendLabel(backend));
        Assert.Equal(backend, VerifyingKeyBackendTags.RequireProductionVerifyBackendLabel(backend));
    }

    [Theory]
    [InlineData("")]
    [InlineData("unknown/privacy/backend")]
    [InlineData("halo2/unknown-native-v1")]
    [InlineData("halo2/ipa:unknown-native-v1")]
    [InlineData("stark/unknown-native-v1")]
    [InlineData("halo2/bn254")]
    [InlineData("groth16")]
    [InlineData("groth16/bls12-377")]
    [InlineData("halo2/ipa/orchard")]
    [InlineData("halo2-ipa-orchard")]
    [InlineData("halo2/ipa/penumbra")]
    [InlineData("halo2/ipa/masp")]
    [InlineData("halo2/ipa/monero")]
    [InlineData("halo2/ipa/curve-tree")]
    [InlineData("halo2/pasta/tiny-add")]
    [InlineData("halo2/ipa/tiny-add")]
    [InlineData("halo2/ipa:tiny-add")]
    [InlineData("halo2/pasta/tiny-commit-open")]
    [InlineData("halo2/pasta/anon-transfer-2x2")]
    [InlineData("halo2/ipa/anon-transfer-2x2")]
    [InlineData("halo2/ipa:anon-transfer-2x2")]
    [InlineData("halo2/pasta/anon-transfer-2x2-merkle2")]
    [InlineData("halo2/ipa/anon-transfer-2x2-merkle8")]
    [InlineData("halo2/ipa:anon-transfer-2x2-merkle16")]
    [InlineData("halo2/pasta/vote-bool-commit")]
    [InlineData("halo2/ipa/vote-bool-commit")]
    [InlineData("halo2/ipa:vote-bool-commit")]
    [InlineData("halo2/pasta/vote-bool-commit-merkle2")]
    [InlineData("halo2/ipa/vote-bool-commit-merkle8")]
    [InlineData("halo2/ipa:vote-bool-commit-merkle16")]
    [InlineData("halo2/pasta/asset-hidden-transfer-public-test")]
    [InlineData("halo2/ipa/asset-hidden-transfer-public-test")]
    [InlineData("halo2/ipa:asset-hidden-transfer-public-test")]
    [InlineData("stark/fri/miden")]
    [InlineData("stark/fri/miden/claimed-production")]
    [InlineData("stark/fri/latest")]
    [InlineData("stark/fri/attestation")]
    [InlineData("stark/fri/contest")]
    [InlineData("stark/fri/random-profile")]
    [InlineData("stark/fri/sha512-goldilocks")]
    [InlineData("stark/fri/audit-proof-v1")]
    [InlineData("stark/fri/sha256 goldilocks")]
    [InlineData("stark/fri/sha256+goldilocks")]
    [InlineData("halo2/ipa+mock")]
    [InlineData("halo2/ipa:production-ready")]
    [InlineData("halo2/ipa:claimed-production")]
    [InlineData("halo2/ipa:mainnet-ready")]
    [InlineData("stark/fri/audit-signoff")]
    [InlineData("stark/fri/externally-audited")]
    [InlineData("stark/fri/security-review-passed")]
    [InlineData("stark/fri/S.e.c.u.r.i.t.yReviewPassed")]
    [InlineData("stark/fri/a-u-d-i-t-c-l-a-i-m")]
    [InlineData("stark/fri/dev-fixture")]
    [InlineData("stark/fri/d-e-v-f-i-x-t-u-r-e")]
    [InlineData("stark/fri/dev")]
    [InlineData("stark/fri/d-e-v")]
    [InlineData("stark/fri/test")]
    [InlineData("stark/fri/t-e-s-t")]
    [InlineData("stark/fri/placeholder")]
    [InlineData("halo2/ipa:dev-fixture")]
    [InlineData("halo2/ipa:dev")]
    [InlineData("halo2/ipa:d-e-v")]
    [InlineData("halo2/ipa:dummy")]
    [InlineData("halo2/ipa:f-a-k-e")]
    [InlineData("halo2/ipa:stub")]
    [InlineData("halo2/ipa:s-a-m-p-l-e")]
    [InlineData("halo2/kzg")]
    [InlineData("halo2/pasta/mock")]
    [InlineData("halo2/pasta/debug-vote")]
    [InlineData("mock/dev")]
    [InlineData("kzg/powersoftau")]
    [InlineData("../halo2/ipa")]
    [InlineData(" halo2/ipa")]
    [InlineData("halo2/ipa ")]
    [InlineData("\thalo2/ipa")]
    [InlineData("halo2/ipa\n")]
    [InlineData(" stark/fri/sha256-goldilocks")]
    [InlineData("stark/fri/sha256-goldilocks ")]
    [InlineData("halo2\uFF0Fipa")]
    [InlineData("halo2/\u200Bipa")]
    [InlineData("h\u0430lo2/ipa")]
    [InlineData("halo2/ipa\0")]
    public void ProductionVerifierBackendClassifierRejectsUnsafeLabels(string backend)
    {
        Assert.False(VerifyingKeyBackendTags.IsProductionVerifyBackendLabel(backend));
        Assert.Throws<ArgumentException>(
            () => VerifyingKeyBackendTags.RequireProductionVerifyBackendLabel(backend));
    }
}
