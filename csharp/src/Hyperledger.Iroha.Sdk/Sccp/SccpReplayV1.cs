using System.Buffers.Binary;
using System.Security.Cryptography;

namespace Hyperledger.Iroha.Sccp;

/// <summary>Closed SCCP replay-boundary tags shared by SORA and destination runtimes.</summary>
public enum SccpReplayBoundaryV1 : byte
{
    SoraOutboundLock = 0x01,
    SoraInboundRelease = 0x02,
    EvmSourceBurn = 0x10,
    EvmDestinationMint = 0x11,
    TronSourceBurn = 0x20,
    TronDestinationMint = 0x21,
    TonBridgeInboundMint = 0x30,
    TonBridgeOutboundBurn = 0x31,
    TonMasterMint = 0x32,
    TonMasterBurn = 0x33,
    TonWalletMintCredit = 0x34,
    TonWalletBurnDebit = 0x35,
    TonWalletRefundDebit = 0x36,
    TonWalletRefundCredit = 0x37,
}

/// <summary>Canonical contract identity committed by one replay domain.</summary>
public sealed class SccpReplayActorV1
{
    private SccpReplayActorV1(byte kind, byte[] bytes)
    {
        Kind = kind;
        Bytes = bytes.ToArray();
    }

    internal byte Kind { get; }

    internal byte[] Bytes { get; }

    public static SccpReplayActorV1 Route() => new(0, []);

    public static SccpReplayActorV1 Evm(ReadOnlySpan<byte> address) =>
        new(1, SccpReplayV1.Exact(address, 20, "EVM replay actor"));

    public static SccpReplayActorV1 Tron(ReadOnlySpan<byte> address) =>
        new(2, SccpReplayV1.Exact(address, 20, "TRON replay actor"));

    public static SccpReplayActorV1 Ton(int workchain, ReadOnlySpan<byte> account) =>
        new(3, SccpReplayV1.Concat(SccpReplayV1.SignedI32BigEndian(workchain),
            SccpReplayV1.Exact(account, 32, "TON replay actor")));
}

/// <summary>Canonical economic principal committed by an occupied replay leaf.</summary>
public sealed class SccpReplayPrincipalV1
{
    private SccpReplayPrincipalV1(byte kind, byte[] bytes)
    {
        if (bytes.Length is 0 or > ushort.MaxValue)
        {
            throw new ArgumentException("Replay principal must have a canonical nonempty u16-sized representation.");
        }

        Kind = kind;
        Bytes = bytes.ToArray();
    }

    internal byte Kind { get; }

    internal byte[] Bytes { get; }

    /// <summary>Construct from exact canonical compact-Norito <c>AccountId</c> bytes.</summary>
    public static SccpReplayPrincipalV1 SoraAccount(ReadOnlySpan<byte> canonicalAccountId) =>
        new(0, SccpReplayV1.CanonicalSoraAccountId(canonicalAccountId));

    public static SccpReplayPrincipalV1 Evm(ReadOnlySpan<byte> address) =>
        new(1, SccpReplayV1.Exact(address, 20, "EVM replay principal"));

    public static SccpReplayPrincipalV1 Tron(ReadOnlySpan<byte> address) =>
        new(2, SccpReplayV1.Exact(address, 20, "TRON replay principal"));

    public static SccpReplayPrincipalV1 Ton(int workchain, ReadOnlySpan<byte> account) =>
        new(3, SccpReplayV1.Concat(SccpReplayV1.SignedI32BigEndian(workchain),
            SccpReplayV1.Exact(account, 32, "TON replay principal")));
}

/// <summary>Canonically compressed 248-level sparse-Merkle witness.</summary>
public sealed class SccpSparseMerkleWitnessV1
{
    public SccpSparseMerkleWitnessV1(
        ReadOnlySpan<byte> expectedShardRoot,
        ReadOnlySpan<byte> priorRecordDigest,
        ReadOnlySpan<byte> siblingBitmap,
        IEnumerable<byte[]> siblings)
    {
        ArgumentNullException.ThrowIfNull(siblings);
        ExpectedShardRoot = SccpReplayV1.Exact(expectedShardRoot, 32, "expected shard root");
        PriorRecordDigest = SccpReplayV1.Exact(
            priorRecordDigest, 32, "prior record digest", nonzero: false);
        SiblingBitmap = SccpReplayV1.Exact(
            siblingBitmap, 32, "sibling bitmap", nonzero: false);
        Siblings = siblings.Select((value, index) =>
            SccpReplayV1.Exact(value, 32, $"sibling[{index}]")).ToArray();
    }

