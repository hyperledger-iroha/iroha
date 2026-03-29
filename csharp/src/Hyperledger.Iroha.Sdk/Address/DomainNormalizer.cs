using System.Globalization;
using System.Text;

namespace Hyperledger.Iroha.Address;

public static class DomainNormalizer
{
    private static readonly IdnMapping IdnMapping = new()
    {
        AllowUnassigned = false,
        UseStd3AsciiRules = true,
    };

    private static readonly char[] DisallowedDots = ['\u3002', '\uFF0E', '\uFF61'];

    public static string NormalizeLabel(string rawDomain)
    {
        ArgumentNullException.ThrowIfNull(rawDomain);

        if (rawDomain.Length == 0)
        {
            throw InvalidLabel("domain label must not be empty");
        }

        if (rawDomain.IndexOfAny(DisallowedDots) >= 0)
        {
            throw InvalidLabel("domain label contains unsupported Unicode full-stop characters");
        }

        foreach (var rune in rawDomain.EnumerateRunes())
        {
            if (Rune.IsWhiteSpace(rune) || rune.Value is '@' or '#' or '$')
            {
                throw InvalidLabel("domain label contains reserved or whitespace characters");
            }

            if (rune.Value is >= 0x1E00 and <= 0x1EFF)
            {
                throw InvalidLabel("domain label contains disallowed Latin Extended Additional characters");
            }
        }

        var normalized = rawDomain.Normalize(NormalizationForm.FormC);
        string ascii;
        try
        {
            ascii = IdnMapping.GetAscii(normalized);
        }
        catch (ArgumentException exception)
        {
            throw InvalidLabel("domain label failed IDN ASCII normalization", exception);
        }

        var canonical = ascii.ToLowerInvariant();
        if (canonical.Length > 255)
        {
            throw InvalidLabel("domain label exceeds the 255 byte FQDN limit");
        }

        var labels = canonical.Split('.');
        if (labels.Length == 0)
        {
            throw InvalidLabel("domain label is empty");
        }

        foreach (var label in labels)
        {
            if (label.Length is < 1 or > 63)
            {
                throw InvalidLabel("domain label length must be between 1 and 63 bytes");
            }

            foreach (var ch in label)
            {
                if (!IsAllowedAscii(ch))
                {
                    throw InvalidLabel("domain label contains unsupported characters after normalization");
                }
            }
        }

        return canonical;
    }

    private static bool IsAllowedAscii(char ch)
    {
        return ch switch
        {
            >= 'a' and <= 'z' => true,
            >= '0' and <= '9' => true,
            '-' or '_' => true,
            _ => false,
        };
    }

    private static AccountAddressException InvalidLabel(string message, Exception? innerException = null)
    {
        return new(AccountAddressErrorCode.InvalidDomainLabel, message, innerException);
    }
}
