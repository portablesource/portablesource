<script lang="ts">
  import { onMount, afterUpdate } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { consoleState, filteredLogs, consoleService, setFilter, clearFilter, toggleConsoleVisibility } from '$lib/stores/console';
  import type { LogLevel } from '$lib/types/console';
  
  let consoleContainer: HTMLDivElement;
  let shouldAutoScroll = true;
  
  // Log level colors
  const levelColors = {
    debug: 'text-gray-500',
    info: 'text-blue-600',
    warn: 'text-yellow-600',
    error: 'text-red-600'
  };
  
  // Source colors
  const sourceColors = {
    GUI: 'text-green-600',
    CLI: 'text-purple-600'
  };
  
  onMount(async () => {
    await consoleService.init();
  });
  
  afterUpdate(() => {
    if (shouldAutoScroll && $consoleState.autoScroll && consoleContainer) {
      consoleContainer.scrollTop = consoleContainer.scrollHeight;
    }
  });
  
  function formatTime(timestamp: string): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('ru-RU', { 
      hour: '2-digit', 
      minute: '2-digit', 
      second: '2-digit',
      fractionalSecondDigits: 3
    });
  }
  
  function handleFilterChange(type: 'level' | 'source', value: string) {
    if (value === '') {
      const newFilter = { ...$consoleState.filter };
      delete newFilter[type];
      setFilter(newFilter);
    } else {
      setFilter({ [type]: value });
    }
  }
  
  async function clearLogs() {
    await consoleService.clearLogs();
  }
  
  function onScroll() {
    if (consoleContainer) {
      const { scrollTop, scrollHeight, clientHeight } = consoleContainer;
      shouldAutoScroll = scrollTop + clientHeight >= scrollHeight - 5;
    }
  }
</script>

{#if $consoleState.isVisible}
  <div class="fixed bottom-0 left-0 right-0 bg-gray-900 text-white border-t border-gray-700 h-80 flex flex-col z-50">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-2 bg-gray-800 border-b border-gray-700">
      <div class="flex items-center space-x-4">
        <h3 class="text-sm font-semibold">{$_('debug_console.header.title')}</h3>
        <div class="flex items-center space-x-2 text-xs">
          <span class="text-gray-400">{$_('debug_console.header.logs_count')}</span>
          <span class="text-green-400">{$filteredLogs.length}</span>
          {#if $consoleState.isEnabled}
            <span class="w-2 h-2 bg-green-400 rounded-full animate-pulse" title={$_('debug_console.header.status_active')}></span>
          {:else}
            <span class="w-2 h-2 bg-gray-500 rounded-full" title={$_('debug_console.header.status_inactive')}></span>
          {/if}
        </div>
      </div>
      
      <!-- Controls -->
      <div class="flex items-center space-x-2">
        <!-- Filters -->
        <select 
          class="bg-gray-700 text-white text-xs rounded px-2 py-1 border border-gray-600"
          on:change={(e) => handleFilterChange('level', e.currentTarget.value)}
          value={$consoleState.filter.level || ''}
        >
          <option value="">{$_('debug_console.filters.all_levels')}</option>
          <option value="debug">{$_('debug_console.filters.level_debug')}</option>
          <option value="info">{$_('debug_console.filters.level_info')}</option>
          <option value="warn">{$_('debug_console.filters.level_warn')}</option>
          <option value="error">{$_('debug_console.filters.level_error')}</option>
        </select>
        
        <select 
          class="bg-gray-700 text-white text-xs rounded px-2 py-1 border border-gray-600"
          on:change={(e) => handleFilterChange('source', e.currentTarget.value)}
          value={$consoleState.filter.source || ''}
        >
          <option value="">{$_('debug_console.filters.all_sources')}</option>
          <option value="GUI">{$_('debug_console.filters.source_gui')}</option>
          <option value="CLI">{$_('debug_console.filters.source_cli')}</option>
        </select>
        
        <input 
          type="text" 
          placeholder={$_('debug_console.filters.search_placeholder')}
          class="bg-gray-700 text-white text-xs rounded px-2 py-1 border border-gray-600 w-32"
          bind:value={$consoleState.filter.search}
          on:input={(e) => setFilter({ search: e.currentTarget.value })}
        />
        
        <!-- Action buttons -->
        <button 
          class="bg-red-600 hover:bg-red-700 text-white text-xs px-2 py-1 rounded"
          on:click={clearLogs}
          title={$_('debug_console.actions.clear_tooltip')}
        >
          {$_('debug_console.actions.clear')}
        </button>
        
        <button 
          class="bg-gray-600 hover:bg-gray-700 text-white text-xs px-2 py-1 rounded"
          on:click={clearFilter}
          title={$_('debug_console.actions.reset_tooltip')}
        >
          {$_('debug_console.actions.reset')}
        </button>
        
        <button 
          class="bg-gray-600 hover:bg-gray-700 text-white text-xs px-2 py-1 rounded"
          on:click={toggleConsoleVisibility}
          title={$_('debug_console.actions.hide')}
        >
          ✕
        </button>
      </div>
    </div>
    
    <!-- Log entries -->
    <div 
      bind:this={consoleContainer}
      class="flex-1 overflow-y-auto p-2 font-mono text-xs"
      on:scroll={onScroll}
    >
      {#each $filteredLogs as log (log.timestamp)}
        <div class="mb-1 flex items-start space-x-2 hover:bg-gray-800 px-1 rounded">
          <!-- Timestamp -->
          <span class="text-gray-400 whitespace-nowrap">
            {formatTime(log.timestamp)}
          </span>
          
          <!-- Level -->
          <span class="font-semibold {levelColors[log.level as keyof typeof levelColors] || 'text-gray-400'} uppercase w-12 text-center">
            {log.level}
          </span>
          
          <!-- Source -->
          <span class="font-semibold {sourceColors[log.source as keyof typeof sourceColors] || 'text-gray-400'} w-8 text-center">
            {log.source}
          </span>
          
          <!-- Module (if present) -->
          {#if log.module}
            <span class="text-cyan-400 whitespace-nowrap">
              [{log.module}]
            </span>
          {/if}
          
          <!-- Message -->
          <span class="flex-1 break-words">
            {log.message}
          </span>
        </div>
      {:else}
        <div class="text-gray-500 text-center py-8">
          {#if !$consoleState.isEnabled}
            {$_('debug_console.messages.disabled')}
          {:else if $consoleState.logs.length === 0}
            {$_('debug_console.messages.no_logs')}
          {:else}
            {$_('debug_console.messages.no_matches')}
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  /* Custom scrollbar for dark theme */
  div::-webkit-scrollbar {
    width: 8px;
  }
  
  div::-webkit-scrollbar-track {
    background: #374151;
  }
  
  div::-webkit-scrollbar-thumb {
    background: #6b7280;
    border-radius: 4px;
  }
  
  div::-webkit-scrollbar-thumb:hover {
    background: #9ca3af;
  }
</style>