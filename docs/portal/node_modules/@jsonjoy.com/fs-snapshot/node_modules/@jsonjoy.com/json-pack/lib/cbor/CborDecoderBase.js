"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CborDecoderBase = void 0;
const tslib_1 = require("tslib");
const f16_1 = require("@jsonjoy.com/buffers/lib/f16");
const JsonPackExtension_1 = require("../JsonPackExtension");
const JsonPackValue_1 = require("../JsonPackValue");
const Reader_1 = require("@jsonjoy.com/buffers/lib/Reader");
const sharedCachedUtf8Decoder_1 = tslib_1.__importDefault(require("@jsonjoy.com/buffers/lib/utf8/sharedCachedUtf8Decoder"));
class CborDecoderBase {
    constructor(reader = new Reader_1.Reader(), keyDecoder = sharedCachedUtf8Decoder_1.default) {
        this.reader = reader;
        this.keyDecoder = keyDecoder;
    }
    read(uint8) {
        this.reader.reset(uint8);
        return this.readAny();
    }
    decode(uint8) {
        this.reader.reset(uint8);
        return this.readAny();
    }
    // -------------------------------------------------------- Any value reading
    val() {
        return this.readAny();
    }
    readAny() {
        const reader = this.reader;
        const octet = reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major < 4 /* MAJOR.ARR */) {
            if (major < 2 /* MAJOR.BIN */)
                return major === 0 /* MAJOR.UIN */ ? this.readUint(minor) : this.readNint(minor);
            else
                return major === 2 /* MAJOR.BIN */ ? this.readBin(minor) : this.readStr(minor);
        }
        else {
            if (major < 6 /* MAJOR.TAG */)
                return major === 4 /* MAJOR.ARR */ ? this.readArr(minor) : this.readObj(minor);
            else
                return major === 6 /* MAJOR.TAG */ ? this.readTag(minor) : this.readTkn(minor);
        }
    }
    readAnyRaw(octet) {
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major < 4 /* MAJOR.ARR */) {
            if (major < 2 /* MAJOR.BIN */)
                return major === 0 /* MAJOR.UIN */ ? this.readUint(minor) : this.readNint(minor);
            else
                return major === 2 /* MAJOR.BIN */ ? this.readBin(minor) : this.readStr(minor);
        }
        else {
            if (major < 6 /* MAJOR.TAG */)
                return major === 4 /* MAJOR.ARR */ ? this.readArr(minor) : this.readObj(minor);
            else
                return major === 6 /* MAJOR.TAG */ ? this.readTag(minor) : this.readTkn(minor);
        }
    }
    readMinorLen(minor) {
        if (minor < 24)
            return minor;
        switch (minor) {
            case 24:
                return this.reader.u8();
            case 25:
                return this.reader.u16();
            case 26:
                return this.reader.u32();
            case 27:
                return Number(this.reader.u64());
            case 31:
                return -1;
            default:
                throw 1 /* ERROR.UNEXPECTED_MINOR */;
        }
    }
    // ----------------------------------------------------- Unsigned int reading
    readUint(minor) {
        if (minor < 25) {
            return minor === 24 ? this.reader.u8() : minor;
        }
        else {
            if (minor < 27) {
                return minor === 25 ? this.reader.u16() : this.reader.u32();
            }
            else {
                const num = this.reader.u64();
                return num > 9007199254740991 /* CONST.MAX_UINT */ ? num : Number(num);
            }
        }
    }
    // ----------------------------------------------------- Negative int reading
    readNint(minor) {
        if (minor < 25) {
            return minor === 24 ? -this.reader.u8() - 1 : -minor - 1;
        }
        else {
            if (minor < 27) {
                return minor === 25 ? -this.reader.u16() - 1 : -this.reader.u32() - 1;
            }
            else {
                const num = this.reader.u64();
                return num > 9007199254740991 /* CONST.MAX_UINT */ - 1 ? -num - BigInt(1) : -Number(num) - 1;
            }
        }
    }
    // ----------------------------------------------------------- Binary reading
    readBin(minor) {
        const reader = this.reader;
        if (minor <= 23)
            return reader.buf(minor);
        switch (minor) {
            case 24:
                return reader.buf(reader.u8());
            case 25:
                return reader.buf(reader.u16());
            case 26:
                return reader.buf(reader.u32());
            case 27:
                return reader.buf(Number(reader.u64()));
            case 31: {
                let size = 0;
                const list = [];
                while (this.reader.peak() !== 255 /* CONST.END */) {
                    const uint8 = this.readBinChunk();
                    size += uint8.length;
                    list.push(uint8);
                }
                this.reader.x++;
                const res = new Uint8Array(size);
                let offset = 0;
                const length = list.length;
                for (let i = 0; i < length; i++) {
                    const arr = list[i];
                    res.set(arr, offset);
                    offset += arr.length;
                }
                return res;
            }
            default:
                throw 1 /* ERROR.UNEXPECTED_MINOR */;
        }
    }
    readBinChunk() {
        const octet = this.reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major !== 2 /* MAJOR.BIN */)
            throw 2 /* ERROR.UNEXPECTED_BIN_CHUNK_MAJOR */;
        if (minor > 27)
            throw 3 /* ERROR.UNEXPECTED_BIN_CHUNK_MINOR */;
        return this.readBin(minor);
    }
    // ----------------------------------------------------------- String reading
    readAsStr() {
        const reader = this.reader;
        const octet = reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major !== 3 /* MAJOR.STR */)
            throw 11 /* ERROR.UNEXPECTED_STR_MAJOR */;
        return this.readStr(minor);
    }
    readStr(minor) {
        const reader = this.reader;
        if (minor <= 23)
            return reader.utf8(minor);
        switch (minor) {
            case 24:
                return reader.utf8(reader.u8());
            case 25:
                return reader.utf8(reader.u16());
            case 26:
                return reader.utf8(reader.u32());
            case 27:
                return reader.utf8(Number(reader.u64()));
            case 31: {
                let str = '';
                while (reader.peak() !== 255 /* CONST.END */)
                    str += this.readStrChunk();
                this.reader.x++;
                return str;
            }
            default:
                throw 1 /* ERROR.UNEXPECTED_MINOR */;
        }
    }
    readStrLen(minor) {
        if (minor <= 23)
            return minor;
        switch (minor) {
            case 24:
                return this.reader.u8();
            case 25:
                return this.reader.u16();
            case 26:
                return this.reader.u32();
            case 27:
                return Number(this.reader.u64());
            default:
                throw 1 /* ERROR.UNEXPECTED_MINOR */;
        }
    }
    readStrChunk() {
        const octet = this.reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major !== 3 /* MAJOR.STR */)
            throw 4 /* ERROR.UNEXPECTED_STR_CHUNK_MAJOR */;
        if (minor > 27)
            throw 5 /* ERROR.UNEXPECTED_STR_CHUNK_MINOR */;
        return this.readStr(minor);
    }
    // ------------------------------------------------------------ Array reading
    readArr(minor) {
        const length = this.readMinorLen(minor);
        if (length >= 0)
            return this.readArrRaw(length);
        return this.readArrIndef();
    }
    readArrRaw(length) {
        const arr = [];
        for (let i = 0; i < length; i++)
            arr.push(this.readAny());
        return arr;
    }
    readArrIndef() {
        const arr = [];
        while (this.reader.peak() !== 255 /* CONST.END */)
            arr.push(this.readAny());
        this.reader.x++;
        return arr;
    }
    // ----------------------------------------------------------- Object reading
    readObj(minor) {
        if (minor < 28) {
            let length = minor;
            switch (minor) {
                case 24:
                    length = this.reader.u8();
                    break;
                case 25:
                    length = this.reader.u16();
                    break;
                case 26:
                    length = this.reader.u32();
                    break;
                case 27:
                    length = Number(this.reader.u64());
                    break;
            }
            const obj = {};
            for (let i = 0; i < length; i++) {
                const key = this.key();
                if (key === '__proto__')
                    throw 6 /* ERROR.UNEXPECTED_OBJ_KEY */;
                const value = this.readAny();
                obj[key] = value;
            }
            return obj;
        }
        else if (minor === 31)
            return this.readObjIndef();
        else
            throw 1 /* ERROR.UNEXPECTED_MINOR */;
    }
    /** Remove this? */
    readObjRaw(length) {
        const obj = {};
        for (let i = 0; i < length; i++) {
            const key = this.key();
            const value = this.readAny();
            obj[key] = value;
        }
        return obj;
    }
    readObjIndef() {
        const obj = {};
        while (this.reader.peak() !== 255 /* CONST.END */) {
            const key = this.key();
            if (this.reader.peak() === 255 /* CONST.END */)
                throw 7 /* ERROR.UNEXPECTED_OBJ_BREAK */;
            const value = this.readAny();
            obj[key] = value;
        }
        this.reader.x++;
        return obj;
    }
    key() {
        const octet = this.reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major !== 3 /* MAJOR.STR */)
            return String(this.readAnyRaw(octet));
        const length = this.readStrLen(minor);
        if (length > 31)
            return this.reader.utf8(length);
        const key = this.keyDecoder.decode(this.reader.uint8, this.reader.x, length);
        this.reader.skip(length);
        return key;
    }
    // -------------------------------------------------------------- Tag reading
    readTag(minor) {
        if (minor <= 23)
            return this.readTagRaw(minor);
        switch (minor) {
            case 24:
                return this.readTagRaw(this.reader.u8());
            case 25:
                return this.readTagRaw(this.reader.u16());
            case 26:
                return this.readTagRaw(this.reader.u32());
            case 27:
                return this.readTagRaw(Number(this.reader.u64()));
            default:
                throw 1 /* ERROR.UNEXPECTED_MINOR */;
        }
    }
    readTagRaw(tag) {
        return new JsonPackExtension_1.JsonPackExtension(tag, this.readAny());
    }
    // ------------------------------------------------------------ Token reading
    readTkn(minor) {
        switch (minor) {
            case 0xf4 & 31 /* CONST.MINOR_MASK */:
                return false;
            case 0xf5 & 31 /* CONST.MINOR_MASK */:
                return true;
            case 0xf6 & 31 /* CONST.MINOR_MASK */:
                return null;
            case 0xf7 & 31 /* CONST.MINOR_MASK */:
                return undefined;
            case 0xf8 & 31 /* CONST.MINOR_MASK */:
                return new JsonPackValue_1.JsonPackValue(this.reader.u8());
            case 0xf9 & 31 /* CONST.MINOR_MASK */:
                return this.f16();
            case 0xfa & 31 /* CONST.MINOR_MASK */:
                return this.reader.f32();
            case 0xfb & 31 /* CONST.MINOR_MASK */:
                return this.reader.f64();
        }
        if (minor <= 23)
            return new JsonPackValue_1.JsonPackValue(minor);
        throw 1 /* ERROR.UNEXPECTED_MINOR */;
    }
    f16() {
        return (0, f16_1.decodeF16)(this.reader.u16());
    }
}
exports.CborDecoderBase = CborDecoderBase;
//# sourceMappingURL=CborDecoderBase.js.map