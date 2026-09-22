"""Installed-package authority joined to the shared retained physical tree owner.

No extraction, npm command, SDK import or native load occurs. The exact original
package/dependency projection remains the sole installed-content authority.
"""
from __future__ import annotations

from pathlib import Path

from sorafs_javascript_installed import InstalledProjection, validate_projection
from sorafs_javascript_tree_custody import OriginalTree, TreeMember


class OriginalInstalledTree(OriginalTree):
    """Keep the exact admitted node_modules tree and original ancestor handles.

    This wrapper validates the complete original package relation before passing
    immutable physical expectations to the shared owner. The root is node_modules,
    not the qualification or consumer environment directory.
    """
    def __init__(self, root: Path, projection: InstalledProjection):
        validate_projection(projection)
        super().__init__(root, tuple(TreeMember(row.path, row.content, row.mode)
                                    for row in projection.members))
