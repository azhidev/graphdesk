import { dateLabel, type Calendar } from '../dates.ts';

/** Explicit offsets only. Never parse a zone's civil time in the machine zone.
 * Explicit instants preserve either fold side, exactly as core anchors do. */
export function parseInstant(text: string): number {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2})?(?:Z|[+-]\d{2}:\d{2})$/.test(text)) throw new Error('Use a Gregorian timestamp with Z or an explicit offset.');
  const [day, clock] = text.split('T');
  const [y, m, d] = day.split('-').map(Number);
  const check = new Date(0); check.setUTCFullYear(y, m - 1, d);
  if (check.toISOString().slice(0, 10) !== day || Number(clock.slice(0, 2)) > 23 || Number(clock.slice(3, 5)) > 59 || (clock[5] === ':' && Number(clock.slice(6, 8)) > 59)) throw new Error('Enter a valid date.');
  const value = Date.parse(text) / 1000;
  if (!Number.isSafeInteger(value) || value < -62135596800 || value > 253402300799) throw new Error('Enter a valid date.');
  return value;
}
export const instantInput = (seconds = Math.floor(Date.now() / 1000)) => new Date(seconds * 1000).toISOString().slice(0, 19) + 'Z';
export function zonedLabel(seconds: number, zone: string, calendar: Calendar, language: 'en' | 'fa'): string {
  const parts = new Intl.DateTimeFormat('en-CA-u-ca-gregory-nu-latn', { timeZone: zone, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).formatToParts(new Date(seconds * 1000));
  const part = (name: string) => parts.find(p => p.type === name)!.value;
  const day = `${part('year').padStart(4, '0')}-${part('month')}-${part('day')}`;
  let label = day;
  try { label = dateLabel(day, calendar, language); } catch { /* Out-of-Jalali range: explicit Gregorian fallback. */ }
  return `${label} · ${part('hour')}:${part('minute')}:${part('second')} · ${zone}`;
}
