<script lang="ts">
  interface Props {
    id?: string;
    value?: string;
  }

  let { id = "", value = $bindable("") }: Props = $props();
</script>

<input {id} type="text" bind:value />

<style>
  input {
    border-radius: 3px;
    border: 1px solid var(--color-border-interactive-muted, #2b659b);
    background: var(--color-background-base-default, #101923);
    color: var(--color-text-base-default, #ffffff);
  }
</style>
