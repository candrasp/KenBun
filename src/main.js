// Tauri v2 – gunakan window.__TAURI__.core.invoke
// Tunggu DOMContentLoaded untuk memastikan elemen tersedia
document.addEventListener('DOMContentLoaded', async () => {
  if (!window.__TAURI__) {
    console.error("Tauri API tidak ditemukan. Pastikan 'withGlobalTauri' diaktifkan di tauri.conf.json.");
    document.getElementById('statusText').textContent = "Error: Tauri API tidak ditemukan";
    return;
  }

  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  // Elements - Views
  const mainView = document.getElementById('mainView');
  const settingsView = document.getElementById('settingsView');
  const logView = document.getElementById('logView');

  // Elements - Main Buttons
  const btn = document.getElementById('toggleBtn');
  const btnText = btn.querySelector('.btn-text');
  const statusText = document.getElementById('statusText');
  const settingsBtn = document.getElementById('settingsBtn');
  const logBtn = document.getElementById('logBtn');

  // Elements - Settings
  const portInput = document.getElementById('portInput');
  const backBtn = document.getElementById('backBtn');
  const saveBtn = document.getElementById('saveBtn');

  // Elements - Logs
  const logContent = document.getElementById('logContent');
  const closeLogBtn = document.getElementById('closeLogBtn');
  const clearLogBtn = document.getElementById('clearLogBtn');

  let isRunning = false;
  const LOG_LIMIT = 500;

  // Load Port from localStorage
  const savedPort = localStorage.getItem('ken_port') || '3000';
  portInput.value = savedPort;

  console.log("KenBun Initialized");

  /**
   * Menambahkan baris log ke tampilan
   */
  function addLog(text, isSystem = false) {
    const line = document.createElement('div');
    line.className = isSystem ? 'log-line system' : 'log-line';
    line.textContent = text;

    logContent.appendChild(line);

    // Limiter 500 baris
    while (logContent.children.length > LOG_LIMIT) {
      logContent.removeChild(logContent.firstChild);
    }

    // Auto-scroll ke bawah
    logContent.scrollTop = logContent.scrollHeight;
  }

  // Tombol Fullscreen Log
  document.getElementById('expandLogBtn').addEventListener('click', async () => {
    try {
      await invoke('open_log_window');
    } catch (err) {
      console.error("Gagal membuka jendela log:", err);
    }
  });

  // Listener Event Log dari Rust
  listen('log-event', (event) => {
    addLog(event.payload);
  });

  /**
   * Perbarui tampilan tombol dan status text
   */
  function setRunningState(running) {
    isRunning = running;
    const port = portInput.value || 3000;

    if (running) {
      btn.className = 'btn btn-running';
      btnText.textContent = 'Stop';
      settingsBtn.style.display = 'none';

      statusText.innerHTML = `
        Aplikasi bisa diakses via browser:<br>
        <a href="#" id="browserLink" style="color: #10b981; text-decoration: underline; font-weight: bold;">http://localhost:${port}</a>
      `;

      statusText.classList.add('active');

      document.getElementById('browserLink').addEventListener('click', (e) => {
        e.preventDefault();
        invoke('open_browser', { url: `http://localhost:${port}` });
      });

      addLog("Server telah diaktifkan.", true);
    } else {
      btn.className = 'btn btn-aktif';
      btnText.textContent = 'Start';
      settingsBtn.style.display = 'flex';
      statusText.textContent = 'Siap dijalankan.';
      statusText.classList.remove('active');

      addLog("Server telah dihentikan.", true);
    }
  }

  /**
   * Navigasi
   */
  settingsBtn.addEventListener('click', () => {
    mainView.classList.add('hidden');
    settingsView.classList.remove('hidden');
  });

  logBtn.addEventListener('click', () => {
    mainView.classList.add('hidden');
    logView.classList.remove('hidden');
  });

  const goBack = () => {
    settingsView.classList.add('hidden');
    logView.classList.add('hidden');
    mainView.classList.remove('hidden');
  };

  backBtn.addEventListener('click', goBack);
  closeLogBtn.addEventListener('click', goBack);

  saveBtn.addEventListener('click', () => {
    localStorage.setItem('ken_port', portInput.value);
    goBack();
    addLog(`Port diubah ke ${portInput.value}`, true);
  });

  clearLogBtn.addEventListener('click', () => {
    logContent.innerHTML = '<div class="log-line system">Log dibersihkan...</div>';
  });

  /**
   * Handler klik tombol Start/Stop
   */
  btn.addEventListener('click', async () => {
    const port = parseInt(portInput.value) || 3000;
    btn.disabled = true;

    try {
      if (!isRunning) {
        statusText.textContent = 'Memulai proses...';
        addLog("Menghubungkan ke Bun...", true);
        const pid = await invoke('start_bun', { port });
        setRunningState(true);

        setTimeout(async () => {
          if (isRunning) {
            await invoke('open_browser', { url: `http://localhost:${port}` });
          }
        }, 500);
      } else {
        statusText.textContent = 'Menghentikan proses...';
        await invoke('stop_bun', { port });
        setRunningState(false);
      }
    } catch (err) {
      statusText.textContent = 'Error: ' + err;
      statusText.classList.remove('active');
      addLog(`ERROR: ${err}`, true);
      console.error('[Ken App] Error:', err);
    } finally {
      btn.disabled = false;
    }
  });
});
