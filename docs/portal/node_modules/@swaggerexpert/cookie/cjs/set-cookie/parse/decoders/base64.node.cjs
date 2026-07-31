"use strict";

exports.__esModule = true;
exports.default = void 0;
var _buffer = require("buffer");
const base64Decoder = input => {
  return _buffer.Buffer.from(input, 'base64').toString();
};
var _default = exports.default = base64Decoder;