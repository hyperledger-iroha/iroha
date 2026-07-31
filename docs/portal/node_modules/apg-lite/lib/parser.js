/*  *************************************************************************************
 *   copyright: Copyright (c) 2023 Lowell D. Thomas, all rights reserved
 *     license: BSD-2-Clause (https://opensource.org/licenses/BSD-2-Clause)
 *
 *    Redistribution and use in source and binary forms, with or without
 *    modification, are permitted provided that the following conditions are met:
 *
 *    1. Redistributions of source code must retain the above copyright notice, this
 *       list of conditions and the following disclaimer.
 *
 *    2. Redistributions in binary form must reproduce the above copyright notice,
 *       this list of conditions and the following disclaimer in the documentation
 *       and/or other materials provided with the distribution.
 *
 *    THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 *    AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 *    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 *    DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 *    FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 *    DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 *    SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 *    CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 *    OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 *    OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 *   ********************************************************************************* */
export { Parser, Ast, Trace, Stats, utilities, identifiers };

const Parser = function fnparser() {
  const id = identifiers;
  const utils = utilities;
  const p = this;
  const thisFileName = 'parser.js: Parser(): ';
  const systemData = function systemData() {
    this.state = id.ACTIVE;
    this.phraseLength = 0;
    this.refresh = () => {
      this.state = id.ACTIVE;
      this.phraseLength = 0;
    };
  };
  p.ast = undefined;
  p.stats = undefined;
  p.trace = undefined;
  p.callbacks = [];
  let lookAhead = 0;
  let treeDepth = 0;
  let maxTreeDepth = 0;
  let nodeHits = 0;
  let maxMatched = 0;
  let rules = undefined;
  let udts = undefined;
  let opcodes = undefined;
  let chars = undefined;
  let sysData = new systemData();
  let ruleCallbacks = undefined;
  let udtCallbacks = undefined;
  let userData = undefined;
  const clear = () => {
    lookAhead = 0;
    treeDepth = 0;
    maxTreeDepth = 0;
    nodeHits = 0;
    maxMatched = 0;
    rules = undefined;
    udts = undefined;
    opcodes = undefined;
    chars = undefined;
    sysData.refresh();
    ruleCallbacks = undefined;
    udtCallbacks = undefined;
    userData = undefined;
  };

  const initializeCallbacks = () => {
    const functionName = `${thisFileName}initializeCallbacks(): `;
    let i;
    ruleCallbacks = [];
    udtCallbacks = [];
    for (i = 0; i < rules.length; i += 1) {
      ruleCallbacks[i] = undefined;
    }
    for (i = 0; i < udts.length; i += 1) {
      udtCallbacks[i] = undefined;
    }
    let func;
    const list = [];
    for (i = 0; i < rules.length; i += 1) {
      list.push(rules[i].lower);
    }
    for (i = 0; i < udts.length; i += 1) {
      list.push(udts[i].lower);
    }
    for (const index in p.callbacks) {
      if (p.callbacks.hasOwnProperty(index)) {
        i = list.indexOf(index.toLowerCase());
        if (i < 0) {
          throw new Error(`${functionName}syntax callback '${index}' not a rule or udt name`);
        }
        func = p.callbacks[index] ? p.callbacks[index] : undefined;
        if (typeof func === 'function' || func === undefined) {
          if (i < rules.length) {
            ruleCallbacks[i] = func;
          } else {
            udtCallbacks[i - rules.length] = func;
          }
        } else {
          throw new Error(`${functionName}syntax callback[${index}] must be function reference or falsy)`);
        }
      }
    }
  };

  p.parse = (grammar, startName, inputString, callbackData) => {
    const functionName = `${thisFileName}parse(): `;
    clear();
    chars = utils.stringToChars(inputString);
    rules = grammar.rules;
    udts = grammar.udts;
    const lower = startName.toLowerCase();
    let startIndex = undefined;
    for (const i in rules) {
      if (rules.hasOwnProperty(i)) {
        if (lower === rules[i].lower) {
          startIndex = rules[i].index;
          break;
        }
      }
    }
    if (startIndex === undefined) {
      throw new Error(`${functionName}start rule name '${startRule}' not recognized`);
    }
    initializeCallbacks();
    if (p.trace) {
      p.trace.init(rules, udts, chars);
    }
    if (p.stats) {
      p.stats.init(rules, udts);
    }
    if (p.ast) {
      p.ast.init(rules, udts, chars);
    }
    userData = callbackData;
    /* create a dummy opcode for the start rule */
    opcodes = [
      {
        type: id.RNM,
        index: startIndex,
      },
    ];
    /* execute the start rule */
    opExecute(0, 0);
    opcodes = undefined;
    /* test and return the sysData */
    let success = false;
    switch (sysData.state) {
      case id.ACTIVE:
        throw new Error(`${functionName}final state should never be 'ACTIVE'`);
      case id.NOMATCH:
        success = false;
        break;
      case id.EMPTY:
      case id.MATCH:
        if (sysData.phraseLength === chars.length) {
          success = true;
        } else {
          success = false;
        }
        break;
      default:
        throw new Error('unrecognized state');
    }
    return {
      success,
      state: sysData.state,
      stateName: id.idName(sysData.state),
      length: chars.length,
      matched: sysData.phraseLength,
      maxMatched,
      maxTreeDepth,
      nodeHits,
    };
  };
  // The `ALT` operator.<br>
  // Executes its child nodes, from left to right, until it finds a match.
  // Fails if *all* of its child nodes fail.
  const opALT = (opIndex, phraseIndex) => {
    const op = opcodes[opIndex];
    for (let i = 0; i < op.children.length; i += 1) {
      opExecute(op.children[i], phraseIndex);
      if (sysData.state !== id.NOMATCH) {
        break;
      }
    }
  };
  // The `CAT` operator.<br>
  // Executes all of its child nodes, from left to right,
  // concatenating the matched phrases.
  // Fails if *any* child nodes fail.
  const opCAT = (opIndex, phraseIndex) => {
    let success;
    let astLength;
    let catCharIndex;
    let catPhrase;
    const op = opcodes[opIndex];
    if (p.ast) {
      astLength = p.ast.getLength();
    }
    success = true;
    catCharIndex = phraseIndex;
    catPhrase = 0;
    for (let i = 0; i < op.children.length; i += 1) {
      opExecute(op.children[i], catCharIndex);
      if (sysData.state === id.NOMATCH) {
        success = false;
        break;
      } else {
        catCharIndex += sysData.phraseLength;
        catPhrase += sysData.phraseLength;
      }
    }
    if (success) {
      sysData.state = catPhrase === 0 ? id.EMPTY : id.MATCH;
      sysData.phraseLength = catPhrase;
    } else {
      sysData.state = id.NOMATCH;
      sysData.phraseLength = 0;
      if (p.ast) {
        p.ast.setLength(astLength);
      }
    }
  };
  // The `REP` operator.<br>
  // Repeatedly executes its single child node,
  // concatenating each of the matched phrases found.
  // The number of repetitions executed and its final sysData depends
  // on its `min` & `max` repetition values.
  const opREP = (opIndex, phraseIndex) => {
    let astLength;
    let repCharIndex;
    let repPhrase;
    let repCount;
    const op = opcodes[opIndex];
    if (op.max === 0) {
      // this is an empty-string acceptor
      // deprecated: use the TLS empty string operator, "", instead
      sysData.state = id.EMPTY;
      sysData.phraseLength = 0;
      return;
    }
    repCharIndex = phraseIndex;
    repPhrase = 0;
    repCount = 0;
    if (p.ast) {
      astLength = p.ast.getLength();
    }
    while (1) {
      if (repCharIndex >= chars.length) {
        /* exit on end of input string */
        break;
      }
      opExecute(opIndex + 1, repCharIndex);
      if (sysData.state === id.NOMATCH) {
        /* always end if the child node fails */
        break;
      }
      if (sysData.state === id.EMPTY) {
        /* REP always succeeds when the child node returns an empty phrase */
        /* this may not seem obvious, but that's the way it works out */
        break;
      }
      repCount += 1;
      repPhrase += sysData.phraseLength;
      repCharIndex += sysData.phraseLength;
      if (repCount === op.max) {
        /* end on maxed out reps */
        break;
      }
    }
    /* evaluate the match count according to the min, max values */
    if (sysData.state === id.EMPTY) {
      sysData.state = repPhrase === 0 ? id.EMPTY : id.MATCH;
      sysData.phraseLength = repPhrase;
    } else if (repCount >= op.min) {
      sysData.state = repPhrase === 0 ? id.EMPTY : id.MATCH;
      sysData.phraseLength = repPhrase;
    } else {
      sysData.state = id.NOMATCH;
      sysData.phraseLength = 0;
      if (p.ast) {
        p.ast.setLength(astLength);
      }
    }
  };
  // Validate the callback function's returned sysData values.
  // It's the user's responsibility to get them right
  // but `RNM` fails if not.
  const validateRnmCallbackResult = (rule, sysData, charsLeft, down) => {
    if (sysData.phraseLength > charsLeft) {
      let str = `${thisFileName}opRNM(${rule.name}): callback function error: `;
      str += `sysData.phraseLength: ${sysData.phraseLength}`;
      str += ` must be <= remaining chars: ${charsLeft}`;
      throw new Error(str);
    }
    switch (sysData.state) {
      case id.ACTIVE:
        if (!down) {
          throw new Error(
            `${thisFileName}opRNM(${rule.name}): callback function return error. ACTIVE state not allowed.`
          );
        }
        break;
      case id.EMPTY:
        sysData.phraseLength = 0;
        break;
      case id.MATCH:
        if (sysData.phraseLength === 0) {
          sysData.state = id.EMPTY;
        }
        break;
      case id.NOMATCH:
        sysData.phraseLength = 0;
        break;
      default:
        throw new Error(
          `${thisFileName}opRNM(${rule.name}): callback function return error. Unrecognized return state: ${sysData.state}`
        );
    }
  };
  // The `RNM` operator.<br>
  // This operator will acts as a root node for a parse tree branch below and
  // returns the matched phrase to its parent.
  // However, its larger responsibility is handling user-defined callback functions and `AST` nodes.
  // Note that the `AST` is a separate object, but `RNM` calls its functions to create its nodes.
  const opRNM = (opIndex, phraseIndex) => {
    let astLength;
    let astDefined;
    let savedOpcodes;
    const op = opcodes[opIndex];
    const rule = rules[op.index];
    const callback = ruleCallbacks[rule.index];
    /* ignore AST in look ahead (AND or NOT operator above) */
    if (!lookAhead) {
      astDefined = p.ast && p.ast.ruleDefined(op.index);
      if (astDefined) {
        astLength = p.ast.getLength();
        p.ast.down(op.index, rules[op.index].name);
      }
    }
    if (callback) {
      /* call user's callback going down the parse tree*/
      const charsLeft = chars.length - phraseIndex;
      callback(sysData, chars, phraseIndex, userData);
      validateRnmCallbackResult(rule, sysData, charsLeft, true);
      if (sysData.state === id.ACTIVE) {
        savedOpcodes = opcodes;
        opcodes = rule.opcodes;
        opExecute(0, phraseIndex);
        opcodes = savedOpcodes;
        /* call user's callback going up the parse tree*/
        callback(sysData, chars, phraseIndex, userData);
        validateRnmCallbackResult(rule, sysData, charsLeft, false);
      } /* implied else clause: just accept the callback sysData - RNM acting as UDT */
    } else {
      /* no callback - just execute the rule */
      savedOpcodes = opcodes;
      opcodes = rule.opcodes;
      opExecute(0, phraseIndex, sysData);
      opcodes = savedOpcodes;
    }
    if (!lookAhead) {
      /* end AST */
      if (astDefined) {
        if (sysData.state === id.NOMATCH) {
          p.ast.setLength(astLength);
        } else {
          p.ast.up(op.index, rule.name, phraseIndex, sysData.phraseLength);
        }
      }
    }
  };
  // The `TRG` operator.<br>
  // Succeeds if the single first character of the phrase is
  // within the `min - max` range.
  const opTRG = (opIndex, phraseIndex) => {
    const op = opcodes[opIndex];
    sysData.state = id.NOMATCH;
    if (phraseIndex < chars.length) {
      if (op.min <= chars[phraseIndex] && chars[phraseIndex] <= op.max) {
        sysData.state = id.MATCH;
        sysData.phraseLength = 1;
      }
    }
  };
  // The `TBS` operator.<br>
  // Matches its pre-defined phrase against the input string.
  // All characters must match exactly.
  // Case-sensitive literal strings (`'string'` & `%s"string"`) are translated to `TBS`
  // operators by `apg`.
  // Phrase length of zero is not allowed.
  // Empty phrases can only be defined with `TLS` operators.
  const opTBS = (opIndex, phraseIndex) => {
    const op = opcodes[opIndex];
    const len = op.string.length;
    sysData.state = id.NOMATCH;
    if (phraseIndex + len <= chars.length) {
      for (let i = 0; i < len; i += 1) {
        if (chars[phraseIndex + i] !== op.string[i]) {
          return;
        }
      }
      sysData.state = id.MATCH;
      sysData.phraseLength = len;
    } /* implied else NOMATCH */
  };
  // The `TLS` operator.<br>
  // Matches its pre-defined phrase against the input string.
  // A case-insensitive match is attempted for ASCII alphbetical characters.
  // `TLS` is the only operator that explicitly allows empty phrases.
  // `apg` will fail for empty `TBS`, case-sensitive strings (`''`) or
  // zero repetitions (`0*0RuleName` or `0RuleName`).
  const opTLS = (opIndex, phraseIndex) => {
    let code;
    const op = opcodes[opIndex];
    sysData.state = id.NOMATCH;
    const len = op.string.length;
    if (len === 0) {
      /* EMPTY match allowed for TLS */
      sysData.state = id.EMPTY;
      return;
    }
    if (phraseIndex + len <= chars.length) {
      for (let i = 0; i < len; i += 1) {
        code = chars[phraseIndex + i];
        if (code >= 65 && code <= 90) {
          code += 32;
        }
        if (code !== op.string[i]) {
          return;
        }
      }
      sysData.state = id.MATCH;
      sysData.phraseLength = len;
    } /* implied else NOMATCH */
  };
  // Validate the callback function's returned sysData values.
  // It's the user's responsibility to get it right but `UDT` fails if not.
  const validateUdtCallbackResult = (udt, sysData, charsLeft) => {
    if (sysData.phraseLength > charsLeft) {
      let str = `${thisFileName}opUDT(${udt.name}): callback function error: `;
      str += `sysData.phraseLength: ${sysData.phraseLength}`;
      str += ` must be <= remaining chars: ${charsLeft}`;
      throw new Error(str);
    }
    switch (sysData.state) {
      case id.ACTIVE:
        throw new Error(`${thisFileName}opUDT(${udt.name}) ACTIVE state return not allowed.`);
      case id.EMPTY:
        if (udt.empty) {
          sysData.phraseLength = 0;
        } else {
          throw new Error(`${thisFileName}opUDT(${udt.name}) may not return EMPTY.`);
        }
        break;
      case id.MATCH:
        if (sysData.phraseLength === 0) {
          if (udt.empty) {
            sysData.state = id.EMPTY;
          } else {
            throw new Error(`${thisFileName}opUDT(${udt.name}) may not return EMPTY.`);
          }
        }
        break;
      case id.NOMATCH:
        sysData.phraseLength = 0;
        break;
      default:
        throw new Error(
          `${thisFileName}opUDT(${udt.name}): callback function return error. Unrecognized return state: ${sysData.state}`
        );
    }
  };
  // The `UDT` operator.<br>
  // Simply calls the user's callback function, but operates like `RNM` with regard to the `AST`
  // and back referencing.
  // There is some ambiguity here. `UDT`s act as terminals for phrase recognition but as named rules
  // for `AST` nodes and back referencing.
  // See [`ast.js`](./ast.html) for usage.
  const opUDT = (opIndex, phraseIndex) => {
    let astLength;
    let astIndex;
    let astDefined;
    const op = opcodes[opIndex];
    const udt = udts[op.index];
    sysData.UdtIndex = udt.index;
    /* ignore AST in look ahead */
    if (!lookAhead) {
      astDefined = p.ast && p.ast.udtDefined(op.index);
      if (astDefined) {
        astIndex = rules.length + op.index;
        astLength = p.ast.getLength();
        p.ast.down(astIndex, udt.name);
      }
    }
    /* call the UDT */
    const charsLeft = chars.length - phraseIndex;
    udtCallbacks[op.index](sysData, chars, phraseIndex, userData);
    validateUdtCallbackResult(udt, sysData, charsLeft);
    if (!lookAhead) {
      /* end AST */
      if (astDefined) {
        if (sysData.state === id.NOMATCH) {
          p.ast.setLength(astLength);
        } else {
          p.ast.up(astIndex, udt.name, phraseIndex, sysData.phraseLength);
        }
      }
    }
  };
  // The `AND` operator.<br>
  // This is the positive `look ahead` operator.
  // Executes its single child node, returning the EMPTY state
  // if it succeedsand NOMATCH if it fails.
  // *Always* backtracks on any matched phrase and returns EMPTY on success.
  const opAND = (opIndex, phraseIndex) => {
    lookAhead += 1;
    opExecute(opIndex + 1, phraseIndex);
    lookAhead -= 1;
    sysData.phraseLength = 0;
    switch (sysData.state) {
      case id.EMPTY:
        sysData.state = id.EMPTY;
        break;
      case id.MATCH:
        sysData.state = id.EMPTY;
        break;
      case id.NOMATCH:
        sysData.state = id.NOMATCH;
        break;
      default:
        throw new Error(`opAND: invalid state ${sysData.state}`);
    }
  };
  // The `NOT` operator.<br>
  // This is the negative `look ahead` operator.
  // Executes its single child node, returning the EMPTY state
  // if it *fails* and NOMATCH if it succeeds.
  // *Always* backtracks on any matched phrase and returns EMPTY
  // on success (failure of its child node).
  const opNOT = (opIndex, phraseIndex) => {
    lookAhead += 1;
    opExecute(opIndex + 1, phraseIndex);
    lookAhead -= 1;
    sysData.phraseLength = 0;
    switch (sysData.state) {
      case id.EMPTY:
      case id.MATCH:
        sysData.state = id.NOMATCH;
        break;
      case id.NOMATCH:
        sysData.state = id.EMPTY;
        break;
      default:
        throw new Error(`opNOT: invalid state ${sysData.state}`);
    }
  };

  const opExecute = (opIndex, phraseIndex) => {
    const functionName = `${thisFileName}opExecute(): `;
    const op = opcodes[opIndex];
    nodeHits += 1;
    if (treeDepth > maxTreeDepth) {
      maxTreeDepth = treeDepth;
    }
    treeDepth += 1;
    sysData.refresh();
    if (p.trace) {
      p.trace.down(op, phraseIndex);
    }
    switch (op.type) {
      case id.ALT:
        opALT(opIndex, phraseIndex);
        break;
      case id.CAT:
        opCAT(opIndex, phraseIndex);
        break;
      case id.REP:
        opREP(opIndex, phraseIndex);
        break;
      case id.RNM:
        opRNM(opIndex, phraseIndex);
        break;
      case id.TRG:
        opTRG(opIndex, phraseIndex);
        break;
      case id.TBS:
        opTBS(opIndex, phraseIndex);
        break;
      case id.TLS:
        opTLS(opIndex, phraseIndex);
        break;
      case id.UDT:
        opUDT(opIndex, phraseIndex);
        break;
      case id.AND:
        opAND(opIndex, phraseIndex);
        break;
      case id.NOT:
        opNOT(opIndex, phraseIndex);
        break;
      default:
        throw new Error(`${functionName}unrecognized operator`);
    }
    if (!lookAhead) {
      if (phraseIndex + sysData.phraseLength > maxMatched) {
        maxMatched = phraseIndex + sysData.phraseLength;
      }
    }
    if (p.stats) {
      p.stats.collect(op, sysData);
    }
    if (p.trace) {
      p.trace.up(op, sysData.state, phraseIndex, sysData.phraseLength);
    }
    treeDepth -= 1;
  };
};

