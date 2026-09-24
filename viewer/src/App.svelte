<script lang="ts">
  import { onMount } from 'svelte';
  import type { MetricDefinition, MetricObservation, ResultBundle, ResultCatalog } from './types';

  let bundles: ResultBundle[] = [];
  let selectedIds = new Set<string>();
  let xMetric = '';
  let yMetric = '';
  let selectedPoint: ResultBundle | null = null;
  let source = '';
  let error = '';
  const width = 820, height = 500, pad = 58;

  $: visible = bundles.filter((bundle) => selectedIds.has(bundle.id));
  $: definitions = uniqueMetrics(visible);
  $: if (!definitions.some((metric) => metric.id === xMetric)) xMetric = definitions[0]?.id ?? '';
  $: if (!definitions.some((metric) => metric.id === yMetric)) yMetric = definitions[1]?.id ?? definitions[0]?.id ?? '';
  $: points = visible.flatMap((bundle) => {
    const x = observation(bundle, xMetric), y = observation(bundle, yMetric);
    return x && y ? [{ bundle, x, y }] : [];
  });
  $: xDomain = domain(points.map((point) => point.x.value));
  $: yDomain = domain(points.map((point) => point.y.value));
  $: xDefinition = definitions.find((metric) => metric.id === xMetric);
  $: yDefinition = definitions.find((metric) => metric.id === yMetric);
  $: frontier = pareto(points, xDefinition, yDefinition);

  onMount(async () => {
    const params = new URLSearchParams(location.search);
    const urls = params.getAll('bundle');
    const catalog = params.get('catalog');
    if (catalog) await loadUrl(catalog);
    else if (urls.length) for (const url of urls) await loadUrl(url);
    else await loadUrl('./results/catalog.json');
  });

  function uniqueMetrics(values: ResultBundle[]): MetricDefinition[] {
    const seen = new Map<string, MetricDefinition>();
    for (const bundle of values) for (const metric of bundle.metrics) if (!seen.has(metric.id)) seen.set(metric.id, metric);
    return [...seen.values()];
  }
  function observation(bundle: ResultBundle, id: string): MetricObservation | undefined {
    return bundle.observations.find((value) => value.metric_id === id);
  }
  function domain(values: number[]): [number, number] {
    if (!values.length) return [0, 1];
    const low = Math.min(...values), high = Math.max(...values);
    const margin = (high - low || Math.abs(high) || 1) * 0.12;
    return [low - margin, high + margin];
  }
  function sx(value: number) { return pad + ((value - xDomain[0]) / (xDomain[1] - xDomain[0])) * (width - pad * 2); }
  function sy(value: number) { return height - pad - ((value - yDomain[0]) / (yDomain[1] - yDomain[0])) * (height - pad * 2); }
  function tickValue(bounds: [number, number], tick: number) { return bounds[1] - tick * (bounds[1] - bounds[0]); }
  function display(value: number) { return Math.abs(value) >= 100 ? value.toFixed(0) : value.toFixed(2).replace(/\.00$/, ''); }
  function better(a: number, b: number, goal?: string) { return goal === 'minimize' ? a <= b : a >= b; }
  function pareto(values: typeof points, x?: MetricDefinition, y?: MetricDefinition) {
    return values.filter((candidate) => !values.some((other) => other !== candidate &&
      better(other.x.value, candidate.x.value, x?.goal) && better(other.y.value, candidate.y.value, y?.goal) &&
      (other.x.value !== candidate.x.value || other.y.value !== candidate.y.value)))
      .sort((a, b) => sx(a.x.value) - sx(b.x.value));
  }
  async function loadUrl(url: string) {
    error = '';
    try {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      const value = await response.json() as ResultBundle | ResultCatalog;
      if (value.schema === 'eval.result-catalog.v1') {
        const base = new URL(url, location.href);
        for (const entry of value.bundles) await loadUrl(new URL(entry.url, base).href);
      } else if (value.schema === 'eval.result-bundle.v1') addBundle(value);
      else throw new Error('Unsupported result schema');
      source = url;
    } catch (cause) { error = `Could not load ${url}: ${cause instanceof Error ? cause.message : cause}`; }
  }
  function addBundle(bundle: ResultBundle) {
    bundles = [...bundles.filter((value) => value.id !== bundle.id), bundle];
    selectedIds = new Set([...selectedIds, bundle.id]);
  }
  async function openFiles(event: Event) {
    const files = (event.currentTarget as HTMLInputElement).files;
    if (!files) return;
    for (const file of files) {
      try { addBundle(JSON.parse(await file.text()) as ResultBundle); }
      catch { error = `${file.name} is not a valid result bundle.`; }
    }
  }
  function toggle(id: string) {
    const next = new Set(selectedIds); next.has(id) ? next.delete(id) : next.add(id); selectedIds = next;
  }
  function downloadSvg() {
    const node = document.querySelector('#pareto-plot'); if (!node) return;
    const blob = new Blob([new XMLSerializer().serializeToString(node)], { type: 'image/svg+xml' });
    const anchor = document.createElement('a'); anchor.href = URL.createObjectURL(blob); anchor.download = 'eval-pareto.svg'; anchor.click(); URL.revokeObjectURL(anchor.href);
  }
  function label(bundle: ResultBundle) { return `${bundle.subject.name} ${bundle.subject.revision}`; }
  function short(value: string) { return value.slice(0, 9); }
