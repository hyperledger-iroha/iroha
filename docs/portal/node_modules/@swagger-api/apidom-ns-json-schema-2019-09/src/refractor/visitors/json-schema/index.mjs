import { isStringElement } from '@swagger-api/apidom-core';
import { FixedFieldsVisitor, JSONSchemaVisitor as JSONSchemaDraft7Visitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import JSONSchemaElement from "../../../elements/JSONSchema.mjs";
/**
 * @public
 */
class JSONSchemaVisitor extends JSONSchemaDraft7Visitor {
  constructor(options) {
    super(options);
    this.element = new JSONSchemaElement();
  }

  // eslint-disable-next-line class-methods-use-this
  get defaultDialectIdentifier() {
    return 'https://json-schema.org/draft/2019-09/schema';
  }
  ObjectElement(objectElement) {
    this.handleDialectIdentifier(objectElement);
    this.handleSchemaIdentifier(objectElement);

    // for further processing consider this Schema Element as parent for all embedded Schema Elements
    this.parent = this.element;
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // mark this SchemaElement with reference metadata
    if (isStringElement(this.element.$ref)) {
      this.element.classes.push('reference-element');
      this.element.setMetaProperty('referenced-element', 'schema');
    }
    return result;
  }
}
export default JSONSchemaVisitor;