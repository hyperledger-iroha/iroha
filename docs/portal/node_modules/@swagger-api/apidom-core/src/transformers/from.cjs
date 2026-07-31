"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _namespace = _interopRequireDefault(require("../namespace.cjs"));
/**
 * Transforms data to an Element from a particular namespace.
 *
 * The name of the function was originally `from`,
 * but it was renamed to `fromFn` to avoid issues with Parcel.js:
 *
 * - https://github.com/parcel-bundler/parcel/issues/9473
 * - https://github.com/swagger-api/swagger-ui/issues/9466#issuecomment-1881053410
 * @public
 */
const fromFn = (data, namespace = _namespace.default) => {
  if ((0, _ramdaAdjunct.isString)(data)) {
    // JSON serialized refract
    try {
      return namespace.fromRefract(JSON.parse(data));
    } catch {
      // noop
    }
  }
  if ((0, _ramdaAdjunct.isPlainObject)(data) && (0, _ramda.has)('element', data)) {
    // refract javascript structure
    return namespace.fromRefract(data);
  }
  return namespace.toElement(data);
};
var _default = exports.default = fromFn;