</script>

<svelte:head><meta name="description" content="Portable, attributed benchmark result viewer" /></svelte:head>

<header>
  <a class="wordmark" href="./">eval/field</a>
  <div class="source"><span>Loaded from</span><strong>{source || 'local data'}</strong></div>
  <label class="file-button">Open JSON<input type="file" accept="application/json,.json" multiple onchange={openFiles} /></label>
</header>

<main>
  <aside aria-label="Result bundles">
    <h1>Evidence, compared.</h1>
    <p class="intro">Every mark resolves to the runs, tasks, and revisions that produced it.</p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="bundle-list">
      {#each bundles as bundle}
        <label class:active={selectedIds.has(bundle.id)}>
          <input type="checkbox" checked={selectedIds.has(bundle.id)} onchange={() => toggle(bundle.id)} />
          <span><strong>{bundle.subject.name}</strong><small>{bundle.benchmark.id} / {bundle.subject.revision}</small></span>
        </label>
      {:else}<p class="empty">Load a catalog, API URL, or result-bundle JSON file.</p>{/each}
    </div>
  </aside>

  <section class="workbench">
    <div class="plot-header">
      <div><h2>Pareto field</h2><p>{points.length} comparable result{points.length === 1 ? '' : 's'}</p></div>
      <div class="axes">
        <label>X axis<select bind:value={xMetric}>{#each definitions as metric}<option value={metric.id}>{metric.label}</option>{/each}</select></label>
        <label>Y axis<select bind:value={yMetric}>{#each definitions as metric}<option value={metric.id}>{metric.label}</option>{/each}</select></label>
        <button onclick={downloadSvg}>Export SVG</button>
      </div>
    </div>

    <div class="plot-wrap">
      <svg id="pareto-plot" viewBox={`0 0 ${width} ${height}`} role="img" aria-labelledby="plot-title plot-desc">
        <title id="plot-title">Pareto comparison of {xDefinition?.label} and {yDefinition?.label}</title>
        <desc id="plot-desc">{points.length} attributed benchmark results. Orange rings indicate the Pareto frontier.</desc>
        <rect width={width} height={height} fill="#f5f7f8" />
        {#each [0, .25, .5, .75, 1] as tick}
          <line x1={pad} x2={width-pad} y1={pad + tick*(height-pad*2)} y2={pad + tick*(height-pad*2)} class="grid" />
          <line y1={pad} y2={height-pad} x1={pad + tick*(width-pad*2)} x2={pad + tick*(width-pad*2)} class="grid" />
          <text x={pad-9} y={pad + tick*(height-pad*2)+4} text-anchor="end" class="tick">{display(tickValue(yDomain, tick))}</text>
          <text x={pad + tick*(width-pad*2)} y={height-pad+19} text-anchor="middle" class="tick">{display(xDomain[0] + tick*(xDomain[1]-xDomain[0]))}</text>
        {/each}
        {#if frontier.length > 1}<polyline points={frontier.map((point) => `${sx(point.x.value)},${sy(point.y.value)}`).join(' ')} class="frontier" />{/if}
        {#each points as point}
          <g class="point" class:on-frontier={frontier.includes(point)} tabindex="0" role="button" aria-label={`${label(point.bundle)}: ${point.x.value}, ${point.y.value}`} onclick={() => selectedPoint = point.bundle} onkeydown={(event) => event.key === 'Enter' && (selectedPoint = point.bundle)}>
            {#if point.x.lower_bound !== undefined && point.x.upper_bound !== undefined}<line class="interval" x1={sx(point.x.lower_bound)} x2={sx(point.x.upper_bound)} y1={sy(point.y.value)} y2={sy(point.y.value)} />{/if}
            {#if point.y.lower_bound !== undefined && point.y.upper_bound !== undefined}<line class="interval" y1={sy(point.y.lower_bound)} y2={sy(point.y.upper_bound)} x1={sx(point.x.value)} x2={sx(point.x.value)} />{/if}
            <circle cx={sx(point.x.value)} cy={sy(point.y.value)} r="8" />
            <text x={sx(point.x.value)+13} y={sy(point.y.value)-10}>{point.bundle.subject.name}</text>
          </g>
        {/each}
        <text class="axis-label" x={width/2} y={height-12} text-anchor="middle">{xDefinition?.label} ({xDefinition?.unit}) · {xDefinition?.goal}</text>
        <text class="axis-label" transform={`translate(17 ${height/2}) rotate(-90)`} text-anchor="middle">{yDefinition?.label} ({yDefinition?.unit}) · {yDefinition?.goal}</text>
      </svg>
    </div>

    <div class="ledger">
      <h2>Source ledger</h2>
      <div class="table-wrap"><table><thead><tr><th>Model</th><th>Benchmark</th><th>Harness</th><th>Tasks</th><th>Runs</th><th>Generated</th></tr></thead>
        <tbody>{#each visible as bundle}<tr class:selected={selectedPoint?.id === bundle.id} onclick={() => selectedPoint = bundle}>
          <td><strong>{bundle.subject.name}</strong><small>{bundle.subject.provider} / {bundle.subject.revision}</small></td>
          <td>{bundle.benchmark.id} v{bundle.benchmark.version}</td><td>{short(bundle.provenance.harness_commit)}</td><td>{short(bundle.provenance.task_set_commit)}</td><td>{bundle.runs.length}</td><td>{bundle.generated_at.slice(0, 10)}</td>
        </tr>{/each}</tbody></table></div>
    </div>
  </section>
</main>

{#if selectedPoint}
  <aside class="drawer" aria-label="Result provenance">
    <button class="close" aria-label="Close provenance" onclick={() => selectedPoint = null}>×</button>
    <p class="kicker">{selectedPoint.benchmark.id}</p><h2>{label(selectedPoint)}</h2>
    <dl><dt>Bundle</dt><dd>{selectedPoint.id}</dd><dt>Harness commit</dt><dd>{selectedPoint.provenance.harness_commit}</dd><dt>Task-set commit</dt><dd>{selectedPoint.provenance.task_set_commit}</dd><dt>Scorer</dt><dd>{selectedPoint.provenance.scorer ?? 'not recorded'}</dd></dl>
    <h3>Attributed metrics</h3>
    {#each selectedPoint.observations as value}<div class="metric"><strong>{selectedPoint.metrics.find((m) => m.id === value.metric_id)?.label}</strong><span>{value.value} · n={value.sample_count}</span><small>{value.run_ids.length} run references / {value.task_ids?.length ?? 0} tasks</small></div>{/each}
    {#if selectedPoint.provenance.source_url}<a class="source-link" href={selectedPoint.provenance.source_url}>Open source artifact</a>{/if}
  </aside>
{/if}
