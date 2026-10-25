/** Journal identity is a local civil day, never a UTC timestamp. */
export function localDate(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

/** Canonical civil-day identity. Calendar presentation must never become a page key. */
export type CivilDate = string;
export interface CalendarPresentation {
  label(day: CivilDate): string;
}

function civil(day: CivilDate): Date {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) throw new Error('Invalid civil date');
  const [year, month, date] = day.split('-').map(Number);
  const value = new Date(0);
  value.setUTCHours(12, 0, 0, 0); value.setUTCFullYear(year, month - 1, date);
  if (value.toISOString().slice(0, 10) !== day) throw new Error('Invalid civil date');
  return value;
}

/** UTC is used only for arithmetic over civil fields, never for today's identity. */
export function adjacentDay(day: CivilDate, delta: number): CivilDate {
  const value = civil(day); value.setUTCDate(value.getUTCDate() + delta);
  const result = value.toISOString().slice(0, 10); civil(result); return result;
}

export const gregorianPresentation: CalendarPresentation = {
  label(day) { civil(day); return day; },
};
