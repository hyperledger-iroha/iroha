// export { uriParser };

const uriParser = (function () {
  /*****************************/
  /* the SABNF parser */
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
  // export { Parser, Ast, Trace, Stats, utilities, identifiers };

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
  /*****************************/
  /*****************************/
  // copyright: Copyright (c) 2024 Lowell D. Thomas, all rights reserved<br>
  //   license: BSD-2-Clause (https://opensource.org/licenses/BSD-2-Clause)<br>
  //
  // Generated by apg-js, Version 4.4.0 [apg-js](https://github.com/ldthomas/apg-js)
  const UriGrammar = function grammar() {
    // ```
    // SUMMARY
    //      rules = 36
    //       udts = 0
    //    opcodes = 217
    //        ---   ABNF original opcodes
    //        ALT = 14
    //        CAT = 30
    //        REP = 30
    //        RNM = 59
    //        TLS = 17
    //        TBS = 37
    //        TRG = 27
    //        ---   SABNF superset opcodes
    //        UDT = 0
    //        AND = 0
    //        NOT = 3
    //        BKA = 0
    //        BKN = 0
    //        BKR = 0
    //        ABG = 0
    //        AEN = 0
    // characters = [10 - 126]
    // ```
    /* OBJECT IDENTIFIER (for internal parser use) */
    this.grammarObject = 'grammarObject';

    /* RULES */
    this.rules = [];
    this.rules[0] = { name: 'URI', lower: 'uri', index: 0, isBkr: false };
    this.rules[1] = { name: 'hier-part', lower: 'hier-part', index: 1, isBkr: false };
    this.rules[2] = { name: 'scheme', lower: 'scheme', index: 2, isBkr: false };
    this.rules[3] = { name: 'authority', lower: 'authority', index: 3, isBkr: false };
    this.rules[4] = { name: 'path-abempty', lower: 'path-abempty', index: 4, isBkr: false };
    this.rules[5] = { name: 'path-absolute', lower: 'path-absolute', index: 5, isBkr: false };
    this.rules[6] = { name: 'path-rootless', lower: 'path-rootless', index: 6, isBkr: false };
    this.rules[7] = { name: 'path-empty', lower: 'path-empty', index: 7, isBkr: false };
    this.rules[8] = { name: 'userinfo-at', lower: 'userinfo-at', index: 8, isBkr: false };
    this.rules[9] = { name: 'userinfo', lower: 'userinfo', index: 9, isBkr: false };
    this.rules[10] = { name: 'host', lower: 'host', index: 10, isBkr: false };
    this.rules[11] = { name: 'IP-literal', lower: 'ip-literal', index: 11, isBkr: false };
    this.rules[12] = { name: 'IPvFuture', lower: 'ipvfuture', index: 12, isBkr: false };
    this.rules[13] = { name: 'IPv6address', lower: 'ipv6address', index: 13, isBkr: false };
    this.rules[14] = { name: 'nodcolon', lower: 'nodcolon', index: 14, isBkr: false };
    this.rules[15] = { name: 'dcolon', lower: 'dcolon', index: 15, isBkr: false };
    this.rules[16] = { name: 'h16', lower: 'h16', index: 16, isBkr: false };
    this.rules[17] = { name: 'h16c', lower: 'h16c', index: 17, isBkr: false };
    this.rules[18] = { name: 'h16n', lower: 'h16n', index: 18, isBkr: false };
    this.rules[19] = { name: 'h16cn', lower: 'h16cn', index: 19, isBkr: false };
    this.rules[20] = { name: 'IPv4address', lower: 'ipv4address', index: 20, isBkr: false };
    this.rules[21] = { name: 'dec-octet', lower: 'dec-octet', index: 21, isBkr: false };
    this.rules[22] = { name: 'dec-digit', lower: 'dec-digit', index: 22, isBkr: false };
    this.rules[23] = { name: 'reg-name', lower: 'reg-name', index: 23, isBkr: false };
    this.rules[24] = { name: 'reg-name-char', lower: 'reg-name-char', index: 24, isBkr: false };
    this.rules[25] = { name: 'port', lower: 'port', index: 25, isBkr: false };
    this.rules[26] = { name: 'query', lower: 'query', index: 26, isBkr: false };
    this.rules[27] = { name: 'fragment', lower: 'fragment', index: 27, isBkr: false };
    this.rules[28] = { name: 'segment', lower: 'segment', index: 28, isBkr: false };
    this.rules[29] = { name: 'segment-nz', lower: 'segment-nz', index: 29, isBkr: false };
    this.rules[30] = { name: 'pchar', lower: 'pchar', index: 30, isBkr: false };
    this.rules[31] = { name: 'pct-encoded', lower: 'pct-encoded', index: 31, isBkr: false };
    this.rules[32] = { name: 'ALPHA', lower: 'alpha', index: 32, isBkr: false };
    this.rules[33] = { name: 'LF', lower: 'lf', index: 33, isBkr: false };
    this.rules[34] = { name: 'DIGIT', lower: 'digit', index: 34, isBkr: false };
    this.rules[35] = { name: 'HEXDIG', lower: 'hexdig', index: 35, isBkr: false };

    /* UDTS */
    this.udts = [];

    /* OPCODES */
    /* URI */
    this.rules[0].opcodes = [];
    this.rules[0].opcodes[0] = { type: 2, children: [1, 2, 3, 4, 8] }; // CAT
    this.rules[0].opcodes[1] = { type: 4, index: 2 }; // RNM(scheme)
    this.rules[0].opcodes[2] = { type: 7, string: [58] }; // TLS
    this.rules[0].opcodes[3] = { type: 4, index: 1 }; // RNM(hier-part)
    this.rules[0].opcodes[4] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[0].opcodes[5] = { type: 2, children: [6, 7] }; // CAT
    this.rules[0].opcodes[6] = { type: 7, string: [63] }; // TLS
    this.rules[0].opcodes[7] = { type: 4, index: 26 }; // RNM(query)
    this.rules[0].opcodes[8] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[0].opcodes[9] = { type: 2, children: [10, 11] }; // CAT
    this.rules[0].opcodes[10] = { type: 7, string: [35] }; // TLS
    this.rules[0].opcodes[11] = { type: 4, index: 27 }; // RNM(fragment)

    /* hier-part */
    this.rules[1].opcodes = [];
    this.rules[1].opcodes[0] = { type: 1, children: [1, 5, 6, 7] }; // ALT
    this.rules[1].opcodes[1] = { type: 2, children: [2, 3, 4] }; // CAT
    this.rules[1].opcodes[2] = { type: 7, string: [47, 47] }; // TLS
    this.rules[1].opcodes[3] = { type: 4, index: 3 }; // RNM(authority)
    this.rules[1].opcodes[4] = { type: 4, index: 4 }; // RNM(path-abempty)
    this.rules[1].opcodes[5] = { type: 4, index: 5 }; // RNM(path-absolute)
    this.rules[1].opcodes[6] = { type: 4, index: 6 }; // RNM(path-rootless)
    this.rules[1].opcodes[7] = { type: 4, index: 7 }; // RNM(path-empty)

    /* scheme */
    this.rules[2].opcodes = [];
    this.rules[2].opcodes[0] = { type: 2, children: [1, 2] }; // CAT
    this.rules[2].opcodes[1] = { type: 4, index: 32 }; // RNM(ALPHA)
    this.rules[2].opcodes[2] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[2].opcodes[3] = { type: 1, children: [4, 5, 6, 7] }; // ALT
    this.rules[2].opcodes[4] = { type: 4, index: 32 }; // RNM(ALPHA)
    this.rules[2].opcodes[5] = { type: 4, index: 34 }; // RNM(DIGIT)
    this.rules[2].opcodes[6] = { type: 6, string: [43] }; // TBS
    this.rules[2].opcodes[7] = { type: 5, min: 45, max: 46 }; // TRG

    /* authority */
    this.rules[3].opcodes = [];
    this.rules[3].opcodes[0] = { type: 2, children: [1, 3, 4] }; // CAT
    this.rules[3].opcodes[1] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[3].opcodes[2] = { type: 4, index: 8 }; // RNM(userinfo-at)
    this.rules[3].opcodes[3] = { type: 4, index: 10 }; // RNM(host)
    this.rules[3].opcodes[4] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[3].opcodes[5] = { type: 2, children: [6, 7] }; // CAT
    this.rules[3].opcodes[6] = { type: 7, string: [58] }; // TLS
    this.rules[3].opcodes[7] = { type: 4, index: 25 }; // RNM(port)

    /* path-abempty */
    this.rules[4].opcodes = [];
    this.rules[4].opcodes[0] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[4].opcodes[1] = { type: 2, children: [2, 3] }; // CAT
    this.rules[4].opcodes[2] = { type: 7, string: [47] }; // TLS
    this.rules[4].opcodes[3] = { type: 4, index: 28 }; // RNM(segment)

    /* path-absolute */
    this.rules[5].opcodes = [];
    this.rules[5].opcodes[0] = { type: 2, children: [1, 2] }; // CAT
    this.rules[5].opcodes[1] = { type: 7, string: [47] }; // TLS
    this.rules[5].opcodes[2] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[5].opcodes[3] = { type: 2, children: [4, 5] }; // CAT
    this.rules[5].opcodes[4] = { type: 4, index: 29 }; // RNM(segment-nz)
    this.rules[5].opcodes[5] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[5].opcodes[6] = { type: 2, children: [7, 8] }; // CAT
    this.rules[5].opcodes[7] = { type: 7, string: [47] }; // TLS
    this.rules[5].opcodes[8] = { type: 4, index: 28 }; // RNM(segment)

    /* path-rootless */
    this.rules[6].opcodes = [];
    this.rules[6].opcodes[0] = { type: 2, children: [1, 2] }; // CAT
    this.rules[6].opcodes[1] = { type: 4, index: 29 }; // RNM(segment-nz)
    this.rules[6].opcodes[2] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[6].opcodes[3] = { type: 2, children: [4, 5] }; // CAT
    this.rules[6].opcodes[4] = { type: 7, string: [47] }; // TLS
    this.rules[6].opcodes[5] = { type: 4, index: 28 }; // RNM(segment)

    /* path-empty */
    this.rules[7].opcodes = [];
    this.rules[7].opcodes[0] = { type: 7, string: [] }; // TLS

    /* userinfo-at */
    this.rules[8].opcodes = [];
    this.rules[8].opcodes[0] = { type: 2, children: [1, 2] }; // CAT
    this.rules[8].opcodes[1] = { type: 4, index: 9 }; // RNM(userinfo)
    this.rules[8].opcodes[2] = { type: 6, string: [64] }; // TBS

    /* userinfo */
    this.rules[9].opcodes = [];
    this.rules[9].opcodes[0] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[9].opcodes[1] = { type: 1, children: [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] }; // ALT
    this.rules[9].opcodes[2] = { type: 5, min: 97, max: 122 }; // TRG
    this.rules[9].opcodes[3] = { type: 5, min: 65, max: 90 }; // TRG
    this.rules[9].opcodes[4] = { type: 5, min: 48, max: 57 }; // TRG
    this.rules[9].opcodes[5] = { type: 4, index: 31 }; // RNM(pct-encoded)
    this.rules[9].opcodes[6] = { type: 6, string: [33] }; // TBS
    this.rules[9].opcodes[7] = { type: 6, string: [36] }; // TBS
    this.rules[9].opcodes[8] = { type: 5, min: 38, max: 46 }; // TRG
    this.rules[9].opcodes[9] = { type: 5, min: 58, max: 59 }; // TRG
    this.rules[9].opcodes[10] = { type: 6, string: [61] }; // TBS
    this.rules[9].opcodes[11] = { type: 6, string: [95] }; // TBS
    this.rules[9].opcodes[12] = { type: 6, string: [126] }; // TBS

    /* host */
    this.rules[10].opcodes = [];
    this.rules[10].opcodes[0] = { type: 1, children: [1, 2, 6] }; // ALT
    this.rules[10].opcodes[1] = { type: 4, index: 11 }; // RNM(IP-literal)
    this.rules[10].opcodes[2] = { type: 2, children: [3, 4] }; // CAT
    this.rules[10].opcodes[3] = { type: 4, index: 20 }; // RNM(IPv4address)
    this.rules[10].opcodes[4] = { type: 13 }; // NOT
    this.rules[10].opcodes[5] = { type: 4, index: 24 }; // RNM(reg-name-char)
    this.rules[10].opcodes[6] = { type: 4, index: 23 }; // RNM(reg-name)

    /* IP-literal */
    this.rules[11].opcodes = [];
    this.rules[11].opcodes[0] = { type: 2, children: [1, 2, 5] }; // CAT
    this.rules[11].opcodes[1] = { type: 7, string: [91] }; // TLS
    this.rules[11].opcodes[2] = { type: 1, children: [3, 4] }; // ALT
    this.rules[11].opcodes[3] = { type: 4, index: 13 }; // RNM(IPv6address)
    this.rules[11].opcodes[4] = { type: 4, index: 12 }; // RNM(IPvFuture)
    this.rules[11].opcodes[5] = { type: 7, string: [93] }; // TLS

    /* IPvFuture */
    this.rules[12].opcodes = [];
    this.rules[12].opcodes[0] = { type: 2, children: [1, 2, 4, 5] }; // CAT
    this.rules[12].opcodes[1] = { type: 7, string: [118] }; // TLS
    this.rules[12].opcodes[2] = { type: 3, min: 1, max: Infinity }; // REP
    this.rules[12].opcodes[3] = { type: 4, index: 35 }; // RNM(HEXDIG)
    this.rules[12].opcodes[4] = { type: 7, string: [46] }; // TLS
    this.rules[12].opcodes[5] = { type: 3, min: 1, max: Infinity }; // REP
    this.rules[12].opcodes[6] = { type: 1, children: [7, 8, 9, 10, 11, 12, 13, 14, 15, 16] }; // ALT
    this.rules[12].opcodes[7] = { type: 5, min: 97, max: 122 }; // TRG
    this.rules[12].opcodes[8] = { type: 5, min: 65, max: 90 }; // TRG
    this.rules[12].opcodes[9] = { type: 5, min: 48, max: 57 }; // TRG
    this.rules[12].opcodes[10] = { type: 6, string: [33] }; // TBS
    this.rules[12].opcodes[11] = { type: 6, string: [36] }; // TBS
    this.rules[12].opcodes[12] = { type: 5, min: 38, max: 46 }; // TRG
    this.rules[12].opcodes[13] = { type: 5, min: 58, max: 59 }; // TRG
    this.rules[12].opcodes[14] = { type: 6, string: [61] }; // TBS
    this.rules[12].opcodes[15] = { type: 6, string: [95] }; // TBS
    this.rules[12].opcodes[16] = { type: 6, string: [126] }; // TBS

    /* IPv6address */
    this.rules[13].opcodes = [];
    this.rules[13].opcodes[0] = { type: 1, children: [1, 2] }; // ALT
    this.rules[13].opcodes[1] = { type: 4, index: 14 }; // RNM(nodcolon)
    this.rules[13].opcodes[2] = { type: 4, index: 15 }; // RNM(dcolon)

    /* nodcolon */
    this.rules[14].opcodes = [];
    this.rules[14].opcodes[0] = { type: 2, children: [1, 5] }; // CAT
    this.rules[14].opcodes[1] = { type: 2, children: [2, 3] }; // CAT
    this.rules[14].opcodes[2] = { type: 4, index: 18 }; // RNM(h16n)
    this.rules[14].opcodes[3] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[14].opcodes[4] = { type: 4, index: 19 }; // RNM(h16cn)
    this.rules[14].opcodes[5] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[14].opcodes[6] = { type: 2, children: [7, 8] }; // CAT
    this.rules[14].opcodes[7] = { type: 6, string: [58] }; // TBS
    this.rules[14].opcodes[8] = { type: 4, index: 20 }; // RNM(IPv4address)

    /* dcolon */
    this.rules[15].opcodes = [];
    this.rules[15].opcodes[0] = { type: 2, children: [1, 6, 7] }; // CAT
    this.rules[15].opcodes[1] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[15].opcodes[2] = { type: 2, children: [3, 4] }; // CAT
    this.rules[15].opcodes[3] = { type: 4, index: 16 }; // RNM(h16)
    this.rules[15].opcodes[4] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[15].opcodes[5] = { type: 4, index: 17 }; // RNM(h16c)
    this.rules[15].opcodes[6] = { type: 6, string: [58, 58] }; // TBS
    this.rules[15].opcodes[7] = { type: 1, children: [8, 17] }; // ALT
    this.rules[15].opcodes[8] = { type: 2, children: [9, 13] }; // CAT
    this.rules[15].opcodes[9] = { type: 2, children: [10, 11] }; // CAT
    this.rules[15].opcodes[10] = { type: 4, index: 18 }; // RNM(h16n)
    this.rules[15].opcodes[11] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[15].opcodes[12] = { type: 4, index: 19 }; // RNM(h16cn)
    this.rules[15].opcodes[13] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[15].opcodes[14] = { type: 2, children: [15, 16] }; // CAT
    this.rules[15].opcodes[15] = { type: 6, string: [58] }; // TBS
    this.rules[15].opcodes[16] = { type: 4, index: 20 }; // RNM(IPv4address)
    this.rules[15].opcodes[17] = { type: 3, min: 0, max: 1 }; // REP
    this.rules[15].opcodes[18] = { type: 4, index: 20 }; // RNM(IPv4address)

    /* h16 */
    this.rules[16].opcodes = [];
    this.rules[16].opcodes[0] = { type: 3, min: 1, max: 4 }; // REP
    this.rules[16].opcodes[1] = { type: 4, index: 35 }; // RNM(HEXDIG)

    /* h16c */
    this.rules[17].opcodes = [];
    this.rules[17].opcodes[0] = { type: 2, children: [1, 2] }; // CAT
    this.rules[17].opcodes[1] = { type: 6, string: [58] }; // TBS
    this.rules[17].opcodes[2] = { type: 3, min: 1, max: 4 }; // REP
    this.rules[17].opcodes[3] = { type: 4, index: 35 }; // RNM(HEXDIG)

    /* h16n */
    this.rules[18].opcodes = [];
    this.rules[18].opcodes[0] = { type: 2, children: [1, 3] }; // CAT
    this.rules[18].opcodes[1] = { type: 3, min: 1, max: 4 }; // REP
    this.rules[18].opcodes[2] = { type: 4, index: 35 }; // RNM(HEXDIG)
    this.rules[18].opcodes[3] = { type: 13 }; // NOT
    this.rules[18].opcodes[4] = { type: 6, string: [46] }; // TBS

    /* h16cn */
    this.rules[19].opcodes = [];
    this.rules[19].opcodes[0] = { type: 2, children: [1, 2, 4] }; // CAT
    this.rules[19].opcodes[1] = { type: 6, string: [58] }; // TBS
    this.rules[19].opcodes[2] = { type: 3, min: 1, max: 4 }; // REP
    this.rules[19].opcodes[3] = { type: 4, index: 35 }; // RNM(HEXDIG)
    this.rules[19].opcodes[4] = { type: 13 }; // NOT
    this.rules[19].opcodes[5] = { type: 6, string: [46] }; // TBS

    /* IPv4address */
    this.rules[20].opcodes = [];
    this.rules[20].opcodes[0] = { type: 2, children: [1, 2, 3, 4, 5, 6, 7] }; // CAT
    this.rules[20].opcodes[1] = { type: 4, index: 21 }; // RNM(dec-octet)
    this.rules[20].opcodes[2] = { type: 7, string: [46] }; // TLS
    this.rules[20].opcodes[3] = { type: 4, index: 21 }; // RNM(dec-octet)
    this.rules[20].opcodes[4] = { type: 7, string: [46] }; // TLS
    this.rules[20].opcodes[5] = { type: 4, index: 21 }; // RNM(dec-octet)
    this.rules[20].opcodes[6] = { type: 7, string: [46] }; // TLS
    this.rules[20].opcodes[7] = { type: 4, index: 21 }; // RNM(dec-octet)

    /* dec-octet */
    this.rules[21].opcodes = [];
    this.rules[21].opcodes[0] = { type: 3, min: 1, max: 3 }; // REP
    this.rules[21].opcodes[1] = { type: 4, index: 22 }; // RNM(dec-digit)

    /* dec-digit */
    this.rules[22].opcodes = [];
    this.rules[22].opcodes[0] = { type: 5, min: 48, max: 57 }; // TRG

    /* reg-name */
    this.rules[23].opcodes = [];
    this.rules[23].opcodes[0] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[23].opcodes[1] = { type: 4, index: 24 }; // RNM(reg-name-char)

    /* reg-name-char */
    this.rules[24].opcodes = [];
    this.rules[24].opcodes[0] = { type: 1, children: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] }; // ALT
    this.rules[24].opcodes[1] = { type: 5, min: 97, max: 122 }; // TRG
    this.rules[24].opcodes[2] = { type: 5, min: 65, max: 90 }; // TRG
    this.rules[24].opcodes[3] = { type: 5, min: 48, max: 57 }; // TRG
    this.rules[24].opcodes[4] = { type: 4, index: 31 }; // RNM(pct-encoded)
    this.rules[24].opcodes[5] = { type: 6, string: [33] }; // TBS
    this.rules[24].opcodes[6] = { type: 6, string: [36] }; // TBS
    this.rules[24].opcodes[7] = { type: 5, min: 38, max: 46 }; // TRG
    this.rules[24].opcodes[8] = { type: 6, string: [59] }; // TBS
    this.rules[24].opcodes[9] = { type: 6, string: [61] }; // TBS
    this.rules[24].opcodes[10] = { type: 6, string: [95] }; // TBS
    this.rules[24].opcodes[11] = { type: 6, string: [126] }; // TBS

    /* port */
    this.rules[25].opcodes = [];
    this.rules[25].opcodes[0] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[25].opcodes[1] = { type: 4, index: 34 }; // RNM(DIGIT)

    /* query */
    this.rules[26].opcodes = [];
    this.rules[26].opcodes[0] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[26].opcodes[1] = { type: 1, children: [2, 3, 4] }; // ALT
    this.rules[26].opcodes[2] = { type: 4, index: 30 }; // RNM(pchar)
    this.rules[26].opcodes[3] = { type: 6, string: [47] }; // TBS
    this.rules[26].opcodes[4] = { type: 6, string: [63] }; // TBS

    /* fragment */
    this.rules[27].opcodes = [];
    this.rules[27].opcodes[0] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[27].opcodes[1] = { type: 1, children: [2, 3, 4] }; // ALT
    this.rules[27].opcodes[2] = { type: 4, index: 30 }; // RNM(pchar)
    this.rules[27].opcodes[3] = { type: 6, string: [47] }; // TBS
    this.rules[27].opcodes[4] = { type: 6, string: [63] }; // TBS

    /* segment */
    this.rules[28].opcodes = [];
    this.rules[28].opcodes[0] = { type: 3, min: 0, max: Infinity }; // REP
    this.rules[28].opcodes[1] = { type: 4, index: 30 }; // RNM(pchar)

    /* segment-nz */
    this.rules[29].opcodes = [];
    this.rules[29].opcodes[0] = { type: 3, min: 1, max: Infinity }; // REP
    this.rules[29].opcodes[1] = { type: 4, index: 30 }; // RNM(pchar)

    /* pchar */
    this.rules[30].opcodes = [];
    this.rules[30].opcodes[0] = { type: 1, children: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] }; // ALT
    this.rules[30].opcodes[1] = { type: 5, min: 97, max: 122 }; // TRG
    this.rules[30].opcodes[2] = { type: 5, min: 65, max: 90 }; // TRG
    this.rules[30].opcodes[3] = { type: 5, min: 48, max: 57 }; // TRG
    this.rules[30].opcodes[4] = { type: 4, index: 31 }; // RNM(pct-encoded)
    this.rules[30].opcodes[5] = { type: 6, string: [33] }; // TBS
    this.rules[30].opcodes[6] = { type: 6, string: [36] }; // TBS
    this.rules[30].opcodes[7] = { type: 5, min: 38, max: 46 }; // TRG
    this.rules[30].opcodes[8] = { type: 5, min: 58, max: 59 }; // TRG
    this.rules[30].opcodes[9] = { type: 6, string: [61] }; // TBS
    this.rules[30].opcodes[10] = { type: 6, string: [64] }; // TBS
    this.rules[30].opcodes[11] = { type: 6, string: [95] }; // TBS
    this.rules[30].opcodes[12] = { type: 6, string: [126] }; // TBS

    /* pct-encoded */
    this.rules[31].opcodes = [];
    this.rules[31].opcodes[0] = { type: 2, children: [1, 2, 3] }; // CAT
    this.rules[31].opcodes[1] = { type: 6, string: [37] }; // TBS
    this.rules[31].opcodes[2] = { type: 4, index: 35 }; // RNM(HEXDIG)
    this.rules[31].opcodes[3] = { type: 4, index: 35 }; // RNM(HEXDIG)

    /* ALPHA */
    this.rules[32].opcodes = [];
    this.rules[32].opcodes[0] = { type: 1, children: [1, 2] }; // ALT
    this.rules[32].opcodes[1] = { type: 5, min: 65, max: 90 }; // TRG
    this.rules[32].opcodes[2] = { type: 5, min: 97, max: 122 }; // TRG

    /* LF */
    this.rules[33].opcodes = [];
    this.rules[33].opcodes[0] = { type: 6, string: [10] }; // TBS

    /* DIGIT */
    this.rules[34].opcodes = [];
    this.rules[34].opcodes[0] = { type: 5, min: 48, max: 57 }; // TRG

    /* HEXDIG */
    this.rules[35].opcodes = [];
    this.rules[35].opcodes[0] = { type: 1, children: [1, 2, 3] }; // ALT
    this.rules[35].opcodes[1] = { type: 5, min: 48, max: 57 }; // TRG
    this.rules[35].opcodes[2] = { type: 5, min: 65, max: 70 }; // TRG
    this.rules[35].opcodes[3] = { type: 5, min: 97, max: 102 }; // TRG

    // The `toString()` function will display the original grammar file(s) that produced these opcodes.
    this.toString = function toString() {
      let str = '';
      str += '; LDT 10/21/3023 \r\n';
      str += '; modified in several significant ways\r\n';
      str += '; 1) Literal strings are replaced with numbers and ranges (%d32 & %d32-126, etc.) when possible.\r\n';
      str += ';    TRB and especially TRG operators are much more efficient than TLS operators.\r\n';
      str +=
        '; 2) RFC 3986 IPv6address does not work because of APG\'s "first-success disambiguation" and "greedy" repetitions.\r\n';
      str +=
        ';    IPv6address redefined and validations moved to callback functions (semantic vs syntactic validation)\r\n';
      str +=
        ';    Redefinition requires negative look-ahead operators, https://en.wikipedia.org/wiki/Syntactic_predicate\r\n';
      str += ';    That is SABNF instead of simple ABNF.\r\n';
      str += '; 3) RFC 3986 IPv4address fails because of "first-success disambiguation".\r\n';
      str += ';    This could be fixed with rearrangement of the alternative terms. However, it would still not\r\n';
      str += ';    accept zero-padded (leading zeros) decimal octets.\r\n';
      str += ';    Therefore, IPv4address is also done with callback functions and semantic validation.\r\n';
      str += '; 4) The negative look-ahead operator is also needed in the definition of host to\r\n';
      str += ';    prevent failure with a reg-name that begins with an IPv4 address.\r\n';
      str += '; 5) NOTE: host = 1.1.1.256 is a valid host name even though it is an invalid IPv4address.\r\n';
      str += ';          The IPv4address alternative fails but the reg-name alternative succeeds.\r\n';
      str += '\r\n';
      str += '\r\n';
      str += '; ------------------------------------------------------------------------------\r\n';
      str += '; RFC 3986\r\n';
      str += '\r\n';
      str += 'URI           = scheme ":" hier-part [ "?" query ] [ "#" fragment ]\r\n';
      str += 'hier-part     = "//" authority path-abempty\r\n';
      str += '              / path-absolute\r\n';
      str += '              / path-rootless\r\n';
      str += '              / path-empty\r\n';
      str += 'scheme        = ALPHA *( ALPHA / DIGIT / %d43 / %d45-46 )\r\n';
      str += 'authority     = [ userinfo-at ] host [ ":" port ]\r\n';
      str += 'path-abempty  = *( "/" segment )\r\n';
      str += 'path-absolute = "/" [ segment-nz *( "/" segment ) ]\r\n';
      str += 'path-rootless = segment-nz *( "/" segment )\r\n';
      str += 'path-empty    = ""\r\n';
      str += 'userinfo-at   = userinfo %d64\r\n';
      str += '                ; userinfo redefined to include the "@" so that it will fail without it\r\n';
      str += '                ; otherwise userinfo can match host and then the parser will backtrack\r\n';
      str += '                ; incorrectly keeping the captured userinfo phrase\r\n';
      str +=
        'userinfo      = *(%d97-122 / %d65-90 / %d48-57 / pct-encoded / %d33 / %d36 / %d38-46 / %d58-59 / %d61 / %d95 / %d126)\r\n';
      str += 'host          = IP-literal / (IPv4address !reg-name-char) / reg-name\r\n';
      str +=
        '                ; negative look-ahead required to prevent IPv4address from being recognized as first part of reg-name\r\n';
      str += '                ; same fix as https://github.com/garycourt/uri-js/issues/4\r\n';
      str += 'IP-literal    = "[" ( IPv6address / IPvFuture  ) "]"\r\n';
      str +=
        'IPvFuture     = "v" 1*HEXDIG "." 1*( %d97-122 / %d65-90 / %d48-57 / %d33 / %d36 /%d38-46 / %d58-59 /%d61 /%d95 / %d126 )\r\n';
      str += 'IPv6address   = nodcolon / dcolon\r\n';
      str += 'nodcolon      = (h16n *h16cn) [%d58 IPv4address]\r\n';
      str += 'dcolon        = [h16 *h16c] %d58.58 (((h16n *h16cn) [%d58 IPv4address]) / [IPv4address])\r\n';
      str += 'h16           = 1*4HEXDIG\r\n';
      str += 'h16c          = %d58 1*4HEXDIG\r\n';
      str += 'h16n          = 1*4HEXDIG !%d46\r\n';
      str += 'h16cn         = %d58 1*4HEXDIG !%d46\r\n';
      str += 'IPv4address   = dec-octet "." dec-octet "." dec-octet "." dec-octet\r\n';
      str +=
        '; Here we will will use callback functions to evaluate and validate the (possibly zero-padded) dec-octet.\r\n';
      str += 'dec-octet     =  1*3dec-digit\r\n';
      str += 'dec-digit     = %d48-57\r\n';
      str += 'reg-name      = *reg-name-char\r\n';
      str +=
        'reg-name-char = %d97-122 / %d65-90 / %d48-57 / pct-encoded / %d33 / %d36 / %d38-46 / %d59 / %d61 /%d95 / %d126\r\n';
      str += 'port          = *DIGIT\r\n';
      str += 'query         = *(pchar / %d47 / %d63)\r\n';
      str += 'fragment      = *(pchar / %d47 / %d63)\r\n';
      str += '\r\n';
      str += 'segment       = *pchar\r\n';
      str += 'segment-nz    = 1*pchar\r\n';
      str +=
        'pchar         = (%d97-122 / %d65-90 / %d48-57 / pct-encoded / %d33 / %d36 / %d38-46 /%d58-59 / %d61 / %d64 / %d95 / %d126)\r\n';
      str += 'pct-encoded   = %d37 HEXDIG HEXDIG\r\n';
      str += '\r\n';
      str += '; no longer needed - expanded for all usage for fewer branches in the parse there\r\n';
      str += '; and more efficient use of the TBS & TRG operators in place of TLS and rule names\r\n';
      str += '; does not work with APG probably because of "first-success disambiguation" and greedy repetitions.\r\n';
      str += '; will replace with semantic checking of valid number of h16s\r\n';
      str += ';IPv6address   =                            6( h16 ":" ) ls32\r\n';
      str += ';              /                       "::" 5( h16 ":" ) ls32\r\n';
      str += ';              / [               h16 ] "::" 4( h16 ":" ) ls32\r\n';
      str += ';              / [ *1( h16 ":" ) h16 ] "::" 3( h16 ":" ) ls32\r\n';
      str += ';              / [ *2( h16 ":" ) h16 ] "::" 2( h16 ":" ) ls32\r\n';
      str += ';              / [ *3( h16 ":" ) h16 ] "::"    h16 ":"   ls32\r\n';
      str += ';              / [ *4( h16 ":" ) h16 ] "::"              ls32\r\n';
      str += ';              / [ *5( h16 ":" ) h16 ] "::"              h16\r\n';
      str += ';              / [ *6( h16 ":" ) h16 ] "::"\r\n';
      str += ';ls32          = ( h16 ":" h16 ) / IPv4address\r\n';
      str += '; dec-octet does not work because of "first-success disambiguation".\r\n';
      str += '; Must have the longest (3-digit) numbers first.\r\n';
      str += '; Even so, this form does not accept leading zeros.\r\n';
      str +=
        '; There does not seem to be a clear standard for this (https://en.wikipedia.org/wiki/Dot-decimal_notation)\r\n';
      str += '; however and early RFC 790 did show leading-zero padding of the three digits.\r\n';
      str += ';dec-octet     = DIGIT                 ; 0-9\r\n';
      str += ';                 / %x31-39 DIGIT         ; 10-99\r\n';
      str += ';                 / "1" 2DIGIT            ; 100-199\r\n';
      str += ';                 / "2" %x30-34 DIGIT     ; 200-249\r\n';
      str += ';                 / "25" %x30-35          ; 250-255\r\n';
      str += ';statement = 1*( reserved / unreserved / " " )\r\n';
      str += ';scheme        = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )\r\n';
      str += ';authority     = [ userinfo "@" ] host [ ":" port ]\r\n';
      str += ';userinfo      = *( unreserved / pct-encoded / sub-delims / ":" )\r\n';
      str += ';query         = *( pchar / "/" / "?" )\r\n';
      str += ';fragment      = *( pchar / "/" / "?" )\r\n';
      str += ';IPvFuture     = "v" 1*HEXDIG "." 1*( unreserved / sub-delims / ":" )\r\n';
      str += ';reg-name      = *( unreserved / pct-encoded / sub-delims )\r\n';
      str += ';pct-encoded   = "%" HEXDIG HEXDIG\r\n';
      str += ';pchar         = unreserved / pct-encoded / sub-delims / ":" / "@"\r\n';
      str += ';path-empty    = 0pchar; deprecated - empty literal string, "", is more efficient \r\n';
      str += ';unreserved    = ALPHA / DIGIT / "-" / "." / "_" / "~"\r\n';
      str += ';reserved      = gen-delims / sub-delims\r\n';
      str += ';gen-delims    = ":" / "/" / "?" / "#" / "[" / "]" / "@"\r\n';
      str += ';sub-delims    = "!" / "$" / "&" / "\'" / "(" / ")"\r\n';
      str += ';              / "*" / "+" / "," / ";" / "="\r\n';
      str += ';HEXDIG         =  DIGIT / "A" / "B" / "C" / "D" / "E" / "F"\r\n';
      str += '\r\n';
      str += '; ------------------------------------------------------------------------------\r\n';
      str += '; RFC 5234\r\n';
      str += '\r\n';
      str += 'ALPHA          =  %x41-5A / %x61-7A   ; A-Z / a-z\r\n';
      str += 'LF             =  %x0A\r\n';
      str += '                  ; linefeed\r\n';
      str += 'DIGIT          =  %x30-39\r\n';
      str += '                  ; 0-9\r\n';
      str += 'HEXDIG         = %d48-57 / %d65-70 / %d97-102\r\n';
      str += '\r\n';
      return str;
    };
  };
  /*****************************/
  /** the URI parser */
  /*****************************/
  const UriParser = function () {
    let apgResult = '';
    let userData = {};
    const parser = new Parser();
    const grammar = new UriGrammar();
    parser.callbacks['uri'] = cb.URI;
    parser.callbacks['scheme'] = cb.scheme;
    parser.callbacks['userinfo-at'] = cb.userinfo;
    parser.callbacks['host'] = cb.host;
    parser.callbacks['IP-literal'] = cb.ipLiteral;
    parser.callbacks['port'] = cb.port;
    parser.callbacks['path-abempty'] = cb.pathAbempty;
    parser.callbacks['path-absolute'] = cb.pathAbsolute;
    parser.callbacks['path-rootless'] = cb.pathRootless;
    parser.callbacks['path-empty'] = cb.pathEmpty;
    parser.callbacks['query'] = cb.query;
    parser.callbacks['fragment'] = cb.fragment;
    parser.callbacks['IPv4address'] = cb.ipv4;
    parser.callbacks['nodcolon'] = cb.nodcolon;
    parser.callbacks['dcolon'] = cb.dcolon;
    parser.callbacks['h16'] = cb.h16;
    parser.callbacks['h16c'] = cb.h16;
    parser.callbacks['h16n'] = cb.h16;
    parser.callbacks['h16cn'] = cb.h16;
    parser.callbacks['dec-octet'] = cb.decOctet;
    parser.callbacks['dec-digit'] = cb.decDigit;
    this.parse = (u, doTrace) => {
      if (doTrace) {
        parser.trace = new Trace();
      } else {
        parser.trace = undefined;
      }
      userData = {
        uriElements: {},
      };
      apgResult = parser.parse(grammar, 'uri', u, userData);
      if (apgResult.success) {
        // normal return
        return {
          uri: userData['uri'],
          scheme: userData.uriElements['scheme'],
          userinfo: userData.uriElements['userinfo'],
          host: userData.uriElements['host'],
          port: userData.uriElements['port'],
          path: userData.uriElements['path'],
          query: userData.uriElements['query'],
          fragment: userData.uriElements['fragment'],
        };
      }
      return undefined;
    };
    this.apgParserResult = () => apgResult;
    this.displayTrace = () => {
      if (parser.trace) {
        return parser.trace.displayTrace();
      }
      return undefined;
    };
  };

  // no reporting of specific errors
  // force parsing failure on semantic errors
  // user will need to trace(debug) the parse tree to find parsing errors
  const id = identifiers;
  const utils = utilities;
  const cb = {
    // handle the URI
    URI: function URI(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.ACTIVE:
          data.errorslength = 0;
          break;
        case id.MATCH:
          data.uri = utils.charsToString(chars, phraseIndex, result.phraseLength);
          break;
        case id.EMPTY:
          result.state = id.NOMATCH;
          result.phraseLength = 0;
          break;
      }
    },
    scheme: function scheme(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.uriElements.scheme = utils.charsToString(chars, phraseIndex, result.phraseLength);
          break;
      }
    },
    userinfo: function userinfo(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.uriElements.userinfo = utils.charsToString(chars, phraseIndex, result.phraseLength - 1);
          break;
      }
    },
    host: function host(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.ACTIVE:
          data.iplit = false;
          break;
        case id.MATCH:
          if (data.iplit) {
            // strip leading "[" and trailing "]" brackets
            data.uriElements.host = utils.charsToString(chars, phraseIndex + 1, result.phraseLength - 2);
          } else {
            data.uriElements.host = utils.charsToString(chars, phraseIndex, result.phraseLength);
          }
          break;
        case id.EMPTY:
          data.uriElements.host = '';
          break;
      }
    },
    ipLiteral: function ipLiteral(result, chars, phraseIndex, data) {
      if (result.state === id.MATCH) {
        data.iplit = true;
      }
    },
    port: function port(result, chars, phraseIndex, data) {
      let parsed = 0;
      let port = '';
      switch (result.state) {
        case id.MATCH:
          port = utils.charsToString(chars, phraseIndex, result.phraseLength);
          parsed = parseInt(port);
          if (Number.isNaN(parsed)) {
            result.state = id.NOMATCH;
            result.phraseLength = 0;
          } else {
            data.uriElements.port = parsed;
          }
          break;
        case id.EMPTY:
          data.uriElements.port = '';
          break;
      }
    },
    pathAbempty: function pathAbempty(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.uriElements.path = utils.charsToString(chars, phraseIndex, result.phraseLength);
          break;
        case id.EMPTY:
          data.uriElements.path = '';
          break;
      }
    },
    pathAbsolute: function pathAbsolute(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.uriElements.path = utils.charsToString(chars, phraseIndex, result.phraseLength);
          break;
      }
    },
    pathRootless: function pathRootless(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.uriElements.path = utils.charsToString(chars, phraseIndex, result.phraseLength);
          break;
      }
    },
    pathEmpty: function pathEmpty(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
        case id.NOMATCH:
          result.state = id.NOMATCH;
          result.phraseLength = 0;
        case id.EMPTY:
          data.uriElements.path = '';
          break;
      }
    },
    query: function query(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.uriElements.query = utils.charsToString(chars, phraseIndex, result.phraseLength);
          break;
        case id.EMPTY:
          data.uriElements.query = '';
          break;
      }
    },
    fragment: function fragment(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.uriElements.fragment = utils.charsToString(chars, phraseIndex, result.phraseLength);
          break;
        case id.EMPTY:
          data.uriElements.fragment = '';
          break;
      }
    },
    ipv4: function ipv4(result, chars, phraseIndex, data) {
      if (result.state === id.MATCH) {
        data.ipv4 = true;
      }
    },
    h16: function h16(result, chars, phraseIndex, data) {
      if (result.state === id.MATCH) {
        data.h16count += 1;
      }
    },
    nodcolon: function nodcolon(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.ACTIVE:
          data.h16count = 0;
          data.ipv4 = false;
          break;
        case id.MATCH:
          // semantically validate the number of 16-bit digits
          if (data.ipv4) {
            if (data.h16count === 6) {
              result.state = id.MATCH;
            } else {
              result.state = id.NOMATCH;
              result.phraseLength = 0;
            }
          } else {
            if (data.h16count === 8) {
              result.state = id.MATCH;
            } else {
              result.state = id.NOMATCH;
              result.phraseLength = 0;
            }
          }
          break;
      }
    },
    dcolon: function dcolon(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.ACTIVE:
          data.h16count = 0;
          data.ipv4 = false;
          break;
        case id.MATCH:
          // semantically validate the number of 16-bit digits
          if (data.ipv4) {
            if (data.h16count < 6) {
              result.state = id.MATCH;
            } else {
              result.state = id.NOMATCH;
              result.phraseLength = 0;
            }
          } else {
            if (data.h16count < 8) {
              result.state = id.MATCH;
            } else {
              result.state = id.NOMATCH;
              result.phraseLength = 0;
            }
          }
          break;
      }
    },
    decOctet: function decOctet(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.ACTIVE:
          data.octet = 0;
          break;
        case id.MATCH:
          // semantically validate the octet
          if (data.octet > 255) {
            result.state = id.NOMATCH;
            result.phraseLength = 0;
          }
          break;
      }
    },
    decDigit: function decDigit(result, chars, phraseIndex, data) {
      switch (result.state) {
        case id.MATCH:
          data.octet = 10 * data.octet + chars[phraseIndex] - 48;
          break;
      }
    },
  };
  return new UriParser();
})();
