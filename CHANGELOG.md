# Changelog

Package versions identify the Python distribution and CLI. Standard versions
identify the GDD document contracts. Viewer versions identify the HTML renderer;
its current profile remains `gdd-viewer-html/0.1`. These version lines do not
imply a registry publication or a certified standard.

## Unreleased

Target package: 0.7.1. Standard: draft GDD 0.3 with explicit 0.2 support.
Viewer renderer: 0.2.1.

- Spread communities with separate attraction centers, node repulsion and
  weaker, longer springs between groups. Component ownership supplies layout
  hints when explicit communities are absent; shared members bridge groups.
- Reserve collision space for node glyphs and both label lines in the bounded
  force layout. Keep retained node positions stable when filtering and lay out
  newly revealed nodes before fitting the view.
- Add browser regression cases for dense communities and ownership groups,
  including label collisions, deterministic reloads and stable filtering.

## 0.7.0 — October 14, 2026

- Package: `gdd-validator` 0.7.0. Standard: draft GDD 0.3, with explicit 0.2
  compatibility. Viewer renderer: 0.2.0.
- Add safe `gdd init PATH --project-id ID --title TITLE` scaffolding. A new
  project is structurally valid and remains unready until its scope is refined.
- Include starter templates and a self-contained GDD agent skill in the wheel,
  with a synchronization check for copied reference guidance.
- Document source and local-wheel installation, agent-skill installation,
  fresh-project and existing-code entry paths, and optional browser dependencies.
- Document local Python 3.11/3.12, core, conformance, installed-wheel and optional
  Chromium checks. Remove the initial GitHub Actions workflow following the
  maintainer's decision to keep verification local.
- Extend installed-wheel verification with source/module/console parity,
  current and legacy CLI checks, actual legacy replay and no-overwrite controls.
- Add opt-in host helpers for actual code snapshots, command receipts and
  checked-binding records, with explicit execution and trust responsibilities.
- Curate the concert reference and preserve its complete original subtree in a
  byte-indexed, verified recovery archive.
- Record the maintainer-selected MIT license in LICENSE and package metadata.
- Establish the first annotated package tag, contribution/fork/PR guidance,
  GitHub issue and pull-request templates, and manual release instructions.

Local repository preparation is complete; actual independent journeys and
verification are recorded in [the readiness review](evidence/repository-readiness.md).
This is the first tagged GitHub development release. The package has not been
published to a registry.

## 0.6.0 — inspected baseline

- Introduce GDD 0.3 design boundaries, component and entity contracts,
  communities and advisory design review, alongside explicit GDD 0.2 support.
- Add boundary derivation, workflow checks and installed legacy replay controls.
- Use viewer renderer 0.2.0 for both GDD 0.2 and 0.3 models.

This is a summary of the baseline package scope, not a reconstructed publication
history. Preserved execution evidence retains its original revisions and Git
identity.
