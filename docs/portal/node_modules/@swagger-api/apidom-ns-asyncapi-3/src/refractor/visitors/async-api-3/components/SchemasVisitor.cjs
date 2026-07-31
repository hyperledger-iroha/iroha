"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _ComponentsSchemas = _interopRequireDefault(require("../../../../elements/nces/ComponentsSchemas.cjs"));
var _MapVisitor = _interopRequireDefault(require("../../generics/MapVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../../predicates.cjs");
var _predicates2 = require("../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class SchemasVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _ComponentsSchemas.default();
    this.specPath = element => (0, _apidomNsAsyncapi.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : (0, _predicates2.isMultiFormatSchemaLikeElement)(element) ? ['document', 'objects', 'MultiFormatSchema'] : ['document', 'objects', 'Schema'];
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    this.element.filter(_predicates.isMultiFormatSchemaElement)
    // @ts-ignore
    .forEach(multiFormatSchemaElement => {
      multiFormatSchemaElement.setMetaProperty('multiformat-schema-element', 'schema');
    });
    return result;
  }
}
var _default = exports.default = SchemasVisitor;