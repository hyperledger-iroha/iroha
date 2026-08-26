using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Verifies the canonical body hash, signer, and pinned context of a sponsored
/// onboarding receipt.
/// </summary>
public static class ToriiAccountOnboardingReceiptVerifier
{
    private static readonly byte[] HashDomain =
        "iroha:account-onboarding-plan-receipt:v1\0"u8.ToArray();

    /// <summary>Validate a receipt against the configured authority and exact network.</summary>
    public static ToriiAccountOnboardingPlanReceipt RequirePinned(
        ToriiAccountOnboardingPlanReceipt receipt,
        string expectedAuthority,
        NetworkId expectedNetworkId,
        ToriiAccountOnboardingPlanBodyEncoder canonicalBodyEncoder)
    {
        ArgumentNullException.ThrowIfNull(receipt);
        ArgumentNullException.ThrowIfNull(canonicalBodyEncoder);
        var authority = RequireCanonicalAccountId(expectedAuthority, nameof(expectedAuthority));
        ArgumentNullException.ThrowIfNull(expectedNetworkId);
        var body = receipt.Body ?? throw new ArgumentException("Receipt body must not be null.", nameof(receipt));
        var request = body.Request ?? throw new ArgumentException("Receipt request must not be null.", nameof(receipt));

        if (body.Version != 1 || request.Version != 1)
        {
            throw new ArgumentException("Receipt and request versions must both be 1.", nameof(receipt));
        }
        if (!AccountIdsHaveSameIdentity(body.Authority, authority))
        {
            throw new ArgumentException("Receipt authority does not match the pinned authority.", nameof(receipt));
        }
        if (body.NetworkId != expectedNetworkId)
        {
            throw new ArgumentException("Receipt network does not match the pinned NetworkId.", nameof(receipt));
        }

        _ = RequireCanonicalAccountId(request.AccountId, $"{nameof(receipt)}.body.request.account_id");
        var canonicalAlias = RequireAlias(request.Alias, $"{nameof(receipt)}.body.request.alias");
        if (!string.Equals(canonicalAlias, request.Alias, StringComparison.Ordinal))
        {
            throw new ArgumentException("Receipt request alias must already be canonical.", nameof(receipt));
        }
        RequireCanonicalPermissions(request.Permissions, $"{nameof(receipt)}.body.request.permissions");
        RequireJsonKind(body.Anchor, JsonValueKind.Object, $"{nameof(receipt)}.body.anchor");
        RequireJsonKind(body.Resource, JsonValueKind.Object, $"{nameof(receipt)}.body.resource");
        RequireJsonKind(body.Acquisition, JsonValueKind.Object, $"{nameof(receipt)}.body.acquisition");
        RequireJsonKind(body.QuoteGuard, JsonValueKind.Object, $"{nameof(receipt)}.body.quote_guard");
        RequireJsonKind(body.Instructions, JsonValueKind.Array, $"{nameof(receipt)}.body.instructions");
        if (body.OwnerAutoRenewInstruction.ValueKind is not JsonValueKind.Null and not JsonValueKind.Object)
        {
            throw new ArgumentException("Receipt owner_auto_renew_instruction must be null or an object.", nameof(receipt));
        }
        if (body.ValidUntilMilliseconds == 0)
        {
            throw new ArgumentException("Receipt valid_until_ms must be positive.", nameof(receipt));
        }

        var hash = DecodeCanonicalHash(receipt.PlanHash, $"{nameof(receipt)}.plan_hash");
        var canonicalBody = canonicalBodyEncoder(body)
            ?? throw new ArgumentException("Canonical body encoder returned null.", nameof(canonicalBodyEncoder));
        if (canonicalBody.Length == 0)
        {
            throw new ArgumentException("Canonical body encoder returned an empty payload.", nameof(canonicalBodyEncoder));
        }
        var hashInput = new byte[HashDomain.Length + canonicalBody.Length];
        HashDomain.CopyTo(hashInput, 0);
        canonicalBody.CopyTo(hashInput, HashDomain.Length);
        var expectedHash = IrohaHash.Hash(hashInput);
        if (!CryptographicOperations.FixedTimeEquals(hash, expectedHash))
        {
            throw new ArgumentException("Receipt plan hash does not match its canonical Norito body.", nameof(receipt));
        }

        var signature = DecodeHex(receipt.Signature, 64, $"{nameof(receipt)}.signature");
        var account = AccountAddress.Parse(authority);
        if (account.AddressClass != AddressClass.SingleKey
            || !string.Equals(account.Algorithm, "ed25519", StringComparison.Ordinal)
            || account.PublicKey.Length != Ed25519Signer.PublicKeyLength
            || !Ed25519Signer.Verify(hash, signature, account.PublicKey))
        {
            throw new ArgumentException("Receipt signature is not valid for the pinned onboarding authority.", nameof(receipt));
        }

        return receipt;
    }

    internal static string RequireAlias(string? value, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Alias must not contain whitespace.", parameterName);
        }
        var separator = exact.IndexOf('@');
        if (separator <= 0 || separator != exact.LastIndexOf('@') || separator == exact.Length - 1)
        {
            throw new ArgumentException("Alias must be label@dataspace or label@domain.dataspace.", parameterName);
        }
        var suffix = exact[(separator + 1)..];
        var parts = suffix.Split('.');
        if (parts.Length is < 1 or > 2 || parts.Any(static part => part.Length == 0))
        {
            throw new ArgumentException("Alias must be label@dataspace or label@domain.dataspace.", parameterName);
        }

