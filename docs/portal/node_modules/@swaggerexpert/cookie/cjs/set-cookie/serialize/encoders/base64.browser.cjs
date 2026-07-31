"use strict";

exports.__esModule = true;
exports.default = void 0;
const base64Encoder = input => {
  const bytes = new TextEncoder().encode(input);
  const binaryString = bytes.reduce((acc, byte) => acc + String.fromCharCode(byte), '');
  return btoa(binaryString);
};
var _default = exports.default = base64Encoder;