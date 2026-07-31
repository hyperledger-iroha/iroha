import { mediaTypes, AsyncAPIMediaTypes } from '@swagger-api/apidom-ns-asyncapi-3';

/**
 * @public
 */
const yamlMediaTypes = new AsyncAPIMediaTypes(...mediaTypes.filterByFormat('generic'), ...mediaTypes.filterByFormat('yaml'));
export default yamlMediaTypes;