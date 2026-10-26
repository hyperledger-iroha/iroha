"""Resolve this acceptance runtime's checkout without host-specific paths."""
from pathlib import Path


def repository_root(source):
    """Select the source's enclosing Git Cargo checkout, never an ambient path."""
    source = Path(source).resolve(strict=True)
    for parent in source.parents:
        if (parent / 'Cargo.toml').is_file() and (parent / '.git').exists():
            return parent
    raise ValueError('acceptance runtime must be inside its Git Cargo checkout')


ROOT = repository_root(__file__)
QUAL = ROOT / 'target/qualification'
