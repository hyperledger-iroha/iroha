import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers/lib';
import type { BinaryJsonEncoder } from '../types';
/**
 * Apache Avro binary encoder for basic value encoding.
 * Implements the Avro binary encoding specification without schema validation.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
export declare class AvroEncoder implements BinaryJsonEncoder {
    readonly writer: IWriter & IWriterGrowable;
    constructor(writer: IWriter & IWriterGrowable);
    encode(value: unknown): Uint8Array;
    /**
     * Called when the encoder encounters a value that it does not know how to encode.
     */
    writeUnknown(value: unknown): void;
    writeAny(value: unknown): void;
    /**
     * Writes an Avro null value.
     */
    writeNull(): void;
    /**
     * Writes an Avro boolean value.
     */
    writeBoolean(bool: boolean): void;
    /**
     * Writes an Avro int value using zigzag encoding.
     */
    writeInt(int: number): void;
    /**
     * Writes an Avro long value using zigzag encoding.
     */
    writeLong(long: number | bigint): void;
    /**
     * Writes an Avro float value using IEEE 754 single-precision.
     */
    writeFloatAvro(float: number): void;
    /**
     * Writes an Avro double value using IEEE 754 double-precision.
     */
    writeDouble(double: number): void;
    /**
     * Writes an Avro bytes value with length-prefixed encoding.
     */
    writeBin(bytes: Uint8Array): void;
    /**
     * Writes an Avro string value with UTF-8 encoding and length prefix.
     */
    writeStr(str: string): void;
    /**
     * Writes an Avro array with length-prefixed encoding.
     */
    writeArr(arr: unknown[]): void;
    /**
     * Writes an Avro map with length-prefixed encoding.
     */
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
     * Writes a float value (interface method)
     */
    writeFloat(float: number): void;
    /**
     * Writes a float value using IEEE 754 single-precision.
     */
    private writeFloatValue;
    /**
     * Writes an ASCII string (same as regular string in Avro)
     */
    writeAsciiStr(str: string): void;
    /**
     * Encodes a variable-length integer (for signed values with zigzag)
     */
    private writeVarIntSigned;
    /**
     * Encodes a variable-length integer (for unsigned values like lengths)
     */
    private writeVarIntUnsigned;
    /**
     * Encodes a variable-length long using Avro's encoding
     */
    private writeVarLong;
    /**
     * Encodes a 32-bit integer using zigzag encoding
     */
    private encodeZigZag32;
    /**
     * Encodes a 64-bit integer using zigzag encoding
     */
    private encodeZigZag64;
}
