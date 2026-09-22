"""Retain the exact fixed JavaScript qualification source/fixture core.

This is not an installed-package or runner owner. Parent candidate/input
custody is independent. No suite or native code is imported or executed here.
"""
from __future__ import annotations

from pathlib import Path

from sorafs_javascript_qualification_source import (
    QualificationProjection, validate_qualification_projection,
)
from sorafs_javascript_tree_custody import OriginalTree, TreeMember


class OriginalQualificationTree(OriginalTree):
    """Hold the private core root/ancestors and recheck every original member.

    The root contains exactly the 191 fixed suite/fixture inputs. The future
    runner/tools live outside this closed core and need their own exact join.
    """
    def __init__(self, root: Path, projection: QualificationProjection):
        validate_qualification_projection(projection)
        super().__init__(root, tuple(TreeMember(row.path, row.content, 0o644)
                                    for row in projection.members))
