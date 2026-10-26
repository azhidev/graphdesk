import type { Block, EditAction, EditResult, EditorTransport, DeletePreview, RecoveryTransport } from './types.ts';
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
  private compositionEpoch = 0;
  error = '';
  canUndo = false;
  canRedo = false;
  revisions: Record<string, number> = {};
  protectionError = '';
  conflicts: { block: Block; revision: number }[] = [];
  private uncertain = new Map<string, { content: string; revision: number }>();
  private captured = new Map<string, number>();
  private captureTimer?: ReturnType<typeof setTimeout>;
  private captureTail = Promise.resolve();
  private recovery?: RecoveryTransport;
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
  private timing: { debounce: number; maxWait: number; groupGap: number; ackTimeout?: number };
  constructor(transport: EditorTransport, changed: () => void = () => {},
    timing: { debounce: number; maxWait: number; groupGap: number; ackTimeout?: number } = { debounce: 300, maxWait: 1000, groupGap: 1000 }, recovery?: RecoveryTransport) {
    this.transport = transport; this.changed = changed; this.timing = timing;
    this.recovery = recovery;
  }

  get dirty(): boolean { return this.drafts.size > 0; }
  /** Graph handoff also guards a complete composition cycle during candidate IO. */
  departureStamp(): string { return JSON.stringify([this.session, this.compositionEpoch, this.generation]); }
  assertDeparture(stamp: string): void {
    this.guardComposition();
    if (stamp !== this.departureStamp() || this.dirty || this.saving || this.working) throw new Error('Editor input changed during graph opening. Try again after saving.');
  }
  get status(): string { return this.error ? 'Error — unsaved changes may remain' : this.composing ? 'Composing — finish input before leaving' : this.saving ? 'Saving…' : this.dirty ? 'Unsaved' : 'Saved'; }
  get protection(): string {
    if (this.protectionError) return this.protectionError;
    if (this.composing) return 'Composition is not recovery-protected until finalized';
    if (!this.dirty) return 'No pending text drafts';
    return [...this.drafts].every(([id, d]) => this.captured.get(id) === d.generation) ? 'Draft recovery snapshot stored locally' : 'Recovery snapshot pending';
  }
  rows(): Row[] { return this.cachedRows ??= visibleRows(this.blocks, this.collapsed); }
  text(id: string): string { return this.drafts.get(id)?.content ?? this.blocks.find(b => b.id === id)?.content ?? ''; }
  selection(id: string): Selection { return this.selections.get(id) ?? { start: this.text(id).length, end: this.text(id).length }; }
  remember(id: string, selection: Selection): void { this.selections.set(id, selection); }
  activate(id: string, selection = this.selection(id)): void {
    if (this.composing && id !== this.activeId) return;
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
    if (this.dirty || this.saving || this.working || this.composing) throw new Error('Flush before changing graph');
    this.clearTimers(); this.groups.clear(); this.selections.clear();
    this.session = session; this.pageId = null; this.blocks = []; this.collapsed.clear();
    this.activeId = null; this.focus = null; this.canUndo = false; this.canRedo = false;
    this.error = ''; this.cachedRows = null; this.changed();
    this.revisions = {}; this.uncertain.clear(); this.conflicts = []; this.captured.clear(); this.protectionError = '';
  }

  private guardComposition(epoch?: number): void {
    if (this.composing || (epoch !== undefined && epoch !== this.compositionEpoch)) throw new Error('Finish text composition before leaving the editor');
  }

  async load(pageId: string): Promise<void> {
    this.guardComposition();
    const epoch = this.compositionEpoch;
    if (this.working) throw new Error('Wait for the current editor operation');
    this.working = true; this.changed();
    try {
      await this.flush();
      this.guardComposition(epoch);
      if (!this.session) throw new Error('Open a graph first');
      const result = await this.transport.page(this.session, pageId);
      this.guardComposition(epoch);
      this.pageId = pageId; this.blocks = result.blocks;
      this.revisions = result.revisions ?? {}; this.conflicts = []; this.uncertain.clear();
      this.canUndo = result.can_undo; this.canRedo = result.can_redo;
      this.activeId = null; this.focus = null; this.groups.clear(); this.cachedRows = null;
    } finally { this.working = false; this.changed(); }
  }

  /** Detach an acknowledged page for a date-only journal preview. No graph writes. */
  async unload(): Promise<void> {
    this.guardComposition();
    const epoch = this.compositionEpoch;
    if (this.working) throw new Error('Wait for the current editor operation');
    this.working = true; this.changed();
    try {
      await this.flush(); this.guardComposition(epoch);
      this.pageId = null; this.blocks = []; this.revisions = {};
      this.activeId = null; this.focus = null; this.canUndo = false; this.canRedo = false;
      this.cachedRows = null; this.groups.clear();
    } finally { this.working = false; this.changed(); }
  }

  setDraft(id: string, content: string): void {
    if ((this.working && !this.composing) || !this.blocks.some(b => b.id === id)) return;
    const now = Date.now();
    let group = this.groups.get(id);
    if (!group || now - group.at > this.timing.groupGap) group = { id: crypto.randomUUID(), at: now };
    group.at = now; this.groups.set(id, group);
    this.drafts.set(id, { content, generation: ++this.generation, firstAt: this.drafts.get(id)?.firstAt ?? now, group: group.id });
    this.schedule(id); this.scheduleCapture(); this.changed();
  }

  composition(active: boolean): void {
    if (active && !this.composing) this.compositionEpoch++;
    this.composing = active;
    if (active) this.clearTimers(); else { for (const id of this.drafts.keys()) this.schedule(id); this.scheduleCapture(); }
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
  private scheduleCapture(): void {
    if (!this.recovery || this.composing || this.captureTimer) return;
    this.captureTimer = setTimeout(() => { this.captureTimer = undefined; void this.capture().catch(() => {}); }, 700);
  }
  async capture(): Promise<void> {
    if (this.captureTimer) clearTimeout(this.captureTimer);
    this.captureTimer = undefined;
    if (!this.recovery || this.composing) return;
    const work = this.captureTail.then(async () => {
      if (this.composing) return;
      for (const [id, draft] of [...this.drafts]) {
        if (this.captured.get(id) === draft.generation) continue;
        const block = this.blocks.find(b => b.id === id);
        if (!block) continue;
        if (block.content === draft.content && !this.uncertain.has(id)) { this.captured.set(id, draft.generation); continue; }
        try {
          await this.recovery!.put(...this.context(), id, block.content, this.revisions[id] ?? 0, draft.content);
          this.captured.set(id, draft.generation); this.protectionError = '';
        } catch { this.protectionError = 'Recovery storage failed — unsaved text is not protected. Keep the app open and retry recovery.'; throw new Error(this.protectionError); }
        finally { this.changed(); }
      }
      this.protectionError = ''; this.changed();
    });
    this.captureTail = work.catch(() => {}); return work;
  }
  private clearRecovery(id: string, content: string): void {
    if (!this.recovery) return;
    const session = this.session!;
    this.captureTail = this.captureTail.then(() => this.recovery!.clear(session, id, content)).catch(() => {
      this.protectionError = 'Recovery cleanup failed — an older snapshot may appear on reopen.'; this.changed();
    });
  }
  private enqueue<T>(work: () => Promise<T>): Promise<T> {
    const next = this.tail.then(work);
    this.tail = next.then(() => {}, () => {});
    return next;
  }
  private async acknowledge<T>(request: Promise<T>): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      return await Promise.race([request, new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error('Text save acknowledgement timed out. Draft retained; Retry compares persisted state.')), this.timing.ackTimeout ?? 10000);
      })]);
    } finally { if (timer) clearTimeout(timer); }
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
    Object.assign(this.revisions, result.revisions ?? {});
  }

  private save(id: string): Promise<void> {
    return this.enqueue(async () => {
      if (this.error) throw new Error(this.error);
      if (this.composing) return;
      const draft = this.drafts.get(id);
      if (!draft) return;
      const block = this.blocks.find(b => b.id === id);
      if (!block) throw new Error('Draft block is missing; reload explicitly to discard');
      if (block.content === draft.content) { this.drafts.delete(id); this.uncertain.delete(id); this.clearRecovery(id, draft.content); this.changed(); return; }
      this.saving = true; this.changed();
      try {
        const revision = this.revisions[id] ?? 0;
        this.uncertain.set(id, { content: draft.content, revision });
        const result = await this.acknowledge(this.transport.apply(...this.context(), { kind: 'save_text', id, expected: block.content, revision, content: draft.content }, draft.group));
        this.apply(result);
        this.uncertain.delete(id); this.clearRecovery(id, draft.content);
        if (this.drafts.get(id)?.generation === draft.generation) this.drafts.delete(id);
      } catch (error) { this.error = errorMessage(error); throw error; }
      finally { this.saving = false; this.changed(); }
    });
  }

  async flush(): Promise<void> {
    this.guardComposition();
    const epoch = this.compositionEpoch;
    this.clearTimers();
    await this.tail;
    this.guardComposition(epoch);
    await this.capture().catch(() => {}); // A working canonical save is still safer than keeping text only in RAM.
    this.guardComposition(epoch);
    if (this.error) throw new Error(this.error);
    while (this.drafts.size) {
      this.guardComposition(epoch);
      for (const id of [...this.drafts.keys()]) { this.guardComposition(epoch); await this.save(id); }
    }
    this.clearTimers();
    await this.captureTail;
    this.guardComposition(epoch);
  }

  async retry(): Promise<void> {
    this.guardComposition();
    if (this.working) return;
    await this.tail;
    this.guardComposition();
    if (this.uncertain.size) {
      const remote = await this.transport.page(...this.context());
      this.guardComposition();
      this.conflicts = [];
      for (const [id, sent] of this.uncertain) {
        const stored = remote.blocks.find(b => b.id === id);
        const old = this.blocks.find(b => b.id === id);
        const revision = remote.revisions?.[id] ?? 0;
        if (stored && ((stored.content === sent.content && revision === sent.revision + 1)
            || (stored.content === old?.content && revision === sent.revision))) {
          if (stored.content === sent.content && revision === sent.revision + 1) this.clearRecovery(id, sent.content);
          this.blocks = this.blocks.map(b => b.id === id ? stored : b); this.revisions[id] = revision; this.cachedRows = null;
          this.uncertain.delete(id);
        } else if (stored) this.conflicts.push({ block: stored, revision });
        else { this.error = 'The edited block was deleted. Your text remains in the draft/recovery store; compare or copy it before discarding.'; this.changed(); throw new Error(this.error); }
      }
      if (this.conflicts.length) { this.error = 'Persisted text changed. Compare both versions below before replacing it.'; this.changed(); throw new Error(this.error); }
    }
    this.error = ''; this.changed(); await this.flush();
  }

  async resolveConflict(id: string): Promise<void> {
    this.guardComposition();
    if (this.working) return;
    const conflict = this.conflicts.find(c => c.block.id === id);
    if (!conflict) return;
    this.blocks = this.blocks.map(b => b.id === id ? conflict.block : b);
    this.revisions[id] = conflict.revision; this.uncertain.delete(id);
    this.conflicts = this.conflicts.filter(c => c.block.id !== id);
    if (!this.conflicts.length) { this.error = ''; await this.flush(); }
    this.changed();
  }

  async deleteSubtree(id: string, confirm: (preview: DeletePreview) => Promise<boolean>): Promise<void> {
    this.guardComposition();
    const epoch = this.compositionEpoch;
    if (this.working) return;
    this.working = true; this.changed();
    try {
      await this.flush();
      const preview = await this.enqueue(() => this.transport.prepareDelete(...this.context(), id));
      this.guardComposition(epoch);
      if (preview.requires_confirmation && !await confirm(preview)) { this.activate(id); return; }
      const result = await this.enqueue(() => { this.guardComposition(epoch); return this.transport.apply(...this.context(), { kind: 'delete', id, token: preview.token }, null); });
      this.apply(result); this.groups.clear();
      if (result.focus_id) this.reveal(result.focus_id);
      else { this.activeId = null; this.focus = null; }
    } catch (error) { this.error = errorMessage(error); throw error; }
    finally { this.working = false; this.changed(); }
  }

  async discardAndReload(): Promise<void> {
    this.guardComposition();
    const epoch = this.compositionEpoch;
    await this.tail;
    this.guardComposition(epoch);
    if (this.working || !this.pageId || !this.session) return;
    this.working = true; this.changed();
    try {
      const result = await this.transport.page(this.session, this.pageId);
      this.guardComposition(epoch);
      this.clearTimers(); this.drafts.clear(); this.groups.clear(); this.error = '';
      this.blocks = result.blocks; this.canUndo = result.can_undo; this.canRedo = result.can_redo;
      this.revisions = result.revisions ?? {}; this.uncertain.clear(); this.conflicts = [];
      this.activeId = null; this.focus = null; this.cachedRows = null;
    } finally { this.working = false; this.changed(); }
  }

  async action(action: EditAction | { kind: 'undo' | 'redo' }, selection?: Selection): Promise<void> {
    this.guardComposition();
    const epoch = this.compositionEpoch;
    if (this.working) return;
    this.working = true; this.changed();
    try {
      await this.flush();
      const result = await this.enqueue(() => { this.guardComposition(epoch); return action.kind === 'undo' || action.kind === 'redo'
        ? this.transport.history(...this.context(), action.kind === 'redo')
        : this.transport.apply(...this.context(), action as EditAction, null); });
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
    this.guardComposition();
    const epoch = this.compositionEpoch;
    if (this.working) return;
    this.working = true; this.changed();
    try {
      await this.flush();
      this.guardComposition(epoch);
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
