namespace Hyperledger.Iroha.Http;

public sealed class CanonicalRequestCredentials
{
    private readonly byte[] privateKeySeed;

    public CanonicalRequestCredentials(string accountId, ReadOnlySpan<byte> privateKeySeed)
    {
        var checkedAccountId = RequireExactNonBlank(accountId, nameof(accountId));
        if (privateKeySeed.Length == 0)
        {
            throw new ArgumentException("private key seed must not be empty", nameof(privateKeySeed));
        }

        AccountId = checkedAccountId;
        this.privateKeySeed = privateKeySeed.ToArray();
    }

    public string AccountId { get; }

    public byte[] PrivateKeySeed => [.. privateKeySeed];

    private static string RequireExactNonBlank(string? value, string parameterName)
    {
        if (string.IsNullOrEmpty(value))
        {
            throw new ArgumentException($"{parameterName} must not be empty", parameterName);
        }
        if (value != value.Trim() || value.Any(char.IsControl))
        {
            throw new ArgumentException($"{parameterName} must not contain surrounding whitespace or control characters", parameterName);
        }

        return value;
    }
}
