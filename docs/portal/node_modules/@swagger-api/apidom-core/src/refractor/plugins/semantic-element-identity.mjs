import { IdentityManager } from "../../identity/index.mjs";
/**
 * @public
 */
/**
 * Plugin for decorating every semantic element in ApiDOM tree with UUID.
 * @public
 */
const plugin = ({
  length = 6
} = {}) => ({
  predicates
}) => {
  let identityManager;
  return {
    pre() {
      identityManager = new IdentityManager({
        length
      });
    },
    visitor: {
      enter(element) {
        if (!predicates.isPrimitiveElement(element)) {
          element.id = identityManager.identify(element); // eslint-disable-line no-param-reassign
        }
      }
    },
    post() {
      identityManager = null;
    }
  };
};
export default plugin;