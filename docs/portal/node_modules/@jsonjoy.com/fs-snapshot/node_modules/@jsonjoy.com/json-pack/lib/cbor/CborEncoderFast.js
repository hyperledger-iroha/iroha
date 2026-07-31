"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CborEncoderFast = void 0;
const Writer_1 = require("@jsonjoy.com/buffers/lib/Writer");
const isSafeInteger = Number.isSafeInteger;
/**
 * Fast CBOR encoder supports only JSON values. Use regular `CborEncoder` if
 * you need ability to encode all CBOR value types.
 */
class CborEncoderFast {
    constructor(writer = new Writer_1.Writer()) {
        this.writer = writer;
    }
    encode(value) {
        this.writeAny(value);
        return this.writer.flush();
    }
    writeAny(value) {
        switch (typeof value) {
            case 'number':
                return this.writeNumber(value);
            case 'string':
                return this.writeStr(value);
            case 'boolean':
                return this.writer.u8(0xf4 + +value);
            case 'object': {
                if (!value)
                    return this.writer.u8(0xf6);
                const constr = value.constructor;
                switch (constr) {
                    case Array:
                        return this.writeArr(value);
                    default:
                        return this.writeObj(value);
                }
            }
        }
    }
    writeCbor() {
        this.writer.u8u16(0xd9, 0xd9f7);
    }
    writeEnd() {
        this.writer.u8(255 /* CONST.END */);
    }
    writeNull() {
        this.writer.u8(0xf6);
    }
    writeBoolean(bool) {
        if (bool)
            this.writer.u8(0xf5);
        else
            this.writer.u8(0xf4);
    }
    writeNumber(num) {
        if (isSafeInteger(num))
            this.writeInteger(num);
        else if (typeof num === 'bigint')
            this.writeBigInt(num);
        else
            this.writeFloat(num);
    }
    writeBigInt(int) {
        if (int >= 0)
            this.writeBigUint(int);
        else
            this.writeBigSint(int);
    }
    writeBigUint(uint) {
        if (uint <= Number.MAX_SAFE_INTEGER)
            return this.writeUInteger(Number(uint));
        this.writer.u8u64(0x1b, uint);
    }
    writeBigSint(int) {
        if (int >= Number.MIN_SAFE_INTEGER)
            return this.encodeNint(Number(int));
        const uint = -BigInt(1) - int;
        this.writer.u8u64(0x3b, uint);
    }
    writeInteger(int) {
        if (int >= 0)
            this.writeUInteger(int);
        else
            this.encodeNint(int);
    }
    writeUInteger(uint) {
        const writer = this.writer;
        writer.ensureCapacity(9);
        const uint8 = writer.uint8;
        let x = writer.x;
        if (uint <= 23) {
            uint8[x++] = 0 /* MAJOR_OVERLAY.UIN */ + uint;
        }
        else if (uint <= 0xff) {
            uint8[x++] = 0x18;
            uint8[x++] = uint;
        }
        else if (uint <= 0xffff) {
            uint8[x++] = 0x19;
            writer.view.setUint16(x, uint);
            x += 2;
        }
        else if (uint <= 0xffffffff) {
            uint8[x++] = 0x1a;
            writer.view.setUint32(x, uint);
            x += 4;
        }
        else {
            uint8[x++] = 0x1b;
            writer.view.setBigUint64(x, BigInt(uint));
            x += 8;
        }
        writer.x = x;
    }
    /** @deprecated Remove and use `writeNumber` instead. */
    encodeNumber(num) {
        this.writeNumber(num);
    }
    /** @deprecated Remove and use `writeInteger` instead. */
    encodeInteger(int) {
        this.writeInteger(int);
    }
    /** @deprecated */
    encodeUint(uint) {
        this.writeUInteger(uint);
    }
    encodeNint(int) {
        const uint = -1 - int;
        const writer = this.writer;
        writer.ensureCapacity(9);
        const uint8 = writer.uint8;
        let x = writer.x;
        if (uint < 24) {
            uint8[x++] = 32 /* MAJOR_OVERLAY.NIN */ + uint;
        }
        else if (uint <= 0xff) {
            uint8[x++] = 0x38;
            uint8[x++] = uint;
        }
        else if (uint <= 0xffff) {
            uint8[x++] = 0x39;
            writer.view.setUint16(x, uint);
            x += 2;
        }
        else if (uint <= 0xffffffff) {
            uint8[x++] = 0x3a;
            writer.view.setUint32(x, uint);
            x += 4;
        }
        else {
            uint8[x++] = 0x3b;
            writer.view.setBigUint64(x, BigInt(uint));
            x += 8;
        }
        writer.x = x;
    }
    writeFloat(float) {
        this.writer.u8f64(0xfb, float);
    }
    writeBin(buf) {
        const length = buf.length;
        this.writeBinHdr(length);
        this.writer.buf(buf, length);
    }
    writeBinHdr(length) {
        const writer = this.writer;
        if (length <= 23)
            writer.u8(64 /* MAJOR_OVERLAY.BIN */ + length);
        else if (length <= 0xff)
            writer.u16((0x58 << 8) + length);
        else if (length <= 0xffff)
            writer.u8u16(0x59, length);
        else if (length <= 0xffffffff)
            writer.u8u32(0x5a, length);
        else
            writer.u8u64(0x5b, length);
    }
    writeStr(str) {
        const writer = this.writer;
        const length = str.length;
        const maxSize = length * 4;
        writer.ensureCapacity(5 + maxSize);
        const uint8 = writer.uint8;
        let lengthOffset = writer.x;
        if (maxSize <= 23)
            writer.x++;
        else if (maxSize <= 0xff) {
            uint8[writer.x++] = 0x78;
            lengthOffset = writer.x;
            writer.x++;
        }
        else if (maxSize <= 0xffff) {
            uint8[writer.x++] = 0x79;
            lengthOffset = writer.x;
            writer.x += 2;
        }
        else {
            uint8[writer.x++] = 0x7a;
            lengthOffset = writer.x;
            writer.x += 4;
        }
        const bytesWritten = writer.utf8(str);
        if (maxSize <= 23)
            uint8[lengthOffset] = 96 /* MAJOR_OVERLAY.STR */ + bytesWritten;
        else if (maxSize <= 0xff)
            uint8[lengthOffset] = bytesWritten;
        else if (maxSize <= 0xffff)
            writer.view.setUint16(lengthOffset, bytesWritten);
        else
            writer.view.setUint32(lengthOffset, bytesWritten);
    }
    writeStrHdr(length) {
        const writer = this.writer;
        if (length <= 23)
            writer.u8(96 /* MAJOR_OVERLAY.STR */ + length);
        else if (length <= 0xff)
            writer.u16((0x78 << 8) + length);
        else if (length <= 0xffff)
            writer.u8u16(0x79, length);
        else
            writer.u8u32(0x7a, length);
    }
    writeAsciiStr(str) {
        this.writeStrHdr(str.length);
        this.writer.ascii(str);
    }
    writeArr(arr) {
        const length = arr.length;
        this.writeArrHdr(length);
        for (let i = 0; i < length; i++)
            this.writeAny(arr[i]);
    }
    writeArrHdr(length) {
        const writer = this.writer;
        if (length <= 23)
            writer.u8(128 /* MAJOR_OVERLAY.ARR */ + length);
        else if (length <= 0xff)
            writer.u16((0x98 << 8) + length);
        else if (length <= 0xffff)
            writer.u8u16(0x99, length);
        else if (length <= 0xffffffff)
            writer.u8u32(0x9a, length);
        else
            writer.u8u64(0x9b, length);
    }
    writeObj(obj) {
        const keys = Object.keys(obj);
        const length = keys.length;
        this.writeObjHdr(length);
        for (let i = 0; i < length; i++) {
            const key = keys[i];
            this.writeStr(key);
            this.writeAny(obj[key]);
        }
    }
    writeObjHdr(length) {
        const writer = this.writer;
        if (length <= 23)
            writer.u8(160 /* MAJOR_OVERLAY.MAP */ + length);
        else if (length <= 0xff)
            writer.u16((0xb8 << 8) + length);
        else if (length <= 0xffff)
            writer.u8u16(0xb9, length);
        else if (length <= 0xffffffff)
            writer.u8u32(0xba, length);
        else
            writer.u8u64(0xbb, length);
    }
    writeMapHdr(length) {
        this.writeObjHdr(length);
    }
    writeStartMap() {
        this.writer.u8(0xbf);
    }
    writeTag(tag, value) {
        this.writeTagHdr(tag);
        this.writeAny(value);
    }
    writeTagHdr(tag) {
        const writer = this.writer;
        if (tag <= 23)
            writer.u8(192 /* MAJOR_OVERLAY.TAG */ + tag);
        else if (tag <= 0xff)
            writer.u16((0xd8 << 8) + tag);
        else if (tag <= 0xffff)
            writer.u8u16(0xd9, tag);
        else if (tag <= 0xffffffff)
            writer.u8u32(0xda, tag);
        else
            writer.u8u64(0xdb, tag);
    }
    writeTkn(value) {
        const writer = this.writer;
        if (value <= 23)
            writer.u8(224 /* MAJOR_OVERLAY.TKN */ + value);
        else if (value <= 0xff)
            writer.u16((0xf8 << 8) + value);
    }
    // ------------------------------------------------------- Streaming encoding
    writeStartStr() {
        this.writer.u8(0x7f);
    }
    writeStrChunk(str) {
        throw new Error('Not implemented');
    }
    writeEndStr() {
        throw new Error('Not implemented');
    }
    writeStartBin() {
        this.writer.u8(0x5f);
    }
    writeBinChunk(buf) {
        throw new Error('Not implemented');
    }
    writeEndBin() {
        throw new Error('Not implemented');
    }
    writeStartArr() {
        this.writer.u8(0x9f);
    }
    writeArrChunk(item) {
        throw new Error('Not implemented');
    }
    writeEndArr() {
        this.writer.u8(255 /* CONST.END */);
    }
    writeStartObj() {
        this.writer.u8(0xbf);
    }
    writeObjChunk(key, value) {
        throw new Error('Not implemented');
    }
    writeEndObj() {
        this.writer.u8(255 /* CONST.END */);
    }
}
exports.CborEncoderFast = CborEncoderFast;
//# sourceMappingURL=CborEncoderFast.js.map