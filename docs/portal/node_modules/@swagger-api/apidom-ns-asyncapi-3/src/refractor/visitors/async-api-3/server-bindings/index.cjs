"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseServerBindingsVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _ServerBindings = _interopRequireDefault(require("../../../../elements/ServerBindings.cjs"));
const BaseServerBindingsVisitor = exports.BaseServerBindingsVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.ServerBindings.$visitor;
/**
 * @public
 */
class ServerBindingsVisitor extends BaseServerBindingsVisitor {
  constructor(options) {
    super(options);
    this.element = new _ServerBindings.default();
  }
}
var _default = exports.default = ServerBindingsVisitor;