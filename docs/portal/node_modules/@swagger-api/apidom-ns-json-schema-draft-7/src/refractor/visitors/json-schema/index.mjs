import { JSONSchemaVisitor as JSONSchemaDraft6Visitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import JSONSchemaElement from "../../../elements/JSONSchema.mjs";
/**
 * @public
 */
class JSONSchemaVisitor extends JSONSchemaDraft6Visitor {
  constructor(options) {
    super(options);
    this.element = new JSONSchemaElement();
  }

  // eslint-disable-next-line class-methods-use-this
  get defaultDialectIdentifier() {
    return 'http://json-schema.org/draft-07/schema#';
  }
}
export default JSONSchemaVisitor;