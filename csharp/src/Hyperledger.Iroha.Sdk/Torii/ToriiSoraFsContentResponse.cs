namespace Hyperledger.Iroha.Torii;

public sealed record class ToriiSoraFsContentResponse
{
    public byte[] Bytes { get; init; } = Array.Empty<byte>();

    public string? ContentType { get; init; }

    public long? ContentLength { get; init; }

    public string? ContentCid { get; init; }
}
