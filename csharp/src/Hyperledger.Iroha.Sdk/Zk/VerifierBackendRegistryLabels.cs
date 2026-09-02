using System;

namespace Hyperledger.Iroha.Zk;

/// <summary>
/// Exact verifier-registry identifiers accepted by the native Rust dispatch.
/// These identifiers are intentionally separate from
/// <see cref="VerifyingKeyBackendTag"/>: a registry profile selects one
/// concrete verifier configuration, while the Norito enum selects its
/// low-level proof engine.
/// </summary>
public static class VerifierBackendRegistryLabels
{
    private static readonly string[] SupportedLabels =
    [
        "halo2/ipa",
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/kaigi-usage-v1",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/offline-cash-v1-mint-fold-merkle16-axiom-poseidon-v1",
        "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
        "stark/fri/poseidon-x7-goldilocks-6x64-v1",
    ];

    public static bool IsSupportedLabel(string? label)
    {
        if (label is null)
        {
            return false;
        }

        foreach (var supported in SupportedLabels)
        {
            if (string.Equals(label, supported, StringComparison.Ordinal))
            {
                return true;
            }
        }

        return false;
    }

    public static string RequireSupportedLabel(
        string? label,
        string context = "backend")
    {
        if (!IsSupportedLabel(label))
        {
            throw new ArgumentException(
                $"{context} is not an exact supported verifier-registry backend label.",
                context);
        }

        return label!;
    }
}
