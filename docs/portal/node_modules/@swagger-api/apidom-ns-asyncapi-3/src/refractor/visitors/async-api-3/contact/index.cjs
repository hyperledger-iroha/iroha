"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseContactVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Contact = _interopRequireDefault(require("../../../../elements/Contact.cjs"));
/**
 * @public
 */
const BaseContactVisitor = exports.BaseContactVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Contact.$visitor;
/**
 * @public
 */
class ContactVisitor extends BaseContactVisitor {
  constructor(options) {
    super(options);
    this.element = new _Contact.default();
  }
}
var _default = exports.default = ContactVisitor;