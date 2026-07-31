"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.resolveSchema$refField = exports.resolveSchema$idField = exports.maybeRefractToSchemaElement = exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = _interopRequireWildcard(require("@swagger-api/apidom-ns-openapi-3-1"));
var _DereferenceStrategy = _interopRequireDefault(require("../DereferenceStrategy.cjs"));
var _Reference = _interopRequireDefault(require("../../../Reference.cjs"));
var _ReferenceSet = _interopRequireDefault(require("../../../ReferenceSet.cjs"));
var _visitor = _interopRequireDefault(require("./visitor.cjs"));
exports.OpenAPI3_1DereferenceVisitor = _visitor.default;
var _util = require("./util.cjs");
exports.resolveSchema$refField = _util.resolveSchema$refField;
exports.resolveSchema$idField = _util.resolveSchema$idField;
exports.maybeRefractToSchemaElement = _util.maybeRefractToSchemaElement;
// @ts-ignore
const visitAsync = _apidomCore.visit[Symbol.for('nodejs.util.promisify.custom')];

/**
 * @public
 */

/**
 * @public
 */
class OpenAPI3_1DereferenceStrategy extends _DereferenceStrategy.default {
  constructor(options) {
    super({
      ...(options != null ? options : {}),
      name: 'openapi-3-1'
    });
  }
  canDereference(file) {
    var _file$parseResult;
    // assert by media type
    if (file.mediaType !== 'text/plain') {
      return _apidomNsOpenapi.mediaTypes.includes(file.mediaType);
    }

    // assert by inspecting ApiDOM
    return (0, _apidomNsOpenapi.isOpenApi3_1Element)((_file$parseResult = file.parseResult) == null ? void 0 : _file$parseResult.result);
  }
  async dereference(file, options) {
    var _options$dereference$;
    const namespace = (0, _apidomCore.createNamespace)(_apidomNsOpenapi.default);
    const immutableRefSet = (_options$dereference$ = options.dereference.refSet) != null ? _options$dereference$ : new _ReferenceSet.default();
    const mutableRefSet = new _ReferenceSet.default();
    let refSet = immutableRefSet;
    let reference;
    if (!immutableRefSet.has(file.uri)) {
      reference = new _Reference.default({
        uri: file.uri,
        value: file.parseResult
      });
      immutableRefSet.add(reference);
    } else {
      // pre-computed refSet was provided as configuration option
      reference = immutableRefSet.find((0, _ramda.propEq)(file.uri, 'uri'));
    }

    /**
     * Clone refSet due the dereferencing process being mutable.
     * We don't want to mutate the original refSet and the references.
     */
    if (options.dereference.immutable) {
      immutableRefSet.refs.map(ref => new _Reference.default({
        ...ref,
        value: (0, _apidomCore.cloneDeep)(ref.value)
      })).forEach(ref => mutableRefSet.add(ref));
      reference = mutableRefSet.find(ref => ref.uri === file.uri);
      refSet = mutableRefSet;
    }
    const visitor = new _visitor.default({
      reference: reference,
      namespace,
      options
    });
    const dereferencedElement = await visitAsync(refSet.rootRef.value, visitor, {
      keyMap: _apidomNsOpenapi.keyMap,
      nodeTypeGetter: _apidomNsOpenapi.getNodeType
    });

    /**
     * If immutable option is set, replay refs from the refSet.
     */
    if (options.dereference.immutable) {
      mutableRefSet.refs.filter(ref => ref.uri.startsWith('immutable://')).map(ref => new _Reference.default({
        ...ref,
        uri: ref.uri.replace(/^immutable:\/\//, '')
      })).forEach(ref => immutableRefSet.add(ref));
    }

    /**
     * Release all memory if this refSet was not provided as an configuration option.
     * If provided as configuration option, then provider is responsible for cleanup.
     */
    if (options.dereference.refSet === null) {
      immutableRefSet.clean();
    }
    mutableRefSet.clean();
    return dereferencedElement;
  }
}
var _default = exports.default = OpenAPI3_1DereferenceStrategy;