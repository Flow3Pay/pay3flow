<script lang="ts">
  import NetworkPicker from "./NetworkPicker.svelte";

  export let open: boolean;
  export let selected: string;
  export let onClose: () => void;
  export let onSelect: (currency: string) => void;

  const choices = [
    { id: "USD", name: "US dollar", mark: "$", color: "#168451" },
    { id: "RUB", name: "Russian ruble", mark: "₽", color: "#21a038" },
    { id: "AMD", name: "Armenian dram", mark: "֏", color: "#6d2c91" },
  ];
</script>

<NetworkPicker {open} mode="currency" currencies={choices} selectedCurrency={selected} {onClose} onSelectCurrency={onSelect} />
