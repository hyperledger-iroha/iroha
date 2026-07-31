"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsonPackExtension = void 0;
/**
 * A wrapping for MessagePack extension or CBOR tag value. When encoder
 * encounters {@link JsonPackExtension} it will encode it as a MessagePack
 * extension or CBOR tag. Likewise, the decoder will
 * decode extensions into {@link JsonPackExtension}.
 *
 * @category Value
 */
class JsonPackExtension {
    constructor(tag, val) {
        this.tag = tag;
        this.val = val;
    }
}
exports.JsonPackExtension = JsonPackExtension;
//# sourceMappingURL=JsonPackExtension.js.map