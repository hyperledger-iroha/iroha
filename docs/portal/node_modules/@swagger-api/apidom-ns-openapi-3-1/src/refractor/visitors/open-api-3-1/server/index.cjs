"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseServerVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Server = _interopRequireDefault(require("../../../../elements/Server.cjs"));
/**
 * @public
 */
const BaseServerVisitor = exports.BaseServerVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Server.$visitor;
/**
 * @public
 */
class ServerVisitor extends BaseServerVisitor {
  constructor(options) {
    super(options);
    this.element = new _Server.default();
  }
}
var _default = exports.default = ServerVisitor;