        var label = CanonicalizeAliasSegment(exact[..separator], parameterName);
        var domain = parts.Length == 2
            ? CanonicalizeAliasSegment(parts[0], parameterName)
            : null;
        var dataspace = CanonicalizeAliasSegment(parts[^1], parameterName);
        return domain is null
            ? $"{label}@{dataspace}"
            : $"{label}@{domain}.{dataspace}";
    }

    internal static string RequireCanonicalAccountId(string? value, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        try
        {
            _ = AccountAddress.Parse(exact);
        }
        catch (AccountAddressException error)
        {
            throw new ArgumentException("Value must be a canonical I105 account id.", parameterName, error);
        }
        return exact;
    }

    internal static string RequireExactText(string? value, string parameterName)
    {
        if (string.IsNullOrEmpty(value))
        {
            throw new ArgumentException("Value must be non-empty.", parameterName);
        }
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal)
            || value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must be exact text without surrounding whitespace or control characters.", parameterName);
        }
        return value;
    }

    internal static string[] NormalizePermissions(IReadOnlyList<string>? values, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(values, parameterName);
        var normalized = values
            .Select((value, index) => RequirePermissionName(value, $"{parameterName}[{index}]"))
            .OrderBy(static value => value, StringComparer.Ordinal)
            .ToArray();
        if (normalized.Distinct(StringComparer.Ordinal).Count() != normalized.Length)
        {
            throw new ArgumentException("Permission names must be unique.", parameterName);
        }
        return normalized;
    }

    private static string CanonicalizeAliasSegment(string value, string parameterName)
    {
        var normalized = value.Normalize(NormalizationForm.FormC);
        if (normalized.Any(static value => value is >= '\u1E00' and <= '\u1EFF'))
        {
            throw new ArgumentException("Alias contains a disallowed Unicode character.", parameterName);
        }

        string ascii;
        try
        {
            ascii = normalized.All(static value => value <= 0x7f)
                ? normalized
                : new IdnMapping { AllowUnassigned = false, UseStd3AsciiRules = true }
                    .GetAscii(normalized);
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException("Alias contains an invalid IDN segment.", parameterName, error);
        }
        ascii = ascii.ToLowerInvariant();
        if (ascii.Length is < 1 or > 63
            || ascii[0] == '-'
            || ascii[^1] == '-'
            || ascii.Any(static value => !char.IsAsciiLetterOrDigit(value) && value is not '-' and not '_'))
        {
            throw new ArgumentException("Alias contains an invalid segment.", parameterName);
        }
        return ascii;
    }

    private static string RequirePermissionName(string? value, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        if (exact.Any(char.IsWhiteSpace) || exact.IndexOfAny(['@', '#', '$']) >= 0)
        {
            throw new ArgumentException("Permission name contains a forbidden character.", parameterName);
        }
        return exact.Normalize(NormalizationForm.FormC);
    }

    private static void RequireCanonicalPermissions(IReadOnlyList<string>? values, string parameterName)
    {
        var normalized = NormalizePermissions(values, parameterName);
        if (!normalized.SequenceEqual(values!, StringComparer.Ordinal))
        {
            throw new ArgumentException("Receipt permission names must be sorted canonically.", parameterName);
        }
    }

    private static void RequireJsonKind(JsonElement value, JsonValueKind kind, string parameterName)
    {
        if (value.ValueKind != kind)
        {
            throw new ArgumentException($"Value must be a JSON {kind.ToString().ToLowerInvariant()}.", parameterName);
        }
    }

    private static byte[] DecodeCanonicalHash(string? value, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        if (exact.Length != 74 || !exact.StartsWith("hash:", StringComparison.Ordinal) || exact[69] != '#')
        {
            throw new ArgumentException("Value must be a canonical checksummed Norito Hash literal.", parameterName);
        }
        var body = exact.Substring(5, 64);
        var checksum = exact.Substring(70, 4);
        if (body.Any(static value => !IsUpperHex(value))
            || checksum.Any(static value => !IsUpperHex(value))
            || !ushort.TryParse(checksum, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var supplied)
            || supplied != Crc16(Encoding.ASCII.GetBytes($"hash:{body}")))
        {
            throw new ArgumentException("Value has a malformed Norito Hash checksum.", parameterName);
        }
        var decoded = Convert.FromHexString(body);
        if ((decoded[^1] & 1) == 0)
        {
            throw new ArgumentException("Norito Hash marker bit must be set.", parameterName);
        }
        return decoded;
    }

    private static byte[] DecodeHex(string? value, int byteLength, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        if (exact.Length != byteLength * 2 || exact.Any(static value => !Uri.IsHexDigit(value)))
        {
            throw new ArgumentException($"Value must contain exactly {byteLength} bytes of hexadecimal.", parameterName);
        }
        return Convert.FromHexString(exact);
    }

    private static bool IsUpperHex(char value) => value is >= '0' and <= '9' or >= 'A' and <= 'F';

    private static bool AccountIdsHaveSameIdentity(string? left, string? right)
    {
        if (left is null || right is null)
        {
            return false;
        }
        try
        {
            return AccountAddress.Parse(left)
                .CanonicalBytes()
                .AsSpan()
                .SequenceEqual(AccountAddress.Parse(right).CanonicalBytes());
        }
        catch (AccountAddressException)
        {
            return false;
        }
    }

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
