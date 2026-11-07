import type { Definition, ExtensionDocument } from './contracts.ts';

const name = { en: 'Tally', fa: 'شمارنده' };
const description = { en: 'A small counter saved in this graph.', fa: 'شمارنده‌ای کوچک که در این گراف ذخیره می‌شود.' };
/** Proof of the host contract: no Svelte, IPC, filesystem or editor imports. */
export const tally: Definition = {
  manifest: { id: 'graphdesk.tally', version: '1.0.0', apiVersion: 1, name, description,
    capabilities: ['storage.document', 'ui.views', 'ui.navigation'] },
  async activate(api) {
    let document: ExtensionDocument = await api.storage.read();
    let disposed = false;
    function count(): number {
      if (document.value === null) return 0;
      const value = document.value;
      if (typeof value !== 'object' || Array.isArray(value) || value.schema !== 1
          || !Number.isSafeInteger(value.count) || typeof value.count !== 'number' || value.count < 0) throw new Error('Unsupported Tally data; retained without changes');
      return value.count;
    }
    const publish = () => api.ui.setView({ id: 'counter', title: name, description, value: String(count()), actions: [
      { id: 'increment', label: { en: 'Add one', fa: 'یکی اضافه کن' } },
      { id: 'decrement', label: { en: 'Subtract one', fa: 'یکی کم کن' } },
      { id: 'reset', label: { en: 'Reset count', fa: 'صفر کردن شمارنده' } },
    ] });
    publish(); api.ui.setNavigation([{ id: 'tally', viewId: 'counter', label: name }]);
    return {
      async invoke(action) {
        if (disposed) throw new Error('Tally context revoked');
        const current = count();
        const next = action === 'increment' ? current + 1 : action === 'decrement' ? Math.max(0, current - 1) : action === 'reset' ? 0 : NaN;
        if (!Number.isSafeInteger(next)) throw new Error('Invalid Tally count');
        document = await api.storage.write(document.revision, { schema: 1, count: next });
        if (!disposed) publish();
      },
      dispose() { disposed = true; },
    };
  },
};
