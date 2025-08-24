import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { LogEntry, ConsoleState, LogLevel, ConsoleSettings } from '$lib/types/console';

// Console state store
export const consoleState = writable<ConsoleState>({
  logs: [],
  isVisible: false,
  isEnabled: false,
  autoScroll: true,
  filter: {}
});

// Filtered logs derived store
export const filteredLogs = derived(consoleState, ($state) => {
  let logs = $state.logs;
  
  // Filter by level
  if ($state.filter.level) {
    logs = logs.filter(log => log.level === $state.filter.level);
  }
  
  // Filter by source
  if ($state.filter.source) {
    logs = logs.filter(log => log.source === $state.filter.source);
  }
  
  // Filter by search text
  if ($state.filter.search) {
    const searchLower = $state.filter.search.toLowerCase();
    logs = logs.filter(log => 
      log.message.toLowerCase().includes(searchLower) ||
      (log.module && log.module.toLowerCase().includes(searchLower))
    );
  }
  
  return logs;
});

// Console service class
class ConsoleService {
  private initialized = false;

  async init() {
    if (this.initialized) return;
    
    try {
      // Load existing logs
      const logs: LogEntry[] = await invoke('get_console_logs');
      const isEnabled: boolean = await invoke('is_console_enabled');
      
      consoleState.update(state => ({
        ...state,
        logs,
        isEnabled
      }));
      
      // Listen for new log entries
      await listen<LogEntry>('console-log', (event) => {
        this.addLogEntry(event.payload);
      });
      
      this.initialized = true;
    } catch (error) {
      console.error('Failed to initialize console service:', error);
    }
  }

  async toggleConsole(enabled?: boolean) {
    try {
      const currentState = this.getCurrentState();
      const newEnabled = enabled !== undefined ? enabled : !currentState.isEnabled;
      
      await invoke('toggle_console', { enabled: newEnabled });
      
      consoleState.update(state => ({
        ...state,
        isEnabled: newEnabled
      }));
      
      return newEnabled;
    } catch (error) {
      console.error('Failed to toggle console:', error);
      throw error;
    }
  }

  async clearLogs() {
    try {
      await invoke('clear_console_logs');
      consoleState.update(state => ({
        ...state,
        logs: []
      }));
    } catch (error) {
      console.error('Failed to clear logs:', error);
    }
  }

  async addLogEntry(entry: LogEntry) {
    consoleState.update(state => ({
      ...state,
      logs: [...state.logs, entry]
    }));
  }

  async logFromFrontend(level: LogLevel, message: string, module?: string) {
    try {
      await invoke('add_log_entry', {
        level,
        source: 'GUI',
        message,
        module
      });
    } catch (error) {
      console.error('Failed to add log entry:', error);
    }
  }

  async getSettings(): Promise<ConsoleSettings> {
    try {
      return await invoke('get_console_settings');
    } catch (error) {
      console.error('Failed to get console settings:', error);
      throw error;
    }
  }

  async updateSettings(settings: ConsoleSettings) {
    try {
      await invoke('set_console_settings', { settings });
      consoleState.update(state => ({
        ...state,
        isEnabled: settings.enabled
      }));
    } catch (error) {
      console.error('Failed to update console settings:', error);
      throw error;
    }
  }

  private getCurrentState(): ConsoleState {
    // Use get() to synchronously get the current state
    return get(consoleState);
  }

  // Utility methods for logging
  debug(message: string, module?: string) {
    return this.logFromFrontend('debug', message, module);
  }

  info(message: string, module?: string) {
    return this.logFromFrontend('info', message, module);
  }

  warn(message: string, module?: string) {
    return this.logFromFrontend('warn', message, module);
  }

  error(message: string, module?: string) {
    return this.logFromFrontend('error', message, module);
  }
}

export const consoleService = new ConsoleService();

// Convenience functions
export const toggleConsoleVisibility = () => {
  consoleState.update(state => ({
    ...state,
    isVisible: !state.isVisible
  }));
};

export const setFilter = (filter: Partial<ConsoleState['filter']>) => {
  consoleState.update(state => ({
    ...state,
    filter: { ...state.filter, ...filter }
  }));
};

export const clearFilter = () => {
  consoleState.update(state => ({
    ...state,
    filter: {}
  }));
};