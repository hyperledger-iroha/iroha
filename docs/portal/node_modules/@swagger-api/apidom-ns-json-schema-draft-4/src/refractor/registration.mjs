import JSONSchemaElement from "../elements/JSONSchema.mjs";
import JSONReferenceElement from "../elements/JSONReference.mjs";
import MediaElement from "../elements/Media.mjs";
import LinkDescriptionElement from "../elements/LinkDescription.mjs";
import { createRefractor } from "./index.mjs"; // register refractors specific to element types
JSONSchemaElement.refract = createRefractor(['visitors', 'document', 'objects', 'JSONSchema', '$visitor']);
JSONReferenceElement.refract = createRefractor(['visitors', 'document', 'objects', 'JSONReference', '$visitor']);
MediaElement.refract = createRefractor(['visitors', 'document', 'objects', 'Media', '$visitor']);
LinkDescriptionElement.refract = createRefractor(['visitors', 'document', 'objects', 'LinkDescription', '$visitor']);
export { JSONSchemaElement, JSONReferenceElement, MediaElement, LinkDescriptionElement };