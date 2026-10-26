import type { TaskFilter } from './tasks.ts';

export type Destination =
  | { kind: 'page'; id: string; block?: string }
  | { kind: 'journal'; date: string; block?: string }
  | { kind: 'tasks'; status: TaskFilter; page: string };

function samePlace(a: Destination, b: Destination): boolean {
  return a.kind === b.kind && (a.kind === 'page' && b.kind === 'page' ? a.id === b.id
    : a.kind === 'journal' && b.kind === 'journal' ? a.date === b.date : a.kind === 'tasks');
}

/** Session-only locations, never persisted content or browser history. Commit only
 * after a successful load; failed departures leave both branches intact. */
export class NavigationHistory {
  private entries: Destination[] = [];
  private index = -1;
  private generation = 0;
  private pending = false;
  private session: string | null = null;
  get current(): Destination | undefined { const d = this.entries[this.index]; return d && { ...d }; }
  get canBack(): boolean { return !this.pending && this.index > 0; }
  get canForward(): boolean { return !this.pending && this.index + 1 < this.entries.length; }
  get length(): number { return this.entries.length; }
  reset(session: string): void {
    this.session = session; this.generation++; this.entries = []; this.index = -1; this.pending = false;
  }
  replace(destination: Destination): void {
    if (this.index >= 0 && samePlace(this.entries[this.index], destination)) this.entries[this.index] = { ...destination };
  }
  async visit(destination: Destination, load: (d: Destination) => Promise<void>): Promise<void> {
    await this.perform(destination, load, null);
  }
  async move(delta: -1 | 1, load: (d: Destination) => Promise<void>): Promise<void> {
    const target = this.index + delta;
    if (target >= 0 && target < this.entries.length) await this.perform(this.entries[target], load, target);
  }
  private async perform(destination: Destination, load: (d: Destination) => Promise<void>, target: number | null): Promise<void> {
    if (!this.session || this.pending) throw new Error('Wait for the current operation');
    const generation = this.generation;
    this.pending = true;
    try {
      await load({ ...destination });
      if (generation !== this.generation) return;
      if (target !== null) { this.index = target; return; }
      const current = this.entries[this.index];
      if (current && samePlace(current, destination)) { this.entries[this.index] = { ...destination }; return; }
      this.entries.splice(this.index + 1);
      this.entries.push({ ...destination });
      if (this.entries.length > 100) this.entries.shift();
      this.index = this.entries.length - 1;
    } finally { if (generation === this.generation) this.pending = false; }
  }
}

export function filterPages<T extends { title: string }>(pages: T[], query: string, label: (page: T) => string): T[] {
  const needle = query.normalize('NFC').toLocaleLowerCase().trim();
  return pages.filter(page => [page.title, label(page)].some(text => text.normalize('NFC').toLocaleLowerCase().includes(needle))).slice(0, 100);
}
