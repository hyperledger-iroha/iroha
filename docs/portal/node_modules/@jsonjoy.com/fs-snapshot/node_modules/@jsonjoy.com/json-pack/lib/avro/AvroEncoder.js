"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.AvroEncoder = void 0;
/**
 * Apache Avro binary encoder for basic value encoding.
 * Implements the Avro binary encoding specification without schema validation.
 * Based on https://avro.apache.org/docs/1.12.0/specification/
 */
class AvroEncoder {
    constructor(writer) {
        this.writer = writer;
    }
    encode(value) {
        const writer = this.writer;
        writer.reset();
        this.writeAny(value);
        return writer.flush();
    }
    /**
     * Called when the encoder encounters a value that it does not know how to encode.
     */
    writeUnknown(value) {
        this.writeNull();
    }
    writeAny(value) {
        switch (typeof value) {
            case 'boolean':
                return this.writeBoolean(value);
            case 'number':
                return this.writeNumber(value);
            case 'string':
                return this.writeStr(value);
            case 'object': {
                if (value === null)
                    return this.writeNull();
                const construct = value.constructor;
                switch (construct) {
                    case Object:
                        return this.writeObj(value);
                    case Array:
                        return this.writeArr(value);
                    case Uint8Array:
                        return this.writeBin(value);
                    default:
                        return this.writeUnknown(value);
                }
            }
            case 'bigint':
                return this.writeLong(value);
            case 'undefined':
                return this.writeNull();
            default:
                return this.writeUnknown(value);
        }
    }
    /**
     * Writes an Avro null value.
     */
    writeNull() {
        // Null values are encoded as zero bytes
    }
    /**
     * Writes an Avro boolean value.
     */
    writeBoolean(bool) {
        this.writer.u8(bool ? 1 : 0);
    }
    /**
     * Writes an Avro int value using zigzag encoding.
     */
    writeInt(int) {
        this.writeVarIntSigned(this.encodeZigZag32(Math.trunc(int)));
    }
    /**
     * Writes an Avro long value using zigzag encoding.
     */
    writeLong(long) {
        if (typeof long === 'bigint') {
            this.writeVarLong(this.encodeZigZag64(long));
        }
        else {
            this.writeVarLong(this.encodeZigZag64(BigInt(Math.trunc(long))));
        }
    }
    /**
     * Writes an Avro float value using IEEE 754 single-precision.
     */
    writeFloatAvro(float) {
        const writer = this.writer;
        writer.ensureCapacity(4);
        writer.view.setFloat32(writer.x, float, true); // little-endian
        writer.move(4);
    }
    /**
     * Writes an Avro double value using IEEE 754 double-precision.
     */
    writeDouble(double) {
        const writer = this.writer;
        writer.ensureCapacity(8);
        writer.view.setFloat64(writer.x, double, true); // little-endian
        writer.move(8);
    }
    /**
     * Writes an Avro bytes value with length-prefixed encoding.
     */
    writeBin(bytes) {
        this.writeVarIntUnsigned(bytes.length);
        this.writer.buf(bytes, bytes.length);
    }
    /**
     * Writes an Avro string value with UTF-8 encoding and length prefix.
     */
    writeStr(str) {
        const writer = this.writer;
        const maxSize = str.length * 4; // Max UTF-8 bytes for string
        writer.ensureCapacity(5 + maxSize); // 5 bytes max for varint length
        // Reserve space for length (we'll come back to fill this)
        const lengthOffset = writer.x;
        writer.x += 5; // Max varint size
        // Write the string and get actual byte count
        const bytesWritten = writer.utf8(str);
        const endPos = writer.x;
        // Go back to encode the actual length
        writer.x = lengthOffset;
        this.writeVarIntUnsigned(bytesWritten);
        const actualLengthSize = writer.x - lengthOffset;
        // If we reserved more space than needed, shift the string data
        if (actualLengthSize < 5) {
            const stringStart = lengthOffset + 5;
            const stringData = writer.uint8.slice(stringStart, endPos);
            writer.x = lengthOffset + actualLengthSize;
            writer.buf(stringData, stringData.length);
        }
        else {
            writer.x = endPos;
        }
    }
    /**
     * Writes an Avro array with length-prefixed encoding.
     */
    writeArr(arr) {
        this.writeVarIntUnsigned(arr.length);
        const length = arr.length;
        for (let i = 0; i < length; i++) {
            this.writeAny(arr[i]);
        }
        this.writeVarIntUnsigned(0); // End of array marker
    }
    /**
     * Writes an Avro map with length-prefixed encoding.
     */
    writeObj(obj) {
        const entries = Object.entries(obj);
        const length = entries.length;
        this.writeVarIntUnsigned(length);
        for (let i = 0; i < length; i++) {
            const entry = entries[i];
            this.writeStr(entry[0]);
            this.writeAny(entry[1]);
        }
        this.writeVarIntUnsigned(0); // End of map marker
    }
    // BinaryJsonEncoder interface methods
    /**
     * Generic number writing - determines type based on value
     */
    writeNumber(num) {
        if (Number.isInteger(num)) {
            if (num >= -2147483648 && num <= 2147483647) {
                this.writeInt(num);
            }
            else {
                this.writeLong(num);
            }
        }
        else {
            this.writeDouble(num);
        }
    }
    /**
     * Writes an integer value
     */
    writeInteger(int) {
        this.writeInt(int);
    }
    /**
     * Writes an unsigned integer value
     */
    writeUInteger(uint) {
        this.writeInt(uint);
    }
    /**
     * Writes a float value (interface method)
     */
    writeFloat(float) {
        this.writeFloatValue(float);
    }
    /**
     * Writes a float value using IEEE 754 single-precision.
     */
    writeFloatValue(float) {
        const writer = this.writer;
        writer.ensureCapacity(4);
        writer.view.setFloat32(writer.x, float, true); // little-endian
        writer.move(4);
    }
    /**
     * Writes an ASCII string (same as regular string in Avro)
     */
    writeAsciiStr(str) {
        const writer = this.writer;
        this.writeVarIntUnsigned(str.length);
        writer.ascii(str);
    }
    // Utility methods for Avro encoding
    /**
     * Encodes a variable-length integer (for signed values with zigzag)
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
     * Encodes a variable-length integer (for unsigned values like lengths)
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
     * Encodes a variable-length long using Avro's encoding
     */
    writeVarLong(value) {
        const writer = this.writer;
        let n = value;
        const mask = BigInt(0x7f);
        const shift = BigInt(7);
        while (n >= BigInt(0x80)) {
            writer.u8(Number((n & mask) | BigInt(0x80)));
            n >>= shift;
        }
        writer.u8(Number(n & mask));
    }
    /**
     * Encodes a 32-bit integer using zigzag encoding
     */
    encodeZigZag32(value) {
        return (value << 1) ^ (value >> 31);
    }
    /**
     * Encodes a 64-bit integer using zigzag encoding
     */
    encodeZigZag64(value) {
        return (value << BigInt(1)) ^ (value >> BigInt(63));
    }
}
exports.AvroEncoder = AvroEncoder;
//# sourceMappingURL=AvroEncoder.js.map