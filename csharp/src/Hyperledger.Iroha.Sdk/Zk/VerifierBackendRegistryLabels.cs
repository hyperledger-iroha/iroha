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
    private static readonly (string Label, VerifyingKeyBackendTag Engine)[] SupportedBindings =
    [
        ("halo2/ipa", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("halo2/pasta/kaigi-roster-v1", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("halo2/pasta/kaigi-usage-v1", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("halo2/pasta/ivm-execution-v1", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4", VerifyingKeyBackendTag.Halo2IpaPasta),
        ("stark/fri", VerifyingKeyBackendTag.Stark),
        ("stark/fri/sha256-goldilocks", VerifyingKeyBackendTag.Stark),
        ("stark/fri/poseidon2-goldilocks", VerifyingKeyBackendTag.Stark),
        ("stark/fri/sha256_goldilocks.v1", VerifyingKeyBackendTag.Stark),
    ];

    public static bool IsSupportedLabel(string? label)
        => TryGetBackendTag(label, out _);

    public static bool TryGetBackendTag(
        string? label,
        out VerifyingKeyBackendTag backendTag)
    {
        foreach (var (supported, engine) in SupportedBindings)
        {
            if (string.Equals(label, supported, StringComparison.Ordinal))
            {
                backendTag = engine;
                return true;
            }
        }

        backendTag = default;
        return false;
    }

    public static VerifyingKeyBackendTag RequireBackendTag(
        string? label,
        string context = "backend")
    {
        if (!TryGetBackendTag(label, out var backendTag))
        {
            throw new ArgumentException(
                $"{context} is not an exact supported verifier-registry backend label.",
                context);
        }

        return backendTag;
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
