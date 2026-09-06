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

    /// <summary>Maximum ASCII bytes preflighted for a three-segment account alias.</summary>
    public const int MaxAliasLiteralBytesV1 = 3 * 255 + 2;

    private static readonly byte[] NetworkDomain = Encoding.UTF8.GetBytes("iroha.app.request.network.v1\0");
    private static readonly Uri CanonicalPathBaseUri = new("https://canonical.invalid", UriKind.Absolute);

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
        var exactPath = RequireHttpTransportPath(path, nameof(path));
        var exactQuery = RequireHttpTransportQuery(query, exactPath);

        var effectiveTimestamp = RequirePositiveTimestamp(
            timestampMs ?? DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
            nameof(timestampMs));
        var effectiveNonce = nonce is null ? GenerateNonce() : RequireExactNonBlank(nonce, nameof(nonce));
        effectiveNonce = RequireCanonicalNonce(effectiveNonce, nameof(nonce));
        EnsureAccountMatchesPrivateKey(canonicalAccountId, privateKeySeed, nameof(accountId));
        var message = BuildSignatureMessage(networkId, exactMethod, exactPath, exactQuery, body, effectiveTimestamp, effectiveNonce);
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
            return string.Empty;
        }

        var pairs = new List<KeyValuePair<string, string>>();
        foreach (var part in query.Split('&', StringSplitOptions.None))
        {
            if (part.Length == 0)
            {
                continue;
            }
            if (pairs.Count >= MaxQueryPairsV1)
            {
                throw new ArgumentException(
                    $"query must not contain more than {MaxQueryPairsV1} non-empty pairs.",
                    nameof(rawQuery));
            }

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
        if (string.IsNullOrEmpty(value)
            || value.Length > 256
            || value.Any(static character => character is < '\u0021' or > '\u007e'))
        {
            throw new ArgumentException(
                $"{paramName} must contain 1...256 printable ASCII bytes without spaces.",
                paramName);
        }

        return value;
    }

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
        RequireCanonicalPathAsciiWireSpelling(exact, paramName);
        if (exact[0] != '/')
        {
            throw new ArgumentException($"{paramName} must be a root-relative path.", paramName);
        }

        if (exact.Length > 1 && exact[1] == '/')
        {
            throw new ArgumentException($"{paramName} must not be a scheme-relative URI.", paramName);
        }

        if (exact.Contains('?', StringComparison.Ordinal)
            || exact.Contains('#', StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{paramName} must not contain query or fragment characters.",
                paramName);
        }

        RequireUriStablePath(exact, paramName);
        return exact;
    }

    private static string RequireHttpTransportPath(string? value, string paramName)
    {
        var exact = RequireRootRelativePath(value, paramName);
        var wirePath = new Uri(CanonicalPathBaseUri, exact).AbsolutePath;
        RequireCanonicalPathByteLength(wirePath, paramName);
        RequireCanonicalPathAsciiWireSpelling(wirePath, paramName);
        RequireUriStablePath(wirePath, paramName);
        return wirePath;
    }

    private static string? RequireHttpTransportQuery(string? value, string wirePath)
    {
        _ = BuildCanonicalQueryString(value);
        if (string.IsNullOrEmpty(value))
        {
            return value;
        }

        var rawQuery = value[0] == '?' ? value[1..] : value;
        var wireQuery = new Uri(CanonicalPathBaseUri, $"{wirePath}?{rawQuery}").Query;
        return wireQuery.Length == 0 ? string.Empty : wireQuery[1..];
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
        if (TryParseCanonicalI105Account(exact, out _)
            || IsCanonicalAsciiAccountAlias(exact))
        {
            return exact;
        }

        throw new ArgumentException(
            $"{paramName} must be a canonical I105 account id or structurally valid ASCII account alias.",
            paramName);
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
        if (!TryParseCanonicalI105Account(accountId, out var account))
        {
            // An alias is resolved to its controller by Torii; the SDK cannot
            // prove that state-dependent binding before sending the request.
            return;
        }

        var publicKey = Ed25519Signer.GetPublicKey(privateKeySeed);
        var expectedAccount = AccountAddress.FromPublicKey(publicKey);
        if (!account!.ControllerBytes().AsSpan().SequenceEqual(expectedAccount.ControllerBytes()))
        {
            var expectedAccountId = expectedAccount.ToI105(AccountAddress.DefaultChainDiscriminant);
            throw new ArgumentException(
                $"accountId must match the account derived from privateKeySeed: {expectedAccountId}.",
                accountParamName);
        }
    }

    internal static string CanonicalAccountHeaderValue(string accountId)
    {
        return TryParseCanonicalI105Account(accountId, out var address)
            ? address!.CanonicalHex
            : accountId;
    }

    private static bool TryParseCanonicalI105Account(string value, out AccountAddress? address)
    {
        try
        {
            address = AccountAddress.Parse(value);
            return true;
        }
        catch (AccountAddressException)
        {
            address = null;
            return false;
        }
    }

    internal static bool IsCanonicalAsciiAccountAlias(string value)
    {
        var separator = value.IndexOf('@');
        if (value.Length > MaxAliasLiteralBytesV1
            || value.StartsWith("0x", StringComparison.Ordinal)
            || separator <= 0
            || separator != value.LastIndexOf('@')
            || separator == value.Length - 1
            || value.Any(static character => character is < '\u0021' or > '\u007e'))
        {
            return false;
        }

        var scope = value[(separator + 1)..].Split('.', StringSplitOptions.None);
        return scope.Length is 1 or 2
            && IsCanonicalAsciiAliasSegment(value[..separator])
            && scope.All(IsCanonicalAsciiAliasSegment);
    }

    private static bool IsCanonicalAsciiAliasSegment(string value)
    {
        if (value.Length is < 1 or > 63
            || value[0] == '-'
            || value[^1] == '-'
            || !value.All(static character => character is >= 'a' and <= 'z'
                or >= '0' and <= '9'
                or '-'
                or '_'))
        {
            return false;
        }

        return value.Length < 4
            || value[2] != '-'
            || value[3] != '-'
            || value.StartsWith("xn--", StringComparison.Ordinal);
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

    internal static void RequireCanonicalPathAsciiWireSpelling(string value, string paramName)
    {
        if (value.Any(static character => !IsCanonicalPathWireCharacter(character)))
        {
            throw new ArgumentException(
                $"{paramName} must use its exact percent-encoded ASCII wire spelling.",
                paramName);
        }
    }

    private static bool IsCanonicalPathWireCharacter(char value)
        => value is >= 'A' and <= 'Z'
            or >= 'a' and <= 'z'
            or >= '0' and <= '9'
            or '!' or '$' or '%' or '&' or '\'' or '(' or ')' or '*'
            or '+' or ',' or '-' or '.' or '/' or ':' or ';' or '=' or '@'
            or '_' or '~';

    private static string DecodeQueryComponent(string value)
    {
        return DecodePercentEncodedQueryComponent(value);
    }

    internal static void RequireUriStablePath(string value, string paramName)
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

            if (IsDotPathSegment(segment))
            {
                throw new ArgumentException($"{paramName} must not contain dot path segments.", paramName);
            }
        }
    }

    private static bool IsDotPathSegment(string value)
    {
        var dotCount = 0;
        for (var index = 0; index < value.Length;)
        {
            if (value[index] == '.')
            {
                dotCount++;
                index++;
                continue;
            }

            if (value[index] == '%'
                && HexValue(value[index + 1]) == 2
                && HexValue(value[index + 2]) == 14)
            {
                dotCount++;
                index += 3;
                continue;
            }

            return false;
        }

        return dotCount is 1 or 2;
    }

    private static string DecodePercentEncodedQueryComponent(string value)
    {
        var raw = Encoding.UTF8.GetBytes(value);
        var decoded = new List<byte>(raw.Length);
        for (var index = 0; index < raw.Length;)
        {
            if (raw[index] == (byte)'+')
            {
                decoded.Add((byte)' ');
                index++;
                continue;
            }

            if (raw[index] == (byte)'%' && index + 2 < raw.Length)
            {
                var high = HexValue(raw[index + 1]);
                var low = HexValue(raw[index + 2]);
                if (high >= 0 && low >= 0)
                {
                    decoded.Add((byte)((high << 4) | low));
                    index += 3;
                    continue;
                }
            }

            decoded.Add(raw[index]);
            index++;
        }

        return DecodeUtf8LossyLikeRust(decoded);
    }

    private static string DecodeUtf8LossyLikeRust(IReadOnlyList<byte> bytes)
    {
        var decoded = new StringBuilder(bytes.Count);
        var index = 0;
        while (index < bytes.Count)
        {
            var first = bytes[index];
            if (first < 0x80)
            {
                decoded.Append((char)first);
                index++;
            }
            else if (first is >= 0xc2 and <= 0xdf)
            {
                if (index + 1 >= bytes.Count)
                {
                    decoded.Append('\uFFFD');
                    break;
                }

                var second = bytes[index + 1];
                if (!IsUtf8Continuation(second))
                {
                    decoded.Append('\uFFFD');
                    index++;
                    continue;
                }

                decoded.Append((char)(((first & 0x1f) << 6) | (second & 0x3f)));
                index += 2;
            }
            else if (first is >= 0xe0 and <= 0xef)
            {
                if (index + 1 >= bytes.Count)
                {
                    decoded.Append('\uFFFD');
                    break;
                }

                var second = bytes[index + 1];
                var validSecond = first switch
                {
                    0xe0 => second is >= 0xa0 and <= 0xbf,
                    0xed => second is >= 0x80 and <= 0x9f,
                    _ => IsUtf8Continuation(second),
                };
                if (!validSecond)
                {
                    decoded.Append('\uFFFD');
                    index++;
                    continue;
                }

                if (index + 2 >= bytes.Count)
                {
                    decoded.Append('\uFFFD');
                    break;
                }

                var third = bytes[index + 2];
                if (!IsUtf8Continuation(third))
                {
                    decoded.Append('\uFFFD');
                    index += 2;
                    continue;
                }

                decoded.Append((char)(
                    ((first & 0x0f) << 12)
                    | ((second & 0x3f) << 6)
                    | (third & 0x3f)));
                index += 3;
            }
            else if (first is >= 0xf0 and <= 0xf4)
            {
                if (index + 1 >= bytes.Count)
                {
                    decoded.Append('\uFFFD');
                    break;
                }

                var second = bytes[index + 1];
                var validSecond = first switch
                {
                    0xf0 => second is >= 0x90 and <= 0xbf,
                    0xf4 => second is >= 0x80 and <= 0x8f,
                    _ => IsUtf8Continuation(second),
                };
                if (!validSecond)
                {
                    decoded.Append('\uFFFD');
                    index++;
                    continue;
                }

                if (index + 2 >= bytes.Count)
                {
                    decoded.Append('\uFFFD');
                    break;
                }

                var third = bytes[index + 2];
                if (!IsUtf8Continuation(third))
                {
                    decoded.Append('\uFFFD');
                    index += 2;
                    continue;
                }

                if (index + 3 >= bytes.Count)
                {
                    decoded.Append('\uFFFD');
                    break;
                }

                var fourth = bytes[index + 3];
                if (!IsUtf8Continuation(fourth))
                {
                    decoded.Append('\uFFFD');
                    index += 3;
                    continue;
                }

                var codePoint = ((first & 0x07) << 18)
                    | ((second & 0x3f) << 12)
                    | ((third & 0x3f) << 6)
                    | (fourth & 0x3f);
                decoded.Append(char.ConvertFromUtf32(codePoint));
                index += 4;
            }
            else
            {
                decoded.Append('\uFFFD');
                index++;
            }
        }

        return decoded.ToString();
    }

    private static bool IsUtf8Continuation(byte value)
        => value is >= 0x80 and <= 0xbf;

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

    private static int HexValue(byte value)
        => value switch
        {
            >= (byte)'0' and <= (byte)'9' => value - (byte)'0',
            >= (byte)'A' and <= (byte)'F' => value - (byte)'A' + 10,
            >= (byte)'a' and <= (byte)'f' => value - (byte)'a' + 10,
            _ => -1,
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
