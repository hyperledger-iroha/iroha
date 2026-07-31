import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers/lib';
import type { AvroSchema, AvroRecordSchema, AvroEnumSchema, AvroArraySchema, AvroMapSchema, AvroUnionSchema, AvroFixedSchema, AvroNullSchema } from './types';
/**
 * Apache Avro binary encoder with schema validation and encoding.
 * Encodes values according to provided Avro schemas with proper validation.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
export declare class AvroSchemaEncoder {
    readonly writer: IWriter & IWriterGrowable;
    private encoder;
    private validator;
    private namedSchemas;
    constructor(writer: IWriter & IWriterGrowable);
    /**
     * Encodes a value according to the provided schema.
     */
    encode(value: unknown, schema: AvroSchema, selectedIndex?: number): Uint8Array;
    /**
     * Writes a null value with schema validation.
     */
    writeNull(schema: AvroNullSchema | AvroSchema): void;
    /**
     * Writes a boolean value with schema validation.
     */
    writeBoolean(value: boolean, schema: AvroSchema): void;
    /**
     * Writes an int value with schema validation.
     */
    writeInt(value: number, schema: AvroSchema): void;
    /**
     * Writes a long value with schema validation.
     */
    writeLong(value: number | bigint, schema: AvroSchema): void;
    /**
     * Writes a float value with schema validation.
     */
    writeFloat(value: number, schema: AvroSchema): void;
    /**
     * Writes a double value with schema validation.
     */
    writeDouble(value: number, schema: AvroSchema): void;
    /**
     * Writes a bytes value with schema validation.
     */
    writeBytes(value: Uint8Array, schema: AvroSchema): void;
    /**
     * Writes a string value with schema validation.
     */
    writeString(value: string, schema: AvroSchema): void;
    /**
     * Writes a record value with schema validation.
     */
    writeRecord(value: Record<string, unknown>, schema: AvroRecordSchema): void;
    /**
     * Writes an enum value with schema validation.
     */
    writeEnum(value: string, schema: AvroEnumSchema): void;
    /**
     * Writes an array value with schema validation.
     */
    writeArray(value: unknown[], schema: AvroArraySchema): void;
    /**
     * Writes a map value with schema validation.
     */
    writeMap(value: Record<string, unknown>, schema: AvroMapSchema): void;
    /**
     * Writes a union value with schema validation.
     */
    writeUnion(value: unknown, schema: AvroUnionSchema, selectedIndex?: number): void;
    /**
     * Writes a fixed value with schema validation.
     */
    writeFixed(value: Uint8Array, schema: AvroFixedSchema): void;
    /**
     * Generic number writing with schema validation.
     */
    writeNumber(value: number, schema: AvroSchema): void;
    /**
     * Writes a value according to its schema.
     */
    private writeValue;
    private validateSchemaType;
    private resolveSchema;
    private collectNamedSchemas;
    private getFullName;
    /**
     * Writes a variable-length integer using Avro's encoding (for lengths)
     */
    private writeVarIntUnsigned;
    /**
     * Writes a variable-length integer using Avro's encoding (for signed values with zigzag)
     */
    private writeVarIntSigned;
    /**
     * Encodes a 32-bit integer using zigzag encoding
     */
    private encodeZigZag32;
}
