"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseServerVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Server = _interopRequireDefault(require("../../../../elements/Server.cjs"));
/**
 * @public
 */
const BaseServerVisitor = exports.BaseServerVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Server.$visitor;
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