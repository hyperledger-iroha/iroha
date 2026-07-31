import type { AvroSchema } from './types';
/**
 * Validates Apache Avro schemas according to the specification.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
export declare class AvroSchemaValidator {
    private namedSchemas;
    /**
     * Validates an Avro schema and resolves named schema references.
     */
    validateSchema(schema: AvroSchema): boolean;
    /**
     * Validates that a value conforms to the given Avro schema.
     */
    validateValue(value: unknown, schema: AvroSchema): boolean;
    private validateSchemaInternal;
    private validateStringSchema;
    private validateUnionSchema;
    private validateNullSchema;
    private validateBooleanSchema;
    private validateIntSchema;
    private validateLongSchema;
    private validateFloatSchema;
    private validateDoubleSchema;
    private validateBytesSchema;
    private validateStringTypeSchema;
    private validateRecordSchema;
    private validateRecordField;
    private validateEnumSchema;
    private validateArraySchema;
    private validateMapSchema;
    private validateFixedSchema;
    private validateValueAgainstSchema;
    private validateValueAgainstStringSchema;
    private validateValueAgainstRecord;
    private validateValueAgainstEnum;
    private validateValueAgainstArray;
    private validateValueAgainstMap;
    private validateValueAgainstFixed;
    private getSchemaTypeName;
    private getFullName;
}
