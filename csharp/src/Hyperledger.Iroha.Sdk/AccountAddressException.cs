namespace Hyperledger.Iroha;

public sealed class AccountAddressException : FormatException
{
    public AccountAddressException(AccountAddressErrorCode code, string message, Exception? innerException = null)
        : base(message, innerException)
    {
        Code = code;
    }

    public AccountAddressErrorCode Code { get; }
}
