"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _OperationServers = _interopRequireDefault(require("../../../../elements/nces/OperationServers.cjs"));
var _ServersVisitor = _interopRequireDefault(require("../ServersVisitor.cjs"));
/**
 * @public
 */
class ServersVisitor extends _ServersVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _OperationServers.default();
  }
}
var _default = exports.default = ServersVisitor;