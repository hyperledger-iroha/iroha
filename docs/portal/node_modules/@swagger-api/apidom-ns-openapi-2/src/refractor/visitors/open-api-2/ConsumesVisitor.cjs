"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _SwaggerConsumes = _interopRequireDefault(require("../../../elements/nces/SwaggerConsumes.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
/**
 * @public
 */
class ConsumesVisitor extends _FallbackVisitor.default {
  constructor(options) {
    super(options);
    this.element = new _SwaggerConsumes.default();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat((0, _apidomCore.cloneDeep)(arrayElement));
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = ConsumesVisitor;