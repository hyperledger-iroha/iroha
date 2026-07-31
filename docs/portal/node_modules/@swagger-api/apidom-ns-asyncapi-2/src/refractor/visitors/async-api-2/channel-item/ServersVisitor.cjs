"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _ChannelItemsServers = _interopRequireDefault(require("../../../../elements/nces/ChannelItemsServers.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ServersVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _ChannelItemsServers.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const element = (0, _apidomCore.cloneDeep)(item);
      if ((0, _apidomCore.isStringElement)(element)) {
        element.classes.push('server-name');
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = ServersVisitor;