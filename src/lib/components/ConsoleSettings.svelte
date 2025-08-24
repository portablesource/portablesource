<script lang="ts">
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { consoleService, consoleState } from '$lib/stores/console';
  import type { ConsoleSettings } from '$lib/types/console';
  
  let settings: ConsoleSettings = {
    enabled: false,
    max_entries: 1000,
    log_level: 'info'
  };
  
  let loading = false;
  let saving = false;
  
  onMount(async () => {
    await loadSettings();
  });
  
  async function loadSettings() {
    loading = true;
    try {
      settings = await consoleService.getSettings();
    } catch (error) {
      console.error('Failed to load console settings:', error);
    } finally {
      loading = false;
    }
  }
  
  async function saveSettings() {
    saving = true;
    try {
      await consoleService.updateSettings(settings);
      // Show success message or toast here
    } catch (error) {
      console.error('Failed to save console settings:', error);
      // Show error message here
    } finally {
      saving = false;
    }
  }
  
  async function toggleConsole() {
    const newEnabled = !settings.enabled;
    settings = { ...settings, enabled: newEnabled };
    await saveSettings();
  }
</script>

<div class="space-y-8">
  <!-- Header Section -->
  <div class="flex items-center justify-between pb-4 border-b" style="border-color: var(--border-color);">
    <div class="flex items-center space-x-3">
      <div class="p-2 rounded-lg" style="background-color: var(--bg-tertiary);">
        <svg class="w-6 h-6" style="color: var(--accent-primary);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
        </svg>
      </div>
      <div>
        <h3 class="text-xl font-semibold" style="color: var(--text-primary);">{$_('debug_console.settings.title')}</h3>
        <p class="text-sm" style="color: var(--text-secondary);">{$_('debug_console.settings.description')}</p>
      </div>
    </div>
    {#if loading}
      <div class="flex items-center space-x-2" style="color: var(--accent-primary);">
        <div class="animate-spin rounded-full h-5 w-5 border-2" style="border-color: var(--accent-primary); border-top-color: transparent;"></div>
        <span class="text-sm font-medium">Loading...</span>
      </div>
    {/if}
  </div>
  
  <div class="space-y-6">
    <!-- Enable/Disable Console -->
    <div class="rounded-xl p-6 shadow-sm hover:shadow-md transition-shadow" style="background-color: var(--card-bg); border: 1px solid var(--card-border);">
      <div class="flex items-center justify-between">
        <div class="flex-1 mr-6">
          <div class="flex items-center space-x-3">
            <div class="p-2 rounded-lg" style="background-color: var(--bg-success);">
              <svg class="w-5 h-5" style="color: var(--success-color);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 9l3 3-3 3m13 0h-6m-2-5v6a2 2 0 002 2h6a2 2 0 002-2v-6a2 2 0 00-2-2h-6a2 2 0 00-2 2z"></path>
              </svg>
            </div>
            <div>
              <label for="console-toggle" class="text-lg font-semibold cursor-pointer" style="color: var(--text-primary);">{$_('debug_console.settings.enable_label')}</label>
              <p class="text-sm mt-1" style="color: var(--text-secondary);">
                {$_('debug_console.settings.enable_description')}
              </p>
            </div>
          </div>
        </div>
        <button
          id="console-toggle"
          type="button"
          class="relative inline-flex h-7 w-12 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-all duration-300 ease-in-out focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed"
          style="background: {settings.enabled ? 'linear-gradient(135deg, var(--accent-primary), var(--accent-hover))' : 'var(--button-secondary)'}; box-shadow: {settings.enabled ? 'var(--shadow-primary)' : 'none'}; {settings.enabled ? '--tw-ring-color: var(--accent-primary); --tw-ring-opacity: 0.2;' : ''}"
          role="switch"
          aria-checked={settings.enabled}
          aria-labelledby="console-toggle-label"
          on:click={toggleConsole}
          disabled={saving}
        >
          <span class="sr-only">{$_('debug_console.settings.enable_sr')}</span>
          <span
            aria-hidden="true"
            class="pointer-events-none inline-block h-6 w-6 transform rounded-full shadow-lg ring-0 transition-all duration-300 ease-in-out"
            style="background-color: var(--text-primary); transform: {settings.enabled ? 'translateX(1.25rem)' : 'translateX(0)'};"
          ></span>
        </button>
      </div>
    </div>
    
    <!-- Console Status -->
    <div class="rounded-xl p-6 shadow-sm" style="background: linear-gradient(135deg, var(--bg-secondary), var(--bg-tertiary)); border: 1px solid var(--card-border);">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center space-x-3">
          <div class="p-2 rounded-lg" style="background-color: var(--bg-tertiary);">
            <svg class="w-5 h-5" style="color: var(--text-secondary);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"></path>
            </svg>
          </div>
          <span class="text-lg font-semibold" style="color: var(--text-primary);">{$_('debug_console.settings.status_label')}</span>
        </div>
        {#if $consoleState.isEnabled}
          <span class="inline-flex items-center px-3 py-1.5 rounded-full text-sm font-semibold" style="background: linear-gradient(135deg, var(--success-color), var(--success-hover)); color: var(--text-primary); box-shadow: var(--shadow-success);">
            <span class="w-2 h-2 rounded-full mr-2 animate-pulse" style="background-color: var(--text-primary);"></span>
            {$_('debug_console.settings.status_active')}
          </span>
        {:else}
          <span class="inline-flex items-center px-3 py-1.5 rounded-full text-sm font-semibold" style="background-color: var(--button-secondary); color: var(--text-primary);">
            <span class="w-2 h-2 rounded-full mr-2" style="background-color: var(--text-muted);"></span>
            {$_('debug_console.settings.status_inactive')}
          </span>
        {/if}
      </div>
      
      <div class="grid grid-cols-2 gap-4">
        <div class="rounded-lg p-4" style="background-color: var(--card-bg); border: 1px solid var(--card-border);">
          <div class="flex items-center space-x-2">
            <svg class="w-4 h-4" style="color: var(--accent-primary);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
            </svg>
            <span class="text-sm font-medium" style="color: var(--text-secondary);">{$_('debug_console.settings.logs_in_buffer')}</span>
          </div>
          <div class="mt-2">
            <span class="text-2xl font-bold" style="color: var(--accent-primary);">{$consoleState.logs.length}</span>
          </div>
        </div>
        <div class="rounded-lg p-4" style="background-color: var(--card-bg); border: 1px solid var(--card-border);">
          <div class="flex items-center space-x-2">
            <svg class="w-4 h-4" style="color: var(--success-color);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"></path>
            </svg>
            <span class="text-sm font-medium" style="color: var(--text-secondary);">{$_('debug_console.settings.max_entries')}</span>
          </div>
          <div class="mt-2">
            <span class="text-2xl font-bold" style="color: var(--success-color);">{settings.max_entries}</span>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Log Level Setting -->
    <div class="rounded-xl p-6 shadow-sm hover:shadow-md transition-shadow" style="background-color: var(--card-bg); border: 1px solid var(--card-border);">
      <div class="flex items-center space-x-3 mb-4">
        <div class="p-2 rounded-lg" style="background-color: var(--bg-danger-lighter);">
          <svg class="w-5 h-5" style="color: var(--warning-color);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"></path>
          </svg>
        </div>
        <div>
          <label for="log-level" class="block text-lg font-semibold" style="color: var(--text-primary);">
            {$_('debug_console.settings.log_level_label')}
          </label>
          <p class="text-sm mt-1" style="color: var(--text-secondary);">
            {$_('debug_console.settings.log_level_description')}
          </p>
        </div>
      </div>
      <select
        id="log-level"
        bind:value={settings.log_level}
        on:change={saveSettings}
        class="w-full rounded-lg shadow-sm text-base py-3 px-4 transition-all duration-200 disabled:cursor-not-allowed"
        style="border: 1px solid var(--input-border); background-color: var(--input-bg); color: var(--text-primary); {saving ? 'background-color: var(--bg-tertiary); cursor: not-allowed;' : ''}"
        disabled={saving}
      >
        <option value="debug">{$_('debug_console.settings.log_level_debug')}</option>
        <option value="info">{$_('debug_console.settings.log_level_info')}</option>
        <option value="warn">{$_('debug_console.settings.log_level_warn')}</option>
        <option value="error">{$_('debug_console.settings.log_level_error')}</option>
      </select>
    </div>
    
    <!-- Actions -->
    <div class="rounded-xl p-6 shadow-sm" style="background-color: var(--card-bg); border: 1px solid var(--card-border);">
      <div class="flex items-center space-x-3 mb-4">
        <div class="p-2 rounded-lg" style="background-color: var(--bg-tertiary);">
          <svg class="w-5 h-5" style="color: var(--accent-primary);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"></path>
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path>
          </svg>
        </div>
        <h4 class="text-lg font-semibold" style="color: var(--text-primary);">{$_('debug_console.settings.quick_actions')}</h4>
      </div>
      <div class="flex flex-wrap gap-3">
        <button
          type="button"
          class="inline-flex items-center px-4 py-2.5 shadow-sm text-sm font-medium rounded-lg transition-all duration-200 hover:shadow-md"
          style="border: 1px solid var(--border-color); background-color: var(--card-bg); color: var(--text-primary); {':hover' in window ? 'hover:background-color: var(--bg-tertiary);' : ''}"
          on:click={() => consoleService.clearLogs()}
        >
          <svg class="w-4 h-4 mr-2" style="color: var(--text-secondary);" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"></path>
          </svg>
          {$_('debug_console.settings.clear_logs')}
        </button>
        
        {#if $consoleState.isVisible}
          <button
            type="button"
            class="inline-flex items-center px-4 py-2.5 border border-transparent text-sm font-medium rounded-lg transition-all duration-200 shadow-lg hover:shadow-xl"
            style="color: var(--text-primary); background: linear-gradient(135deg, var(--accent-primary), var(--accent-hover)); box-shadow: var(--shadow-primary);"
            on:click={() => consoleState.update(s => ({ ...s, isVisible: false }))}
          >
            <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.878 9.878L3 3m6.878 6.878L21 21"></path>
            </svg>
            {$_('debug_console.actions.hide')}
          </button>
        {:else}
          <button
            type="button"
            class="inline-flex items-center px-4 py-2.5 border border-transparent text-sm font-medium rounded-lg transition-all duration-200 shadow-lg hover:shadow-xl disabled:opacity-50 disabled:cursor-not-allowed disabled:shadow-none"
            style="color: var(--text-primary); background: linear-gradient(135deg, var(--success-color), var(--success-hover)); box-shadow: var(--shadow-success);"
            on:click={() => consoleState.update(s => ({ ...s, isVisible: true }))}
            disabled={!$consoleState.isEnabled}
          >
            <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path>
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"></path>
            </svg>
            {$_('debug_console.actions.show')}
          </button>
        {/if}
      </div>
    </div>
  </div>
  
  {#if saving}
    <div class="flex items-center justify-center p-4 rounded-xl" style="background-color: var(--bg-tertiary); border: 1px solid var(--border-color);">
      <div class="flex items-center space-x-3">
        <div class="animate-spin rounded-full h-5 w-5 border-2" style="border-color: var(--accent-primary); border-top-color: transparent;"></div>
        <span class="text-sm font-medium" style="color: var(--accent-primary);">{$_('debug_console.actions.saving')}</span>
      </div>
    </div>
  {/if}
</div>