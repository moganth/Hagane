// ── Installer Engine — JS Bridge ─────────────────────────────────────────────
// All pages include this script. It handles:
//   1. Sending messages to the Rust engine
//   2. Receiving state + events from the Rust engine
//   3. Applying theme CSS variables from state
//   4. Page-specific render hooks

'use strict';

// ── Send a message to Rust ────────────────────────────────────────────────────
function send(type, payload = {}) {
  const msg = JSON.stringify({ type, ...payload });
  if (window.chrome && window.chrome.webview) {
    window.chrome.webview.postMessage(msg);
  } else {
    // Dev-mode fallback (browser testing)
    console.log('[IPC → Rust]', msg);
  }
}

// ── Receive events from Rust ──────────────────────────────────────────────────
window.__engineEvent = function(event) {
  console.log('[IPC ← Rust]', event.event, event);
  switch (event.event) {
    case 'state_update':      onStateUpdate(event.state);           break;
    case 'navigate':          onNavigate(event.page);               break;
    case 'progress':          onProgress(event);                    break;
    case 'log_line':          onLogLine(event.text, false);         break;
    case 'install_complete':  onInstallComplete(event);             break;
    case 'requirements_result': onRequirementsResult(event);        break;
    case 'browse_result':     onBrowseResult(event.path);           break;
    case 'error':             onError(event.title, event.message);  break;
    default: console.warn('Unknown engine event:', event.event);
  }
};

// ── State update ──────────────────────────────────────────────────────────────
let _state = null;

function onStateUpdate(state) {
  _state = state;
  applyTheme(state);
  if (typeof onPageStateUpdate === 'function') {
    onPageStateUpdate(state);
  }
}

// ── Apply CSS theme variables from state ──────────────────────────────────────
function applyTheme(state) {
  const root = document.documentElement;
  root.style.setProperty('--accent',    state.accent_color     || '#0078D4');
  root.style.setProperty('--bg',        state.background_color || '#FFFFFF');
  root.style.setProperty('--text',      state.text_color       || '#1A1A1A');
  root.style.setProperty('--font',      state.font_family      || 'Segoe UI, sans-serif');

  // Derive accent-dark (darken by 20%)
  const dark = shadeColor(state.accent_color || '#0078D4', -20);
  root.style.setProperty('--accent-dark', dark);

  // Banner logo
  const logo = document.getElementById('banner-logo');
  if (logo && state.logo_b64) {
    logo.src = `data:image/png;base64,${state.logo_b64}`;
    logo.style.display = 'block';
    const placeholder = document.getElementById('banner-logo-placeholder');
    if (placeholder) placeholder.style.display = 'none';
  }

  // Back / Next button states
  const btnBack = document.getElementById('btn-back');
  const btnNext = document.getElementById('btn-next');
  if (btnBack) btnBack.disabled = !state.can_go_back_state;
  if (btnNext) btnNext.disabled = !state.can_go_next_state;
}

// ── Navigation events ─────────────────────────────────────────────────────────
function onNavigate(page) {
  // Navigation is handled by Rust (it changes the HTML loaded in WebView)
  console.log('Navigate to:', page);
}

// ── Progress ──────────────────────────────────────────────────────────────────
function onProgress(e) {
  const bar = document.getElementById('progress-bar');
  const label = document.getElementById('progress-label');
  const pct = document.getElementById('progress-pct');
  if (bar)   bar.style.width = e.percent + '%';
  if (label) label.textContent = e.label;
  if (pct)   pct.textContent = e.percent + '%';
}

function onLogLine(text, isError = false) {
  const box = document.getElementById('log-box');
  if (!box) return;
  const line = document.createElement('div');
  line.className = 'log-line' + (isError ? ' err' : '');
  line.textContent = text;
  box.appendChild(line);
  box.scrollTop = box.scrollHeight;
}

// ── Install complete ──────────────────────────────────────────────────────────
function onInstallComplete(e) {
  if (typeof handleInstallComplete === 'function') {
    handleInstallComplete(e);
  }
}

// ── Requirements results ──────────────────────────────────────────────────────
function onRequirementsResult(e) {
  if (typeof renderRequirements === 'function') {
    renderRequirements(e.results, e.all_passed);
  }
}

// ── Browse result ─────────────────────────────────────────────────────────────
function onBrowseResult(path) {
  if (path) {
    const input = document.getElementById('install-dir-input');
    if (input) {
      input.value = path;
      send('set_install_dir', { path });
    }
  }
}

// ── Error ─────────────────────────────────────────────────────────────────────
function onError(title, message) {
  alert(`${title}\n\n${message}`);
}

// ── Standard footer button wiring ─────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', () => {
  // Signal to Rust that the page is ready
  send('ready');

  const btnNext   = document.getElementById('btn-next');
  const btnBack   = document.getElementById('btn-back');
  const btnCancel = document.getElementById('btn-cancel');

  if (btnNext)   btnNext.addEventListener('click', () => send('next'));
  if (btnBack)   btnBack.addEventListener('click', () => send('back'));
  if (btnCancel) btnCancel.addEventListener('click', () => {
    if (confirm('Are you sure you want to cancel the installation?')) {
      send('cancel');
    }
  });
});

// ── Utility ───────────────────────────────────────────────────────────────────
function shadeColor(hex, percent) {
  const n = parseInt(hex.replace('#', ''), 16);
  const r = Math.min(255, Math.max(0, (n >> 16) + percent * 2.55 | 0));
  const g = Math.min(255, Math.max(0, ((n >> 8) & 0xff) + percent * 2.55 | 0));
  const b = Math.min(255, Math.max(0, (n & 0xff) + percent * 2.55 | 0));
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`;
}

function fmtSize(mb) {
  if (!mb) return '';
  return mb >= 1024 ? (mb / 1024).toFixed(1) + ' GB' : mb + ' MB';
}