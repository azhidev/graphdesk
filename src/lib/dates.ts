import { toJalaali, toGregorian, isValidJalaaliDate } from 'jalaali-js';

/** Journal identity is a local civil day, never a UTC timestamp. */
export function localDate(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

/** Canonical civil-day identity. Calendar presentation must never become a page key. */
export type CivilDate = string;
export type Calendar = 'gregorian' | 'jalali';
export interface DateFields { year: number; month: number; day: number }
export class CalendarError extends Error {
  constructor(publicCode: 'invalid' | 'unsupported') { super(publicCode); this.name = publicCode; }
}
export interface CalendarPresentation {
  label(day: CivilDate): string;
}

function civil(day: CivilDate): Date {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) throw new CalendarError('invalid');
  const [year, month, date] = day.split('-').map(Number);
  const value = new Date(0);
  value.setUTCHours(12, 0, 0, 0); value.setUTCFullYear(year, month - 1, date);
  if (value.toISOString().slice(0, 10) !== day) throw new CalendarError('invalid');
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

export function asciiDigits(value: string): string {
  return value.replace(/[۰-۹٠-٩]/g, c => String(c.charCodeAt(0) - (c >= '۰' ? 0x6f0 : 0x660)));
}

export function dateNumber(value: string): number {
  const text = asciiDigits(value.trim());
  if (!/^\d{1,4}$/.test(text)) throw new CalendarError('invalid');
  return Number(text);
}

function jalaliRange(day: CivilDate) {
  if (day < '1800-01-01' || day > '2256-12-31') throw new CalendarError('unsupported');
}

export function calendarFields(day: CivilDate, calendar: Calendar): DateFields {
  civil(day);
  const [year, month, date] = day.split('-').map(Number);
  if (calendar === 'gregorian') return { year, month, day: date };
  jalaliRange(day);
  const { jy, jm, jd } = toJalaali(year, month, date);
  return { year: jy, month: jm, day: jd };
}

export function calendarDate(fields: DateFields, calendar: Calendar): CivilDate {
  let { year, month, day } = fields;
  if (![year, month, day].every(Number.isInteger)) throw new CalendarError('invalid');
  if (calendar === 'jalali') {
    if (year < -61 || year > 3177) throw new CalendarError('unsupported');
    if (!isValidJalaaliDate(year, month, day)) throw new CalendarError('invalid');
    const converted = toGregorian(year, month, day);
    year = converted.gy; month = converted.gm; day = converted.gd;
  }
  const iso = `${String(year).padStart(4, '0')}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
  civil(iso);
  if (calendar === 'jalali') jalaliRange(iso);
  return iso;
}

export function dateLabel(day: CivilDate, calendar: Calendar, language: 'en' | 'fa'): string {
  const fields = calendarFields(day, calendar);
  const label = calendar === 'gregorian' ? day : `${fields.year}/${String(fields.month).padStart(2, '0')}/${String(fields.day).padStart(2, '0')}`;
  return language === 'fa' ? label.replace(/\d/g, d => '۰۱۲۳۴۵۶۷۸۹'[Number(d)]) : label;
}
