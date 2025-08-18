<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { _, locale } from 'svelte-i18n';

  // Installation flow state
  let currentStep = 'initial-check'; // 'initial-check', 'path-selection', 'installing', 'main-interface', 'environment-missing'
  let installPath = '';
  let isInstalling = false;
  let installStatus = '';
  let installTimer = 0;
  let installTimerInterval: number | null = null;
  let installProgress = 0;
  let maxInstallTime = 1200; // Maximum expected install time in seconds
  
  // Environment missing dialog state
  let showEnvironmentMissingDialog = false;
  let registryPath = '';
  
  // Main interface state
  let cliInstalled = false;
  
  // (removed) Legacy CLI version management state

  // App updater state
  let currentAppVersion = ''; // Will be loaded from Tauri
  let isCheckingUpdates = false;
  let isInstallingUpdate = false;
  let updateInfo: any = null;

  // MSVC Build Tools state
  let msvcInstalled: boolean | null = null;
  let isAdminUser: boolean = false;
  let isInstallingMsvc = false;

  let environmentStatus = {
    environment_exists: false,
    setup_completed: false,
    overall_status: 'Unknown'
  };
  let isCheckingEnvironment = false;
  let isSettingUpEnvironment = false;
  let envSetupProgress = { phase: '', done: 0, total: 0 };
  let envProgressText = '';
  let currentToolIcon = '🔧';
  const toolNames: Record<string, string> = { python: 'Python', git: 'Git', ffmpeg: 'FFmpeg', cuda: 'CUDA' };
  const toolIcons: Record<string, string> = { python: '🐍', git: '🧰', ffmpeg: '🎬', cuda: '⚡' };
  // Watchdog against stuck progress
  let lastProgressAt = 0;
  let stallWatchInterval: number | null = null;
  const stallTimeoutSec = 30; // if no progress for 30s, verify status and fast-forward

  function formatDuration(totalSeconds: number): string {
    const minutes = Math.floor((totalSeconds || 0) / 60);
    const seconds = Math.max(totalSeconds % 60, 0);
    return `${minutes}:${String(seconds).padStart(2, '0')}`;
  }
  interface Repository {
    id?: number;
    name: string;
    description: string;
    repositoryUrl?: string;
    downloadCount?: number;
    uploadedByUsername?: string;
  }

  interface InstalledRepository {
    name: string;
    status?: string;
    hasLauncher?: boolean;
    sourceLabel?: string; // GitHub/Git/Сервер
  }

  let installedRepos: InstalledRepository[] = [];
  let availableRepos: Repository[] = [];  let selectedRepo = '';
  let isInstallingRepo = false;
  let installingRepoName = '';
  let isUpdatingRepo = false;
  let updatingRepoName = '';
  let isRemovingRepo = false;
  let removingRepoName = '';
  
  // UI state
  let sidebarOpen = false;
  let currentView = 'top-repos'; // 'top-repos', 'installed-repos', 'settings'
  let showInstallNotification = false;
  let installedRepoName = '';

  // Theme state
  let currentTheme = 'system'; // 'light', 'dark', 'system'
  let isDarkMode = false;

  // Theme management functions
  function getSystemTheme(): boolean {
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  }

  function applyTheme(theme: string) {
    const html = document.documentElement;
    
    if (theme === 'system') {
      isDarkMode = getSystemTheme();
    } else {
      isDarkMode = theme === 'dark';
    }
    
    if (isDarkMode) {
      html.setAttribute('data-theme', 'dark');
    } else {
      html.removeAttribute('data-theme');
    }
    
    // Save theme preference to localStorage
    localStorage.setItem('theme', theme);
  }

  function setTheme(theme: string) {
    currentTheme = theme;
    applyTheme(theme);
  }

  function initializeTheme() {
    // Load theme from localStorage or default to 'system'
    const savedTheme = localStorage.getItem('theme') || 'system';
    currentTheme = savedTheme;
    applyTheme(savedTheme);
    
    // Listen for system theme changes
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    mediaQuery.addEventListener('change', () => {
      if (currentTheme === 'system') {
        applyTheme('system');
      }
    });
  }

  export const host = "server.portables.dev";

  onMount(async () => {
    initializeTheme();
    await loadAppVersion();
    await performInitialCheck();
    await refreshMsvcStatus();
  });

  async function loadAppVersion() {
    try {
      currentAppVersion = await invoke('get_app_version');
    } catch (error) {
      console.error('Failed to load app version:', error);
      currentAppVersion = '0.0.4'; // Fallback version
    }
  }

  async function performInitialCheck() {
    try {
      // Try registry first
      try {
        registryPath = await invoke('get_install_path');
        
        // Check if environment exists at registry path
        if (registryPath) {
          const environmentExists = await invoke('check_environment_exists_at_path', { install_path: registryPath });
          
          if (!environmentExists) {
            // Environment missing at registry path - show dialog
            showEnvironmentMissingDialog = true;
            currentStep = 'environment-missing';
            return;
          }
          
          // Environment exists, use registry path
          installPath = registryPath;
        } else {
          // No registry path, try heuristics
          installPath = await invoke('find_cli_installation');
        }
      } catch (e) {
        // Registry failed, try heuristics
        installPath = await invoke('find_cli_installation');
      }
      
      cliInstalled = true;
      
      // CLI is present; proceed with environment checks

      // Проверяем статус окружения сначала
      const st = await checkEnvironmentStatus();
      if (st && st.setup_completed) {
        currentStep = 'main-interface';
        await loadEnvironmentAndRepos();
      } else {
        // Запускаем настройку окружения с потоковым прогрессом
        await startEnvironmentSetupStream();
      }
    } catch (error) {
      
      cliInstalled = false;
      currentStep = 'path-selection';
    }
    
    // Always load available repos for the interface
    await loadAvailableRepos();
  }

  async function selectInstallPath() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: $_('installation.select_folder')
      });
      
      if (selected) {
        installPath = selected;
      }
    } catch (error) {
      installStatus = `Folder selection error: ${error}`;
    }
  }

  async function handleNewInstallPath() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Выберите новый путь установки'
      });
      
      if (selected) {
        installPath = selected;
        showEnvironmentMissingDialog = false;
        currentStep = 'path-selection';
      }
    } catch (error) {
      installStatus = `Ошибка выбора папки: ${error}`;
    }
  }

  async function clearRegistryAndSelectPath() {
    try {
      await invoke('clear_install_path');
      showEnvironmentMissingDialog = false;
      currentStep = 'path-selection';
      installPath = '';
    } catch (error) {
      installStatus = `Ошибка очистки реестра: ${error}`;
    }
  }

  async function savePathAndStartInstallation() {
    if (!installPath) {
      installStatus = $_('installation.select_installation_folder');
      return;
    }

    try {
      const result = await invoke('set_install_path', { path: installPath }) as {success: boolean, message?: string, normalized_path?: string};
      if (result.success) {
        if (result.normalized_path) {
          installPath = result.normalized_path;
        }
        currentStep = 'installing';
        await startEnvironmentSetupStream();
      } else {
        installStatus = `Error: ${result.message}`;
      }
    } catch (error) {
      installStatus = `Path saving error: ${error}`;
    }
  }

  async function startInstallationProcess() {
    isInstalling = true;
    installTimer = 0;
    installProgress = 0;
    installStatus = $_('common.loading');
    
    // Start timer and progress
    installTimerInterval = setInterval(() => {
      installTimer++;
      // Update progress based on time (smooth progress bar)
      installProgress = Math.min((installTimer / maxInstallTime) * 100, 95); // Cap at 95% until completion
      
      if (installTimer >= 15 && isInstalling) {
        installStatus = $_('common.loading');
        maxInstallTime = 30; // Extend expected time for slow connections
      }
    }, 1000);
    
    try {
      const result = await invoke('download_and_install_cli', { install_path: installPath }) as {success: boolean, message?: string, normalized_path?: string};
      if (result.success) {
        if (result.normalized_path) {
          installPath = result.normalized_path;
        }
        await testCliInstallation();
        if (cliInstalled) {
          await finishInstallation();
        } else {
          installStatus = $_('common.error');
        }
      } else {
        installStatus = `Installation error: ${result.message}`;
      }
    } catch (error) {
      installStatus = `Error: ${error}`;
    }
  }
  
  async function finishInstallation() {
    installProgress = 100; // Complete the progress bar
    isInstalling = false;
    if (installTimerInterval) {
      clearInterval(installTimerInterval);
      installTimerInterval = null;
    }
    
    // После установки сразу запускаем настройку окружения
    await startEnvironmentSetupStream();
  }
  
  async function loadEnvironmentAndRepos() {
    await checkEnvironmentSetup();
    await loadInstalledRepos();
    await loadAvailableRepos();
    // Принудительно перечитаем статус из бэкенда после установки, чтобы увидеть новое значение
    await checkEnvironmentStatus();
  }

  async function testCliInstallation(showError = true) {
    try {
      const result = await invoke('test_cli_installation', { install_path: installPath }) as {success: boolean, message?: string};
      if (result.success) {
        cliInstalled = true;
        // CLI test ok
      } else {
        cliInstalled = false;
        if (showError) {
          installStatus = `CLI testing error: ${result.message}`;
        }
      }
    } catch (error) {
      if (showError) {
        installStatus = `Testing error: ${error}`;
      }
      cliInstalled = false;
    }
  }

  // Environment functions
  async function checkEnvironmentSetup() {
    try {
      const envStatus = await invoke('check_environment_status', { install_path: installPath, installPath }) as {
        environment_exists: boolean,
        setup_completed: boolean,
        overall_status: string
      };
      environmentStatus = envStatus;
    } catch (error) {
      console.error('Error checking environment setup:', error);
      // Environment check failed
    }
  }

  async function checkEnvironmentStatus() {
    try {
      isCheckingEnvironment = true;
      const status = await invoke('check_environment_status', { install_path: installPath, installPath }) as {
        environment_exists: boolean,
        setup_completed: boolean,
        overall_status: string
      };
      environmentStatus = status;
      return status;
    } catch (error) {
      console.error('Error checking environment status:', error);
      environmentStatus = {
        environment_exists: false,
        setup_completed: false,
        overall_status: $_('installation.check_failed')
      };
      return environmentStatus;
    } finally {
      isCheckingEnvironment = false;
    }
  }

  async function startEnvironmentSetupStream() {
    isSettingUpEnvironment = true;
    installStatus = $_('installation.setup_environment');
    envSetupProgress = { phase: 'init', done: 0, total: 0 };
    envProgressText = '';
    currentStep = 'installing';
    lastProgressAt = Date.now();

    // стартуем таймер отображения времени
    installTimer = 0;
    if (installTimerInterval) { clearInterval(installTimerInterval); }
    installTimerInterval = setInterval(() => { installTimer++; }, 1000) as any;

    const eventId = `${Date.now()}`;
    // Подписка на события
    const unlistenProgress = await listen(`env-setup-progress-${eventId}`, (e: any) => {
      const { phase, done, total } = e.payload as any;
      envSetupProgress = { phase, done, total };
      const remaining = Math.max(total - done, 0);
      const key = phase === 'init' ? '' : phase;
      const displayName = key && toolNames[key] ? toolNames[key] : '';
      currentToolIcon = key && toolIcons[key] ? toolIcons[key] : '🔧';
      envProgressText = $_('installation.installing_tool', { values: { tool: displayName } });
      installProgress = total ? Math.min(Math.round((done / total) * 100), 99) : 0;
      lastProgressAt = Date.now();
    });
    const unlistenError = await listen(`env-setup-error-${eventId}`, (e: any) => {
      installStatus = $_('installation.environment_setup_failed', { values: { error: String(e.payload) } });
    });
    const unlistenFinished = await listen(`env-setup-finished-${eventId}`, async (e: any) => {
      const { success } = e.payload as any;
      if (success) {
        installProgress = 100;
        installStatus = $_('installation.all_installed');
        // краткая задержка и переход
        setTimeout(async () => {
          currentStep = 'main-interface';
          await loadEnvironmentAndRepos();
        }, 1000);
      } else {
        installStatus = $_('installation.environment_setup_failed', { values: { error: $_('repositories.unknown_error') } });
      }
      unlistenProgress();
      unlistenError();
      unlistenFinished();
      isSettingUpEnvironment = false;
      if (installTimerInterval) { clearInterval(installTimerInterval); installTimerInterval = null; }
      if (stallWatchInterval) { clearInterval(stallWatchInterval); stallWatchInterval = null; }
    });

    await invoke('setup_environment_stream', { install_path: installPath, installPath, event_id: eventId, eventId });

    // Start watchdog to handle rare cases when finished event is missed
    if (stallWatchInterval) { clearInterval(stallWatchInterval); }
    stallWatchInterval = setInterval(async () => {
      const secondsSince = Math.floor((Date.now() - lastProgressAt) / 1000);
      if (secondsSince >= stallTimeoutSec) {
        try {
          const status = await invoke('check_environment_status', { install_path: installPath, installPath }) as {
            environment_exists: boolean,
            setup_completed: boolean,
            overall_status: string
          };
          if (status && status.setup_completed) {
            // consider setup completed and fast-forward
            installProgress = 100;
            envProgressText = $_('installation.all_installed');
            if (stallWatchInterval) { clearInterval(stallWatchInterval); stallWatchInterval = null; }
            if (installTimerInterval) { clearInterval(installTimerInterval); installTimerInterval = null; }
            setTimeout(async () => {
              currentStep = 'main-interface';
              await loadEnvironmentAndRepos();
            }, 500);
          }
        } catch (_) {
          // ignore check errors
        } finally {
          // reset timer to avoid hammering
          lastProgressAt = Date.now();
        }
      }
    }, 5000) as any;
  }

  async function loadInstalledRepos() {
    try {
      const installed: InstalledRepository[] = [];

      // Prefer asking CLI to list repos with source tags
      try {
        const res = await invoke('run_cli_command', { install_path: installPath, installPath, args: ['list-repos'] }) as { success: boolean, stdout: string, stderr: string };
        if (res && res.success) {
          const lines = (res.stdout || '').split(/\r?\n/).map(l => l.trim()).filter(Boolean);
          for (const line of lines) {
            const m = line.match(/^\s*-\s*(.+)$/); // lines like "  - name [From github]"
            if (m) {
              const raw = m[1].trim();
              let name = raw;
              let sourceLabel: string | undefined = undefined;
              const suff = raw.match(/\[(.*?)\]$/);
              if (suff) {
                name = raw.replace(/\s*\[(.*?)\]$/, '').trim();
                const low = (suff[1] || '').toLowerCase();
                if (low.includes('github')) sourceLabel = 'GitHub';
                else if (low.includes('git')) sourceLabel = 'Git';
                else if (low.includes('server')) sourceLabel = 'Server';
              }
              installed.push({ name, status: 'installed', hasLauncher: true, sourceLabel });
            }
          }
        }
      } catch (_) {
        // ignore and fallback
      }

      // Fallback: filesystem intersection if CLI not available or returned nothing
      if (installed.length === 0) {
        const envsFolders = await invoke('list_directory_folders', { install_path: installPath, installPath, directory_name: 'envs', directoryName: 'envs' }) as string[];
        const reposFolders = await invoke('list_directory_folders', { install_path: installPath, installPath, directory_name: 'repos', directoryName: 'repos' }) as string[];
        const envSet = new Set((envsFolders || []).map((n) => (n || '').toLowerCase()));
        for (const repoName of reposFolders || []) {
          const match = envSet.has((repoName || '').toLowerCase());
          if (match) {
            installed.push({ name: repoName, status: 'installed', hasLauncher: true });
          }
        }
      }

      installedRepos = installed;
    } catch (error) {
      console.error('Error loading installed repositories:', error);
      installedRepos = [];
    }
  }

  async function loadAvailableRepos() {
    try {
      const response = await invoke('proxy_request', { url: `https://${host}/api/repositories/top?limit=10` }) as string;
      const data = JSON.parse(response);
      
      if (data.success && data.repositories) {
        availableRepos = data.repositories.map((repo: any) => ({
          id: repo.id,
          name: repo.name,
          description: repo.description,
          repositoryUrl: repo.repositoryUrl,
          downloadCount: repo.downloadCount,
          uploadedByUsername: repo.uploadedByUsername
        }));
      } else {
        console.error('Failed to load repositories:', data);
        // Fallback to stubs in case of error
        availableRepos = [
          { name: 'stable-diffusion-webui', description: 'Popular interface for image generation' },
          { name: 'comfyui', description: 'Node-based interface for Stable Diffusion' },
          { name: 'ollama', description: 'Local language models' }
        ];
      }
    } catch (error) {
      console.error('Error loading repositories:', error);
      // Fallback to stubs if API is unavailable
      availableRepos = [
        { name: 'stable-diffusion-webui', description: 'Popular interface for image generation' },
        { name: 'comfyui', description: 'Node-based interface for Stable Diffusion' },
        { name: 'ollama', description: 'Local language models' }
      ];
    }
  }

  // Repository management functions
  async function installRepo(repoName: string) {
    try {
      isInstallingRepo = true;
      installingRepoName = repoName;
      
      // Check if CLI is installed first
      if (!cliInstalled) {
        installStatus = $_('cli.not_installed');
        setTimeout(() => {
          setCurrentView('settings');
          sidebarOpen = false;
        }, 1500);
        return;
      }

      // Check if repository is already installed by folder presence
      const isInstalled = await checkRepoInstallStatus(repoName);
      
      if (isInstalled) {
        // Show notification and navigate to installed repositories
        installedRepoName = repoName;
        showInstallNotification = true;
        setTimeout(() => {
          showInstallNotification = false;
          setCurrentView('installed-repos');
        }, 2000);
        return;
      }

      // Check environment presence before installation using proper status check
      const envStatus = await invoke('check_environment_status', { install_path: installPath, installPath }) as {
        environment_exists: boolean,
        setup_completed: boolean,
        overall_status: string
      };
      
      if (!envStatus.setup_completed) {
        installStatus = $_('installation.environment_setup_first');
        return;
      }

      installStatus = $_('repositories.installing');
      
      const cliArgs = ['--install-repo', repoName];
      
      const result = await invoke('run_cli_command', { install_path: installPath, installPath, args: cliArgs }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        await loadInstalledRepos();
        
        // Show successful installation notification
        installedRepoName = repoName;
        showInstallNotification = true;
        installStatus = $_('repositories.successfully_installed', { values: { name: repoName } });
        
        // Automatically hide notification after 3 seconds and navigate to installed repositories
        setTimeout(() => {
          showInstallNotification = false;
          setCurrentView('installed-repos');
        }, 3000);
      } else {
        installStatus = $_('repositories.installation_error', { values: { repoName, error: result.stderr || result.stdout || $_('repositories.unknown_error') } });
      }
    } catch (error) {
      console.error('Error during repository installation:', error);
      installStatus = $_('repositories.installation_error', { values: { repoName, error: String(error) } });
    } finally {
      isInstallingRepo = false;
      installingRepoName = '';
    }
  }

  // Helper: extract display folder name from input (URL or name)
  function extractRepoDisplayName(input: string): string {
    try {
      if (input.startsWith('http') || input.startsWith('git@')) {
        const url = new URL(input.replace('git@', 'ssh://git@'));
        let last = url.pathname.split('/').filter(Boolean).pop() || input;
        return last.endsWith('.git') ? last.slice(0, -4) : last;
      }
    } catch (_) {
      // Fallback to raw input
    }
    // owner/name or plain name
    const seg = input.split('/').filter(Boolean).pop() || input;
    return seg.endsWith('.git') ? seg.slice(0, -4) : seg;
  }

  // Install repository by user-provided input (URL or name)
  async function installRepoFromInput(userInput: string) {
    const displayName = extractRepoDisplayName(userInput);
    try {
      isInstallingRepo = true;
      installingRepoName = displayName;

      if (!cliInstalled) {
        installStatus = $_('cli.not_installed');
        setTimeout(() => { setCurrentView('settings'); sidebarOpen = false; }, 1500);
        return;
      }

      // Check if repo with this display name seems installed
      const isInstalled = await checkRepoInstallStatus(displayName);
      if (isInstalled) {
        installedRepoName = displayName;
        showInstallNotification = true;
        setTimeout(() => { showInstallNotification = false; setCurrentView('installed-repos'); }, 2000);
        return;
      }

      // Ensure environment is ready
      const envStatus = await invoke('check_environment_status', { install_path: installPath, installPath }) as {
        environment_exists: boolean, setup_completed: boolean, overall_status: string
      };
      if (!envStatus.setup_completed) { installStatus = $_('installation.environment_setup_first'); return; }

      installStatus = $_('repositories.installing');
      const result = await invoke('run_cli_command', {
        install_path: installPath, installPath, args: ['--install-repo', userInput]
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};

      if (result.success) {
        await loadInstalledRepos();
        installedRepoName = displayName;
        showInstallNotification = true;
        installStatus = $_('repositories.successfully_installed', { values: { name: displayName } });
        setTimeout(() => { showInstallNotification = false; setCurrentView('installed-repos'); }, 3000);
      } else {
        installStatus = $_('repositories.installation_error', { values: { repoName: displayName, error: result.stderr || result.stdout || $_('repositories.unknown_error') } });
      }
    } catch (error) {
      console.error('Error during repository installation (input):', error);
      installStatus = $_('repositories.installation_error', { values: { repoName: displayName, error: String(error) } });
    } finally {
      isInstallingRepo = false;
      installingRepoName = '';
    }
  }

  // Modern modal instead of native prompt
  let showAddRepoModal = false;
  let newRepoInput = '';
  function openAddRepoModal() {
    newRepoInput = '';
    showAddRepoModal = true;
  }
  function closeAddRepoModal() { showAddRepoModal = false; }
  function confirmAddRepoModal() {
    const v = (newRepoInput || '').trim();
    if (!v) { closeAddRepoModal(); return; }
    closeAddRepoModal();
    installRepoFromInput(v);
  }

  // Complete uninstall modal
  let showUninstallModal = false;
  let uninstallConfirmText = '';
  function openUninstallModal() {
    uninstallConfirmText = '';
    showUninstallModal = true;
  }
  function closeUninstallModal() { showUninstallModal = false; }
  function confirmUninstallModal() {
    if (uninstallConfirmText.toLowerCase() !== 'delete') {
      return;
    }
    closeUninstallModal();
    completeUninstall();
  }

  // Check repository installation status by folder presence
  async function checkRepoInstallStatus(repoName: string): Promise<boolean> {
    try {
      // Get folder lists in envs and repos directories
      const envsFolders = await invoke('list_directory_folders', { install_path: installPath, installPath, directory_name: 'envs', directoryName: 'envs' }) as string[];
      
      const reposFolders = await invoke('list_directory_folders', { install_path: installPath, installPath, directory_name: 'repos', directoryName: 'repos' }) as string[];
      
      // Repository is considered installed only if it exists in both directories
      const isInEnvs = envsFolders.includes(repoName);
      const isInRepos = reposFolders.includes(repoName);
      const isInstalled = isInEnvs && isInRepos;
      
      return isInstalled;
    } catch (error) {
      console.error('Error checking repository installation status:', error);
      return false;
    }
  }

  async function runRepo(repoName: string) {
    try {
      installStatus = $_('repositories.starting', { values: { repoName } });
      
      // Run batch file start_repo_name.bat from repository folder in new console window
      const batFile = `start_${repoName}.bat`;
      const workingDir = `${installPath}\\repos\\${repoName}`;
      
      
      
      const result = await invoke('run_batch_in_new_window', {
        batch_file: batFile,
        working_dir: workingDir,
        batchFile: batFile,
        workingDir: workingDir
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      
      
      if (result.success) {
        installStatus = $_('repositories.started_success', { values: { repoName } });
      } else {
        installStatus = $_('repositories.start_error', { values: { repoName, error: result.stderr || result.stdout || $_('repositories.unknown_error') } });
      }
    } catch (error) {
      console.error('Error in runRepo:', error);
      installStatus = $_('repositories.start_error', { values: { repoName, error: String(error) } });
    }
  }

  async function updateRepo(repoName: string) {
    try {
      sidebarOpen = false;
      isUpdatingRepo = true;
      updatingRepoName = repoName;
      installStatus = $_('repositories.updating');
      
      // Use CLI command --update-repo
      const result = await invoke('run_cli_command', {
        install_path: installPath,
        installPath,
        args: ['--update-repo', repoName]
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        installStatus = $_('repositories.updated_success', { values: { repoName } });
      } else {
        installStatus = $_('repositories.update_error', { values: { repoName, error: result.stderr || $_('repositories.unknown_error') } });
      }
    } catch (error) {
      installStatus = $_('repositories.update_error', { values: { repoName, error: String(error) } });
    } finally {
      isUpdatingRepo = false;
      updatingRepoName = '';
    }
  }

  async function removeRepo(repoName: string) {
    try {
      sidebarOpen = false;
      isRemovingRepo = true;
      removingRepoName = repoName;
      installStatus = $_('repositories.removing');
      
      // Use CLI command to delete repository (with watchdog fallback)
      
      let completed = false;
      let result: { success: boolean; message: string } = { success: false, message: 'pending' };

      // Start native delete without awaiting to avoid hanging the handler
      (invoke('delete_repository', { install_path: installPath, installPath, repo_name: repoName, repoName: repoName }) as Promise<{success: boolean, message: string}>)
        .then((res) => {
          if (!completed) {
            completed = true;
            result = res;
            
            // trigger UI refresh
            loadInstalledRepos();
          }
        })
        .catch((e) => {
          console.error('delete_repository error (native)', e);
        });

      // Watchdog fallback after 2000ms if still not completed
      await new Promise((resolve) => setTimeout(resolve, 2000));
      if (!completed) {
        const envPath = `${installPath}\\envs\\${repoName}`;
        const repoPath = `${installPath}\\repos\\${repoName}`;
        try {
          await invoke('run_command', { command: `Remove-Item -Path "${envPath}" -Recurse -Force -ErrorAction SilentlyContinue`, working_dir: installPath, workingDir: installPath });
          await invoke('run_command', { command: `Remove-Item -Path "${repoPath}" -Recurse -Force -ErrorAction SilentlyContinue`, working_dir: installPath, workingDir: installPath });
          result = { success: true, message: 'removed by fallback' };
          completed = true;
          
        } catch (e) {
          console.error('fallback remove error', e);
          result = { success: false, message: String(e) } as any;
        }
      }
      
      // Update installed repositories list
      await loadInstalledRepos();
      
      if (result.success) {
        installStatus = $_('repositories.removed_success', { values: { repoName } });
      } else {
        installStatus = $_('repositories.installation_error', { values: { repoName, error: result.message } });
      }
    } catch (error) {
      installStatus = $_('repositories.installation_error', { values: { repoName, error: String(error) } });
    } finally {
      isRemovingRepo = false;
      removingRepoName = '';
    }
  }

  function toggleSidebar() {
    sidebarOpen = !sidebarOpen;
  }

  function setCurrentView(view: string) {
    currentView = view;
    if (view === 'top-repos') {
      loadAvailableRepos();
    } else if (view === 'installed-repos') {
      loadInstalledRepos();
    }
  }

  async function removeAllRepos() {
    try {
      installStatus = $_('repositories.removing');
      
      // Remove all folders from envs
      const envResult = await invoke('run_command', {
        command: `Remove-Item -Path "${installPath}\\envs\\*" -Recurse -Force -ErrorAction SilentlyContinue`,
        working_dir: installPath,
        workingDir: installPath
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      // Remove all folders from repos
      const repoResult = await invoke('run_command', {
        command: `Remove-Item -Path "${installPath}\\repos\\*" -Recurse -Force -ErrorAction SilentlyContinue`,
        working_dir: installPath,
        workingDir: installPath
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      // Update installed repositories list
      await loadInstalledRepos();
      
      if (envResult.success && repoResult.success) {
        installStatus = $_('common.success');
      } else {
        installStatus = $_('common.error');
      }
    } catch (error) {
      installStatus = $_('repositories.installation_error', { values: { repoName: 'all', error: String(error) } });
    }
  }

  async function removeSelectedRepo() {
    try {
      if (installedRepos.length === 0) {
        installStatus = $_('repositories.no_installed');
        return;
      }
      
      // Show repository list for selection
      const repoNames = installedRepos.map((repo, index) => `${index + 1}. ${repo.name}`);
      const repoList = repoNames.join('\n');
      
      const userChoice = prompt(`Select repository to remove:\n\n${repoList}\n\nEnter repository number (1-${installedRepos.length}):`);
      
      if (!userChoice) {
        installStatus = $_('common.cancel');
        return;
      }
      
      const selectedIndex = parseInt(userChoice) - 1;
      
      if (selectedIndex >= 0 && selectedIndex < installedRepos.length) {
        const repoToRemove = installedRepos[selectedIndex].name;
        
        // Confirm deletion
        const confirmDelete = confirm($_('repositories.confirm_remove', { values: { name: repoToRemove } }));
        
        if (confirmDelete) {
          await removeRepo(repoToRemove);
        } else {
          installStatus = $_('common.cancel');
        }
      } else {
        installStatus = $_('common.error');
      }
    } catch (error) {
      installStatus = `Repository removal error: ${error}`;
    }
  }

  async function completeUninstall() {
    try {
      // Confirm complete uninstallation
      const confirmUninstall = confirm($_('settings.confirm_uninstall'));
      
      if (!confirmUninstall) {
        installStatus = $_('common.cancel');
        return;
      }
      
      // Second confirmation
      const finalConfirm = confirm(
        'Final warning!\n\n' +
        'All your data will be permanently deleted.\n' +
        'Continue with complete uninstall?'
      );
      
      if (!finalConfirm) {
        installStatus = $_('common.cancel');
        return;
      }
      
      installStatus = $_('common.loading');
      
      const result = await invoke('complete_uninstall') as {success: boolean, message: string};
      
      if (result.success) {
        installStatus = result.message;
        // Reset all state
        installPath = '';
        cliInstalled = false;
        installedRepos = [];
        availableRepos = [];
        environmentStatus = {
          environment_exists: false,
          setup_completed: false,
          overall_status: 'Unknown'
        };
        currentStep = 'path-selection';
        
        // Show success message for a few seconds, then close app
        setTimeout(() => {
          // Try to close the application
          if (window.__TAURI__) {
            window.__TAURI__.process.exit(0);
          }
        }, 3000);
      } else {
        installStatus = `Uninstall error: ${result.message}`;
      }
    } catch (error) {
      installStatus = `Complete uninstall error: ${error}`;
    }
  }

  // App updater functions
  async function checkForUpdates() {
    try {
      isCheckingUpdates = true;
      updateInfo = null;
      
      const result = await invoke('check_for_updates');
      updateInfo = result;
      
    } catch (error) {
      console.error('Failed to check for updates:', error);
      updateInfo = { available: false, error: String(error) };
    } finally {
      isCheckingUpdates = false;
    }
  }

  async function installUpdate() {
    try {
      isInstallingUpdate = true;
      
      await invoke('install_update');
      
      // Show success message
      alert($_('updater.update_installed') + '\n' + $_('updater.restart_required'));
      
      // The app should restart automatically after update
      
    } catch (error) {
      console.error('Failed to install update:', error);
      alert($_('updater.update_failed', { values: { error: String(error) } }));
    } finally {
      isInstallingUpdate = false;
    }
  }

  // --- MSVC Build Tools helpers ---
  async function refreshMsvcStatus() {
    try {
      const [installed, admin] = await Promise.all([
        invoke('check_msvc_bt_installed') as Promise<boolean>,
        invoke('is_admin') as Promise<boolean>
      ]);
      msvcInstalled = installed;
      isAdminUser = admin;
    } catch (_) {
      msvcInstalled = false;
      isAdminUser = false;
    }
  }

  async function installMsvcBt() {
    if (msvcInstalled || !isAdminUser || isInstallingMsvc) return;
    try {
      isInstallingMsvc = true;
      await invoke('install_msvc_bt');
      await refreshMsvcStatus();
      alert($_('common.success'));
    } catch (e) {
      alert($_('common.error') + ': ' + String(e));
    } finally {
      isInstallingMsvc = false;
    }
  }
</script>

<main>
  <!-- Hamburger Menu Button - only show in main interface -->
  {#if currentStep === 'main-interface'}
    <button class="hamburger-btn" on:click={toggleSidebar} aria-label="Toggle navigation menu">
      <div class="hamburger-line"></div>
      <div class="hamburger-line"></div>
      <div class="hamburger-line"></div>
    </button>
  {/if}

  <!-- Sidebar - only show in main interface -->
  {#if currentStep === 'main-interface'}
    <div class="sidebar" class:open={sidebarOpen}>
    <div class="sidebar-content">
      <h3>{$_('app.title')}</h3>
      
      <!-- Navigation -->
      <div class="nav-section">
        <button 
          class="nav-item" 
          class:active={currentView === 'top-repos'}
          on:click={() => { setCurrentView('top-repos'); sidebarOpen = false; }}
        >
          🔥 {$_('repositories.top_repositories')}
        </button>
        <button 
          class="nav-item" 
          class:active={currentView === 'installed-repos'}
          on:click={() => { setCurrentView('installed-repos'); sidebarOpen = false; }}
        >
          📦 {$_('navigation.installed')}
        </button>
      </div>
      
      <!-- Settings at bottom -->
      <div class="settings-section">
        <button 
          class="nav-item settings-btn" 
          class:active={currentView === 'settings'}
          on:click={() => { setCurrentView('settings'); sidebarOpen = false; }}
        >
          ⚙️ {$_('navigation.settings')}
        </button>
      </div>
    </div>
    </div>
  {/if}

  <!-- Overlay - only show in main interface -->
  {#if currentStep === 'main-interface' && sidebarOpen}
    <div 
      class="overlay" 
      role="button" 
      tabindex="0"
      on:click={() => sidebarOpen = false}
      on:keydown={(e) => {
        if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') {
          sidebarOpen = false;
        }
      }}
      aria-label="Close side menu"
    ></div>
  {/if}

  <!-- Main Content -->
  <div class="main-content" class:shifted={sidebarOpen}>
    <!-- Initial Check Step -->
    {#if currentStep === 'initial-check'}
      <div class="step-container environment-step">
        <h1 class="step-title">{$_('app.title')}</h1>
        <p class="step-description">{$_('installation.checking_installation')}</p>
        
        <div class="installation-progress">
          <div class="spinner"></div>
          <p class="installation-text">{$_('installation.performing_checks')}</p>
        </div>
      </div>
    {/if}
    
    <!-- Environment Missing Dialog -->
    {#if currentStep === 'environment-missing'}
      <div class="step-container">
        <h1 class="step-title">{$_('installation.environment_missing_title')}</h1>
        <p class="step-description">
          {$_('installation.environment_missing_description')} <strong>{registryPath}</strong>
        </p>
        <p class="step-description">
          {$_('installation.environment_missing_instruction')}
        </p>
        
        <div class="button-group">
          <button 
            class="confirm-button" 
            on:click={handleNewInstallPath}
          >
            {$_('installation.select_new_path')}
          </button>
          
          <button 
            class="secondary-button" 
            on:click={clearRegistryAndSelectPath}
          >
            {$_('installation.clear_registry_key')}
          </button>
        </div>
      </div>
    {/if}
    
    <!-- Step 1: Path Selection -->
    {#if currentStep === 'path-selection'}
      <div class="step-container">
        <h1 class="step-title">{$_('app.installer_title')}</h1>
        <p class="step-description">{$_('installation.select_folder')}</p>
        
        <div class="path-input-container">
          <input 
            type="text" 
            bind:value={installPath} 
            placeholder="{$_('installation.select_installation_folder')}"
            readonly
          />
          <button on:click={selectInstallPath}>
            {$_('installation.select_folder_btn')}
          </button>
        </div>
        
        {#if installPath}
          <button 
            class="confirm-button" 
            on:click={savePathAndStartInstallation}
            disabled={!installPath}
          >
            {$_('installation.confirm_start')}
          </button>
        {/if}
      </div>
    {/if}
    
    <!-- Единый экран прогресса установки/настройки -->
    {#if currentStep === 'installing'}
      <div class="step-container installation-step">
        <h2 class="nice-title">{$_('installation.installing_env')}</h2>
        <div class="installation-progress fancy">
          <div class="big-icon">{currentToolIcon}</div>
          <p class="installation-text">{envProgressText || installStatus}</p>
          <p class="timer">{$_('installation.time_elapsed', { values: { time: formatDuration(installTimer) } })}</p>
          <div class="progress-container glass">
            <div class="progress-bar">
              <div class="progress-fill gradient" style="width: {installProgress}%"></div>
            </div>
            <div class="progress-text">{Math.round(installProgress)}%</div>
          </div>
        </div>
      </div>
    {/if}
    
    <!-- Step 4: Main Interface -->
    {#if currentStep === 'main-interface'}
      <!-- Successful installation notification -->
      {#if showInstallNotification}
        <div class="install-notification">
          <div class="notification-content">
            <span class="notification-icon">✅</span>
            <span class="notification-text">{$_('repositories.successfully_installed', { values: { name: installedRepoName } })}</span>
            <button class="notification-close" on:click={() => showInstallNotification = false}>×</button>
          </div>
        </div>
      {/if}
      
      <div class="content-header">
        <h1>
          {#if currentView === 'top-repos'}
            🔥 {$_('repositories.top_repositories')}
          {:else if currentView === 'installed-repos'}
            📦 {$_('repositories.installed_repositories')}
          {:else if currentView === 'settings'}
            ⚙️ {$_('navigation.settings')}
          {/if}
        </h1>
      </div>

      <!-- Top Repositories View -->
      {#if currentView === 'top-repos'}
        <!-- Add by URL/name CTA on top -->
        <div class="add-repo-cta">
          <button class="add-repo-btn" title={$_('repositories.install_by_url')} on:click={openAddRepoModal}>
            + {$_('repositories.install_by_url')}
          </button>
        </div>

        <div class="repos-grid">
           {#each availableRepos as repo}
             <div class="repo-card">
               <h3>{repo.name}</h3>
               <p>{repo.description}</p>
               {#if repo.downloadCount}
                 <div class="repo-stats">
                   <span class="download-count">📥 {$_('repositories.downloads', { values: { count: repo.downloadCount } })}</span>
                   {#if repo.uploadedByUsername}
                     <span class="author">👤 {repo.uploadedByUsername}</span>
                   {/if}
                 </div>
               {/if}
               {#await checkRepoInstallStatus(repo.name)}
                 <button class="install-repo-btn" disabled>{$_('repositories.checking')}</button>
               {:then isInstalled}
                 {#if isInstalled}
                   <button class="open-repo-btn" on:click={() => setCurrentView('installed-repos')}>
                     {$_('repositories.open')}
                   </button>
                 {:else if isInstallingRepo && installingRepoName === repo.name}
                   <button class="install-repo-btn installing" disabled>
                     <span class="spinner"></span>
                     {$_('repositories.installing')}
                   </button>
                 {:else}
                   <button class="install-repo-btn" on:click={() => {
                     installRepo(repo.name);
                   }} disabled={isInstallingRepo}>{$_('repositories.install')}</button>
                 {/if}
               {:catch}
                 {#if isInstallingRepo && installingRepoName === repo.name}
                   <button class="install-repo-btn installing" disabled>
                     <span class="spinner"></span>
                     {$_('repositories.installing')}
                   </button>
                 {:else}
                   <button class="install-repo-btn" on:click={() => {
                     installRepo(repo.name);
                   }} disabled={isInstallingRepo}>{$_('repositories.install')}</button>
                 {/if}
               {/await}
             </div>
           {/each}
         </div>

          {#if showAddRepoModal}
           <div class="modal-backdrop" role="button" tabindex="0" aria-label="Close modal"
                on:click={closeAddRepoModal}
                on:keydown={(e) => { if (e.key==='Escape' || e.key==='Enter' || e.key===' ') closeAddRepoModal(); }}></div>
           <div class="modal-card" role="dialog" aria-modal="true">
             <h3>🧩 {$_('repositories.install_by_url')}</h3>
             <p class="modal-sub">{$_('repositories.enter_repo_url')}</p>
             <input type="text" bind:value={newRepoInput} placeholder={$_('repositories.repo_input_placeholder')} on:keydown={(e) => { if (e.key==='Enter') confirmAddRepoModal(); if (e.key==='Escape') closeAddRepoModal(); }} />
             <div class="modal-actions">
               <button class="btn-secondary" on:click={closeAddRepoModal}>{$_('common.cancel')}</button>
               <button class="btn-primary" on:click={confirmAddRepoModal}>{$_('common.confirm')}</button>
             </div>
           </div>
         {/if}


         
         <!-- Installation Status Display -->
         {#if installStatus && isInstallingRepo}
           <div class="installation-status">
             <div class="status-content">
               <span class="spinner"></span>
               <span class="status-text">{installStatus}</span>
             </div>
           </div>
         {/if}
      {/if}

      <!-- Installed Repositories View -->
      {#if currentView === 'installed-repos'}
        <div class="installed-repos">
          {#if installedRepos.length === 0}
            <div class="empty-state">
              <p>{$_('repositories.no_installed')}</p>
              <button on:click={() => setCurrentView('top-repos')}>{$_('repositories.view_top_repositories')}</button>
            </div>
          {:else}
            <div class="repos-list">
              {#each installedRepos as repo}
                <div class="installed-repo-item">
                  <h3>{repo.name} {#if repo.sourceLabel}<span class="repo-source-badge" class:github={repo.sourceLabel==='GitHub'} class:git={repo.sourceLabel==='Git'} class:server={repo.sourceLabel==='Сервер'}>{repo.sourceLabel}</span>{/if}</h3>
                  <div class="repo-actions">
                    {#if repo.hasLauncher}
                      <button class="launch-btn" on:click={() => runRepo(repo.name)}>{$_('repositories.launch')}</button>
                    {/if}
                    {#if isUpdatingRepo && updatingRepoName === repo.name}
                      <button class="update-btn updating" disabled>
                        <span class="spinner"></span>
                        {$_('repositories.updating')}
                      </button>
                    {:else}
                      <button class="update-btn" on:click={() => updateRepo(repo.name)} disabled={isUpdatingRepo || isRemovingRepo}>{$_('repositories.update')}</button>
                    {/if}
                    {#if isRemovingRepo && removingRepoName === repo.name}
                      <button class="remove-btn removing" disabled>
                        <span class="spinner"></span>
                        {$_('repositories.removing')}
                      </button>
                    {:else}
                      <button class="remove-btn" on:click={() => removeRepo(repo.name)} disabled={isUpdatingRepo || isRemovingRepo}>{$_('repositories.remove')}</button>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <!-- Settings View -->
      {#if currentView === 'settings'}
        <div class="settings-content">
          <div class="settings-section">
            <h2>{$_('settings.installation_path')}</h2>
            <div class="path-selector">
              <input 
                type="text" 
                bind:value={installPath} 
                placeholder="{$_('installation.select_installation_folder')}"
                readonly
              />
              <button on:click={selectInstallPath} disabled={isInstalling}>
                {$_('settings.change')}
              </button>
            </div>
          </div>

          <div class="settings-section">
            <h2>🔄 {$_('updater.check_for_updates')}</h2>
            
            <!-- Version Status Display -->
            {#if currentAppVersion}
              {#if isCheckingUpdates}
                <p class="info">{$_('updater.checking_updates')}</p>
              {:else if updateInfo && updateInfo.available}
                <p class="warning">✗ {$_('updater.version_outdated')}</p>
              {:else}
                <p class="success">{$_('updater.version_latest', { values: { version: currentAppVersion } })}</p>
              {/if}
            {/if}
            
            {#if updateInfo}
              {#if updateInfo.available}
                <p class="version-info">{$_('updater.new_version', { values: { version: updateInfo.version } })}</p>
                {#if updateInfo.body}
                  <details class="release-notes">
                    <summary>{$_('updater.release_notes')}</summary>
                    <div class="release-notes-content">{updateInfo.body}</div>
                  </details>
                {/if}
                <div class="action-buttons">
                  <button on:click={checkForUpdates} disabled={isInstallingUpdate}>{$_('updater.check_for_updates')}</button>
                  {#if isInstallingUpdate}
                    <button class="update-btn updating" disabled>
                      <span class="spinner"></span>
                      {$_('updater.installing_update')}
                    </button>
                  {:else}
                    <button class="update-btn" on:click={installUpdate}>{$_('updater.install_update')}</button>
                  {/if}
                </div>
              {:else}
                <div class="action-buttons">
                  <button on:click={checkForUpdates}>{$_('updater.check_for_updates')}</button>
                </div>
              {/if}
            {:else}
              <div class="action-buttons">
                <button on:click={checkForUpdates}>{$_('updater.check_for_updates')}</button>
              </div>
            {/if}
          </div>

          <!-- MSVC Build Tools Section -->
          <div class="settings-section">
            <h2>{$_('msvc.title')}</h2>
            {#if msvcInstalled === null}
              <p class="info">{$_('common.loading')}</p>
            {:else if msvcInstalled}
              <p class="success">{$_('msvc.installed')}</p>
            {:else}
              <p class="warning">{$_('msvc.not_installed')}</p>
              {#if !isAdminUser}
                <p class="warning">{$_('msvc.not_admin')}</p>
              {/if}
            {/if}
            <div class="action-buttons">
              <button on:click={installMsvcBt} disabled={msvcInstalled || !isAdminUser || isInstallingMsvc}>
                {#if isInstallingMsvc}
                  <span class="spinner"></span>
                {/if}
                {$_('msvc.install_button')}
              </button>
            </div>
          </div>

          <div class="settings-section">
            <h2>🌐 Выбор языка / Language Selection</h2>
            <div class="language-selector">
              <button 
                class="language-btn" 
                class:active={$locale === 'ru'}
                on:click={() => $locale = 'ru'}
              >
                RU Русский
              </button>
              <button 
                class="language-btn" 
                class:active={$locale === 'en'}
                on:click={() => $locale = 'en'}
              >
                EN English
              </button>
            </div>
          </div>

          <div class="settings-section">
            <h2>🎨 {$_('settings.theme')}</h2>
            <div class="theme-selector">
              <button 
                class="theme-btn" 
                class:active={currentTheme === 'light'}
                on:click={() => setTheme('light')}
              >
                ☀️ {$_('settings.theme_light')}
              </button>
              <button 
                class="theme-btn" 
                class:active={currentTheme === 'dark'}
                on:click={() => setTheme('dark')}
              >
                🌙 {$_('settings.theme_dark')}
              </button>
              <button 
                class="theme-btn" 
                class:active={currentTheme === 'system'}
                on:click={() => setTheme('system')}
              >
                🖥️ {$_('settings.theme_system')}
              </button>
            </div>
          </div>

           <div class="settings-section">
              <h2>{$_('repositories.repository_management')}</h2>
              <div class="action-buttons">
                <button class="reset-btn" on:click={removeAllRepos}>{$_('repositories.remove_all')}</button>
                <button class="reset-btn" on:click={removeSelectedRepo}>{$_('repositories.remove_selected')}</button>
              </div>
            </div>

            <div class="settings-section danger-section">
              <h2>⚠️ {$_('settings.danger_zone')}</h2>
              <p class="danger-warning">
                {$_('settings.danger_warning')}
              </p>
              <div class="action-buttons">
                <button class="danger-btn" on:click={openUninstallModal}>
                  🗑️ {$_('settings.complete_uninstall')}
                </button>
              </div>
            </div>
        </div>
      {/if}
    {/if}
  </div>

  <!-- Complete Uninstall Modal - Global -->
  {#if showUninstallModal}
    <div class="modal-backdrop" role="button" tabindex="0" aria-label="Close modal"
         on:click={closeUninstallModal}
         on:keydown={(e) => { if (e.key==='Escape') closeUninstallModal(); }}></div>
    <div class="modal-card danger-modal" role="dialog" aria-modal="true">
      <div class="modal-header">
        <h3>⚠️ {$_('settings.confirm_uninstall')}</h3>
      </div>
      <div class="modal-body">
        <div class="danger-warning-box">
          <p class="danger-text">{$_('settings.uninstall_warning_text')}</p>
          <ul class="uninstall-list">
            <li>• {$_('settings.uninstall_items_list.repositories')}</li>
            <li>• {$_('settings.uninstall_items_list.environments')}</li>
            <li>• {$_('settings.uninstall_items_list.program_files')}</li>
            <li>• {$_('settings.uninstall_items_list.registry')}</li>
          </ul>
          <p class="final-warning">{$_('settings.uninstall_irreversible')}</p>
        </div>
        <div class="confirmation-input">
          <p class="confirm-instruction">{$_('settings.uninstall_confirm_question')}</p>
          <p class="type-instruction">{$_('settings.uninstall_type_delete')}</p>
          <input 
            type="text" 
            bind:value={uninstallConfirmText} 
            placeholder={$_('settings.uninstall_placeholder')}
            class="danger-input"
            on:keydown={(e) => { 
              if (e.key==='Enter' && uninstallConfirmText.toLowerCase() === 'delete') confirmUninstallModal(); 
              if (e.key==='Escape') closeUninstallModal(); 
            }} 
          />
        </div>
      </div>
      <div class="modal-actions">
        <button class="btn-secondary" on:click={closeUninstallModal}>{$_('settings.uninstall_cancel')}</button>
        <button 
          class="btn-danger" 
          class:disabled={uninstallConfirmText.toLowerCase() !== 'delete'}
          disabled={uninstallConfirmText.toLowerCase() !== 'delete'}
          on:click={confirmUninstallModal}
        >
          🗑️ {$_('settings.uninstall_button')}
        </button>
      </div>
    </div>
  {/if}
</main>

<style>
  /* Global Styles */
  :global(body) {
    margin: 0;
    padding: 0;
    background: var(--bg-primary);
    min-height: 100vh;
    font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
    overflow-x: hidden;
  }

  main {
    position: relative;
    min-height: 100vh;
  }

  /* Hamburger Menu */
  .hamburger-btn {
    position: fixed;
    top: 20px;
    left: 20px;
    z-index: 1001;
    background: var(--bg-hamburger);
    border: none;
    border-radius: 8px;
    padding: 12px;
    cursor: pointer;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
    transition: all 0.3s ease;
  }

  .hamburger-btn:hover {
    background: var(--card-bg);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.15);
  }

  .hamburger-line {
    width: 20px;
    height: 2px;
    background: var(--text-dark);
    margin: 4px 0;
    transition: 0.3s;
  }

  /* Sidebar */
  .sidebar {
    position: fixed;
    top: 0;
    left: 0;
    width: 300px;
    height: 100vh;
    background: var(--bg-primary);
    box-shadow: 2px 0 15px rgba(0, 0, 0, 0.08);
    transform: translateX(-100%);
    transition: transform 280ms cubic-bezier(0.22, 1, 0.36, 1);
    will-change: transform;
    contain: paint;
    z-index: 1000;
    border-right: 1px solid var(--border-color);
    backface-visibility: hidden;
  }

  .sidebar.open {
    transform: translateX(0);
  }

  .sidebar-content {
    padding: 80px 20px 20px;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .sidebar-content h3 {
    color: var(--text-primary);
    font-weight: 600;
    font-size: 1.5rem;
    margin-bottom: 30px;
    text-align: center;
  }

  .nav-section {
    flex: 1;
  }

  .nav-item {
    display: block;
    width: 100%;
    padding: 15px 20px;
    margin-bottom: 8px;
    background: transparent;
    border: none;
    border-radius: 12px;
    text-align: left;
    font-size: 16px;
    color: var(--text-muted-light);
    cursor: pointer;
    transition: background 160ms ease, color 160ms ease, transform 160ms ease;
    will-change: transform;
  }

  .nav-item:hover {
    background: var(--bg-tertiary);
    color: var(--text-dark);
  }

  .nav-item.active {
    background: linear-gradient(135deg, var(--button-primary) 0%, var(--button-primary-hover) 100%);
    color: var(--text-primary);
    box-shadow: 0 2px 8px rgba(0, 122, 204, 0.3);
  }

  .settings-section {
    border-top: 1px solid #dee2e6;
    padding-top: 20px;
  }

  .settings-btn {
    background: var(--bg-tertiary) !important;
    border: 1px solid var(--card-border) !important;
  }

  .settings-btn:hover {
    background: var(--bg-hover) !important;
  }

  .settings-btn.active {
    background: linear-gradient(135deg, var(--text-muted-light) 0%, var(--text-dark) 100%) !important;
    border-color: var(--text-dark) !important;
  }

  /* Overlay */
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: var(--overlay-bg);
    z-index: 999;
    animation: fadeIn 180ms ease forwards;
    will-change: opacity;
  }

  /* Main Content */
  .main-content {
    padding: 40px 20px;
    min-height: 100vh;
    transition: margin-left 260ms ease;
    will-change: margin-left;
  }

  .main-content.shifted {
    margin-left: 300px;
  }

  .content-header {
    text-align: center;
    margin-bottom: 0px;
  }

  .content-header h1 {
    color: var(--text-dark);
    font-weight: 300;
    font-size: 2.5rem;
    margin: 0;
  }

  /* Installation Steps Styles - Pastel Grey Theme */
  .step-container {
    max-width: 700px;
    margin: 0 auto;
    text-align: center;
    padding: 60px 40px;
    background: var(--card-bg);
    border-radius: 20px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.1);
    border: 1px solid var(--card-border);
  }

  .step-title {
    font-size: 32px;
    color: var(--text-primary);
    margin-bottom: 15px;
    font-weight: 300;
    letter-spacing: -0.5px;
  }
  .nice-title {
    letter-spacing: 0.3px;
    background: linear-gradient(90deg, var(--text-primary), var(--accent-color));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .step-description {
    font-size: 18px;
    color: var(--text-secondary);
    margin-bottom: 40px;
    line-height: 1.6;
  }

  .path-input-container {
    display: flex;
    gap: 20px;
    margin-bottom: 40px;
    align-items: stretch;
    background: var(--input-bg);
    padding: 8px;
    border-radius: 16px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
    border: 2px solid var(--input-border);
  }

  .path-input-container input {
    flex: 1;
    padding: 20px 24px;
    font-size: 16px;
    border: none;
    border-radius: 12px;
    background: transparent;
    color: var(--text-dark);
    outline: none;
    font-weight: 400;
  }

  .path-input-container input::placeholder {
    color: var(--text-light-gray);
    font-weight: 300;
  }

  .path-input-container button {
    padding: 20px 32px;
    font-size: 16px;
    background: linear-gradient(135deg, var(--text-light-gray) 0%, var(--text-muted) 100%);
    color: var(--text-primary);
    border: none;
    border-radius: 12px;
    font-weight: 500;
    transition: all 0.3s ease;
    box-shadow: 0 2px 8px rgba(173, 181, 189, 0.3);
  }

  .path-input-container button:hover:not(:disabled) {
    background: linear-gradient(135deg, var(--text-muted) 0%, var(--text-dark) 100%);
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(173, 181, 189, 0.4);
  }

  .confirm-button {
    background: linear-gradient(135deg, var(--text-light-gray) 0%, var(--text-muted) 100%);
    color: var(--text-primary);
    border: none;
    padding: 18px 48px;
    font-size: 18px;
    border-radius: 16px;
    cursor: pointer;
    transition: all 0.3s ease;
    margin-top: 20px;
    font-weight: 500;
    letter-spacing: 0.5px;
  }

  .confirm-button:hover:not(:disabled) {
    background: linear-gradient(135deg, var(--text-muted) 0%, var(--text-dark) 100%);
    transform: translateY(-3px);
    box-shadow: 0 8px 25px rgba(173, 181, 189, 0.4);
  }

  .confirm-button:disabled {
    background: var(--bg-tertiary);
    color: var(--text-light-gray);
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
  }

  .button-group {
    display: flex;
    flex-direction: column;
    gap: 15px;
    margin-top: 30px;
    align-items: center;
  }

  .secondary-button {
    background: linear-gradient(135deg, var(--warning-color) 0%, var(--warning-dark) 100%);
    color: white;
    border: none;
    padding: 15px 36px;
    font-size: 16px;
    border-radius: 12px;
    cursor: pointer;
    transition: all 0.3s ease;
    font-weight: 500;
    letter-spacing: 0.3px;
  }

  .secondary-button:hover {
    background: linear-gradient(135deg, var(--warning-dark) 0%, var(--warning-color) 100%);
    transform: translateY(-2px);
    box-shadow: var(--shadow-warning);
  }

  /* removed .status */

  /* Repositories Grid */
  .repos-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 20px;
    max-width: 1200px;
    margin: 0 auto;
  }

  .repo-card {
    background: var(--card-bg);
    padding: 25px;
    border-radius: 16px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
    border: 1px solid var(--card-border);
    transition: all 0.3s ease;
  }

  .repo-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(0, 0, 0, 0.12);
  }

  .repo-card h3 {
    color: var(--text-dark);
    font-size: 1.3rem;
    margin-bottom: 10px;
    font-weight: 600;
  }

  .repo-card p {
    color: var(--text-muted-light);
    line-height: 1.5;
    margin-bottom: 20px;
  }

  .repo-stats {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 1rem;
    font-size: 0.8rem;
  }

  .download-count {
    color: var(--success-color);
    font-weight: 500;
  }

  .author {
    color: var(--text-muted-light);
  }

  .install-repo-btn {
    background: linear-gradient(135deg, var(--button-primary) 0%, var(--button-primary-hover) 100%);
    color: var(--text-primary);
    border: none;
    padding: 12px 24px;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 500;
    transition: transform 150ms ease, box-shadow 150ms ease, opacity 150ms ease;
    will-change: transform;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }

  .install-repo-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
    transform: none;
  }

  .install-repo-btn.installing {
    background: linear-gradient(135deg, var(--text-muted-light) 0%, var(--text-dark) 100%);
  }



  .open-repo-btn {
    background: linear-gradient(135deg, var(--button-primary) 0%, var(--button-primary-hover) 100%);
    color: var(--text-primary);
    border: none;
    padding: 12px 24px;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 500;
    transition: transform 150ms ease, box-shadow 150ms ease;
    will-change: transform;
    box-shadow: 0 2px 8px rgba(23, 162, 184, 0.3);
  }

  .open-repo-btn:hover {
    background: linear-gradient(135deg, var(--button-primary-hover) 0%, var(--button-primary-active) 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(23, 162, 184, 0.4);
  }

  /* Modal styles */
  .modal-backdrop {
    position: fixed; inset: 0; background: var(--modal-backdrop); z-index: 1100;
    animation: fadeIn 160ms ease;
  }
  .modal-card {
    position: fixed; z-index: 1101;
    left: 50%; top: 50%; transform: translate(-50%, -50%);
    width: min(520px, 92vw);
    max-height: 80vh;
    background: var(--card-bg); border-radius: 16px; padding: 20px;
    box-shadow: 0 20px 60px rgba(0,0,0,0.25);
    border: 1px solid var(--card-border);
    animation: fadeIn 180ms ease;
    overflow: auto;
  }
  .modal-card h3 { margin: 0 0 8px 0; color: var(--text-primary); font-weight: 700; }
  .modal-sub { margin: 0 0 14px 0; color: var(--text-secondary); }
  .modal-card input {
    width: 100%; padding: 12px 14px; border: 1px solid var(--input-border); border-radius: 10px; outline: none;
    font-size: 14px; background: var(--input-bg); color: var(--text-primary);
  }
  .modal-card input:focus { border-color: var(--input-focus); box-shadow: 0 0 0 3px rgba(96, 165, 250, 0.15); }
  .modal-actions { display: flex; gap: 10px; justify-content: flex-end; margin-top: 14px; }
  .btn-secondary { background: var(--button-secondary); border: none; padding: 10px 16px; border-radius: 8px; cursor: pointer; color: var(--button-secondary-text); }
  .btn-primary { background: var(--button-primary); color: var(--text-primary); border: none; padding: 10px 16px; border-radius: 8px; cursor: pointer; }
  .btn-primary:hover { background: var(--button-primary-hover); }

  /* Danger Modal Styles */
  .danger-modal {
    border: 2px solid var(--button-danger);
    box-shadow: 0 20px 60px rgba(220, 53, 69, 0.3);
  }
  
  .modal-header {
    border-bottom: 2px solid var(--danger-color);
    padding-bottom: 15px;
    margin-bottom: 20px;
  }
  
  .modal-header h3 {
    color: var(--danger-color) !important;
    font-size: 20px;
    font-weight: 700;
    margin: 0;
  }
  
  .modal-body {
    margin-bottom: 20px;
  }
  
  .danger-warning-box {
    background: linear-gradient(135deg, var(--bg-danger-light) 0%, var(--bg-danger-lighter) 100%);
    border: 1px solid #dc3545;
    border-radius: 12px;
    padding: 20px;
    margin-bottom: 20px;
  }
  
  .danger-text {
    color: var(--text-error-dark);
    font-weight: 600;
    margin: 0 0 15px 0;
    font-size: 16px;
  }
  
  .uninstall-list {
    color: var(--text-error-dark);
    margin: 15px 0;
    padding-left: 0;
    list-style: none;
  }
  
  .uninstall-list li {
    margin: 8px 0;
    font-weight: 500;
  }
  
  .final-warning {
    color: var(--danger-color);
    font-weight: 700;
    font-size: 18px;
    text-align: center;
    margin: 20px 0 0 0;
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  
  .confirmation-input {
    background: var(--bg-primary);
    border: 2px solid var(--danger-color);
    border-radius: 12px;
    padding: 20px;
  }
  
  .confirm-instruction {
    color: var(--text-dark-gray);
    font-weight: 600;
    margin: 0 0 10px 0;
    font-size: 16px;
  }
  
  .type-instruction {
    color: var(--text-muted-light);
    margin: 0 0 15px 0;
    font-size: 14px;
  }
  
  .danger-input {
    width: 100%;
    padding: 12px 16px;
    border: 2px solid var(--danger-color);
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
    text-align: center;
    text-transform: uppercase;
    letter-spacing: 2px;
    outline: none;
    transition: all 0.3s ease;
  }
  
  .danger-input:focus {
    border-color: var(--border-error);
    box-shadow: 0 0 0 3px rgba(220, 53, 69, 0.25);
  }
  
  .btn-danger {
    background: linear-gradient(135deg, var(--danger-color) 0%, var(--button-danger-hover) 100%);
    color: var(--text-primary);
    border: 2px solid var(--danger-color);
    padding: 12px 20px;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 600;
    font-size: 16px;
    transition: all 0.3s ease;
  }
  
  .btn-danger:hover:not(:disabled) {
    background: linear-gradient(135deg, var(--button-danger-hover) 0%, var(--border-error) 100%);
    border-color: var(--border-error);
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(220, 53, 69, 0.4);
  }
  
  .btn-danger.disabled,
  .btn-danger:disabled {
    background: var(--text-muted-light);
    border-color: var(--text-muted-light);
    cursor: not-allowed;
    opacity: 0.6;
    transform: none;
    box-shadow: none;
  }

  /* Add by URL CTA */
  .add-repo-cta { display: flex; justify-content: center; margin: 20px 0 20px 0; }
  .add-repo-btn {
    padding: 14px 20px;
    border-radius: 12px;
    border: none;
    background: linear-gradient(135deg, var(--success-color) 0%, var(--success-hover) 100%);
    color: var(--text-primary);
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 8px 24px rgba(32, 201, 151, 0.25);
    transition: transform 120ms ease, box-shadow 120ms ease;
  }
  .add-repo-btn:hover { transform: translateY(-1px); box-shadow: 0 12px 28px rgba(32,201,151,0.35); }
  .add-repo-btn:active { transform: translateY(0); }
  @media (max-width: 640px) { .add-repo-btn { width: 100%; max-width: 480px; } }

  .install-repo-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(0, 122, 204, 0.3);
  }

  /* Installed Repositories */
  .installed-repos {
    max-width: 800px;
    margin: 0 auto;
  }

  .empty-state {
    text-align: center;
    padding: 60px 20px;
    background: var(--card-bg);
    border-radius: 16px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
  }

  .empty-state p {
    color: var(--text-muted-light);
    font-size: 1.1rem;
    margin-bottom: 20px;
  }

  .empty-state button {
    background: linear-gradient(135deg, var(--button-primary) 0%, var(--button-primary-hover) 100%);
    color: var(--text-primary);
    border: none;
    padding: 12px 24px;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 500;
    transition: all 0.3s ease;
  }

  .repos-list {
    display: flex;
    flex-direction: column;
    gap: 15px;
  }

  .installed-repo-item {
    background: var(--card-bg);
    padding: 20px;
    border-radius: 12px;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.06);
    will-change: transform;
    border: 1px solid var(--card-border);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .installed-repo-item h3 {
    color: var(--text-dark);
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
  }
  .repo-source-badge {
    margin-left: 8px;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    color: var(--repo-badge-success-text);
    background: var(--repo-badge-success);
    border: 1px solid var(--repo-badge-success-border);
  }
  .repo-source-badge.github { color: var(--text-github); background: var(--bg-github); border-color: var(--border-github); }
  .repo-source-badge.git { color: var(--text-git); background: var(--bg-git); border-color: var(--border-git); }
  .repo-source-badge.server { color: var(--repo-badge-success-text); background: var(--repo-badge-success); border-color: var(--repo-badge-success-border); }

  .repo-actions {
    display: flex;
    gap: 10px;
  }

  .launch-btn, .update-btn, .remove-btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    transition: transform 150ms ease, box-shadow 150ms ease, opacity 150ms ease;
    will-change: transform;
  }

  .launch-btn {
    background: linear-gradient(135deg, var(--success-color) 0%, var(--success-hover) 100%);
    color: var(--text-primary);
  }

  .update-btn {
    background: var(--button-primary);
    color: var(--text-primary);
  }

  .remove-btn {
    background: var(--button-danger);
    color: var(--text-primary);
  }

  .launch-btn:hover, .update-btn:hover, .remove-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  }

  /* Settings Content */
  .settings-content {
    max-width: 800px;
    margin: 0 auto;
  }

  .settings-section {
    background: var(--card-bg);
    padding: 30px;
    border-radius: 12px;
    margin-bottom: 20px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
    border: 1px solid var(--card-border);
  }

  .settings-section h2 {
    color: var(--text-dark);
    font-size: 1.3rem;
    margin-bottom: 15px;
    font-weight: 600;
  }

  .path-selector {
    display: flex;
    gap: 15px;
    align-items: center;
  }

  .path-selector input {
    flex: 1;
    padding: 12px 16px;
    border: 1px solid var(--border-color);
    border-radius: 8px;
    font-size: 14px;
    background: var(--input-bg);
    color: var(--text-color);
  }

  .path-selector button {
    padding: 12px 24px;
    background: var(--button-primary);
    color: var(--text-primary);
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.3s;
  }

  .path-selector button:hover:not(:disabled) {
    background: var(--button-primary-hover);
  }

  .action-buttons {
    display: flex;
    gap: 10px;
    margin-top: 15px;
  }

  .action-buttons button {
    padding: 10px 20px;
    background: var(--button-primary);
    color: var(--text-primary);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.3s;
  }

  .reset-btn {
    background: var(--danger-color) !important;
  }

  .reset-btn:hover:not(:disabled) {
    background: var(--button-danger-hover) !important;
  }

  /* Danger Zone Styles */
  .danger-section {
    border: 2px solid var(--danger-color) !important;
    background: linear-gradient(135deg, var(--bg-danger-light) 0%, var(--bg-danger-lighter) 100%) !important;
  }

  .danger-section h2 {
    color: var(--danger-color) !important;
    font-weight: 700;
  }

  .danger-warning {
    color: var(--text-error-dark);
    font-weight: 500;
    margin: 15px 0;
    padding: 15px;
    background: var(--bg-danger-alpha);
    border-radius: 8px;
    border-left: 4px solid var(--danger-color);
  }

  .danger-btn {
    background: linear-gradient(135deg, var(--danger-color) 0%, var(--button-danger-hover) 100%) !important;
    color: var(--text-primary) !important;
    border: 2px solid var(--danger-color) !important;
    font-weight: 600 !important;
    font-size: 16px !important;
    padding: 12px 24px !important;
    transition: all 0.3s ease !important;
  }

  .danger-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, var(--button-danger-hover) 0%, var(--button-danger-active) 100%) !important;
    border-color: var(--button-danger-active) !important;
    transform: translateY(-2px) !important;
    box-shadow: 0 6px 20px var(--shadow-danger) !important;
  }

  .success {
    color: var(--success-color);
    font-weight: bold;
    margin: 10px 0;
  }

  .warning {
    color: var(--warning-color);
    font-weight: bold;
    margin: 10px 0;
  }

  .action-buttons {
    display: flex;
    gap: 10px;
    margin-top: 15px;
    flex-wrap: wrap;
  }

  .action-buttons button {
    padding: 10px 20px;
    background: var(--button-primary);
    color: var(--text-primary);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.3s;
    font-size: 14px;
    font-weight: 500;
  }

  .action-buttons button:disabled {
    background: var(--button-disabled-bg);
    color: var(--button-disabled-text);
    cursor: not-allowed;
  }

  .action-buttons button:hover:not(:disabled) {
    background: var(--button-primary-hover);
    transform: translateY(-1px);
  }

  .reset-btn {
    background: var(--danger-color) !important;
  }

  .reset-btn:hover:not(:disabled) {
    background: var(--button-danger-hover) !important;
  }

  /* Install Notification */
  .install-notification {
    position: fixed;
    top: 20px;
    right: 20px;
    z-index: 1000;
    animation: slideInRight 0.3s ease-out;
  }

  .notification-content {
    background: linear-gradient(135deg, var(--success-color) 0%, var(--success-hover) 100%);
    color: var(--text-primary);
    padding: 16px 20px;
    border-radius: 12px;
    box-shadow: 0 8px 25px var(--shadow-success);
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 300px;
    max-width: 400px;
  }

  .notification-icon {
    font-size: 20px;
    flex-shrink: 0;
  }

  .notification-text {
    flex: 1;
    font-weight: 500;
    font-size: 14px;
  }

  .notification-close {
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 20px;
    cursor: pointer;
    padding: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    transition: background 0.2s;
  }

  .notification-close:hover {
    background: var(--bg-hover-alpha);
  }

  @keyframes slideInRight {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  /* Environment Setup Page Styles (kept minimal now) */
  .environment-step { max-width: 600px; margin: 0 auto; text-align: center; }

  /* Progress Bar Styles */
  .progress-container {
    margin-top: 20px;
    width: 100%;
    max-width: 400px;
    margin-left: auto;
    margin-right: auto;
  }
  .installation-progress.fancy { display: flex; flex-direction: column; align-items: center; gap: 10px; }
  .big-icon { font-size: 40px; filter: drop-shadow(0 2px 6px rgba(0,0,0,0.15)); }
  .progress-container.glass {
    background: var(--bg-glass);
    backdrop-filter: blur(8px);
    border-radius: 12px;
    padding: 10px 14px;
    border: 1px solid var(--border-glass);
    box-shadow: 0 6px 20px var(--shadow-glass);
  }
  .progress-fill.gradient {
    background: linear-gradient(90deg, var(--button-primary) 0%, var(--success-color) 100%);
    box-shadow: inset 0 2px 8px var(--shadow-inset);
  }

  .progress-bar {
    width: 100%;
    height: 14px;
    background: var(--progress-bg); /* цвет подложки (трек) в стиле карточки */
    border: 1px solid var(--border-color);
    border-radius: 8px;
    overflow: hidden;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.08), 0 1px 0 rgba(255,255,255,0.6);
  }

  .progress-fill {
    height: 100%;
    background: var(--progress-fill);
    border-radius: 6px;
    transition: width 0.3s ease;
    position: relative;
    overflow: hidden;
  }

  .progress-fill::after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(
      90deg,
      transparent 0%,
      var(--shimmer-color) 50%,
      transparent 100%
    );
    animation: shimmer 2s infinite;
  }

  @keyframes shimmer {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  .progress-text {
    text-align: center;
    margin-top: 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-dark);
  }

  @media (max-width: 768px) {
    .environment-step {
      max-width: 90%;
      padding: 0 15px;
    }

    /* legacy selectors removed */

    .progress-container {
      max-width: 100%;
    }
  }

  /* Spinner Styles */
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--spinner-border);
    border-top: 2px solid var(--text-primary);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    0% {
      transform: rotate(0deg);
    }
    100% {
      transform: rotate(360deg);
    }
  }

  /* Installation Status Styles */
  .installation-status {
    position: fixed;
    bottom: 20px;
    right: 20px;
    background: linear-gradient(135deg, var(--button-primary) 0%, var(--button-primary-hover) 100%);
    color: var(--text-primary);
    padding: 16px 20px;
    border-radius: 12px;
    box-shadow: 0 4px 20px var(--shadow-primary);
    z-index: 1000;
    animation: slideInUp 0.3s ease;
  }

  .status-content {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .status-content .spinner {
    border: 2px solid var(--spinner-border);
    border-top: 2px solid var(--text-primary);
  }

  .status-text {
    font-weight: 500;
    font-size: 14px;
  }

  @keyframes slideInUp {
    from {
      transform: translateY(100%);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  /* Setup Progress Styles (not used directly anymore) */

  /* Update and Remove Button Spinner Styles */
  .update-btn.updating,
  .remove-btn.removing {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: not-allowed;
    opacity: 0.8;
  }

  .update-btn.updating .spinner {
    border: 2px solid var(--spinner-border);
    border-top: 2px solid var(--text-primary);
  }

  .remove-btn.removing .spinner {
    border: 2px solid var(--spinner-border);
    border-top: 2px solid var(--text-primary);
  }

  /* Version management styles */
  .version-info {
    color: var(--text-dark);
    font-size: 14px;
    margin: 8px 0;
    font-weight: 500;
  }

  .info {
    color: var(--info-color);
    font-size: 14px;
    margin: 8px 0;
    font-style: italic;
  }

  .update-btn {
    background: linear-gradient(135deg, var(--warning-color) 0%, var(--warning-dark) 100%);
    color: var(--text-primary);
    border: none;
    padding: 10px 20px;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 600;
    transition: transform 150ms ease, box-shadow 150ms ease;
    will-change: transform;
  }

  .update-btn:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px var(--shadow-warning);
  }

  .update-btn:active {
    transform: translateY(0);
  }

  /* App updater styles */
  .release-notes {
    margin: 15px 0;
    border: 1px solid var(--card-border);
    border-radius: 8px;
    overflow: hidden;
  }

  .release-notes summary {
    padding: 12px 16px;
    background: var(--bg-tertiary);
    cursor: pointer;
    font-weight: 500;
    color: var(--text-dark);
    border-bottom: 1px solid var(--card-border);
  }

  .release-notes summary:hover {
    background: var(--bg-hover);
  }

  .release-notes-content {
    padding: 16px;
    background: var(--card-bg);
    color: var(--text-dark);
    line-height: 1.6;
    white-space: pre-wrap;
    max-height: 200px;
    overflow-y: auto;
  }

  .release-notes[open] summary {
    border-bottom: 1px solid var(--card-border);
  }

  /* Language selector styles */
  .language-selector {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .language-btn {
    padding: 12px 20px;
    border: 2px solid var(--input-border);
    border-radius: 8px;
    background: var(--input-bg);
    color: var(--text-primary);
    cursor: pointer;
    font-weight: 500;
    transition: transform 150ms ease, border-color 150ms ease, background 150ms ease;
    will-change: transform;
    font-size: 14px;
  }

  .language-btn:hover {
    border-color: var(--input-focus);
    background: var(--button-secondary);
    transform: translateY(-2px);
  }

  .language-btn.active {
    border-color: var(--input-focus);
    background: var(--button-primary);
    color: var(--text-primary);
    box-shadow: 0 4px 12px var(--shadow-primary);
  }

  .language-btn.active:hover {
    background: var(--button-primary-hover);
  }

  @media (max-width: 768px) {
    .language-selector {
      flex-direction: column;
    }

    .language-btn {
      width: 100%;
      text-align: center;
    }
  }

  /* Theme selector styles */
  .theme-selector {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .theme-btn {
    padding: 12px 20px;
    border: 2px solid var(--input-border);
    border-radius: 8px;
    background: var(--input-bg);
    color: var(--text-primary);
    cursor: pointer;
    font-weight: 500;
    transition: transform 150ms ease, border-color 150ms ease, background 150ms ease;
    will-change: transform;
    font-size: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .theme-btn:hover {
    border-color: var(--input-focus);
    background: var(--button-secondary);
    transform: translateY(-2px);
  }

  .theme-btn.active {
    border-color: var(--input-focus);
    background: var(--button-primary);
    color: var(--text-primary);
    box-shadow: 0 4px 12px var(--shadow-primary);
  }

  .theme-btn.active:hover {
    background: var(--button-primary-hover);
  }

  @media (max-width: 768px) {
    .theme-selector {
      flex-direction: column;
    }

    .theme-btn {
      width: 100%;
      justify-content: center;
    }
  }
</style>
