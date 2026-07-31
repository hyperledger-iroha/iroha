import { omit, propOr } from 'ramda';
import { isNotUndefined } from 'ramda-adjunct';
import { createNamespace } from '@swagger-api/apidom-core';
import { parse as parseYAML, detect as detectYAML } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import asyncApiNamespace, { AsyncApi2Element } from '@swagger-api/apidom-ns-asyncapi-2';
export { default as mediaTypes } from "./media-types.mjs";
/**
 * @public
 */
export const detectionRegExp = /(?<YAML>^(["']?)asyncapi\2\s*:\s*(["']?)(?<version_yaml>2\.(?:[1-9]\d*|0)\.(?:[1-9]\d*|0))\3(?:\s+|$))|(?<JSON>"asyncapi"\s*:\s*"(?<version_json>2\.(?:[1-9]\d*|0)\.(?:[1-9]\d*|0))")/m;

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
    const asyncApiElement = AsyncApi2Element.refract(result, refractorOpts);
    asyncApiElement.classes.push('result');
    parseResultElement.replaceResult(asyncApiElement);
  }
  return parseResultElement;
};

/**
 * @public
 */
export const namespace = createNamespace(asyncApiNamespace);