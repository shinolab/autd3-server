<script lang="ts">
  export let disabled = false;
  export let label = "";
  export let click = () => {};
</script>

{#if disabled}
  <button disabled>{label}</button>
{:else}
  <button on:click={click}>{label}</button>
{/if}

<style>
  button {
    color: var(--color-text-interactive-default, #4dacff);

    text-align: center;
    font-size: 16px;
    font-style: normal;
    font-weight: 400;
    line-height: 20px;
    letter-spacing: 0.08px;

    display: inline-flex;
    padding: 8px 16px;
    justify-content: center;
    align-items: center;

    background: none;

    border-radius: 3px;
    border: 1px solid var(--color-border-interactive-default, #4dacff);

    grid-column: span 2;
  }

  button:disabled {
    opacity: 0.4;
  }
</style>
