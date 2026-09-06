namespace Hyperledger.Iroha.Kagemusha;

/// <summary>
/// Public operation-16 input containing exact independent mint-authorization and finalized-credit
/// archives. Shape validation grants no monetary authority and exposes no private device state.
/// </summary>
public sealed class KagemushaDeviceMintStageCommandV1
{
    private readonly byte[] canonicalAuthorization;
    private readonly byte[] canonicalMintCredit;

    /// <summary>Construct a bounded public command with defensively owned nested archives.</summary>
    public KagemushaDeviceMintStageCommandV1(
        ushort version,
        ReadOnlySpan<byte> canonicalAuthorization,
        ReadOnlySpan<byte> canonicalMintCredit)
    {
        if (version != Kagemusha.DeviceLifecycleVersion)
            throw new ArgumentException("KAGEMUSHA V1 device command version is invalid.", nameof(version));
        RequireNestedBound(canonicalAuthorization, Kagemusha.MaximumMintAuthorizationBytes,
            nameof(canonicalAuthorization));
        RequireNestedBound(canonicalMintCredit, Kagemusha.MaximumMintCreditBytes,
            nameof(canonicalMintCredit));
        Version = version;
        this.canonicalAuthorization = canonicalAuthorization.ToArray();
        this.canonicalMintCredit = canonicalMintCredit.ToArray();
    }

    /// <summary>The sole supported secure-device lifecycle version.</summary>
    public ushort Version { get; }

    /// <summary>A defensive copy of the exact canonical pre-debit authorization archive.</summary>
    public ReadOnlyMemory<byte> CanonicalAuthorization => canonicalAuthorization.ToArray();

    /// <summary>A defensive copy of the exact canonical finalized mint-credit archive.</summary>
    public ReadOnlyMemory<byte> CanonicalMintCredit => canonicalMintCredit.ToArray();

    internal ReadOnlySpan<byte> CanonicalAuthorizationSpan => canonicalAuthorization;
    internal ReadOnlySpan<byte> CanonicalMintCreditSpan => canonicalMintCredit;

    private static void RequireNestedBound(ReadOnlySpan<byte> value, int maximum, string name)
    {
        if (value.IsEmpty || value.Length > maximum)
            throw new ArgumentException("KAGEMUSHA V1 nested archive is empty or oversized.", name);
    }
}

/// <summary>
/// Bounded public operation-16 result. The private Guard certificate and authenticated device
/// response remain mandatory; this value alone is not evidence of a durable hardware stage.
/// </summary>
public sealed class KagemushaDeviceMintStageResultV1
{
    /// <summary>A previously unseen finalized credit was durably staged.</summary>
    public const byte Staged = 0;

    /// <summary>The exact canonical credit was already pending or consumed.</summary>
    public const byte ExactDuplicate = 1;

    private readonly byte[] creditId;

    /// <summary>Construct a closed-discriminant result with a defensively owned credit identity.</summary>
    public KagemushaDeviceMintStageResultV1(ushort version, byte disposition, ReadOnlySpan<byte> creditId)
    {
        if (version != Kagemusha.DeviceLifecycleVersion)
            throw new ArgumentException("KAGEMUSHA V1 device result version is invalid.", nameof(version));
        if (disposition is not Staged and not ExactDuplicate)
            throw new ArgumentException("KAGEMUSHA V1 mint-stage disposition is invalid.", nameof(disposition));
        Version = version;
        Disposition = disposition;
        this.creditId = KagemushaModelValidation.Fixed32(creditId, nameof(creditId));
    }

    /// <summary>The sole supported secure-device lifecycle version.</summary>
    public ushort Version { get; }

    /// <summary>Zero for a new stage, one for an exact pending or consumed duplicate.</summary>
    public byte Disposition { get; }

    /// <summary>A defensive copy of the nonzero finalized-credit identity.</summary>
    public ReadOnlyMemory<byte> CreditId => creditId.ToArray();

    internal ReadOnlySpan<byte> CreditIdSpan => creditId;
}
