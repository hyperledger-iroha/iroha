"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.MsgPackToJsonConverter = void 0;
const asString_1 = require("@jsonjoy.com/util/lib/strings/asString");
const toDataUri_1 = require("../util/buffers/toDataUri");
/**
 * @category Decoder
 */
class MsgPackToJsonConverter {
    constructor() {
        /** @ignore */
        this.uint8 = new Uint8Array([]);
        /** @ignore */
        this.view = new DataView(this.uint8.buffer);
        /** @ignore */
        this.x = 0;
    }
    reset(uint8) {
        this.x = 0;
        this.uint8 = uint8;
        this.view = new DataView(uint8.buffer, uint8.byteOffset, uint8.length);
    }
    /**
     * Converts a MessagePack blob directly to JSON string.
     *
     * @param uint8 Binary data with MessagePack encoded value.
     * @returns JSON string.
     */
    convert(uint8) {
        this.reset(uint8);
        return this.val();
    }
    /** @ignore */
    val() {
        const byte = this.u8();
        if (byte >= 0xe0)
            return (byte - 0x100).toString(); // 0xe0
        if (byte <= 0xbf) {
            if (byte < 0x90) {
                if (byte <= 0b1111111)
                    return byte.toString(); // 0x7f
                return this.obj(byte & 0b1111); // 0x80
            }
            else {
                if (byte < 0xa0)
                    return this.arr(byte & 0b1111);
                // 0x90
                else
                    return this.str(byte & 0b11111); // 0xa0
            }
        }
        if (byte <= 0xd0) {
            if (byte <= 0xc8) {
                if (byte <= 0xc4) {
                    if (byte <= 0xc2)
                        return byte === 0xc2 ? 'false' : 'null';
                    else
                        return byte === 0xc4 ? this.bin(this.u8()) : 'true';
                }
                else {
                    if (byte <= 0xc6)
                        return byte === 0xc6 ? this.bin(this.u32()) : this.bin(this.u16());
                    else
                        return byte === 0xc8 ? this.ext(this.u16()) : this.ext(this.u8());
                }
            }
            else {
                return byte <= 0xcc
                    ? byte <= 0xca
                        ? byte === 0xca
                            ? this.f32().toString()
                            : this.ext(this.u32())
                        : byte === 0xcc
                            ? this.u8().toString()
                            : this.f64().toString()
                    : byte <= 0xce
                        ? byte === 0xce
                            ? this.u32().toString()
                            : this.u16().toString()
                        : byte === 0xd0
                            ? this.i8().toString()
                            : (this.u32() * 4294967296 + this.u32()).toString();
            }
        }
        else if (byte <= 0xd8) {
            return byte <= 0xd4
                ? byte <= 0xd2
                    ? byte === 0xd2
                        ? this.i32().toString()
                        : this.i16().toString()
                    : byte === 0xd4
                        ? this.ext(1)
                        : (this.i32() * 4294967296 + this.i32()).toString()
                : byte <= 0xd6
                    ? byte === 0xd6
                        ? this.ext(4)
                        : this.ext(2)
                    : byte === 0xd8
                        ? this.ext(16)
                        : this.ext(8);
        }
        else {
            switch (byte) {
                case 0xd9:
                    return this.str(this.u8());
                case 0xda:
                    return this.str(this.u16());
                case 0xdb:
                    return this.str(this.u32());
                case 0xdc:
                    return this.arr(this.u16());
                case 0xdd:
                    return this.arr(this.u32());
                case 0xde:
                    return this.obj(this.u16());
                case 0xdf:
                    return this.obj(this.u32());
            }
        }
        return '';
    }
    /** @ignore */
    str(size) {
        const uint8 = this.uint8;
        const end = this.x + size;
        let x = this.x;
        let str = '';
        while (x < end) {
            const b1 = uint8[x++];
            if ((b1 & 0x80) === 0) {
                str += String.fromCharCode(b1);
            }
            else if ((b1 & 0xe0) === 0xc0) {
                str += String.fromCharCode(((b1 & 0x1f) << 6) | (uint8[x++] & 0x3f));
            }
            else if ((b1 & 0xf0) === 0xe0) {
                str += String.fromCharCode(((b1 & 0x1f) << 12) | ((uint8[x++] & 0x3f) << 6) | (uint8[x++] & 0x3f));
            }
            else if ((b1 & 0xf8) === 0xf0) {
                const b2 = uint8[x++] & 0x3f;
                const b3 = uint8[x++] & 0x3f;
                const b4 = uint8[x++] & 0x3f;
                let code = ((b1 & 0x07) << 0x12) | (b2 << 0x0c) | (b3 << 0x06) | b4;
                if (code > 0xffff) {
                    code -= 0x10000;
                    str += String.fromCharCode(((code >>> 10) & 0x3ff) | 0xd800);
                    code = 0xdc00 | (code & 0x3ff);
                }
                str += String.fromCharCode(code);
            }
            else {
                str += String.fromCharCode(b1);
            }
        }
        this.x = end;
        return (0, asString_1.asString)(str);
    }
    /** @ignore */
    obj(size) {
        let str = '{';
        for (let i = 0; i < size; i++) {
            if (i > 0)
                str += ',';
            str += this.key();
            str += ':';
            str += this.val();
        }
        return (str + '}');
    }
    /** @ignore */
    key() {
        return this.val();
    }
    /** @ignore */
    arr(size) {
        let str = '[';
        for (let i = 0; i < size; i++) {
            if (i > 0)
                str += ',';
            str += this.val();
        }
        return (str + ']');
    }
    /** @ignore */
    bin(size) {
        const end = this.x + size;
        const buf = this.uint8.subarray(this.x, end);
        this.x = end;
        return '"' + (0, toDataUri_1.toDataUri)(buf) + '"';
    }
    /** @ignore */
    ext(size) {
        const ext = this.u8();
        const end = this.x + size;
        const buf = this.uint8.subarray(this.x, end);
        this.x = end;
        return '"' + (0, toDataUri_1.toDataUri)(buf, { ext }) + '"';
    }
    /** @ignore */
    u8() {
        return this.view.getUint8(this.x++);
    }
    /** @ignore */
    u16() {
        const num = this.view.getUint16(this.x);
        this.x += 2;
        return num;
    }
    /** @ignore */
    u32() {
        const num = this.view.getUint32(this.x);
        this.x += 4;
        return num;
    }
    /** @ignore */
    i8() {
        return this.view.getInt8(this.x++);
    }
    /** @ignore */
    i16() {
        const num = this.view.getInt16(this.x);
        this.x += 2;
        return num;
    }
    /** @ignore */
    i32() {
        const num = this.view.getInt32(this.x);
        this.x += 4;
        return num;
    }
    /** @ignore */
    f32() {
        const pos = this.x;
        this.x += 4;
        return this.view.getFloat32(pos);
    }
    /** @ignore */
    f64() {
        const pos = this.x;
        this.x += 8;
        return this.view.getFloat64(pos);
    }
}
exports.MsgPackToJsonConverter = MsgPackToJsonConverter;
//# sourceMappingURL=MsgPackToJsonConverter.js.map