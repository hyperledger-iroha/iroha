using System.Security.Cryptography;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Queries;

internal static class SignedQueryRequestContext
{
    internal const ulong DefaultTimeToLiveMilliseconds = 100_000;
    internal const int NonceLength = 32;

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

        var writer = new CanonicalNoritoWriter();
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
}
