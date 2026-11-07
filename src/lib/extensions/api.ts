import { invoke } from '@tauri-apps/api/core';
import type { ExtensionTransport } from './contracts';

/** Shell-only adapter. Bundled extension modules import contracts, never this. */
export const extensionTransport: ExtensionTransport = {
  state: session => invoke('extension_state', { session }),
  configure: (session, extensionId, enabled) => invoke('extension_configure', { session, extensionId, enabled }),
  request: (session, extensionId, apiVersion, request) => invoke('extension_request', { session, extensionId, apiVersion, request }),
};
