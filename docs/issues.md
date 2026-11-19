# Link work packages with tracker issues

Store links in a directory alongside the project. Each file identifies the GDD project, the stable package ID and the existing issue URL. Keep the directory under version control and reuse those IDs in successive workplans. A retry, new run directory or new code revision uses the same link. Linking the same URL again is safe. To replace a link, review and edit its file.

For the quote demo, replace the issue URL with an existing issue in your repository.

```sh
gdd issues link examples/agent-native/model.gdd.json --run examples/agent-native/native-run \
  --package package.discount --issues issue-links --url https://github.com/OWNER/REPO/issues/17
gdd work examples/agent-native/model.gdd.json --run examples/agent-native/native-run \
  --issues issue-links --refresh-issues --format text
```

By default, reports read local links offline. Add `--refresh-issues` to read the current title, assignees and open/closed state from GitHub.com. Use `GITHUB_TOKEN` or `GH_TOKEN` for private issues. Other trackers can be linked by HTTPS URL. Reviewed work, integration and verification come from the GDD journal and revision-bound evidence. Closing an issue only changes its tracker state.

To post a result, supply an existing HTTPS address that your team can open. This command writes to GitHub. It looks for the authenticated user's GDD comment for this project and package, updates it when the result changes, and skips an identical result. Use one publisher account across runs and serialize publishers from separate checkouts. The result artifact must already be available at the supplied address.

```sh
gdd issues publish examples/agent-native/model.gdd.json --run examples/agent-native/native-run \
  --package package.discount --issues issue-links \
  --host-context examples/agent-native/host.acceptance.json \
  --result-url https://example.org/results/discount
```

Publishing requires a user token with permission to write issue comments in the linked repository. It leaves issue state, assignees and other comments unchanged. The adapter uses the [GitHub issue comments API](https://docs.github.com/en/rest/issues/comments).
