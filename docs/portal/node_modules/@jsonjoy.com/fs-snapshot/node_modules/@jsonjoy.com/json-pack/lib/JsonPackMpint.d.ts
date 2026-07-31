/**
 * Represents an SSH multiprecision integer (mpint).
 *
 * An mpint is stored in two's complement format, 8 bits per byte, MSB first.
 * According to RFC 4251:
 * - Negative numbers have the value 1 as the most significant bit of the first byte
 * - If the most significant bit would be set for a positive number, the number MUST be preceded by a zero byte
 * - Unnecessary leading bytes with the value 0 or 255 MUST NOT be included
 * - The value zero MUST be stored as a string with zero bytes of data
 */
export declare class JsonPackMpint {
    /**
     * The raw bytes representing the mpint in two's complement format, MSB first.
     */
    readonly data: Uint8Array;
    constructor(data: Uint8Array);
    /**
     * Create an mpint from a BigInt value.
     */
    static fromBigInt(value: bigint): JsonPackMpint;
    /**
     * Convert the mpint to a BigInt value.
     */
    toBigInt(): bigint;
    /**
     * Create an mpint from a number (limited to safe integer range).
     */
    static fromNumber(value: number): JsonPackMpint;
    /**
     * Convert the mpint to a number (throws if out of safe integer range).
     */
    toNumber(): number;
}
