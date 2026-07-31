"use strict";

exports.__esModule = true;
exports["default"] = void 0;
var _ramda = require("ramda");
var compareLength = (0, _ramda.curry)(function (comparator, value, list) {
  return (0, _ramda.compose)(comparator(value), _ramda.length)(list);
});
var _default = exports["default"] = compareLength;