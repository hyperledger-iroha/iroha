using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Queries;

internal static class SignedQueryRequestContext
{
    internal const ulong DefaultTimeToLiveMilliseconds = 100_000;
    internal const int NonceLength = 32;

    internal static byte[] DecodeNetworkId(string networkId, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(networkId, parameterName);
        if (networkId.Length != 74 ||
            !networkId.StartsWith("hash:", StringComparison.Ordinal) ||
            networkId[69] != '#')
        {
            throw new ArgumentException(
                "Network id must be a canonical checksummed Norito Hash literal.",
                parameterName);
        }

        var body = networkId.Substring(5, 64);
        var checksum = networkId.Substring(70, 4);
        if (body.Any(static value => !IsUpperHex(value)) ||
            checksum.Any(static value => !IsUpperHex(value)) ||
            !ushort.TryParse(checksum, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var supplied) ||
            supplied != Crc16(Encoding.ASCII.GetBytes($"hash:{body}")))
        {
            throw new ArgumentException(
                "Network id has a malformed or invalid Norito Hash checksum.",
                parameterName);
        }

        var decoded = Convert.FromHexString(body);
        if ((decoded[^1] & 1) == 0)
        {
            throw new ArgumentException("Network id Hash marker bit must be set.", parameterName);
        }
        return decoded;
    }

    internal static (ulong CreationTimeMilliseconds, byte[] Nonce) CreateFresh()
    {
        var unixTimeMilliseconds = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        if (unixTimeMilliseconds < 0)
        {
            throw new InvalidOperationException("System clock precedes the Unix epoch.");
        }

        var nonce = new byte[NonceLength];
        for (var attempt = 0; attempt < 16; attempt++)
        {
            RandomNumberGenerator.Fill(nonce);
            if (!IsAllZero(nonce))
            {
                return ((ulong)unixTimeMilliseconds, nonce);
            }
        }
        throw new CryptographicException(
            "Operating-system RNG repeatedly returned the forbidden all-zero signed-query nonce.");
    }

    internal static byte[] EncodePayload(
        TransactionEncodingContext context,
        ReadOnlySpan<byte> networkId,
        string authorityAccountId,
        ulong creationTimeMilliseconds,
        ulong timeToLiveMilliseconds,
        ReadOnlySpan<byte> nonce,
        ReadOnlySpan<byte> request)
    {
        if (networkId.Length != NonceLength)
        {
            throw new ArgumentException("Network id must encode exactly 32 bytes.", nameof(networkId));
        }
        if (timeToLiveMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(timeToLiveMilliseconds),
                "Signed-query time-to-live must be non-zero.");
        }
        if (nonce.Length != NonceLength)
        {
            throw new ArgumentException(
                "Signed-query nonce must contain exactly 32 bytes.",
                nameof(nonce));
        }
        if (IsAllZero(nonce))
        {
            throw new ArgumentException(
                "Signed-query nonce must not be all-zero.",
                nameof(nonce));
        }

        var writer = new OfflineNoritoWriter();
        writer.WriteField(networkId);
        writer.WriteField(context.EncodeAccountId(authorityAccountId));
        writer.WriteField(context.EncodeUInt64(creationTimeMilliseconds));
        writer.WriteField(context.EncodeUInt64(timeToLiveMilliseconds));
        writer.WriteField(nonce);
        writer.WriteField(request);
        return writer.ToArray();
    }

    private static bool IsAllZero(ReadOnlySpan<byte> value)
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

    private static bool IsUpperHex(char value) => value is >= '0' and <= '9' or >= 'A' and <= 'F';

    private static ushort Crc16(ReadOnlySpan<byte> bytes)
    {
        var crc = 0xffff;
        foreach (var value in bytes)
        {
            crc ^= value << 8;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 0x8000) != 0
                    ? ((crc << 1) ^ 0x1021) & 0xffff
                    : (crc << 1) & 0xffff;
            }
        }
        return (ushort)crc;
    }
}