const Ast = function fnast() {
  const thisFileName = 'parser.js: Ast()): ';
  const id = identifiers;
  const utils = utilities;
  const a = this;
  let rules = undefined;
  let udts = undefined;
  let chars = undefined;
  let nodeCount = 0;
  const nodeCallbacks = [];
  const stack = [];
  const records = [];
  a.callbacks = [];
  /* called by the parser to initialize the AST with the rules, UDTs and the input characters */
  a.init = (rulesIn, udtsIn, charsIn) => {
    stack.length = 0;
    records.length = 0;
    nodeCount = 0;
    rules = rulesIn;
    udts = udtsIn;
    chars = charsIn;
    let i;
    const list = [];
    for (i = 0; i < rules.length; i += 1) {
      list.push(rules[i].lower);
    }
    for (i = 0; i < udts.length; i += 1) {
      list.push(udts[i].lower);
    }
    nodeCount = rules.length + udts.length;
    for (i = 0; i < nodeCount; i += 1) {
      nodeCallbacks[i] = undefined;
    }
    for (const index in a.callbacks) {
      if (a.callbacks.hasOwnProperty(index)) {
        const lower = index.toLowerCase();
        i = list.indexOf(lower);
        if (i < 0) {
          throw new Error(`${thisFileName}init: node '${index}' not a rule or udt name`);
        }
        nodeCallbacks[i] = a.callbacks[index];
      }
    }
  };
  /* AST node rule callbacks - called by the parser's `RNM` operator */
  a.ruleDefined = (index) => !!nodeCallbacks[index];
  /* AST node UDT callbacks - called by the parser's `UDT` operator */
  a.udtDefined = (index) => !!nodeCallbacks[rules.length + index];
  /* called by the parser's `RNM` & `UDT` operators
     builds a record for the downward traversal of the node */
  a.down = (callbackIndex, name) => {
    const thisIndex = records.length;
    stack.push(thisIndex);
    records.push({
      name,
      thisIndex,
      thatIndex: undefined,
      state: id.SEM_PRE,
      callbackIndex,
      phraseIndex: undefined,
      phraseLength: undefined,
      stack: stack.length,
    });
    return thisIndex;
  };
  /* called by the parser's `RNM` & `UDT` operators */
  /* builds a record for the upward traversal of the node */
  a.up = (callbackIndex, name, phraseIndex, phraseLength) => {
    const thisIndex = records.length;
    const thatIndex = stack.pop();
    records.push({
      name,
      thisIndex,
      thatIndex,
      state: id.SEM_POST,
      callbackIndex,
      phraseIndex,
      phraseLength,
      stack: stack.length,
    });
    records[thatIndex].thatIndex = thisIndex;
    records[thatIndex].phraseIndex = phraseIndex;
    records[thatIndex].phraseLength = phraseLength;
    return thisIndex;
  };
  // Called by the user to translate the AST.
  // Translate means to associate or apply some semantic action to the
  // phrases that were syntactically matched to the AST nodes according
  // to the defining grammar.
  // ```
  // data - optional user-defined data
  //        passed to the callback functions by the translator
  // ```
  a.translate = (data) => {
    let ret;
    let callback;
    let record;
    for (let i = 0; i < records.length; i += 1) {
      record = records[i];
      callback = nodeCallbacks[record.callbackIndex];
      if (callback) {
        if (record.state === id.SEM_PRE) {
          callback(id.SEM_PRE, chars, record.phraseIndex, record.phraseLength, data);
        } else if (callback) {
          callback(id.SEM_POST, chars, record.phraseIndex, record.phraseLength, data);
        }
      }
    }
  };
  /* called by the parser to reset the length of the records array */
  /* necessary on backtracking */
  a.setLength = (length) => {
    records.length = length;
    if (length > 0) {
      stack.length = records[length - 1].stack;
    } else {
      stack.length = 0;
    }
  };
  /* called by the parser to get the length of the records array */
  a.getLength = () => records.length;
  /* helper for XML display */
  function indent(n) {
    let ret = '';
    while (n-- > 0) {
      ret += ' ';
    }
    return ret;
  }
  // Generate an `XML` version of the AST.
  // Useful if you want to use a special or favorite XML parser to translate the
  // AST. Node data are JavaScript strings.
  a.toXml = () => {
    let xml = '';
    let depth = 0;
    xml += '<?xml version="1.0" encoding="utf-8"?>\n';
    xml += `<root nodes="${records.length / 2}" characters="${chars.length}">\n`;
    xml += `<!-- input string -->\n`;
    xml += indent(depth + 2);
    xml += utils.charsToString(chars);
    xml += '\n';
    records.forEach((rec) => {
      if (rec.state === id.SEM_PRE) {
        depth += 1;
        xml += indent(depth);
        xml += `<node name="${rec.name}" index="${rec.phraseIndex}" length="${rec.phraseLength}">\n`;
        xml += indent(depth + 2);
        xml += utils.charsToString(chars, rec.phraseIndex, rec.phraseLength);
        xml += '\n';
      } else {
        xml += indent(depth);
        xml += `</node><!-- name="${rec.name}" -->\n`;
        depth -= 1;
      }
    });

    xml += '</root>\n';
    return xml;
  };
};

