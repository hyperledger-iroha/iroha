import { mergeDeepRight, lens, path, assocPath, over } from 'ramda';
import { isEmptyString } from 'ramda-adjunct';
import { cwd } from "../util/url.mjs";
/**
 * Algorithm for deep merging options.
 */

const baseURILens = lens(path(['resolve', 'baseURI']), assocPath(['resolve', 'baseURI']));
const baseURIDefault = baseURI => isEmptyString(baseURI) ? cwd() : baseURI;

/**
 * @public
 */
// eslint-disable-next-line import/prefer-default-export
export const merge = (lObj, rObj) => {
  const withoutDefaults = mergeDeepRight(lObj, rObj);
  // @ts-ignore
  return over(baseURILens, baseURIDefault, withoutDefaults);
};