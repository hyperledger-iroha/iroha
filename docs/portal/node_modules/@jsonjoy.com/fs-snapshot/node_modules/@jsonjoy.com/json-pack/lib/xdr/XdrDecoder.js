"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.XdrDecoder = void 0;
const Reader_1 = require("@jsonjoy.com/buffers/lib/Reader");
/**
 * XDR (External Data Representation) binary decoder for basic value decoding.
 * Implements XDR binary decoding according to RFC 4506.
 *
 * Key XDR decoding principles:
 * - All data types are aligned to 4-byte boundaries
 * - Multi-byte quantities are transmitted in big-endian byte order
 * - Strings and opaque data are padded to 4-byte boundaries
 * - Variable-length arrays and strings are preceded by their length
 */
class XdrDecoder {
    constructor(reader = new Reader_1.Reader()) {
        this.reader = reader;
    }
    read(uint8) {
        this.reader.reset(uint8);
        return this.readAny();
    }
    decode(uint8) {
        this.reader.reset(uint8);
        return this.readAny();
    }
    readAny() {
        // Basic implementation - in practice this would need schema info
        // For now, we'll throw as this should be used with schema decoder
        throw new Error('not implemented');
    }
    /**
     * Reads an XDR void value (no data is actually read).
     */
    readVoid() {
        // Void values have no representation in XDR
    }
    /**
     * Reads an XDR boolean value as a 4-byte integer.
     * Returns true for non-zero values, false for zero.
     */
    readBoolean() {
        return this.readInt() !== 0;
    }
    /**
     * Reads an XDR signed 32-bit integer in big-endian format.
     */
    readInt() {
        const reader = this.reader;
        const value = reader.view.getInt32(reader.x, false); // false = big-endian
        reader.x += 4;
        return value;
    }
    /**
     * Reads an XDR unsigned 32-bit integer in big-endian format.
     */
    readUnsignedInt() {
        const reader = this.reader;
        const value = reader.view.getUint32(reader.x, false); // false = big-endian
        reader.x += 4;
        return value;
    }
    /**
     * Reads an XDR signed 64-bit integer (hyper) in big-endian format.
     */
    readHyper() {
        const reader = this.reader;
        const value = reader.view.getBigInt64(reader.x, false); // false = big-endian
        reader.x += 8;
        return value;
    }
    /**
     * Reads an XDR unsigned 64-bit integer (unsigned hyper) in big-endian format.
     */
    readUnsignedHyper() {
        const reader = this.reader;
        const value = reader.view.getBigUint64(reader.x, false); // false = big-endian
        reader.x += 8;
        return value;
    }
    /**
     * Reads an XDR float value using IEEE 754 single-precision in big-endian format.
     */
    readFloat() {
        const reader = this.reader;
        const value = reader.view.getFloat32(reader.x, false); // false = big-endian
        reader.x += 4;
        return value;
    }
    /**
     * Reads an XDR double value using IEEE 754 double-precision in big-endian format.
     */
    readDouble() {
        const reader = this.reader;
        const value = reader.view.getFloat64(reader.x, false); // false = big-endian
        reader.x += 8;
        return value;
    }
    /**
     * Reads an XDR quadruple value (128-bit float).
     * Note: JavaScript doesn't have native 128-bit float support.
     */
    readQuadruple() {
        throw new Error('not implemented');
    }
    /**
     * Reads XDR opaque data with known fixed length.
     * Data is padded to 4-byte boundary but only the actual data is returned.
     */
    readOpaque(size) {
        const reader = this.reader;
        const data = reader.buf(size);
        // Skip padding bytes to reach 4-byte boundary
        const paddedSize = size % 4 === 0 ? size : size + (4 - (size % 4));
        reader.skip(paddedSize - size);
        return data;
    }
    /**
     * Reads XDR variable-length opaque data.
     * Length is read first, followed by data padded to 4-byte boundary.
     */
    readVarlenOpaque() {
        const size = this.readUnsignedInt();
        return this.readOpaque(size);
    }
    /**
     * Reads an XDR string with UTF-8 encoding.
     * Length is read first, followed by UTF-8 bytes padded to 4-byte boundary.
     */
    readString() {
        const size = this.readUnsignedInt();
        const reader = this.reader;
        const text = reader.utf8(size);
        // Skip padding bytes to reach 4-byte boundary
        const paddedSize = size % 4 === 0 ? size : size + (4 - (size % 4));
        reader.skip(paddedSize - size);
        return text;
    }
    /**
     * Reads an XDR enum value as an unsigned integer.
     */
    readEnum() {
        return this.readInt();
    }
    /**
     * Reads a fixed-size array of elements.
     * Caller must provide the decode function for each element.
     */
    readArray(size, elementReader) {
        const array = [];
        for (let i = 0; i < size; i++)
            array.push(elementReader());
        return array;
    }
    /**
     * Reads a variable-length array of elements.
     * Length is read first, followed by elements.
     */
    readVarlenArray(elementReader) {
        const size = this.readUnsignedInt();
        return this.readArray(size, elementReader);
    }
}
exports.XdrDecoder = XdrDecoder;
//# sourceMappingURL=XdrDecoder.js.map