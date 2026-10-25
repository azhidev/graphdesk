<script lang="ts">
  import { preferences, tr, uiError } from './preferences';
  let { message }: { message: string } = $props();
</script>
<div role="alert">
  {$uiError(message)}
  {#if $preferences.language !== 'en' && $uiError(message) !== message}
    <details><summary>{$tr('Technical details')}</summary><pre dir="ltr">{message}</pre></details>
  {/if}
</div>
<style>div { color: #a01818; overflow-wrap: anywhere; } pre { white-space: pre-wrap; }</style>
