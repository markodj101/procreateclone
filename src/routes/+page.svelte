<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let selectedColor = "#000000";

  async function changeColor(hex: string) {
    selectedColor = hex;
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    await invoke("set_color", { r, g, b });
  }

  async function clearCanvas() {
    await invoke("clear_canvas");
  }

  const colors = ["#000000", "#ef4444", "#3b82f6", "#22c55e", "#f59e0b"];

  onMount(() => {
    // ✅ Samo govori Tauriju da je frontend spreman — nema mouse listenera
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        invoke("frontend_ready");
      });
    });
  });
</script>

<!-- Nema canvas-capture diva, nema mouse event listenera -->
<div class="toolbar">
  <span class="title">Draw</span>

  {#each colors as color}
    <button
      class="swatch"
      class:active={selectedColor === color}
      style="background:{color}"
      aria-label="Select color {color}"
      on:click={() => changeColor(color)}
    ></button>
  {/each}

  <input
    type="range"
    min="1"
    max="20"
    value="4"
    on:input={(e) => {
      const target = e.target as HTMLInputElement;
      invoke("set_pen_size", { size: Number(target.value) });
    }}
  />

  <button class="erase-btn" on:click={clearCanvas}>
    🗑 Erase All
  </button>
</div>

<style>
  .toolbar {
    position: fixed;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 999;
     background: rgba(255, 255, 255, 1);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 16px;
    padding: 10px 16px;
    display: flex;
    align-items: center;
    gap: 10px;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
    user-select: none;
    -webkit-user-select: none;
  }

  .title {
    font-weight: 600;
    font-size: 14px;
    font-family: Arial, Helvetica, sans-serif;
    color: #1e293b;
  }

  .swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    transition: transform 0.1s;
  }

  .swatch.active {
    border-color: white;
    transform: scale(1.2);
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.3);
  }

  .erase-btn {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 13px;
    cursor: pointer;
    font-weight: 500;
    transition: background 0.2s;
  }

  .erase-btn:hover {
    background: rgba(239, 68, 68, 0.3);
  }

  input[type="range"] {
    width: 80px;
    accent-color: #3b82f6;
  }

  :global(body) {
    user-select: none;
    -webkit-user-select: none;
    margin: 0;
    overflow: hidden;
    background: transparent;
    animation: fadein 0.2s ease-in;
  }

  @keyframes fadein {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
