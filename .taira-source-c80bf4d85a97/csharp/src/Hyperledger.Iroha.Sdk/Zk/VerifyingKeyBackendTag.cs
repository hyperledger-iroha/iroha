using System;

namespace Hyperledger.Iroha.Zk;

/// <summary>
/// Low-level proof engines accepted by the canonical Norito
/// <c>iroha_data_model::zk::BackendTag</c> wire contract.
/// </summary>
/// <remarks>
/// Privacy protocols use their own typed protocol and engine identifiers. They
/// must never be inferred from a generic backend label.
/// </remarks>
public enum VerifyingKeyBackendTag : uint
{
    Halo2IpaPasta = 0,
    Stark = 1,
}

public static class VerifyingKeyBackendTags
{
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
