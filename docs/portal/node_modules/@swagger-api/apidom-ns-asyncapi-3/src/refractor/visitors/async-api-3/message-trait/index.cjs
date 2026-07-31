"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseMessageTraitVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _MessageTrait = _interopRequireDefault(require("../../../../elements/MessageTrait.cjs"));
const BaseMessageTraitVisitor = exports.BaseMessageTraitVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.MessageTrait.$visitor;
/**
 * @public
 */
class MessageTraitVisitor extends BaseMessageTraitVisitor {
  constructor(options) {
    super(options);
    this.element = new _MessageTrait.default();
  }
}
var _default = exports.default = MessageTraitVisitor;