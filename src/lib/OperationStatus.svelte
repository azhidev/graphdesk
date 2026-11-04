<script lang="ts">
  import { tr } from './preferences';
  import type { MessageKey } from './i18n';
  let { label, scope, region }: { label: MessageKey | ''; scope: 'shell' | 'content'; region: 'shell' | 'content' } = $props();
</script>

{#if region === 'content'}
  <!-- Reserve local feedback space rather than inserting/removing a shell row. -->
  <div class="content-operation" role="status">{scope === region && label ? $tr(label) : ''}</div>
{:else if scope === region && label}
  <p role="status">{$tr(label)}</p>
{/if}

<style>
  .content-operation { height: 28px; padding: 5px 16px; color: var(--muted); font-size: 12px;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
