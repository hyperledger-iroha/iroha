"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CborDecoder = void 0;
const CborDecoderBase_1 = require("./CborDecoderBase");
const JsonPackValue_1 = require("../JsonPackValue");
class CborDecoder extends CborDecoderBase_1.CborDecoderBase {
    // -------------------------------------------------------------- Map reading
    readAsMap() {
        const octet = this.reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        switch (major) {
            case 5 /* MAJOR.MAP */:
                return this.readMap(minor);
            default:
                throw 0 /* ERROR.UNEXPECTED_MAJOR */;
        }
    }
    readMap(minor) {
        const length = this.readMinorLen(minor);
        if (length >= 0)
            return this.readMapRaw(length);
        else
            return this.readMapIndef();
    }
    readMapRaw(length) {
        const map = new Map();
        for (let i = 0; i < length; i++) {
            const key = this.readAny();
            const value = this.readAny();
            map.set(key, value);
        }
        return map;
    }
    readMapIndef() {
        const map = new Map();
        while (this.reader.peak() !== 255 /* CONST.END */) {
            const key = this.readAny();
            if (this.reader.peak() === 255 /* CONST.END */)
                throw 7 /* ERROR.UNEXPECTED_OBJ_BREAK */;
            const value = this.readAny();
            map.set(key, value);
        }
        this.reader.x++;
        return map;
    }
    // ----------------------------------------------------------- Value skipping
    skipN(n) {
        for (let i = 0; i < n; i++)
            this.skipAny();
    }
    skipAny() {
        this.skipAnyRaw(this.reader.u8());
    }
    skipAnyRaw(octet) {
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        switch (major) {
            case 0 /* MAJOR.UIN */:
            case 1 /* MAJOR.NIN */:
                this.skipUNint(minor);
                break;
            case 2 /* MAJOR.BIN */:
                this.skipBin(minor);
                break;
            case 3 /* MAJOR.STR */:
                this.skipStr(minor);
                break;
            case 4 /* MAJOR.ARR */:
                this.skipArr(minor);
                break;
            case 5 /* MAJOR.MAP */:
                this.skipObj(minor);
                break;
            case 7 /* MAJOR.TKN */:
                this.skipTkn(minor);
                break;
            case 6 /* MAJOR.TAG */:
                this.skipTag(minor);
                break;
        }
    }
    skipMinorLen(minor) {
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
            case 31:
                return -1;
            default:
                throw 1 /* ERROR.UNEXPECTED_MINOR */;
        }
    }
    // --------------------------------------------------------- Integer skipping
    skipUNint(minor) {
        if (minor <= 23)
            return;
        switch (minor) {
            case 24:
                return this.reader.skip(1);
            case 25:
                return this.reader.skip(2);
            case 26:
                return this.reader.skip(4);
            case 27:
                return this.reader.skip(8);
            default:
                throw 1 /* ERROR.UNEXPECTED_MINOR */;
        }
    }
    // ---------------------------------------------------------- Binary skipping
    skipBin(minor) {
        const length = this.skipMinorLen(minor);
        if (length >= 0)
            this.reader.skip(length);
        else {
            while (this.reader.peak() !== 255 /* CONST.END */)
                this.skipBinChunk();
            this.reader.x++;
        }
    }
    skipBinChunk() {
        const octet = this.reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major !== 2 /* MAJOR.BIN */)
            throw 2 /* ERROR.UNEXPECTED_BIN_CHUNK_MAJOR */;
        if (minor > 27)
            throw 3 /* ERROR.UNEXPECTED_BIN_CHUNK_MINOR */;
        this.skipBin(minor);
    }
    // ---------------------------------------------------------- String skipping
    skipStr(minor) {
        const length = this.skipMinorLen(minor);
        if (length >= 0)
            this.reader.skip(length);
        else {
            while (this.reader.peak() !== 255 /* CONST.END */)
                this.skipStrChunk();
            this.reader.x++;
        }
    }
    skipStrChunk() {
        const octet = this.reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        if (major !== 3 /* MAJOR.STR */)
            throw 4 /* ERROR.UNEXPECTED_STR_CHUNK_MAJOR */;
        if (minor > 27)
            throw 5 /* ERROR.UNEXPECTED_STR_CHUNK_MINOR */;
        this.skipStr(minor);
    }
    // ----------------------------------------------------------- Array skipping
    skipArr(minor) {
        const length = this.skipMinorLen(minor);
        if (length >= 0)
            this.skipN(length);
        else {
            while (this.reader.peak() !== 255 /* CONST.END */)
                this.skipAny();
            this.reader.x++;
        }
    }
    // ---------------------------------------------------------- Object skipping
    skipObj(minor) {
        const length = this.readMinorLen(minor);
        if (length >= 0)
            return this.skipN(length * 2);
        else {
            while (this.reader.peak() !== 255 /* CONST.END */) {
                this.skipAny();
                if (this.reader.peak() === 255 /* CONST.END */)
                    throw 7 /* ERROR.UNEXPECTED_OBJ_BREAK */;
                this.skipAny();
            }
            this.reader.x++;
        }
    }
    // ------------------------------------------------------------- Tag skipping
    skipTag(minor) {
        const length = this.skipMinorLen(minor);
        if (length < 0)
            throw 1 /* ERROR.UNEXPECTED_MINOR */;
        this.skipAny();
    }
    // ----------------------------------------------------------- Token skipping
    skipTkn(minor) {
        switch (minor) {
            case 0xf8 & 31 /* CONST.MINOR_MASK */:
                this.reader.skip(1);
                return;
            case 0xf9 & 31 /* CONST.MINOR_MASK */:
                this.reader.skip(2);
                return;
            case 0xfa & 31 /* CONST.MINOR_MASK */:
                this.reader.skip(4);
                return;
            case 0xfb & 31 /* CONST.MINOR_MASK */:
                this.reader.skip(8);
                return;
        }
        if (minor <= 23)
            return;
        throw 1 /* ERROR.UNEXPECTED_MINOR */;
    }
    // --------------------------------------------------------------- Validation
    /**
     * Throws if at given offset in a buffer there is an invalid CBOR value, or
     * if the value does not span the exact length specified in `size`. I.e.
     * throws if:
     *
     * - The value is not a valid CBOR value.
     * - The value is shorter than `size`.
     * - The value is longer than `size`.
     *
     * @param value Buffer in which to validate CBOR value.
     * @param offset Offset at which the value starts.
     * @param size Expected size of the value.
     */
    validate(value, offset = 0, size = value.length) {
        this.reader.reset(value);
        this.reader.x = offset;
        const start = offset;
        this.skipAny();
        const end = this.reader.x;
        if (end - start !== size)
            throw 8 /* ERROR.INVALID_SIZE */;
    }
    // -------------------------------------------- One level reading - any value
    decodeLevel(value) {
        this.reader.reset(value);
        return this.readLevel();
    }
    /**
     * Decodes only one level of objects and arrays. Other values are decoded
     * completely.
     *
     * @returns One level of decoded CBOR value.
     */
    readLevel() {
        const octet = this.reader.u8();
        const major = octet >> 5;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
        switch (major) {
            case 4 /* MAJOR.ARR */:
                return this.readArrLevel(minor);
            case 5 /* MAJOR.MAP */:
                return this.readObjLevel(minor);
            default:
                return super.readAnyRaw(octet);
        }
    }
    /**
     * Decodes primitive values, returns container values as `JsonPackValue`.
     *
     * @returns A primitive value, or CBOR container value as a blob.
     */
    readPrimitiveOrVal() {
        const octet = this.reader.peak();
        const major = octet >> 5;
        switch (major) {
            case 4 /* MAJOR.ARR */:
            case 5 /* MAJOR.MAP */:
                return this.readAsValue();
            default:
                return this.readAny();
        }
    }
    readAsValue() {
        const reader = this.reader;
        const start = reader.x;
        this.skipAny();
        const end = reader.x;
        return new JsonPackValue_1.JsonPackValue(reader.uint8.subarray(start, end));
    }
    // ----------------------------------------------- One level reading - object
    readObjLevel(minor) {
        const length = this.readMinorLen(minor);
        if (length >= 0)
            return this.readObjRawLevel(length);
        else
            return this.readObjIndefLevel();
    }
    readObjRawLevel(length) {
        const obj = {};
        for (let i = 0; i < length; i++) {
            const key = this.key();
            const value = this.readPrimitiveOrVal();
            obj[key] = value;
        }
        return obj;
    }
    readObjIndefLevel() {
        const obj = {};
        while (this.reader.peak() !== 255 /* CONST.END */) {
            const key = this.key();
            if (this.reader.peak() === 255 /* CONST.END */)
                throw 7 /* ERROR.UNEXPECTED_OBJ_BREAK */;
            const value = this.readPrimitiveOrVal();
            obj[key] = value;
        }
        this.reader.x++;
        return obj;
    }
    // ------------------------------------------------ One level reading - array
    readArrLevel(minor) {
        const length = this.readMinorLen(minor);
        if (length >= 0)
            return this.readArrRawLevel(length);
        return this.readArrIndefLevel();
    }
    readArrRawLevel(length) {
        const arr = [];
        for (let i = 0; i < length; i++)
            arr.push(this.readPrimitiveOrVal());
        return arr;
    }
    readArrIndefLevel() {
        const arr = [];
        while (this.reader.peak() !== 255 /* CONST.END */)
            arr.push(this.readPrimitiveOrVal());
        this.reader.x++;
        return arr;
    }
    // ---------------------------------------------------------- Shallow reading
    readHdr(expectedMajor) {
        const octet = this.reader.u8();
        const major = octet >> 5;
        if (major !== expectedMajor)
            throw 0 /* ERROR.UNEXPECTED_MAJOR */;
        const minor = octet & 31 /* CONST.MINOR_MASK */;
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
        }
        throw 1 /* ERROR.UNEXPECTED_MINOR */;
    }
    readStrHdr() {
        return this.readHdr(3 /* MAJOR.STR */);
    }
    readObjHdr() {
        return this.readHdr(5 /* MAJOR.MAP */);
    }
    readArrHdr() {
        return this.readHdr(4 /* MAJOR.ARR */);
    }
    findKey(key) {
        const size = this.readObjHdr();
        for (let i = 0; i < size; i++) {
            const k = this.key();
            if (k === key)
                return this;
            this.skipAny();
        }
        throw 9 /* ERROR.KEY_NOT_FOUND */;
    }
    findIndex(index) {
        const size = this.readArrHdr();
        if (index >= size)
            throw 10 /* ERROR.INDEX_OUT_OF_BOUNDS */;
        for (let i = 0; i < index; i++)
            this.skipAny();
        return this;
    }
    find(path) {
        for (let i = 0; i < path.length; i++) {
            const segment = path[i];
            if (typeof segment === 'string')
                this.findKey(segment);
            else
                this.findIndex(segment);
        }
        return this;
    }
}
exports.CborDecoder = CborDecoder;
//# sourceMappingURL=CborDecoder.js.map