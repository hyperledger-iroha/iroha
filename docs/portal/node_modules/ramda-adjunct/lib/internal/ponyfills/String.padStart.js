"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _isFunction = _interopRequireDefault(require("../../isFunction.js"));
var _isNotUndefined = _interopRequireDefault(require("../../isNotUndefined.js"));
var _StringRepeat = _interopRequireDefault(require("./String.repeat.js"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { "default": e }; }
var padStartPonyfill = function padStartPonyfill(padString, targetLength, value) {
  // eslint-disable-next-line no-bitwise
  var finalLength = targetLength >> 0; // truncate if number, or convert non-number to 0;
  var finalPadString = String((0, _isNotUndefined["default"])(padString) ? padString : ' ');

  // return the original string, if targeted length is less than original strings length
  if (value.length >= finalLength) {
    return String(value);
  }
  finalLength -= value.length;
  if (finalLength > finalPadString.length) {
    var lenghtToPad = finalLength / finalPadString.length;
    // append to original to ensure we are longer than needed
    finalPadString += (0, _isFunction["default"])(String.prototype.repeat) ? finalPadString.repeat(lenghtToPad) : (0, _StringRepeat["default"])(finalPadString, lenghtToPad);
  }
  return finalPadString.slice(0, finalLength) + String(value);
};
var _default = exports["default"] = padStartPonyfill;