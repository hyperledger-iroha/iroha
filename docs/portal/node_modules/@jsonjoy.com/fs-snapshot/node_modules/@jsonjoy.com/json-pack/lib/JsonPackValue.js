"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsonPackValue = void 0;
/**
 * Use this wrapper is you have a pre-encoded MessagePack or CBOR value and you would
 * like to dump it into a the document as-is. The contents of `buf` will
 * be written as is to the document.
 *
 * It also serves as CBOR simple value container. In which case the type of value
 * `val` field is "number".
 *
 * @category Value
 */
class JsonPackValue {
    constructor(val) {
        this.val = val;
    }
}
exports.JsonPackValue = JsonPackValue;
//# sourceMappingURL=JsonPackValue.js.map