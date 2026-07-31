"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.transclude = exports.default = void 0;
var _Transcluder = _interopRequireDefault(require("./Transcluder.cjs"));
/**
 * This is a mutating function. If you don't want your Element to be mutated,
 * clone in before passing it to this function.
 * @public
 */
const transclude = (search, replace, element) => {
  const transcluder = new _Transcluder.default({
    element
  });
  return transcluder.transclude(search, replace);
};
exports.transclude = transclude;
var _default = exports.default = _Transcluder.default;