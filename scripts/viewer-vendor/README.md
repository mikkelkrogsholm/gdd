# Offline viewer engine bundle

This private build project produces the checked-in browser bundle at
`gdd_validator/viewer/vendor/cytoscape-fcose.js`. GDD CLI users do not need Node,
npm, a CDN, or a build step. The generated graph includes the bundle and its
third-party notices inside the same HTML file.

The browser global is `window.GDDVendor`, containing `cytoscape` and
`versions: { cytoscape: "3.34.4", fcose: "2.2.0" }`. fCoSE is already registered.
No BubbleSets extension is included; community outlines belong to the viewer.

## Rebuild locally

Use Node.js 20 or newer and npm with lockfile version 3 support. From this
directory:

```sh
npm ci --ignore-scripts
npm run build
npm run check
```

`npm ci` needs the npm registry or a populated local cache. The build and smoke
check run offline after installation. esbuild 0.28.2 is pinned in the lockfile;
its platform binary is installed as a locked optional dependency. The check
rebuilds all output in memory and requires byte-for-byte equality. It then
loads the IIFE in a JavaScript VM with dynamic code generation disabled and no
network globals, verifies the public versions and registered layout, and runs
a headless fCoSE triangle to check finite, distinct positions.

The build contains no timestamp or absolute checkout path. `manifest.json`
records the output digest and size, build tool, input digests, locked package
URLs and integrity values. The published Cytoscape ESM file is an input; its
embedded Heap, Lodash, and gl-matrix versions follow the upstream
[`v3.34.4` lockfile](https://github.com/cytoscape/cytoscape.js/blob/v3.34.4/package-lock.json).
Those packages are installed here only to retain their exact license texts,
and are not bundled a second time. esbuild is build tooling and is not part
of the browser runtime.

## CSP and notices

The build replaces the single embedded Lodash global-object fallback
`Function('return this')()` with `globalThis`. This removes dynamic code
generation while preserving browser behavior. The source match is checked
before replacement and the patch is recorded in the manifest. All other
input code passes through esbuild without source patches.

The build rejects `eval`/`Function` calls, worker constructors, dynamic imports,
and the `fetch`/`XMLHttpRequest`/`WebSocket` APIs. It escapes HTML script closing
sequences, so the result can be placed in a single script element under a
hash-based Content Security Policy without enabling `unsafe-eval`. Cytoscape's
general image styling API remains available; the GDD renderer must avoid
external image URLs and restrict image sources with CSP. The renderer must
hash the exact final script bytes after combining it with its own runtime.

The bundle retains full notices as an inline comment. Separate license files
and `THIRD-PARTY-NOTICES.txt` make review easier. Cytoscape, fCoSE, cose-base,
layout-base, and the dependencies embedded by Cytoscape use MIT licenses;
layout-base additionally embeds modified JamaJS SVD code under Apache-2.0.
The original modification and attribution notice is retained with that license.
