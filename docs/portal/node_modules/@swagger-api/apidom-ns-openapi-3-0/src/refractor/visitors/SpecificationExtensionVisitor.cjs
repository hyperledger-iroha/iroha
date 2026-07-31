"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("./SpecificationVisitor.cjs"));
/**
 * @public
 */
class SpecificationExtensionVisitor extends _SpecificationVisitor.default {
  MemberElement(memberElement) {
    this.element = (0, _apidomCore.cloneDeep)(memberElement);
    this.element.classes.push('specification-extension');
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = SpecificationExtensionVisitor;