"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
var _PatternedFieldsVisitor = _interopRequireDefault(require("./PatternedFieldsVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class MapVisitor extends _PatternedFieldsVisitor.default {
  constructor(options) {
    super(options);
    this.fieldPatternPredicate = _ramdaAdjunct.isNonEmptyString;
  }
}
var _default = exports.default = MapVisitor;