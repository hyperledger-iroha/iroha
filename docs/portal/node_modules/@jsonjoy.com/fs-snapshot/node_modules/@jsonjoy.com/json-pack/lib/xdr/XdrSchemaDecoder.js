"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.XdrSchemaDecoder = void 0;
const Reader_1 = require("@jsonjoy.com/buffers/lib/Reader");
const XdrDecoder_1 = require("./XdrDecoder");
const XdrUnion_1 = require("./XdrUnion");
/**
 * XDR (External Data Representation) schema-aware decoder.
 * Decodes values according to provided XDR schemas with proper validation.
 * Based on RFC 4506 specification.
 */
class XdrSchemaDecoder {
    constructor(reader = new Reader_1.Reader()) {
        this.reader = reader;
        this.decoder = new XdrDecoder_1.XdrDecoder(reader);
    }
    /**
     * Decodes a value according to the provided schema.
     */
    decode(data, schema) {
        this.reader.reset(data);
        return this.readValue(schema);
    }
    /**
     * Reads a value according to its schema.
     */
    readValue(schema) {
        switch (schema.type) {
            // Primitive types
            case 'void':
                return this.decoder.readVoid();
            case 'int':
                return this.decoder.readInt();
            case 'unsigned_int':
                return this.decoder.readUnsignedInt();
            case 'boolean':
                return this.decoder.readBoolean();
            case 'hyper':
                return this.decoder.readHyper();
            case 'unsigned_hyper':
                return this.decoder.readUnsignedHyper();
            case 'float':
                return this.decoder.readFloat();
            case 'double':
                return this.decoder.readDouble();
            case 'quadruple':
                return this.decoder.readQuadruple();
            case 'enum':
                return this.readEnum(schema);
            // Wide primitive types
            case 'opaque':
                return this.readOpaque(schema);
            case 'vopaque':
                return this.readVarlenOpaque(schema);
            case 'string':
                return this.readString(schema);
            // Composite types
            case 'array':
                return this.readArray(schema);
            case 'varray':
                return this.readVarlenArray(schema);
            case 'struct':
                return this.readStruct(schema);
            case 'union':
                return this.readUnion(schema);
            case 'optional':
                return this.readOptional(schema);
            case 'const':
                // Constants are not decoded; they have no runtime representation
                return undefined;
            default:
                throw new Error(`Unknown schema type: ${schema.type}`);
        }
    }
    /**
     * Reads an enum value according to the enum schema.
     */
    readEnum(schema) {
        const value = this.decoder.readEnum();
        // Find the enum name for this value
        for (const [name, enumValue] of Object.entries(schema.values)) {
            if (enumValue === value) {
                return name;
            }
        }
        // If no matching name found, return the numeric value
        return value;
    }
    /**
     * Reads opaque data according to the opaque schema.
     */
    readOpaque(schema) {
        return this.decoder.readOpaque(schema.size);
    }
    /**
     * Reads variable-length opaque data according to the schema.
     */
    readVarlenOpaque(schema) {
        const data = this.decoder.readVarlenOpaque();
        // Check size constraint if specified
        if (schema.size !== undefined && data.length > schema.size) {
            throw new Error(`Variable-length opaque data size ${data.length} exceeds maximum ${schema.size}`);
        }
        return data;
    }
    /**
     * Reads a string according to the string schema.
     */
    readString(schema) {
        const str = this.decoder.readString();
        // Check size constraint if specified
        if (schema.size !== undefined && str.length > schema.size) {
            throw new Error(`String length ${str.length} exceeds maximum ${schema.size}`);
        }
        return str;
    }
    /**
     * Reads a fixed-size array according to the array schema.
     */
    readArray(schema) {
        return this.decoder.readArray(schema.size, () => this.readValue(schema.elements));
    }
    /**
     * Reads a variable-length array according to the schema.
     */
    readVarlenArray(schema) {
        const array = this.decoder.readVarlenArray(() => this.readValue(schema.elements));
        // Check size constraint if specified
        if (schema.size !== undefined && array.length > schema.size) {
            throw new Error(`Variable-length array size ${array.length} exceeds maximum ${schema.size}`);
        }
        return array;
    }
    /**
     * Reads a struct according to the struct schema.
     */
    readStruct(schema) {
        const struct = {};
        for (const [fieldSchema, fieldName] of schema.fields) {
            struct[fieldName] = this.readValue(fieldSchema);
        }
        return struct;
    }
    /**
     * Reads a union according to the union schema.
     */
    readUnion(schema) {
        // Read discriminant
        const discriminant = this.decoder.readInt();
        // Find matching arm
        for (const [armDiscriminant, armSchema] of schema.arms) {
            if (armDiscriminant === discriminant) {
                const value = this.readValue(armSchema);
                return new XdrUnion_1.XdrUnion(discriminant, value);
            }
        }
        // If no matching arm found, try default
        if (schema.default) {
            const value = this.readValue(schema.default);
            return new XdrUnion_1.XdrUnion(discriminant, value);
        }
        throw new Error(`No matching union arm for discriminant: ${discriminant}`);
    }
    /**
     * Reads optional-data according to the optional schema (RFC 1832 Section 3.19).
     * Optional-data is syntactic sugar for a union with boolean discriminant.
     * Returns null if opted is FALSE, otherwise returns the decoded value.
     */
    readOptional(schema) {
        const opted = this.decoder.readBoolean();
        if (!opted)
            return null;
        return this.readValue(schema.element);
    }
}
exports.XdrSchemaDecoder = XdrSchemaDecoder;
//# sourceMappingURL=XdrSchemaDecoder.js.map