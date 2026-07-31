"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.AvroSchemaEncoder = void 0;
const AvroEncoder_1 = require("./AvroEncoder");
const AvroSchemaValidator_1 = require("./AvroSchemaValidator");
/**
 * Apache Avro binary encoder with schema validation and encoding.
 * Encodes values according to provided Avro schemas with proper validation.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
class AvroSchemaEncoder {
    constructor(writer) {
        this.writer = writer;
        this.namedSchemas = new Map();
        this.encoder = new AvroEncoder_1.AvroEncoder(writer);
        this.validator = new AvroSchemaValidator_1.AvroSchemaValidator();
    }
    /**
     * Encodes a value according to the provided schema.
     */
    encode(value, schema, selectedIndex) {
        this.writer.reset();
        this.namedSchemas.clear();
        // Validate schema first
        if (!this.validator.validateSchema(schema)) {
            throw new Error('Invalid Avro schema');
        }
        // Validate value against schema
        if (!this.validator.validateValue(value, schema)) {
            throw new Error('Value does not conform to schema');
        }
        this.collectNamedSchemas(schema);
        if (Array.isArray(schema) && selectedIndex !== undefined) {
            this.writeUnion(value, schema, selectedIndex);
        }
        else {
            this.writeValue(value, schema);
        }
        return this.writer.flush();
    }
    /**
     * Writes a null value with schema validation.
     */
    writeNull(schema) {
        this.validateSchemaType(schema, 'null');
        this.encoder.writeNull();
    }
    /**
     * Writes a boolean value with schema validation.
     */
    writeBoolean(value, schema) {
        this.validateSchemaType(schema, 'boolean');
        this.encoder.writeBoolean(value);
    }
    /**
     * Writes an int value with schema validation.
     */
    writeInt(value, schema) {
        this.validateSchemaType(schema, 'int');
        if (!Number.isInteger(value) || value < -2147483648 || value > 2147483647) {
            throw new Error('Value is not a valid 32-bit integer');
        }
        this.encoder.writeInt(value);
    }
    /**
     * Writes a long value with schema validation.
     */
    writeLong(value, schema) {
        this.validateSchemaType(schema, 'long');
        this.encoder.writeLong(value);
    }
    /**
     * Writes a float value with schema validation.
     */
    writeFloat(value, schema) {
        this.validateSchemaType(schema, 'float');
        this.encoder.writeFloat(value);
    }
    /**
     * Writes a double value with schema validation.
     */
    writeDouble(value, schema) {
        this.validateSchemaType(schema, 'double');
        this.encoder.writeDouble(value);
    }
    /**
     * Writes a bytes value with schema validation.
     */
    writeBytes(value, schema) {
        this.validateSchemaType(schema, 'bytes');
        this.encoder.writeBin(value);
    }
    /**
     * Writes a string value with schema validation.
     */
    writeString(value, schema) {
        this.validateSchemaType(schema, 'string');
        this.encoder.writeStr(value);
    }
    /**
     * Writes a record value with schema validation.
     */
    writeRecord(value, schema) {
        if (typeof schema === 'object' && schema.type !== 'record') {
            throw new Error('Schema is not a record schema');
        }
        const recordSchema = this.resolveSchema(schema);
        if (recordSchema.type !== 'record') {
            throw new Error('Schema is not a record schema');
        }
        for (let i = 0; i < recordSchema.fields.length; i++) {
            const field = recordSchema.fields[i];
            const fieldValue = value[field.name];
            if (fieldValue !== undefined) {
                this.writeValue(fieldValue, field.type);
            }
            else if (field.default !== undefined) {
                this.writeValue(field.default, field.type);
            }
            else {
                throw new Error(`Missing required field: ${field.name}`);
            }
        }
    }
    /**
     * Writes an enum value with schema validation.
     */
    writeEnum(value, schema) {
        if (typeof schema === 'object' && schema.type !== 'enum') {
            throw new Error('Schema is not an enum schema');
        }
        const enumSchema = this.resolveSchema(schema);
        if (enumSchema.type !== 'enum') {
            throw new Error('Schema is not an enum schema');
        }
        const index = enumSchema.symbols.indexOf(value);
        if (index === -1) {
            throw new Error(`Invalid enum value: ${value}`);
        }
        this.writeVarIntSigned(this.encodeZigZag32(index));
    }
    /**
     * Writes an array value with schema validation.
     */
    writeArray(value, schema) {
        if (typeof schema === 'object' && schema.type !== 'array') {
            throw new Error('Schema is not an array schema');
        }
        const arraySchema = this.resolveSchema(schema);
        if (arraySchema.type !== 'array') {
            throw new Error('Schema is not an array schema');
        }
        // Write array length
        this.writeVarIntUnsigned(value.length);
        // Write array items
        const length = value.length;
        for (let i = 0; i < length; i++) {
            this.writeValue(value[i], arraySchema.items);
        }
        // Write end-of-array marker
        this.writeVarIntUnsigned(0);
    }
    /**
     * Writes a map value with schema validation.
     */
    writeMap(value, schema) {
        if (typeof schema === 'object' && schema.type !== 'map') {
            throw new Error('Schema is not a map schema');
        }
        const mapSchema = this.resolveSchema(schema);
        if (mapSchema.type !== 'map') {
            throw new Error('Schema is not a map schema');
        }
        const entries = Object.entries(value);
        // Write map length
        this.writeVarIntUnsigned(entries.length);
        // Write map entries
        const length = entries.length;
        for (let i = 0; i < length; i++) {
            const entry = entries[i];
            this.encoder.writeStr(entry[0]);
            this.writeValue(entry[1], mapSchema.values);
        }
        // Write end-of-map marker
        this.writeVarIntUnsigned(0);
    }
    /**
     * Writes a union value with schema validation.
     */
    writeUnion(value, schema, selectedIndex) {
        if (!Array.isArray(schema)) {
            throw new Error('Schema is not a union schema');
        }
        let index = selectedIndex;
        if (index === undefined) {
            // Find the first matching schema in the union
            index = schema.findIndex((subSchema) => this.validator.validateValue(value, subSchema));
            if (index === -1) {
                throw new Error('Value does not match any schema in the union');
            }
        }
        if (index < 0 || index >= schema.length) {
            throw new Error('Invalid union index');
        }
        // Write union index
        this.writeVarIntSigned(this.encodeZigZag32(index));
        // Write the value according to the selected schema
        this.writeValue(value, schema[index]);
    }
    /**
     * Writes a fixed value with schema validation.
     */
    writeFixed(value, schema) {
        if (typeof schema === 'object' && schema.type !== 'fixed') {
            throw new Error('Schema is not a fixed schema');
        }
        const fixedSchema = this.resolveSchema(schema);
        if (fixedSchema.type !== 'fixed') {
            throw new Error('Schema is not a fixed schema');
        }
        if (value.length !== fixedSchema.size) {
            throw new Error(`Fixed value length ${value.length} does not match schema size ${fixedSchema.size}`);
        }
        this.writer.buf(value, value.length);
    }
    /**
     * Generic number writing with schema validation.
     */
    writeNumber(value, schema) {
        const resolvedSchema = this.resolveSchema(schema);
        const schemaType = typeof resolvedSchema === 'string'
            ? resolvedSchema
            : Array.isArray(resolvedSchema)
                ? 'union'
                : resolvedSchema.type;
        switch (schemaType) {
            case 'int':
                this.writeInt(value, schema);
                break;
            case 'long':
                this.writeLong(value, schema);
                break;
            case 'float':
                this.writeFloat(value, schema);
                break;
            case 'double':
                this.writeDouble(value, schema);
                break;
            default:
                throw new Error(`Schema type ${schemaType} is not a numeric type`);
        }
    }
    /**
     * Writes a value according to its schema.
     */
    writeValue(value, schema) {
        const resolvedSchema = this.resolveSchema(schema);
        if (typeof resolvedSchema === 'string') {
            switch (resolvedSchema) {
                case 'null':
                    this.encoder.writeNull();
                    break;
                case 'boolean':
                    this.encoder.writeBoolean(value);
                    break;
                case 'int':
                    this.encoder.writeInt(value);
                    break;
                case 'long':
                    this.encoder.writeLong(value);
                    break;
                case 'float':
                    this.encoder.writeFloat(value);
                    break;
                case 'double':
                    this.encoder.writeDouble(value);
                    break;
                case 'bytes':
                    this.encoder.writeBin(value);
                    break;
                case 'string':
                    this.encoder.writeStr(value);
                    break;
                default:
                    throw new Error(`Unknown primitive type: ${resolvedSchema}`);
            }
            return;
        }
        if (Array.isArray(resolvedSchema)) {
            this.writeUnion(value, resolvedSchema);
            return;
        }
        switch (resolvedSchema.type) {
            case 'record':
                this.writeRecord(value, resolvedSchema);
                break;
            case 'enum':
                this.writeEnum(value, resolvedSchema);
                break;
            case 'array':
                this.writeArray(value, resolvedSchema);
                break;
            case 'map':
                this.writeMap(value, resolvedSchema);
                break;
            case 'fixed':
                this.writeFixed(value, resolvedSchema);
                break;
            default:
                throw new Error(`Unknown schema type: ${resolvedSchema.type}`);
        }
    }
    validateSchemaType(schema, expectedType) {
        const resolvedSchema = this.resolveSchema(schema);
        const actualType = typeof resolvedSchema === 'string'
            ? resolvedSchema
            : Array.isArray(resolvedSchema)
                ? 'union'
                : resolvedSchema.type;
        if (actualType !== expectedType) {
            throw new Error(`Expected schema type ${expectedType}, got ${actualType}`);
        }
    }
    resolveSchema(schema) {
        if (typeof schema === 'string') {
            const namedSchema = this.namedSchemas.get(schema);
            return namedSchema || schema;
        }
        return schema;
    }
    collectNamedSchemas(schema) {
        if (typeof schema === 'string' || Array.isArray(schema)) {
            return;
        }
        if (typeof schema === 'object' && schema !== null) {
            switch (schema.type) {
                case 'record': {
                    const recordSchema = schema;
                    const recordFullName = this.getFullName(recordSchema.name, recordSchema.namespace);
                    this.namedSchemas.set(recordFullName, recordSchema);
                    recordSchema.fields.forEach((field) => this.collectNamedSchemas(field.type));
                    break;
                }
                case 'enum': {
                    const enumSchema = schema;
                    const enumFullName = this.getFullName(enumSchema.name, enumSchema.namespace);
                    this.namedSchemas.set(enumFullName, enumSchema);
                    break;
                }
                case 'fixed': {
                    const fixedSchema = schema;
                    const fixedFullName = this.getFullName(fixedSchema.name, fixedSchema.namespace);
                    this.namedSchemas.set(fixedFullName, fixedSchema);
                    break;
                }
                case 'array':
                    this.collectNamedSchemas(schema.items);
                    break;
                case 'map':
                    this.collectNamedSchemas(schema.values);
                    break;
            }
        }
    }
    getFullName(name, namespace) {
        return namespace ? `${namespace}.${name}` : name;
    }
    /**
     * Writes a variable-length integer using Avro's encoding (for lengths)
     */
    writeVarIntUnsigned(value) {
        const writer = this.writer;
        let n = value >>> 0; // Convert to unsigned 32-bit
        while (n >= 0x80) {
            writer.u8((n & 0x7f) | 0x80);
            n >>>= 7;
        }
        writer.u8(n & 0x7f);
    }
    /**
     * Writes a variable-length integer using Avro's encoding (for signed values with zigzag)
     */
    writeVarIntSigned(value) {
        const writer = this.writer;
        let n = value >>> 0; // Convert to unsigned 32-bit
        while (n >= 0x80) {
            writer.u8((n & 0x7f) | 0x80);
            n >>>= 7;
        }
        writer.u8(n & 0x7f);
    }
    /**
     * Encodes a 32-bit integer using zigzag encoding
     */
    encodeZigZag32(value) {
        return (value << 1) ^ (value >> 31);
    }
}
exports.AvroSchemaEncoder = AvroSchemaEncoder;
//# sourceMappingURL=AvroSchemaEncoder.js.map