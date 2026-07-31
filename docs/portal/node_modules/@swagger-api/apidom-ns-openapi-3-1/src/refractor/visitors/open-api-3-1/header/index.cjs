"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseHeaderVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Header = _interopRequireDefault(require("../../../../elements/Header.cjs"));
/**
 * @public
 */
const BaseHeaderVisitor = exports.BaseHeaderVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.$visitor;
/**
 * @public
 */
class HeaderVisitor extends BaseHeaderVisitor {
  constructor(options) {
    super(options);
    this.element = new _Header.default();
  }
}
var _default = exports.default = HeaderVisitor;