<script module lang="ts">
  import { invoke } from "../backend";

  const icons = new Map<string, string | null>();
  const pending = new Map<string, Promise<string | null>>();

  async function fetchIcon(key: string) {
    if (icons.has(key)) return icons.get(key) ?? null;
    let request = pending.get(key);
    if (!request) {
      request = invoke<string | null>("app_icon", { key }).catch(() => null);
      pending.set(key, request);
    }
    const result = await request;
    icons.set(key, result);
    pending.delete(key);
    return result;
  }
</script>

<script lang="ts">
  interface Props {
    iconKey?: string | null;
    name?: string;
    kind?: "app" | "unit" | "kernel";
    size?: "row" | "large";
  }

  let {
    iconKey = null,
    name = "",
    kind = "app",
    size = "row",
  }: Props = $props();

  let icon = $state<string | null>(null);

  function initials(value: string) {
    const words = value.trim().split(/\s+/).filter(Boolean);
    return (words.length > 1 ? `${words[0][0]}${words[1][0]}` : words[0]?.slice(0, 2) || "?").toUpperCase();
  }

  $effect(() => {
    const requested = iconKey;
    icon = null;
    if (!requested) return;

    let cancelled = false;
    fetchIcon(requested).then((value) => {
      if (!cancelled && iconKey === requested) icon = value;
    });
    return () => {
      cancelled = true;
    };
  });
</script>

<span class="process-icon" class:large={size === "large"} class:unit={kind === "unit"} class:kernel={kind === "kernel"} aria-hidden="true">
  {#if icon}
    <img src={icon} alt="" />
  {:else if kind === "kernel"}
    <span class="kernel-mark">⌁</span>
  {:else if kind === "unit"}
    <span class="unit-mark"></span>
  {:else}
    <span class="initials">{initials(name)}</span>
  {/if}
</span>
