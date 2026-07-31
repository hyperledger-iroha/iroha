"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseDiscriminatorVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Discriminator = _interopRequireDefault(require("../../../../elements/Discriminator.cjs"));
/**
 * @public
 */
const BaseDiscriminatorVisitor = exports.BaseDiscriminatorVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Discriminator.$visitor;
/**
 * @public
 */
class DiscriminatorVisitor extends BaseDiscriminatorVisitor {
  constructor(options) {
    super(options);
    this.element = new _Discriminator.default();
    this.canSupportSpecificationExtensions = true;
  }
}
var _default = exports.default = DiscriminatorVisitor;