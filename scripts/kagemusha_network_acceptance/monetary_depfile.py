"""Exact emitted-target dependency rules for retained monetary compiler inputs."""
import shlex

def target_inputs(contents, target):
    """Select the actual compiler target's Make rule, never an unrelated first rule."""
    rules = []
    for line in contents.replace('\\\n', ' ').splitlines():
        if not line or line.startswith('#'):
            continue
        lhs, sep, rhs = line.partition(': ')
        if not sep:
            continue
        if target in shlex.split(lhs):
            rules.append(set(shlex.split(rhs)))
    if len(rules) != 1 or not rules[0]:
        raise ValueError('exact nonempty compiler target rule is absent or ambiguous')
    return rules[0]
