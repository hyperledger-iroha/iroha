"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Codegen = exports.CodegenStepExecJs = void 0;
const compile_1 = require("./compile");
/**
 * Inline JavaScript statements that are executed in main function body.
 */
class CodegenStepExecJs {
    constructor(js) {
        this.js = js;
    }
}
exports.CodegenStepExecJs = CodegenStepExecJs;
/**
 * A helper class which helps with building JavaScript code for a single
 * function. It keeps track of external dependencies, internally generated
 * constants, and execution steps, which at the end are all converted to
 * to an executable JavaScript function.
 *
 * The final output is a JavaScript function enclosed in a closure:
 *
 * ```js
 * (function(d1, d2, d3) {
 *   var c1 = something;
 *   var c2 = something;
 *   var c3 = something;
 *   return function(r0) {
 *     var r1 = something;
 *     var r2 = something;
 *     var r3 = something;
 *     return something;
 *   }
 * })
 * ```
 *
 * Where `d*` are the external dependencies, `c*` are the internal constants,
 * and `r*` are the local immutable infinite registers.
 */
class Codegen {
    constructor(opts) {
        /** @ignore */
        this.steps = [];
        /** @ignore */
        this.dependencies = [];
        this.dependencyNames = [];
        this.linked = {};
        /** @ignore */
        this.constants = [];
        this.constantNames = [];
        this.options = {
            args: ['r0'],
            name: '',
            prologue: '',
            epilogue: '',
            processSteps: (steps) => steps.filter((step) => step instanceof CodegenStepExecJs),
            linkable: {},
            ...opts,
        };
        this.registerCounter = this.options.args.length;
    }
    /**
     * Add one or more JavaScript statements to the main function body.
     */
    js(js) {
        this.steps.push(new CodegenStepExecJs(js));
    }
    var(expression) {
        const r = this.getRegister();
        if (expression)
            this.js('var ' + r + ' = ' + expression + ';');
        else
            this.js('var ' + r + ';');
        return r;
    }
    if(condition, then, otherwise) {
        this.js('if (' + condition + ') {');
        then();
        if (otherwise) {
            this.js('} else {');
            otherwise();
        }
        this.js('}');
    }
    while(condition, block) {
        this.js('while (' + condition + ') {');
        block();
        this.js('}');
    }
    doWhile(block, condition) {
        this.js('do {');
        block();
        this.js('} while (' + condition + ');');
    }
    switch(expression, cases, def) {
        this.js('switch (' + expression + ') {');
        for (const [match, block, noBreak] of cases) {
            this.js('case ' + match + ': {');
            block();
            if (!noBreak)
                this.js('break;');
            this.js('}');
        }
        if (def) {
            this.js('default: {');
            def();
            this.js('}');
        }
        this.js('}');
    }
    return(expression) {
        this.js('return ' + expression + ';');
    }
    /**
     * Add any application specific execution step. Steps of `unknown` type
     * later need to converted to `CodegenStepExecJs` steps in the `.processStep`
     * callback.
     *
     * @param step A step in function execution logic.
     */
    step(step) {
        this.steps.push(step);
    }
    /**
     * Codegen uses the idea of infinite registers. It starts with `0` and
     * increments it by one for each new register. Best practice is to use
     * a new register for each new variable and keep them immutable.
     *
     * Usage:
     *
     * ```js
     * const r = codegen.getRegister();
     * codegen.js(`const ${r} = 1;`);
     * ```
     *
     * @returns a unique identifier for a variable.
     */
    getRegister() {
        return `r${this.registerCounter++}`;
    }
    r() {
        return this.getRegister();
    }
    /**
     * Allows to wire up dependencies to the generated code.
     *
     * @param dep Any JavaScript dependency, could be a function, an object,
     *        or anything else.
     * @param name Optional name of the dependency. If not provided, a unique
     *        name will be generated, which starts with `d` and a counter
     *        appended.
     * @returns Returns the dependency name, a code symbol which can be used as
     *          variable name.
     */
    linkDependency(dep, name = 'd' + this.dependencies.length) {
        this.dependencies.push(dep);
        this.dependencyNames.push(name);
        return name;
    }
    /**
     * Sames as {@link Codegen#linkDependency}, but allows to wire up multiple
     * dependencies at once.
     */
    linkDependencies(deps) {
        return deps.map((dep) => this.linkDependency(dep));
    }
    /**
     * Link a dependency from the pre-defined `options.linkable` object. This method
     * can be called many times with the same dependency name, the dependency will
     * be linked only once.
     *
     * @param name Linkable dependency name.
     */
    link(name) {
        if (this.linked[name])
            return;
        this.linked[name] = 1;
        this.linkDependency(this.options.linkable[name], name);
    }
    /**
     * Allows to encode any code or value in the closure of the generated
     * function.
     *
     * @param constant Any JavaScript value in string form.
     * @param name Optional name of the constant. If not provided, a unique
     *        name will be generated, which starts with `c` and a counter
     *        appended.
     * @returns Returns the constant name, a code symbol which can be used as
     *          variable name.
     */
    addConstant(constant, name = 'c' + this.constants.length) {
        this.constants.push(constant);
        this.constantNames.push(name);
        return name;
    }
    /**
     * Sames as {@link Codegen#addConstant}, but allows to create multiple
     * constants at once.
     */
    addConstants(constants) {
        return constants.map((constant) => this.addConstant(constant));
    }
    /**
     * Returns generated JavaScript code with the dependency list.
     *
     * ```js
     * const code = codegen.generate();
     * const fn = eval(code.js)(...code.deps);
     * const result = fn(...args);
     * ```
     */
    generate(opts = {}) {
        const { name, args, prologue, epilogue } = { ...this.options, ...opts };
        const steps = this.options.processSteps(this.steps);
        const js = `(function(${this.dependencyNames.join(', ')}) {
${this.constants.map((constant, index) => `var ${this.constantNames[index]} = (${constant});`).join('\n')}
return ${name ? `function ${name}` : 'function'}(${args.join(',')}){
${prologue}
${steps.map((step) => step.js).join('\n')}
${typeof epilogue === 'function' ? epilogue() : epilogue || ''}
}})`;
        // console.log(js);
        return {
            deps: this.dependencies,
            js: js,
        };
    }
    /**
     * Compiles the generated JavaScript code into a function.
     *
     * @returns JavaScript function ready for execution.
     */
    compile(opts) {
        const closure = this.generate(opts);
        return (0, compile_1.compileClosure)(closure);
    }
}
exports.Codegen = Codegen;
//# sourceMappingURL=Codegen.js.map