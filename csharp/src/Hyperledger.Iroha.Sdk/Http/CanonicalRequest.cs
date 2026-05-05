using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Crypto;

namespace Hyperledger.Iroha.Http;

public static class CanonicalRequest
{
    public static CanonicalRequestHeaders BuildHeaders(
        string accountId,
        ReadOnlySpan<byte> privateKeySeed,
        string method,
        string path,
        string? query = null,
        ReadOnlySpan<byte> body = default,
        long? timestampMs = null,
        string? nonce = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(accountId);
        ArgumentException.ThrowIfNullOrWhiteSpace(method);
        ArgumentException.ThrowIfNullOrWhiteSpace(path);

        var effectiveTimestamp = timestampMs ?? DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var effectiveNonce = string.IsNullOrWhiteSpace(nonce) ? GenerateNonce() : nonce;
        var message = BuildSignatureMessage(method, path, query, body, effectiveTimestamp, effectiveNonce);
        var signature = Ed25519Signer.Sign(message, privateKeySeed);
        return new CanonicalRequestHeaders(accountId, Convert.ToBase64String(signature), effectiveTimestamp, effectiveNonce);
    }

    public static string BuildCanonicalQueryString(string? rawQuery)
    {
        if (string.IsNullOrEmpty(rawQuery))
        {
            return string.Empty;
        }

        var query = rawQuery[0] == '?' ? rawQuery[1..] : rawQuery;
        if (query.Length == 0)
        {
            return string.Empty;
        }

        var pairs = new List<KeyValuePair<string, string>>();
        foreach (var part in query.Split('&', StringSplitOptions.None))
        {
            var components = part.Split('=', 2, StringSplitOptions.None);
            var key = DecodeQueryComponent(components[0]);
            var value = components.Length > 1 ? DecodeQueryComponent(components[1]) : string.Empty;
            pairs.Add(new KeyValuePair<string, string>(key, value));
        }

        pairs.Sort(static (left, right) =>
        {
            var keyOrder = CompareUtf8(left.Key, right.Key);
            return keyOrder != 0 ? keyOrder : CompareUtf8(left.Value, right.Value);
        });

        return string.Join("&", pairs.Select(static pair => $"{PercentEncode(pair.Key)}={PercentEncode(pair.Value)}"));
    }

    public static byte[] BuildMessage(string method, string path, string? query = null, ReadOnlySpan<byte> body = default)
    {
        var bodyHash = Convert.ToHexString(SHA256.HashData(body)).ToLowerInvariant();
        var canonicalQuery = BuildCanonicalQueryString(query);
        return Encoding.UTF8.GetBytes($"{method.ToUpperInvariant()}\n{path}\n{canonicalQuery}\n{bodyHash}");
    }

    public static byte[] BuildSignatureMessage(
        string method,
        string path,
        string? query = null,
        ReadOnlySpan<byte> body = default,
        long timestampMs = 0,
        string? nonce = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(nonce);
        var baseMessage = Encoding.UTF8.GetString(BuildMessage(method, path, query, body));
        return Encoding.UTF8.GetBytes($"{baseMessage}\n{timestampMs}\n{nonce}");
    }

    private static int CompareUtf8(string left, string right)
    {
        if (ReferenceEquals(left, right))
        {
            return 0;
        }

        var leftBytes = Encoding.UTF8.GetBytes(left);
        var rightBytes = Encoding.UTF8.GetBytes(right);
        var minLength = Math.Min(leftBytes.Length, rightBytes.Length);
        for (var index = 0; index < minLength; index += 1)
        {
            var difference = leftBytes[index] - rightBytes[index];
            if (difference != 0)
            {
                return difference;
            }
        }

        return leftBytes.Length - rightBytes.Length;
    }

    private static string DecodeQueryComponent(string value)
    {
        return Uri.UnescapeDataString(value.Replace("+", " ", StringComparison.Ordinal));
    }

    private static string GenerateNonce()
    {
        return Convert.ToHexString(RandomNumberGenerator.GetBytes(16)).ToLowerInvariant();
    }

    private static string PercentEncode(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        var builder = new StringBuilder(bytes.Length);
        foreach (var b in bytes)
        {
            if (IsUnreserved(b))
            {
                builder.Append((char)b);
            }
            else if (b == 0x20)
            {
                builder.Append('+');
            }
            else
            {
                builder.Append('%');
                builder.Append(b.ToString("X2", CultureInfo.InvariantCulture));
            }
        }

        return builder.ToString();
    }

    private static bool IsUnreserved(byte value)
    {
        return value switch
        {
            >= (byte)'A' and <= (byte)'Z' => true,
            >= (byte)'a' and <= (byte)'z' => true,
            >= (byte)'0' and <= (byte)'9' => true,
            (byte)'-' or (byte)'.' or (byte)'_' or (byte)'~' => true,
            _ => false,
        };
    }
}
