"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _predicates = require("../../predicates.cjs");
var _AlternatingVisitor = _interopRequireDefault(require("../generics/AlternatingVisitor.cjs"));
/**
 * @public
 */
class SchemaOrReferenceVisitor extends _AlternatingVisitor.default {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: _predicates.isJSONReferenceLikeElement,
      specPath: ['document', 'objects', 'JSONReference']
    }, {
      predicate: _ramda.T,
      specPath: ['document', 'objects', 'JSONSchema']
    }];
  }
}
var _default = exports.default = SchemaOrReferenceVisitor;