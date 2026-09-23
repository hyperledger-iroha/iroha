"""Exact reviewed private child tool selection, separate from the191-file core.

These fixed source hashes are review inputs, not caller-provided approvals.
Candidate/toolchain/source cleanliness and execution authority remain parent-owned.
"""
CHILD_TOOL_SHA256 = (
    ('sorafs_javascript_child.mjs', 'a16ef21f1b821610ff768e7e1cd2c5aec30e65103252a62b94546af6a0723566'),
    ('sorafs_javascript_child_entry.mjs', '4069786bf3b9d782a840af35e430f8ca37f4abdfed7697b75f94d2f0f87df89b'),
    ('sorafs_javascript_child_files.mjs', 'e7308e4d997d2a759555024e16193eafe2267ad286849e77a2397fe714c19dfe'),
    ('sorafs_javascript_child_input.mjs', '2ee0048bef03aadfbe48c8a961ad10518b5f545cebfb487644bc5628db7c668e'),
    ('sorafs_javascript_child_loads.mjs', 'b69112bf278c6341b36380cc74b60fdae0ad286ffebdb98bddc7b8ee3b72a28b'),
    ('sorafs_javascript_child_session.mjs', '7d171155645f220e94fc3d313e22e539048ba6f6dd9e0e383a63412a8e5e5f00'),
    ('sorafs_javascript_native_cache.mjs', '504ffaa891127e9eb4839baecb0fb51b70d782a0eeea0d96485747152b0bc39d'),
    ('sorafs_javascript_test_events.mjs', 'db3aa95d20b6a9ce20b2c51835af4f41658807cd708f82f578951af0dfd74094'),
)
