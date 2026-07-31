"use strict";

exports.__esModule = true;
exports.default = void 0;
var _buffer = require("buffer");
const base64Encoder = input => {
  return _buffer.Buffer.from(input).toString('base64');
};
var _default = exports.default = base64Encoder;