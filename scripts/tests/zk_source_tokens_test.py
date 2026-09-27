"""Regression tests for literal-preserving ZK source-contract fingerprints."""

import unittest

from zk_source_tokens import rust_tokens, token_hash


class RustSourceTokenTests(unittest.TestCase):
    def test_formatting_and_nested_comments_preserve_code(self) -> None:
        source = 'assert!(x == "// literal /* text */");'
        formatted = '/* outer /* inner */ */ assert ! (x == "// literal /* text */") ; // end'
        self.assertEqual(rust_tokens(source), rust_tokens(formatted))
        self.assertEqual(token_hash(source), token_hash(formatted))

    def test_literals_and_boundaries_are_preserved(self) -> None:
        source = r'''r##"a"#//b"## b"c\"d" '\n' b'x' 'a value != other'''
        self.assertEqual(
            rust_tokens(source),
            ('r##"a"#//b"##', r'b"c\"d"', r"'\n'", "b'x'", "'", "a", "value", "!=", "other"),
        )
        for old, new in [('"hello world"', '"helloworld"'), ("ab", "a b"), ("!=", "! =")]:
            with self.subTest(old=old):
                self.assertNotEqual(token_hash(old), token_hash(new))

    def test_unterminated_literals_and_comments_fail(self) -> None:
        for source in ('/* open', 'r#"open', '"open', 'b"open\\'):
            with self.subTest(source=source), self.assertRaises(AssertionError):
                rust_tokens(source)


if __name__ == "__main__":
    unittest.main()
