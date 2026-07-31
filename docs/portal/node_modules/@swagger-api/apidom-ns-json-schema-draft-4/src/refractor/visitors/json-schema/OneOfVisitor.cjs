"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _ParentSchemaAwareVisitor = _interopRequireDefault(require("./ParentSchemaAwareVisitor.cjs"));
var _predicates = require("../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class OneOfVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _ParentSchemaAwareVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _apidomCore.ArrayElement();
    this.element.classes.push('json-schema-oneOf');
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = (0, _predicates.isJSONReferenceLikeElement)(item) ? ['document', 'objects', 'JSONReference'] : ['document', 'objects', 'JSONSchema'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = OneOfVisitor;