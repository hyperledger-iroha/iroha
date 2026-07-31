export declare const enum MaxEncodingOverhead {
    Null = 4,// Literal "null"
    Boolean = 5,// Literal "false"
    Number = 22,// Literal "1.1111111111111111e+21" = JSON.stringify(1111111111111111111112)
    String = 5,// As per TLV: 1 byte for type, 4 bytes for length.
    StringLengthMultiplier = 5,// 4x UTF-8 overhead + 1.3x Base64 overhead, plus, 1 byte for each non-ASCII character.
    Binary = 41,// 2 for two quotes, 37 for "data:application/octet-stream;base64,'" literal, 2 bytes for Base64 padding.
    BinaryLengthMultiplier = 2,// 1.3x Base64 overhead.
    Array = 5,// As per TLV: 1 byte for type, 4 bytes for length.
    ArrayElement = 1,// Separator "," literal.
    Object = 5,// As per TLV: 1 byte for type, 4 bytes for length.
    ObjectElement = 2,// 1 byte for Key-value separator ":" literal, and 1 byte for separator "," literal.
    Undefined = 45
}
export declare const maxEncodingCapacity: (value: unknown) => number;
