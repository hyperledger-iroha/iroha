import { Reader } from '@jsonjoy.com/buffers/lib/Reader';
import type { AvroSchema } from './types';
/**
 * Apache Avro binary decoder with schema validation and decoding.
 * Decodes values according to provided Avro schemas with proper validation.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
export declare class AvroSchemaDecoder {
    readonly reader: Reader;
    private decoder;
    private validator;
    private namedSchemas;
    constructor(reader?: Reader);
    /**
     * Decodes a value according to the provided schema.
     */
    decode(data: Uint8Array, schema: AvroSchema): unknown;
    /**
     * Reads a value according to its schema.
     */
    private readValue;
    /**
     * Reads a record value according to the record schema.
     */
    private readRecord;
    /**
     * Reads an enum value according to the enum schema.
     */
    private readEnum;
    /**
     * Reads an array value according to the array schema.
     */
    private readArray;
    /**
     * Reads a map value according to the map schema.
     */
    private readMap;
    /**
     * Reads a union value according to the union schema.
     */
    private readUnion;
    /**
     * Reads a fixed value according to the fixed schema.
     */
    private readFixed;
    /**
     * Reads a null value with schema validation.
     */
    readNull(schema: AvroSchema): null;
    /**
     * Reads a boolean value with schema validation.
     */
    readBoolean(schema: AvroSchema): boolean;
    /**
     * Reads an int value with schema validation.
     */
    readInt(schema: AvroSchema): number;
    /**
     * Reads a long value with schema validation.
     */
    readLong(schema: AvroSchema): number | bigint;
    /**
     * Reads a float value with schema validation.
     */
    readFloat(schema: AvroSchema): number;
    /**
     * Reads a double value with schema validation.
     */
    readDouble(schema: AvroSchema): number;
    /**
     * Reads a bytes value with schema validation.
     */
    readBytes(schema: AvroSchema): Uint8Array;
    /**
     * Reads a string value with schema validation.
     */
    readString(schema: AvroSchema): string;
    private validateSchemaType;
    private resolveSchema;
    private collectNamedSchemas;
    private getFullName;
}
