"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var url = _interopRequireWildcard(require("../util/url.cjs"));
var _File = _interopRequireDefault(require("../File.cjs"));
var plugins = _interopRequireWildcard(require("../util/plugins.cjs"));
var _ParseError = _interopRequireDefault(require("../errors/ParseError.cjs"));
var _UnmatchedResolverError = _interopRequireDefault(require("../errors/UnmatchedResolverError.cjs"));
var _util = require("../resolve/util.cjs");
/**
 * Parses the given file's contents, using the configured parser plugins.
 */
const parseFile = async (file, options) => {
  const optsBoundParsers = options.parse.parsers.map(parser => {
    const clonedParser = Object.create(parser);
    return Object.assign(clonedParser, options.parse.parserOpts);
  });
  const parsers = await plugins.filter('canParse', [file, options], optsBoundParsers);

  // we couldn't find any parser for this File
  if ((0, _ramda.isEmpty)(parsers)) {
    throw new _UnmatchedResolverError.default(file.uri);
  }
  try {
    const {
      plugin,
      result
    } = await plugins.run('parse', [file, options], parsers);

    // empty files handling
    if (!plugin.allowEmpty && result.isEmpty) {
      return Promise.reject(new _ParseError.default(`Error while parsing file "${file.uri}". File is empty.`));
    }
    return result;
  } catch (error) {
    throw new _ParseError.default(`Error while parsing file "${file.uri}"`, {
      cause: error
    });
  }
};

/**
 * Parses a file into ApiDOM.
 */
const parse = async (uri, options) => {
  /**
   * If the path is a filesystem path, then convert it to a URL.
   *
   * NOTE: According to the JSON Reference spec, these should already be URLs,
   * but, in practice, many people use local filesystem paths instead.
   * So we're being generous here and doing the conversion automatically.
   * This is not intended to be a 100% bulletproof solution.
   * If it doesn't work for your use-case, then use a URL instead.
   */
  const file = new _File.default({
    uri: url.sanitize(url.stripHash(uri)),
    mediaType: options.parse.mediaType
  });
  const data = await (0, _util.readFile)(file, options);
  return parseFile(new _File.default({
    ...file,
    data
  }), options);
};
var _default = exports.default = parse;