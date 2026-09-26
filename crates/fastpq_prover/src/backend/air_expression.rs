//! Shared symbolic arithmetic graph for the exact compact hash and SMT equations.
//!
//! This is the existing hash compiler's polynomial expression owner. Numeric
//! evaluation and degree interpretation consume the same interned operations.
//! Degree bounds themselves never implement a field trait.

use std::{cell::RefCell, collections::BTreeMap};

use super::{add_mod, mul_mod, sub_mod};
use crate::gadgets::transfer_integer_air::IntegerAirField;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Node {
    Constant(u64),
    Input(usize),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
}

impl Node {
    pub(super) fn is_arithmetic(self) -> bool {
        matches!(self, Self::Add(..) | Self::Sub(..) | Self::Mul(..))
    }
}

#[derive(Default)]
pub(super) struct Builder {
    pub(super) nodes: Vec<Node>,
    pub(super) degrees: Vec<usize>,
    interned: BTreeMap<Node, usize>,
}

impl Builder {
    pub(super) fn intern(&mut self, node: Node) -> usize {
        if let Some(&index) = self.interned.get(&node) {
            return index;
        }
        let degree = match node {
            Node::Constant(_) => 0,
            Node::Input(_) => 1,
            Node::Add(left, right) | Node::Sub(left, right) => {
                self.degrees[left].max(self.degrees[right])
            }
            Node::Mul(left, right) => self.degrees[left] + self.degrees[right],
        };
        let index = self.nodes.len();
        self.nodes.push(node);
        self.degrees.push(degree);
        self.interned.insert(node, index);
        index
    }
}

/// Copy handles borrow one local compilation arena; constants need no arena.
#[derive(Clone, Copy)]
pub(super) enum Expression<'a> {
    Constant(u64),
    Node(&'a RefCell<Builder>, usize),
}

#[derive(Clone, Copy)]
enum Operation {
    Add,
    Sub,
    Mul,
}

impl<'a> Expression<'a> {
    pub(super) fn id(self, arena: &'a RefCell<Builder>) -> usize {
        match self {
            Self::Constant(value) => arena.borrow_mut().intern(Node::Constant(value)),
            Self::Node(owner, index) => {
                assert!(core::ptr::eq(owner, arena), "one fixed compilation arena");
                index
            }
        }
    }

    fn binary(self, other: Self, operation: Operation) -> Self {
        if let (Self::Constant(left), Self::Constant(right)) = (self, other) {
            return Self::Constant(match operation {
                Operation::Add => add_mod(left, right),
                Operation::Sub => sub_mod(left, right),
                Operation::Mul => mul_mod(left, right),
            });
        }
        match (operation, self, other) {
            (Operation::Add | Operation::Sub, _, Self::Constant(0))
            | (Operation::Mul, _, Self::Constant(1)) => return self,
            (Operation::Add, Self::Constant(0), _) | (Operation::Mul, Self::Constant(1), _) => {
                return other;
            }
            (Operation::Mul, Self::Constant(0), _) | (Operation::Mul, _, Self::Constant(0)) => {
                return Self::ZERO;
            }
            (Operation::Sub, Self::Node(left_arena, left), Self::Node(right_arena, right))
                if core::ptr::eq(left_arena, right_arena) && left == right =>
            {
                return Self::ZERO;
            }
            _ => {}
        }
        let arena = match (self, other) {
            (Self::Node(arena, _), _) | (_, Self::Node(arena, _)) => arena,
            _ => unreachable!("constant arithmetic handled above"),
        };
        let mut left = self.id(arena);
        let mut right = other.id(arena);
        if matches!(operation, Operation::Add | Operation::Mul) && right < left {
            core::mem::swap(&mut left, &mut right);
        }
        let node = match operation {
            Operation::Add => Node::Add(left, right),
            Operation::Sub => Node::Sub(left, right),
            Operation::Mul => Node::Mul(left, right),
        };
        let index = arena.borrow_mut().intern(node);
        Self::Node(arena, index)
    }
}

impl IntegerAirField for Expression<'_> {
    const ZERO: Self = Self::Constant(0);
    const ONE: Self = Self::Constant(1);

    fn from_u32(value: u32) -> Self {
        Self::Constant(u64::from(value))
    }

    fn add(self, other: Self) -> Self {
        self.binary(other, Operation::Add)
    }

    fn sub(self, other: Self) -> Self {
        self.binary(other, Operation::Sub)
    }

    fn mul(self, other: Self) -> Self {
        self.binary(other, Operation::Mul)
    }
}
