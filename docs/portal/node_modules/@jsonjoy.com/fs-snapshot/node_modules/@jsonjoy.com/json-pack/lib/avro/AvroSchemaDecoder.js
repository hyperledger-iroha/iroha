"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.AvroSchemaDecoder = void 0;
const Reader_1 = require("@jsonjoy.com/buffers/lib/Reader");
const AvroDecoder_1 = require("./AvroDecoder");
const AvroSchemaValidator_1 = require("./AvroSchemaValidator");
/**
 * Apache Avro binary decoder with schema validation and decoding.
 * Decodes values according to provided Avro schemas with proper validation.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
class AvroSchemaDecoder {
    constructor(reader = new Reader_1.Reader()) {
        this.reader = reader;
        this.namedSchemas = new Map();
        this.decoder = new AvroDecoder_1.AvroDecoder();
        this.decoder.reader = reader;
        this.validator = new AvroSchemaValidator_1.AvroSchemaValidator();
    }
    /**
     * Decodes a value according to the provided schema.
     */
    decode(data, schema) {
        this.reader.reset(data);
        this.namedSchemas.clear();
        // Validate schema first
        if (!this.validator.validateSchema(schema)) {
            throw new Error('Invalid Avro schema');
        }
        this.collectNamedSchemas(schema);
        return this.readValue(schema);
    }
    /**
     * Reads a value according to its schema.
     */
    readValue(schema) {
        const resolvedSchema = this.resolveSchema(schema);
        if (typeof resolvedSchema === 'string') {
            switch (resolvedSchema) {
                case 'null':
                    return this.decoder.readNull();
                case 'boolean':
                    return this.decoder.readBoolean();
                case 'int':
                    return this.decoder.readInt();
                case 'long':
                    return this.decoder.readLong();
                case 'float':
                    return this.decoder.readFloat();
                case 'double':
                    return this.decoder.readDouble();
                case 'bytes':
                    return this.decoder.readBytes();
                case 'string':
                    return this.decoder.readString();
                default:
                    throw new Error(`Unknown primitive type: ${resolvedSchema}`);
            }
        }
        if (Array.isArray(resolvedSchema)) {
            return this.readUnion(resolvedSchema);
        }
        switch (resolvedSchema.type) {
            case 'record':
                return this.readRecord(resolvedSchema);
            case 'enum':
                return this.readEnum(resolvedSchema);
            case 'array':
                return this.readArray(resolvedSchema);
            case 'map':
                return this.readMap(resolvedSchema);
            case 'fixed':
                return this.readFixed(resolvedSchema);
            default:
                throw new Error(`Unknown schema type: ${resolvedSchema.type}`);
        }
    }
    /**
     * Reads a record value according to the record schema.
     */
    readRecord(schema) {
        const result = {};
        for (let i = 0; i < schema.fields.length; i++) {
            const field = schema.fields[i];
            try {
                result[field.name] = this.readValue(field.type);
            }
            catch (error) {
                throw new Error(`Error reading field '${field.name}': ${error.message}`);
            }
        }
        return result;
    }
    /**
     * Reads an enum value according to the enum schema.
     */
    readEnum(schema) {
        const index = this.decoder.readEnum();
        if (index < 0 || index >= schema.symbols.length) {
            throw new Error(`Invalid enum index ${index} for enum with ${schema.symbols.length} symbols`);
        }
        return schema.symbols[index];
    }
    /**
     * Reads an array value according to the array schema.
     */
    readArray(schema) {
        return this.decoder.readArray(() => this.readValue(schema.items));
    }
    /**
     * Reads a map value according to the map schema.
     */
    readMap(schema) {
        return this.decoder.readMap(() => this.readValue(schema.values));
    }
    /**
     * Reads a union value according to the union schema.
     */
    readUnion(schema) {
        const schemaReaders = schema.map((subSchema) => () => this.readValue(subSchema));
        const result = this.decoder.readUnion(schemaReaders);
        return result.value;
    }
    /**
     * Reads a fixed value according to the fixed schema.
     */
    readFixed(schema) {
        return this.decoder.readFixed(schema.size);
    }
    /**
     * Reads a null value with schema validation.
     */
    readNull(schema) {
        this.validateSchemaType(schema, 'null');
        return this.decoder.readNull();
    }
    /**
     * Reads a boolean value with schema validation.
     */
    readBoolean(schema) {
        this.validateSchemaType(schema, 'boolean');
        return this.decoder.readBoolean();
    }
    /**
     * Reads an int value with schema validation.
     */
    readInt(schema) {
        this.validateSchemaType(schema, 'int');
        const value = this.decoder.readInt();
        if (!Number.isInteger(value) || value < -2147483648 || value > 2147483647) {
            throw new Error('Decoded value is not a valid 32-bit integer');
        }
        return value;
    }
    /**
     * Reads a long value with schema validation.
     */
    readLong(schema) {
        this.validateSchemaType(schema, 'long');
        return this.decoder.readLong();
    }
    /**
     * Reads a float value with schema validation.
     */
    readFloat(schema) {
        this.validateSchemaType(schema, 'float');
        return this.decoder.readFloat();
    }
    /**
     * Reads a double value with schema validation.
     */
    readDouble(schema) {
        this.validateSchemaType(schema, 'double');
        return this.decoder.readDouble();
    }
    /**
     * Reads a bytes value with schema validation.
     */
    readBytes(schema) {
        this.validateSchemaType(schema, 'bytes');
        return this.decoder.readBytes();
    }
    /**
     * Reads a string value with schema validation.
     */
    readString(schema) {
        this.validateSchemaType(schema, 'string');
        return this.decoder.readString();
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
}
exports.AvroSchemaDecoder = AvroSchemaDecoder;
//# sourceMappingURL=AvroSchemaDecoder.js.map