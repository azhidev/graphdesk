import type { ExtensionAPI } from '../extensions/contracts.ts';
import type { Habit, HabitEvent, HabitStatus, HabitsRequest, HabitsResponse } from '../extensions/habits-contracts.ts';

/** One mounted graph/lease. No optimistic writes, automatic mutation replay or
 * unbounded scans. Failed mutation payloads (including UUIDs) survive retry. */
export class HabitsController {
  habits: Habit[] = []; events: HabitEvent[] = []; statuses = new Map<string, HabitStatus>();
  selected: Habit | null = null;
  more = false; moreEvents = false; busy = false; error = ''; pending: HabitsRequest | null = null;
  loaded = false; private after: string | null = null;
  private live = true;
  private api: ExtensionAPI['habits'];
  private changed: () => void;
  constructor(api: ExtensionAPI['habits'], changed: () => void = () => {}) { this.api = api; this.changed = changed; }
  dispose() { this.live = false; this.habits = []; this.events = []; this.statuses.clear(); this.selected = null; }
  private async request(request: HabitsRequest) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const response = await Promise.race([
      this.api.request(structuredClone(request)),
      new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Response timed out; outcome may be uncertain.')), 10000); }),
    ]).finally(() => clearTimeout(timer));
    if (!this.live) throw new Error('Extension context revoked');
    return response;
  }
  private async work(action: () => Promise<void>) {
    if (!this.live || this.busy) return;
    this.busy = true; this.error = ''; this.changed();
    try { await action(); } catch (e) { if (this.live) this.error = e instanceof Error ? e.message : String(e); }
    finally { if (this.live) { this.busy = false; this.changed(); } }
  }
  async load(append = false) {
    await this.work(async () => {
      const r = await this.request({ op: 'list', after: append ? this.after : null, limit: 30 });
      if (r.type !== 'habits') throw new Error('Unexpected Habits response');
      this.habits = r.data; this.after = r.data.at(-1)?.id ?? null; this.more = r.data.length === 30;
      this.statuses.clear(); this.loaded = true;
      await this.readStatuses();
    });
  }
  private async readStatuses() {
    // Only the current bounded page; sequential requests avoid an IPC burst.
    for (const h of this.habits.slice(-30)) {
      if (h.definition.archived || h.definition.kind === 'negative') continue;
      const r = await this.request({ op: 'status', id: h.id, now: Math.floor(Date.now() / 1000) });
      if (r.type === 'status') this.statuses.set(h.id, r.data);
    }
  }
  async refresh() { if (!this.pending) await this.work(() => this.readStatuses()); }
  async details(habit: Habit, append = false) {
    await this.work(async () => {
      const r = await this.request({ op: 'events', habit_id: habit.id, before: append && this.events.length ? { occurred_at: this.events.at(-1)!.occurred_at, id: this.events.at(-1)!.id } : null, limit: 20 });
      if (r.type !== 'events') throw new Error('Unexpected Habits response');
      this.selected = habit; this.events = r.data; this.moreEvents = r.data.length === 20;
    });
  }
  async mutate(payload?: HabitsRequest) {
    if (this.busy || (!this.pending && !payload)) return false;
    if (this.pending && payload) return false;
    this.pending ??= structuredClone(payload!); this.changed();
    let acknowledged = false;
    await this.work(async () => {
      const r: HabitsResponse = await this.request(this.pending!);
      if (r.type === 'habit') {
        this.habits = [r.data, ...this.habits.filter(h => h.id !== r.data.id)].sort((a, b) => a.id.localeCompare(b.id)).slice(0, 30);
        this.after = this.habits.at(-1)?.id ?? null;
        this.more ||= this.habits.length === 30;
        if (this.selected?.id === r.data.id) this.selected = r.data;
      } else if (r.type === 'event') {
        if (this.selected?.id === r.data.habit_id) this.events = [r.data, ...this.events.filter(e => e.id !== r.data.id)].sort((a,b) => b.occurred_at - a.occurred_at || b.id.localeCompare(a.id)).slice(0, 20);
        this.moreEvents ||= this.events.length === 20;
      } else throw new Error('Unexpected Habits response');
      this.pending = null; acknowledged = true;
      await this.readStatuses();
    });
    return acknowledged;
  }
  /** Explicitly abandon a retry, then reload revisions; never overwrite a conflict. */
  async reconcile() {
    if (this.busy) return;
    const pending = this.pending;
    const selectedId = pending?.op === 'create' ? pending.input.id : pending?.op === 'record' ? pending.input.habit_id : pending?.op === 'update' ? pending.id : this.selected?.id;
    this.pending = null;
    await this.load();
    if (selectedId) await this.work(async () => {
      const r = await this.request({ op: 'get', id: selectedId });
      if (r.type === 'habit') this.selected = r.data;
    });
    if (this.selected) await this.details(this.selected);
  }
}
