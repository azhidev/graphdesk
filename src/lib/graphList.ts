/** Pure graph-list presentation helpers. No Tauri/IPC dependency, so these are
 * exercised directly by the editor test suite.
 *
 * A "known graph" is a graph path plus its logical UUID. Paths are canonical
 * (resolved by the backend), so the path is the identity used for de-duplication;
 * the UUID is never used to merge copies that share a logical graph identity. */

export interface KnownGraph { path: string; graph_id: string }

const EXTENSION = /\.(?:sqlite3|sqlite|db3|db)$/i;

/** File stem without the SQLite extension; never the full path. */
export function graphDisplayName(path: string): string {
  const filename = path.split(/[\\/]/).at(-1) ?? path;
  const trimmed = filename.replace(EXTENSION, '').trim();
  return trimmed || filename;
}

function parentName(path: string): string {
  return path.split(/[\\/]/).slice(0, -1).at(-1) ?? '';
}

/** Unique, human-readable labels. Copies or same-named files in different
 * directories stay distinguishable instead of collapsing into one identity. */
export function graphLabels(graphs: KnownGraph[]): Map<string, string> {
  const counts = new Map<string, number>();
  for (const graph of graphs) {
    const name = graphDisplayName(graph.path);
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  const candidates = graphs.map(graph => {
    const name = graphDisplayName(graph.path);
    return (counts.get(name) ?? 0) > 1 ? `${name} — ${parentName(graph.path)}` : name;
  });
  const labels = new Map<string, string>();
  graphs.forEach((graph, index) => {
    const candidate = candidates[index];
    // Copies can share UUIDs AND parent-directory names. Use the canonical path
    // only when short labels cannot disambiguate them.
    labels.set(graph.path, candidates.filter(label => label === candidate).length > 1
      ? `${candidate} — ${graph.path}` : candidate);
  });
  return labels;
}

/** Session-visible graphs first (most recent first), then persisted entries,
 * de-duplicated by canonical path and bounded to the registry limit. */
export function mergeGraphs(session: KnownGraph[], persisted: KnownGraph[], limit = 10): KnownGraph[] {
  const seen = new Set<string>();
  const merged: KnownGraph[] = [];
  for (const graph of [...session, ...persisted]) {
    if (!graph.path || seen.has(graph.path)) continue;
    seen.add(graph.path);
    merged.push(graph);
    if (merged.length >= limit) break;
  }
  return merged;
}

/** Index for the Ctrl+Tab switcher. `-1` means nothing is selected yet, so the
 * first forward step selects the first entry and the first backward step the last. */
export function cycleCursor(current: number, delta: number, length: number): number {
  if (length <= 0) return -1;
  if (current < 0) return delta > 0 ? 0 : length - 1;
  return ((current + delta) % length + length) % length;
}
