"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomError = require("@swagger-api/apidom-error");
var _apidomCore = require("@swagger-api/apidom-core");
var _modern = require("@swagger-api/apidom-json-pointer/modern");
var _MaximumResolveDepthError = _interopRequireDefault(require("../../../errors/MaximumResolveDepthError.cjs"));
var url = _interopRequireWildcard(require("../../../util/url.cjs"));
var _index = _interopRequireDefault(require("../../../parse/index.cjs"));
var _Reference = _interopRequireDefault(require("../../../Reference.cjs"));
var _elementId = require("./selectors/element-id.cjs");
// @ts-ignore
const visitAsync = _apidomCore.visit[Symbol.for('nodejs.util.promisify.custom')];

/**
 * The following rules apply:
 *
 * 1. When referencing an element in the local document, the id of the element MAY be used
 * 2. When referencing remote elements, an absolute URL or relative URL MAY be used
 * 3. When a URL fragment exists in the URL given, it references the element with the matching id in the given document. The URL fragment MAY need to be URL decoded before making a match.
 * 4. When a URL fragment does not exist, the URL references the root element
 * 5. When path is used, it references the given property of the referenced element
 * 6. When path is used in an element that includes the data of the pointer (such as with ref), the referenced path MAY need to be converted to a refract structure in order to be valid
 *
 * WARNING: this implementation only supports referencing elements in the local document. Points 2-4 are not supported.
 * @public
 */

/**
 * @public
 */
class ApiDOMDereferenceVisitor {
  reference;
  options;
  constructor({
    reference,
    options
  }) {
    this.reference = reference;
    this.options = options;
  }
  handleDereferenceError(error, refEl) {
    var _this$options$derefer;
    if ((_this$options$derefer = this.options.dereference.dereferenceOpts) != null && _this$options$derefer.continueOnError) {
      var _this$options$derefer2;
      (_this$options$derefer2 = this.options.dereference.dereferenceOpts) == null || _this$options$derefer2.errors.push({
        error,
        refEl
      });
      return undefined;
    }
    throw error;
  }
  toBaseURI(uri) {
    return url.resolve(this.reference.uri, url.sanitize(url.stripHash(uri)));
  }
  async toReference(uri) {
    // detect maximum depth of resolution
    if (this.reference.depth >= this.options.resolve.maxDepth) {
      throw new _MaximumResolveDepthError.default(`Maximum resolution depth of ${this.options.resolve.maxDepth} has been exceeded by file "${this.reference.uri}"`);
    }
    const baseURI = this.toBaseURI(uri);
    const {
      refSet
    } = this.reference;

    // we've already processed this Reference in past
    if (refSet.has(baseURI)) {
      return refSet.find((0, _ramda.propEq)(baseURI, 'uri'));
    }
    const parseResult = await (0, _index.default)(url.unsanitize(baseURI), {
      ...this.options,
      parse: {
        ...this.options.parse,
        mediaType: 'text/plain'
      }
    });

    // register new mutable reference with a refSet
    const mutableReference = new _Reference.default({
      uri: baseURI,
      value: (0, _apidomCore.cloneDeep)(parseResult),
      depth: this.reference.depth + 1
    });
    refSet.add(mutableReference);
    if (this.options.dereference.immutable) {
      // register new immutable reference with a refSet
      const immutableReference = new _Reference.default({
        uri: `immutable://${baseURI}`,
        value: parseResult,
        depth: this.reference.depth + 1
      });
      refSet.add(immutableReference);
    }
    return mutableReference;
  }
  async RefElement(refElement, key, parent, path, ancestors) {
    const refURI = (0, _apidomCore.toValue)(refElement);
    const refNormalizedURI = refURI.includes('#') ? refURI : `#${refURI}`;
    const retrievalURI = this.toBaseURI(refNormalizedURI);
    const isInternalReference = url.stripHash(this.reference.uri) === retrievalURI;
    const isExternalReference = !isInternalReference;

    // ignore resolving internal RefElements
    if (!this.options.resolve.internal && isInternalReference) {
      // skip traversing this ref element
      return false;
    }
    // ignore resolving external RefElements
    if (!this.options.resolve.external && isExternalReference) {
      // skip traversing this ref element
      return false;
    }
    let reference;
    try {
      reference = await this.toReference(refNormalizedURI);
    } catch (error) {
      return this.handleDereferenceError(error, refElement);
    }
    const refBaseURI = url.resolve(retrievalURI, refNormalizedURI);
    const elementID = _modern.URIFragmentIdentifier.fromURIReference(refBaseURI);
    let referencedElement;
    try {
      referencedElement = (0, _elementId.evaluate)(elementID, reference.value.result);
    } catch (error) {
      return this.handleDereferenceError(error, refElement);
    }
    if (!(0, _apidomCore.isElement)(referencedElement)) {
      const error = new _apidomError.ApiDOMError(`Referenced element with id="${elementID}" was not found`);
      return this.handleDereferenceError(error, refElement);
    }
    if (refElement === referencedElement) {
      const error = new _apidomError.ApiDOMError('RefElement cannot reference itself');
      return this.handleDereferenceError(error, refElement);
    }
    if ((0, _apidomCore.isRefElement)(referencedElement)) {
      const error = new _apidomError.ApiDOMError('RefElement cannot reference another RefElement');
      return this.handleDereferenceError(error, refElement);
    }
    if (isExternalReference) {
      // dive deep into the fragment
      const visitor = new ApiDOMDereferenceVisitor({
        reference,
        options: this.options
      });
      referencedElement = await visitAsync(referencedElement, visitor);
    }

    /**
     * When path is used, it references the given property of the referenced element
     */
    const referencedElementPath = (0, _apidomCore.toValue)(refElement.path);
    if (referencedElementPath !== 'element' && (0, _apidomCore.isElement)(referencedElement)) {
      referencedElement = (0, _apidomCore.refract)(referencedElement[referencedElementPath]);
    }

    /**
     * Transclusion of a Ref Element SHALL be defined in the if/else block below.
     */
    if ((0, _apidomCore.isObjectElement)(referencedElement) && (0, _apidomCore.isObjectElement)(ancestors[ancestors.length - 1]) && Array.isArray(parent) && typeof key === 'number') {
      /**
       * If the Ref Element is held by an Object Element and references an Object Element,
       * its content entries SHALL be inserted in place of the Ref Element.
       */
      parent.splice(key, 1, ...referencedElement.content);
    } else if ((0, _apidomCore.isArrayElement)(referencedElement) && Array.isArray(parent) && typeof key === 'number') {
      /**
       * If the Ref Element is held by an Array Element and references an Array Element,
       * its content entries SHALL be inserted in place of the Ref Element.
       */
      parent.splice(key, 1, ...referencedElement.content);
    } else if ((0, _apidomCore.isMemberElement)(parent)) {
      /**
       * The Ref Element is substituted by the Element it references.
       */
      parent.value = referencedElement; // eslint-disable-line no-param-reassign
    } else if (Array.isArray(parent)) {
      /**
       * The Ref Element is substituted by the Element it references.
       */
      parent[key] = referencedElement; // eslint-disable-line no-param-reassign
    }
    return !parent ? referencedElement : false;
  }
}
var _default = exports.default = ApiDOMDereferenceVisitor;