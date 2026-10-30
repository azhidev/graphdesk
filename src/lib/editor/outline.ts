import type { Block } from './types.ts';
export interface Row { block: Block; depth: number; hasChildren: boolean }

/** Presentation only; resolve the outline edge from acknowledged text, not drafts. */
export function blockDirection(content: string): 'ltr' | 'rtl' | undefined {
  const letter = content.match(/\p{Letter}/u)?.[0];
  return letter ? (/\p{Script=Arabic}|\p{Script=Hebrew}/u.test(letter) ? 'rtl' : 'ltr') : undefined;
}

/** Presentation only: mutations/ordering decisions are validated and applied in Rust. */
export function visibleRows(blocks: Block[], collapsed: Set<string>): Row[] {
  const children = new Map<string | null, Block[]>();
  for (const block of blocks) {
    const group = children.get(block.parent_id) ?? [];
    group.push(block); children.set(block.parent_id, group);
  }
  for (const group of children.values()) group.sort((a, b) => a.position - b.position || a.id.localeCompare(b.id));
  const result: Row[] = [];
  const stack = [...(children.get(null) ?? [])].reverse().map(block => ({ block, depth: 0 }));
  const seen = new Set<string>();
  while (stack.length) {
    const row = stack.pop()!;
    if (seen.has(row.block.id)) continue;
    seen.add(row.block.id);
    const descendants = children.get(row.block.id) ?? [];
    result.push({ ...row, hasChildren: descendants.length > 0 });
    if (!collapsed.has(row.block.id)) {
      for (let i = descendants.length - 1; i >= 0; i--) stack.push({ block: descendants[i], depth: row.depth + 1 });
    }
  }
  return result;
}
