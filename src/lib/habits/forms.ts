import type { Habit, HabitDefinition, HabitEventInput, HabitKind, HabitSchedule, EventKind } from '../extensions/habits-contracts.ts';
import { parseInstant } from './time.ts';

export interface DefinitionDraft {
  name: string; kind: HabitKind; anchor: string; every: number; unit: HabitSchedule['unit']; mode: 'fixed' | 'after_completion'; zone: string;
}
export function definitionInput(d: DefinitionDraft, original: Habit | null): HabitDefinition {
  if (!d.name.trim() || d.name.includes('\0') || new TextEncoder().encode(d.name.trim()).length > 512) throw new Error('Enter a name (up to 512 UTF-8 bytes).');
  const kind = original?.definition.kind ?? d.kind;
  let schedule: HabitSchedule | null = null;
  if (kind === 'positive') {
    if (!Number.isInteger(d.every) || d.every < 1 || d.every > 10000) throw new Error('Interval must be between 1 and 10000.');
    if (!d.zone.trim() || d.zone.trim().length > 100 || !/^[A-Za-z0-9_+/-]+$/.test(d.zone.trim())) throw new Error('Choose an IANA timezone.');
    try { new Intl.DateTimeFormat('en', { timeZone: d.zone.trim() }); } catch { throw new Error('Choose an IANA timezone.'); }
    schedule = { anchor: parseInstant(d.anchor), every: d.every, unit: d.unit, mode: d.mode, timezone: d.zone.trim() };
  }
  return { name: d.name.trim(), kind, schedule, archived: original?.definition.archived ?? false };
}
export function eventInput(habit: Habit, id: string, kind: EventKind, time: string, start: string, end: string, now = Math.floor(Date.now() / 1000)): HabitEventInput {
  if (habit.definition.archived) throw new Error('Archived habits cannot accept new events.');
  const occurred_at = parseInstant(time);
  if (occurred_at > now) throw new Error('Future events are not allowed.');
  if (habit.definition.kind === 'positive') {
    if (kind !== 'completion' || occurred_at < habit.definition.schedule!.anchor) throw new Error('Completion must be on or after the first occurrence.');
  } else if (kind === 'completion') throw new Error('Negative habits accept lapse or confirmation only.');
  const period_start = kind === 'confirmation' ? parseInstant(start) : null;
  const period_end = kind === 'confirmation' ? parseInstant(end) : null;
  if (period_start !== null && period_end !== null && (period_start >= period_end || period_end > occurred_at)) throw new Error('Select a completed period ending no later than the event.');
  return { id, habit_id: habit.id, kind, occurred_at, period_start, period_end };
}
