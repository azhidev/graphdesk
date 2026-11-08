import { ExtensionRegistry } from './host';
import { tally } from './tally';
import { habits } from './habits';

export function bundledExtensions(): ExtensionRegistry {
  const registry = new ExtensionRegistry();
  registry.register(tally);
  registry.register(habits);
  return registry;
}
