"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseExampleVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Example = _interopRequireDefault(require("../../../../elements/Example.cjs"));
/**
 * @public
 */
const BaseExampleVisitor = exports.BaseExampleVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Example.$visitor;
/**
 * @public
 */
class ExampleVisitor extends BaseExampleVisitor {
  constructor(options) {
    super(options);
    this.element = new _Example.default();
  }
}
var _default = exports.default = ExampleVisitor;