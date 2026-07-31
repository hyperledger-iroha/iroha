import { JsonPackMpint } from '../JsonPackMpint';
import type { IReader, IReaderResettable } from '@jsonjoy.com/buffers/lib';
import type { BinaryJsonDecoder } from '../types';
/**
 * SSH 2.0 binary decoder for SSH protocol data types.
 * Implements SSH binary decoding according to RFC 4251.
 *
 * Key SSH decoding principles:
 * - Multi-byte quantities are transmitted in big-endian byte order (network byte order)
 * - Strings are length-prefixed with uint32
 * - No padding is used (unlike XDR)
 */
export declare class SshDecoder<R extends IReader & IReaderResettable = IReader & IReaderResettable> implements BinaryJsonDecoder {
    reader: R;
    constructor(reader?: R);
    read(uint8: Uint8Array): unknown;
    decode(uint8: Uint8Array): unknown;
    readAny(): unknown;
    /**
     * Reads an SSH boolean value as a single byte.
     * Returns true for non-zero values, false for zero.
     */
    readBoolean(): boolean;
    /**
     * Reads an SSH byte value (8-bit).
     */
    readByte(): number;
    /**
     * Reads an SSH uint32 value in big-endian format.
     */
    readUint32(): number;
    /**
     * Reads an SSH uint64 value in big-endian format.
     */
    readUint64(): bigint;
    /**
     * Reads an SSH string as binary data (Uint8Array).
     * Format: uint32 length + data bytes (no padding).
     */
    readBinStr(): Uint8Array;
    /**
     * Reads an SSH string with UTF-8 encoding.
     * Format: uint32 length + UTF-8 bytes (no padding).
     */
    readStr(): string;
    /**
     * Reads an SSH string with ASCII encoding.
     * Format: uint32 length + ASCII bytes (no padding).
     */
    readAsciiStr(): string;
    /**
     * Reads an SSH mpint (multiple precision integer).
     * Format: uint32 length + data bytes in two's complement format, MSB first.
     */
    readMpint(): JsonPackMpint;
    /**
     * Reads an SSH name-list.
     * Format: uint32 length + comma-separated names.
     * Returns an array of name strings.
     */
    readNameList(): string[];
    /**
     * Reads binary data as SSH string (alias for readBinStr)
     */
    readBin(): Uint8Array;
}
