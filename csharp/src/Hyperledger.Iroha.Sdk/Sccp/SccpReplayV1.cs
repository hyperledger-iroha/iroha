using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;

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
    TonWalletBurnAuthorization = 0x35,
    TonWalletBurnLock = 0x36,
    TonWalletBurnRefund = 0x37,
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

    /// <summary>Construct from exact canonical Norito <c>AccountId</c> bytes.</summary>
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
        ExpectedShardRoot = SccpReplayV1.Exact(
            expectedShardRoot, 32, "expected shard root", nonzero: false);
        PriorRecordDigest = SccpReplayV1.Exact(
            priorRecordDigest, 32, "prior record digest", nonzero: false);
        SiblingBitmap = SccpReplayV1.Exact(
            siblingBitmap, 32, "sibling bitmap", nonzero: false);
        Siblings = siblings.Select((value, index) =>
            SccpReplayV1.Exact(value, 32, $"sibling[{index}]", nonzero: false)).ToArray();
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
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

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
        if (!Enum.IsDefined(typeof(SccpNetworkV1), source)
            || !Enum.IsDefined(typeof(SccpNetworkV1), target)
            || !Enum.IsDefined(typeof(SccpReplayBoundaryV1), boundary)
            || !IsProduction(source)
            || !IsProduction(target)
            || routeRevision == 0)
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
        if (!Enum.IsDefined(typeof(SccpReplayBoundaryV1), operation)
            || amountScale9 == 0)
        {
            throw new ArgumentException("Replay operation and amount must be canonical.");
        }
        if (principal.Kind != PrincipalKindForBoundary(operation))
        {
            throw new ArgumentException("Replay operation and principal kind are inconsistent.");
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
        var digest = Hash(
            Magic,
            [2, (byte)operation],
            Exact(replayId, 32, "replay id"),
            Exact(payloadSha256, 32, "payload SHA-256"),
            UnsignedBigEndian(amountScale9),
            principalDigest,
            auxiliaryDigest);
        if (IsZero(digest))
        {
            throw new ArgumentException("Occupied replay record digest must be nonzero.");
        }
        return digest;
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
        ReadOnlySpan<byte> recordDigest,
        SccpSparseMerkleWitnessV1 witness)
    {
        ArgumentNullException.ThrowIfNull(witness);
        var key = Exact(keyValue, 32, "replay key", nonzero: false);
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
        var digest = Exact(recordDigest, 32, "record digest", nonzero: false);
        if (!digest.AsSpan().SequenceEqual(witness.PriorRecordDigest))
        {
            throw new ArgumentException("Witness record digest does not match.");
        }
        var current = IsZero(digest)
            ? empty[0]
            : Hash(Magic, [0x11], key, digest);

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

    /// <summary>Verify a replay witness against the caller's exact current shard root.</summary>
    public static SccpReplayWitnessRootV1 VerifyAgainstCurrentRoot(
        ReadOnlySpan<byte> keyValue,
        ReadOnlySpan<byte> recordDigest,
        SccpSparseMerkleWitnessV1 witness,
        ReadOnlySpan<byte> currentRootValue)
    {
        var currentRoot = Exact(currentRootValue, 32, "current shard root", nonzero: false);
        var reconstructed = RootFromWitness(keyValue, recordDigest, witness);
        if (!reconstructed.ExpectedRoot.AsSpan().SequenceEqual(currentRoot)
            || !reconstructed.Root.AsSpan().SequenceEqual(currentRoot))
        {
            throw new ArgumentException("Replay witness does not match the current shard root.");
        }
        return reconstructed;
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

    internal static byte[] CanonicalSoraAccountId(ReadOnlySpan<byte> payload)
    {
        if (payload.IsEmpty || payload.Length > ushort.MaxValue)
        {
            throw new ArgumentException(
                "SORA replay principal must be canonical nonempty AccountId bytes.");
        }

        var reader = new CanonicalNoritoReader(
            payload,
            "SORA replay principal",
            nameof(payload));
        var controllerTag = reader.ReadUInt32LittleEndian("controller");
        var controllerPayload = reader.ReadField("controller_payload");
        var canonicalController = controllerTag switch
        {
            0 => CanonicalSingleController(controllerPayload),
            1 => CanonicalMultisigController(controllerPayload),
            _ => throw new ArgumentException(
                "SORA replay principal uses an unknown AccountId controller tag.",
                nameof(payload)),
        };
        reader.RequireEnd();
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(controllerTag);
        writer.WriteField(canonicalController);
        var canonical = writer.ToArray();
        if (!canonical.AsSpan().SequenceEqual(payload))
        {
            throw new ArgumentException(
                "SORA replay principal is not the canonical AccountId encoding.");
        }
        return canonical;
    }

    private static byte[] CanonicalSingleController(ReadOnlySpan<byte> payload)
    {
        var reader = new CanonicalNoritoReader(
            payload,
            "SORA replay principal single-key controller",
            nameof(payload));
        var publicKey = ReadCanonicalCompactPublicKey(ref reader, "public_key");
        reader.RequireEnd();
        return publicKey.Encoded;
    }

    private static byte[] CanonicalMultisigController(ReadOnlySpan<byte> payload)
    {
        var reader = new CanonicalNoritoReader(
            payload,
            "SORA replay principal multisig controller",
            nameof(payload));
        var version = reader.ReadByte("version");
        var threshold = BinaryPrimitives.ReadUInt16LittleEndian(
            reader.ReadExact(sizeof(ushort), "threshold"));
        var memberCount = reader.ReadSequenceLength("members");
        if (version != 1 || threshold == 0 || memberCount is 0 or > ushort.MaxValue)
        {
            throw new ArgumentException(
                "SORA replay principal multisig controller has an invalid version, threshold, or member count.",
                nameof(payload));
        }

        uint totalWeight = 0;
        byte[]? previousSortKey = null;
        var canonicalMembers = new List<byte[]>(checked((int)memberCount));
        for (var index = 0UL; index < memberCount; index++)
        {
            var memberPayload = reader.ReadField($"members[{index}]");
            var memberReader = new CanonicalNoritoReader(
                memberPayload,
                $"SORA replay principal multisig member {index}",
                nameof(payload));
            var publicKey = ReadCanonicalCompactPublicKey(ref memberReader, "public_key");
            var weight = BinaryPrimitives.ReadUInt16LittleEndian(
                memberReader.ReadExact(sizeof(ushort), "weight"));
            memberReader.RequireEnd();
            var sortKey = CompactPublicKeySortKey(publicKey.Raw);
            if (weight == 0
                || previousSortKey is not null
                    && previousSortKey.AsSpan().SequenceCompareTo(sortKey) >= 0)
            {
                throw new ArgumentException(
                    "SORA replay principal multisig members must have nonzero weights and be unique and canonically sorted.",
                    nameof(payload));
            }

            totalWeight = checked(totalWeight + weight);
            previousSortKey = sortKey;
            var canonicalMember = new CanonicalNoritoWriter();
            canonicalMember.WriteBytes(publicKey.Encoded);
            canonicalMember.WriteUInt16LittleEndian(weight);
            canonicalMembers.Add(canonicalMember.ToArray());
        }

        reader.RequireEnd();
        if (totalWeight < threshold)
        {
            throw new ArgumentException(
                "SORA replay principal multisig threshold exceeds total member weight.",
                nameof(payload));
        }

        var canonical = new CanonicalNoritoWriter();
        canonical.WriteByte(version);
        canonical.WriteUInt16LittleEndian(threshold);
        canonical.WriteSequenceLength(memberCount);
        foreach (var member in canonicalMembers)
        {
            canonical.WriteField(member);
        }
        return canonical.ToArray();
    }

    private static (byte[] Encoded, byte[] Raw) ReadCanonicalCompactPublicKey(
        ref CanonicalNoritoReader reader,
        string field)
    {
        var byteCount = reader.ReadSequenceLength($"{field}.bytes");
        if (byteCount is 0 or > ushort.MaxValue)
        {
            throw new ArgumentException($"{field} length is invalid.", nameof(reader));
        }

        var raw = new byte[checked((int)byteCount)];
        for (var index = 0; index < raw.Length; index++)
        {
            var element = reader.ReadField($"{field}[{index}]");
            if (element.Length != 1)
            {
                throw new ArgumentException(
                    $"{field} byte element is not canonically framed.",
                    nameof(reader));
            }
            raw[index] = element[0];
        }
        ValidateCompactPublicKey(raw, field);

        var canonical = new CanonicalNoritoWriter();
        canonical.WriteSequenceLength(byteCount);
        canonical.WriteByteElements(raw);
        return (canonical.ToArray(), raw);
    }

    private static void ValidateCompactPublicKey(ReadOnlySpan<byte> compact, string field)
    {
        if (compact.Length < 2)
        {
            throw new ArgumentException($"{field} is not a compact public key.", nameof(compact));
        }

        var payload = compact[1..];
        var expectedLength = compact[0] switch
        {
            0 => 32,
            1 => 33,
            2 => 48,
            3 => 96,
            4 => 1_952,
            5 or 6 or 7 => 64,
            8 or 9 => 128,
            10 => Sm2PayloadLength(payload, field),
            _ => throw new ArgumentException($"{field} uses an unknown public-key algorithm.", nameof(compact)),
        };
        if (payload.Length != expectedLength || IsZero(payload))
        {
            throw new ArgumentException($"{field} has an invalid public-key envelope.", nameof(compact));
        }
        if (compact[0] == 1 && payload[0] is not (0x02 or 0x03))
        {
            throw new ArgumentException($"{field} has an invalid secp256k1 public-key envelope.", nameof(compact));
        }
    }

    private static int Sm2PayloadLength(ReadOnlySpan<byte> payload, string field)
    {
        const int lengthPrefixBytes = 2;
        const int sec1Bytes = 65;
        if (payload.Length < lengthPrefixBytes)
        {
            throw new ArgumentException($"{field} has a truncated SM2 public-key envelope.", nameof(payload));
        }

        var identifierLength = BinaryPrimitives.ReadUInt16BigEndian(payload[..lengthPrefixBytes]);
        if (identifierLength > ushort.MaxValue / 8)
        {
            throw new ArgumentException($"{field} has an oversized SM2 identifier.", nameof(payload));
        }
        var sec1Offset = checked(lengthPrefixBytes + identifierLength);
        var expectedLength = checked(sec1Offset + sec1Bytes);
        if (payload.Length != expectedLength || payload[sec1Offset] != 0x04)
        {
            throw new ArgumentException($"{field} has an invalid SM2 public-key envelope.", nameof(payload));
        }
        try
        {
            _ = StrictUtf8.GetString(payload[lengthPrefixBytes..sec1Offset]);
        }
        catch (DecoderFallbackException error)
        {
            throw new ArgumentException($"{field} has a non-UTF-8 SM2 identifier.", nameof(payload), error);
        }
        return expectedLength;
    }

    private static byte[] CompactPublicKeySortKey(ReadOnlySpan<byte> compact)
    {
        var algorithm = compact[0] switch
        {
            0 => "ed25519",
            1 => "secp256k1",
            2 => "bls_normal",
            3 => "bls_small",
            4 => "ml-dsa",
            5 => "gost3410-2012-256-paramset-a",
            6 => "gost3410-2012-256-paramset-b",
            7 => "gost3410-2012-256-paramset-c",
            8 => "gost3410-2012-512-paramset-a",
            9 => "gost3410-2012-512-paramset-b",
            10 => "sm2",
            _ => throw new ArgumentException("Public-key algorithm is unknown.", nameof(compact)),
        };
        var prefix = StrictUtf8.GetBytes(algorithm);
        var sortKey = new byte[prefix.Length + compact.Length];
        prefix.CopyTo(sortKey, 0);
        // Rust orders members by (algorithm.as_static_str(), raw_payload).
        // A zero separator preserves that tuple order because every algorithm
        // label is nonempty ASCII and public-key payloads follow it verbatim.
        compact[1..].CopyTo(sortKey.AsSpan(prefix.Length + 1));
        return sortKey;
    }

    private static byte PrincipalKindForBoundary(SccpReplayBoundaryV1 boundary) => boundary switch
    {
        SccpReplayBoundaryV1.SoraOutboundLock or
        SccpReplayBoundaryV1.SoraInboundRelease => 0,
        SccpReplayBoundaryV1.EvmSourceBurn or
        SccpReplayBoundaryV1.EvmDestinationMint => 1,
        SccpReplayBoundaryV1.TronSourceBurn or
        SccpReplayBoundaryV1.TronDestinationMint => 2,
        SccpReplayBoundaryV1.TonBridgeInboundMint or
        SccpReplayBoundaryV1.TonBridgeOutboundBurn or
        SccpReplayBoundaryV1.TonMasterMint or
        SccpReplayBoundaryV1.TonMasterBurn or
        SccpReplayBoundaryV1.TonWalletMintCredit or
        SccpReplayBoundaryV1.TonWalletBurnAuthorization or
        SccpReplayBoundaryV1.TonWalletBurnLock or
        SccpReplayBoundaryV1.TonWalletBurnRefund => 3,
        _ => throw new ArgumentOutOfRangeException(nameof(boundary)),
    };

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
        SccpReplayBoundaryV1.TonWalletMintCredit =>
            source == SccpNetworkV1.SoraTaira && target == SccpNetworkV1.TonMainnet && actorKind == 3,
        SccpReplayBoundaryV1.TonBridgeOutboundBurn or
        SccpReplayBoundaryV1.TonMasterBurn or
        SccpReplayBoundaryV1.TonWalletBurnAuthorization or
        SccpReplayBoundaryV1.TonWalletBurnLock or
        SccpReplayBoundaryV1.TonWalletBurnRefund =>
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
