// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Support for generating R1CS shape using bellperson.

use crate::traits::Engine;
use bellpepper_core::{ConstraintSystem, Index, LinearCombination, SynthesisError, Variable};
use ff::{Field, PrimeField};

#[cfg(debug_assertions)]
use std::collections::HashMap;

#[cfg(debug_assertions)]
#[derive(Debug)]
enum NamedObject {
    Constraint,
    Var,
    Namespace,
}

/// `ShapeCS` is a `ConstraintSystem` for creating `R1CSShape`s for a circuit.
///
/// In release mode, optimized for speed: skips all naming, path computation,
/// and uniqueness checking. In debug mode, retains full name tracking and
/// uniqueness assertions to catch circuit design bugs.
pub struct ShapeCS<E: Engine>
where
    E::Scalar: PrimeField + Field,
{
    #[cfg(debug_assertions)]
    named_objects: HashMap<String, NamedObject>,
    #[cfg(debug_assertions)]
    current_namespace: Vec<String>,
    #[cfg(not(debug_assertions))]
    namespace_depth: usize,
    /// All constraints added to the `ShapeCS`.
    pub constraints: Vec<(
        LinearCombination<E::Scalar>,
        LinearCombination<E::Scalar>,
        LinearCombination<E::Scalar>,
    )>,
    #[cfg(debug_assertions)]
    inputs: Vec<String>,
    #[cfg(debug_assertions)]
    aux: Vec<String>,
    #[cfg(not(debug_assertions))]
    num_inputs: usize,
    #[cfg(not(debug_assertions))]
    num_aux: usize,
}

impl<E: Engine> ShapeCS<E>
where
    E::Scalar: PrimeField,
{
    /// Create a new, default `ShapeCS`,
    pub fn new() -> Self {
        ShapeCS::default()
    }

    /// Returns the number of constraints defined for this `ShapeCS`.
    pub fn num_constraints(&self) -> usize {
        self.constraints.len()
    }

    /// Returns the number of inputs defined for this `ShapeCS`.
    pub fn num_inputs(&self) -> usize {
        #[cfg(debug_assertions)]
        {
            self.inputs.len()
        }
        #[cfg(not(debug_assertions))]
        {
            self.num_inputs
        }
    }

    /// Returns the number of aux inputs defined for this `ShapeCS`.
    pub fn num_aux(&self) -> usize {
        #[cfg(debug_assertions)]
        {
            self.aux.len()
        }
        #[cfg(not(debug_assertions))]
        {
            self.num_aux
        }
    }

    /// Associate `NamedObject` with `path`.
    /// `path` must not already have an associated object.
    #[cfg(debug_assertions)]
    fn set_named_obj(&mut self, path: String, to: NamedObject) {
        assert!(
            !self.named_objects.contains_key(&path),
            "tried to create object at existing path: {path}"
        );

        self.named_objects.insert(path, to);
    }
}

impl<E: Engine> Default for ShapeCS<E>
where
    E::Scalar: PrimeField,
{
    fn default() -> Self {
        ShapeCS {
            #[cfg(debug_assertions)]
            named_objects: {
                let mut map = HashMap::new();
                map.insert("ONE".into(), NamedObject::Var);
                map
            },
            #[cfg(debug_assertions)]
            current_namespace: vec![],
            #[cfg(not(debug_assertions))]
            namespace_depth: 0,
            constraints: vec![],
            #[cfg(debug_assertions)]
            inputs: vec![String::from("ONE")],
            #[cfg(debug_assertions)]
            aux: vec![],
            #[cfg(not(debug_assertions))]
            num_inputs: 1,
            #[cfg(not(debug_assertions))]
            num_aux: 0,
        }
    }
}

