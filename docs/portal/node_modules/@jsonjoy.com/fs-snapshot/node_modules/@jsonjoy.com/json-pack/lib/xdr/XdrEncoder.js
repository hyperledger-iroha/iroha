"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.XdrEncoder = void 0;
/**
 * XDR (External Data Representation) binary encoder for basic value encoding.
 * Implements XDR binary encoding according to RFC 4506.
 *
 * Key XDR encoding principles:
 * - All data types are aligned to 4-byte boundaries
 * - Multi-byte quantities are transmitted in big-endian byte order
 * - Strings and opaque data are padded to 4-byte boundaries
 * - Variable-length arrays and strings are preceded by their length
 */
class XdrEncoder {
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
        this.writeVoid();
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
                    return this.writeVoid();
                const construct = value.constructor;
                switch (construct) {
                    case Uint8Array:
                        return this.writeBin(value);
                    default:
                        return this.writeUnknown(value);
                }
            }
            case 'bigint':
                return this.writeHyper(value);
            case 'undefined':
                return this.writeVoid();
            default:
                return this.writeUnknown(value);
        }
    }
    /**
     * Writes an XDR void value (no data is actually written).
     */
    writeVoid() {
        // Void values are encoded as no data
    }
    /**
     * Writes an XDR null value (for interface compatibility).
     */
    writeNull() {
        this.writeVoid();
    }
    /**
     * Writes an XDR boolean value as a 4-byte integer.
     */
    writeBoolean(bool) {
        this.writeInt(bool ? 1 : 0);
    }
    /**
     * Writes an XDR signed 32-bit integer in big-endian format.
     */
    writeInt(int) {
        const writer = this.writer;
        writer.ensureCapacity(4);
        writer.view.setInt32(writer.x, Math.trunc(int), false); // big-endian
        writer.move(4);
    }
    /**
     * Writes an XDR unsigned 32-bit integer in big-endian format.
     */
    writeUnsignedInt(uint) {
        const writer = this.writer;
        writer.ensureCapacity(4);
        writer.view.setUint32(writer.x, Math.trunc(uint) >>> 0, false); // big-endian
        writer.move(4);
    }
    /**
     * Writes an XDR signed 64-bit integer (hyper) in big-endian format.
     */
    writeHyper(hyper) {
        const writer = this.writer;
        writer.ensureCapacity(8);
        if (typeof hyper === 'bigint') {
            writer.view.setBigInt64(writer.x, hyper, false); // big-endian
        }
        else {
            const truncated = Math.trunc(hyper);
            const high = Math.floor(truncated / 0x100000000);
            const low = truncated >>> 0;
            writer.view.setInt32(writer.x, high, false); // high 32 bits
            writer.view.setUint32(writer.x + 4, low, false); // low 32 bits
        }
        writer.move(8);
    }
    /**
     * Writes an XDR unsigned 64-bit integer (unsigned hyper) in big-endian format.
     */
    writeUnsignedHyper(uhyper) {
        const writer = this.writer;
        writer.ensureCapacity(8);
        if (typeof uhyper === 'bigint') {
            writer.view.setBigUint64(writer.x, uhyper, false); // big-endian
        }
        else {
            const truncated = Math.trunc(Math.abs(uhyper));
            const high = Math.floor(truncated / 0x100000000);
            const low = truncated >>> 0;
            writer.view.setUint32(writer.x, high, false); // high 32 bits
            writer.view.setUint32(writer.x + 4, low, false); // low 32 bits
        }
        writer.move(8);
    }
    /**
     * Writes an XDR float value using IEEE 754 single-precision in big-endian format.
     */
    writeFloat(float) {
        const writer = this.writer;
        writer.ensureCapacity(4);
        writer.view.setFloat32(writer.x, float, false); // big-endian
        writer.move(4);
    }
    /**
     * Writes an XDR double value using IEEE 754 double-precision in big-endian format.
     */
    writeDouble(double) {
        const writer = this.writer;
        writer.ensureCapacity(8);
        writer.view.setFloat64(writer.x, double, false); // big-endian
        writer.move(8);
    }
    /**
     * Writes an XDR quadruple value (128-bit float).
     * Note: JavaScript doesn't have native 128-bit float support.
     */
    writeQuadruple(quad) {
        throw new Error('not implemented');
    }
    /**
     * Writes XDR opaque data with fixed length.
     * Data is padded to 4-byte boundary.
     */
    writeOpaque(data) {
        const size = data.length;
        const writer = this.writer;
        const paddedSize = Math.ceil(size / 4) * 4;
        writer.ensureCapacity(paddedSize);
        writer.buf(data, size);
        const padding = paddedSize - size;
        for (let i = 0; i < padding; i++)
            writer.u8(0);
    }
    /**
     * Writes XDR variable-length opaque data.
     * Length is written first, followed by data padded to 4-byte boundary.
     */
    writeVarlenOpaque(data) {
        this.writeUnsignedInt(data.length);
        this.writeOpaque(data);
    }
    /**
     * Writes an XDR string with UTF-8 encoding.
     * Length is written first, followed by UTF-8 bytes padded to 4-byte boundary.
     */
    writeStr(str) {
        const writer = this.writer;
        // Write string using writer's UTF-8 method and get actual byte count
        const lengthOffset = writer.x;
        writer.x += 4; // Reserve space for length
        const bytesWritten = writer.utf8(str);
        const paddedSize = Math.ceil(bytesWritten / 4) * 4;
        const padding = paddedSize - bytesWritten;
        for (let i = 0; i < padding; i++)
            writer.u8(0);
        // Go back and write the actual byte length
        const currentPos = writer.x;
        writer.x = lengthOffset;
        this.writeUnsignedInt(bytesWritten);
        writer.x = currentPos;
    }
    writeArr(arr) {
        throw new Error('not implemented');
    }
    writeObj(obj) {
        throw new Error('not implemented');
    }
    // BinaryJsonEncoder interface methods
    /**
     * Generic number writing - determines type based on value
     */
    writeNumber(num) {
        if (Number.isInteger(num)) {
            if (num >= -2147483648 && num <= 2147483647)
                this.writeInt(num);
            else
                this.writeHyper(num);
        }
        else
            this.writeDouble(num);
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
        this.writeUnsignedInt(uint);
    }
    /**
     * Writes binary data
     */
    writeBin(buf) {
        this.writeVarlenOpaque(buf);
    }
    /**
     * Writes an ASCII string (same as regular string in XDR)
     */
    writeAsciiStr(str) {
        this.writeStr(str);
    }
}
exports.XdrEncoder = XdrEncoder;
//# sourceMappingURL=XdrEncoder.js.map