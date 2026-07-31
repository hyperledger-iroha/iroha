"use strict";

exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("../parse/index.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const testJSONPointer = jsonPointer => {
  try {
    const parseResult = (0, _index.default)(jsonPointer, {
      translator: null
    });
    return parseResult.result.success;
  } catch {
    return false;
  }
};
var _default = exports.default = testJSONPointer;