    internal byte[] ExpectedShardRoot { get; }

    internal byte[] PriorRecordDigest { get; }

    internal byte[] SiblingBitmap { get; }

    internal byte[][] Siblings { get; }
}

/// <summary>Result of reconstructing one canonical replay witness.</summary>
public sealed class SccpReplayWitnessRootV1
{
    private readonly byte[] root;
    private readonly byte[] expectedRoot;

    internal SccpReplayWitnessRootV1(byte[] root, byte[] expectedRoot, byte shard)
    {
        this.root = root.ToArray();
        this.expectedRoot = expectedRoot.ToArray();
        Shard = shard;
    }

    public byte[] Root => root.ToArray();

    public byte[] ExpectedRoot => expectedRoot.ToArray();

    public byte Shard { get; }

    public bool MatchesExpectedRoot => root.AsSpan().SequenceEqual(expectedRoot);
}

/// <summary>Canonical SHA-256 sparse-Merkle replay hashing for SCCP final V1.</summary>
public static class SccpReplayV1
{
    public const int Depth = 248;

    private static readonly byte[] Magic = "SCCP-REPLAY-SMT-V1"u8.ToArray();

    /// <summary>Hash one complete production replay domain.</summary>
    public static byte[] DomainHash(
        SccpNetworkV1 source,
        SccpNetworkV1 target,
        SccpReplayBoundaryV1 boundary,
        uint routeRevision,
        ReadOnlySpan<byte> routeConfigurationHash,
        SccpReplayActorV1 actor)
    {
        ArgumentNullException.ThrowIfNull(actor);
        if (!IsProduction(source) || !IsProduction(target) || routeRevision == 0)
        {
            throw new ArgumentException("Replay domains require production networks and a nonzero revision.");
        }
        if (!ValidDirection(source, target, boundary, actor.Kind))
        {
            throw new ArgumentException("Replay boundary, network direction, and actor are inconsistent.");
        }

        return Hash(
            Magic,
            [0],
            UnsignedBigEndian((uint)source),
            UnsignedBigEndian((uint)target),
            [(byte)boundary],
            UnsignedBigEndian(routeRevision),
            Exact(routeConfigurationHash, 32, "route configuration hash"),
            [actor.Kind],
            UnsignedBigEndian((ushort)actor.Bytes.Length),
            actor.Bytes);
    }

    /// <summary>Derive the full replay key; byte zero selects one of 256 shards.</summary>
    public static byte[] ReplayKey(ReadOnlySpan<byte> domainHash, ReadOnlySpan<byte> replayId) =>
        Hash(Magic, [1], Exact(domainHash, 32, "domain hash"),
            Exact(replayId, 32, "replay id"));

    /// <summary>Hash one canonical occupied replay record with a positive scale-9 u128 amount.</summary>
    public static byte[] RecordDigest(
        SccpReplayBoundaryV1 operation,
        ReadOnlySpan<byte> replayId,
        ReadOnlySpan<byte> payloadSha256,
        UInt128 amountScale9,
        SccpReplayPrincipalV1 principal,
        ReadOnlySpan<byte> auxiliaryIdentitySha256)
    {
        ArgumentNullException.ThrowIfNull(principal);
        if (!Enum.IsDefined(typeof(SccpReplayBoundaryV1), operation))
        {
            throw new ArgumentOutOfRangeException(
                nameof(operation), operation, "Unknown SCCP replay boundary.");
        }
        if (amountScale9 == 0)
        {
            throw new ArgumentException("Replay amount must be a positive u128.", nameof(amountScale9));
        }
        var principalDigest = Hash(
            Magic,
            [3, principal.Kind],
            UnsignedBigEndian((ushort)principal.Bytes.Length),
            principal.Bytes);
        var auxiliaryDigest = Hash(
            Magic,
            [4, (byte)operation],
            Exact(auxiliaryIdentitySha256, 32, "auxiliary identity SHA-256"));
        return Hash(
            Magic,
            [2, (byte)operation],
            Exact(replayId, 32, "replay id"),
            Exact(payloadSha256, 32, "payload SHA-256"),
            UnsignedBigEndian(amountScale9),
            principalDigest,
            auxiliaryDigest);
    }

