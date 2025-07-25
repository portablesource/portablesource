<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';

  // Installation flow state
  let currentStep = 'initial-check'; // 'initial-check', 'path-selection', 'installing', 'environment-setup', 'main-interface'
  let installPath = '';
  let isInstalling = false;
  let installStatus = '';
  let installTimer = 0;
  let installTimerInterval: number | null = null;
  let installProgress = 0;
  let maxInstallTime = 20; // Maximum expected install time in seconds
  
  // Main interface state
  let cliInstalled = false;
  let cliOutput: string = '';
  let cliCommand = '';

  let environmentStatus = {
    environment_exists: false,
    setup_completed: false,
    overall_status: 'Unknown'
  };
  let isCheckingEnvironment = false;
  let isSettingUpEnvironment = false;
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

  export const host = "portables.dev";

  onMount(async () => {
    await performInitialCheck();
  });

  async function performInitialCheck() {
    try {
      // Try to find CLI installation automatically
      installPath = await invoke('find_cli_installation');
      console.log('CLI found at:', installPath);
      cliInstalled = true;
      
      // Check environment status using CLI command
      await checkEnvironmentStatus();
      
      if (environmentStatus.setup_completed) {
        // Environment is ready, go to main interface
        currentStep = 'main-interface';
        await loadEnvironmentAndRepos();
      } else {
        // Environment needs setup
        currentStep = 'environment-setup';
      }
    } catch (error) {
      console.log('CLI not found, showing installation options:', error);
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
        title: 'Select folder for PortableSource installation'
      });
      
      if (selected) {
        installPath = selected;
      }
    } catch (error) {
      installStatus = `Folder selection error: ${error}`;
    }
  }

  async function savePathAndStartInstallation() {
    if (!installPath) {
      installStatus = 'Please select installation path';
      return;
    }

    try {
      const result = await invoke('set_install_path', { path: installPath }) as {success: boolean, message?: string};
      if (result.success) {
        currentStep = 'installing';
        startInstallationProcess();
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
    installStatus = 'Installation in progress, please wait a moment';
    
    // Start timer and progress
    installTimerInterval = setInterval(() => {
      installTimer++;
      // Update progress based on time (smooth progress bar)
      installProgress = Math.min((installTimer / maxInstallTime) * 100, 95); // Cap at 95% until completion
      
      if (installTimer >= 15 && isInstalling) {
        installStatus = 'Please wait a little longer';
        maxInstallTime = 30; // Extend expected time for slow connections
      }
    }, 1000);
    
    try {
      const result = await invoke('download_and_install_cli', { installPath }) as {success: boolean, message?: string};
      if (result.success) {
        await testCliInstallation();
        if (cliInstalled) {
          await finishInstallation();
        } else {
          installStatus = 'CLI installation check error';
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
    
    // After CLI installation, check environment status
    await checkEnvironmentStatus();
    
    if (environmentStatus.setup_completed) {
      // Environment is ready, go to main interface
      currentStep = 'main-interface';
      await loadEnvironmentAndRepos();
    } else {
      // Environment needs setup
      currentStep = 'environment-setup';
    }
  }
  
  async function loadEnvironmentAndRepos() {
    await checkEnvironmentSetup();
    await loadInstalledRepos();
    await loadAvailableRepos();
  }

  async function testCliInstallation(showError = true) {
    try {
      const result = await invoke('test_cli_installation', { installPath }) as {success: boolean, message?: string};
      if (result.success) {
        cliInstalled = true;
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

  async function checkCliInstallation() {
    if (installPath) {
      await testCliInstallation();
    }
  }

  async function resetInstallation() {
    try {
      // Clear installation path from registry
      await invoke('clear_install_path');
      // Reset state
      installPath = '';
      cliInstalled = false;
      installedRepos = [];
      availableRepos = [];
      installStatus = '';
      // Return to first step
      currentStep = 'path-selection';
    } catch (error) {
      installStatus = `Reset error: ${error}`;
    }
  }

  // Environment functions
  async function checkEnvironmentSetup() {
    try {
      const envStatus = await invoke('check_environment_status', { installPath }) as {
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
      const status = await invoke('check_environment_status', { installPath }) as {
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
        overall_status: 'Check failed'
      };
      return environmentStatus;
    } finally {
      isCheckingEnvironment = false;
    }
  }

  async function setupEnvironment() {
    try {
      isSettingUpEnvironment = true;
      installStatus = 'Setting up environment...';
      
      const result = await invoke('run_cli_command', { 
        installPath, 
        args: ['--setup-env'] 
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        installStatus = 'Environment setup completed!';
        const status = await checkEnvironmentStatus();
        
        if (status.setup_completed) {
          currentStep = 'main-interface';
          await loadEnvironmentAndRepos();
        }
      } else {
        installStatus = `Environment setup failed: ${result.stderr || 'Unknown error'}`;
      }
    } catch (error) {
      installStatus = `Environment setup error: ${error}`;
    } finally {
      isSettingUpEnvironment = false;
    }
  }

  async function loadInstalledRepos() {
    try {
      const installed: InstalledRepository[] = [];
      
      // Get folder lists from envs and repos directories
      const envsFolders = await invoke('list_directory_folders', { installPath, directoryName: 'envs' }) as string[];
      const reposFolders = await invoke('list_directory_folders', { installPath, directoryName: 'repos' }) as string[];
      
      // Find intersection - repositories that exist in both envs and repos
      const installedRepoNames = envsFolders.filter(envRepo => reposFolders.includes(envRepo));
      
      // Get additional information for each found repository
      for (const repoName of installedRepoNames) {
        try {
          // Get repository information from server
          const response = await fetch(`/api/search?q=${repoName}`);
          if (response.ok) {
            const data = await response.json();
            const repoInfo = data.repositories?.find((r: Repository) => r.name === repoName);
            
            installed.push({
              name: repoName,
              status: 'installed',
              hasLauncher: true
            });
          } else {
            // If unable to get server information, add basic info
            installed.push({
              name: repoName,
              status: 'installed',
              hasLauncher: true
            });
          }
        } catch (error) {
          // In case of server request error, add basic info
          installed.push({
            name: repoName,
            status: 'installed',
            hasLauncher: true
          });
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
        installStatus = 'CLI must be installed first. Redirecting to settings...';
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
      const envStatus = await invoke('check_environment_status', { installPath }) as {
        environment_exists: boolean,
        setup_completed: boolean,
        overall_status: string
      };
      
      if (!envStatus.setup_completed) {
        installStatus = 'Environment must be set up first. Go to settings and click "Setup Environment".';
        return;
      }

      installStatus = `Installing ${repoName}...`;
      
      const cliArgs = ['--install-repo', repoName];
      
      const result = await invoke('run_cli_command', { installPath, args: cliArgs }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        await loadInstalledRepos();
        
        // Show successful installation notification
        installedRepoName = repoName;
        showInstallNotification = true;
        installStatus = `${repoName} installed successfully!`;
        
        // Automatically hide notification after 3 seconds and navigate to installed repositories
        setTimeout(() => {
          showInstallNotification = false;
          setCurrentView('installed-repos');
        }, 3000);
      } else {
        installStatus = `Installation error ${repoName}: ${result.stderr || result.stdout || 'Unknown error'}`;
      }
    } catch (error) {
      console.error('Error during repository installation:', error);
      installStatus = `Installation error ${repoName}: ${error}`;
    } finally {
      isInstallingRepo = false;
      installingRepoName = '';
    }
  }

  // Check repository installation status by folder presence
  async function checkRepoInstallStatus(repoName: string): Promise<boolean> {
    try {
      // Get folder lists in envs and repos directories
      const envsFolders = await invoke('list_directory_folders', { installPath, directoryName: 'envs' }) as string[];
      
      const reposFolders = await invoke('list_directory_folders', { installPath, directoryName: 'repos' }) as string[];
      
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
      installStatus = `Starting ${repoName}...`;
      
      // Run batch file start_repo_name.bat from repository folder in new console window
      const batFile = `start_${repoName}.bat`;
      const workingDir = `${installPath}\\repos\\${repoName}`;
      
      console.log(`Trying to run batch file: ${batFile}`);
      console.log(`Working directory: ${workingDir}`);
      
      const result = await invoke('run_batch_in_new_window', {
        batchFile: batFile,
        workingDir: workingDir
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      console.log('Result:', result);
      
      if (result.success) {
        installStatus = `${repoName} started in new console window!`;
      } else {
        installStatus = `Start error ${repoName}: ${result.stderr || result.stdout || 'Unknown error'}`;
      }
    } catch (error) {
      console.error('Error in runRepo:', error);
      installStatus = `Start error ${repoName}: ${error}`;
    }
  }

  async function updateRepo(repoName: string) {
    try {
      isUpdatingRepo = true;
      updatingRepoName = repoName;
      installStatus = `Updating ${repoName}...`;
      
      // Use CLI command --update-repo
      const result = await invoke('run_cli_command', {
        installPath,
        args: ['--update-repo', repoName]
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        installStatus = `${repoName} updated!`;
      } else {
        installStatus = `Update error ${repoName}: ${result.stderr || 'Unknown error'}`;
      }
    } catch (error) {
      installStatus = `Update error ${repoName}: ${error}`;
    } finally {
      isUpdatingRepo = false;
      updatingRepoName = '';
    }
  }

  async function removeRepo(repoName: string) {
    try {
      isRemovingRepo = true;
      removingRepoName = repoName;
      installStatus = `Removing ${repoName}...`;
      
      // Remove repository folders from envs and repos
      const envsPath = `${installPath}\\envs\\${repoName}`;
      const reposPath = `${installPath}\\repos\\${repoName}`;
      
      // Remove folder from envs
      const envResult = await invoke('run_command', {
        command: `Remove-Item -Path "${envsPath}" -Recurse -Force -ErrorAction SilentlyContinue`,
        working_dir: installPath
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      // Remove folder from repos
      const repoResult = await invoke('run_command', {
        command: `Remove-Item -Path "${reposPath}" -Recurse -Force -ErrorAction SilentlyContinue`,
        working_dir: installPath
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      // Update installed repositories list
      await loadInstalledRepos();
      
      if (envResult.success && repoResult.success) {
        installStatus = `${repoName} removed!`;
      } else {
        installStatus = `Partial removal of ${repoName} - check manually`;
      }
    } catch (error) {
      installStatus = `Removal error ${repoName}: ${error}`;
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
      installStatus = 'Removing all repositories...';
      
      // Remove all folders from envs
      const envResult = await invoke('run_command', {
        command: `Remove-Item -Path "${installPath}\\envs\\*" -Recurse -Force -ErrorAction SilentlyContinue`,
        working_dir: installPath
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      // Remove all folders from repos
      const repoResult = await invoke('run_command', {
        command: `Remove-Item -Path "${installPath}\\repos\\*" -Recurse -Force -ErrorAction SilentlyContinue`,
        working_dir: installPath
      }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      // Update installed repositories list
      await loadInstalledRepos();
      
      if (envResult.success && repoResult.success) {
        installStatus = 'All repositories successfully removed!';
      } else {
        installStatus = 'Partial removal completed - check manually';
      }
    } catch (error) {
      installStatus = `Repository removal error: ${error}`;
    }
  }

  async function removeSelectedRepo() {
    try {
      if (installedRepos.length === 0) {
        installStatus = 'No installed repositories to remove';
        return;
      }
      
      // Show repository list for selection
      const repoNames = installedRepos.map((repo, index) => `${index + 1}. ${repo.name}`);
      const repoList = repoNames.join('\n');
      
      const userChoice = prompt(`Select repository to remove:\n\n${repoList}\n\nEnter repository number (1-${installedRepos.length}):`);
      
      if (!userChoice) {
        installStatus = 'Removal cancelled';
        return;
      }
      
      const selectedIndex = parseInt(userChoice) - 1;
      
      if (selectedIndex >= 0 && selectedIndex < installedRepos.length) {
        const repoToRemove = installedRepos[selectedIndex].name;
        
        // Confirm deletion
        const confirmDelete = confirm(`Are you sure you want to remove repository "${repoToRemove}"?\n\nThis action cannot be undone.`);
        
        if (confirmDelete) {
          await removeRepo(repoToRemove);
        } else {
          installStatus = 'Removal cancelled';
        }
      } else {
        installStatus = 'Invalid repository number';
      }
    } catch (error) {
      installStatus = `Repository removal error: ${error}`;
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
      <h3>PortableSource</h3>
      
      <!-- Navigation -->
      <div class="nav-section">
        <button 
          class="nav-item" 
          class:active={currentView === 'top-repos'}
          on:click={() => { setCurrentView('top-repos'); sidebarOpen = false; }}
        >
          🔥 Top Repositories
        </button>
        <button 
          class="nav-item" 
          class:active={currentView === 'installed-repos'}
          on:click={() => { setCurrentView('installed-repos'); sidebarOpen = false; }}
        >
          📦 Installed
        </button>
      </div>
      
      <!-- Settings at bottom -->
      <div class="settings-section">
        <button 
          class="nav-item settings-btn" 
          class:active={currentView === 'settings'}
          on:click={() => { setCurrentView('settings'); sidebarOpen = false; }}
        >
          ⚙️ Settings
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
        <h1 class="step-title">PortableSource</h1>
        <p class="step-description">Checking your installation...</p>
        
        <div class="installation-progress">
          <div class="spinner"></div>
          <p class="installation-text">Performing initial checks...</p>
        </div>
      </div>
    {/if}
    
    <!-- Step 1: Path Selection -->
    {#if currentStep === 'path-selection'}
      <div class="step-container">
        <h1 class="step-title">PortableSource Installer</h1>
        <p class="step-description">Select folder for PortableSource installation</p>
        
        <div class="path-input-container">
          <input 
            type="text" 
            bind:value={installPath} 
            placeholder="Select installation folder"
            readonly
          />
          <button on:click={selectInstallPath}>
            Select Folder
          </button>
        </div>
        
        {#if installPath}
          <button 
            class="confirm-button" 
            on:click={savePathAndStartInstallation}
            disabled={!installPath}
          >
            Confirm and Start Installation
          </button>
        {/if}
        
        {#if installStatus}
          <p class="status">{installStatus}</p>
        {/if}
      </div>
    {/if}
    
    <!-- Step 2: Installation Progress -->
    {#if currentStep === 'installing'}
      <div class="step-container installation-step">
        <h2>Installing PortableSource CLI</h2>
        <div class="installation-progress">
          <div class="spinner"></div>
          <p class="installation-text">{installStatus}</p>
          <p class="timer">Time elapsed: {installTimer} sec</p>
          
          <!-- Progress Bar -->
          <div class="progress-container">
            <div class="progress-bar">
              <div class="progress-fill" style="width: {installProgress}%"></div>
            </div>
            <div class="progress-text">{Math.round(installProgress)}%</div>
          </div>
        </div>
      </div>
    {/if}
    
    <!-- Step 3: Environment Setup -->
    {#if currentStep === 'environment-setup'}
      <div class="step-container environment-step">
        <h1 class="step-title">Environment Setup Required</h1>
        <p class="step-description">PortableSource CLI is installed, but the environment needs to be set up.</p>
        
        <div class="environment-status">
          <h3>Current Status:</h3>
          <div class="status-item">
            <span class="status-label">Environment exists:</span>
            <span class="status-value {environmentStatus.environment_exists ? 'success' : 'error'}">
              {environmentStatus.environment_exists ? '✅' : '❌'}
            </span>
          </div>
          <div class="status-item">
            <span class="status-label">Setup completed:</span>
            <span class="status-value {environmentStatus.setup_completed ? 'success' : 'error'}">
              {environmentStatus.setup_completed ? 'YES' : 'NO'}
            </span>
          </div>
          <div class="status-item">
            <span class="status-label">Overall status:</span>
            <span class="status-value">{environmentStatus.overall_status}</span>
          </div>
        </div>
        
        <div class="environment-actions">
          {#if isSettingUpEnvironment}
            <p class="status">{installStatus}</p>
          {:else if isCheckingEnvironment}
            <p class="status">Checking environment status...</p>
          {:else}
            <button 
              class="setup-env-button" 
              on:click={setupEnvironment}
              disabled={environmentStatus.setup_completed}
            >
              {environmentStatus.setup_completed ? '✓ Environment Ready' : 'Setup Environment'}
            </button>
            
            <button 
              class="check-env-button" 
              on:click={checkEnvironmentStatus}
            >
              Check Status Again
            </button>
            
            {#if environmentStatus.setup_completed}
              <button 
                class="continue-button" 
                on:click={() => { currentStep = 'main-interface'; loadEnvironmentAndRepos(); }}
              >
                Continue to Repositories
              </button>
            {/if}
          {/if}
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
            <span class="notification-text">Repository "{installedRepoName}" successfully installed!</span>
            <button class="notification-close" on:click={() => showInstallNotification = false}>×</button>
          </div>
        </div>
      {/if}
      
      <div class="content-header">
        <h1>
          {#if currentView === 'top-repos'}
            🔥 Top Repositories
          {:else if currentView === 'installed-repos'}
            📦 Installed Repositories
          {:else if currentView === 'settings'}
            ⚙️ Settings
          {/if}
        </h1>
      </div>

      <!-- Top Repositories View -->
      {#if currentView === 'top-repos'}
        <div class="repos-grid">
           {#each availableRepos as repo}
             <div class="repo-card">
               <h3>{repo.name}</h3>
               <p>{repo.description}</p>
               {#if repo.downloadCount}
                 <div class="repo-stats">
                   <span class="download-count">📥 {repo.downloadCount} downloads</span>
                   {#if repo.uploadedByUsername}
                     <span class="author">👤 {repo.uploadedByUsername}</span>
                   {/if}
                 </div>
               {/if}
               {#await checkRepoInstallStatus(repo.name)}
                 <button class="install-repo-btn" disabled>Checking...</button>
               {:then isInstalled}
                 {#if isInstalled}
                   <button class="open-repo-btn" on:click={() => setCurrentView('installed-repos')}>
                     Open
                   </button>
                 {:else if isInstallingRepo && installingRepoName === repo.name}
                   <button class="install-repo-btn installing" disabled>
                     <span class="spinner"></span>
                     Installing...
                   </button>
                 {:else}
                   <button class="install-repo-btn" on:click={() => {
                     installRepo(repo.name);
                   }} disabled={isInstallingRepo}>Install</button>
                 {/if}
               {:catch}
                 {#if isInstallingRepo && installingRepoName === repo.name}
                   <button class="install-repo-btn installing" disabled>
                     <span class="spinner"></span>
                     Installing...
                   </button>
                 {:else}
                   <button class="install-repo-btn" on:click={() => {
                     installRepo(repo.name);
                   }} disabled={isInstallingRepo}>Install</button>
                 {/if}
               {/await}
             </div>
           {/each}
         </div>
         
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
              <p>No installed repositories yet</p>
              <button on:click={() => setCurrentView('top-repos')}>View top repositories</button>
            </div>
          {:else}
            <div class="repos-list">
              {#each installedRepos as repo}
                <div class="installed-repo-item">
                  <h3>{repo.name}</h3>
                  <div class="repo-actions">
                    {#if repo.hasLauncher}
                      <button class="launch-btn" on:click={() => runRepo(repo.name)}>Launch</button>
                    {/if}
                    {#if isUpdatingRepo && updatingRepoName === repo.name}
                      <button class="update-btn updating" disabled>
                        <span class="spinner"></span>
                        Updating...
                      </button>
                    {:else}
                      <button class="update-btn" on:click={() => updateRepo(repo.name)} disabled={isUpdatingRepo || isRemovingRepo}>Update</button>
                    {/if}
                    {#if isRemovingRepo && removingRepoName === repo.name}
                      <button class="remove-btn removing" disabled>
                        <span class="spinner"></span>
                        Removing...
                      </button>
                    {:else}
                      <button class="remove-btn" on:click={() => removeRepo(repo.name)} disabled={isUpdatingRepo || isRemovingRepo}>Remove</button>
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
            <h2>Installation Path</h2>
            <div class="path-selector">
              <input 
                type="text" 
                bind:value={installPath} 
                placeholder="Select installation folder"
                readonly
              />
              <button on:click={selectInstallPath} disabled={isInstalling}>
                Change
              </button>
            </div>
          </div>

          <div class="settings-section">
            <h2>CLI Status</h2>
            {#if cliInstalled}
              <p class="success">✓ CLI installed and working</p>
              <div class="action-buttons">
                <button on:click={() => testCliInstallation(true)}>Check again</button>
                <button class="reset-btn" on:click={resetInstallation}>Reinstall</button>
              </div>
            {:else}
              <p class="warning">CLI not installed</p>
              <button class="install-cli-btn" on:click={savePathAndStartInstallation}>Install CLI</button>
            {/if}
          </div>
           <div class="settings-section">
              <h2>Repository Management</h2>
              <div class="action-buttons">
                <button class="reset-btn" on:click={removeAllRepos}>Remove All</button>
                <button class="reset-btn" on:click={removeSelectedRepo}>Remove Selected</button>
              </div>
            </div>
        </div>
      {/if}
    {/if}
  </div>
</main>

<style>
  /* Global Styles */
  :global(body) {
    margin: 0;
    padding: 0;
    background: linear-gradient(135deg, #f5f7fa 0%, #c3cfe2 100%);
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
    background: rgba(255, 255, 255, 0.9);
    border: none;
    border-radius: 8px;
    padding: 12px;
    cursor: pointer;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
    transition: all 0.3s ease;
  }

  .hamburger-btn:hover {
    background: white;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.15);
  }

  .hamburger-line {
    width: 20px;
    height: 2px;
    background: #495057;
    margin: 4px 0;
    transition: 0.3s;
  }

  /* Sidebar */
  .sidebar {
    position: fixed;
    top: 0;
    left: -300px;
    width: 300px;
    height: 100vh;
    background: linear-gradient(180deg, #ffffff 0%, #f8f9fa 100%);
    box-shadow: 2px 0 15px rgba(0, 0, 0, 0.1);
    transition: left 0.3s ease;
    z-index: 1000;
    border-right: 1px solid #dee2e6;
  }

  .sidebar.open {
    left: 0;
  }

  .sidebar-content {
    padding: 80px 20px 20px;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .sidebar-content h3 {
    color: #495057;
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
    color: #6c757d;
    cursor: pointer;
    transition: all 0.3s ease;
  }

  .nav-item:hover {
    background: #e9ecef;
    color: #495057;
  }

  .nav-item.active {
    background: linear-gradient(135deg, #007acc 0%, #005a9e 100%);
    color: white;
    box-shadow: 0 2px 8px rgba(0, 122, 204, 0.3);
  }

  .settings-section {
    border-top: 1px solid #dee2e6;
    padding-top: 20px;
  }

  .settings-btn {
    background: #f8f9fa !important;
    border: 1px solid #dee2e6 !important;
  }

  .settings-btn:hover {
    background: #e9ecef !important;
  }

  .settings-btn.active {
    background: linear-gradient(135deg, #6c757d 0%, #495057 100%) !important;
    border-color: #495057 !important;
  }

  /* Overlay */
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.5);
    z-index: 999;
  }

  /* Main Content */
  .main-content {
    padding: 40px 20px;
    min-height: 100vh;
    transition: margin-left 0.3s ease;
  }

  .main-content.shifted {
    margin-left: 300px;
  }

  .content-header {
    text-align: center;
    margin-bottom: 40px;
  }

  .content-header h1 {
    color: #495057;
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
    background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);
    border-radius: 20px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.1);
    border: 1px solid #dee2e6;
  }

  .step-title {
    font-size: 32px;
    color: #495057;
    margin-bottom: 15px;
    font-weight: 300;
    letter-spacing: -0.5px;
  }

  .step-description {
    font-size: 18px;
    color: #6c757d;
    margin-bottom: 40px;
    line-height: 1.6;
  }

  .path-input-container {
    display: flex;
    gap: 20px;
    margin-bottom: 40px;
    align-items: stretch;
    background: #ffffff;
    padding: 8px;
    border-radius: 16px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
    border: 2px solid #e9ecef;
  }

  .path-input-container input {
    flex: 1;
    padding: 20px 24px;
    font-size: 16px;
    border: none;
    border-radius: 12px;
    background: transparent;
    color: #495057;
    outline: none;
    font-weight: 400;
  }

  .path-input-container input::placeholder {
    color: #adb5bd;
    font-weight: 300;
  }

  .path-input-container button {
    padding: 20px 32px;
    font-size: 16px;
    background: linear-gradient(135deg, #adb5bd 0%, #868e96 100%);
    color: white;
    border: none;
    border-radius: 12px;
    font-weight: 500;
    transition: all 0.3s ease;
    box-shadow: 0 2px 8px rgba(173, 181, 189, 0.3);
  }

  .path-input-container button:hover:not(:disabled) {
    background: linear-gradient(135deg, #868e96 0%, #6c757d 100%);
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(173, 181, 189, 0.4);
  }

  .confirm-button {
    background: linear-gradient(135deg, #adb5bd 0%, #868e96 100%);
    color: white;
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
    background: linear-gradient(135deg, #868e96 0%, #6c757d 100%);
    transform: translateY(-3px);
    box-shadow: 0 8px 25px rgba(173, 181, 189, 0.4);
  }

  .confirm-button:disabled {
    background: #dee2e6;
    color: #adb5bd;
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
  }

  .status {
    margin-top: 20px;
    padding: 15px;
    border-radius: 8px;
    background: #d4edda;
    color: #155724;
    border: 1px solid #c3e6cb;
  }

  /* Repositories Grid */
  .repos-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 20px;
    max-width: 1200px;
    margin: 0 auto;
  }

  .repo-card {
    background: white;
    padding: 25px;
    border-radius: 16px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
    border: 1px solid #e9ecef;
    transition: all 0.3s ease;
  }

  .repo-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(0, 0, 0, 0.12);
  }

  .repo-card h3 {
    color: #495057;
    font-size: 1.3rem;
    margin-bottom: 10px;
    font-weight: 600;
  }

  .repo-card p {
    color: #6c757d;
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
    color: #28a745;
    font-weight: 500;
  }

  .author {
    color: #6c757d;
  }

  .install-repo-btn {
    background: linear-gradient(135deg, #007acc 0%, #005a9e 100%);
    color: white;
    border: none;
    padding: 12px 24px;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 500;
    transition: all 0.3s ease;
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
    background: linear-gradient(135deg, #6c757d 0%, #495057 100%);
  }



  .open-repo-btn {
    background: linear-gradient(135deg, #17a2b8 0%, #138496 100%);
    color: white;
    border: none;
    padding: 12px 24px;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 500;
    transition: all 0.3s ease;
    box-shadow: 0 2px 8px rgba(23, 162, 184, 0.3);
  }

  .open-repo-btn:hover {
    background: linear-gradient(135deg, #138496 0%, #117a8b 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(23, 162, 184, 0.4);
  }

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
    background: white;
    border-radius: 16px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
  }

  .empty-state p {
    color: #6c757d;
    font-size: 1.1rem;
    margin-bottom: 20px;
  }

  .empty-state button {
    background: linear-gradient(135deg, #007acc 0%, #005a9e 100%);
    color: white;
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
    background: white;
    padding: 20px;
    border-radius: 12px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
    border: 1px solid #e9ecef;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .installed-repo-item h3 {
    color: #495057;
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
  }

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
    transition: all 0.3s ease;
  }

  .launch-btn {
    background: linear-gradient(135deg, #28a745 0%, #20c997 100%);
    color: white;
  }

  .update-btn {
    background: linear-gradient(135deg, #007acc 0%, #005a9e 100%);
    color: white;
  }

  .remove-btn {
    background: linear-gradient(135deg, #dc3545 0%, #c82333 100%);
    color: white;
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
    background: white;
    padding: 30px;
    border-radius: 12px;
    margin-bottom: 20px;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
    border: 1px solid #e9ecef;
  }

  .settings-section h2 {
    color: #495057;
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
    border: 1px solid #dee2e6;
    border-radius: 8px;
    font-size: 14px;
  }

  .path-selector button {
    padding: 12px 24px;
    background: #007acc;
    color: white;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.3s;
  }

  .path-selector button:hover:not(:disabled) {
    background: #005a9e;
  }

  .action-buttons {
    display: flex;
    gap: 10px;
    margin-top: 15px;
  }

  .action-buttons button {
    padding: 10px 20px;
    background: #007acc;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.3s;
  }

  .reset-btn {
    background: #dc3545 !important;
  }

  .reset-btn:hover:not(:disabled) {
    background: #c82333 !important;
  }

  .install-cli-btn {
    background: linear-gradient(135deg, #28a745 0%, #20c997 100%);
    color: white;
    border: none;
    padding: 12px 24px;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 500;
    transition: all 0.3s ease;
  }

  .install-cli-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(40, 167, 69, 0.3);
  }

  .success {
    color: #28a745;
    font-weight: bold;
    margin: 10px 0;
  }

  .warning {
    color: #ffc107;
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
    background: #007acc;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.3s;
    font-size: 14px;
    font-weight: 500;
  }

  .action-buttons button:hover:not(:disabled) {
    background: #005a9e;
    transform: translateY(-1px);
  }

  .reset-btn {
    background: #dc3545 !important;
  }

  .reset-btn:hover:not(:disabled) {
    background: #c82333 !important;
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
    background: linear-gradient(135deg, #28a745 0%, #20c997 100%);
    color: white;
    padding: 16px 20px;
    border-radius: 12px;
    box-shadow: 0 8px 25px rgba(40, 167, 69, 0.3);
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
    color: white;
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
    background: rgba(255, 255, 255, 0.2);
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

  /* Environment Setup Page Styles */
  .environment-step {
    max-width: 600px;
    margin: 0 auto;
    text-align: center;
  }

  .environment-status {
    background: white;
    padding: 30px;
    border-radius: 12px;
    margin: 30px 0;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.08);
    border: 1px solid #e9ecef;
  }

  .environment-status h3 {
    color: #495057;
    font-size: 1.2rem;
    margin-bottom: 20px;
    font-weight: 600;
  }

  .status-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 0;
    border-bottom: 1px solid #f8f9fa;
  }

  .status-item:last-child {
    border-bottom: none;
  }

  .status-label {
    font-weight: 500;
    color: #495057;
  }

  .status-value {
    font-weight: 600;
  }

  .status-value.success {
    color: #28a745;
  }

  .status-value.error {
    color: #dc3545;
  }

  .environment-actions {
    display: flex;
    flex-direction: column;
    gap: 15px;
    align-items: center;
  }

  .setup-env-button, .check-env-button, .continue-button {
    padding: 15px 30px;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    font-size: 16px;
    font-weight: 600;
    transition: all 0.3s ease;
    min-width: 200px;
  }

  .setup-env-button {
    background: linear-gradient(135deg, #007acc 0%, #005a9e 100%);
    color: white;
  }

  .setup-env-button:disabled {
    background: linear-gradient(135deg, #28a745 0%, #20c997 100%);
    cursor: default;
  }

  .check-env-button {
    background: linear-gradient(135deg, #6c757d 0%, #495057 100%);
    color: white;
  }

  .continue-button {
    background: linear-gradient(135deg, #28a745 0%, #20c997 100%);
    color: white;
  }

  .setup-env-button:hover:not(:disabled),
  .check-env-button:hover,
  .continue-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
  }

  /* Progress Bar Styles */
  .progress-container {
    margin-top: 20px;
    width: 100%;
    max-width: 400px;
    margin-left: auto;
    margin-right: auto;
  }

  .progress-bar {
    width: 100%;
    height: 12px;
    background: #e9ecef;
    border-radius: 6px;
    overflow: hidden;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.1);
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #007acc 0%, #20c997 50%, #28a745 100%);
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
      rgba(255, 255, 255, 0.3) 50%,
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
    color: #495057;
  }

  @media (max-width: 768px) {
    .environment-step {
      max-width: 90%;
      padding: 0 15px;
    }

    .environment-status {
      padding: 20px;
      margin: 20px 0;
    }

    .setup-env-button, .check-env-button, .continue-button {
      min-width: 100%;
      padding: 12px 20px;
      font-size: 14px;
    }

    .progress-container {
      max-width: 100%;
    }
  }

  /* Spinner Styles */
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top: 2px solid white;
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
    background: linear-gradient(135deg, #007acc 0%, #005a9e 100%);
    color: white;
    padding: 16px 20px;
    border-radius: 12px;
    box-shadow: 0 4px 20px rgba(0, 123, 204, 0.3);
    z-index: 1000;
    animation: slideInUp 0.3s ease;
  }

  .status-content {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .status-content .spinner {
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top: 2px solid white;
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
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top: 2px solid white;
  }

  .remove-btn.removing .spinner {
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top: 2px solid white;
  }
</style>
