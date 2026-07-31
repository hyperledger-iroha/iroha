"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseComponentsVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Components = _interopRequireDefault(require("../../../../elements/Components.cjs"));
/**
 * @public
 */
const BaseComponentsVisitor = exports.BaseComponentsVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.$visitor;
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