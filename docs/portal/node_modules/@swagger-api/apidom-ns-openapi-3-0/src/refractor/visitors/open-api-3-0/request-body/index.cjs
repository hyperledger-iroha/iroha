"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _RequestBody = _interopRequireDefault(require("../../../../elements/RequestBody.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class RequestBodyVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _RequestBody.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'RequestBody']);
  }
  ObjectElement(objectElement) {
    const result = _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // decorate every MediaTypeElement with media type metadata
    if ((0, _apidomCore.isObjectElement)(this.element.contentProp)) {
      this.element.contentProp.filter(_predicates.isMediaTypeElement)
      // @ts-ignore
      .forEach((mediaTypeElement, key) => {
        mediaTypeElement.setMetaProperty('media-type', (0, _apidomCore.toValue)(key));
      });
    }
    return result;
  }
}
var _default = exports.default = RequestBodyVisitor;