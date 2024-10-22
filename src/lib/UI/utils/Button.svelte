<script lang="ts">
  interface Props {
    disabled?: boolean;
    label?: string;
    click?: any;
  }

  let { disabled = false, label = "", click = () => {} }: Props = $props();
</script>

{#if disabled}
  <button disabled>{label}</button>
{:else}
  <button onclick={click}>{label}</button>
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
