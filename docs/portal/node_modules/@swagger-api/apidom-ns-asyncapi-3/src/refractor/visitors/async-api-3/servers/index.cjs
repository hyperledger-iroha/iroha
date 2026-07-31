"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseServersVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Servers = _interopRequireDefault(require("../../../../elements/Servers.cjs"));
/**
 * @public
 */
const BaseServersVisitor = exports.BaseServersVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Servers.$visitor;
/**
 * @public
 */
class ServersVisitor extends BaseServersVisitor {
  constructor(options) {
    super(options);
    this.element = new _Servers.default();
    this.element.classes.push('servers');
  }
}
var _default = exports.default = ServersVisitor;