# Eval result viewer

The viewer consumes `eval.result-bundle.v1` without containing benchmark-specific scoring logic.

- Default: loads `public/results/catalog.json`.
- API/catalog: `/?catalog=https://example.test/api/eval/catalog.json`
- Direct bundles: repeat `?bundle=https://example.test/results/run.json`.
- Offline/Canvas: open or import the same JSON bundle file.
- CI: `npm run render:ci` writes attributed `pareto.svg` and `pareto.png` artifacts.

Remote APIs must return the same JSON shapes and allow the viewer origin through CORS. A catalog is only an index of bundle URLs; immutable bundles remain the source of truth.
