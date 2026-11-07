import { ExtensionRegistry } from './host';
import { tally } from './tally';

export function bundledExtensions(): ExtensionRegistry {
  const registry = new ExtensionRegistry();
  registry.register(tally);
  return registry;
}
