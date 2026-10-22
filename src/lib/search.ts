export interface SearchHit { id: string; page_id: string; kind: string; text: string }

/** Query and graph changes invalidate successes and failures already in flight. */
export class SearchController {
  hits: SearchHit[] = [];
  error = '';
  pending = false;
  private generation = 0;
  private search: (session: string, text: string) => Promise<SearchHit[]>;
  private changed: () => void;
  constructor(search: (session: string, text: string) => Promise<SearchHit[]>, changed = () => {}) {
    this.search = search; this.changed = changed;
  }
  reset() { ++this.generation; this.hits = []; this.error = ''; this.pending = false; this.changed(); }
  async query(session: string, text: string) {
    this.reset();
    if (!text.trim()) return;
    const generation = this.generation;
    this.pending = true; this.changed();
    try { const hits = await this.search(session, text); if (generation === this.generation) this.hits = hits; }
    catch (error) { if (generation === this.generation) this.error = String(error && typeof error === 'object' && 'message' in error ? error.message : error); }
    finally { if (generation === this.generation) { this.pending = false; this.changed(); } }
  }
}
