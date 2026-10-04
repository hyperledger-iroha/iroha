"""Exact reviewed private child tool selection, separate from the191-file core.

These fixed source hashes are review inputs, not caller-provided approvals.
Candidate/toolchain/source cleanliness and execution authority remain parent-owned.
"""
CHILD_TOOL_SHA256 = (
    ('sorafs_javascript_child.mjs', 'a16ef21f1b821610ff768e7e1cd2c5aec30e65103252a62b94546af6a0723566'),
    ('sorafs_javascript_child_entry.mjs', '4069786bf3b9d782a840af35e430f8ca37f4abdfed7697b75f94d2f0f87df89b'),
    ('sorafs_javascript_child_files.mjs', 'e7308e4d997d2a759555024e16193eafe2267ad286849e77a2397fe714c19dfe'),
    ('sorafs_javascript_child_input.mjs', '4f0f0a4f5df67fbe660ae681575b4ccd0e0100f6e0fb23a744a4d957eb001829'),
    ('sorafs_javascript_child_loads.mjs', 'b69112bf278c6341b36380cc74b60fdae0ad286ffebdb98bddc7b8ee3b72a28b'),
    ('sorafs_javascript_child_session.mjs', 'ea9effc7d1f881fa39513890b1256a81c09a65cf9343ba23fb1f963a3951a244'),
    ('sorafs_javascript_native_cache.mjs', '504ffaa891127e9eb4839baecb0fb51b70d782a0eeea0d96485747152b0bc39d'),
    ('sorafs_javascript_test_events.mjs', '9c784cb341c85fd540f1144f327e28d57895863bdb35eba2a9247e4409d721c4'),
)
