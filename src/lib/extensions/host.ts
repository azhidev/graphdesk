import { EXTENSION_API_VERSION } from './contracts.ts';
import type { Capability, Definition, ExtensionAPI, ExtensionInstance, ExtensionTransport, Localized, Manifest, NavigationEntry, ViewModel } from './contracts.ts';

const extensionId = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+$/;
const localId = /^[a-z][a-z0-9-]{0,63}$/;
const capabilities = new Set(['storage.document', 'ui.views', 'ui.navigation']);
const message = (error: unknown) => error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
const localized = (value: unknown): value is Localized => !!value && typeof value === 'object'
  && ['en', 'fa'].every(key => typeof (value as Record<string, unknown>)[key] === 'string'
    && ((value as Record<string, string>)[key].length > 0 && (value as Record<string, string>)[key].length <= 512));

/** Registration has no graph access and never activates code. Bundled definitions
 * are trusted application code; this registry is NOT a third-party sandbox. */
export class ExtensionRegistry {
  private definitions = new Map<string, Definition>();
  register(definition: Definition): void {
    const m = definition.manifest;
    if (!extensionId.test(m.id) || m.id.length > 100 || m.apiVersion !== EXTENSION_API_VERSION
        || !/^\d+\.\d+\.\d+$/.test(m.version) || this.definitions.has(m.id)
        || m.capabilities.some(capability => !capabilities.has(capability))
        || !localized(m.name) || !localized(m.description)) {
      throw new Error('Invalid, duplicate or incompatible extension manifest');
    }
    this.definitions.set(m.id, { manifest: structuredClone(m), activate: definition.activate });
  }
  all(): Definition[] { return [...this.definitions.values()]; }
}

export interface ExtensionState {
  manifest: Manifest; enabled: boolean; status: 'disabled' | 'starting' | 'active' | 'error';
  error: string; pending: boolean; navigation: NavigationEntry[]; views: ViewModel[];
}
interface Slot extends ExtensionState { live: boolean; instance?: ExtensionInstance }

/** One graph-session lifetime. No editor/store/connection/path is given to an
 * extension. Capabilities are revoked synchronously on disable, error or switch. */