const Trace = function fntrace() {
  const id = identifiers;
  const utils = utilities;
  const thisFile = 'parser.js: Trace(): ';
  let chars = undefined;
  let rules = undefined;
  let udts = undefined;
  let out = '';
  let treeDepth = 0;
  const MAX_PHRASE = 100;
  const t = this;
  const indent = (n) => {
    let ret = '';
    let count = 0;
    if (n >= 0) {
      while (n--) {
        count += 1;
        if (count === 5) {
          ret += '|';
          count = 0;
        } else {
          ret += '.';
        }
      }
    }
    return ret;
  };
  t.init = (r, u, c) => {
    rules = r;
    udts = u;
    chars = c;
  };
  const opName = (op) => {
    let name;
    switch (op.type) {
      case id.ALT:
        name = 'ALT';
        break;
      case id.CAT:
        name = 'CAT';
        break;
      case id.REP:
        if (op.max === Infinity) {
          name = `REP(${op.min},inf)`;
        } else {
          name = `REP(${op.min},${op.max})`;
        }
        break;
      case id.RNM:
        name = `RNM(${rules[op.index].name})`;
        break;
      case id.TRG:
        name = `TRG(${op.min},${op.max})`;
        break;
      case id.TBS:
        if (op.string.length > 6) {
          name = `TBS(${utils.charsToString(op.string, 0, 3)}...)`;
        } else {
          name = `TBS(${utils.charsToString(op.string, 0, 6)})`;
        }
        break;
      case id.TLS:
        if (op.string.length > 6) {
          name = `TLS(${utils.charsToString(op.string, 0, 3)}...)`;
        } else {
          name = `TLS(${utils.charsToString(op.string, 0, 6)})`;
        }
        break;
      case id.UDT:
        name = `UDT(${udts[op.index].name})`;
        break;
      case id.AND:
        name = 'AND';
        break;
      case id.NOT:
        name = 'NOT';
        break;
      default:
        throw new Error(`${thisFile}Trace: opName: unrecognized opcode`);
    }
    return name;
  };
  t.down = (op, offset) => {
    const lead = indent(treeDepth);
    const len = Math.min(MAX_PHRASE, chars.length - offset);
    let phrase = utils.charsToString(chars, offset, len);
    if (len < chars.length - offset) {
      phrase += '...';
    }
    phrase = `${lead}|-|[${opName(op)}]${phrase}\n`;
    out += phrase;
    treeDepth += 1;
  };
  t.up = (op, state, offset, phraseLength) => {
    const thisFunc = `${thisFile}trace.up: `;
    treeDepth -= 1;
    const lead = indent(treeDepth);
    let len;
    let phrase;
    let st;
    switch (state) {
      case id.EMPTY:
        st = '|E|';
        phrase = `''`;
        break;
      case id.MATCH:
        st = '|M|';
        len = Math.min(MAX_PHRASE, phraseLength);
        if (len < phraseLength) {
          phrase = `'${utils.charsToString(chars, offset, len)}...'`;
        } else {
          phrase = `'${utils.charsToString(chars, offset, len)}'`;
        }
        break;
      case id.NOMATCH:
        st = '|N|';
        phrase = '';
        break;
      default:
        throw new Error(`${thisFunc} unrecognized state`);
    }
    phrase = `${lead}${st}[${opName(op)}]${phrase}\n`;
    out += phrase;
  };
  t.displayTrace = () => out;
};

