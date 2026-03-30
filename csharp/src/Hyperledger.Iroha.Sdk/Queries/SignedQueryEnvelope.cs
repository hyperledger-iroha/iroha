namespace Hyperledger.Iroha.Queries;

public sealed class SignedQueryEnvelope
{
    public SignedQueryEnvelope(
        byte[] versionedNoritoBytes,
        byte[] signedQueryBytes,
        byte[] payloadBytes,
        byte[] signatureBytes)
    {
        VersionedNoritoBytes = versionedNoritoBytes;
        SignedQueryBytes = signedQueryBytes;
        PayloadBytes = payloadBytes;
        SignatureBytes = signatureBytes;
    }

    public byte[] VersionedNoritoBytes { get; }

    public byte[] SignedQueryBytes { get; }

    public byte[] PayloadBytes { get; }

    public byte[] SignatureBytes { get; }
}
