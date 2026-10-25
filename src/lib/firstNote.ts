import type { Block, Page } from './editor/types';

export interface PendingNote { id: string; graph_id: string; block_id: string; journal_date: string; content: string; version: number; updated_at: number }
export interface NoteState { block: Block | null; revision: number | null; deleted: boolean }
export interface NoteSubmission { page: Page; block: Block; revision: number }
export interface NoteTransport {
  put(session: string, id: string, blockId: string, date: string, content: string): Promise<PendingNote>;
  remove(session: string, id: string, version: number): Promise<void>;
  submit(session: string, date: string, blockId: string, content: string): Promise<NoteSubmission>;
}

/** Pending text uses the same recovery store, but has no canonical page before submit. */
export class FirstNoteController {
  content = '';
  composing = false;
  working = false;
  protected = false;
  error = '';
  id: string;
  blockId: string;
  private version = 0;
  private generation = 0;
  private captured = -1;
  private epoch = 0;
  private timer?: ReturnType<typeof setTimeout>;
  private tail = Promise.resolve();
  readonly session: string;
  readonly date: string;
  private transport: NoteTransport;
  private changed: () => void;
  private delay: number;
  private ackTimeout: number;
  constructor(session: string, date: string, transport: NoteTransport,
    changed = () => {}, record?: PendingNote, delay = 700, ackTimeout = 10000) {
    this.session = session; this.date = date; this.transport = transport;
    this.changed = changed; this.delay = delay; this.ackTimeout = ackTimeout;
    this.id = record?.id ?? crypto.randomUUID(); this.blockId = record?.block_id ?? crypto.randomUUID();
    if (record) { this.content = record.content; this.version = record.version; this.captured = 0; this.protected = true; }
  }
  input(text: string) {
    if (this.working && !this.composing) return;
    this.content = text; this.generation++; this.protected = false;
    this.schedule(); this.changed();
  }
  composition(active: boolean) {
    if (active && !this.composing) this.epoch++;
    this.composing = active;
    if (active) { this.stopTimer(); this.protected = false; } else this.schedule();
    this.changed();
  }
  private schedule() {
    if (!this.composing && !this.timer) this.timer = setTimeout(() => { this.timer = undefined; void this.capture().catch(() => {}); }, this.delay);
  }
  private stopTimer() { if (this.timer) clearTimeout(this.timer); this.timer = undefined; }
  private guard(epoch = this.epoch) { if (this.composing || epoch !== this.epoch) throw new Error('Finish text composition before leaving the editor'); }
  get stamp() { return `${this.epoch}:${this.generation}`; }
  assertStable(stamp: string) { this.guard(); if (stamp !== this.stamp) throw new Error('Finish text composition before leaving the editor'); }
  async capture() {
    this.stopTimer();
    const work = this.tail.then(async () => {
      if (this.composing || this.captured === this.generation) return;
      const generation = this.generation, content = this.content;
      try {
        const record = await this.transport.put(this.session, this.id, this.blockId, this.date, content);
        this.version = record.version; this.captured = generation;
        this.protected = generation === this.generation && !this.composing; this.error = '';
      } catch { this.error = 'Recovery storage failed — unsaved text is not protected. Keep the app open and retry recovery.'; this.protected = false; throw new Error(this.error); }
      finally { this.changed(); }
    });
    this.tail = work.catch(() => {}); return work;
  }
  async flush() { this.guard(); const epoch = this.epoch; await this.capture(); this.guard(epoch); }
  async discard() {
    this.guard(); const epoch = this.epoch;
    this.working = true; this.changed();
    try { await this.flush(); this.guard(epoch); await this.transport.remove(this.session, this.id, this.version); this.stopTimer(); }
    finally { this.working = false; this.changed(); }
  }
  async submit(): Promise<NoteSubmission> {
    this.guard();
    if (this.working || !this.content.trim()) throw new Error('Wait for the current operation');
    const epoch = this.epoch, generation = this.generation, content = this.content;
    this.working = true; this.changed();
    let timeout: ReturnType<typeof setTimeout> | undefined;
    try {
      await this.capture().catch(() => {}); this.guard(epoch);
      const result = await Promise.race([
        this.transport.submit(this.session, this.date, this.blockId, content),
        new Promise<never>((_, reject) => { timeout = setTimeout(() => reject(new Error('Text save acknowledgement timed out. Draft retained; Retry compares persisted state.')), this.ackTimeout); }),
      ]);
      this.guard(epoch);
      if (generation !== this.generation) throw new Error('Finish text composition before leaving the editor');
      // Only the exact captured generation may be cleared; cleanup failure keeps
      // the acknowledged intent reviewable and a retry is idempotent in Rust.
      if (this.version) await this.transport.remove(this.session, this.id, this.version);
      this.stopTimer(); return result;
    } finally { if (timeout) clearTimeout(timeout); this.working = false; this.changed(); }
  }
}
