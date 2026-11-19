# Repository language

All repository prose must be English: documentation, comments, user-facing
messages, example contracts and pseudocode, evidence summaries, and generated
visuals. Use English when adding or updating these artifacts.

Proper names, identifiers, and intentional Unicode parser/identity test data
retain their technical meaning. English prose does not imply ASCII-only data.

Changing model or normative asset bytes changes their revisions. Update current
snapshot and record references, regenerate derived artifacts, and run relevant
checks. Historical execution claims and digests remain tied to their original
Git state; identify translated presentations and reconstructed captures rather
than treating them as new executions. See [evidence](evidence/README.md).

# Local verification

Verification for the GDD repository runs locally. The maintainer has chosen
not to run CI on GitHub because it is too heavy. Do not add GitHub Actions
workflows unless the maintainer explicitly changes that direction. Run checks
appropriate to the change; full conformance, wheel and browser checks remain
available for local release verification. See [contributing](CONTRIBUTING.md).

# Disk use and cleanup

- The local Rust migration worktree has a shared 10 GiB limit for the checkout,
  build outputs, caches, and temporary files. In this worktree, run builds and
  tests through `gdd-work` and check free space with `gdd-work --status`.
- Check space before a large job and while it runs. If the remaining space may
  be insufficient, stop the job and reclaim space before continuing. Keep
  temporary files inside the budget, including when retrying a failed job.
- After every task, including failed or interrupted runs, remove the build
  outputs, temporary files, and disposable caches you created. Preserve user
  files and requested deliverables, then check disk usage again.
