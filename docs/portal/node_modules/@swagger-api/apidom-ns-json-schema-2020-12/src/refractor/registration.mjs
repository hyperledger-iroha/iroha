import JSONSchemaElement from "../elements/JSONSchema.mjs";
import LinkDescriptionElement from "../elements/LinkDescription.mjs";
import { createRefractor } from "./index.mjs"; // register refractors specific to element types
JSONSchemaElement.refract = createRefractor(['visitors', 'document', 'objects', 'JSONSchema', '$visitor']);
LinkDescriptionElement.refract = createRefractor(['visitors', 'document', 'objects', 'LinkDescription', '$visitor']);
export { JSONSchemaElement, LinkDescriptionElement };