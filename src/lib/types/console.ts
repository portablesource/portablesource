export interface LogEntry {
  timestamp: string;
  level: string;
  source: string; // "GUI" or "CLI"
  message: string;
  module?: string;
}

export interface ConsoleSettings {
  enabled: boolean;
  max_entries: number;
  log_level: string;
}

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface ConsoleState {
  logs: LogEntry[];
  isVisible: boolean;
  isEnabled: boolean;
  autoScroll: boolean;
  filter: {
    level?: LogLevel;
    source?: string;
    search?: string;
  };
}