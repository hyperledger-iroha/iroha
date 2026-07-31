import type { XdrSchema } from './types';
/**
 * XDR schema validator for validating XDR schemas and values according to RFC 4506.
 */
export declare class XdrSchemaValidator {
    /**
     * Validates an XDR schema structure.
     */
    validateSchema(schema: XdrSchema): boolean;
    /**
     * Validates if a value conforms to the given XDR schema.
     */
    validateValue(value: unknown, schema: XdrSchema): boolean;
    private validateSchemaInternal;
    private validateEnumSchema;
    private validateOpaqueSchema;
    private validateVarlenOpaqueSchema;
    private validateStringSchema;
    private validateArraySchema;
    private validateVarlenArraySchema;
    private validateStructSchema;
    private validateUnionSchema;
    private validateOptionalSchema;
    private validateConstantSchema;
    private validateValueInternal;
}
