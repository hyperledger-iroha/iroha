"use strict";

exports.__esModule = true;
exports.default = void 0;
const base64Decoder = input => {
  const binaryString = atob(input);
  const bytes = new Uint8Array([...binaryString].map(char => char.charCodeAt(0)));
  return new TextDecoder().decode(bytes);
};
var _default = exports.default = base64Decoder;