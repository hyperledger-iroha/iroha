using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha;

public sealed class IrohaClient : IDisposable
{
    /// <summary>Creates a client with the SDK-managed one-shot Torii transport.</summary>
    public IrohaClient(Uri toriiBaseUri, ToriiClientOptions? options = null)
    {
        Torii = new ToriiClient(toriiBaseUri, options);
        Ledger = new LedgerClient(Torii);
    }

    /// <summary>
    /// Creates an anonymous client over a caller-owned transport. Bearer and canonical request
    /// credentials are rejected because the SDK cannot verify the transport's redirect and retry
    /// behavior.
    /// </summary>
    public IrohaClient(
        Uri toriiBaseUri,
        HttpClient httpClient,
        ToriiClientOptions? options = null)
    {
        Torii = new ToriiClient(toriiBaseUri, httpClient, options);
        Ledger = new LedgerClient(Torii);
    }

    internal IrohaClient(
        Uri toriiBaseUri,
        HttpClient httpClient,
        ToriiClientOptions? toriiOptions,
        TransactionSubmissionTransportAssurance transportAssurance)
    {
        Torii = new ToriiClient(
            toriiBaseUri,
            httpClient,
            toriiOptions,
            transportAssurance);
        Ledger = new LedgerClient(Torii);
    }

    public ToriiClient Torii { get; }

    public LedgerClient Ledger { get; }

    public void Dispose()
    {
        Torii.Dispose();
    }
}
