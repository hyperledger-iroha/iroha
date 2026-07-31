import type { IReader, IReaderResettable } from '@jsonjoy.com/buffers/lib';
import type { XdrSchema } from './types';
/**
 * XDR (External Data Representation) schema-aware decoder.
 * Decodes values according to provided XDR schemas with proper validation.
 * Based on RFC 4506 specification.
 */
export declare class XdrSchemaDecoder {
    readonly reader: IReader & IReaderResettable;
    private decoder;
    constructor(reader?: IReader & IReaderResettable);
    /**
     * Decodes a value according to the provided schema.
     */
    decode(data: Uint8Array, schema: XdrSchema): unknown;
    /**
     * Reads a value according to its schema.
     */
    private readValue;
    /**
     * Reads an enum value according to the enum schema.
     */
    private readEnum;
    /**
     * Reads opaque data according to the opaque schema.
     */
    private readOpaque;
    /**
     * Reads variable-length opaque data according to the schema.
     */
    private readVarlenOpaque;
    /**
     * Reads a string according to the string schema.
     */
    private readString;
    /**
     * Reads a fixed-size array according to the array schema.
     */
    private readArray;
    /**
     * Reads a variable-length array according to the schema.
     */
    private readVarlenArray;
    /**
     * Reads a struct according to the struct schema.
     */
    private readStruct;
    /**
     * Reads a union according to the union schema.
     */
    private readUnion;
    /**
     * Reads optional-data according to the optional schema (RFC 1832 Section 3.19).
     * Optional-data is syntactic sugar for a union with boolean discriminant.
     * Returns null if opted is FALSE, otherwise returns the decoded value.
     */
    private readOptional;
}
