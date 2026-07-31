import { propOr, omit } from 'ramda';
import { isNotUndefined } from 'ramda-adjunct';
import { createNamespace } from '@swagger-api/apidom-core';
import { parse as parseYAML, detect as detectYAML } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import openApiNamespace, { OpenApi3_0Element } from '@swagger-api/apidom-ns-openapi-3-0';
export { default as mediaTypes } from "./media-types.mjs";
/**
 * @public
 */
export const detectionRegExp = /(?<YAML>^(["']?)openapi\2\s*:\s*(["']?)(?<version_yaml>3\.0\.(?:[1-9]\d*|0))\3(?:\s+|$))|(?<JSON>"openapi"\s*:\s*"(?<version_json>3\.0\.(?:[1-9]\d*|0))")/m;

/**
 * @public
 */
export const detect = async source => detectionRegExp.test(source) && (await detectYAML(source));

/**
 * @public
 */
export const parse = async (source, options = {}) => {
  const refractorOpts = propOr({}, 'refractorOpts', options);
  const parserOpts = omit(['refractorOpts'], options);
  const parseResultElement = await parseYAML(source, parserOpts);
  const {
    result
  } = parseResultElement;
  if (isNotUndefined(result)) {
    const openApiElement = OpenApi3_0Element.refract(result, refractorOpts);
    openApiElement.classes.push('result');
    parseResultElement.replaceResult(openApiElement);
  }
  return parseResultElement;
};

/**
 * @public
 */
export const namespace = createNamespace(openApiNamespace);