import { IdentityManager } from "../../identity/index.mjs";
/**
 * Plugin for decorating every element in ApiDOM tree with UUID.
 * @public
 */
const plugin = ({
  length = 6
} = {}) => () => {
  let identityManager;
  return {
    pre() {
      identityManager = new IdentityManager({
        length
      });
    },
    visitor: {
      enter(element) {
        element.id = identityManager.identify(element); // eslint-disable-line no-param-reassign
      }
    },
    post() {
      identityManager = null;
    }
  };
};
export default plugin;