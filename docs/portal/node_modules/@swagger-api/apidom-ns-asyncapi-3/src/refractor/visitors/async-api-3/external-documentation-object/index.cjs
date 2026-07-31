"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseExternalDocumentationVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _ExternalDocumentation = _interopRequireDefault(require("../../../../elements/ExternalDocumentation.cjs"));
const BaseExternalDocumentationVisitor = exports.BaseExternalDocumentationVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.ExternalDocumentation.$visitor;
/**
 * @public
 */
class ExternalDocumentationVisitor extends BaseExternalDocumentationVisitor {
  constructor(options) {
    super(options);
    this.element = new _ExternalDocumentation.default();
  }
}
var _default = exports.default = ExternalDocumentationVisitor;