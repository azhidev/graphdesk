import type { Block, EditAction, EditResult, EditorTransport, DeletePreview } from './types.ts';
import { visibleRows, type Row } from './outline.ts';

interface Draft { content: string; generation: number; firstAt: number; group: string }
export interface Selection { start: number; end: number; direction?: 'forward' | 'backward' | 'none' }
export interface Focus extends Selection { id: string; serial: number }
export function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error && 'message' in error) return String(error.message);
  return String(error);
}

/** UI-independent draft/IO coordinator. This is not an authoritative document model.
 * Blocks are acknowledged SQLite DTOs; drafts survive failed saves until explicitly discarded.
 */
export class EditorController {
  session: string | null = null;
  pageId: string | null = null;
  blocks: Block[] = [];
  collapsed = new Set<string>();
  activeId: string | null = null;
  focus: Focus | null = null;
  working = false;
  saving = false;
  composing = false;
  error = '';
  canUndo = false;
  canRedo = false;
  private drafts = new Map<string, Draft>();
  private timers = new Map<string, ReturnType<typeof setTimeout>>();
  private groups = new Map<string, { id: string; at: number }>();
  private selections = new Map<string, Selection>();
  private tail = Promise.resolve();
  private generation = 0;
  private focusSerial = 0;
  private cachedRows: Row[] | null = null;

  private transport: EditorTransport;
  private changed: () => void;
  private timing: { debounce: number; maxWait: number; groupGap: number };
  constructor(transport: EditorTransport, changed: () => void = () => {},
    timing = { debounce: 300, maxWait: 1000, groupGap: 1000 }) {
    this.transport = transport; this.changed = changed; this.timing = timing;
  }

  get dirty(): boolean { return this.drafts.size > 0; }
  get status(): string { return this.error ? 'Error — unsaved changes may remain' : this.saving ? 'Saving…' : this.dirty ? 'Unsaved' : 'Saved'; }
  rows(): Row[] { return this.cachedRows ??= visibleRows(this.blocks, this.collapsed); }
  text(id: string): string { return this.drafts.get(id)?.content ?? this.blocks.find(b => b.id === id)?.content ?? ''; }
  selection(id: string): Selection { return this.selections.get(id) ?? { start: this.text(id).length, end: this.text(id).length }; }
  remember(id: string, selection: Selection): void { this.selections.set(id, selection); }
  activate(id: string, selection = this.selection(id)): void {
    this.activeId = id;
    this.focus = { id, ...selection, serial: ++this.focusSerial };
    this.changed();
  }

  reveal(id: string): void {
    if (!this.blocks.some(b => b.id === id)) throw new Error('Matching block no longer exists');
    let parent = this.blocks.find(b => b.id === id)?.parent_id;
    const seen = new Set<string>();
    while (parent && !seen.has(parent)) {
      seen.add(parent); this.collapsed.delete(parent);
      parent = this.blocks.find(b => b.id === parent)?.parent_id;
    }
    this.cachedRows = null; this.activate(id, { start: 0, end: 0 });
  }

  reset(session: string): void {
    if (this.dirty || this.saving || this.working) throw new Error('Flush before changing graph');
    this.clearTimers(); this.groups.clear(); this.selections.clear();
    this.session = session; this.pageId = null; this.blocks = []; this.collapsed.clear();
    this.activeId = null; this.focus = null; this.canUndo = false; this.canRedo = false;
    this.error = ''; this.cachedRows = null; this.changed();
  }

  async load(pageId: string): Promise<void> {
    if (this.working) throw new Error('Wait for the current editor operation');
    this.working = true; this.changed();
    try {
      await this.flush();
      if (!this.session) throw new Error('Open a graph first');
      const result = await this.transport.page(this.session, pageId);
      this.pageId = pageId; this.blocks = result.blocks;
      this.canUndo = result.can_undo; this.canRedo = result.can_redo;
      this.activeId = null; this.focus = null; this.groups.clear(); this.cachedRows = null;
    } finally { this.working = false; this.changed(); }
  }

  setDraft(id: string, content: string): void {
    if (this.working || !this.blocks.some(b => b.id === id)) return;
    const now = Date.now();
    let group = this.groups.get(id);
    if (!group || now - group.at > this.timing.groupGap) group = { id: crypto.randomUUID(), at: now };
    group.at = now; this.groups.set(id, group);
    this.drafts.set(id, { content, generation: ++this.generation, firstAt: this.drafts.get(id)?.firstAt ?? now, group: group.id });
    this.schedule(id); this.changed();
  }

  composition(active: boolean): void {
    this.composing = active;
    if (active) this.clearTimers(); else for (const id of this.drafts.keys()) this.schedule(id);
    this.changed();
  }

  leave(id: string): void {
    this.groups.delete(id);
    if (!this.composing) void this.save(id).catch(() => {});
  }

