export type Goal = 'minimize' | 'maximize';

export interface MetricDefinition {
  id: string; label: string; unit: string; goal: Goal; aggregation: string;
}
export interface MetricObservation {
  metric_id: string; value: number; sample_count: number;
  lower_bound?: number; upper_bound?: number; run_ids: string[]; task_ids?: string[];
}
export interface RunRecord {
  id: string; task_id: string; trial: number; duration_ms?: number; cost_usd?: number;
  artifacts?: { name: string; media_type: string; url: string; sha256?: string }[];
}
export interface ResultBundle {
  schema: 'eval.result-bundle.v1'; id: string; generated_at: string;
  benchmark: { id: string; version: string; title?: string };
  subject: { kind: string; provider: string; name: string; revision: string };
  provenance: { harness_commit: string; task_set_commit: string; scorer?: string; source_url?: string };
  metrics: MetricDefinition[]; observations: MetricObservation[]; runs: RunRecord[];
}
export interface ResultCatalog {
  schema: 'eval.result-catalog.v1';
  bundles: { id: string; url: string }[];
}
