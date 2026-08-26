using System.Buffers;
using System.Buffers.Binary;
using System.Numerics;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

public sealed class ToriiAccountFaucetSolveOptions
{
    public ulong StartNonce { get; init; }

    public int MaxAttempts { get; init; } = 1_000_000;

    public int NonceByteCount { get; init; } = sizeof(ulong);
}

public sealed record class ToriiAccountFaucetSolution
{
    public string AccountId { get; init; } = string.Empty;

    public ulong AnchorHeight { get; init; }

    public string NonceHex { get; init; } = string.Empty;

    public string DigestHex { get; init; } = string.Empty;

    public int LeadingZeroBits { get; init; }

    public byte DifficultyBits { get; init; }

    public int Attempts { get; init; }

    public ToriiAccountFaucetClaimV1 ToClaim()
    {
        return new ToriiAccountFaucetClaimV1
        {
            AccountId = AccountId,
            PowAnchorHeight = AnchorHeight,
            PowNonceHex = NonceHex,
        };
    }
}

public static class ToriiAccountFaucetPow
{
    public const string Algorithm = "scrypt-leading-zero-bits-v1";

    internal const long MaxScryptRomixMemoryBytes = 64L * 1024 * 1024;
    internal const uint MaxScryptParallelization = 16;

    private static readonly byte[] DomainSeparator = Encoding.ASCII.GetBytes("iroha:accounts:faucet:pow:v1");