export class ExtensionHost {
  private generation = 0;
  private session: string | null = null;
  private slots = new Map<string, Slot>();
  private configuring = new Set<string>();
  loading = false;
  error = '';
  private registry: ExtensionRegistry;
  private transport: ExtensionTransport;
  private changed: () => void;
  constructor(registry: ExtensionRegistry, transport: ExtensionTransport, changed = () => {}) {
    this.registry = registry; this.transport = transport; this.changed = changed;
  }
  states(): ExtensionState[] {
    return [...this.slots.values()].map(({ manifest, enabled, status, error, pending, navigation, views }) =>
      structuredClone({ manifest, enabled, status, error, pending, navigation, views }));
  }
  navigation(): (NavigationEntry & { extensionId: string })[] {
    return [...this.slots.values()].filter(slot => slot.status === 'active')
      .flatMap(slot => slot.navigation.map(entry => ({ ...structuredClone(entry), extensionId: slot.manifest.id })));
  }
  view(extension: string, id: string): ViewModel | undefined {
    const slot = this.slots.get(extension);
    const view = slot?.status === 'active' ? slot.views.find(view => view.id === id) : undefined;
    return view && structuredClone(view);
  }
  private revoke(slot: Slot): void {
    slot.live = false; slot.navigation = []; slot.views = [];
    const instance = slot.instance; slot.instance = undefined;
    if (instance) { try { instance.dispose(); } catch (error) { slot.error = message(error); } }
  }
  detach(): void {
    this.generation++; this.session = null;
    for (const slot of this.slots.values()) this.revoke(slot);
    this.slots.clear(); this.configuring.clear(); this.loading = false; this.error = ''; this.changed();
  }
  async attach(session: string, graphId: string): Promise<void> {
    this.detach(); this.session = session;
    const generation = this.generation;
    for (const definition of this.registry.all()) this.slots.set(definition.manifest.id, {
      manifest: definition.manifest, enabled: false, status: 'disabled', pending: false,
      error: '', live: false, navigation: [], views: [],
    });
    this.loading = true; this.changed();
    try {
      const state = await this.transport.state(session);
      if (generation !== this.generation) return;
      if (state.graph_id !== graphId) throw new Error('Extension graph identity changed');
      await Promise.all(this.registry.all().map(async definition => {
        if (state.extensions.find(row => row.id === definition.manifest.id)?.enabled) {
          const slot = this.slots.get(definition.manifest.id)!;
          slot.enabled = true; await this.activate(definition, slot, generation);
        }
      }));
    } catch (error) { if (generation === this.generation) this.error = message(error); }
    finally { if (generation === this.generation) { this.loading = false; this.changed(); } }
  }
  private async activate(definition: Definition, slot: Slot, generation: number): Promise<void> {
    slot.live = true; slot.status = 'starting'; slot.error = ''; this.changed();
    const session = this.session!;
    const assertLive = (capability?: Capability) => {
      if (!slot.live || generation !== this.generation || this.slots.get(slot.manifest.id) !== slot) throw new Error('Extension context revoked');
      if (capability && !slot.manifest.capabilities.includes(capability)) throw new Error('Extension capability denied');
    };
    const request = async (payload: Parameters<ExtensionTransport['request']>[3]) => {
      assertLive('storage.document');
      const result = await this.transport.request(session, slot.manifest.id, EXTENSION_API_VERSION, structuredClone(payload));
      assertLive('storage.document');
      if (!Number.isSafeInteger(result.revision) || result.revision < 0) throw new Error('Unsupported extension storage revision');
      return structuredClone(result);
    };
    const api: ExtensionAPI = {
      version: EXTENSION_API_VERSION,
      storage: {
        read: () => request({ op: 'storage_read' }),
        write: (expectedRevision, value) => request({ op: 'storage_write', expected_revision: expectedRevision, value }),
      },
      ui: {
        setView: view => {
          assertLive('ui.views');
          if (!localId.test(view.id) || !localized(view.title) || !localized(view.description)
              || (view.value !== undefined && (typeof view.value !== 'string' || view.value.length > 200))
              || !Array.isArray(view.actions) || view.actions.length > 8 || new Set(view.actions.map(a => a.id)).size !== view.actions.length
              || view.actions.some(action => !localId.test(action.id) || !localized(action.label)) || JSON.stringify(view).length > 8192) throw new Error('Invalid extension view');
          if (!slot.views.some(v => v.id === view.id) && slot.views.length >= 4) throw new Error('Extension view limit');
          slot.views = [...slot.views.filter(v => v.id !== view.id), structuredClone(view)]; this.changed();
        },
        setNavigation: entries => {
          assertLive('ui.navigation');
          if (entries.length > 4 || new Set(entries.map(e => e.id)).size !== entries.length
              || entries.some(e => !localId.test(e.id) || !localized(e.label) || !slot.views.some(v => v.id === e.viewId))
              || JSON.stringify(entries).length > 8192) throw new Error('Invalid extension navigation');
          slot.navigation = structuredClone(entries); this.changed();
        },
      },
    };
    try {
      const instance = await definition.activate(api);
      if (!slot.live || generation !== this.generation) { try { instance.dispose(); } catch { /* Old graph; already revoked. */ } return; }
      slot.instance = instance; slot.status = 'active';
    } catch (error) {
      if (generation === this.generation && slot.live) { this.revoke(slot); slot.status = 'error'; slot.error = message(error); }
    } finally { if (generation === this.generation) this.changed(); }
  }
  async configure(id: string, enabled: boolean): Promise<void> {
    if (!this.session || this.loading || this.error || this.configuring.has(id)) return;
    const slot = this.slots.get(id), definition = this.registry.all().find(d => d.manifest.id === id);
    if (!slot || !definition || slot.pending) return;
    const generation = this.generation, session = this.session;
    this.configuring.add(id); slot.pending = true; slot.error = ''; this.changed();
    try {
      await this.transport.configure(session, id, enabled);
      if (generation !== this.generation) return;
      this.revoke(slot); slot.enabled = enabled; slot.status = 'disabled';
      if (enabled) await this.activate(definition, slot, generation);
    } catch (error) { if (generation === this.generation) slot.error = message(error); }
    finally { if (generation === this.generation) { this.configuring.delete(id); slot.pending = false; this.changed(); } }
  }
  async invoke(id: string, action: string): Promise<void> {
    const slot = this.slots.get(id);
    if (!slot || slot.status !== 'active' || slot.pending || this.configuring.has(id)) return;
    if (!slot.views.some(view => view.actions.some(a => a.id === action))) return;
    const generation = this.generation;
    slot.pending = true; this.changed();
    try { await slot.instance!.invoke(action); }
    catch (error) {
      if (generation === this.generation && slot.live) { this.revoke(slot); slot.status = 'error'; slot.error = message(error); }
    } finally { if (generation === this.generation) { slot.pending = false; this.changed(); } }
  }
}
