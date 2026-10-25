export interface SearchHit { id: string; page_id: string; kind: string; text: string }

/** Query and graph changes invalidate successes and failures already in flight. */
export class SearchController {
  hits: SearchHit[] = [];
  error = '';
  pending = false;
  private generation = 0;
  private timer?: ReturnType<typeof setTimeout>;
  private release?: () => void;
  private search: (session: string, text: string) => Promise<SearchHit[]>;
  private changed: () => void;
  private delay: number;
  constructor(search: (session: string, text: string) => Promise<SearchHit[]>, changed = () => {}, delay = 200) {
    this.search = search; this.changed = changed; this.delay = delay;
  }
  reset() {
    ++this.generation;
    if (this.timer) clearTimeout(this.timer);
    this.release?.(); this.release = undefined; this.timer = undefined;
    this.hits = []; this.error = ''; this.pending = false; this.changed();
  }
  async query(session: string, text: string) {
    this.reset();
    if (!text.trim()) return;
    const generation = this.generation;
    this.pending = true; this.changed();
    if (this.delay) await new Promise<void>(resolve => { this.release = resolve; this.timer = setTimeout(resolve, this.delay); });
    if (generation !== this.generation) return;
    this.timer = undefined; this.release = undefined;
    try { const hits = await this.search(session, text); if (generation === this.generation) this.hits = hits; }
    catch (error) { if (generation === this.generation) this.error = String(error && typeof error === 'object' && 'message' in error ? error.message : error); }
    finally { if (generation === this.generation) { this.pending = false; this.changed(); } }
  }
}
