"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseInfoVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Info = _interopRequireDefault(require("../../../../elements/Info.cjs"));
const BaseInfoVisitor = exports.BaseInfoVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Info.$visitor;
/**
 * @public
 */
class InfoVisitor extends BaseInfoVisitor {
  constructor(options) {
    super(options);
    this.element = new _Info.default();
  }
}
var _default = exports.default = InfoVisitor;