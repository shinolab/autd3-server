<script lang="ts">
  export let id = "";
  export let value = "";
</script>

<input
  {id}
  type="text"
  pattern="\d{3}\.\d{3}\.\d{3}\.\d{3}"
  placeholder="127.0.0.1"
  required
  bind:value
/>

<style>
  input {
    border-radius: 3px;
    border: 1px solid var(--color-border-interactive-muted, #2b659b);
    background: var(--color-background-base-default, #101923);
    color: var(--color-text-base-default, #ffffff);
  }

  input::placeholder {
    opacity: 0.4;
  }
</style>
