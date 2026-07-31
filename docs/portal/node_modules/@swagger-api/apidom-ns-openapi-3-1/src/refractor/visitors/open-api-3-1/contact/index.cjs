"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseContactVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Contact = _interopRequireDefault(require("../../../../elements/Contact.cjs"));
/**
 * @public
 */
const BaseContactVisitor = exports.BaseContactVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Contact.$visitor;
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