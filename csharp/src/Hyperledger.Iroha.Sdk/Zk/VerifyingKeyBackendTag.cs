using System;
using System.Collections.Generic;
using System.Text;

namespace Hyperledger.Iroha.Zk;

public enum VerifyingKeyBackendTag : uint
{
    Halo2IpaPasta = 0,
    Halo2Bn254 = 1,
    Groth16 = 2,
    Stark = 3,
    Unsupported = 4,
    Halo2IpaOrchard = 5,
    Groth16Bls12377 = 6,
    FcmpPlusPlusCurveTree = 7,
    LatticePcsSis = 8,
    MidenStark = 9,
    AztecPlonkishPrivateKernel = 10,
    PqMaspStarkFri = 11,
    AnonymousPgc = 12,
    VeRange = 13,
    ZkAt = 14,
    RecursiveAnonymousAdmission = 15,
    VegaExistingCredentialZk = 16,
    SilentThresholdAnoncred = 17,
    ZkX509 = 18,
    SisWithHints = 19,
}

public static class VerifyingKeyBackendTags
{
    public static uint NoritoDiscriminant(this VerifyingKeyBackendTag tag) => (uint)tag;

    public static string CanonicalLabel(this VerifyingKeyBackendTag tag) => tag switch
    {
        VerifyingKeyBackendTag.Halo2IpaPasta => "halo2-ipa-pasta",
        VerifyingKeyBackendTag.Halo2Bn254 => "halo2-bn254",
        VerifyingKeyBackendTag.Groth16 => "groth16",
        VerifyingKeyBackendTag.Stark => "stark",
        VerifyingKeyBackendTag.Unsupported => "unsupported",
        VerifyingKeyBackendTag.Halo2IpaOrchard => "halo2-ipa-orchard",
        VerifyingKeyBackendTag.Groth16Bls12377 => "groth16-bls12-377",
        VerifyingKeyBackendTag.FcmpPlusPlusCurveTree => "fcmp-plus-plus-curve-tree",
        VerifyingKeyBackendTag.LatticePcsSis => "lattice-pcs-sis",
        VerifyingKeyBackendTag.MidenStark => "miden-stark",
        VerifyingKeyBackendTag.AztecPlonkishPrivateKernel => "aztec-plonkish-private-kernel",
        VerifyingKeyBackendTag.PqMaspStarkFri => "pq-masp-stark-fri",
        VerifyingKeyBackendTag.AnonymousPgc => "anonymous-pgc",
        VerifyingKeyBackendTag.VeRange => "verange",
        VerifyingKeyBackendTag.ZkAt => "zkat",
        VerifyingKeyBackendTag.RecursiveAnonymousAdmission => "recursive-anonymous-admission",
        VerifyingKeyBackendTag.VegaExistingCredentialZk => "vega-existing-credential-zk",
        VerifyingKeyBackendTag.SilentThresholdAnoncred => "silent-threshold-anoncred",
        VerifyingKeyBackendTag.ZkX509 => "zk-x509",
        VerifyingKeyBackendTag.SisWithHints => "sis-with-hints",
        _ => "unsupported",
    };

    public static bool IsPendingProductionBackend(this VerifyingKeyBackendTag tag) => tag switch
    {
        VerifyingKeyBackendTag.Halo2IpaOrchard
            or VerifyingKeyBackendTag.Groth16Bls12377
            or VerifyingKeyBackendTag.FcmpPlusPlusCurveTree
            or VerifyingKeyBackendTag.LatticePcsSis
            or VerifyingKeyBackendTag.MidenStark
            or VerifyingKeyBackendTag.AztecPlonkishPrivateKernel
            or VerifyingKeyBackendTag.PqMaspStarkFri
            or VerifyingKeyBackendTag.AnonymousPgc
            or VerifyingKeyBackendTag.VeRange
            or VerifyingKeyBackendTag.ZkAt
            or VerifyingKeyBackendTag.RecursiveAnonymousAdmission
            or VerifyingKeyBackendTag.VegaExistingCredentialZk
            or VerifyingKeyBackendTag.SilentThresholdAnoncred
            or VerifyingKeyBackendTag.ZkX509
            or VerifyingKeyBackendTag.SisWithHints => true,
        _ => false,
    };

