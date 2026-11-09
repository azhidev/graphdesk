import type { Definition } from './contracts.ts';

/** Trusted bundled definition; interactive rendering remains shell-owned. */
export const habits: Definition = {
  manifest: {
    id: 'graphdesk.habits', version: '0.1.0', apiVersion: 1,
    name: { en: 'Habits', fa: 'عادت‌ها' },
    description: { en: 'Simple habits, schedules and retained event history.', fa: 'عادت‌های ساده، برنامه‌ها و تاریخچهٔ ماندگار رویدادها.' },
    capabilities: ['habits.core', 'ui.views', 'ui.navigation'],
  },
  async activate(api) {
    // A read validates the graph capability; activation never creates events.
    const response = await api.habits.request({ op: 'list', after: null, limit: 1 });
    if (response.type !== 'habits') throw new Error('Unexpected Habits API response');
    api.ui.setView({ id: 'habits', title: { en: 'Habits', fa: 'عادت‌ها' }, description: { en: 'Habits', fa: 'عادت‌ها' }, actions: [] });
    api.ui.setNavigation([{ id: 'habits', viewId: 'habits', label: { en: 'Habits', fa: 'عادت‌ها' } }]);
    return { async invoke() { throw new Error('Habits actions are host-rendered'); }, dispose() {} };
  },
};
