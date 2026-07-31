"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _FixedFieldsVisitor = _interopRequireDefault(require("./FixedFieldsVisitor.cjs"));
var _PatternedFieldsVisitor = _interopRequireDefault(require("./PatternedFieldsVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class MixedFieldsVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _PatternedFieldsVisitor.default) {
  specPathFixedFields;
  specPathPatternedFields;
  constructor({
    specPathFixedFields,
    specPathPatternedFields,
    ...rest
  }) {
    super({
      ...rest
    });
    this.specPathFixedFields = specPathFixedFields;
    this.specPathPatternedFields = specPathPatternedFields;
  }
  ObjectElement(objectElement) {
    const {
      specPath,
      ignoredFields
    } = this;
    try {
      this.specPath = this.specPathFixedFields;
      const fixedFields = this.retrieveFixedFields(this.specPath(objectElement));
      // let FixedFieldsVisitor only process fixed fields and leave rest to PatternedFieldsVisitor
      // @ts-ignore
      this.ignoredFields = [...ignoredFields, ...(0, _ramda.difference)(objectElement.keys(), fixedFields)];
      _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
      this.specPath = this.specPathPatternedFields;
      this.ignoredFields = fixedFields;
      _PatternedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
    } catch (e) {
      this.specPath = specPath;
      throw e;
    }
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = MixedFieldsVisitor;