    public static VerifyingKeyBackendTag FromCatalogLabel(string? raw)
    {
        var label = (raw ?? string.Empty).Trim().ToLowerInvariant();
        if (label.Length == 0)
        {
            return VerifyingKeyBackendTag.Unsupported;
        }
        if (HasNonAscii(label))
        {
            return VerifyingKeyBackendTag.Unsupported;
        }

        var compact = CompactAscii(label);
        return CatalogBackendAliases.TryGetValue(compact, out var tag)
            ? tag
            : VerifyingKeyBackendTag.Unsupported;
    }

    public static bool IsPendingProductionBackendLabel(string? raw) =>
        FromCatalogLabel(raw).IsPendingProductionBackend();

    public static bool IsProductionVerifyBackendLabel(string? raw)
    {
        if (raw is null)
        {
            return false;
        }

        var backend = raw;
        if (backend.Length == 0
            || backend.Trim() != backend
            || !IsPortableVerifierBackendLabel(backend)
            || IsPendingProductionBackendLabel(backend)
            || IsProductionClaimBackendLabel(backend)
            || IsTrustedSetupBackendLabel(backend)
            || IsDeveloperOnlyBackendLabel(backend))
        {
            return false;
        }

        return backend == "halo2/ipa"
            || IsStarkFriProductionBackendLabel(backend)
            || IsNativeHalo2PastaProductionBackendLabel(backend);
    }

    public static string RequireProductionVerifyBackendLabel(string? raw, string context = "backend")
    {
        if (raw is null || raw.Trim().Length == 0)
        {
            throw new ArgumentException($"{context} must not be blank.", context);
        }

        var backend = raw;
        if (backend.Trim() != backend)
        {
            throw new ArgumentException($"{context} must not contain surrounding whitespace.", context);
        }

        if (!IsProductionVerifyBackendLabel(backend))
        {
            throw new ArgumentException(
                $"{context} uses unsupported production verifier backend {backend}.",
                context);
        }

        return backend;
    }

    private static readonly HashSet<string> ProductionNativeHalo2PastaBackends =
        new(StringComparer.Ordinal)
        {
            "halo2/pasta/kaigi-roster-v1",
            "halo2/pasta/kaigi-usage-v1",
            "halo2/pasta/ivm-overlay-bind",
            "halo2/pasta/ivm-execution-v1",
            "halo2/pasta/offline-note-recursive",
            "halo2/pasta/kagemusha-folded-v1",
            "halo2/pasta/kagemusha-recursive-aggregation-v1",
            "halo2/pasta/kagemusha-recursive-compact-v1",
            "halo2/pasta/kagemusha-recursive-spend-lineage-v1",
            "halo2/pasta/kagemusha-recursive-spend-lineage-onehop-v1",
            "halo2/pasta/kagemusha-recursive-spend-lineage-append-v1",
            "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
            "halo2/pasta/anon-unshield-merkle16-poseidon-diversified",
            "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified",
        };