impl<E: Engine> ConstraintSystem<E::Scalar> for ShapeCS<E>
where
    E::Scalar: PrimeField,
{
    type Root = Self;

    fn alloc<F, A, AR>(&mut self, _annotation: A, _f: F) -> Result<Variable, SynthesisError>
    where
        F: FnOnce() -> Result<E::Scalar, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        #[cfg(debug_assertions)]
        {
            let path = compute_path(&self.current_namespace, &_annotation().into());
            self.aux.push(path);
            Ok(Variable::new_unchecked(Index::Aux(self.aux.len() - 1)))
        }
        #[cfg(not(debug_assertions))]
        {
            let idx = self.num_aux;
            self.num_aux += 1;
            Ok(Variable::new_unchecked(Index::Aux(idx)))
        }
    }

    fn alloc_input<F, A, AR>(&mut self, _annotation: A, _f: F) -> Result<Variable, SynthesisError>
    where
        F: FnOnce() -> Result<E::Scalar, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        #[cfg(debug_assertions)]
        {
            let path = compute_path(&self.current_namespace, &_annotation().into());
            self.inputs.push(path);
            Ok(Variable::new_unchecked(Index::Input(self.inputs.len() - 1)))
        }
        #[cfg(not(debug_assertions))]
        {
            let idx = self.num_inputs;
            self.num_inputs += 1;
            Ok(Variable::new_unchecked(Index::Input(idx)))
        }
    }

    fn enforce<A, AR, LA, LB, LC>(&mut self, _annotation: A, a: LA, b: LB, c: LC)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
        LA: FnOnce(LinearCombination<E::Scalar>) -> LinearCombination<E::Scalar>,
        LB: FnOnce(LinearCombination<E::Scalar>) -> LinearCombination<E::Scalar>,
        LC: FnOnce(LinearCombination<E::Scalar>) -> LinearCombination<E::Scalar>,
    {
        #[cfg(debug_assertions)]
        {
            let path = compute_path(&self.current_namespace, &_annotation().into());
            self.set_named_obj(path, NamedObject::Constraint);
        }

        let a = a(LinearCombination::zero());
        let b = b(LinearCombination::zero());
        let c = c(LinearCombination::zero());
        self.constraints.push((a, b, c));
    }

    fn push_namespace<NR, N>(&mut self, _name_fn: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
        #[cfg(debug_assertions)]
        {
            let name = _name_fn().into();
            let path = compute_path(&self.current_namespace, &name);
            self.set_named_obj(path, NamedObject::Namespace);
            self.current_namespace.push(name);
        }
        #[cfg(not(debug_assertions))]
        {
            self.namespace_depth += 1;
        }
    }

    fn pop_namespace(&mut self) {
        #[cfg(debug_assertions)]
        {
            assert!(self.current_namespace.pop().is_some());
        }
        #[cfg(not(debug_assertions))]
        {
            assert!(self.namespace_depth > 0);
            self.namespace_depth -= 1;
        }
    }

    fn get_root(&mut self) -> &mut Self::Root {
        self
    }
}

#[cfg(debug_assertions)]
fn compute_path(ns: &[String], this: &str) -> String {
    assert!(
        !this.chars().any(|a| a == '/'),
        "'/' is not allowed in names"
    );

    let mut name = ns.join("/");
    if !name.is_empty() {
        name.push('/');
    }

    name.push_str(this);

    name
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    #[cfg(debug_assertions)]
    fn test_compute_path() {
        let ns = vec!["path".to_string(), "to".to_string(), "dir".to_string()];
        let this = "file";
        assert_eq!(compute_path(&ns, this), "path/to/dir/file");

        let ns = vec!["".to_string(), "".to_string(), "".to_string()];
        let this = "file";
        assert_eq!(compute_path(&ns, this), "///file");
    }

    #[test]
    #[should_panic(expected = "'/' is not allowed in names")]
    #[cfg(debug_assertions)]
    fn test_compute_path_invalid() {
        let ns = vec!["path".to_string(), "to".to_string(), "dir".to_string()];
        let this = "fi/le";
        compute_path(&ns, this);
    }
}
