"use strict";

exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
/**
 * @public
 */

/**
 * @public
 */
class PatternPropertiesVisitor extends (0, _tsMixer.Mixin)(_apidomNsJsonSchemaDraft.MapVisitor, _apidomNsJsonSchemaDraft.ParentSchemaAwareVisitor, _apidomNsJsonSchemaDraft.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _apidomCore.ObjectElement();
    this.element.classes.push('json-schema-patternProperties');
    this.specPath = (0, _ramda.always)(['document', 'objects', 'JSONSchema']);
  }
}
var _default = exports.default = PatternPropertiesVisitor;