import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers/lib';
import type { XdrSchema, XdrEnumSchema, XdrOpaqueSchema, XdrVarlenOpaqueSchema, XdrStringSchema, XdrArraySchema, XdrVarlenArraySchema, XdrStructSchema, XdrUnionSchema, XdrOptionalSchema } from './types';
export declare class XdrSchemaEncoder {
    readonly writer: IWriter & IWriterGrowable;
    private encoder;
    constructor(writer: IWriter & IWriterGrowable);
    encode(value: unknown, schema: XdrSchema): Uint8Array;
    writeVoid(schema: XdrSchema): void;
    writeInt(value: number, schema: XdrSchema): void;
    writeUnsignedInt(value: number, schema: XdrSchema): void;
    writeBoolean(value: boolean, schema: XdrSchema): void;
    writeHyper(value: number | bigint, schema: XdrSchema): void;
    writeUnsignedHyper(value: number | bigint, schema: XdrSchema): void;
    writeFloat(value: number, schema: XdrSchema): void;
    writeDouble(value: number, schema: XdrSchema): void;
    writeQuadruple(value: number, schema: XdrSchema): void;
    writeEnum(value: string, schema: XdrEnumSchema): void;
    writeOpaque(value: Uint8Array, schema: XdrOpaqueSchema): void;
    writeVarlenOpaque(value: Uint8Array, schema: XdrVarlenOpaqueSchema): void;
    writeString(value: string, schema: XdrStringSchema): void;
    writeArray(value: unknown[], schema: XdrArraySchema): void;
    writeVarlenArray(value: unknown[], schema: XdrVarlenArraySchema): void;
    writeStruct(value: Record<string, unknown>, schema: XdrStructSchema): void;
    writeUnion(value: unknown, schema: XdrUnionSchema, discriminant: number | string | boolean): void;
    /**
     * Writes optional-data value (RFC 1832 Section 3.19).
     * Optional-data is syntactic sugar for a union with boolean discriminant.
     * If value is null/undefined, writes FALSE; otherwise writes TRUE and the value.
     */
    writeOptional(value: unknown, schema: XdrOptionalSchema): void;
    writeNumber(value: number, schema: XdrSchema): void;
    private writeValue;
    private validateSchemaType;
    private writeDiscriminant;
}
