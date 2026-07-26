using Hyperledger.Iroha;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

var baseUrl = Environment.GetEnvironmentVariable("IROHA_CSHARP_TORII_BASE_URL")
    ?? "https://taira.sora.org";
var seedHex = Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX");
var canonicalAccountId = Environment.GetEnvironmentVariable("IROHA_CSHARP_CANONICAL_ACCOUNT_ID");
var privateKeySeed = string.IsNullOrWhiteSpace(seedHex)
    ? null
    : Convert.FromHexString(seedHex);
var toriiOptions = privateKeySeed is null || string.IsNullOrWhiteSpace(canonicalAccountId)
    ? null
    : new ToriiClientOptions
    {
        CanonicalRequestCredentials = new CanonicalRequestCredentials(
            canonicalAccountId,
            privateKeySeed),
    };

using var client = new IrohaClient(
    new Uri(baseUrl, UriKind.Absolute),
    toriiOptions: toriiOptions);
try
{
    var capabilities = await client.Torii.GetNodeCapabilitiesAsync();
    var accounts = await client.Torii.GetAccountsAsync(limit: 5);
    var aliases = accounts.Items.Count == 0
        ? null
        : await client.Torii.LookupAliasesByAccountAsync(accounts.Items[0].Id);
    var faucetPuzzle = await client.Torii.GetAccountFaucetPuzzleAsync();

    Console.WriteLine($"Torii ABI version: {capabilities.AbiVersion}");
    Console.WriteLine($"Torii data model version: {capabilities.DataModelVersion}");
    Console.WriteLine($"Visible accounts in first page: {accounts.Items.Count}");
    Console.WriteLine($"Aliases on first account: {aliases?.Total ?? 0}");
    Console.WriteLine($"Faucet puzzle difficulty bits: {faucetPuzzle.DifficultyBits}");

    if (privateKeySeed is not null && !string.IsNullOrWhiteSpace(canonicalAccountId))
    {
        var transaction = client.Ledger
            .BuildTransaction(
                "00000042",
                canonicalAccountId,
                FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
            .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "1", canonicalAccountId)
            .SetCreationTime(DateTimeOffset.UtcNow)
            .SetTimeToLiveMilliseconds(5_000)
            .SetNonce(1);
        var signed = await client.Ledger.QuoteAndSignAsync(transaction, privateKeySeed);

        Console.WriteLine($"Prepared quoted transaction hash: {signed.Transaction.TransactionHashHex}");
    }
}
catch (ToriiApiException exception)
{
    Console.WriteLine($"Torii call failed with status {(int?)exception.StatusCode}: {exception.ResponseBody ?? exception.Message}");
}
catch (Exception exception)
{
    Console.WriteLine($"Torii call failed: {exception.Message}");
}
