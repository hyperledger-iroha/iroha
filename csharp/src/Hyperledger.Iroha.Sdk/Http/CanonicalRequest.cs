using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;

namespace Hyperledger.Iroha.Http;

public static partial class CanonicalRequest
{
    /// <summary>Maximum decoded non-empty form pairs in a canonical V1 request.</summary>
    public const int MaxQueryPairsV1 = 64;

    /// <summary>Maximum UTF-8 bytes in the raw canonical V1 query.</summary>
    public const int MaxRawQueryBytesV1 = 64 * 1024;

    /// <summary>Maximum UTF-8 bytes in the canonical V1 HTTP method token.</summary>
    public const int MaxMethodBytesV1 = 32;

    /// <summary>Maximum UTF-8 bytes in the percent-encoded canonical V1 path.</summary>
    public const int MaxPathBytesV1 = 64 * 1024;

    /// <summary>Maximum UTF-8 bytes in a canonical V1 account identity or alias.</summary>
    public const int MaxAccountLiteralBytesV1 = 36 * 1024;

    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);
    private static readonly byte[] NetworkDomain = Encoding.UTF8.GetBytes("iroha.app.request.network.v1\0");

    public static CanonicalRequestHeaders BuildHeaders(
        NetworkId networkId,
        string accountId,
        ReadOnlySpan<byte> privateKeySeed,
        string method,
        string path,
        string? query = null,
        ReadOnlySpan<byte> body = default,
        long? timestampMs = null,
        string? nonce = null)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        var exactAccountId = RequireExactNonBlank(accountId, nameof(accountId));
        var canonicalAccountId = RequireCanonicalAccountId(exactAccountId, nameof(accountId));
        var exactMethod = RequireHttpMethodToken(method, nameof(method));
        var exactPath = RequireRootRelativePath(path, nameof(path));

        var effectiveTimestamp = RequirePositiveTimestamp(
            timestampMs ?? DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
            nameof(timestampMs));
        var effectiveNonce = nonce is null ? GenerateNonce() : RequireExactNonBlank(nonce, nameof(nonce));
        effectiveNonce = RequireCanonicalNonce(effectiveNonce, nameof(nonce));
        EnsureAccountMatchesPrivateKey(canonicalAccountId, privateKeySeed, nameof(accountId));
        var message = BuildSignatureMessage(networkId, exactMethod, exactPath, query, body, effectiveTimestamp, effectiveNonce);
        var signature = Ed25519Signer.Sign(message, privateKeySeed);
        return new CanonicalRequestHeaders(
            canonicalAccountId,
            Convert.ToBase64String(signature),
            effectiveTimestamp,
            effectiveNonce);
    }

    public static string BuildCanonicalQueryString(string? rawQuery)
    {
        if (string.IsNullOrEmpty(rawQuery))
        {
            return string.Empty;
        }

        var hasQueryPrefix = rawQuery[0] == '?';
        var query = hasQueryPrefix ? rawQuery[1..] : rawQuery;
        if (Encoding.UTF8.GetByteCount(query) > MaxRawQueryBytesV1)
        {
            throw new ArgumentException(
                $"query must not exceed {MaxRawQueryBytesV1} raw UTF-8 bytes.",
                nameof(rawQuery));
        }
        if (query.Length == 0)
        {
            if (hasQueryPrefix)
            {
                throw new ArgumentException("query must contain at least one query parameter.", nameof(rawQuery));
            }

            return string.Empty;
        }

        var pairs = new List<KeyValuePair<string, string>>();
        foreach (var part in query.Split('&', StringSplitOptions.None))
        {
            if (part.Length == 0)
            {
                throw new ArgumentException("query must not contain empty query segments.", nameof(rawQuery));
            }
            if (pairs.Count >= MaxQueryPairsV1)
            {
                throw new ArgumentException(
                    $"query must not contain more than {MaxQueryPairsV1} non-empty pairs.",
                    nameof(rawQuery));
            }

            var components = part.Split('=', 2, StringSplitOptions.None);
            var key = DecodeQueryComponent(components[0]);
            if (string.IsNullOrEmpty(key) || key.Any(char.IsWhiteSpace))
            {
                throw new ArgumentException("query parameter names must not be empty or whitespace.", nameof(rawQuery));
            }

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
        var checkedMethod = RequireHttpMethodToken(method, nameof(method));
        var checkedPath = RequireRootRelativePath(path, nameof(path));
        return BuildMessageForExactPath(checkedMethod, checkedPath, query, body);
    }

    private static byte[] BuildMessageForExactPath(
        string exactMethod,
        string exactPath,
        string? query,
        ReadOnlySpan<byte> body)
    {
        var bodyHash = Convert.ToHexString(SHA256.HashData(body)).ToLowerInvariant();
        var canonicalQuery = BuildCanonicalQueryString(query);
        return Encoding.UTF8.GetBytes($"{exactMethod.ToUpperInvariant()}\n{exactPath}\n{canonicalQuery}\n{bodyHash}");
    }

    public static byte[] BuildSignatureMessage(
        NetworkId networkId,
        string method,
        string path,
        string? query = null,
        ReadOnlySpan<byte> body = default,
        long timestampMs = 0,
        string? nonce = null)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        var exactNonce = RequireExactNonBlank(nonce, nameof(nonce));
        exactNonce = RequireCanonicalNonce(exactNonce, nameof(nonce));
        var exactTimestamp = RequirePositiveTimestamp(timestampMs, nameof(timestampMs));
        var exactMethod = RequireHttpMethodToken(method, nameof(method));
        var exactPath = RequireRootRelativePath(path, nameof(path));
        return BuildSignatureMessageForExactPath(
            networkId,
            exactMethod,
            exactPath,
            query,
            body,
            exactTimestamp,
            exactNonce);
    }

    internal static byte[] BuildSignatureMessageForExactPath(
        NetworkId networkId,
        string exactMethod,
        string exactPath,
        string? query,
        ReadOnlySpan<byte> body,
        long exactTimestamp,
        string exactNonce)
    {
        var baseMessage = BuildMessageForExactPath(exactMethod, exactPath, query, body);
        var suffix = Encoding.UTF8.GetBytes($"\n{exactTimestamp}\n{exactNonce}");
        var message = new byte[checked(NetworkDomain.Length + NetworkId.ByteLength + baseMessage.Length + suffix.Length)];
        var offset = 0;
        NetworkDomain.CopyTo(message, offset);
        offset += NetworkDomain.Length;
        networkId.AsSpan().CopyTo(message.AsSpan(offset, NetworkId.ByteLength));
        offset += NetworkId.ByteLength;
        baseMessage.CopyTo(message, offset);
        offset += baseMessage.Length;
        suffix.CopyTo(message, offset);
        return message;
    }

    internal static string RequireExactNonBlank(string? value, string paramName)
    {
        if (string.IsNullOrEmpty(value))
        {
            throw new ArgumentException($"{paramName} must not be empty.", paramName);
        }
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain surrounding whitespace.", paramName);
        }
        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{paramName} must not contain whitespace.", paramName);
        }
        if (value.Any(char.IsControl))
        {
            throw new ArgumentException($"{paramName} must not contain control characters.", paramName);
        }

        return value;
    }

    internal static string RequireCanonicalNonce(string? value, string paramName)
    {
        var exact = RequireExactNonBlank(value, paramName);
        if (exact.Length != 32 || !exact.All(IsLowerHex))
        {
            throw new ArgumentException($"{paramName} must be a 16-byte lowercase hex nonce.", paramName);
        }

        return exact;
    }

    private static bool IsLowerHex(char value)
        => value is >= '0' and <= '9' or >= 'a' and <= 'f';

    private static string RequireHttpMethodToken(string? value, string paramName)
    {
        var exact = RequireExactNonBlank(value, paramName);
        if (Encoding.UTF8.GetByteCount(exact) > MaxMethodBytesV1)
        {
            throw new ArgumentException(
                $"{paramName} must not exceed {MaxMethodBytesV1} UTF-8 bytes.",
                paramName);
        }
        if (!exact.All(IsHttpTokenCharacter))
        {
            throw new ArgumentException($"{paramName} must be an HTTP token.", paramName);
        }

        return exact;
    }

    private static bool IsHttpTokenCharacter(char value)
        => value is >= 'A' and <= 'Z'
            or >= 'a' and <= 'z'
            or >= '0' and <= '9'
            or '!' or '#' or '$' or '%' or '&' or '\'' or '*' or '+'
            or '-' or '.' or '^' or '_' or '`' or '|' or '~';

    internal static string RequireRootRelativePath(string? value, string paramName)
    {
        var exact = RequireExactNonBlank(value, paramName);
        RequireCanonicalPathByteLength(exact, paramName);
        if (exact[0] != '/')
        {
            throw new ArgumentException($"{paramName} must be a root-relative path.", paramName);
        }

        if (exact.Length > 1 && exact[1] == '/')
        {
            throw new ArgumentException($"{paramName} must not be a scheme-relative URI.", paramName);
        }

        if (exact.Contains(':', StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain raw ':' characters.", paramName);
        }

        RequireUriStablePath(exact, paramName);
        return exact;
    }

    internal static string RequireCanonicalAccountId(string? value, string paramName)
    {
        var exact = RequireExactNonBlank(value, paramName);
        if (Encoding.UTF8.GetByteCount(exact) > MaxAccountLiteralBytesV1)
        {
            throw new ArgumentException(
                $"{paramName} must not exceed {MaxAccountLiteralBytesV1} UTF-8 bytes.",
                paramName);
        }
        try
        {
            return AccountAddress.Parse(exact, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        catch (AccountAddressException exception)
        {
            throw new ArgumentException($"{paramName} must be a canonical I105 account id.", paramName, exception);
        }
    }

    internal static long RequirePositiveTimestamp(long timestampMs, string paramName)
    {
        if (timestampMs <= 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "timestampMs must be positive Unix milliseconds.");
        }

        return timestampMs;
    }

    internal static void EnsureAccountMatchesPrivateKey(
        string accountId,
        ReadOnlySpan<byte> privateKeySeed,
        string accountParamName)
    {
        var publicKey = Ed25519Signer.GetPublicKey(privateKeySeed);
        var expectedAccountId = AccountAddress.FromPublicKey(publicKey, "ed25519")
            .ToI105(AccountAddress.DefaultChainDiscriminant);
        if (!string.Equals(expectedAccountId, accountId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"accountId must match the account derived from privateKeySeed: {expectedAccountId}.",
                accountParamName);
        }
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

    internal static void RequireCanonicalPathByteLength(string value, string paramName)
    {
        if (Encoding.UTF8.GetByteCount(value) > MaxPathBytesV1)
        {
            throw new ArgumentException(
                $"{paramName} must not exceed {MaxPathBytesV1} UTF-8 bytes.",
                paramName);
        }
    }

    private static string DecodeQueryComponent(string value)
    {
        ValidatePercentEscapes(value);
        var decoded = DecodePercentEncodedQueryComponent(value);
        if (decoded.Any(char.IsControl))
        {
            throw new ArgumentException("query components must not contain control characters.", nameof(value));
        }

        return decoded;
    }

    private static void RequireUriStablePath(string value, string paramName)
    {
        if (value.Contains('\\', StringComparison.Ordinal))
        {
            throw new ArgumentException($"{paramName} must not contain raw backslash characters.", paramName);
        }

        ValidatePathPercentEscapes(value, paramName);
        foreach (var segment in value.Split('/', StringSplitOptions.None))
        {
            if (segment.Length == 0)
            {
                continue;
            }

            var decodedSegment = DecodePercentEncodedPathSegment(segment, paramName);
            if (decodedSegment.Any(char.IsControl))
            {
                throw new ArgumentException($"{paramName} path segments must not contain percent-decoded control characters.", paramName);
            }

            if (decodedSegment is "." or "..")
            {
                throw new ArgumentException($"{paramName} must not contain dot path segments.", paramName);
            }
        }
    }

    private static string DecodePercentEncodedPathSegment(string value, string paramName)
    {
        var builder = new StringBuilder(value.Length);
        for (var index = 0; index < value.Length;)
        {
            if (value[index] != '%')
            {
                builder.Append(value[index]);
                index++;
                continue;
            }

            var bytes = new List<byte>();
            while (index < value.Length && value[index] == '%')
            {
                bytes.Add((byte)((HexValue(value[index + 1]) << 4) | HexValue(value[index + 2])));
                index += 3;
            }

            try
            {
                builder.Append(StrictUtf8.GetString(bytes.ToArray()));
            }
            catch (DecoderFallbackException exception)
            {
                throw new ArgumentException(
                    $"{paramName} path segments must contain valid UTF-8 percent-encoded bytes.",
                    paramName,
                    exception);
            }
        }

        return builder.ToString();
    }

    private static string DecodePercentEncodedQueryComponent(string value)
    {
        var builder = new StringBuilder(value.Length);
        for (var index = 0; index < value.Length;)
        {
            if (value[index] == '+')
            {
                builder.Append(' ');
                index++;
                continue;
            }

            if (value[index] != '%')
            {
                builder.Append(value[index]);
                index++;
                continue;
            }

            var bytes = new List<byte>();
            while (index < value.Length && value[index] == '%')
            {
                bytes.Add((byte)((HexValue(value[index + 1]) << 4) | HexValue(value[index + 2])));
                index += 3;
            }

            try
            {
                builder.Append(StrictUtf8.GetString(bytes.ToArray()));
            }
            catch (DecoderFallbackException exception)
            {
                throw new ArgumentException(
                    "query components must contain valid UTF-8 percent-encoded bytes.",
                    nameof(value),
                    exception);
            }
        }

        return builder.ToString();
    }

    private static void ValidatePercentEscapes(string value)
    {
        for (var index = 0; index < value.Length; index++)
        {
            if (value[index] != '%')
            {
                continue;
            }

            if (index + 2 >= value.Length
                || !Uri.IsHexDigit(value[index + 1])
                || !Uri.IsHexDigit(value[index + 2]))
            {
                throw new ArgumentException("query components must contain valid percent escapes.", nameof(value));
            }

            index += 2;
        }
    }

    private static void ValidatePathPercentEscapes(string value, string paramName)
    {
        for (var index = 0; index < value.Length; index++)
        {
            if (value[index] != '%')
            {
                continue;
            }

            if (index + 2 >= value.Length
                || !Uri.IsHexDigit(value[index + 1])
                || !Uri.IsHexDigit(value[index + 2]))
            {
                throw new ArgumentException($"{paramName} must contain valid percent escapes.", paramName);
            }

            index += 2;
        }
    }

    private static int HexValue(char value)
        => value switch
        {
            >= '0' and <= '9' => value - '0',
            >= 'A' and <= 'F' => value - 'A' + 10,
            >= 'a' and <= 'f' => value - 'a' + 10,
            _ => throw new ArgumentException("query components must contain valid percent escapes."),
        };

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
            if (IsFormUrlEncodedSafe(b))
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

    private static bool IsFormUrlEncodedSafe(byte value)
    {
        return value switch
        {
            >= (byte)'A' and <= (byte)'Z' => true,
            >= (byte)'a' and <= (byte)'z' => true,
            >= (byte)'0' and <= (byte)'9' => true,
            (byte)'*' or (byte)'-' or (byte)'.' or (byte)'_' => true,
            _ => false,
        };
    }
}
