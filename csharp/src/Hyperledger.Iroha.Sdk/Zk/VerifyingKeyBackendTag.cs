using System;
using System.Collections.Generic;
using System.Text;

namespace Hyperledger.Iroha.Zk;

/// <summary>
/// Low-level proof engines accepted by the canonical Norito
/// <c>iroha_data_model::zk::BackendTag</c> wire contract.
/// </summary>
/// <remarks>
/// Privacy protocols and verifier profiles remain separate catalog labels and
/// never become wire-enum variants.
/// </remarks>
public enum VerifyingKeyBackendTag : uint
{
    Halo2IpaPasta = 0,
    Stark = 1,
}

/// <summary>
/// Human-facing catalog classification separate from the Norito wire enum.
/// </summary>
public enum VerifyingKeyBackendCatalogTag
{
    Production,
    Pending,
    Unsupported,
}

public static class VerifyingKeyBackendTags
{
    private static readonly HashSet<string> StarkFriProductionBackends =
        new(StringComparer.Ordinal)
        {
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
        };

    private static readonly HashSet<string> ProductionNativeHalo2PastaBackends =
        new(StringComparer.Ordinal)
        {
            "halo2/pasta/kaigi-roster-v1",
            "halo2/pasta/kaigi-usage-v1",
            "halo2/pasta/ivm-overlay-bind",
            "halo2/pasta/ivm-execution-v1",
            "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
            "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
            "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
            "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
            "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
            "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
        };

    private static readonly HashSet<string> PendingCatalogBackendAliases =
        new(StringComparer.Ordinal)
        {
            "halo2ipaorchard", "orchard", "zcashorchard",
            "groth16bls12377", "groth16bls12377decaf377", "bls12377",
            "decaf377", "masp", "penumbra", "penumbramasp", "halo2ipapenumbra",
            "halo2ipamasp", "fcmppluspluscurvetree", "fcmp", "monero",
            "monerofcmp", "monerofcmpplusplus", "curvetree", "halo2ipamonero",
            "halo2ipacurvetree", "latticepcssis", "latticepcszk", "jindo",
            "jindolatticepcszk", "jindolatticepcszkv0", "jindolatticepcssis",
            "starkfrimiden", "midenstark", "aztecplonkishprivatekernel",
            "aztecprivatekernel", "pqmaspstarkfri", "pqmaspstark",
            "starkfripqmaspstarkfri", "postquantummasp", "anonymouspgc",
            "anonymouspgckoutofn", "anonymouspgckoutofnv1", "verange",
            "verangetransparentrange", "verangetransparentrangev1", "zkat",
            "zkatpolicyprivateauthenticator", "zkatpolicyprivateauthv1",
            "recursiveanonymousadmission", "recursiveanonymousadmissionv0",
            "zkamsrecursiveadmission", "zkamsrecursiveadmissionv0",
            "vegaexistingcredentialzk", "vegaexistingcredentialzkv0",
            "silentthresholdanoncred", "silentthresholdanoncredv0",
            "silentthresholdanonymouscredential", "thresholdanonymouscredentials",
            "zkx509", "zkvmx509identity", "zkx509onchainidentity",
            "zkx509onchainidentityv0", "siswithhints", "sishints",
            "sishintsanoncredpqv0", "latticeanonymouscredentials",
        };

    private static readonly HashSet<string> TrustedSetupBackendSegments =
        new(StringComparer.Ordinal)
        {
            "groth16", "kzg", "bn254", "bn256", "bls12", "srs", "crs",
            "ptau", "ceremony", "powersoftau",
        };

    private static readonly string[] TrustedSetupCompactTokens =
    {
        "groth16", "kzg", "bn254", "bn256", "bls12381", "bls12",
        "srs", "crs", "ptau", "ceremony", "trustedsetup",
        "structuredreferencestring", "universalsrs", "powersoftau",
    };