    /// <summary>Return all 249 canonical empty hashes in leaf-up order.</summary>
    public static IReadOnlyList<byte[]> EmptyHashes()
    {
        var hashes = new List<byte[]>(Depth + 1) { Hash(Magic, [0x10]) };
        for (var level = 0; level < Depth; level++)
        {
            hashes.Add(Parent(level, hashes[level], hashes[level]));
        }
        return hashes.Select(value => value.ToArray()).ToArray();
    }

    /// <summary>Strictly reconstruct a compressed membership or non-membership witness.</summary>
    public static SccpReplayWitnessRootV1 RootFromWitness(
        ReadOnlySpan<byte> keyValue,
        byte[]? recordDigest,
        SccpSparseMerkleWitnessV1 witness)
    {
        ArgumentNullException.ThrowIfNull(witness);
        var key = Exact(keyValue, 32, "replay key");
        if (witness.SiblingBitmap[0] != 0)
        {
            throw new ArgumentException("Witness bitmap has reserved high bits.");
        }
        var setBits = witness.SiblingBitmap.Sum(CountBits);
        if (setBits != witness.Siblings.Length || setBits > Depth)
        {
            throw new ArgumentException("Witness sibling count does not match its bitmap.");
        }

        var empty = EmptyHashes();
        byte[] current;
        if (recordDigest is null)
        {
            if (!IsZero(witness.PriorRecordDigest))
            {
                throw new ArgumentException("Non-membership witness has an occupied digest.");
            }
            current = empty[0];
        }
        else
        {
            var digest = Exact(recordDigest, 32, "record digest");
            if (!digest.AsSpan().SequenceEqual(witness.PriorRecordDigest))
            {
                throw new ArgumentException("Membership witness record digest does not match.");
            }
            current = Hash(Magic, [0x11], key, digest);
        }

        var supplied = 0;
        for (var level = 0; level < Depth; level++)
        {
            var sibling = empty[level];
            if (Bit(witness.SiblingBitmap, level))
            {
                sibling = witness.Siblings[supplied++];
                if (sibling.AsSpan().SequenceEqual(empty[level]))
                {
                    throw new ArgumentException("Witness explicitly encodes a default sibling.");
                }
            }
            current = Bit(key, level)
                ? Parent(level, sibling, current)
                : Parent(level, current, sibling);
        }
        return new SccpReplayWitnessRootV1(current, witness.ExpectedShardRoot, key[0]);
    }

    internal static byte[] CanonicalSoraAccountId(ReadOnlySpan<byte> canonicalAccountId)
    {
        if (canonicalAccountId.IsEmpty || canonicalAccountId.Length > ushort.MaxValue)
        {
            throw new ArgumentException(
                "SORA replay principal must be canonical nonempty u16-sized AccountId bytes.",
                nameof(canonicalAccountId));
        }

        try
        {
            return SccpSubmitValidation.RequireCanonicalAccountIdPayload(canonicalAccountId);
        }
        catch (Exception error) when (error is ArgumentException or FormatException or OverflowException)
        {
            throw new ArgumentException(
                "SORA replay principal is not a canonical compact-Norito AccountId.",
                nameof(canonicalAccountId),
                error);
        }
    }

    internal static byte[] Exact(
        ReadOnlySpan<byte> value,
        int length,
        string label,
        bool nonzero = true)
    {
        if (value.Length != length || (nonzero && IsZero(value)))
        {
            throw new ArgumentException($"{label} must be {(nonzero ? "nonzero " : string.Empty)}{length} bytes.");
        }
        return value.ToArray();
    }

    internal static byte[] Concat(params byte[][] parts)
    {
        var size = parts.Sum(part => part.Length);
        var result = new byte[size];
        var offset = 0;
        foreach (var part in parts)
        {
            part.CopyTo(result, offset);
            offset += part.Length;
        }
        return result;
    }

