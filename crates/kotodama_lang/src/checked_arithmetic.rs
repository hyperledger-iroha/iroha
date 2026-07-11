//! Bytecode sequences implementing checked source-level `i64` arithmetic.
//!
//! IVM scalar arithmetic intentionally exposes deterministic two's-complement
//! wrapping primitives. Kotodama's ordinary arithmetic is safer: the compiler
//! emits one of these sequences and calls `ABORT` when the wrapped result is
//! not representable as an `i64`. Explicit `wrapping_*` builtins bypass this
//! module and lower to the single underlying primitive.

use iroha_primitives::{AmountError, Numeric};
use ivm_abi::{encoding, instruction, syscalls};

use crate::{
    ast::{BinaryOp, UnaryOp},
    semantic::{ExprKind, TypedExpr},
};

/// First compiler-reserved register used by overflow checks.
pub(crate) const OVERFLOW_SCRATCH_A: u8 = 25;
/// Second compiler-reserved register used by overflow checks.
pub(crate) const OVERFLOW_SCRATCH_B: u8 = 26;

/// Checked binary arithmetic operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckedBinaryOp {
    /// Signed `i64` addition.
    Add,
    /// Signed `i64` subtraction.
    Sub,
    /// Signed `i64` multiplication.
    Mul,
}

/// A compile-time `i64` overflow that must not be folded to a wrapped value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConstantOverflow {
    /// Overflow from unary negation.
    Neg(i64),
    /// Overflow from a binary operation.
    Binary {
        /// Operation that overflowed.
        operation: CheckedBinaryOp,
        /// Constant left operand.
        left: i64,
        /// Constant right operand.
        right: i64,
    },
}

impl core::fmt::Display for ConstantOverflow {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Neg(value) => write!(
                formatter,
                "E_INT_OVERFLOW: negating {value} is outside the i64 range"
            ),
            Self::Binary {
                operation,
                left,
                right,
            } => {
                let symbol = match operation {
                    CheckedBinaryOp::Add => '+',
                    CheckedBinaryOp::Sub => '-',
                    CheckedBinaryOp::Mul => '*',
                };
                write!(
                    formatter,
                    "E_INT_OVERFLOW: {left} {symbol} {right} is outside the i64 range"
                )
            }
        }
    }
}

/// Evaluate a source expression when it is composed entirely of checked `i64`
/// literals. `Ok(None)` means the expression is not a foldable constant.
pub(crate) fn evaluate_checked_i64(
    expression: &TypedExpr,
) -> Result<Option<i64>, ConstantOverflow> {
    match &expression.expr {
        ExprKind::Number(value) => Ok(Some(*value)),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => {
            let Some(value) = evaluate_checked_i64(expr)? else {
                return Ok(None);
            };
            value
                .checked_neg()
                .map(Some)
                .ok_or(ConstantOverflow::Neg(value))
        }
        ExprKind::Binary {
            op: operation,
            left,
            right,
        } => {
            let operation = match operation {
                BinaryOp::Add => CheckedBinaryOp::Add,
                BinaryOp::Sub => CheckedBinaryOp::Sub,
                BinaryOp::Mul => CheckedBinaryOp::Mul,
                _ => return Ok(None),
            };
            let Some(left) = evaluate_checked_i64(left)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_checked_i64(right)? else {
                return Ok(None);
            };
            let result = match operation {
                CheckedBinaryOp::Add => left.checked_add(right),
                CheckedBinaryOp::Sub => left.checked_sub(right),
                CheckedBinaryOp::Mul => left.checked_mul(right),
            };
            result.map(Some).ok_or(ConstantOverflow::Binary {
                operation,
                left,
                right,
            })
        }
        _ => Ok(None),
    }
}

/// Invalid constant arithmetic over canonical nonnegative `Amount` values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ConstantAmountError {
    operation: BinaryOp,
    source: AmountError,
}

impl ConstantAmountError {
    /// Render the human detail without embedding its stable diagnostic code.
    pub(crate) fn message(self) -> String {
        let symbol = match self.operation {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            _ => "?",
        };
        format!("constant Amount `{symbol}` failed: {}", self.source)
    }
}

impl core::fmt::Display for ConstantAmountError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "E_AMOUNT_CONSTANT_ARITHMETIC: {}",
            (*self).message()
        )
    }
}

