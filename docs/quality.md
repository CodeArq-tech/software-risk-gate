# Quality Gates

The repository CI must run:

~~~bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --workspace --all-features --fail-under-lines 95.01
cargo mutants --baseline run --copy-target true --minimum-test-timeout 60 --no-shuffle -j 4
bash scripts/check-mutation-score.sh
~~~

Coverage must be strictly above 95%. Mutation score is calculated from caught,
missed, and timed-out mutants, excluding unviable mutants, and must also be
strictly above 95%.

The current local validation reached 95.95% line coverage and a 96.38%
mutation score, with 266 caught mutants out of 276 scored mutants.

## Failure policy

The CI must fail when formatting, Clippy, tests, coverage, or mutation score
fails. A successful process with incomplete evidence must not be reported as a
successful quality result.
