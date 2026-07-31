import { Ast as AST } from 'apg-lite';
import cstCallback from "../callbacks/cst.mjs";
class CSTTranslator extends AST {
  constructor() {
    super();
    this.callbacks['json-pointer'] = cstCallback('json-pointer');
    this.callbacks['reference-token'] = cstCallback('reference-token');
    this.callbacks['slash'] = cstCallback('text');
  }
  getTree() {
    const data = {
      stack: [],
      root: null
    };
    this.translate(data);
    delete data.stack;
    return data;
  }
}
export default CSTTranslator;