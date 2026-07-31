import { mediaTypes, OpenAPIMediaTypes } from '@swagger-api/apidom-ns-openapi-3-1';

/**
 * @public
 */
const yamlMediaTypes = new OpenAPIMediaTypes(...mediaTypes.filterByFormat('generic'), ...mediaTypes.filterByFormat('yaml'));
export default yamlMediaTypes;