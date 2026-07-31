import type { IReader, IReaderResettable } from '@jsonjoy.com/buffers/lib';
import type { BinaryJsonDecoder } from '../types';
/**
 * XDR (External Data Representation) binary decoder for basic value decoding.
 * Implements XDR binary decoding according to RFC 4506.
 *
 * Key XDR decoding principles:
 * - All data types are aligned to 4-byte boundaries
 * - Multi-byte quantities are transmitted in big-endian byte order
 * - Strings and opaque data are padded to 4-byte boundaries
 * - Variable-length arrays and strings are preceded by their length
 */
export declare class XdrDecoder<R extends IReader & IReaderResettable = IReader & IReaderResettable> implements BinaryJsonDecoder {
    reader: R;
    constructor(reader?: R);
    read(uint8: Uint8Array): unknown;
    decode(uint8: Uint8Array): unknown;
    readAny(): unknown;
    /**
     * Reads an XDR void value (no data is actually read).
     */
    readVoid(): void;
    /**
     * Reads an XDR boolean value as a 4-byte integer.
     * Returns true for non-zero values, false for zero.
     */
    readBoolean(): boolean;
    /**
     * Reads an XDR signed 32-bit integer in big-endian format.
     */
    readInt(): number;
    /**
     * Reads an XDR unsigned 32-bit integer in big-endian format.
     */
    readUnsignedInt(): number;
    /**
     * Reads an XDR signed 64-bit integer (hyper) in big-endian format.
     */
    readHyper(): bigint;
    /**
     * Reads an XDR unsigned 64-bit integer (unsigned hyper) in big-endian format.
     */
    readUnsignedHyper(): bigint;
    /**
     * Reads an XDR float value using IEEE 754 single-precision in big-endian format.
     */
    readFloat(): number;
    /**
     * Reads an XDR double value using IEEE 754 double-precision in big-endian format.
     */
    readDouble(): number;
    /**
     * Reads an XDR quadruple value (128-bit float).
     * Note: JavaScript doesn't have native 128-bit float support.
     */
    readQuadruple(): number;
    /**
     * Reads XDR opaque data with known fixed length.
     * Data is padded to 4-byte boundary but only the actual data is returned.
     */
    readOpaque(size: number): Uint8Array;
    /**
     * Reads XDR variable-length opaque data.
     * Length is read first, followed by data padded to 4-byte boundary.
     */
    readVarlenOpaque(): Uint8Array;
    /**
     * Reads an XDR string with UTF-8 encoding.
     * Length is read first, followed by UTF-8 bytes padded to 4-byte boundary.
     */
    readString(): string;
    /**
     * Reads an XDR enum value as an unsigned integer.
     */
    readEnum(): number;
    /**
     * Reads a fixed-size array of elements.
     * Caller must provide the decode function for each element.
     */
    readArray<T>(size: number, elementReader: () => T): T[];
    /**
     * Reads a variable-length array of elements.
     * Length is read first, followed by elements.
     */
    readVarlenArray<T>(elementReader: () => T): T[];
}
