"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
var _MapVisitor = _interopRequireDefault(require("../generics/MapVisitor.cjs"));
var _ParentSchemaAwareVisitor = _interopRequireDefault(require("./ParentSchemaAwareVisitor.cjs"));
var _predicates = require("../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class PropertiesVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _ParentSchemaAwareVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _apidomCore.ObjectElement();
    this.element.classes.push('json-schema-properties');
    this.specPath = element => (0, _predicates.isJSONReferenceLikeElement)(element) ? ['document', 'objects', 'JSONReference'] : ['document', 'objects', 'JSONSchema'];
  }
}
var _default = exports.default = PropertiesVisitor;