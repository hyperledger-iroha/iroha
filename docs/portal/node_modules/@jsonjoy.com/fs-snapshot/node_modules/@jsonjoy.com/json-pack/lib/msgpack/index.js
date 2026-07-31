"use strict";
/**
 * # `json-pack` MessagePack`
 *
 * Library for encoding and decoding JavaScript native structures to MessagePack
 * format.
 *
 * Use `Encoder` to encode plain JSON values.
 *
 * ```ts
 * import {Encoder, Decoder} from 'json-pack/lib/json-pack';
 *
 * const encoder = new Encoder();
 * const decoder = new Decoder();
 * const buffer = encoder.encode({foo: 'bar'});
 * const obj = decoder.decode(buffer);
 *
 * console.log(obj); // { foo: 'bar' }
 * ```
 *
 * For more:
 *
 * - Use {@link Encoder} to encode only JSON values.
 * - Use {@link EncoderFull} to also encode binary data, extensions and pre-computed MessagePack buffers.
 * - To encode binary data use `Uint8Array`.
 * - To encode an extension use {@link JsonPackExtension}.
 * - To encode a pre-computed MessagePack value use {@link JsonPackValue}.
 *
 * @module
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.MessagePackDecoder = exports.MessagePackEncoder = exports.JsonPackExtension = exports.JsonPackValue = exports.MsgPackToJsonConverter = exports.MsgPackDecoderFast = exports.MsgPackDecoder = exports.MsgPackEncoderStable = exports.MsgPackEncoder = exports.MsgPackEncoderFast = void 0;
const tslib_1 = require("tslib");
tslib_1.__exportStar(require("./types"), exports);
var MsgPackEncoderFast_1 = require("./MsgPackEncoderFast");
Object.defineProperty(exports, "MsgPackEncoderFast", { enumerable: true, get: function () { return MsgPackEncoderFast_1.MsgPackEncoderFast; } });
var MsgPackEncoder_1 = require("./MsgPackEncoder");
Object.defineProperty(exports, "MsgPackEncoder", { enumerable: true, get: function () { return MsgPackEncoder_1.MsgPackEncoder; } });
var MsgPackEncoderStable_1 = require("./MsgPackEncoderStable");
Object.defineProperty(exports, "MsgPackEncoderStable", { enumerable: true, get: function () { return MsgPackEncoderStable_1.MsgPackEncoderStable; } });
var MsgPackDecoder_1 = require("./MsgPackDecoder");
Object.defineProperty(exports, "MsgPackDecoder", { enumerable: true, get: function () { return MsgPackDecoder_1.MsgPackDecoder; } });
var MsgPackDecoderFast_1 = require("./MsgPackDecoderFast");
Object.defineProperty(exports, "MsgPackDecoderFast", { enumerable: true, get: function () { return MsgPackDecoderFast_1.MsgPackDecoderFast; } });
var MsgPackToJsonConverter_1 = require("./MsgPackToJsonConverter");
Object.defineProperty(exports, "MsgPackToJsonConverter", { enumerable: true, get: function () { return MsgPackToJsonConverter_1.MsgPackToJsonConverter; } });
var JsonPackValue_1 = require("../JsonPackValue");
Object.defineProperty(exports, "JsonPackValue", { enumerable: true, get: function () { return JsonPackValue_1.JsonPackValue; } });
var JsonPackExtension_1 = require("../JsonPackExtension");
Object.defineProperty(exports, "JsonPackExtension", { enumerable: true, get: function () { return JsonPackExtension_1.JsonPackExtension; } });
// User-friendly aliases
var MsgPackEncoder_2 = require("./MsgPackEncoder");
Object.defineProperty(exports, "MessagePackEncoder", { enumerable: true, get: function () { return MsgPackEncoder_2.MsgPackEncoder; } });
var MsgPackDecoder_2 = require("./MsgPackDecoder");
Object.defineProperty(exports, "MessagePackDecoder", { enumerable: true, get: function () { return MsgPackDecoder_2.MsgPackDecoder; } });
//# sourceMappingURL=index.js.map