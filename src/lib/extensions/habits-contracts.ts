/** Headless Habits API v1. Instants are integer Unix seconds UTC, not locale
 * strings. Consumers must reuse IDs on retry and compare mutation revisions. */
export type HabitKind = 'positive' | 'negative';
export type EventKind = 'completion' | 'lapse' | 'confirmation';
export interface HabitSchedule {
  anchor: number; every: number; unit: 'hours' | 'days' | 'weeks' | 'months' | 'years';
  mode?: 'fixed' | 'after_completion';
  /** New create: omit/null to snapshot Settings/system. Updates preserve the
   * existing zone when omitted. Legacy stored definitions without this are UTC. */
  timezone?: string | null;
}
export interface HabitDefinition { name: string; kind: HabitKind; schedule: HabitSchedule | null; archived?: boolean }
export interface Habit { id: string; revision: number; definition: HabitDefinition }
export interface HabitCreate { id: string; definition: HabitDefinition }
export interface HabitEventInput {
  id: string; habit_id: string;
  /** Omission defaults to lapse, suitable for negative habits only. */
  kind?: EventKind; occurred_at: number; period_start: number | null; period_end: number | null;
}
export interface HabitEvent {
  id: string; habit_id: string; kind: EventKind; occurred_at: number; recorded_at: number;
  period_start: number | null; period_end: number | null; revision: number; voided: boolean;
}
export interface EventCursor { occurred_at: number; id: string }
export interface HabitStatus {
  habit_id: string;
  positive: { due: boolean; occurrence: number; next_occurrence: number } | null;
  /** Explicit reports, never inference that an unreported period was successful. */
  negative: { confirmation: HabitEvent | null; last_lapse: HabitEvent | null } | null;
}
export type HabitsRequest =
  | { op: 'create'; input: HabitCreate }
  | { op: 'update'; id: string; expected_revision: number; definition: HabitDefinition }
  | { op: 'get'; id: string }
  | { op: 'list'; after: string | null; limit: number }
  | { op: 'record'; input: HabitEventInput }
  | { op: 'correct'; event_id: string; expected_revision: number; voided: boolean }
  | { op: 'events'; habit_id: string; before: EventCursor | null; limit: number }
  | { op: 'status'; id: string; now: number };
export type HabitsResponse =
  | { type: 'habit'; data: Habit }
  | { type: 'habits'; data: Habit[] }
  | { type: 'event'; data: HabitEvent }
  | { type: 'events'; data: HabitEvent[] }
  | { type: 'status'; data: HabitStatus };
