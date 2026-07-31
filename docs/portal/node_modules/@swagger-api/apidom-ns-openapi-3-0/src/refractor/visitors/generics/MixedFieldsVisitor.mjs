import { Mixin } from 'ts-mixer';
import { difference } from 'ramda';
import { BREAK } from '@swagger-api/apidom-core';
import FixedFieldsVisitor from "./FixedFieldsVisitor.mjs";
import PatternedFieldsVisitor from "./PatternedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MixedFieldsVisitor extends Mixin(FixedFieldsVisitor, PatternedFieldsVisitor) {
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
      this.ignoredFields = [...ignoredFields, ...difference(objectElement.keys(), fixedFields)];
      FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
      this.specPath = this.specPathPatternedFields;
      this.ignoredFields = fixedFields;
      PatternedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
    } catch (e) {
      this.specPath = specPath;
      throw e;
    }
    return BREAK;
  }
}
export default MixedFieldsVisitor;