    private static readonly Dictionary<string, VerifyingKeyBackendTag> CatalogBackendAliases =
        new(StringComparer.Ordinal)
        {
            ["unsupported"] = VerifyingKeyBackendTag.Unsupported,
            ["halo2ipa"] = VerifyingKeyBackendTag.Halo2IpaPasta,
            ["halo2ipapasta"] = VerifyingKeyBackendTag.Halo2IpaPasta,
            ["halo2pasta"] = VerifyingKeyBackendTag.Halo2IpaPasta,
            ["halo2pastaipavotebool"] = VerifyingKeyBackendTag.Halo2IpaPasta,
            ["halo2bn254"] = VerifyingKeyBackendTag.Halo2Bn254,
            ["groth16"] = VerifyingKeyBackendTag.Groth16,
            ["groth16bn254"] = VerifyingKeyBackendTag.Groth16,
            ["stark"] = VerifyingKeyBackendTag.Stark,
            ["starkfri"] = VerifyingKeyBackendTag.Stark,
            ["starkfrisha256goldilocks"] = VerifyingKeyBackendTag.Stark,
            ["starkfriposeidon2goldilocks"] = VerifyingKeyBackendTag.Stark,
            ["starkfrisha256goldilocksv1"] = VerifyingKeyBackendTag.Stark,
            ["halo2ipaorchard"] = VerifyingKeyBackendTag.Halo2IpaOrchard,
            ["orchard"] = VerifyingKeyBackendTag.Halo2IpaOrchard,
            ["zcashorchard"] = VerifyingKeyBackendTag.Halo2IpaOrchard,
            ["groth16bls12377"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["groth16bls12377decaf377"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["bls12377"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["decaf377"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["masp"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["penumbra"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["penumbramasp"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["halo2ipapenumbra"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["halo2ipamasp"] = VerifyingKeyBackendTag.Groth16Bls12377,
            ["fcmppluspluscurvetree"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["fcmp"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["monero"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["monerofcmp"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["monerofcmpplusplus"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["curvetree"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["halo2ipamonero"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["halo2ipacurvetree"] = VerifyingKeyBackendTag.FcmpPlusPlusCurveTree,
            ["latticepcssis"] = VerifyingKeyBackendTag.LatticePcsSis,
            ["latticepcszk"] = VerifyingKeyBackendTag.LatticePcsSis,
            ["jindo"] = VerifyingKeyBackendTag.LatticePcsSis,
            ["jindolatticepcszk"] = VerifyingKeyBackendTag.LatticePcsSis,
            ["jindolatticepcszkv0"] = VerifyingKeyBackendTag.LatticePcsSis,
            ["jindolatticepcssis"] = VerifyingKeyBackendTag.LatticePcsSis,
            ["starkfrimiden"] = VerifyingKeyBackendTag.MidenStark,
            ["midenstark"] = VerifyingKeyBackendTag.MidenStark,
            ["aztecplonkishprivatekernel"] = VerifyingKeyBackendTag.AztecPlonkishPrivateKernel,
            ["aztecprivatekernel"] = VerifyingKeyBackendTag.AztecPlonkishPrivateKernel,
            ["pqmaspstarkfri"] = VerifyingKeyBackendTag.PqMaspStarkFri,
            ["pqmaspstark"] = VerifyingKeyBackendTag.PqMaspStarkFri,
            ["starkfripqmaspstarkfri"] = VerifyingKeyBackendTag.PqMaspStarkFri,
            ["postquantummasp"] = VerifyingKeyBackendTag.PqMaspStarkFri,
            ["anonymouspgc"] = VerifyingKeyBackendTag.AnonymousPgc,
            ["anonymouspgckoutofn"] = VerifyingKeyBackendTag.AnonymousPgc,
            ["anonymouspgckoutofnv1"] = VerifyingKeyBackendTag.AnonymousPgc,
            ["verange"] = VerifyingKeyBackendTag.VeRange,
            ["verangetransparentrange"] = VerifyingKeyBackendTag.VeRange,
            ["verangetransparentrangev1"] = VerifyingKeyBackendTag.VeRange,
            ["zkat"] = VerifyingKeyBackendTag.ZkAt,
            ["zkatpolicyprivateauthenticator"] = VerifyingKeyBackendTag.ZkAt,
            ["zkatpolicyprivateauthv1"] = VerifyingKeyBackendTag.ZkAt,
            ["recursiveanonymousadmission"] = VerifyingKeyBackendTag.RecursiveAnonymousAdmission,
            ["recursiveanonymousadmissionv0"] = VerifyingKeyBackendTag.RecursiveAnonymousAdmission,
            ["zkamsrecursiveadmission"] = VerifyingKeyBackendTag.RecursiveAnonymousAdmission,
            ["zkamsrecursiveadmissionv0"] = VerifyingKeyBackendTag.RecursiveAnonymousAdmission,
            ["vegaexistingcredentialzk"] = VerifyingKeyBackendTag.VegaExistingCredentialZk,
            ["vegaexistingcredentialzkv0"] = VerifyingKeyBackendTag.VegaExistingCredentialZk,
            ["silentthresholdanoncred"] = VerifyingKeyBackendTag.SilentThresholdAnoncred,
            ["silentthresholdanoncredv0"] = VerifyingKeyBackendTag.SilentThresholdAnoncred,
            ["silentthresholdanonymouscredential"] = VerifyingKeyBackendTag.SilentThresholdAnoncred,
            ["thresholdanonymouscredentials"] = VerifyingKeyBackendTag.SilentThresholdAnoncred,
            ["zkx509"] = VerifyingKeyBackendTag.ZkX509,
            ["zkvmx509identity"] = VerifyingKeyBackendTag.ZkX509,
            ["zkx509onchainidentity"] = VerifyingKeyBackendTag.ZkX509,
            ["zkx509onchainidentityv0"] = VerifyingKeyBackendTag.ZkX509,
            ["siswithhints"] = VerifyingKeyBackendTag.SisWithHints,
            ["sishints"] = VerifyingKeyBackendTag.SisWithHints,
            ["sishintsanoncredpqv0"] = VerifyingKeyBackendTag.SisWithHints,
            ["latticeanonymouscredentials"] = VerifyingKeyBackendTag.SisWithHints,
        };

    private static readonly HashSet<string> StarkFriProductionBackends =
        new(StringComparer.Ordinal)
        {
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
        };

    private static readonly HashSet<string> TrustedSetupBackendSegments =
        new(StringComparer.Ordinal)
        {
            "groth16",
            "kzg",
            "bn254",
            "bn256",
            "bls12",
            "srs",
            "crs",
            "ptau",
            "ceremony",
            "powersoftau",
        };

    private static readonly string[] TrustedSetupCompactTokens =
    [
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12381",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "trustedsetup",
        "structuredreferencestring",
        "universalsrs",
        "powersoftau",
    ];

    private static readonly string[] ProductionClaimBackendFragments =
    [
        "productionready",
        "productionhardened",
        "productionenabled",
        "productionapproved",
        "productioncertified",
        "productionclaim",
        "claimedproduction",
        "mainnetready",
        "mainnetcomplete",
        "mainnetclaim",
        "claimedmainnet",
        "mainnetcertified",
        "mainnetapproved",
        "mainnetrelease",
        "auditedproduction",
        "externallyaudited",
        "thirdpartyaudited",
        "boiaudited",
        "auditedmainnet",
        "externalaudit",
        "auditpassed",
        "auditapproved",
        "auditsignoff",
        "auditclaim",
        "claimedaudit",
        "securityreviewpassed",
        "securityauditpassed",
        "securityaudited",
        "externalsecurityreview",
        "certifiedproduction",
        "certifiedmainnet",
        "releaseready",
        "releaseapproved",
        "releasecertified",
    ];

    private static bool IsTrustedSetupBackendLabel(string raw)
    {
        var label = raw.Trim().ToLowerInvariant();
        var compact = CompactAscii(label);
        foreach (var segment in LowercaseAsciiSegments(label))
        {
            if (TrustedSetupBackendSegments.Contains(segment))
            {
                return true;
            }
        }
        foreach (var token in TrustedSetupCompactTokens)
        {
            if (compact.Contains(token, StringComparison.Ordinal))
            {
                return true;
            }
        }

        return label == "groth16"
            || label.StartsWith("groth16/", StringComparison.Ordinal)
            || label == "kzg"
            || label.StartsWith("kzg/", StringComparison.Ordinal)
            || label == "bn254"
            || label == "bn256"
            || label == "bls12_381"
            || label == "bls12-381"
            || label == "halo2/bn254"
            || label.StartsWith("halo2/bn254/", StringComparison.Ordinal)
            || label.Contains("/bn254", StringComparison.Ordinal)
            || label.Contains(":bn254", StringComparison.Ordinal)
            || label.Contains("/bn256", StringComparison.Ordinal)
            || label.Contains(":bn256", StringComparison.Ordinal)
            || label.Contains("/bls12", StringComparison.Ordinal)
            || label.Contains(":bls12", StringComparison.Ordinal)
            || label == "halo2/kzg"
            || label.StartsWith("halo2/kzg/", StringComparison.Ordinal)
            || label.Contains("/kzg", StringComparison.Ordinal)
            || label.Contains(":kzg", StringComparison.Ordinal);
    }

    private static bool IsDeveloperOnlyBackendLabel(string raw)
    {
        var label = raw.Trim().ToLowerInvariant();
        var compact = CompactAscii(label);
        foreach (var fragment in new[]
                 {
                     "notforproduction",
                     "notproduction",
                     "notproductionready",
                     "notready",
                     "replacebeforeproduction",
                     "replacebeforemainnet",
                     "draftonly",
                 })
        {
            if (compact.Contains(fragment, StringComparison.Ordinal))
            {
                return true;
            }
        }

        var letterRun = new StringBuilder();
        foreach (var token in LowercaseAsciiSegments(label))
        {
            if (IsDeveloperOnlyBackendRun(token))
            {
                return true;
            }
            if (token.Length == 1)
            {
                letterRun.Append(token);
                continue;
            }
            if (IsDeveloperOnlyBackendRun(letterRun.ToString()))
            {
                return true;
            }
            letterRun.Clear();
        }
        return IsDeveloperOnlyBackendRun(letterRun.ToString());
    }

    private static bool IsProductionClaimBackendLabel(string raw)
    {
        var compact = CompactAscii(raw.ToLowerInvariant());
        foreach (var fragment in ProductionClaimBackendFragments)
        {
            if (compact.Contains(fragment, StringComparison.Ordinal))
            {
                return true;
            }
        }
        return false;
    }

    private static bool IsDeveloperOnlyBackendRun(string value)
    {
        return value.Contains("debug", StringComparison.Ordinal)
            || value.Contains("mock", StringComparison.Ordinal)
            || value.Contains("fixture", StringComparison.Ordinal)
            || value.Contains("dev", StringComparison.Ordinal)
            || value.Contains("todo", StringComparison.Ordinal)
            || value.Contains("draft", StringComparison.Ordinal)
            || value.Contains("pending", StringComparison.Ordinal)
            || value.Contains("replace", StringComparison.Ordinal)
            || value == "test"
            || value == "dummy"
            || value == "fake"
            || value == "stub"
            || value == "sample"
            || value == "placeholder"
            || value == "todo"
            || value == "draft";
    }

    private static bool IsStarkFriProductionBackendLabel(string backend)
    {
        return StarkFriProductionBackends.Contains(backend);
    }

    private static bool IsNativeHalo2PastaProductionBackendLabel(string backend)
    {
        var normalized = NormalizeNativeHalo2PastaBackendLabel(backend);
        return normalized is not null && ProductionNativeHalo2PastaBackends.Contains(normalized);
    }

    private static string? NormalizeNativeHalo2PastaBackendLabel(string raw)
    {
        var backend = raw;
        if (backend.Length == 0 || backend.Trim() != backend)
        {
            return null;
        }
        (string Prefix, string TargetPrefix)[] prefixes =
        [
            ("halo2/pasta/ipa/", "halo2/pasta/"),
            ("halo2/pasta/", "halo2/pasta/"),
            ("halo2/ipa::", "halo2/pasta/"),
            ("halo2/ipa:", "halo2/pasta/"),
            ("halo2/ipa/", "halo2/pasta/"),
        ];

        foreach (var (prefix, targetPrefix) in prefixes)
        {
            if (backend.StartsWith(prefix, StringComparison.Ordinal))
            {
                var rest = backend[prefix.Length..];
                return rest.Length == 0 ? null : targetPrefix + rest;
            }
        }

        return null;
    }

    private static bool IsPortableVerifierBackendLabel(string value)
    {
        if (value.Length == 0
            || !IsLowerAsciiAlphanumeric(value[0])
            || !IsLowerAsciiAlphanumeric(value[^1]))
        {
            return false;
        }
        foreach (var ch in value)
        {
            var allowed = IsLowerAsciiAlphanumeric(ch)
                || ch == '/'
                || ch == ':'
                || ch == '.'
                || ch == '_'
                || ch == '-';
            if (!allowed)
            {
                return false;
            }
        }

        foreach (var separator in new[] { "//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:" })
        {
            if (value.Contains(separator, StringComparison.Ordinal))
            {
                return false;
            }
        }
        return true;
    }

    private static bool IsLowerAsciiAlphanumeric(char ch)
    {
        return (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'z');
    }

    private static IEnumerable<string> LowercaseAsciiSegments(string value)
    {
        var builder = new StringBuilder(value.Length);
        foreach (var ch in value)
        {
            if ((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'z'))
            {
                builder.Append(ch);
            }
            else if (builder.Length > 0)
            {
                yield return builder.ToString();
                builder.Clear();
            }
        }
        if (builder.Length > 0)
        {
            yield return builder.ToString();
        }
    }

    private static string CompactAscii(string value)
    {
        var builder = new StringBuilder(value.Length);
        foreach (var ch in value)
        {
            if ((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'z'))
            {
                builder.Append(ch);
            }
        }
        return builder.ToString();
    }

    private static bool HasNonAscii(string value)
    {
        foreach (var ch in value)
        {
            if (ch > 0x7F)
            {
                return true;
            }
        }

        return false;
    }
}
