"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _isFunction = _interopRequireDefault(require("../../isFunction.js"));
var _isNotUndefined = _interopRequireDefault(require("../../isNotUndefined.js"));
var _StringRepeat = _interopRequireDefault(require("./String.repeat.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var padEndPonyfill = function padEndPonyfill(padString, targetLength, value) {
  // eslint-disable-next-line no-bitwise
  var finalLength = targetLength >> 0;
  var finalPadString = String((0, _isNotUndefined["default"])(padString) ? padString : ' ');
  if (value.length > finalLength) {
    return String(value);
  }
  finalLength -= value.length;
  if (finalLength > finalPadString.length) {
    var remainingLength = finalLength / finalPadString.length;
    finalPadString += (0, _isFunction["default"])(String.prototype.repeat) ? finalPadString.repeat(remainingLength) : (0, _StringRepeat["default"])(finalPadString, remainingLength);
  }
  return String(value) + finalPadString.slice(0, finalLength);
};
var _default = exports["default"] = padEndPonyfill;