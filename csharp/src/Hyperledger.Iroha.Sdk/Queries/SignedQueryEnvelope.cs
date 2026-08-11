using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Queries;

public sealed class SignedQueryEnvelope
{
    private const byte SupportedSignedQueryVersion = 1;

    private readonly byte[] versionedNoritoBytes;
    private readonly byte[] signedQueryBytes;
    private readonly byte[] payloadBytes;
    private readonly byte[] signatureBytes;

    public SignedQueryEnvelope(
        byte[] versionedNoritoBytes,
        byte[] signedQueryBytes,
        byte[] payloadBytes,
        byte[] signatureBytes)
    {
        ArgumentNullException.ThrowIfNull(versionedNoritoBytes);
        ArgumentNullException.ThrowIfNull(signedQueryBytes);
        ArgumentNullException.ThrowIfNull(payloadBytes);
        ArgumentNullException.ThrowIfNull(signatureBytes);

        this.versionedNoritoBytes = CopyNonEmpty(versionedNoritoBytes, nameof(versionedNoritoBytes));
        this.signedQueryBytes = CopyNonEmpty(signedQueryBytes, nameof(signedQueryBytes));
        this.payloadBytes = CopyNonEmpty(payloadBytes, nameof(payloadBytes));
        this.signatureBytes = CopyExactLength(
            signatureBytes,
            Ed25519Signer.SignatureLength,
            nameof(signatureBytes),
            "Signature");

        if (this.versionedNoritoBytes[0] != SupportedSignedQueryVersion)
        {
            throw new ArgumentException(
                $"Signed query version must be {SupportedSignedQueryVersion}.",
                nameof(versionedNoritoBytes));
        }

        if (this.versionedNoritoBytes.Length != this.signedQueryBytes.Length + 1 ||
            !this.versionedNoritoBytes.AsSpan(1).SequenceEqual(this.signedQueryBytes))
        {
            throw new ArgumentException(
                "Versioned Norito bytes must contain the signed query bytes after the version byte.",
                nameof(versionedNoritoBytes));
        }

        ValidateSignedQueryFields(this.signedQueryBytes, this.payloadBytes, this.signatureBytes);
    }

    public byte[] VersionedNoritoBytes => versionedNoritoBytes.ToArray();

    public byte[] SignedQueryBytes => signedQueryBytes.ToArray();

    public byte[] PayloadBytes => payloadBytes.ToArray();

    public byte[] SignatureBytes => signatureBytes.ToArray();

    private static byte[] CopyNonEmpty(byte[] value, string paramName)
    {
        if (value.Length == 0)
        {
            throw new ArgumentException("Value must not be empty.", paramName);
        }

        return value.ToArray();
    }

    private static byte[] CopyExactLength(byte[] value, int expectedLength, string paramName, string displayName)
    {
        if (value.Length != expectedLength)
        {
            throw new ArgumentException($"{displayName} must be {expectedLength} bytes.", paramName);
        }

        return value.ToArray();
    }

    private static void ValidateSignedQueryFields(
        byte[] signedQueryBytes,
        byte[] payloadBytes,
        byte[] signatureBytes)
    {
        var reader = new CanonicalNoritoReader(
            signedQueryBytes,
            "Signed query bytes",
            nameof(signedQueryBytes));
        var signatureField = reader.ReadField("signature");
        var payloadField = reader.ReadField("payload");
        reader.RequireEnd();

        var decodedSignature = DecodeConstVec(signatureField, nameof(signedQueryBytes));
        if (!decodedSignature.AsSpan().SequenceEqual(signatureBytes))
        {
            throw new ArgumentException("Signature bytes must match the signed query body.", nameof(signatureBytes));
        }

        if (!payloadField.SequenceEqual(payloadBytes))
        {
            throw new ArgumentException("Payload bytes must match the signed query body.", nameof(payloadBytes));
        }
    }

    private static byte[] DecodeConstVec(ReadOnlySpan<byte> bytes, string paramName)
    {
        var reader = new CanonicalNoritoReader(
            bytes,
            "Signed query signature field",
            paramName);
        var count = reader.ReadSequenceLength("count");
        if (count != Ed25519Signer.SignatureLength)
        {
            throw new ArgumentException(
                $"Signed query signature field must encode {Ed25519Signer.SignatureLength} bytes.",
                paramName);
        }

        var output = new byte[Ed25519Signer.SignatureLength];
        for (var index = 0; index < output.Length; index++)
        {
            var item = reader.ReadField($"signature[{index}]");
            if (item.Length != 1)
            {
                throw new ArgumentException("Signed query signature bytes must be encoded as one-byte fields.", paramName);
            }

            output[index] = item[0];
        }

        reader.RequireEnd();

        return output;
    }
}
