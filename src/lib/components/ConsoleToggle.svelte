<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { consoleState, consoleService, toggleConsoleVisibility } from '$lib/stores/console';
  
  async function toggleConsole() {
    try {
      const enabled = await consoleService.toggleConsole();
      if (enabled && !$consoleState.isVisible) {
        toggleConsoleVisibility();
      }
    } catch (error) {
      console.error('Failed to toggle console:', error);
    }
  }
  
  function toggleVisibility() {
    if ($consoleState.isEnabled) {
      toggleConsoleVisibility();
    } else {
      toggleConsole();
    }
  }
</script>

<button
  class="fixed bottom-4 right-4 bg-gray-800 hover:bg-gray-700 text-white p-3 rounded-full shadow-lg z-40 transition-colors"
  on:click={toggleVisibility}
  title={$consoleState.isEnabled ? $_('debug_console.toggle.toggle_tooltip') : $_('debug_console.toggle.enable_tooltip')}
>
  {#if $consoleState.isEnabled}
    {#if $consoleState.isVisible}
      <!-- Console open icon -->
      <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
        <path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 011 1v8a1 1 0 01-1 1H7.414l-2.707 2.707A1 1 0 013 15V4zm8 6a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd" />
      </svg>
    {:else}
      <!-- Console closed icon -->
      <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
        <path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 011 1v8a1 1 0 01-1 1H7.414l-2.707 2.707A1 1 0 013 15V4zm4 6a1 1 0 100-2 1 1 0 000 2zm4 0a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd" />
      </svg>
    {/if}
    {#if $consoleState.isEnabled}
      <span class="absolute -top-1 -right-1 w-3 h-3 bg-green-400 rounded-full animate-pulse"></span>
    {/if}
  {:else}
    <!-- Console disabled icon -->
    <svg class="w-5 h-5 text-gray-500" fill="currentColor" viewBox="0 0 20 20">
      <path fill-rule="evenodd" d="M3.707 2.293a1 1 0 00-1.414 1.414l14 14a1 1 0 001.414-1.414l-1.473-1.473A10.014 10.014 0 0019.542 10C18.268 5.943 14.478 3 10 3a9.958 9.958 0 00-4.512 1.074l-1.78-1.781zm4.261 4.26l1.514 1.515a2.003 2.003 0 012.45 2.45l1.514 1.514a4 4 0 00-5.478-5.478z" clip-rule="evenodd" />
      <path d="M12.454 16.697L9.75 13.992a4 4 0 01-3.742-3.741L2.335 6.578A9.98 9.98 0 00.458 10c1.274 4.057 5.065 7 9.542 7 .847 0 1.669-.105 2.454-.303z" />
    </svg>
  {/if}
</button>