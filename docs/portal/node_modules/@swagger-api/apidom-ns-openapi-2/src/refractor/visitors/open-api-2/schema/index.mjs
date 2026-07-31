import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SchemaElement from "../../../../elements/Schema.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SchemaVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SchemaElement();
    this.specPath = always(['document', 'objects', 'Schema']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default SchemaVisitor;