    private static readonly string[] ProductionClaimBackendFragments =
    {
        "productionready", "productionhardened", "productionenabled",
        "productionapproved", "productioncertified", "productionclaim",
        "claimedproduction", "mainnetready", "mainnetcomplete", "mainnetclaim",
        "claimedmainnet", "mainnetcertified", "mainnetapproved", "mainnetrelease",
        "auditedproduction", "externallyaudited", "thirdpartyaudited",
        "boiaudited", "auditedmainnet", "externalaudit", "auditpassed",
        "auditapproved", "auditsignoff", "auditclaim", "claimedaudit",
        "securityreviewpassed", "securityauditpassed", "securityaudited",
        "externalsecurityreview", "certifiedproduction", "certifiedmainnet",
        "releaseready", "releaseapproved", "releasecertified",
    };

    public static uint NoritoDiscriminant(this VerifyingKeyBackendTag tag)
    {
        EnsureDefined(tag);
        return (uint)tag;
    }

    public static string CanonicalLabel(this VerifyingKeyBackendTag tag) => tag switch
    {
        VerifyingKeyBackendTag.Halo2IpaPasta => "halo2-ipa-pasta",
        VerifyingKeyBackendTag.Stark => "stark",
        _ => throw new ArgumentOutOfRangeException(
            nameof(tag),
            tag,
            "Unknown Norito verifying-key backend tag."),
    };

    public static bool TryFromCanonicalLabel(
        string? label,
        out VerifyingKeyBackendTag tag)
    {
        switch (label)
        {
            case "halo2-ipa-pasta":
                tag = VerifyingKeyBackendTag.Halo2IpaPasta;
                return true;
            case "stark":
                tag = VerifyingKeyBackendTag.Stark;
                return true;
            default:
                tag = default;
                return false;
        }
    }

    public static VerifyingKeyBackendTag FromCanonicalLabel(
        string? label,
        string context = "backend")
    {
        if (!TryFromCanonicalLabel(label, out var tag))
        {
            throw new ArgumentException(
                $"{context} must be one of the exact canonical backend labels: "
                    + "\"halo2-ipa-pasta\" or \"stark\".",
                context);
        }

        return tag;
    }

    public static string RequireCanonicalLabel(
        string? label,
        string context = "backend")
    {
        _ = FromCanonicalLabel(label, context);
        return label!;
    }

    public static VerifyingKeyBackendCatalogTag FromCatalogLabel(string? raw)
    {
        var label = raw?.Trim().ToLowerInvariant() ?? string.Empty;
        if (label.Length == 0 || HasNonAscii(label))
        {
            return VerifyingKeyBackendCatalogTag.Unsupported;
        }

        if (PendingCatalogBackendAliases.Contains(CompactAscii(label)))
        {
            return VerifyingKeyBackendCatalogTag.Pending;
        }
        if (VerifierBackendRegistryLabels.IsSupportedLabel(label)
            || TryFromCanonicalLabel(label, out _))
        {
            return VerifyingKeyBackendCatalogTag.Production;
        }
        return VerifyingKeyBackendCatalogTag.Unsupported;
    }

    public static bool IsPendingProductionBackend(
        this VerifyingKeyBackendCatalogTag tag) =>
        tag == VerifyingKeyBackendCatalogTag.Pending;

    public static bool IsPendingProductionBackendLabel(string? raw) =>
        FromCatalogLabel(raw).IsPendingProductionBackend();

