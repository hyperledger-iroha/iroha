"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
class FixedFieldsVisitor extends _SpecificationVisitor.default {
  specPath;
  ignoredFields;
  constructor({
    specPath,
    ignoredFields,
    ...rest
  }) {
    super({
      ...rest
    });
    this.specPath = specPath;
    this.ignoredFields = ignoredFields || [];
  }
  ObjectElement(objectElement) {
    const specPath = this.specPath(objectElement);
    const fields = this.retrieveFixedFields(specPath);

    // @ts-ignore
    objectElement.forEach((value, key, memberElement) => {
      if ((0, _apidomCore.isStringElement)(key) && fields.includes((0, _apidomCore.toValue)(key)) && !this.ignoredFields.includes((0, _apidomCore.toValue)(key))) {
        const fixedFieldElement = this.toRefractedElement([...specPath, 'fixedFields', (0, _apidomCore.toValue)(key)], value);
        const newMemberElement = new _apidomCore.MemberElement((0, _apidomCore.cloneDeep)(key), fixedFieldElement);
        newMemberElement.classes.push('fixed-field');
        this.copyMetaAndAttributes(memberElement, newMemberElement);
        this.element.content.push(newMemberElement);
      } else if (!this.ignoredFields.includes((0, _apidomCore.toValue)(key))) {
        this.element.content.push((0, _apidomCore.cloneDeep)(memberElement));
      }
    });
    this.copyMetaAndAttributes(objectElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = FixedFieldsVisitor;