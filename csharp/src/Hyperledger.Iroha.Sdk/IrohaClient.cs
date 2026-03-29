using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha;

public sealed class IrohaClient : IDisposable
{
    public IrohaClient(Uri toriiBaseUri, HttpClient? httpClient = null, ToriiClientOptions? toriiOptions = null)
    {
        Torii = new ToriiClient(toriiBaseUri, httpClient, toriiOptions);
        Ledger = new LedgerClient(Torii);
    }

    public ToriiClient Torii { get; }

    public LedgerClient Ledger { get; }

    public void Dispose()
    {
        Torii.Dispose();
    }
}
