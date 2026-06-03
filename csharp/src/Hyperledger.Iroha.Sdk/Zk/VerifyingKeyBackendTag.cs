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
        if (label == "unsupported" || compact == "unsupported")
        {
            return VerifyingKeyBackendTag.Unsupported;
        }

        if (compact.Contains("pqmasp", StringComparison.Ordinal)
            || compact.Contains("postquantummasp", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.PqMaspStarkFri;
        }
        if (compact.Contains("anonymouspgc", StringComparison.Ordinal)
            || compact.Contains("pgckoutofn", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.AnonymousPgc;
        }
        if (compact.Contains("verange", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.VeRange;
        }
        if (compact.Contains("zkat", StringComparison.Ordinal)
            || compact.Contains("policyprivateauthenticator", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.ZkAt;
        }
        if (compact.Contains("zkams", StringComparison.Ordinal)
            || compact.Contains("recursiveanonymousadmission", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.RecursiveAnonymousAdmission;
        }
        if (compact.Contains("vega", StringComparison.Ordinal)
            || compact.Contains("existingcredentialzk", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.VegaExistingCredentialZk;
        }
        if (compact.Contains("silentthreshold", StringComparison.Ordinal)
            || compact.Contains("thresholdanonymouscredential", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.SilentThresholdAnoncred;
        }
        if (compact.Contains("zkx509", StringComparison.Ordinal)
            || compact.Contains("x509", StringComparison.Ordinal)
            || compact.Contains("zkvmx509", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.ZkX509;
        }
        if (compact.Contains("siswithhints", StringComparison.Ordinal)
            || compact.Contains("sishints", StringComparison.Ordinal)
            || compact.Contains("latticeanonymouscredentials", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.SisWithHints;
        }
        if (compact.Contains("orchard", StringComparison.Ordinal)
            || compact.Contains("zcashorchard", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.Halo2IpaOrchard;
        }
        if (compact.Contains("penumbra", StringComparison.Ordinal)
            || compact.Contains("masp", StringComparison.Ordinal)
            || compact.Contains("bls12377", StringComparison.Ordinal)
            || compact.Contains("decaf377", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.Groth16Bls12377;
        }
        if (compact.Contains("fcmp", StringComparison.Ordinal)
            || compact.Contains("monero", StringComparison.Ordinal)
            || compact.Contains("curvetree", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.FcmpPlusPlusCurveTree;
        }
        if (compact.Contains("lattice", StringComparison.Ordinal)
            || compact.Contains("pcssis", StringComparison.Ordinal)
            || compact.Contains("jindo", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.LatticePcsSis;
        }
        if (compact.Contains("miden", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.MidenStark;
        }
        if (compact.Contains("aztec", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.AztecPlonkishPrivateKernel;
        }
        if (compact.Contains("halo2", StringComparison.Ordinal)
            && compact.Contains("bn254", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.Halo2Bn254;
        }
        if (compact.Contains("groth16", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.Groth16;
        }
        if (compact.Contains("stark", StringComparison.Ordinal))
        {
            return VerifyingKeyBackendTag.Stark;
        }
        if (compact == "halo2ipa"
            || compact == "halo2ipapasta"
            || compact == "halo2pasta"
            || (compact.Contains("halo2", StringComparison.Ordinal)
                && (compact.Contains("ipa", StringComparison.Ordinal)
                    || compact.Contains("pasta", StringComparison.Ordinal))))
        {
            return VerifyingKeyBackendTag.Halo2IpaPasta;
        }

        return VerifyingKeyBackendTag.Unsupported;
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
            "halo2/pasta/kagemusha-recursive-spend-lineage-v1",
            "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
            "halo2/pasta/anon-unshield-merkle16-poseidon-diversified",
            "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified",
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
        "auditedproduction",
        "externallyaudited",
        "auditpassed",
        "auditapproved",
        "auditsignoff",
        "auditclaim",
        "claimedaudit",
        "securityreviewpassed",
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
            || value == "test"
            || value == "dummy"
            || value == "fake"
            || value == "stub"
            || value == "sample"
            || value == "placeholder";
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
        foreach (var ch in value)
        {
            var allowed = (ch >= '0' && ch <= '9')
                || (ch >= 'A' && ch <= 'Z')
                || (ch >= 'a' && ch <= 'z')
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

        return true;
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
