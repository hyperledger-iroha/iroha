using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

public sealed class SignedTransactionEnvelope
{
    public SignedTransactionEnvelope(
        byte[] noritoBytes,
        byte[] signedTransactionBytes,
        byte[] payloadBytes,
        byte[] transactionHash)
    {
        NoritoBytes = noritoBytes;
        SignedTransactionBytes = signedTransactionBytes;
        PayloadBytes = payloadBytes;

        if (transactionHash.Length != IrohaHash.Length)
        {
            throw new ArgumentException($"Transaction hash must be {IrohaHash.Length} bytes.", nameof(transactionHash));
        }

        TransactionHash = transactionHash;
    }

    public byte[] NoritoBytes { get; }

    public byte[] SignedTransactionBytes { get; }

    public byte[] PayloadBytes { get; }

    public byte[] TransactionHash { get; }

    public string TransactionHashHex => Convert.ToHexString(TransactionHash).ToLowerInvariant();
}
