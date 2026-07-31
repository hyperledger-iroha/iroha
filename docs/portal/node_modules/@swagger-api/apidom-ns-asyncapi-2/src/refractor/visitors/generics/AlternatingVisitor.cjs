"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
class AlternatingVisitor extends _SpecificationVisitor.default {
  alternator;
  constructor({
    alternator,
    ...rest
  }) {
    super({
      ...rest
    });
    this.alternator = alternator;
  }
  enter(element) {
    const functions = this.alternator.map(({
      predicate,
      specPath
    }) => (0, _ramda.ifElse)(predicate, (0, _ramda.always)(specPath), _ramdaAdjunct.stubUndefined));
    const specPath = (0, _ramdaAdjunct.dispatch)(functions)(element);
    this.element = this.toRefractedElement(specPath, element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = AlternatingVisitor;