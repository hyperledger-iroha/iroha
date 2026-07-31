"use strict";

exports.__esModule = true;
exports.merge = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _url = require("../util/url.cjs");
/**
 * Algorithm for deep merging options.
 */

const baseURILens = (0, _ramda.lens)((0, _ramda.path)(['resolve', 'baseURI']), (0, _ramda.assocPath)(['resolve', 'baseURI']));
const baseURIDefault = baseURI => (0, _ramdaAdjunct.isEmptyString)(baseURI) ? (0, _url.cwd)() : baseURI;

/**
 * @public
 */
// eslint-disable-next-line import/prefer-default-export
const merge = (lObj, rObj) => {
  const withoutDefaults = (0, _ramda.mergeDeepRight)(lObj, rObj);
  // @ts-ignore
  return (0, _ramda.over)(baseURILens, baseURIDefault, withoutDefaults);
};
exports.merge = merge;