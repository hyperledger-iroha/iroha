"use strict";

exports.__esModule = true;
exports.default = void 0;
var _utils = require("../../../utils.cjs");
const base64urlDecoder = input => {
  const base64 = (0, _utils.toBase64)(input);
  const binaryString = atob(base64);
  const bytes = new Uint8Array([...binaryString].map(char => char.charCodeAt(0)));
  return new TextDecoder().decode(bytes);
};
var _default = exports.default = base64urlDecoder;