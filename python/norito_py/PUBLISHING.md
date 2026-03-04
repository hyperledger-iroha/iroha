# Publishing `iroha-norito`

1. Build artifacts without isolation (ensures we reuse the repository's
   `setuptools` installation in air-gapped environments). Install the
   optional `compression` extra locally if you want to exercise Zstandard paths
   before release (`python3 -m pip install .[compression]`):
   ```bash
   cd python/norito_py
   python3 -m pip wheel . -w dist --no-build-isolation
   python3 -m pip install --upgrade build twine  # optional when networked
   python3 -m build  # to produce both wheel and sdist when `build` is available
   ```
2. Upload to TestPyPI for smoke testing (requires credentials):
   ```bash
   python3 -m twine upload --repository testpypi dist/*
   ```
3. Verify installation:
   ```bash
   python3 -m pip install --index-url https://test.pypi.org/simple/ --extra-index-url https://pypi.org/simple iroha-norito
   ```
4. Publish to PyPI:
   ```bash
   python3 -m twine upload dist/*
   ```

Remember to bump the version in both `pyproject.toml` and `setup.cfg` before
shipping a new release.
