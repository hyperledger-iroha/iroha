/**
 * Keys for possible error messages used by `unraw`.
 * Note: These do _not_ map to actual error object types. All errors thrown
 * are `SyntaxError`.
 */
export declare enum ErrorType {
    /**
     * Thrown when a badly formed Unicode escape sequence is found. Possible
     * reasons include the code being too short (`"\u25"`) or having invalid
     * characters (`"\u2$A5"`).
     */
    MalformedUnicode = "MALFORMED_UNICODE",
    /**
     * Thrown when a badly formed hexadecimal escape sequence is found. Possible
     * reasons include the code being too short (`"\x2"`) or having invalid
     * characters (`"\x2$"`).
     */
    MalformedHexadecimal = "MALFORMED_HEXADECIMAL",
    /**
     * Thrown when a Unicode code point escape sequence has too high of a code
     * point. The maximum code point allowed is `\u{10FFFF}`, so `\u{110000}` and
     * higher will throw this error.
     */
    CodePointLimit = "CODE_POINT_LIMIT",
    /**
     * Thrown when an octal escape sequences is encountered and `allowOctals` is
     * `false`. For example, `unraw("\234", false)`.
     */
    OctalDeprecation = "OCTAL_DEPRECATION",
    /**
     * Thrown only when a single backslash is found at the end of a string. For
     * example, `"\\"` or `"test\\x24\\"`.
     */
    EndOfString = "END_OF_STRING"
}
/** Map of error message names to the full text of the message. */
export declare const errorMessages: Readonly<Map<ErrorType, string>>;