  private schedule(id: string): void {
    const timer = this.timers.get(id); if (timer) clearTimeout(timer);
    if (this.error || this.composing) return;
    const elapsed = Date.now() - (this.drafts.get(id)?.firstAt ?? Date.now());
    this.timers.set(id, setTimeout(() => { this.timers.delete(id); void this.save(id).catch(() => {}); },
      Math.max(0, Math.min(this.timing.debounce, this.timing.maxWait - elapsed))));
  }
  private clearTimers(): void { for (const timer of this.timers.values()) clearTimeout(timer); this.timers.clear(); }
  private enqueue<T>(work: () => Promise<T>): Promise<T> {
    const next = this.tail.then(work);
    this.tail = next.then(() => {}, () => {});
    return next;
  }
  private context(): [string, string] {
    if (!this.session || !this.pageId) throw new Error('Select a page first');
    return [this.session, this.pageId];
  }
  private apply(result: EditResult): void {
    const removed = new Set(result.removed);
    const updates = new Map(result.changed.map(b => [b.id, b]));
    this.blocks = this.blocks.filter(b => !removed.has(b.id)).map(b => {
      const updated = updates.get(b.id); updates.delete(b.id); return updated ?? b;
    }).concat([...updates.values()]);
    for (const id of removed) { this.drafts.delete(id); this.selections.delete(id); this.collapsed.delete(id); }
    this.canUndo = result.can_undo; this.canRedo = result.can_redo; this.cachedRows = null;
  }

  private save(id: string): Promise<void> {
    return this.enqueue(async () => {
      if (this.error) throw new Error(this.error);
      if (this.composing) return;
      const draft = this.drafts.get(id);
      if (!draft) return;
      const block = this.blocks.find(b => b.id === id);
      if (!block) throw new Error('Draft block is missing; reload explicitly to discard');
      if (block.content === draft.content) { this.drafts.delete(id); this.changed(); return; }
      this.saving = true; this.changed();
      try {
        const result = await this.transport.apply(...this.context(), { kind: 'save', id, expected: block.content, content: draft.content }, draft.group);
        this.apply(result);
        if (this.drafts.get(id)?.generation === draft.generation) this.drafts.delete(id);
      } catch (error) { this.error = errorMessage(error); throw error; }
      finally { this.saving = false; this.changed(); }
    });
  }

  async flush(): Promise<void> {
    this.clearTimers();
    await this.tail;
    if (this.composing) throw new Error('Finish text composition before leaving the editor');
    if (this.error) throw new Error(this.error);
    while (this.drafts.size) for (const id of [...this.drafts.keys()]) await this.save(id);
    this.clearTimers();
  }

  async retry(): Promise<void> { this.error = ''; this.changed(); await this.flush(); }

  async deleteSubtree(id: string, confirm: (preview: DeletePreview) => Promise<boolean>): Promise<void> {
    if (this.working) return;
    this.working = true; this.changed();
    try {
      await this.flush();
      const preview = await this.enqueue(() => this.transport.prepareDelete(...this.context(), id));
      if (preview.requires_confirmation && !await confirm(preview)) { this.activate(id); return; }
      const result = await this.enqueue(() => this.transport.apply(...this.context(), { kind: 'delete', id, token: preview.token }, null));
      this.apply(result); this.groups.clear();
      if (result.focus_id) this.reveal(result.focus_id);
      else { this.activeId = null; this.focus = null; }
    } catch (error) { this.error = errorMessage(error); throw error; }
    finally { this.working = false; this.changed(); }
  }

  async discardAndReload(): Promise<void> {
    await this.tail;
    if (this.working || !this.pageId || !this.session) return;
    this.working = true; this.changed();
    try {
      const result = await this.transport.page(this.session, this.pageId);
      this.clearTimers(); this.drafts.clear(); this.groups.clear(); this.error = '';
      this.blocks = result.blocks; this.canUndo = result.can_undo; this.canRedo = result.can_redo;
      this.activeId = null; this.focus = null; this.cachedRows = null;
    } finally { this.working = false; this.changed(); }
  }

  async action(action: EditAction | { kind: 'undo' | 'redo' }, selection?: Selection): Promise<void> {
    if (this.working) return;
    this.working = true; this.changed();
    try {
      await this.flush();
      const result = await this.enqueue(() => action.kind === 'undo' || action.kind === 'redo'
        ? this.transport.history(...this.context(), action.kind === 'redo')
        : this.transport.apply(...this.context(), action as EditAction, null));
      this.apply(result); this.groups.clear();
      if (result.focus_id) {
        // History/moves may reveal a node inside a collapsed subtree.
        let parent = this.blocks.find(b => b.id === result.focus_id)?.parent_id;
        const seen = new Set<string>();
        while (parent && !seen.has(parent)) { seen.add(parent); this.collapsed.delete(parent); parent = this.blocks.find(b => b.id === parent)?.parent_id; }
        this.cachedRows = null;
        const caret = action.kind === 'split' || action.kind === 'append' ? { start: 0, end: 0 }
          : selection ?? this.selection(result.focus_id);
        this.activate(result.focus_id, caret);
      } else if (this.activeId && !this.blocks.some(b => b.id === this.activeId)) { this.activeId = null; this.focus = null; }
    } catch (error) { this.error = errorMessage(error); throw error; }
    finally { this.working = false; this.changed(); }
  }

  async toggle(id: string): Promise<void> {
    if (this.working) return;
    this.working = true; this.changed();
    try {
      await this.flush();
      if (this.collapsed.has(id)) this.collapsed.delete(id); else this.collapsed.add(id);
      this.cachedRows = null;
      if (this.activeId && !this.rows().some(row => row.block.id === this.activeId)) this.activate(id);
    } finally { this.working = false; this.changed(); }
  }

  navigate(id: string, direction: -1 | 1): void {
    const rows = this.rows(); const index = rows.findIndex(row => row.block.id === id);
    const target = rows[index + direction];
    if (target) { this.leave(id); const at = direction === -1 ? this.text(target.block.id).length : 0; this.activate(target.block.id, { start: at, end: at }); }
  }
}
