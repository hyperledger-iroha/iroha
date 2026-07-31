"use strict";

exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class SchemasVisitor extends (0, _tsMixer.Mixin)(_apidomNsOpenapi.MapVisitor, _apidomNsOpenapi.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _apidomNsOpenapi.ComponentsSchemasElement();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'Schema']);
  }
  ObjectElement(objectElement) {
    const result = _apidomNsOpenapi.MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate Schemas elements with Schema name
    this.element.filter(_predicates.isSchemaElement)
    // @ts-ignore
    .forEach((schemaElement, schemaName) => {
      schemaElement.setMetaProperty('schemaName', (0, _apidomCore.toValue)(schemaName));
    });
    return result;
  }
}
var _default = exports.default = SchemasVisitor;