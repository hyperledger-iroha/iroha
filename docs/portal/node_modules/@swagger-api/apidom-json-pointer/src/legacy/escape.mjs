import { escape as baseEscape, URIFragmentIdentifier } from '@swaggerexpert/json-pointer';

/**
 * @public
 * @deprecated
 */
const escape = referenceToken => {
  return URIFragmentIdentifier.to(baseEscape(referenceToken)).slice(1);
};
export default escape;