const Stats = function fnstats() {
  const id = identifiers;
  const thisFileName = 'parser.js: Stats(): ';
  let rules;
  let udts;
  let totals;
  const stats = [];
  const ruleStats = [];
  const udtStats = [];
  /* called by parser to initialize the stats */
  this.init = (r, u) => {
    rules = r;
    udts = u;
    clear();
  };
  /* This function is the main interaction with the parser. */
  /* The parser calls it after each node has been traversed. */
  this.collect = (op, sys) => {
    incStat(totals, sys.state, sys.phraseLength);
    incStat(stats[op.type], sys.state, sys.phraseLength);
    if (op.type === id.RNM) {
      incStat(ruleStats[op.index], sys.state, sys.phraseLength);
    }
    if (op.type === id.UDT) {
      incStat(udtStats[op.index], sys.state, sys.phraseLength);
    }
  };
  this.displayStats = () => {
    let out = '';
    const totals = {
      match: 0,
      empty: 0,
      nomatch: 0,
      total: 0,
    };
    const displayRow = (op, m, e, n, t) => {
      totals.match += m;
      totals.empty += e;
      totals.nomatch += n;
      totals.total += t;
      const mm = normalize(m);
      const ee = normalize(e);
      const nn = normalize(n);
      const tt = normalize(t);
      return `${op} | ${mm} | ${ee} | ${nn} | ${tt} |\n`;
    };
    out += '          OPERATOR STATS\n';
    out += '      |   MATCH |   EMPTY | NOMATCH |   TOTAL |\n';
    out += displayRow('  ALT', stats[id.ALT].match, stats[id.ALT].empty, stats[id.ALT].nomatch, stats[id.ALT].total);
    out += displayRow('  CAT', stats[id.CAT].match, stats[id.CAT].empty, stats[id.CAT].nomatch, stats[id.CAT].total);
    out += displayRow('  REP', stats[id.REP].match, stats[id.REP].empty, stats[id.REP].nomatch, stats[id.REP].total);
    out += displayRow('  RNM', stats[id.RNM].match, stats[id.RNM].empty, stats[id.RNM].nomatch, stats[id.RNM].total);
    out += displayRow('  TRG', stats[id.TRG].match, stats[id.TRG].empty, stats[id.TRG].nomatch, stats[id.TRG].total);
    out += displayRow('  TBS', stats[id.TBS].match, stats[id.TBS].empty, stats[id.TBS].nomatch, stats[id.TBS].total);
    out += displayRow('  TLS', stats[id.TLS].match, stats[id.TLS].empty, stats[id.TLS].nomatch, stats[id.TLS].total);
    out += displayRow('  UDT', stats[id.UDT].match, stats[id.UDT].empty, stats[id.UDT].nomatch, stats[id.UDT].total);
    out += displayRow('  AND', stats[id.AND].match, stats[id.AND].empty, stats[id.AND].nomatch, stats[id.AND].total);
    out += displayRow('  NOT', stats[id.NOT].match, stats[id.NOT].empty, stats[id.NOT].nomatch, stats[id.NOT].total);
    out += displayRow('TOTAL', totals.match, totals.empty, totals.nomatch, totals.total);
    return out;
  };
  /*
  Display rule/udt
  */
  this.displayHits = (type) => {
    let out = '';
    const displayRow = (m, e, n, t, name) => {
      totals.match += m;
      totals.empty += e;
      totals.nomatch += n;
      totals.total += t;
      const mm = normalize(m);
      const ee = normalize(e);
      const nn = normalize(n);
      const tt = normalize(t);
      return `| ${mm} | ${ee} | ${nn} | ${tt} | ${name}\n`;
    };
    if (typeof type === 'string' && type.toLowerCase()[0] === 'a') {
      ruleStats.sort(sortAlpha);
      udtStats.sort(sortAlpha);
      out += '    RULES/UDTS ALPHABETICALLY\n';
    } else if (typeof type === 'string' && type.toLowerCase()[0] === 'i') {
      ruleStats.sort(sortIndex);
      udtStats.sort(sortIndex);
      out += '    RULES/UDTS BY INDEX\n';
    } else {
      ruleStats.sort(sortHits);
      udtStats.sort(sortHits);
      out += '    RULES/UDTS BY HIT COUNT\n';
    }
    out += '|   MATCH |   EMPTY | NOMATCH |   TOTAL | NAME\n';
    for (let i = 0; i < ruleStats.length; i += 1) {
      let r = ruleStats[i];
      if (r.total) {
        out += displayRow(r.match, r.empty, r.nomatch, r.total, r.name);
      }
    }
    for (let i = 0; i < udtStats.length; i += 1) {
      let r = udtStats[i];
      if (r.total) {
        out += displayRow(r.match, r.empty, r.nomatch, r.total, r.name);
      }
    }
    return out;
  };
  const normalize = (n) => {
    if (n < 10) {
      return `      ${n}`;
    }
    if (n < 100) {
      return `     ${n}`;
    }
    if (n < 1000) {
      return `    ${n}`;
    }
    if (n < 10000) {
      return `   ${n}`;
    }
    if (n < 100000) {
      return `  ${n}`;
    }
    if (n < 1000000) {
      return ` ${n}`;
    }
    return `${n}`;
  };
  const sortAlpha = (lhs, rhs) => {
    if (lhs.lower < rhs.lower) {
      return -1;
    }
    if (lhs.lower > rhs.lower) {
      return 1;
    }
    return 0;
  };
  const sortHits = (lhs, rhs) => {
    if (lhs.total < rhs.total) {
      return 1;
    }
    if (lhs.total > rhs.total) {
      return -1;
    }
    return sortAlpha(lhs, rhs);
  };
  const sortIndex = (lhs, rhs) => {
    if (lhs.index < rhs.index) {
      return -1;
    }
    if (lhs.index > rhs.index) {
      return 1;
    }
    return 0;
  };
  const EmptyStat = function fnempty() {
    this.empty = 0;
    this.match = 0;
    this.nomatch = 0;
    this.total = 0;
  };
  /* Zero out all stats */
  const clear = () => {
    stats.length = 0;
    totals = new EmptyStat();
    stats[id.ALT] = new EmptyStat();
    stats[id.CAT] = new EmptyStat();
    stats[id.REP] = new EmptyStat();
    stats[id.RNM] = new EmptyStat();
    stats[id.TRG] = new EmptyStat();
    stats[id.TBS] = new EmptyStat();
    stats[id.TLS] = new EmptyStat();
    stats[id.UDT] = new EmptyStat();
    stats[id.AND] = new EmptyStat();
    stats[id.NOT] = new EmptyStat();
    ruleStats.length = 0;
    for (let i = 0; i < rules.length; i += 1) {
      ruleStats.push({
        empty: 0,
        match: 0,
        nomatch: 0,
        total: 0,
        name: rules[i].name,
        lower: rules[i].lower,
        index: rules[i].index,
      });
    }
    if (udts.length > 0) {
      udtStats.length = 0;
      for (let i = 0; i < udts.length; i += 1) {
        udtStats.push({
          empty: 0,
          match: 0,
          nomatch: 0,
          total: 0,
          name: udts[i].name,
          lower: udts[i].lower,
          index: udts[i].index,
        });
      }
    }
  };
  /* increment the designated operator hit count by one */
  const incStat = (stat, state) => {
    stat.total += 1;
    switch (state) {
      case id.EMPTY:
        stat.empty += 1;
        break;
      case id.MATCH:
        stat.match += 1;
        break;
      case id.NOMATCH:
        stat.nomatch += 1;
        break;
      default:
        throw new Error(`${thisFileName}collect(): incStat(): unrecognized state: ${state}`);
    }
  };
};

