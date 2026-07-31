"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseComponentsVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Components = _interopRequireDefault(require("../../../../elements/Components.cjs"));
const BaseComponentsVisitor = exports.BaseComponentsVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Components.$visitor;
/**
 * @public
 */
class ComponentsVisitor extends BaseComponentsVisitor {
  constructor(options) {
    super(options);
    this.element = new _Components.default();
  }
}
var _default = exports.default = ComponentsVisitor;