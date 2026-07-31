import { map } from 'ramda';
import { specificationObj as JSONSchemaDraft7Specification } from '@swagger-api/apidom-ns-json-schema-draft-7';
import SchemaOrReferenceVisitor from "./SchemaOrReferenceVisitor.mjs"; // @ts-ignore
const inheritedFixedFields = map(visitor => {
  if (visitor === JSONSchemaDraft7Specification.visitors.JSONSchemaOrJSONReferenceVisitor) {
    return SchemaOrReferenceVisitor;
  }
  return visitor;
}, JSONSchemaDraft7Specification.visitors.document.objects.JSONSchema.fixedFields);
export default inheritedFixedFields;