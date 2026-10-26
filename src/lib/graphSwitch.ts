/** A small staged desktop handoff, not a general operation/command framework.
 * The source stays installed until activation; install must synchronously match
 * the acknowledged backend session before any optional follow-up work. */
export class GraphSwitch<Ticket extends { ticket: string }, Session, Request> {
  private generation = 0;
  private pending = false;
  private transport: {
    prepare(request: Request): Promise<Ticket>;
    activate(ticket: string, source: string | null): Promise<Session>;
    reconcile(ticket: string): Promise<Session | null>;
    cancel(ticket: string): Promise<void>;
  };
  constructor(transport: GraphSwitch<Ticket, Session, Request>['transport']) { this.transport = transport; }
  invalidate(): void { this.generation++; }
  async open(request: Request, source: string | null, hooks: {
    flush(): Promise<void>; check(): void; detach(): Promise<void>; install(session: Session): void;
  }): Promise<void> {
    if (this.pending) throw new Error('Wait for the current operation');
    this.pending = true;
    const generation = ++this.generation;
    let ticket: Ticket | undefined;
    let activated = false;
    const check = () => {
      if (generation !== this.generation) throw new Error('The graph operation changed. Try again.');
      hooks.check();
    };
    try {
      await hooks.flush(); check();
      ticket = await this.transport.prepare(request); check();
      await hooks.detach(); check();
      let session: Session;
      try { session = await this.transport.activate(ticket.ticket, source); }
      catch (error) {
        // Never replay activation after an uncertain response. Only an exact
        // receipt for this ticket and the still-current session may reconcile.
        const receipt = await this.transport.reconcile(ticket.ticket);
        if (receipt === null) throw error;
        session = receipt;
      }
      activated = true;
      // Activation is authoritative, even if a later landing/registry read fails.
      hooks.install(session);
    } finally {
      if (ticket && !activated) await this.transport.cancel(ticket.ticket).catch(() => {});
      this.pending = false;
    }
  }
}
