<script lang="ts">
  interface Props {
    id?: string;
    checked?: boolean;
  }

  let { id = "", checked = $bindable(false) }: Props = $props();
</script>

<input {id} type="checkbox" bind:checked />
<label for={id}></label>

<style>
  input[type="checkbox"] {
    display: none;
  }
  input[type="checkbox"] + label {
    box-sizing: border-box;
    cursor: pointer;
    position: relative;
    width: auto;
  }
  input[type="checkbox"] + label::before {
    position: absolute;
    top: 50%;
    display: block;
    margin-top: -8px;
    width: 16px;
    height: 16px;
    content: "";
    border-radius: 4px;
    background-color: none;
    border: 1px solid var(--color-border-interactive-default, #4dacff);
  }
  input[type="checkbox"] + label::after {
    content: "";
    display: block;
    width: 6px;
    height: 6px;
    left: 6px;
    margin-top: -5px;
    position: absolute;
    top: 50%;
    transform: rotate(45deg);
    border-color: var(--color-text-interactive-default, #4dacff);
  }
  input[type="checkbox"]:checked + label {
    box-sizing: border-box;
    cursor: pointer;
    position: relative;
    width: auto;
  }
  input[type="checkbox"]:checked + label::before {
    position: absolute;
    top: 50%;
    display: block;
    margin-top: -8px;
    width: 16px;
    height: 16px;
    content: "";
    border-radius: 4px;
    background-color: none;
    border: 1px solid var(--color-border-interactive-default, #4dacff);
  }
  input[type="checkbox"]:checked + label::after {
    border-right: 2px solid #fff;
    border-bottom: 2px solid #fff;
    content: "";
    display: block;
    width: 6px;
    height: 6px;
    left: 5px;
    margin-top: -4px;
    position: absolute;
    top: 50%;
    transform: rotate(45deg);
    border-color: var(--color-text-interactive-default, #4dacff);
  }
</style>
