<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';

  // Installation flow state
  let currentStep = 'path-selection'; // 'path-selection', 'installing', 'main-interface'
  let installPath = '';
  let isPathSaved = false;
  let isInstalling = false;
  let installStatus = '';
  let installTimer = 0;
  let installTimerInterval: number | null = null;
  
  // Main interface state
  let cliInstalled = false;
  let cliOutput: string = '';
  let cliCommand = '';
  let environmentSetup = false;
  let installedRepos: Array<{name: string, hasLauncher: boolean}> = [];
  let availableRepos: string[] = [];
  let selectedRepo = '';
  let isSettingUpEnv = false;
  let isInstallingRepo = false;
  let activeTab = 'setup';

  onMount(async () => {
    try {
      installPath = await invoke('get_install_path');
      isPathSaved = true;
      // If path is already saved, check CLI installation
      await checkCliInstallation();
      if (cliInstalled) {
        currentStep = 'main-interface';
        await checkEnvironmentSetup();
        await loadInstalledRepos();
        await loadAvailableRepos();
      } else {
        // If path exists but CLI is not installed, show path selection page
        currentStep = 'path-selection';
      }
    } catch (error) {
      console.log('No install path found in registry');
      currentStep = 'path-selection';
      isPathSaved = false;
    }
  });

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
        isPathSaved = true;
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
    installStatus = 'Installation in progress, please wait a moment';
    
    // Start timer
    installTimerInterval = setInterval(() => {
      installTimer++;
      if (installTimer >= 15 && isInstalling) {
        installStatus = 'Please wait a little longer';
      }
    }, 1000);
    
    try {
      const result = await invoke('download_and_install_cli', { installPath }) as {success: boolean, message?: string};
      if (result.success) {
        await testCliInstallation();
        if (cliInstalled) {
          finishInstallation();
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
  
  function finishInstallation() {
    isInstalling = false;
    if (installTimerInterval) {
      clearInterval(installTimerInterval);
      installTimerInterval = null;
    }
    currentStep = 'main-interface';
    loadEnvironmentAndRepos();
  }
  
  async function loadEnvironmentAndRepos() {
    await checkEnvironmentSetup();
    await loadInstalledRepos();
    await loadAvailableRepos();
  }

  async function testCliInstallation() {
    try {
      const result = await invoke('test_cli_installation', { installPath }) as {success: boolean, message?: string};
      if (result.success) {
        cliInstalled = true;
      } else {
        cliInstalled = false;
        installStatus = `CLI testing error: ${result.message}`;
      }
    } catch (error) {
      installStatus = `Testing error: ${error}`;
      cliInstalled = false;
    }
  }

  async function checkCliInstallation() {
    if (installPath) {
      await testCliInstallation();
    }
  }

  async function runCliCommand() {
    if (!cliCommand.trim()) {
      cliOutput = 'Enter command to execute';
      return;
    }

    try {
      const args = cliCommand.trim().split(' ');
      const result = await invoke('run_cli_command', { installPath, args }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        cliOutput = result.stdout;
      } else {
        cliOutput = `Command failed (exit code: ${result.exit_code}):\nSTDOUT: ${result.stdout}\nSTDERR: ${result.stderr}`;
      }
    } catch (error) {
      cliOutput = `Command execution error: ${error}`;
    }
  }

  function showHelp() {
    cliCommand = '-h';
    runCliCommand();
  }

  // New functions for environment and repository management
  async function checkEnvironmentSetup() {
    try {
      const envInstalled = await invoke('check_environment_installed', { installPath }) as boolean;
      environmentSetup = envInstalled;
    } catch (error) {
      environmentSetup = false;
    }
  }

  async function setupEnvironment() {
    isSettingUpEnv = true;
    installStatus = 'Setting up environment...';
    
    try {
      const result = await invoke('run_cli_command', { installPath, args: ['--setup-env'] }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        cliOutput = result.stdout;
        await checkEnvironmentSetup();
        installStatus = environmentSetup ? 'Environment successfully set up!' : 'Environment setup error';
      } else {
        cliOutput = `Setup failed (exit code: ${result.exit_code}):\nSTDOUT: ${result.stdout}\nSTDERR: ${result.stderr}`;
        installStatus = 'Environment setup error';
      }
    } catch (error) {
      installStatus = `Environment setup error: ${error}`;
      cliOutput = String(error);
    } finally {
      isSettingUpEnv = false;
    }
  }

  async function loadInstalledRepos() {
    try {
      const result = await invoke('run_cli_command', { installPath, args: ['--list-repos'] }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        // Parse the output to extract repository names
        const lines = result.stdout.split('\n');
        installedRepos = [];
        for (const line of lines) {
          if (line.includes('✅') || line.includes('❌')) {
            const match = line.match(/- (\S+)/);
            if (match) {
              const repoName = match[1];
              const hasLauncher = line.includes('✅');
              installedRepos.push({ name: repoName, hasLauncher });
            }
          }
        }
      } else {
        console.log('Error loading installed repos:', result.stderr);
      }
    } catch (error) {
      console.log('Error loading installed repos:', error);
    }
  }

  async function loadAvailableRepos() {
    try {
      const response = await fetch('http://localhost:5173/api/repositories/top?limit=5');
      if (response.ok) {
        const repos = await response.json() as Array<{name: string}>;
        availableRepos = repos.map(repo => repo.name);
      } else {
        // Fallback to default repos if API is not available
        availableRepos = ['facefusion', 'comfyui', 'stable-diffusion-webui-forge', 'liveportrait', 'deep-live-cam'];
      }
    } catch (error) {
      console.error('Failed to load available repos from API:', error);
      // Fallback to default repos
      availableRepos = ['facefusion', 'comfyui', 'stable-diffusion-webui-forge', 'liveportrait', 'deep-live-cam'];
    }
  }

  async function installRepository() {
    if (!selectedRepo) {
      installStatus = 'Select repository to install';
      return;
    }

    // Check environment presence before installation
    try {
      const envInstalled = await invoke('check_environment_installed', { installPath }) as boolean;
      if (!envInstalled) {
        installStatus = 'Environment must be set up first. Go to "Environment" tab and click "Setup Environment".';
        return;
      }
    } catch (error) {
      installStatus = `Environment check error: ${error}`;
      return;
    }

    isInstallingRepo = true;
    installStatus = `Installing ${selectedRepo}...`;
    
    try {
      const result = await invoke('run_cli_command', { installPath, args: ['--install-repo', selectedRepo] }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        cliOutput = result.stdout;
        await loadInstalledRepos();
        installStatus = `Repository ${selectedRepo} successfully installed!`;
        selectedRepo = '';
      } else {
        cliOutput = `Installation failed (exit code: ${result.exit_code}):\nSTDOUT: ${result.stdout}\nSTDERR: ${result.stderr}`;
        installStatus = `Installation error ${selectedRepo}`;
      }
    } catch (error) {
      installStatus = `Installation error ${selectedRepo}: ${error}`;
      cliOutput = String(error);
    } finally {
      isInstallingRepo = false;
    }
  }

  async function updateRepository(repoName: string) {
    installStatus = `Updating ${repoName}...`;
    
    try {
      const result = await invoke('run_cli_command', { installPath, args: ['--update-repo', repoName] }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      
      if (result.success) {
        cliOutput = result.stdout;
        await loadInstalledRepos();
        installStatus = `Repository ${repoName} successfully updated!`;
      } else {
        cliOutput = `Update failed (exit code: ${result.exit_code}):\nSTDOUT: ${result.stdout}\nSTDERR: ${result.stderr}`;
        installStatus = `Update error ${repoName}`;
      }
    } catch (error) {
      installStatus = `Update error ${repoName}: ${error}`;
      cliOutput = String(error);
    }
  }

  async function launchRepository(repoName: string) {
    try {
      // Find the batch file and run it
      const result = await invoke('run_cli_command', { installPath, args: ['--system-info'] }) as {success: boolean, stdout: string, stderr: string, exit_code: number | null};
      installStatus = `Starting ${repoName}... (check console)`;
    } catch (error) {
      installStatus = `Start error ${repoName}: ${error}`;
    }
  }

  async function resetInstallation() {
    try {
      // Clear installation path from registry
      await invoke('clear_install_path');
      // Reset state
      installPath = '';
      cliInstalled = false;
      environmentSetup = false;
      installedRepos = [];
      availableRepos = [];
      installStatus = '';
      // Return to first step
      currentStep = 'path-selection';
    } catch (error) {
      installStatus = `Reset error: ${error}`;
    }
  }
</script>

<main>
  <div class="container">
    <!-- Step 1: Path Selection -->
    {#if currentStep === 'path-selection'}
      <div class="step-container">
        <h1>PortableSource Installer</h1>
        <h2 class="step-title">Select Installation Path</h2>
        <p class="step-description">
          Please select the folder where PortableSource will be installed.
          It is recommended to choose a folder on a drive with sufficient free space.
        </p>
        
        <div class="path-input-container">
          <input 
            type="text" 
            bind:value={installPath} 
            placeholder="Installation path not selected"
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
        </div>
      </div>
    {/if}
    
    <!-- Step 3: Main Interface -->
    {#if currentStep === 'main-interface'}
      <h1>PortableSource Manager</h1>
      
      <!-- Tab Navigation -->
      <div class="tabs">
        <button 
          class="tab" 
          class:active={activeTab === 'setup'}
          on:click={() => activeTab = 'setup'}
        >
          Setup
        </button>
        <button 
          class="tab" 
          class:active={activeTab === 'environment'}
          on:click={() => activeTab = 'environment'}
        >
          Environment
        </button>
        <button 
          class="tab" 
          class:active={activeTab === 'repositories'}
          on:click={() => activeTab = 'repositories'}
        >
          Repositories
        </button>
        <button 
          class="tab" 
          class:active={activeTab === 'cli'}
          on:click={() => activeTab = 'cli'}
        >
          CLI
        </button>
      </div>

      <!-- Setup Tab -->
      {#if activeTab === 'setup'}
        <div class="section">
          <h2>Installation Path</h2>
          <div class="path-selector">
            <input 
              type="text" 
              bind:value={installPath} 
              placeholder="Select installation folder"
              readonly
            />
            <button on:click={selectInstallPath} disabled={isInstalling}>
              Select Folder
            </button>
          </div>
          
          {#if installPath && !isPathSaved}
            <button class="save-btn" on:click={savePathAndStartInstallation} disabled={isInstalling}>
              Save Path
            </button>
          {/if}
          
          {#if isPathSaved}
            <p class="success">✓ Path saved in registry</p>
          {/if}
        </div>

        <div class="section">
          <h2>CLI Installation</h2>
          {#if cliInstalled}
            <p class="success">✓ CLI installed and working</p>
            <div class="action-buttons">
              <button on:click={testCliInstallation}>Check Again</button>
              <button class="reset-btn" on:click={resetInstallation}>Reinstall</button>
            </div>
          {/if}
        </div>
      {/if}

      <!-- Environment Tab -->
      {#if activeTab === 'environment'}
      <div class="section">
        <h2>Environment Management</h2>
        
        <div class="status-card">
          <h3>Environment Status</h3>
          {#if environmentSetup}
            <p class="success">✓ Base environment is set up and working</p>
          {:else}
            <p class="error">❌ Base environment is not set up</p>
          {/if}
        </div>

        {#if !environmentSetup}
          <button 
            class="install-btn" 
            on:click={setupEnvironment} 
            disabled={isSettingUpEnv}
          >
            {isSettingUpEnv ? 'Setting up environment...' : 'Setup Environment'}
          </button>
        {:else}
          <div class="action-buttons">
            <button on:click={checkEnvironmentSetup}>Check Environment</button>
            <button on:click={setupEnvironment} disabled={isSettingUpEnv}>
              {isSettingUpEnv ? 'Reinstalling...' : 'Reinstall Environment'}
            </button>
          </div>
        {/if}
      </div>
      {/if}

      <!-- Repositories Tab -->
      {#if activeTab === 'repositories'}
      <div class="section">
        <h2>Repository Installation</h2>
        
        <div class="repo-installer">
          <select bind:value={selectedRepo}>
            <option value="">Select repository</option>
            {#each availableRepos as repo}
              <option value={repo}>{repo}</option>
            {/each}
          </select>
          
          <button 
            class="install-btn" 
            on:click={installRepository} 
            disabled={isInstallingRepo || !selectedRepo}
          >
            {isInstallingRepo ? 'Installing...' : 'Install Repository'}
          </button>
        </div>
      </div>

      <div class="section">
        <h2>Installed Repositories</h2>
        
        {#if installedRepos.length === 0}
          <p class="no-repos">No repositories installed</p>
        {:else}
          <div class="repos-grid">
            {#each installedRepos as repo}
              <div class="repo-card">
                <h3>{repo.name}</h3>
                <div class="repo-status">
                  {#if repo.hasLauncher}
                    <span class="success">✓ Ready to launch</span>
                  {:else}
                    <span class="error">❌ No launcher</span>
                  {/if}
                </div>
                <div class="repo-actions">
                  {#if repo.hasLauncher}
                    <button class="launch-btn" on:click={() => launchRepository(repo.name)}>
                      Launch
                    </button>
                  {/if}
                  <button on:click={() => updateRepository(repo.name)}>Update</button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
      {/if}

      <!-- CLI Tab -->
      {#if activeTab === 'cli'}
      <div class="section cli-section">
        <h2>CLI Interface</h2>
        <div class="cli-controls">
          <input 
            type="text" 
            bind:value={cliCommand} 
            placeholder="Enter command (e.g.: --help, --list-repos, --install-repo repo_name)"
            on:keydown={(e) => e.key === 'Enter' && runCliCommand()}
          />
          <button on:click={runCliCommand}>Execute</button>
          <button on:click={showHelp}>Help</button>
        </div>
      </div>
      {/if}

      <!-- Status and Output -->
      {#if installStatus}
        <div class="status" class:error={installStatus.includes('Error')}>
          {installStatus}
        </div>
      {/if}

      {#if cliOutput && (activeTab === 'cli' || activeTab === 'environment')}
        <div class="cli-output">
          <h3>Command Output:</h3>
          <pre>{cliOutput}</pre>
        </div>
      {/if}
    {/if}
  </div>
</main>

<style>
  .container {
    max-width: 1000px;
    margin: 0 auto;
    padding: 20px;
    font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  }

  /* Path Selection Page Styles */
  .container:has(.step-container) {
    background: linear-gradient(135deg, #f1f3f4 0%, #e8eaed 50%, #dadce0 100%);
    min-height: 100vh;
    padding: 40px 20px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  h1 {
    text-align: center;
    color: #495057;
    margin-bottom: 40px;
    font-weight: 300;
    font-size: 36px;
    letter-spacing: -1px;
  }

  /* Special styling for path selection page title */
  .step-container h1 {
    color: #495057;
    font-size: 36px;
    margin-bottom: 10px;
    font-weight: 300;
    letter-spacing: -1px;
  }

  /* Tab Navigation */
  .tabs {
    display: flex;
    border-bottom: 2px solid #ddd;
    margin-bottom: 30px;
  }

  .tab {
    padding: 12px 24px;
    background: #f8f9fa;
    border: 1px solid #ddd;
    border-bottom: none;
    cursor: pointer;
    transition: all 0.3s ease;
    margin-right: 2px;
    font-size: 14px;
  }

  .tab:hover {
    background: #e9ecef;
  }

  .tab.active {
    background: #007acc;
    color: white;
    border-color: #007acc;
  }

  .section {
    margin-bottom: 30px;
    padding: 20px;
    border: 1px solid #ddd;
    border-radius: 8px;
    background: #f9f9f9;
  }

  .path-selector {
    display: flex;
    gap: 10px;
    margin-bottom: 15px;
  }

  input[type="text"] {
    flex: 1;
    padding: 10px;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-size: 14px;
  }

  select {
    flex: 1;
    padding: 10px;
    border: 1px solid #ccc;
    border-radius: 4px;
    background: white;
    font-size: 14px;
  }

  button {
    padding: 10px 20px;
    background: #007acc;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
    transition: background 0.3s;
  }

  button:hover:not(:disabled) {
    background: #005a9e;
  }

  button:disabled {
    background: #ccc;
    cursor: not-allowed;
  }

  .save-btn, .install-btn {
    background: #28a745;
    width: 100%;
    margin-top: 10px;
  }

  .save-btn:hover, .install-btn:hover {
    background: #218838;
  }

  .launch-btn {
    background: #17a2b8;
  }

  .launch-btn:hover:not(:disabled) {
    background: #138496;
  }

  .success {
    color: #28a745;
    font-weight: bold;
    margin: 10px 0;
  }

  .error {
    color: #dc3545;
    font-weight: bold;
  }

  .status {
    padding: 15px;
    border-radius: 4px;
    margin: 20px 0;
    background: #d4edda;
    border: 1px solid #c3e6cb;
    color: #155724;
  }

  .status.error {
    background: #f8d7da;
    border: 1px solid #f5c6cb;
    color: #721c24;
  }

  /* Environment Tab Styles */
  .status-card {
    background: white;
    padding: 20px;
    border-radius: 8px;
    border: 1px solid #ddd;
    margin-bottom: 20px;
  }

  .status-card h3 {
    margin-top: 0;
    color: #333;
  }

  .action-buttons {
    display: flex;
    gap: 10px;
    margin-top: 15px;
  }

  .reset-btn {
    background: #dc3545;
    color: white;
  }

  .reset-btn:hover:not(:disabled) {
    background: #c82333;
  }

  /* Repository Tab Styles */
  .repo-installer {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 20px;
  }

  .repos-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 20px;
  }

  .repo-card {
    background: white;
    padding: 20px;
    border-radius: 8px;
    border: 1px solid #ddd;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
    transition: transform 0.2s ease;
  }

  .repo-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 8px rgba(0,0,0,0.15);
  }

  .repo-card h3 {
    margin-top: 0;
    margin-bottom: 10px;
    color: #333;
  }

  .repo-status {
    margin-bottom: 15px;
  }

  .repo-actions {
    display: flex;
    gap: 10px;
  }

  .repo-actions button {
    flex: 1;
    padding: 8px 16px;
    font-size: 14px;
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
    font-weight: 300;
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
    box-shadow: 0 6px 20px rgba(173, 181, 189, 0.3);
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

  /* Installing Page Styles */
  .installing-container {
    text-align: center;
    padding: 60px 20px;
  }

  .installing-title {
    font-size: 32px;
    color: #333;
    margin-bottom: 20px;
    font-weight: 300;
  }

  .installing-message {
    font-size: 18px;
    color: #666;
    margin-bottom: 40px;
    line-height: 1.6;
  }

  .timer-display {
    font-size: 48px;
    color: #007acc;
    font-weight: bold;
    margin: 30px 0;
    font-family: 'Courier New', monospace;
  }

  .progress-indicator {
    width: 100%;
    max-width: 400px;
    margin: 0 auto 30px;
  }

  .progress-bar {
    width: 100%;
    height: 8px;
    background: #e9ecef;
    border-radius: 4px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #007acc, #28a745);
    border-radius: 4px;
    transition: width 0.3s ease;
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0% { opacity: 1; }
    50% { opacity: 0.7; }
    100% { opacity: 1; }
  }

  .installation-steps {
    text-align: left;
    max-width: 400px;
    margin: 0 auto;
    background: #f8f9fa;
    padding: 20px;
    border-radius: 8px;
    border: 1px solid #ddd;
  }

  .installation-steps h4 {
    margin-top: 0;
    color: #333;
    font-size: 16px;
  }

  .installation-steps ul {
    margin: 0;
    padding-left: 20px;
  }

  .installation-steps li {
    margin-bottom: 8px;
    color: #666;
    font-size: 14px;
  }

  .step-completed {
    color: #28a745 !important;
    text-decoration: line-through;
  }

  .no-repos {
    text-align: center;
    color: #666;
    font-style: italic;
    padding: 40px;
  }

  .cli-section {
    background: #f0f0f0;
    border-color: #007acc;
  }

  .cli-controls {
    display: flex;
    gap: 10px;
    margin-bottom: 20px;
  }

  .cli-output {
    background: #1e1e1e;
    color: #fff;
    padding: 15px;
    border-radius: 4px;
    margin-top: 15px;
  }

  .cli-output h3 {
    margin: 0 0 10px 0;
    color: #fff;
  }

  .cli-output pre {
    margin: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 13px;
    line-height: 1.4;
  }

  h2 {
    margin-top: 0;
    color: #333;
  }

  /* Responsive Design */
  @media (max-width: 768px) {
    .container {
      padding: 10px;
    }

    .tabs {
      flex-wrap: wrap;
    }

    .tab {
      flex: 1;
      text-align: center;
      min-width: 120px;
    }

    .repos-grid {
      grid-template-columns: 1fr;
    }

    .path-selector {
      flex-direction: column;
    }

    .cli-controls {
      flex-direction: column;
    }

    .action-buttons {
      flex-direction: column;
    }

    .repo-installer {
      flex-direction: column;
    }

    .repo-actions {
      flex-direction: column;
    }
  }
</style>