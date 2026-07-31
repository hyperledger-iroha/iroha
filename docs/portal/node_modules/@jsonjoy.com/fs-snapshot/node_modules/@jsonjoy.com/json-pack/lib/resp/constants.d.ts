export declare const enum RESP {
    R = 13,// \r
    N = 10,// \n
    RN = 3338,// \r\n
    NULL = 95,// _
    BOOL = 35,// #
    INT = 58,// :
    BIG = 40,// (
    FLOAT = 44,// ,
    STR_SIMPLE = 43,// +
    STR_BULK = 36,// $
    STR_VERBATIM = 61,// =
    ERR_SIMPLE = 45,// -
    ERR_BULK = 33,// !
    ARR = 42,// *
    SET = 126,// ~
    OBJ = 37,// %
    PUSH = 62,// >
    ATTR = 124,// |
    PLUS = 43,// +
    MINUS = 45
}
