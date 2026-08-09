namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    private static void ValidateExplorerCursor(ToriiExplorerCursorQuery query, string paramName)
    {
        if (query.Cursor is not null)
        {
            try
            {
                ToriiExplorerDirectMetadata.RequireCanonicalExplorerCursor(query.Cursor, nameof(query.Cursor));
            }
            catch (ArgumentException exception)
            {
                throw new ArgumentException(exception.Message, paramName, exception);
            }
        }

        if (query.Limit.HasValue)
        {
            try
            {
                ToriiExplorerDirectMetadata.RequireExplorerCursorLimit(query.Limit.Value, nameof(query.Limit));
            }
            catch (ArgumentOutOfRangeException exception)
            {
                throw new ArgumentOutOfRangeException(paramName, exception.Message);
            }
        }
    }
}
