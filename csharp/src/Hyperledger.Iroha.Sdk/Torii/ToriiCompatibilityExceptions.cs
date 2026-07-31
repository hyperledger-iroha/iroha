namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Raised when a Torii mutation target advertises a data-model version that this SDK cannot encode.
/// </summary>
public sealed class ToriiDataModelMismatchException : InvalidOperationException
{
    public ToriiDataModelMismatchException(int expected, int actual)
        : base($"Torii node data_model_version {actual} does not match client version {expected}.")
    {
        Expected = expected;
        Actual = actual;
    }

    public int Expected { get; }

    public int Actual { get; }
}

/// <summary>
/// Raised when a Torii mutation target advertises a different signed-transaction schema.
/// </summary>
public sealed class ToriiTransactionSchemaMismatchException : InvalidOperationException
{
    public ToriiTransactionSchemaMismatchException(string expected, string actual)
        : base(
            "Torii node signed_transaction_schema_hash_hex "
            + $"{actual} does not match client schema {expected}.")
    {
        Expected = expected;
        Actual = actual;
    }

    public string Expected { get; }

    public string Actual { get; }
}
