namespace Hyperledger.Iroha.Torii;

public sealed record class ToriiSoraFsContentResponse
{
    private byte[] bytes = Array.Empty<byte>();
    private long? contentLength;
    private string? contentCid;

    public byte[] Bytes
    {
        get => bytes.ToArray();
        init
        {
            ArgumentNullException.ThrowIfNull(value);
            bytes = value.ToArray();
        }
    }

    public string? ContentType { get; init; }

    public long? ContentLength
    {
        get => contentLength;
        init
        {
            if (value is < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(ContentLength), "Content length must be non-negative.");
            }

            contentLength = value;
        }
    }

    public string? ContentCid
    {
        get => contentCid;
        init
        {
            ValidateOptionalContentCid(value, nameof(ContentCid));
            contentCid = value;
        }
    }

    private static void ValidateOptionalContentCid(string? value, string paramName)
    {
        if (value is null)
        {
            return;
        }

        if (value.Length == 0)
        {
            throw new ArgumentException("Value must be non-empty when provided.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        if (value[0] != 'b' || value.Length == 1)
        {
            throw new ArgumentException("Value must be lowercase multibase base32 CID text.", paramName);
        }

        for (var index = 1; index < value.Length; index++)
        {
            var character = value[index];
            if (character is not (>= 'a' and <= 'z') and not (>= '2' and <= '7'))
            {
                throw new ArgumentException("Value must be lowercase multibase base32 CID text.", paramName);
            }
        }
    }
}
