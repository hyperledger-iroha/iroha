"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.MsgPackEncoderFast = void 0;
const Writer_1 = require("@jsonjoy.com/buffers/lib/Writer");
/**
 * @category Encoder
 */
class MsgPackEncoderFast {
    constructor(writer = new Writer_1.Writer()) {
        this.writer = writer;
    }
    /**
     * Use this method to encode a JavaScript document into MessagePack format.
     *
     * @param json JSON value to encode.
     * @returns Encoded memory buffer with MessagePack contents.
     */
    encode(json) {
        this.writer.reset();
        this.writeAny(json);
        return this.writer.flush();
    }
    /** @deprecated */
    encodeAny(json) {
        this.writeAny(json);
    }
    writeAny(value) {
        switch (value) {
            case null:
                return this.writer.u8(0xc0);
            case false:
                return this.writer.u8(0xc2);
            case true:
                return this.writer.u8(0xc3);
        }
        if (value instanceof Array)
            return this.writeArr(value);
        switch (typeof value) {
            case 'number':
                return this.writeNumber(value);
            case 'string':
                return this.writeStr(value);
            case 'object':
                return this.writeObj(value);
        }
    }
    /** @deprecated */
    encodeFloat64(num) {
        this.writeFloat(num);
    }
    writeNull() {
        return this.writer.u8(0xc0);
    }
    writeFloat(float) {
        this.writer.u8f64(0xcb, float);
    }
    u32(num) {
        const writer = this.writer;
        this.writer.ensureCapacity(5);
        const uint8 = writer.uint8;
        if (num <= 0b1111111) {
            uint8[writer.x++] = num;
            // Commenting this out improves performance, there is not much space savings.
            // } else if (num <= 0xff) {
            //   uint8[writer.x++] = 0xcc;
            //   uint8[writer.x++] = num;
        }
        else if (num <= 0xffff) {
            uint8[writer.x++] = 0xcd;
            writer.view.setUint16(writer.x, num);
            writer.x += 2;
        }
        else if (num <= 0xffffffff) {
            uint8[writer.x++] = 0xce;
            writer.view.setUint32(writer.x, num);
            writer.x += 4;
        }
        else
            this.writeFloat(num);
    }
    n32(num) {
        const writer = this.writer;
        this.writer.ensureCapacity(5);
        const uint8 = writer.uint8;
        if (num >= -0x20) {
            uint8[writer.x++] = 0x100 + num;
            // Commenting this out improves performance, there is not much space savings.
            // } else if (num >= -0x80) {
            //   uint8[writer.x++] = 0xd0;
            //   uint8[writer.x++] = num + 0x100;
        }
        else if (num >= -0x8000) {
            uint8[writer.x++] = 0xd1;
            writer.view.setInt16(writer.x, num);
            writer.x += 2;
        }
        else if (num >= -0x80000000) {
            uint8[writer.x++] = 0xd2;
            writer.view.setInt32(writer.x, num);
            writer.x += 4;
        }
        else
            this.writeFloat(num);
    }
    /** @deprecated */
    encodeNumber(num) {
        this.writeNumber(num);
    }
    writeNumber(num) {
        if (num >>> 0 === num)
            return this.u32(num);
        if (num >> 0 === num)
            return this.n32(num);
        this.writeFloat(num);
    }
    writeInteger(int) {
        if (int >= 0)
            if (int <= 0xffffffff)
                return this.u32(int);
            else if (int > -0x80000000)
                return this.n32(int);
        this.writeFloat(int);
    }
    writeUInteger(uint) {
        if (uint <= 0xffffffff)
            return this.u32(uint);
        this.writeFloat(uint);
    }
    encodeNull() {
        this.writer.u8(0xc0);
    }
    encodeTrue() {
        this.writer.u8(0xc3);
    }
    encodeFalse() {
        this.writer.u8(0xc2);
    }
    /** @deprecated */
    encodeBoolean(bool) {
        this.writeBoolean(bool);
    }
    writeBoolean(bool) {
        if (bool)
            this.writer.u8(0xc3);
        else
            this.writer.u8(0xc2);
    }
    /** @deprecated */
    encodeStringHeader(length) {
        this.writeStrHdr(length);
    }
    writeStrHdr(length) {
        if (length <= 0b11111)
            this.writer.u8(0b10100000 | length);
        else if (length <= 0xff)
            this.writer.u16(0xd900 + length);
        else if (length <= 0xffff)
            this.writer.u8u16(0xda, length);
        else
            this.writer.u8u32(0xdb, length);
    }
    /** @deprecated */
    encodeString(str) {
        this.writeStr(str);
    }
    writeStr(str) {
        const writer = this.writer;
        const length = str.length;
        const maxSize = length * 4;
        writer.ensureCapacity(5 + maxSize);
        const uint8 = writer.uint8;
        let lengthOffset = writer.x;
        if (maxSize <= 0b11111)
            writer.x++;
        else if (maxSize <= 0xff) {
            uint8[writer.x++] = 0xd9;
            lengthOffset = writer.x;
            writer.x++;
        }
        else if (maxSize <= 0xffff) {
            uint8[writer.x++] = 0xda;
            lengthOffset = writer.x;
            writer.x += 2;
        }
        else {
            uint8[writer.x++] = 0xdb;
            lengthOffset = writer.x;
            writer.x += 4;
        }
        const bytesWritten = this.writer.utf8(str);
        if (maxSize <= 0b11111)
            uint8[lengthOffset] = 0b10100000 | bytesWritten;
        else if (maxSize <= 0xff)
            uint8[lengthOffset] = bytesWritten;
        else if (maxSize <= 0xffff)
            writer.view.setUint16(lengthOffset, bytesWritten);
        else
            writer.view.setUint32(lengthOffset, bytesWritten);
    }
    /** @deprecated */
    encodeAsciiString(str) {
        this.writeAsciiStr(str);
    }
    writeAsciiStr(str) {
        this.writeStrHdr(str.length);
        this.writer.ascii(str);
    }
    /** @deprecated */
    encodeArrayHeader(length) {
        this.writeArrHdr(length);
    }
    /** @deprecated */
    encodeArray(arr) {
        this.writeArr(arr);
    }
    writeArrHdr(length) {
        if (length <= 0b1111)
            this.writer.u8(0b10010000 | length);
        else if (length <= 0xffff)
            this.writer.u8u16(0xdc, length);
        else if (length <= 0xffffffff)
            this.writer.u8u32(0xdd, length);
    }
    writeArr(arr) {
        const length = arr.length;
        if (length <= 0b1111)
            this.writer.u8(0b10010000 | length);
        else if (length <= 0xffff)
            this.writer.u8u16(0xdc, length);
        else if (length <= 0xffffffff)
            this.writer.u8u32(0xdd, length);
        // else return;
        for (let i = 0; i < length; i++)
            this.writeAny(arr[i]);
    }
    /** @deprecated */
    encodeObjectHeader(length) {
        this.writeObjHdr(length);
    }
    /** @deprecated */
    encodeObject(obj) {
        this.writeObj(obj);
    }
    writeObjHdr(length) {
        if (length <= 0b1111)
            this.writer.u8(0b10000000 | length);
        else if (length <= 0xffff) {
            this.writer.u8u16(0xde, length);
        }
        else if (length <= 0xffffffff) {
            this.writer.u8u32(0xdf, length);
        }
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
    encodeExtHeader(type, length) {
        switch (length) {
            case 1:
                this.writer.u16((0xd4 << 8) | type);
                break;
            case 2:
                this.writer.u16((0xd5 << 8) | type);
                break;
            case 4:
                this.writer.u16((0xd6 << 8) | type);
                break;
            case 8:
                this.writer.u16((0xd7 << 8) | type);
                break;
            case 16:
                this.writer.u16((0xd8 << 8) | type);
                break;
            default:
                if (length <= 0xff) {
                    this.writer.u16((0xc7 << 8) | length);
                    this.writer.u8(type);
                }
                else if (length <= 0xffff) {
                    this.writer.u8u16(0xc8, length);
                    this.writer.u8(type);
                }
                else if (length <= 0xffffffff) {
                    this.writer.u8u32(0xc9, length);
                    this.writer.u8(type);
                }
        }
    }
    encodeExt(ext) {
        const { tag: type, val: buf } = ext;
        const length = buf.length;
        this.encodeExtHeader(type, length);
        this.writer.buf(buf, length);
    }
    /** @deprecated */
    encodeBinaryHeader(length) {
        this.writeBinHdr(length);
    }
    /** @deprecated */
    encodeBinary(buf) {
        this.writeBin(buf);
    }
    writeBinHdr(length) {
        if (length <= 0xff)
            this.writer.u16((0xc4 << 8) | length);
        else if (length <= 0xffff) {
            this.writer.u8u16(0xc5, length);
        }
        else if (length <= 0xffffffff) {
            this.writer.u8u32(0xc6, length);
        }
    }
    writeBin(buf) {
        const length = buf.length;
        this.writeBinHdr(length);
        this.writer.buf(buf, length);
    }
}
exports.MsgPackEncoderFast = MsgPackEncoderFast;
//# sourceMappingURL=MsgPackEncoderFast.js.map