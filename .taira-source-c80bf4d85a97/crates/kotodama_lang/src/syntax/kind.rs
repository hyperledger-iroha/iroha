//! Concrete syntax node and token kinds.

/// Kotodama concrete syntax kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntaxKind {
    /// Entire source file.
    Root,
    /// `seiyaku`/`誓約` or `module` source unit.
    SourceUnit,
    /// Source-unit item list.
    ItemList,
    /// Function or lifecycle declaration.
    FunctionItem,
    /// Structure declaration.
    StructItem,
    /// Stable error enumeration.
    ErrorEnumItem,
    /// Constant declaration.
    ConstItem,
    /// State declaration.
    StateItem,
    /// Trigger declaration.
    TriggerItem,
    /// Local test fixture declaration.
    FixtureItem,
    /// Standalone local test target declaration.
    TestTargetItem,
    /// Attribute attached to an item.
    Attribute,
    /// Function parameter list.
    ParamList,
    /// Source call argument list.
    ArgumentList,
    /// One `name: expression` call argument.
    NamedArgument,
    /// Named struct construction expression.
    StructLiteral,
    /// Bounded list literal expression.
    ListExpr,
    /// Capacity-proven bounded list comprehension expression.
    ListComprehension,
    /// Native JSON object construction expression.
    JsonObjectExpr,
    /// One native JSON object entry.
    JsonObjectEntry,
    /// Native JSON array construction expression.
    JsonArrayExpr,
    /// One shorthand or explicit struct literal field.
    StructLiteralField,
    /// Function body or nested block.
    Block,
    /// Statement list.
    StatementList,
    /// Immutable or mutable binding statement.
    LetStmt,
    /// Expression or assignment statement.
    ExprStmt,
    /// Return statement.
    ReturnStmt,
    /// Break statement.
    BreakStmt,
    /// Continue statement.
    ContinueStmt,
    /// Conditional statement.
    IfStmt,
    /// Expression-valued conditional.
    IfExpr,
    /// Expression-valued exhaustive match.
    MatchExpr,
    /// One match arm.
    MatchArm,
    /// Namespaced `Option` or `Result` pattern.
    SumPattern,
    /// Final expression in a block without a semicolon.
    TailExpr,
    /// Bounded loop statement.
    ForStmt,
    /// Tokens skipped during recovery.
    ErrorNode,

    /// Spaces, newlines, or other Unicode whitespace.
    Whitespace,
    /// `//` comment.
    LineComment,
    /// `/* ... */` comment.
    BlockComment,
    /// Identifier.
    Ident,
    /// Integer literal.
    Number,
    /// Exact base-10 decimal literal.
    Decimal,
    /// String or raw-string literal.
    String,
    /// Byte-string or raw-byte-string literal.
    Bytes,
    /// Invalid or budget-collapsed source text.
    ErrorToken,
    /// Zero-width recovery token.
    Missing,
    /// End of file.
    Eof,

    /// `fn`.
    KwFn,
    /// `let`.
    KwLet,
    /// `var`.
    KwVar,
    /// `const`.
    KwConst,
    /// `return`.
    KwReturn,
    /// `break`.
    KwBreak,
    /// `continue`.
    KwContinue,
    /// `state`.
    KwState,
    /// `struct`.
    KwStruct,
    /// `error`.
    KwError,
    /// `enum`.
    KwEnum,
    /// `authorize`.
    KwAuthorize,
    /// `trigger`.
    KwTrigger,
    /// `if`.
    KwIf,
    /// `match`.
    KwMatch,
    /// `else`.
    KwElse,
    /// `for`.
    KwFor,
    /// `in`.
    KwIn,
    /// `seiyaku` or `誓約`.
    KwSeiyaku,
    /// `module`.
    KwModule,
    /// `kotoage` or `言挙げ`.
    KwKotoage,
    /// `hajimari` or `始まり`.
    KwHajimari,
    /// `kaizen` or `改善`.
    KwKaizen,
    /// `view`.
    KwView,
    /// `true`.
    KwTrue,
    /// `false`.
    KwFalse,
    /// `+`.
    Plus,
    /// `+=`.
    PlusEqual,
    /// `-`.
    Minus,
    /// `-=`.
    MinusEqual,
    /// `->`.
    Arrow,
    /// `=>`.
    FatArrow,
    /// `*`.
    Star,
    /// `*=`.
    StarEqual,
    /// `/`.
    Slash,
    /// `/=`.
    SlashEqual,
    /// `%`.
    Percent,
    /// `%=`.
    PercentEqual,
    /// `!`.
    Bang,
    /// `!=`.
    BangEqual,
    /// `=`.
    Equal,
    /// `==`.
    EqualEqual,
    /// `<`.
    Less,
    /// `<=`.
    LessEqual,
    /// `>`.
    Greater,
    /// `>=`.
    GreaterEqual,
    /// `&&`.
    AndAnd,
    /// `||`.
    OrOr,
    /// `(`.
    LParen,
    /// `)`.
    RParen,
    /// `{`.
    LBrace,
    /// `}`.
    RBrace,
    /// `[`.
    LBracket,
    /// `]`.
    RBracket,
    /// `;`.
    Semicolon,
    /// `,`.
    Comma,
    /// `:`.
    Colon,
    /// `::`.
    ColonColon,
    /// `.`.
    Dot,
    /// `?`.
    Question,
    /// `#`.
    Hash,
}

impl SyntaxKind {
    /// Return whether the kind is source trivia.
    #[must_use]
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment
        )
    }

    /// Return whether the kind begins a source-unit item.
    #[must_use]
    pub const fn starts_item(self) -> bool {
        matches!(
            self,
            Self::KwFn
                | Self::KwKotoage
                | Self::KwView
                | Self::KwHajimari
                | Self::KwKaizen
                | Self::KwStruct
                | Self::KwError
                | Self::KwConst
                | Self::KwState
                | Self::KwTrigger
                | Self::Hash
        )
    }

    /// Return whether the kind is a unary prefix operator.
    #[must_use]
    pub const fn is_prefix_operator(self) -> bool {
        matches!(self, Self::Bang | Self::Minus | Self::Plus)
    }
}
