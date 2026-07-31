import { Reader } from '@jsonjoy.com/buffers/lib/Reader';
import type { BinaryJsonDecoder } from '../types';
/**
 * Apache Avro binary decoder for basic value decoding.
 * Implements the Avro binary decoding specification without schema validation.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
export declare class AvroDecoder implements BinaryJsonDecoder {
    reader: Reader;
    read(uint8: Uint8Array): unknown;
    decode(uint8: Uint8Array): unknown;
    /**
     * Generic method to read any value - typically used when schema type is unknown
     */
    readAny(): unknown;
    /**
     * Reads an Avro null value.
     */
    readNull(): null;
    /**
     * Reads an Avro boolean value.
     */
    readBoolean(): boolean;
    /**
     * Reads an Avro int value using zigzag decoding.
     */
    readInt(): number;
    /**
     * Reads an Avro long value using zigzag decoding.
     */
    readLong(): number | bigint;
    /**
     * Reads an Avro float value using IEEE 754 single-precision.
     */
    readFloat(): number;
    /**
     * Reads an Avro double value using IEEE 754 double-precision.
     */
    readDouble(): number;
    /**
     * Reads an Avro bytes value with length-prefixed encoding.
     */
    readBytes(): Uint8Array;
    /**
     * Reads an Avro string value with UTF-8 encoding and length prefix.
     */
    readString(): string;
    /**
     * Reads an Avro array with length-prefixed encoding.
     * The itemReader function is called for each array item.
     */
    readArray<T>(itemReader: () => T): T[];
    /**
     * Reads an Avro map with length-prefixed encoding.
     * The valueReader function is called for each map value.
     */
    readMap<T>(valueReader: () => T): Record<string, T>;
    /**
     * Reads an Avro union value.
     * Returns an object with index and value.
     */
    readUnion<T>(schemaReaders: Array<() => T>): {
        index: number;
        value: T;
    };
    /**
     * Reads an Avro enum value.
     * Returns the symbol index.
     */
    readEnum(): number;
    /**
     * Reads an Avro fixed value with specified length.
     */
    readFixed(size: number): Uint8Array;
    /**
     * Reads an Avro record.
     * The fieldReaders array contains functions to read each field in order.
     */
    readRecord<T>(fieldReaders: Array<() => any>): T;
    /**
     * Reads a variable-length integer (for unsigned values like lengths)
     */
    private readVarIntUnsigned;
    /**
     * Reads a variable-length long
     */
    private readVarLong;
    /**
     * Decodes a 32-bit integer using zigzag decoding
     */
    private decodeZigZag32;
    /**
     * Decodes a 64-bit integer using zigzag decoding
     */
    private decodeZigZag64;
}
