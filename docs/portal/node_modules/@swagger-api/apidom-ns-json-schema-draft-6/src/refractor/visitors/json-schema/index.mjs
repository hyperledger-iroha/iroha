import { JSONSchemaVisitor as JSONSchemaDraft4Visitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import JSONSchemaElement from "../../../elements/JSONSchema.mjs";
/**
 * @public
 */
class JSONSchemaVisitor extends JSONSchemaDraft4Visitor {
  constructor(options) {
    super(options);
    this.element = new JSONSchemaElement();
  }

  // eslint-disable-next-line class-methods-use-this
  get defaultDialectIdentifier() {
    return 'http://json-schema.org/draft-06/schema#';
  }
  BooleanElement(booleanElement) {
    const result = this.enter(booleanElement);
    this.element.classes.push('boolean-json-schema');
    return result;
  }
  handleSchemaIdentifier(objectElement, identifierKeyword = '$id') {
    return super.handleSchemaIdentifier(objectElement, identifierKeyword);
  }
}
export default JSONSchemaVisitor;