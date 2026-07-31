"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseMessageBindingsVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _MessageBindings = _interopRequireDefault(require("../../../../elements/MessageBindings.cjs"));
const BaseMessageBindingsVisitor = exports.BaseMessageBindingsVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.MessageBindings.$visitor;
/**
 * @public
 */
class MessageBindingsVisitor extends BaseMessageBindingsVisitor {
  constructor(options) {
    super(options);
    this.element = new _MessageBindings.default();
  }
}
var _default = exports.default = MessageBindingsVisitor;