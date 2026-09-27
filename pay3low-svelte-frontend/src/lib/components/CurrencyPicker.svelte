<script lang="ts">
  import NetworkPicker from "./NetworkPicker.svelte";

  export let open: boolean;
  export let selected: string;
  export let choices: Array<{ id: string; name: string; mark: string; color: string }>;
  export let onClose: () => void;
  export let onSelect: (currency: string) => void;
</script>

<NetworkPicker {open} mode="currency" currencies={choices} selectedCurrency={selected} {onClose} onSelectCurrency={onSelect} />