/// Evaluate a source expression composed entirely of `Amount` literals and
/// exact Amount operators. `Ok(None)` means the expression is dynamic.
pub(crate) fn evaluate_checked_amount(
    expression: &TypedExpr,
) -> Result<Option<Numeric>, ConstantAmountError> {
    match &expression.expr {
        ExprKind::AmountLiteral { value, .. } => Ok(Some(value.clone())),
        ExprKind::Binary {
            op: operation,
            left,
            right,
        } if matches!(
            operation,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
        ) =>
        {
            let Some(left) = evaluate_checked_amount(left)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_checked_amount(right)? else {
                return Ok(None);
            };
            let result = match operation {
                BinaryOp::Add => left.checked_amount_add(&right),
                BinaryOp::Sub => left.checked_amount_sub(&right),
                BinaryOp::Mul => left.checked_amount_mul(&right),
                BinaryOp::Div => left.checked_amount_div_exact(&right),
                _ => unreachable!("guard admits exact Amount arithmetic only"),
            };
            result.map(Some).map_err(|source| ConstantAmountError {
                operation: *operation,
                source,
            })
        }
        _ => Ok(None),
    }
}

fn abort_word() -> u32 {
    encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        u8::try_from(syscalls::SYSCALL_ABORT).expect("ABORT syscall fits in SCALL"),
    )
}

fn skip_abort_if_non_negative(register: u8) -> u32 {
    // Branch offsets are relative to the branch word. Two words lands after
    // the immediately following ABORT instruction.
    encoding::wide::encode_branch(instruction::wide::control::BGE, register, 0, 2)
}

fn skip_abort_if_equal(left: u8, right: u8) -> u32 {
    encoding::wide::encode_branch(instruction::wide::control::BEQ, left, right, 2)
}

/// Encode checked `i64` addition, subtraction, or multiplication.
pub(crate) fn encode_checked_binary(
    operation: CheckedBinaryOp,
    rd: u8,
    rs1: u8,
    rs2: u8,
) -> [u32; 6] {
    use instruction::wide::arithmetic as op;

    match operation {
        CheckedBinaryOp::Add => [
            encoding::wide::encode_rr(op::ADD, rd, rs1, rs2),
            // Overflow iff ((lhs ^ result) & (rhs ^ result)) is negative.
            encoding::wide::encode_rr(op::XOR, OVERFLOW_SCRATCH_A, rs1, rd),
            encoding::wide::encode_rr(op::XOR, OVERFLOW_SCRATCH_B, rs2, rd),
            encoding::wide::encode_rr(
                op::AND,
                OVERFLOW_SCRATCH_A,
                OVERFLOW_SCRATCH_A,
                OVERFLOW_SCRATCH_B,
            ),
            skip_abort_if_non_negative(OVERFLOW_SCRATCH_A),
            abort_word(),
        ],
        CheckedBinaryOp::Sub => [
            encoding::wide::encode_rr(op::SUB, rd, rs1, rs2),
            // Overflow iff ((lhs ^ rhs) & (lhs ^ result)) is negative.
            encoding::wide::encode_rr(op::XOR, OVERFLOW_SCRATCH_A, rs1, rs2),
            encoding::wide::encode_rr(op::XOR, OVERFLOW_SCRATCH_B, rs1, rd),
            encoding::wide::encode_rr(
                op::AND,
                OVERFLOW_SCRATCH_A,
                OVERFLOW_SCRATCH_A,
                OVERFLOW_SCRATCH_B,
            ),
            skip_abort_if_non_negative(OVERFLOW_SCRATCH_A),
            abort_word(),
        ],
        CheckedBinaryOp::Mul => [
            encoding::wide::encode_rr(op::MUL, rd, rs1, rs2),
            // A signed product fits exactly when its high word equals the sign
            // extension of its low word.
            encoding::wide::encode_rr(op::MULH, OVERFLOW_SCRATCH_A, rs1, rs2),
            encoding::wide::encode_ri(op::ADDI, OVERFLOW_SCRATCH_B, 0, 63),
            encoding::wide::encode_rr(op::SRA, OVERFLOW_SCRATCH_B, rd, OVERFLOW_SCRATCH_B),
            skip_abort_if_equal(OVERFLOW_SCRATCH_A, OVERFLOW_SCRATCH_B),
            abort_word(),
        ],
    }
}

