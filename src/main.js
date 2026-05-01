document.addEventListener('DOMContentLoaded', async () => {
  if (!window.__TAURI__) {
    console.error("Tauri API not found.");
    return;
  }

  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const { getCurrentWindow } = window.__TAURI__.window;

  // Views & Navigation
  const views = document.querySelectorAll('.view');
  const navItems = document.querySelectorAll('.nav-item');
  
  // Elements - Dashboard
  const toggleBtn = document.getElementById('toggleBtn');
  const stopBtn = document.getElementById('stopBtn');
  const statusIndicator = document.getElementById('statusIndicator');
  const statusTextBig = document.getElementById('statusTextBig');
  const browserLink = document.getElementById('browserLink');
  
  // Elements - Settings
  const portInput = document.getElementById('portInput');
  const saveSettingsBtn = document.getElementById('saveSettingsBtn');
  const resetSettingsBtn = document.getElementById('resetSettingsBtn');
  const scriptPathInput = document.getElementById('scriptPathInput');
  
  // Elements - Logs
  const logContent = document.getElementById('logContent');

  // Window Controls

  let isRunning = false;
  let isManualStopping = false;
  let portMismatchDetected = false;
  const LOG_LIMIT = 500;
  let uptimeInterval = null;
  let serverStartTime = null;
  let lcdCycle = 0; // 0: Status, 1: Uptime, 2: Port
  let retryCount = 0;

  // Initialization
  const savedPort = localStorage.getItem('ken_port') || '3000';
  const savedAutoRestart = (localStorage.getItem('ken_auto_restart') ?? 'true') === 'true';
  const savedNotifyStatus = (localStorage.getItem('ken_notify_status') ?? 'true') === 'true';
  const savedNotifyCrash = (localStorage.getItem('ken_notify_crash') ?? 'true') === 'true';
  const savedBackoff = (localStorage.getItem('ken_backoff') ?? 'true') === 'true';
  const savedConflictSolver = (localStorage.getItem('ken_conflict_solver') ?? 'true') === 'true';
  const savedForcePort = (localStorage.getItem('ken_force_port') ?? 'false') === 'true';

  const savedProjectPath = localStorage.getItem('ken_project_path') || '';
  const savedScriptPath = localStorage.getItem('ken_script_path') || 'index.js';
  const savedInitialDelay = localStorage.getItem('ken_initial_delay') || '2';
  const savedMaxDelay = localStorage.getItem('ken_max_delay') || '60';
  const savedMaxRetries = localStorage.getItem('ken_max_retries') || '5';
  
  portInput.value = savedPort;
  document.getElementById('autoRestart').checked = savedAutoRestart;
  document.getElementById('notifyStatus').checked = savedNotifyStatus;
  document.getElementById('notifyCrash').checked = savedNotifyCrash;
  document.getElementById('backoffStrategy').checked = savedBackoff;
  document.getElementById('portConflictSolver').checked = savedConflictSolver;
  document.getElementById('forcePort').checked = savedForcePort;

  if (scriptPathInput) scriptPathInput.value = savedScriptPath;
  if (document.getElementById('initialDelay')) {
    document.getElementById('initialDelay').value = savedInitialDelay;
    document.getElementById('initialDelayVal').textContent = savedInitialDelay;
    document.getElementById('maxDelay').value = savedMaxDelay;
    document.getElementById('maxDelayVal').textContent = savedMaxDelay;
    document.getElementById('maxRetries').value = savedMaxRetries;
  }
  
  browserLink.textContent = `http://localhost:${savedPort}`;

  // Range Slider Sync
  const initialDelay = document.getElementById('initialDelay');
  if (initialDelay) {
    initialDelay.addEventListener('input', (e) => {
      document.getElementById('initialDelayVal').textContent = e.target.value;
    });
    document.getElementById('maxDelay').addEventListener('input', (e) => {
      document.getElementById('maxDelayVal').textContent = e.target.value;
    });
  }

  // Collapsible Logic
  const collHeader = document.querySelector('.collapsible-header');
  if (collHeader) {
    collHeader.addEventListener('click', () => {
      collHeader.parentElement.classList.toggle('open');
    });
  }

  // Get Project Path
  async function initProjectPath() {
    try {
      const projectPath = await invoke('get_project_path');
      const display = document.getElementById('projectNameDisplay');
      const prompt = document.getElementById('projectPrompt');
      const openFolderBtn = document.getElementById('openFolderBtn');
      const btnSpan = openFolderBtn.querySelector('span');
      
      if (projectPath) {
        display.textContent = projectPath;
        display.classList.remove('empty');
        prompt.style.display = 'none';
        prompt.classList.remove('danger');
        btnSpan.textContent = 'Switch';
        toggleBtn.classList.remove('dimmed');
      } else {
        display.textContent = "No Folder Selected";
        display.classList.add('empty');
        prompt.style.display = 'flex';
        btnSpan.textContent = 'Select';
        toggleBtn.classList.add('dimmed');
      }
      
      // Update tray menu with the new project name
      const cleanName = projectPath ? projectPath : "";
      invoke('update_tray_menu', { projectName: cleanName, isRunning: isRunning }).catch(err => console.error("Tray Update Failed:", err));

    } catch (err) {
      console.error("Failed to get project path:", err);
    }
  }

  // Load initial path from localStorage if exists
  if (savedProjectPath) {
    invoke('set_project_path', { path: savedProjectPath }).then(() => {
      initProjectPath();
    });
  } else {
    initProjectPath();
  }

  // Get Bun Version
  async function initBunVersion() {
    const bunDisplay = document.getElementById('bunVersionDisplay');
    const startBtn = document.getElementById('toggleBtn');
    const dashboardContent = document.getElementById('dashboardContent');
    const bunMissingView = document.getElementById('bunMissingView');
    
    const cachedVersion = localStorage.getItem('ken_bun_version');
    if (cachedVersion) {
      bunDisplay.textContent = "v" + cachedVersion;
    } else {
      bunDisplay.textContent = "Checking...";
    }

    try {
      const bunVersion = await invoke('get_bun_version');
      if (bunVersion && bunVersion !== "Not Found") {
        bunDisplay.textContent = "v" + bunVersion;
        bunDisplay.classList.remove('error');
        
        if (dashboardContent) dashboardContent.classList.remove('hidden');
        if (bunMissingView) bunMissingView.classList.add('hidden');

        if (startBtn) {
          startBtn.disabled = false;
          startBtn.classList.remove('dimmed');
        }
        localStorage.setItem('ken_bun_version', bunVersion);
      } else {
        throw new Error("NOT_INSTALLED");
      }
    } catch (err) {
      bunDisplay.textContent = "MISSING";
      bunDisplay.classList.add('error');
      
      if (dashboardContent) dashboardContent.classList.add('hidden');
      if (bunMissingView) bunMissingView.classList.remove('hidden');

      if (startBtn) {
        startBtn.disabled = true;
        startBtn.classList.add('dimmed');
        startBtn.title = "Please install Bun to use KenBun";
      }
      localStorage.removeItem('ken_bun_version');
      addLog("SYSTEM: Bun runtime not found. Vital functions disabled.", true);
    }
  }

  initBunVersion();

  /**
   * Navigation Logic
   */
  navItems.forEach(item => {
    item.addEventListener('click', () => {
      const targetView = item.getAttribute('data-view');
      
      // Update Sidebar
      navItems.forEach(nav => nav.classList.remove('active'));
      item.classList.add('active');
      
      // Update Views
      views.forEach(view => {
        if (view.id === targetView) {
          view.classList.add('active');
        } else {
          view.classList.remove('active');
        }
      });
    });
  });

  /**
   * Logging
   */
  function addLog(text, isSystem = false) {
    const isError = text.toLowerCase().includes('error:') || text.toLowerCase().includes('exception') || text.toLowerCase().includes('trace');
    const isWarning = text.toLowerCase().includes('warn') || text.toLowerCase().includes('deprecated');

    const line = document.createElement('div');
    line.className = 'log-line';
    
    if (isSystem) line.classList.add('system');
    else if (isError) line.classList.add('error');
    else if (isWarning) line.classList.add('warning');
    else line.classList.add('normal');

    const time = new Date().toLocaleTimeString();
    const timeSpan = document.createElement('span');
    timeSpan.className = 'timestamp';
    timeSpan.textContent = `[${time}] `;

    const msgSpan = document.createElement('span');
    msgSpan.className = 'message';
    msgSpan.textContent = text;

    line.appendChild(timeSpan);
    line.appendChild(msgSpan);

    logContent.appendChild(line);
    
    // Trigger LED activity blink
    const actLed = document.getElementById('activityIndicator');
    if (actLed && isRunning) {
      actLed.classList.add('blink');
      setTimeout(() => actLed.classList.remove('blink'), 80);
    }

    while (logContent.children.length > LOG_LIMIT) {
      logContent.removeChild(logContent.firstChild);
    }
    logContent.scrollTop = logContent.scrollHeight;
  }

  listen('log-event', (event) => {
    addLog(event.payload);
  });

  // User clicked 'I've Updated It' → retry starting server
  listen('guide-retry', () => {
    toggleBtn.click();
  });

  // User clicked Start/Stop Server from System Tray
  listen('tray-toggle', () => {
    if (toggleBtn && !toggleBtn.disabled && !toggleBtn.classList.contains('dimmed')) {
      toggleBtn.click();
    }
  });

  listen('port-detected', (event) => {
    const detectedPort = event.payload;
    const configuredPort = parseInt(portInput.value) || 3000;
    
    // Update the browser link to the actual detected port
    browserLink.textContent = `http://localhost:${detectedPort}`;
    browserLink.onclick = (e) => {
      e.preventDefault();
      invoke('open_browser', { url: `http://localhost:${detectedPort}` });
    };

    if (detectedPort !== configuredPort) {
      portMismatchDetected = true;
      const msg = `[KenBun Resolver] Port mismatch detected. Configured: ${configuredPort}, Script used: ${detectedPort}. Adapting to script's port.`;
      addLog(msg, true);
      showToast(`Adapted to Port ${detectedPort}`, "warning");
    } else {
      portMismatchDetected = false;
      addLog(`[KenBun Resolver] Port verified: ${detectedPort}`, true);
    }
  });

  listen('process-exit', () => {
    const isAutoRestart = localStorage.getItem('ken_auto_restart') === 'true';
    
    if (isManualStopping) {
      // Jika berhenti karena tombol STOP, jangan restart
      isManualStopping = false;
      setRunningState(false);
      return;
    }

    if (portMismatchDetected && isRunning) {
      // Mismatch Guard: Jangan auto-restart jika port mismatch terdeteksi
      addLog("[KenBun Guard] Auto-restart blocked! Port mismatch caused crash.", true);
      addLog("[KenBun Guard] Your script uses a hardcoded port that may be occupied.", true);
      addLog("[KenBun Guard] Fix: update your script to use process.env.PORT", true);
      showToast("Auto-restart blocked due to port mismatch. Check logs.", "error");
      portMismatchDetected = false;
      setRunningState(false);
    } else if (isAutoRestart && isRunning) {
      const maxRetries = parseInt(localStorage.getItem('ken_max_retries')) || 5;
      
      if (retryCount < maxRetries) {
        retryCount++;
        addLog(`Process exited unexpectedly. Auto-restarting (${retryCount}/${maxRetries})...`, true);
        
        const useBackoff = localStorage.getItem('ken_backoff') === 'true';
        const initDelaySec = parseInt(localStorage.getItem('ken_initial_delay')) || 2;
        const maxDelaySec = parseInt(localStorage.getItem('ken_max_delay')) || 60;
        
        let delayMs = initDelaySec * 1000;
        if (useBackoff) {
          let calcDelay = initDelaySec * Math.pow(2, retryCount - 1);
          if (calcDelay > maxDelaySec) calcDelay = maxDelaySec;
          delayMs = calcDelay * 1000;
        }

        setTimeout(() => {
          setRunningState(false);
          toggleBtn.click();
        }, delayMs);
      } else {
        addLog(`[KenBun Guard] Max retries (${maxRetries}) reached. Auto-restart aborted.`, true);
        addLog("[KenBun Guard] Please check terminal logs for errors and fix your code.", true);
        showToast("Max retries reached. Check logs and fix your code.", "error");
        retryCount = 0; // Reset for next manual attempt
        setRunningState(false);
      }
    } else {
      setRunningState(false);
    }
  });

  /**
   * Update UI State
   */
  function setRunningState(running) {
    isRunning = running;
    const port = portInput.value || 3000;
    const serverUrlBox = document.getElementById('serverUrlBox');

    const lcdScreen = document.querySelector('.lcd-screen');
    const activityIndicator = document.getElementById('activityIndicator');
    const networkIndicator = document.getElementById('networkIndicator');
    const chassis = document.querySelector('.server-chassis');

    if (running) {
      statusIndicator.className = 'status-indicator running';
      if (activityIndicator) activityIndicator.className = 'status-indicator running act';
      if (networkIndicator) networkIndicator.className = 'status-indicator running net';
      
      lcdScreen.classList.add('running');
      if (chassis) chassis.classList.add('running');
      statusTextBig.textContent = 'RUNNING';
      statusTextBig.style.color = '#00f2fe';
      statusTextBig.style.textShadow = '0 0 15px rgba(0, 242, 254, 0.6)';
      
      toggleBtn.textContent = 'RESTART SERVER';
      stopBtn.classList.remove('hidden');
      serverUrlBox.classList.remove('hidden');
      
      browserLink.textContent = `http://localhost:${port}`;
      browserLink.onclick = (e) => {
        e.preventDefault();
        invoke('open_browser', { url: `http://localhost:${port}` });
      };

      addLog("Server is running on port " + port, true);

      // Start Uptime & LCD Animation
      serverStartTime = Date.now();
      clearInterval(uptimeInterval);
      uptimeInterval = setInterval(() => {
        if (!isRunning) return;
        
        const now = Date.now();
        const diff = now - serverStartTime;
        const h = Math.floor(diff / 3600000).toString().padStart(2, '0');
        const m = Math.floor((diff % 3600000) / 60000).toString().padStart(2, '0');
        const s = Math.floor((diff % 60000) / 1000).toString().padStart(2, '0');
        
        // Change display every 4 seconds
        lcdCycle = Math.floor(now / 4000) % 2;
        
        if (lcdCycle === 0) {
          statusTextBig.textContent = 'RUNNING';
        } else {
          statusTextBig.textContent = `UP ${h}:${m}:${s}`;
        }
      }, 500);
    } else {
      statusIndicator.className = 'status-indicator stopped';
      if (activityIndicator) activityIndicator.className = 'status-indicator stopped act';
      if (networkIndicator) networkIndicator.className = 'status-indicator stopped net';
      
      lcdScreen.classList.remove('running');
      if (chassis) chassis.classList.remove('running');
      statusTextBig.textContent = '';
      statusTextBig.style.color = '#334155';
      statusTextBig.style.textShadow = 'none';
      
      toggleBtn.textContent = 'START SERVER';
      stopBtn.classList.add('hidden');
      serverUrlBox.classList.add('hidden');
      
      addLog("Server has been stopped.", true);
      
      // Stop Uptime
      clearInterval(uptimeInterval);
      serverStartTime = null;
    }

    // Update system tray menu dynamically
    const projectName = document.getElementById('projectNameDisplay').textContent.replace('No Folder Selected', '').trim();
    invoke('update_tray_menu', { projectName: projectName, isRunning: running }).catch(err => console.error("Tray Update Failed:", err));
  }

  /**
   * UI Helpers
   */
  function showToast(message, type = 'info') {
    const container = document.getElementById('toastContainer');
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    
    let icon = 'ℹ️';
    if (type === 'error') icon = '❌';
    if (type === 'warning') icon = '⚠️';
    if (type === 'success') icon = '✅';
    
    toast.innerHTML = `
      <div class="toast-icon">${icon}</div>
      <div class="toast-content">${message}</div>
    `;
    
    container.appendChild(toast);
    
    // Trigger animation
    setTimeout(() => toast.classList.add('show'), 10);
    
    // Auto remove (Ditingkatkan menjadi 6 detik agar lebih mudah dibaca)
    setTimeout(() => {
      toast.classList.remove('show');
      setTimeout(() => toast.remove(), 500);
    }, 6000);
  }

  /**
   * Actions
   */
  toggleBtn.addEventListener('click', async (e) => {
    // Reset retry count on manual interaction
    if (e && e.isTrusted) {
      retryCount = 0;
    }

    // Validasi folder project
    const currentPath = await invoke('get_project_path');
    if (!currentPath || currentPath === "") {
      const prompt = document.getElementById('projectPrompt');
      prompt.classList.add('danger');
      setTimeout(() => prompt.classList.remove('danger'), 500);
      return;
    }

    const scriptPath = localStorage.getItem('ken_script_path') || 'index.js';

    // 1. Verifikasi folder & entry script
    try {
      await invoke('verify_project', { projectPath: currentPath, scriptPath: scriptPath });
    } catch (err) {
      toggleBtn.disabled = false;
      
      if (err === "FOLDER_NOT_PROJECT") {
        showToast("This folder is not a valid Bun project. Please run 'bun init'.", "warning");
        return;
      } else if (err === "NODE_PROJECT_DETECTED") {
        showToast("Pure Node.js project detected. Please run 'bun install' first to migrate to Bun.", "warning");
        return;
      } else if (typeof err === 'string' && err.startsWith("NODE_FRAMEWORK_DETECTED:")) {
        const frameworks = err.split(":")[1];
        const port = parseInt(portInput.value) || 3000;
        
        // Simpan ke localStorage agar bisa dibaca jendela baru/lama
        localStorage.setItem('ken_guide_frameworks', frameworks);
        localStorage.setItem('ken_guide_port', port);

        try {
          await invoke('open_guide_window');
        } catch (e) {
          showToast(`Node.js framework detected: ${frameworks}. Update your script to use process.env.PORT.`, "warning");
        }
        return;
      } else if (typeof err === 'string' && err.startsWith("SCRIPT_MISSING:")) {
        const missingFile = err.split(":")[1];
        showToast(`File "${missingFile}" not found. Please create it to start the server.`, "error");
        return;
      } else {
        showToast(`Verification failed: ${err}`, "error");
        return;
      }
    }

    const port = parseInt(portInput.value) || 3000;
    toggleBtn.classList.add('is-loading');
    
    // Artificial delay to make the animation visible and feel "solid"
    await new Promise(resolve => setTimeout(resolve, 800));

    try {
      if (isRunning) {
        addLog("Restarting server...", true);
        isManualStopping = true; 
        await invoke('stop_bun', { port });
      }
      
      portMismatchDetected = false; // Reset mismatch guard on fresh start
      addLog("Starting Bun process...", true);
      const conflictSolver = document.getElementById('portConflictSolver').checked;
      const forcePort = document.getElementById('forcePort').checked;
      const pid = await invoke('start_bun', { 
        port, 
        scriptPath: scriptPath,
        conflictSolver: conflictSolver,
        forcePort: forcePort
      });

      console.log("[KenBun] Started with PID:", pid);
      setRunningState(true);
      
      // Jika berhasil jalan, tutup jendela panduan edukasi (jika terbuka)
      if (window.__TAURI__) {
        window.__TAURI__.event.emit('guide-close', {});
      }
    } catch (err) {
      console.error("[KenBun] Server Start Fail:", err);
      
      if (err === "PORT_IN_USE") {
        showToast(`Port ${port} is already in use by another application. Please choose a different port in Settings.`, "warning");
        addLog(`CONFLICT: Port ${port} is occupied. Please change the port.`, true);
      } else {
        showToast(`Failed to start: ${err}`, "error");
        addLog(`ERROR: ${err}`, true);
      }
      
      setRunningState(false);
      isManualStopping = false;
    } finally {
      toggleBtn.classList.remove('is-loading');
    }
  });

  stopBtn.addEventListener('click', async () => {
    const port = parseInt(portInput.value) || 3000;
    stopBtn.classList.add('is-loading');
    
    try {
      addLog("Stopping server...", true);
      isManualStopping = true; // Tandai sebagai manual
      await invoke('stop_bun', { port });
      
      // Small delay for stop animation feel
      await new Promise(resolve => setTimeout(resolve, 500));
      
      setRunningState(false);
    } catch (err) {
      addLog(`ERROR: ${err}`, true);
      isManualStopping = false;
    } finally {
      stopBtn.classList.remove('is-loading');
    }
  });

  // Save Settings
  if (saveSettingsBtn) {
    saveSettingsBtn.addEventListener('click', async () => {
      localStorage.setItem('ken_port', portInput.value);
      localStorage.setItem('ken_auto_restart', document.getElementById('autoRestart').checked);
      localStorage.setItem('ken_notify_status', document.getElementById('notifyStatus').checked);
      localStorage.setItem('ken_notify_crash', document.getElementById('notifyCrash').checked);
      localStorage.setItem('ken_backoff', document.getElementById('backoffStrategy').checked);
      localStorage.setItem('ken_conflict_solver', document.getElementById('portConflictSolver').checked);
      localStorage.setItem('ken_force_port', document.getElementById('forcePort').checked);

      localStorage.setItem('ken_script_path', scriptPathInput.value);
      localStorage.setItem('ken_initial_delay', document.getElementById('initialDelay').value);
      localStorage.setItem('ken_max_delay', document.getElementById('maxDelay').value);
      localStorage.setItem('ken_max_retries', document.getElementById('maxRetries').value);
      
      browserLink.textContent = `http://localhost:${portInput.value}`;
      showToast("Settings saved successfully.", "success");
      
      // Feedback effect
      const originalText = saveSettingsBtn.textContent;
      saveSettingsBtn.textContent = "SAVED!";
      setTimeout(() => { 
        saveSettingsBtn.textContent = originalText;
      }, 2000);
    });
  }

  // Reset Settings
  if (resetSettingsBtn) {
    resetSettingsBtn.addEventListener('click', () => {
      if (confirm("Reset all settings to default?")) {
        localStorage.clear();
        window.location.reload();
      }
    });
  }

  // Open Folder Logic
  const openFolderBtn = document.getElementById('openFolderBtn');
  openFolderBtn.addEventListener('click', async () => {
    if (isRunning) {
      showToast("Please stop the server before switching the project folder.", "warning");
      return;
    }

    try {
      const { open } = window.__TAURI__.dialog;
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Bun Project Folder"
      });

      if (selected) {
        await invoke('set_project_path', { path: selected });
        localStorage.setItem('ken_project_path', selected);
        initProjectPath();
        addLog(`Project folder changed to: ${selected}`, true);
      }
    } catch (err) {
      console.error("Gagal memilih folder:", err);
    }
  });

  // Handle GitHub Link
  const githubLink = document.getElementById('githubLink');
  if (githubLink) {
    githubLink.onclick = (e) => {
      e.preventDefault();
      invoke('open_browser', { url: 'https://github.com/candrasp/KenBun' });
    };
  }

  // Handle Copy Installation Command
  const copyBtn = document.getElementById('copyCommandBtn');
  const installCode = document.getElementById('installCommand');
  if (copyBtn && installCode) {
    copyBtn.onclick = async () => {
      try {
        await navigator.clipboard.writeText(installCode.textContent);
        copyBtn.classList.add('copied');
        const originalIcon = copyBtn.innerHTML;
        copyBtn.innerHTML = `
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>`;
        
        setTimeout(() => {
          copyBtn.classList.remove('copied');
          copyBtn.innerHTML = originalIcon;
        }, 2000);
      } catch (err) {
        console.error('Failed to copy:', err);
      }
    };
  }

  // Check initial state if possible (optional, for now assuming stopped)
  setRunningState(false);
});
