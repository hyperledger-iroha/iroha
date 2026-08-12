# Iroha hooks

To ease the development process, you can copy or link these hooks after you clone the repository.

```sh
$ cp hooks/pre-commit.sample .git/hooks/pre-commit
$ cp hooks/commit-msg.sample .git/hooks/commit-msg
$ cp hooks/pre-push.sample .git/hooks/pre-push
```

This way you won't forget to generate the docs if anything is changed.

## Compilation check

We recommend the `pre-push` hook for Rust changes. It runs a fast compilation
check across every workspace target and keeps checking after the first error so
you can fix all reported failures together:

```sh
cargo check --workspace --all-targets --message-format=short --keep-going
```

The check can add a few minutes to each push, depending on the machine and the
state of the build cache, but helps catch non-compiling Rust changes before they
are pushed. It complements rather than replaces the full test and Clippy
workflow.

## Sign-off

The `commit-msg` hook will automatically sign-off your commits.

By signing off your commits, you certify that you have the right to contribute the code within the signed-off commits, i.e. that you are not violating copyright law, DMCA, or any software patent. Check [Developer Certificate of Origin](https://developercertificate.org/) for details.

To learn more about why we require the `signed-off-by:` line, please consult [this question on Stack Overflow](https://stackoverflow.com/questions/1962094/what-is-the-sign-off-feature-in-git-for).
