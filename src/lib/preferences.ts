import { writable, derived } from 'svelte/store';
import { translate, errorText, type Language } from './i18n';
import type { Calendar } from './dates';

export interface Preferences { language: Language; calendar: Calendar; search_mode: 'original' | 'persian'; theme: 'system' | 'light' | 'dark'; habit_timezone: string | null }
export const defaults: Preferences = { language: 'en', calendar: 'gregorian', search_mode: 'original', theme: 'system', habit_timezone: null };
export const preferences = writable<Preferences>({ ...defaults });
export const tr = derived(preferences, p => (key: Parameters<typeof translate>[1], values?: Record<string, string | number>) => translate(p.language, key, values));
export const uiError = derived(preferences, p => (message: string) => errorText(p.language, message));
