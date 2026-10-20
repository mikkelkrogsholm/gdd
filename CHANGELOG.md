# Changelog

Package versions identify the Python distribution and CLI. Standard versions
identify the GDD document contracts. Viewer versions identify the HTML renderer;
its current profile remains `gdd-viewer-html/0.1`. These version lines do not
imply a registry publication or a certified standard.

## Unreleased

Target package: 0.8.0. Standard: draft GDD 0.3 with explicit 0.2 support.
Viewer renderer: 0.3.0.

- Make JSON/YAML loading independent of interpreter recursion thresholds with
  an explicit 256-container consumer limit. Greater depth remains UNKNOWN;
  the GDD format and historical executions are unchanged.
- Classify symlink loops consistently across Python versions while retaining
  missing-source UNKNOWN and bundle containment checks.
- Default Graphify proposal candidates to GDD 0.3; `--format gdd/0.2` retains
  the previous candidate contract and exact legacy fixture serialization.
  Proposals remain unaccepted sketches without invented boundary decisions.
- Name missing readiness fields and locate existing fields or their nearest
  existing parents, retaining diagnostic codes and aggregate results.
- Add a small executable planning tutorial and a local check entry point with
  focused, full, explicit interpreter-matrix and installed-wheel release modes.
- Remove the stale package version from the Graphify guide and synchronize
  the portable skill guidance.

- Replace the custom SVG physics/rendering engine with pinned Cytoscape.js 3.34.4
  and fCoSE 2.2.0. Layout runs within visual affinity regions, followed by
  deterministic packing, without the earlier 200-figure solver cutoff.
- Display numbered community names, transparent overlapping extent envelopes,
  shared-node color rings and a keyboard-accessible exact-member legend.
  Community extents are visual hints; explicit memberships remain authoritative.
- Preserve canonical graph payloads, directed relation bundles, folding, scope,
  inspector text and downloaded semantic manifests. Filtering retained figures
  preserves their positions; newly revealed figures reflow the layout.
- Keep one-file offline export with an embedded locked vendor bundle, complete
  third-party notices, restrictive CSP and no runtime build/CDN dependency.
- Extend browser checks for actual canvas interaction, overlapping community
  containment, readable headings, stable layouts and a bounded 300-node fixture.

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
