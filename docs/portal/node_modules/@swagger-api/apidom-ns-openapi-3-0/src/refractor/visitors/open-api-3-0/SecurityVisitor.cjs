"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _Security = _interopRequireDefault(require("../../../elements/nces/Security.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class SecurityVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Security.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      if ((0, _apidomCore.isObjectElement)(item)) {
        const element = this.toRefractedElement(['document', 'objects', 'SecurityRequirement'], item);
        this.element.push(element);
      } else {
        this.element.push((0, _apidomCore.cloneDeep)(item));
      }
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = SecurityVisitor;