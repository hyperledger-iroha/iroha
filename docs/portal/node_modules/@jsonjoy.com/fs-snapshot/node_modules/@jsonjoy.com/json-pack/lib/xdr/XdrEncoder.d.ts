import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers/lib';
import type { BinaryJsonEncoder } from '../types';
/**
 * XDR (External Data Representation) binary encoder for basic value encoding.
 * Implements XDR binary encoding according to RFC 4506.
 *
 * Key XDR encoding principles:
 * - All data types are aligned to 4-byte boundaries
 * - Multi-byte quantities are transmitted in big-endian byte order
 * - Strings and opaque data are padded to 4-byte boundaries
 * - Variable-length arrays and strings are preceded by their length
 */
export declare class XdrEncoder implements BinaryJsonEncoder {
    readonly writer: IWriter & IWriterGrowable;
    constructor(writer: IWriter & IWriterGrowable);
    encode(value: unknown): Uint8Array;
    /**
     * Called when the encoder encounters a value that it does not know how to encode.
     */
    writeUnknown(value: unknown): void;
    writeAny(value: unknown): void;
    /**
     * Writes an XDR void value (no data is actually written).
     */
    writeVoid(): void;
    /**
     * Writes an XDR null value (for interface compatibility).
     */
    writeNull(): void;
    /**
     * Writes an XDR boolean value as a 4-byte integer.
     */
    writeBoolean(bool: boolean): void;
    /**
     * Writes an XDR signed 32-bit integer in big-endian format.
     */
    writeInt(int: number): void;
    /**
     * Writes an XDR unsigned 32-bit integer in big-endian format.
     */
    writeUnsignedInt(uint: number): void;
    /**
     * Writes an XDR signed 64-bit integer (hyper) in big-endian format.
     */
    writeHyper(hyper: number | bigint): void;
    /**
     * Writes an XDR unsigned 64-bit integer (unsigned hyper) in big-endian format.
     */
    writeUnsignedHyper(uhyper: number | bigint): void;
    /**
     * Writes an XDR float value using IEEE 754 single-precision in big-endian format.
     */
    writeFloat(float: number): void;
    /**
     * Writes an XDR double value using IEEE 754 double-precision in big-endian format.
     */
    writeDouble(double: number): void;
    /**
     * Writes an XDR quadruple value (128-bit float).
     * Note: JavaScript doesn't have native 128-bit float support.
     */
    writeQuadruple(quad: number): void;
    /**
     * Writes XDR opaque data with fixed length.
     * Data is padded to 4-byte boundary.
     */
    writeOpaque(data: Uint8Array): void;
    /**
     * Writes XDR variable-length opaque data.
     * Length is written first, followed by data padded to 4-byte boundary.
     */
    writeVarlenOpaque(data: Uint8Array): void;
    /**
     * Writes an XDR string with UTF-8 encoding.
     * Length is written first, followed by UTF-8 bytes padded to 4-byte boundary.
     */
    writeStr(str: string): void;
    writeArr(arr: unknown[]): void;
    writeObj(obj: Record<string, unknown>): void;
    /**
     * Generic number writing - determines type based on value
     */
    writeNumber(num: number): void;
    /**
     * Writes an integer value
     */
    writeInteger(int: number): void;
    /**
     * Writes an unsigned integer value
     */
    writeUInteger(uint: number): void;
    /**
     * Writes binary data
     */
    writeBin(buf: Uint8Array): void;
    /**
     * Writes an ASCII string (same as regular string in XDR)
     */
    writeAsciiStr(str: string): void;
}
