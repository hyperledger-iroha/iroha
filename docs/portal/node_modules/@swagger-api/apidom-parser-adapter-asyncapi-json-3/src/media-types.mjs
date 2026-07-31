import { mediaTypes, AsyncAPIMediaTypes } from '@swagger-api/apidom-ns-asyncapi-3';

/**
 * @public
 */
const jsonMediaTypes = new AsyncAPIMediaTypes(...mediaTypes.filterByFormat('generic'), ...mediaTypes.filterByFormat('json'));
export default jsonMediaTypes;