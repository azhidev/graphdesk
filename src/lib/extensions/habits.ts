import type { Definition } from './contracts.ts';

/** Headless first phase. No dashboard, events at activation, timers or reminders. */
export const habits: Definition = {
  manifest: {
    id: 'graphdesk.habits', version: '0.1.0', apiVersion: 1,
    name: { en: 'Habits', fa: 'عادت‌ها' },
    description: { en: 'Habit definitions, event history and recurrence. Core only; no dashboard yet.', fa: 'تعریف عادت‌ها، تاریخچهٔ رویدادها و تکرار. فعلاً فقط هسته، بدون پیشخوان.' },
    capabilities: ['habits.core'],
  },
  async activate(api) {
    // A read validates the graph capability; activation never creates events.
    const response = await api.habits.request({ op: 'list', after: null, limit: 1 });
    if (response.type !== 'habits') throw new Error('Unexpected Habits API response');
    return { async invoke() { throw new Error('Habits has no UI actions in this phase'); }, dispose() {} };
  },
};
