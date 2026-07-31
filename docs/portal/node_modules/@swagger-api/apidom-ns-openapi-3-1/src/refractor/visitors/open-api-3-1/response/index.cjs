"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseResponseVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Response = _interopRequireDefault(require("../../../../elements/Response.cjs"));
/**
 * @public
 */
const BaseResponseVisitor = exports.BaseResponseVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Response.$visitor;
/**
 * @public
 */
class ResponseVisitor extends BaseResponseVisitor {
  constructor(options) {
    super(options);
    this.element = new _Response.default();
  }
}
var _default = exports.default = ResponseVisitor;