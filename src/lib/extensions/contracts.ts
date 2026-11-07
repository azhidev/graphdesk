/** Internal M9 protocol, not a public SDK. All payloads are JSON-compatible.
 * A future isolated runtime can proxy these methods without exposing UI stores. */
export const EXTENSION_API_VERSION = 1;
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type Localized = { en: string; fa: string };
export type Capability = 'storage.document' | 'ui.views' | 'ui.navigation';
export interface Manifest {
  id: string; version: string; apiVersion: number; name: Localized; description: Localized;
  capabilities: readonly Capability[];
}
export interface ExtensionDocument { revision: number; value: Json }
export interface ViewModel {
  id: string; title: Localized; description: Localized; value?: string;
  actions: { id: string; label: Localized }[];
}
export interface NavigationEntry { id: string; viewId: string; label: Localized }
export interface ExtensionAPI {
  readonly version: 1;
  storage: {
    read(): Promise<ExtensionDocument>;
    write(expectedRevision: number, value: Json): Promise<ExtensionDocument>;
  };
  ui: {
    setView(view: ViewModel): void;
    setNavigation(entries: NavigationEntry[]): void;
  };
}
export interface ExtensionInstance {
  invoke(action: string): Promise<void>;
  /** Synchronous teardown; host revokes capabilities before calling this. */
  dispose(): void;
}
export interface Definition {
  manifest: Manifest;
  activate(api: ExtensionAPI): Promise<ExtensionInstance>;
}
export type StorageRequest = { op: 'storage_read' }
  | { op: 'storage_write'; expected_revision: number; value: Json };
export interface ExtensionTransport {
  state(session: string): Promise<{ graph_id: string; extensions: { id: string; enabled: boolean }[] }>;
  configure(session: string, extensionId: string, enabled: boolean): Promise<void>;
  request(session: string, extensionId: string, apiVersion: number, request: StorageRequest): Promise<ExtensionDocument>;
}
