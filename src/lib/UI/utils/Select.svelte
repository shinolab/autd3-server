<script lang="ts">
  interface Props {
    id?: string;
    value?: string;
    values?: readonly string[];
  }

  let { id = "", value = $bindable(""), values = [] }: Props = $props();
</script>

<div class="container">
  <select {id} bind:value>
    {#each values as key (key)}
      <option>{key}</option>
    {/each}
  </select>
</div>

<style>
  select {
    padding: 0px 4px;
    border: 1px solid var(--color-border-interactive-muted, #2b659b);
    background: var(--color-background-base-default, #101923);
    color: var(--color-text-base-default, #ffffff);
    width: 100%;
    white-space: nowrap;
  }
  .container {
    position: relative;
    width: 100%;
  }
  .container::before {
    border-top: 6px solid #4dacff;
    border-left: 4px solid transparent;
    border-right: 4px solid transparent;
    content: "";
    position: absolute;
    right: 6px;
    top: 50%;
    transform: translate(0px, -50%);
    width: 0;
    pointer-events: none;
  }
</style>