const utilities = {
  // utility functions
  stringToChars: (string) => [...string].map((cp) => cp.codePointAt(0)),
  charsToString: (chars, beg, len) => {
    let subChars = chars;
    while (1) {
      if (beg === undefined || beg < 0) {
        break;
      }
      if (len === undefined) {
        subChars = chars.slice(beg);
        break;
      }
      if (len <= 0) {
        // always an empty string
        return '';
      }
      subChars = chars.slice(beg, beg + len);
      break;
    }
    return String.fromCodePoint(...subChars);
  },
};

const identifiers = {
  // Identifies the operator type.
  // NB: These must match the values in apg-js 4.3.0, apg-lib/identifiers.
  /* the original ABNF operators */
  ALT: 1 /* alternation */,
  CAT: 2 /* concatenation */,
  REP: 3 /* repetition */,
  RNM: 4 /* rule name */,
  TRG: 5 /* terminal range */,
  TBS: 6 /* terminal binary string, case sensitive */,
  TLS: 7 /* terminal literal string, case insensitive */,
  /* the super set, SABNF operators */
  UDT: 11 /* user-defined terminal */,
  AND: 12 /* positive look ahead */,
  NOT: 13 /* negative look ahead */,
  // Used by the parser and the user's `RNM` and `UDT` callback functions.
  // Identifies the parser state as it traverses the parse tree nodes.
  // - *ACTIVE* - indicates the downward direction through the parse tree node.
  // - *MATCH* - indicates the upward direction and a phrase, of length \> 0, has been successfully matched
  // - *EMPTY* - indicates the upward direction and a phrase, of length = 0, has been successfully matched
  // - *NOMATCH* - indicates the upward direction and the parser failed to match any phrase at all
  ACTIVE: 100,
  MATCH: 101,
  EMPTY: 102,
  NOMATCH: 103,
  // Used by [`AST` translator](./ast.html) (semantic analysis) and the user's callback functions
  // to indicate the direction of flow through the `AST` nodes.
  // - *SEM_PRE* - indicates the downward (pre-branch) direction through the `AST` node.
  // - *SEM_POST* - indicates the upward (post-branch) direction through the `AST` node.
  SEM_PRE: 200,
  SEM_POST: 201,
  // Ignored. Retained for backwords compatibility.
  SEM_OK: 300,
  idName: (s) => {
    switch (s) {
      case identifiers.ALT:
        return 'ALT';
      case identifiers.CAT:
        return 'CAT';
      case identifiers.REP:
        return 'REP';
      case identifiers.RNM:
        return 'RNM';
      case identifiers.TRG:
        return 'TRG';
      case identifiers.TBS:
        return 'TBS';
      case identifiers.TLS:
        return 'TLS';
      case identifiers.UDT:
        return 'UDT';
      case identifiers.AND:
        return 'AND';
      case identifiers.NOT:
        return 'NOT';
      case identifiers.ACTIVE:
        return 'ACTIVE';
      case identifiers.EMPTY:
        return 'EMPTY';
      case identifiers.MATCH:
        return 'MATCH';
      case identifiers.NOMATCH:
        return 'NOMATCH';
      case identifiers.SEM_PRE:
        return 'SEM_PRE';
      case identifiers.SEM_POST:
        return 'SEM_POST';
      case identifiers.SEM_OK:
        return 'SEM_OK';
      default:
        return 'UNRECOGNIZED STATE';
    }
  },
};