    public static bool IsProductionVerifyBackendLabel(string? raw)
    {
        if (raw is null)
        {
            return false;
        }
        var backend = raw;
        if (string.IsNullOrWhiteSpace(backend)
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
            || StarkFriProductionBackends.Contains(backend)
            || ProductionNativeHalo2PastaBackends.Contains(backend);
    }

    public static string RequireProductionVerifyBackendLabel(
        string? raw,
        string context = "backend")
    {
        if (string.IsNullOrWhiteSpace(raw))
        {
            throw new ArgumentException($"{context} must not be blank.", context);
        }
        var backend = raw;
        if (backend.Trim() != backend)
        {
            throw new ArgumentException(
                $"{context} must not contain surrounding whitespace.",
                context);
        }
        if (!IsProductionVerifyBackendLabel(backend))
        {
            throw new ArgumentException(
                $"{context} uses unsupported production verifier backend {backend}.",
                context);
        }
        return backend;
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

    private static bool IsTrustedSetupBackendLabel(string raw)
    {
        var label = raw.ToLowerInvariant();
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
        return false;
    }

    private static bool IsDeveloperOnlyBackendLabel(string raw)
    {
        var label = raw.ToLowerInvariant();
        var compact = CompactAscii(label);
        foreach (var fragment in new[]
        {
            "notforproduction", "notproduction", "notproductionready", "notready",
            "replacebeforeproduction", "replacebeforemainnet", "draftonly",
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
            }
            else
            {
                if (IsDeveloperOnlyBackendRun(letterRun.ToString()))
                {
                    return true;
                }
                letterRun.Clear();
            }
        }
        return IsDeveloperOnlyBackendRun(letterRun.ToString());
    }

    private static bool IsDeveloperOnlyBackendRun(string value) =>
        value.Contains("debug", StringComparison.Ordinal)
        || value.Contains("mock", StringComparison.Ordinal)
        || value.Contains("fixture", StringComparison.Ordinal)
        || value.Contains("dev", StringComparison.Ordinal)
        || value.Contains("todo", StringComparison.Ordinal)
        || value.Contains("draft", StringComparison.Ordinal)
        || value.Contains("pending", StringComparison.Ordinal)
        || value.Contains("replace", StringComparison.Ordinal)
        || value is "test" or "dummy" or "fake" or "stub" or "sample" or "placeholder";

    private static bool IsPortableVerifierBackendLabel(string value)
    {
        if (value.Length == 0
            || !IsLowerAsciiAlphanumeric(value[0])
            || !IsLowerAsciiAlphanumeric(value[^1]))
        {
            return false;
        }
        foreach (var character in value)
        {
            if (!IsLowerAsciiAlphanumeric(character)
                && character is not ('/' or ':' or '.' or '_' or '-'))
            {
                return false;
            }
        }
        foreach (var separator in new[]
        {
            "//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:",
        })
        {
            if (value.Contains(separator, StringComparison.Ordinal))
            {
                return false;
            }
        }
        return true;
    }

    private static IEnumerable<string> LowercaseAsciiSegments(string value)
    {
        var segment = new StringBuilder();
        foreach (var character in value)
        {
            if (IsLowerAsciiAlphanumeric(character))
            {
                segment.Append(character);
            }
            else if (segment.Length != 0)
            {
                yield return segment.ToString();
                segment.Clear();
            }
        }
        if (segment.Length != 0)
        {
            yield return segment.ToString();
        }
    }

    private static string CompactAscii(string value)
    {
        var compact = new StringBuilder(value.Length);
        foreach (var character in value)
        {
            if (IsLowerAsciiAlphanumeric(character))
            {
                compact.Append(character);
            }
        }
        return compact.ToString();
    }

    private static bool IsLowerAsciiAlphanumeric(char value) =>
        value is >= '0' and <= '9' or >= 'a' and <= 'z';

    private static bool HasNonAscii(string value)
    {
        foreach (var character in value)
        {
            if (character > 0x7F)
            {
                return true;
            }
        }
        return false;
    }

    private static void EnsureDefined(VerifyingKeyBackendTag tag)
    {
        if (tag is not VerifyingKeyBackendTag.Halo2IpaPasta
            and not VerifyingKeyBackendTag.Stark)
        {
            throw new ArgumentOutOfRangeException(
                nameof(tag),
                tag,
                "Unknown Norito verifying-key backend tag.");
        }
    }
}
