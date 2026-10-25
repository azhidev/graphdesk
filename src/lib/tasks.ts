export type TaskFilter = 'active' | 'todo' | 'doing' | 'done';
export interface TaskHit { id: string; page_id: string; page_title: string; content: string; task_status: string }
export type TaskQuery = (session: string, status: TaskFilter, pageId: string | null, offset: number) => Promise<TaskHit[]>;

/** Bounded acknowledged-state discovery, not another editor or task store. */
export class TaskController {
  hits: TaskHit[] = [];
  pending = false;
  error = '';
  more = false;
  private generation = 0;
  private context?: [string, TaskFilter, string | null];
  private query: TaskQuery;
  private changed: () => void;
  constructor(query: TaskQuery, changed = () => {}) { this.query = query; this.changed = changed; }
  async load(session: string, status: TaskFilter, page: string | null) {
    const generation = ++this.generation;
    this.context = [session, status, page]; this.hits = []; this.more = false;
    await this.fetch(generation, 0);
  }
  async next() {
    if (this.pending || !this.more) return;
    await this.fetch(this.generation, this.hits.length);
  }
  private async fetch(generation: number, offset: number) {
    if (!this.context) return;
    this.pending = true; this.error = ''; this.changed();
    try {
      const hits = await this.query(...this.context, offset);
      if (generation !== this.generation) return;
      this.hits = offset ? [...this.hits, ...hits] : hits; this.more = hits.length === 100;
    } catch (e) { if (generation === this.generation) this.error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { if (generation === this.generation) { this.pending = false; this.changed(); } }
  }
}
