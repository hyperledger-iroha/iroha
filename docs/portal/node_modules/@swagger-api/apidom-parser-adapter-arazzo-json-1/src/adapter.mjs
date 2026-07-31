import { propOr, omit } from 'ramda';
import { isNotUndefined } from 'ramda-adjunct';
import { createNamespace } from '@swagger-api/apidom-core';
import { parse as parseJSON, detect as detectJSON } from '@swagger-api/apidom-parser-adapter-json';
import arazzoNamespace, { ArazzoSpecification1Element } from '@swagger-api/apidom-ns-arazzo-1';
export { default as mediaTypes } from "./media-types.mjs";
/**
 * @public
 */
export const detectionRegExp = /"arazzo"\s*:\s*"(?<version_json>1\.(?:[1-9]\d*|0)\.(?:[1-9]\d*|0))"/;

/**
 * @public
 */
export const detect = async source => detectionRegExp.test(source) && (await detectJSON(source));

/**
 * @public
 */
export const parse = async (source, options = {}) => {
  const refractorOpts = propOr({}, 'refractorOpts', options);
  const parserOpts = omit(['refractorOpts'], options);
  const parseResultElement = await parseJSON(source, parserOpts);
  const {
    result
  } = parseResultElement;
  if (isNotUndefined(result)) {
    const arazzoSpecificationElement = ArazzoSpecification1Element.refract(result, refractorOpts);
    arazzoSpecificationElement.classes.push('result');
    parseResultElement.replaceResult(arazzoSpecificationElement);
  }
  return parseResultElement;
};

/**
 * @public
 */
export const namespace = createNamespace(arazzoNamespace);