    internal static byte[] SignedI32BigEndian(int value)
    {
        var result = new byte[sizeof(int)];
        BinaryPrimitives.WriteInt32BigEndian(result, value);
        return result;
    }

    private static bool ValidDirection(
        SccpNetworkV1 source,
        SccpNetworkV1 target,
        SccpReplayBoundaryV1 boundary,
        byte actorKind) => boundary switch
    {
        SccpReplayBoundaryV1.SoraOutboundLock =>
            source == SccpNetworkV1.SoraTaira && IsExternalMainnet(target) && actorKind == 0,
        SccpReplayBoundaryV1.SoraInboundRelease =>
            IsExternalMainnet(source) && target == SccpNetworkV1.SoraTaira && actorKind == 0,
        SccpReplayBoundaryV1.EvmSourceBurn =>
            IsEvm(source) && target == SccpNetworkV1.SoraTaira && actorKind == 1,
        SccpReplayBoundaryV1.EvmDestinationMint =>
            source == SccpNetworkV1.SoraTaira && IsEvm(target) && actorKind == 1,
        SccpReplayBoundaryV1.TronSourceBurn =>
            source == SccpNetworkV1.TronMainnet && target == SccpNetworkV1.SoraTaira && actorKind == 2,
        SccpReplayBoundaryV1.TronDestinationMint =>
            source == SccpNetworkV1.SoraTaira && target == SccpNetworkV1.TronMainnet && actorKind == 2,
        SccpReplayBoundaryV1.TonBridgeInboundMint or
        SccpReplayBoundaryV1.TonMasterMint or
        SccpReplayBoundaryV1.TonWalletMintCredit or
        SccpReplayBoundaryV1.TonWalletRefundDebit or
        SccpReplayBoundaryV1.TonWalletRefundCredit =>
            source == SccpNetworkV1.SoraTaira && target == SccpNetworkV1.TonMainnet && actorKind == 3,
        SccpReplayBoundaryV1.TonBridgeOutboundBurn or
        SccpReplayBoundaryV1.TonMasterBurn or
        SccpReplayBoundaryV1.TonWalletBurnDebit =>
            source == SccpNetworkV1.TonMainnet && target == SccpNetworkV1.SoraTaira && actorKind == 3,
        _ => false,
    };

    private static bool IsProduction(SccpNetworkV1 network) =>
        network == SccpNetworkV1.SoraTaira || IsExternalMainnet(network);

    private static bool IsExternalMainnet(SccpNetworkV1 network) =>
        IsEvm(network) || network is SccpNetworkV1.TronMainnet or SccpNetworkV1.TonMainnet;

    private static bool IsEvm(SccpNetworkV1 network) =>
        network is SccpNetworkV1.EthereumMainnet or SccpNetworkV1.BscMainnet;

    private static byte[] Parent(int level, byte[] left, byte[] right) =>
        Hash(Magic, [0x12], UnsignedBigEndian((ushort)level), left, right);

    private static bool Bit(ReadOnlySpan<byte> value, int level) =>
        (value[31 - level / 8] & (1 << (level % 8))) != 0;

    private static int CountBits(byte value)
    {
        var count = 0;
        while (value != 0)
        {
            count += value & 1;
            value >>= 1;
        }
        return count;
    }

    private static bool IsZero(ReadOnlySpan<byte> value)
    {
        foreach (var item in value)
        {
            if (item != 0)
            {
                return false;
            }
        }
        return true;
    }

    private static byte[] UnsignedBigEndian(ushort value)
    {
        var result = new byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16BigEndian(result, value);
        return result;
    }

    private static byte[] UnsignedBigEndian(uint value)
    {
        var result = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32BigEndian(result, value);
        return result;
    }

    private static byte[] UnsignedBigEndian(UInt128 value)
    {
        var result = new byte[16];
        for (var index = result.Length - 1; index >= 0; index--)
        {
            result[index] = (byte)value;
            value >>= 8;
        }
        return result;
    }

    private static byte[] Hash(params byte[][] parts)
    {
        using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        foreach (var part in parts)
        {
            hash.AppendData(part);
        }
        return hash.GetHashAndReset();
    }
}