/// Encode checked `i64` negation.
pub(crate) fn encode_checked_neg(rd: u8, rs: u8) -> [u32; 6] {
    use instruction::wide::arithmetic as op;

    [
        encoding::wide::encode_rr(op::NEG, rd, rs, 0),
        // Only zero and i64::MIN are their own two's-complement negation.
        // Combining equality with non-zero therefore isolates i64::MIN.
        encoding::wide::encode_rr(op::SNE, OVERFLOW_SCRATCH_A, rs, 0),
        encoding::wide::encode_rr(op::SEQ, OVERFLOW_SCRATCH_B, rd, rs),
        encoding::wide::encode_rr(
            op::AND,
            OVERFLOW_SCRATCH_A,
            OVERFLOW_SCRATCH_A,
            OVERFLOW_SCRATCH_B,
        ),
        encoding::wide::encode_branch(instruction::wide::control::BEQ, OVERFLOW_SCRATCH_A, 0, 2),
        abort_word(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::Type;

    fn number(value: i64) -> TypedExpr {
        TypedExpr {
            expr: ExprKind::Number(value),
            ty: Type::Int,
        }
    }

    fn binary(operation: BinaryOp, left: i64, right: i64) -> TypedExpr {
        TypedExpr {
            expr: ExprKind::Binary {
                op: operation,
                left: Box::new(number(left)),
                right: Box::new(number(right)),
            },
            ty: Type::Int,
        }
    }

    #[test]
    fn checked_sequences_end_in_a_skippable_abort() {
        for words in [
            encode_checked_binary(CheckedBinaryOp::Add, 2, 3, 4),
            encode_checked_binary(CheckedBinaryOp::Sub, 2, 3, 4),
            encode_checked_binary(CheckedBinaryOp::Mul, 2, 3, 4),
            encode_checked_neg(2, 3),
        ] {
            let branch = words[4];
            assert_eq!(instruction::wide::imm8(branch), 2);
            assert_eq!(
                encoding::wide::decode_sys(words[5]),
                (
                    instruction::wide::system::SCALL,
                    u8::try_from(syscalls::SYSCALL_ABORT).unwrap()
                )
            );
        }
    }

    #[test]
    fn constant_evaluation_never_wraps() {
        assert_eq!(
            evaluate_checked_i64(&binary(BinaryOp::Add, i64::MAX - 1, 1)),
            Ok(Some(i64::MAX))
        );
        assert_eq!(
            evaluate_checked_i64(&binary(BinaryOp::Add, i64::MAX, 1)),
            Err(ConstantOverflow::Binary {
                operation: CheckedBinaryOp::Add,
                left: i64::MAX,
                right: 1,
            })
        );
        let neg_min = TypedExpr {
            expr: ExprKind::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(number(i64::MIN)),
            },
            ty: Type::Int,
        };
        assert_eq!(
            evaluate_checked_i64(&neg_min),
            Err(ConstantOverflow::Neg(i64::MIN))
        );
    }

    fn amount(value: &str) -> TypedExpr {
        let value = value
            .parse::<Numeric>()
            .expect("numeric literal")
            .canonicalize_amount()
            .expect("Amount literal");
        TypedExpr {
            expr: ExprKind::AmountLiteral {
                spelling: format!("{value}amt"),
                value,
            },
            ty: Type::Amount,
        }
    }

    fn amount_binary(operation: BinaryOp, left: &str, right: &str) -> TypedExpr {
        TypedExpr {
            expr: ExprKind::Binary {
                op: operation,
                left: Box::new(amount(left)),
                right: Box::new(amount(right)),
            },
            ty: Type::Amount,
        }
    }

    #[test]
    fn constant_amount_evaluation_is_exact_and_fail_closed() {
        assert_eq!(
            evaluate_checked_amount(&amount_binary(BinaryOp::Div, "1", "8")),
            Ok(Some(Numeric::new(125, 3)))
        );
        assert!(matches!(
            evaluate_checked_amount(&amount_binary(BinaryOp::Sub, "1", "2")),
            Err(ConstantAmountError {
                source: AmountError::Underflow,
                ..
            })
        ));
        assert!(matches!(
            evaluate_checked_amount(&amount_binary(BinaryOp::Div, "1", "3")),
            Err(ConstantAmountError {
                source: AmountError::InexactDivision,
                ..
            })
        ));
    }
}
