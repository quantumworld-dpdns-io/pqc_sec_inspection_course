/**
 * Tool/plugin pairs offered by New Test. The plugin prefix is what the API maps onto a
 * prober adapter (`services/api/src/routes/tests.rs`), so the names here are not cosmetic:
 * `bq`/`bo` → BoringSSL, `wo` → wolfSSL, `go` → Go, `open` → OpenSSL.
 */
export const TOOLS: Array<{ tool: string; plugins: string[] }> = [
  { tool: "pq-dsa", plugins: ["open3x", "bq2606", "bo2605", "wolf54", "go125"] },
  { tool: "pq-kem", plugins: ["open3x", "go125"] },
];

export function pluginsFor(tool: string): string[] {
  return TOOLS.find((entry) => entry.tool === tool)?.plugins ?? [];
}