    public static byte[] ComputeChallenge(
        string accountId,
        NetworkId networkId,
        ushort chainDiscriminant,
        ulong anchorHeight,
        string anchorBlockHashHex,
        string? challengeSaltHex = null)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        var exactAccountId = RequireExactAccountId(
            accountId,
            nameof(accountId),
            chainDiscriminant);
        if (anchorHeight == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(anchorHeight), "Anchor height must be positive.");
        }

        var anchorHash = Convert.FromHexString(
            ToriiExplorerDirectMetadata.RequireExactSizedHex(
                anchorBlockHashHex,
                nameof(anchorBlockHashHex),
                32));
        var challengeSalt = challengeSaltHex is null
            ? Array.Empty<byte>()
            : Convert.FromHexString(
                ToriiExplorerDirectMetadata.RequireExactSizedHex(
                    challengeSaltHex,
                    nameof(challengeSaltHex),
                    32));
        var accountBytes = Encoding.UTF8.GetBytes(exactAccountId);
        var payload = new byte[
            DomainSeparator.Length
            + NetworkId.ByteLength
            + accountBytes.Length
            + sizeof(ulong)
            + anchorHash.Length
            + challengeSalt.Length];
        var offset = 0;

        DomainSeparator.CopyTo(payload, offset);
        offset += DomainSeparator.Length;

        networkId.AsSpan().CopyTo(payload.AsSpan(offset, NetworkId.ByteLength));
        offset += NetworkId.ByteLength;

        accountBytes.CopyTo(payload, offset);
        offset += accountBytes.Length;

        BinaryPrimitives.WriteUInt64BigEndian(payload.AsSpan(offset, sizeof(ulong)), anchorHeight);
        offset += sizeof(ulong);

        anchorHash.CopyTo(payload, offset);
        offset += anchorHash.Length;

        challengeSalt.CopyTo(payload, offset);
        return SHA256.HashData(payload);
    }

    public static byte[] ComputeChallenge(string accountId, ToriiAccountFaucetPuzzle puzzle)
    {
        ArgumentNullException.ThrowIfNull(puzzle);
        return ComputeChallenge(
            accountId,
            puzzle.NetworkId,
            puzzle.ChainDiscriminant,
            puzzle.AnchorHeight,
            puzzle.AnchorBlockHashHex,
            puzzle.ChallengeSaltHex);
    }

    public static byte[] ComputeDigest(
        ReadOnlySpan<byte> nonceBytes,
        ReadOnlySpan<byte> challenge,
        byte scryptLogN,
        uint scryptR,
        uint scryptP)
    {
        if (nonceBytes.IsEmpty || nonceBytes.Length > 32)
        {
            throw new ArgumentOutOfRangeException(nameof(nonceBytes), "Nonce bytes must be between 1 and 32 bytes.");
        }

        if (challenge.Length == 0)
        {
            throw new ArgumentException("Challenge cannot be empty.", nameof(challenge));
        }

        var (n, r, p) = CheckedScryptParameters(scryptLogN, scryptR, scryptP);

        return ManagedScrypt.DeriveKey(nonceBytes, challenge, n, r, p, dkLen: 32);
    }

    public static int CountLeadingZeroBits(ReadOnlySpan<byte> bytes)
    {
        var total = 0;
        foreach (var value in bytes)
        {
            if (value == 0)
            {
                total += 8;
                continue;
            }

            total += BitOperations.LeadingZeroCount(value) - 24;
            break;
        }

        return total;
    }

    public static bool Verify(
        string accountId,
        ToriiAccountFaucetPuzzle puzzle,
        string nonceHex,
        out int leadingZeroBits)
    {
        ArgumentNullException.ThrowIfNull(puzzle);
        RequirePositiveDifficulty(puzzle);
        var normalizedNonce = RequireExactHex(nonceHex, nameof(nonceHex));
        var nonceBytes = Convert.FromHexString(normalizedNonce);
        var challenge = ComputeChallenge(accountId, puzzle);
        var digest = ComputeDigest(nonceBytes, challenge, puzzle.ScryptLogN, puzzle.ScryptR, puzzle.ScryptP);
        leadingZeroBits = CountLeadingZeroBits(digest);
        return leadingZeroBits >= puzzle.DifficultyBits;
    }

    public static ToriiAccountFaucetSolution Solve(
        string accountId,
        ToriiAccountFaucetPuzzle puzzle,
        ToriiAccountFaucetSolveOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(puzzle);
        RequireSupportedAlgorithm(
            puzzle.Algorithm,
            $"{nameof(puzzle)}.{nameof(ToriiAccountFaucetPuzzle.Algorithm)}");
        RequirePositiveDifficulty(puzzle);
        var exactAccountId = RequireExactAccountId(
            accountId,
            nameof(accountId),
            puzzle.ChainDiscriminant);

        options ??= new ToriiAccountFaucetSolveOptions();
        if (options.MaxAttempts <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(options.MaxAttempts), "MaxAttempts must be positive.");
        }

        if (options.NonceByteCount <= 0 || options.NonceByteCount > 32)
        {
            throw new ArgumentOutOfRangeException(nameof(options.NonceByteCount), "NonceByteCount must be between 1 and 32.");
        }

        var maxAttemptOffset = (ulong)(options.MaxAttempts - 1);
        if (options.StartNonce > ulong.MaxValue - maxAttemptOffset)
        {
            throw new ArgumentOutOfRangeException(
                nameof(options.MaxAttempts),
                "MaxAttempts must not advance the nonce past UInt64.MaxValue from StartNonce.");
        }

        var challenge = ComputeChallenge(exactAccountId, puzzle);
        var nonceBytes = new byte[options.NonceByteCount];

        for (var attempt = 0; attempt < options.MaxAttempts; attempt++)
        {
            cancellationToken.ThrowIfCancellationRequested();

            var nonceValue = checked(options.StartNonce + (ulong)attempt);
            WriteNonceBytes(nonceValue, nonceBytes);
            var digest = ComputeDigest(nonceBytes, challenge, puzzle.ScryptLogN, puzzle.ScryptR, puzzle.ScryptP);
            var leadingZeroBits = CountLeadingZeroBits(digest);
            if (leadingZeroBits < puzzle.DifficultyBits)
            {
                continue;
            }

            return new ToriiAccountFaucetSolution
            {
                AccountId = exactAccountId,
                AnchorHeight = puzzle.AnchorHeight,
                NonceHex = Convert.ToHexString(nonceBytes).ToLowerInvariant(),
                DigestHex = Convert.ToHexString(digest).ToLowerInvariant(),
                LeadingZeroBits = leadingZeroBits,
                DifficultyBits = puzzle.DifficultyBits,
                Attempts = attempt + 1,
            };
        }

        throw new InvalidOperationException(
            $"No valid faucet PoW nonce was found within {options.MaxAttempts} attempts starting at {options.StartNonce}.");
    }

    private static void WriteNonceBytes(ulong nonceValue, Span<byte> destination)
    {
        destination.Clear();
        Span<byte> nonceBuffer = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64BigEndian(nonceBuffer, nonceValue);
        nonceBuffer[^destination.Length..].CopyTo(destination);
    }

    private static void RequirePositiveDifficulty(ToriiAccountFaucetPuzzle puzzle)
    {
        if (puzzle.DifficultyBits == 0)
        {
            throw new ArgumentOutOfRangeException(
                $"{nameof(puzzle)}.{nameof(ToriiAccountFaucetPuzzle.DifficultyBits)}",
                "Faucet PoW difficulty must be positive.");
        }
    }

    private static int CheckedWorkFactor(byte scryptLogN)
    {
        if (scryptLogN == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(scryptLogN), "Scrypt logN must be positive.");
        }

        if (scryptLogN >= 31)
        {
            throw new ArgumentOutOfRangeException(nameof(scryptLogN), "Scrypt logN must be less than 31.");
        }

        return 1 << scryptLogN;
    }

    internal static (int N, int R, int P) CheckedScryptParameters(
        byte scryptLogN,
        uint scryptR,
        uint scryptP)
    {
        var n = CheckedWorkFactor(scryptLogN);
        if (scryptR == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(scryptR), "Scrypt r must be positive.");
        }
        if (scryptR > int.MaxValue)
        {
            throw new ArgumentOutOfRangeException(nameof(scryptR), "Scrypt r is too large.");
        }
        if (scryptP == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(scryptP), "Scrypt p must be positive.");
        }
        if (scryptP > MaxScryptParallelization)
        {
            throw new ArgumentOutOfRangeException(
                nameof(scryptP),
                $"Scrypt p must be at most {MaxScryptParallelization}.");
        }
        if (scryptP > int.MaxValue)
        {
            throw new ArgumentOutOfRangeException(nameof(scryptP), "Scrypt p is too large.");
        }

        var r = (int)scryptR;
        var p = (int)scryptP;
        var romixMemoryBytes = checked(128L * r * n);
        if (romixMemoryBytes > MaxScryptRomixMemoryBytes)
        {
            throw new ArgumentOutOfRangeException(
                nameof(scryptLogN),
                $"Scrypt N and r require at most {MaxScryptRomixMemoryBytes} bytes of ROMix memory.");
        }

        return (n, r, p);
    }

    internal static string RequireExactAccountId(string? value, string paramName)
    {
        return RequireExactAccountId(value, paramName, AccountAddress.DefaultChainDiscriminant);
    }

    internal static string RequireExactAccountId(
        string? value,
        string paramName,
        ushort? chainDiscriminant)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Account id cannot be null or whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Account id must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Account id must not contain control characters.", paramName);
        }

        try
        {
            var address = AccountAddress.Parse(value, chainDiscriminant);
            return chainDiscriminant.HasValue
                ? address.ToI105(chainDiscriminant.Value)
                : value;
        }
        catch (AccountAddressException exception)
        {
            throw new ArgumentException("Account id must be a canonical I105 account id.", paramName, exception);
        }
    }

    internal static string RequireExactHex(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Hex value cannot be null or whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Hex value must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Hex value must not contain control characters.", paramName);
        }

        if ((value.Length & 1) != 0 || !value.All(Uri.IsHexDigit))
        {
            throw new ArgumentException("Hex value must contain an even number of hexadecimal characters.", paramName);
        }

        return value;
    }

    private static void RequireSupportedAlgorithm(string? value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Faucet PoW algorithm cannot be null or whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Faucet PoW algorithm must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Faucet PoW algorithm must not contain control characters.", paramName);
        }

        if (!string.Equals(value, Algorithm, StringComparison.Ordinal))
        {
            throw new NotSupportedException($"Unsupported faucet PoW algorithm `{value}`.");
        }
    }

    private static class ManagedScrypt
    {
        public static byte[] DeriveKey(
            ReadOnlySpan<byte> password,
            ReadOnlySpan<byte> salt,
            int n,
            int r,
            int p,
            int dkLen)
        {
            ValidateParameters(n, r, p, dkLen);

            var blockLength = checked(128 * r);
            var mixed = new byte[checked(blockLength * p)];
            Rfc2898DeriveBytes.Pbkdf2(password, salt, mixed, 1, HashAlgorithmName.SHA256);

            var v = ArrayPool<byte>.Shared.Rent(checked(blockLength * n));
            var xy = ArrayPool<byte>.Shared.Rent(checked(blockLength * 2));
            try
            {
                for (var i = 0; i < p; i++)
                {
                    Romix(mixed.AsSpan(i * blockLength, blockLength), v, xy, n, r);
                }

                var derivedKey = new byte[dkLen];
                Rfc2898DeriveBytes.Pbkdf2(password, mixed, derivedKey, 1, HashAlgorithmName.SHA256);
                return derivedKey;
            }
            finally
            {
                CryptographicOperations.ZeroMemory(v.AsSpan());
                CryptographicOperations.ZeroMemory(xy.AsSpan());
                ArrayPool<byte>.Shared.Return(v);
                ArrayPool<byte>.Shared.Return(xy);
            }
        }

        private static void ValidateParameters(int n, int r, int p, int dkLen)
        {
            if (n <= 1 || (n & (n - 1)) != 0)
            {
                throw new ArgumentOutOfRangeException(nameof(n), "Scrypt N must be a power of two greater than 1.");
            }

            if (r <= 0)
            {
                throw new ArgumentOutOfRangeException(nameof(r), "Scrypt r must be positive.");
            }

            if (p <= 0)
            {
                throw new ArgumentOutOfRangeException(nameof(p), "Scrypt p must be positive.");
            }

            if (dkLen <= 0)
            {
                throw new ArgumentOutOfRangeException(nameof(dkLen), "Derived key length must be positive.");
            }
        }

        private static void Romix(Span<byte> block, byte[] vBuffer, byte[] xyBuffer, int n, int r)
        {
            var blockLength = block.Length;
            var v = vBuffer.AsSpan(0, n * blockLength);
            var x = xyBuffer.AsSpan(0, blockLength);
            var y = xyBuffer.AsSpan(blockLength, blockLength);
            block.CopyTo(x);

            for (var i = 0; i < n; i++)
            {
                x.CopyTo(v.Slice(i * blockLength, blockLength));
                BlockMix(x, y, r);
                y.CopyTo(x);
            }

            for (var i = 0; i < n; i++)
            {
                var j = (int)(Integerify(x, r) & (ulong)(n - 1));
                Xor(x, v.Slice(j * blockLength, blockLength), y);
                BlockMix(y, x, r);
            }

            x.CopyTo(block);
        }

        private static ulong Integerify(ReadOnlySpan<byte> block, int r)
        {
            return BinaryPrimitives.ReadUInt64LittleEndian(block.Slice((2 * r - 1) * 64, sizeof(ulong)));
        }

        private static void BlockMix(ReadOnlySpan<byte> input, Span<byte> output, int r)
        {
            Span<byte> x = stackalloc byte[64];
            input.Slice((2 * r - 1) * 64, 64).CopyTo(x);

            for (var i = 0; i < 2 * r; i++)
            {
                XorInPlace(x, input.Slice(i * 64, 64));
                Salsa208(x);
                var destinationIndex = (i / 2 + (i % 2) * r) * 64;
                x.CopyTo(output.Slice(destinationIndex, 64));
            }
        }

        private static void Xor(ReadOnlySpan<byte> left, ReadOnlySpan<byte> right, Span<byte> destination)
        {
            for (var i = 0; i < left.Length; i++)
            {
                destination[i] = (byte)(left[i] ^ right[i]);
            }
        }

        private static void XorInPlace(Span<byte> left, ReadOnlySpan<byte> right)
        {
            for (var i = 0; i < left.Length; i++)
            {
                left[i] ^= right[i];
            }
        }

        private static void Salsa208(Span<byte> block)
        {
            Span<uint> state = stackalloc uint[16];
            Span<uint> working = stackalloc uint[16];
            for (var i = 0; i < 16; i++)
            {
                state[i] = BinaryPrimitives.ReadUInt32LittleEndian(block.Slice(i * 4, 4));
                working[i] = state[i];
            }

            for (var round = 0; round < 8; round += 2)
            {
                working[4] ^= BitOperations.RotateLeft(working[0] + working[12], 7);
                working[8] ^= BitOperations.RotateLeft(working[4] + working[0], 9);
                working[12] ^= BitOperations.RotateLeft(working[8] + working[4], 13);
                working[0] ^= BitOperations.RotateLeft(working[12] + working[8], 18);

                working[9] ^= BitOperations.RotateLeft(working[5] + working[1], 7);
                working[13] ^= BitOperations.RotateLeft(working[9] + working[5], 9);
                working[1] ^= BitOperations.RotateLeft(working[13] + working[9], 13);
                working[5] ^= BitOperations.RotateLeft(working[1] + working[13], 18);

                working[14] ^= BitOperations.RotateLeft(working[10] + working[6], 7);
                working[2] ^= BitOperations.RotateLeft(working[14] + working[10], 9);
                working[6] ^= BitOperations.RotateLeft(working[2] + working[14], 13);
                working[10] ^= BitOperations.RotateLeft(working[6] + working[2], 18);

                working[3] ^= BitOperations.RotateLeft(working[15] + working[11], 7);
                working[7] ^= BitOperations.RotateLeft(working[3] + working[15], 9);
                working[11] ^= BitOperations.RotateLeft(working[7] + working[3], 13);
                working[15] ^= BitOperations.RotateLeft(working[11] + working[7], 18);

                working[1] ^= BitOperations.RotateLeft(working[0] + working[3], 7);
                working[2] ^= BitOperations.RotateLeft(working[1] + working[0], 9);
                working[3] ^= BitOperations.RotateLeft(working[2] + working[1], 13);
                working[0] ^= BitOperations.RotateLeft(working[3] + working[2], 18);

                working[6] ^= BitOperations.RotateLeft(working[5] + working[4], 7);
                working[7] ^= BitOperations.RotateLeft(working[6] + working[5], 9);
                working[4] ^= BitOperations.RotateLeft(working[7] + working[6], 13);
                working[5] ^= BitOperations.RotateLeft(working[4] + working[7], 18);

                working[11] ^= BitOperations.RotateLeft(working[10] + working[9], 7);
                working[8] ^= BitOperations.RotateLeft(working[11] + working[10], 9);
                working[9] ^= BitOperations.RotateLeft(working[8] + working[11], 13);
                working[10] ^= BitOperations.RotateLeft(working[9] + working[8], 18);

                working[12] ^= BitOperations.RotateLeft(working[15] + working[14], 7);
                working[13] ^= BitOperations.RotateLeft(working[12] + working[15], 9);
                working[14] ^= BitOperations.RotateLeft(working[13] + working[12], 13);
                working[15] ^= BitOperations.RotateLeft(working[14] + working[13], 18);
            }

            for (var i = 0; i < 16; i++)
            {
                BinaryPrimitives.WriteUInt32LittleEndian(block.Slice(i * 4, 4), working[i] + state[i]);
            }
        }
    }
}
