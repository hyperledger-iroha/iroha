"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BasePathItemVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _PathItem = _interopRequireDefault(require("../../../../elements/PathItem.cjs"));
/**
 * @public
 */
const BasePathItemVisitor = exports.BasePathItemVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.PathItem.$visitor;
/**
 * @public
 */
class PathItemVisitor extends BasePathItemVisitor {
  constructor(options) {
    super(options);
    this.element = new _PathItem.default();
  }
}
var _default = exports.default = PathItemVisitor;