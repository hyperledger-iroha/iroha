"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseMessageExampleVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _MessageExample = _interopRequireDefault(require("../../../../elements/MessageExample.cjs"));
const BaseMessageExampleVisitor = exports.BaseMessageExampleVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.MessageExample.$visitor;
/**
 * @public
 */
class MessageExampleVisitor extends BaseMessageExampleVisitor {
  constructor(options) {
    super(options);
    this.element = new _MessageExample.default();
  }
}
var _default = exports.default = MessageExampleVisitor;