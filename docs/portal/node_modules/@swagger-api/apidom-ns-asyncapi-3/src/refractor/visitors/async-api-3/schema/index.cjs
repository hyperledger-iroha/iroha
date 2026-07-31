"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseSchemaVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Schema = _interopRequireDefault(require("../../../../elements/Schema.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
const BaseSchemaVisitor = exports.BaseSchemaVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Schema.$visitor;
/**
 * @public
 */
class SchemaVisitor extends BaseSchemaVisitor {
  constructor(options) {
    super(options);
    this.element = new _Schema.default();
  }
  ObjectElement(objectElement) {
    this.element = new _Schema.default();
    return _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
  }
}
var _default = exports.default = SchemaVisitor;