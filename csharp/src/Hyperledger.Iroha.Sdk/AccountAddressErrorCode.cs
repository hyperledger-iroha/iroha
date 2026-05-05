namespace Hyperledger.Iroha;

public enum AccountAddressErrorCode
{
    CanonicalHashFailure,
    ChecksumMismatch,
    InvalidDomainLabel,
    InvalidI105Base,
    InvalidI105Char,
    InvalidI105Digit,
    InvalidI105Discriminant,
    InvalidLength,
    InvalidNormVersion,
    InvalidPublicKey,
    InvalidHeaderVersion,
    I105TooShort,
    MissingI105Sentinel,
    UnexpectedNetworkPrefix,
    UnexpectedTrailingBytes,
    UnknownAddressClass,
    UnknownControllerTag,
    UnknownCurve,
    UnsupportedAddressFormat,
    UnsupportedAlgorithm,
}
