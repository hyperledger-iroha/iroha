"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var signPonyfill = function signPonyfill(number) {
  return (number > 0) - (number < 0) || +number;
};
var _default = exports["default"] = signPonyfill;