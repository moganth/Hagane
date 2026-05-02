let _installStarted = false;
let _installDone = false;
let _handledInstallComplete = false;
let _pollTimer = null;

function applyTheme(s){const r=document.documentElement;r.style.setProperty('--accent',s.accent_color||'#0078D4');r.style.setProperty('--accent-dark',s.accent_dark_color||'#005A9E');r.style.setProperty('--accent-light',s.accent_light_color||'#EBF3FB');r.style.setProperty('--bg',s.background_color||'#FFFFFF');r.style.setProperty('--bg-alt',s.surface_color||'#F5F5F5');r.style.setProperty('--text',s.text_color||'#1A1A1A');r.style.setProperty('--text-muted',s.text_muted_color||'#6B6B6B');r.style.setProperty('--border',s.border_color||'#E0E0E0');r.style.setProperty('--radius',(s.border_radius??6)+'px');r.style.setProperty('--font',s.font_family||"'Segoe UI',system-ui,sans-serif");r.style.setProperty('--success',s.success_color||'#107C10');r.style.setProperty('--success-bg',s.success_bg_color||'#F7F9F8');r.style.setProperty('--error',s.error_color||'#C42B1C');r.style.setProperty('--error-bg',s.error_bg_color||'#FFF7F6');r.style.setProperty('--progress',s.progress_color||'#0078D4');r.style.setProperty('--progress-light',s.progress_light_color||'#EBF3FB');}

function onPageStateUpdate(s) {
  applyTheme(s);
  const isUninstall = !!s.is_uninstall;
  document.getElementById('page-title').textContent = isUninstall ? 'Uninstalling' : 'Brewing';
  const iconAsset = s.logo_b64;
  if (iconAsset) {
    const l = document.getElementById("banner-logo");
    l.src = (iconAsset.startsWith && iconAsset.startsWith("data:")) ? iconAsset : "data:image/png;base64," + iconAsset;
    l.style.display = "block";
    document.getElementById("banner-logo-ph").style.display = "none";
  }
  const bannerAsset = s.banner_b64;
  if (bannerAsset) {
    const art = document.getElementById("banner-art");
    if (art) {
      const src = (bannerAsset.startsWith && bannerAsset.startsWith("data:")) ? bannerAsset : "data:image/png;base64," + bannerAsset;
      art.style.backgroundImage = "url(" + src + ")";
      art.style.backgroundSize = "cover";
      art.style.backgroundPosition = "center";
    }
  }
  if (s.progress) {
    updateProgress(s.progress.percent, s.progress.current_label, s.progress.current_step, s.progress.total_steps);
  }
  if (!_handledInstallComplete && s.install_succeeded !== null && s.install_succeeded !== undefined) {
    _handledInstallComplete = true;
    handleInstallComplete({success: s.install_succeeded, error: s.install_error});
  }
  if (!_installStarted) {
    _installStarted = true;
    send('next');
  }
}

function updateProgress(pct, label, current, total) {
  const progressBar = document.getElementById('progress-bar');
  if (progressBar) progressBar.style.width = pct + '%';
  document.getElementById('progress-pct').textContent = pct + '%';
  const brewStage = document.getElementById('brew-stage-label');
  const brewLeft = document.getElementById('brew-caption-left');
  const brewRight = document.getElementById('brew-caption-right');
  const cupFill = document.getElementById('cup-fill');
  if (cupFill) cupFill.style.height = Math.max(4, Math.min(100, pct)) + '%';
  if (brewRight) brewRight.textContent = pct + '%';
  const hasBrewUi = !!brewStage && !!brewLeft && !!brewRight && !!cupFill;

  if (!hasBrewUi) {
    document.getElementById('progress-label').textContent = label || '';
    document.getElementById('status-text').textContent =
      (current && total) ? `Step ${current} of ${total}` : 'Please wait...';
    return;
  }

  if (pct < 18) {
    document.getElementById('progress-label').textContent = 'Warming the kettle...';
    if (brewStage) brewStage.textContent = 'Warming the kettle';
    if (brewLeft) brewLeft.textContent = 'Heating up the brew...';
  } else if (pct < 45) {
    document.getElementById('progress-label').textContent = 'Grinding the beans...';
    if (brewStage) brewStage.textContent = 'Grinding the beans';
    if (brewLeft) brewLeft.textContent = 'Grinding and blending...';
  } else if (pct < 75) {
    document.getElementById('progress-label').textContent = 'Frothing the latte...';
    if (brewStage) brewStage.textContent = 'Frothing the latte';
    if (brewLeft) brewLeft.textContent = 'Building the foam...';
  } else if (pct < 100) {
    document.getElementById('progress-label').textContent = 'Pouring the final cup...';
    if (brewStage) brewStage.textContent = 'Pouring the final cup';
    if (brewLeft) brewLeft.textContent = 'Finishing the pour...';
  } else {
    document.getElementById('progress-label').textContent = 'Coffee is ready.';
    if (brewStage) brewStage.textContent = 'Coffee is ready';
    if (brewLeft) brewLeft.textContent = 'Serving the final cup...';
  }
  document.getElementById('status-text').textContent =
    (current && total) ? `Step ${current} of ${total}` : 'Please wait...';
}

function addLog(text, isErr) {
  // Intentionally no-op for the caramel_latte visual mode.
}

function handleInstallComplete(e) {
  if (_installDone) return;
  _installDone = true;
  const isUninstall = (window.__lastState && window.__lastState.is_uninstall) || false;
  if (_pollTimer) {
    clearInterval(_pollTimer);
    _pollTimer = null;
  }
  document.getElementById('spinner').style.display = 'none';
  const btnCancel = document.getElementById('btn-cancel');
  const btnNext   = document.getElementById('btn-next');
  if (e.success) {
    const progressBar = document.getElementById('progress-bar');
    if (progressBar) progressBar.style.width = '100%';
    document.getElementById('page-title').textContent = isUninstall ? 'Uninstall Complete' : 'Brewing Complete';
    document.getElementById('progress-pct').textContent = '100%';
    document.getElementById('progress-label').textContent = isUninstall ? 'Uninstall complete.' : 'Done';
    const brewStage = document.getElementById('brew-stage-label');
    const brewLeft = document.getElementById('brew-caption-left');
    const brewRight = document.getElementById('brew-caption-right');
    const cupFill = document.getElementById('cup-fill');
    if (cupFill) cupFill.style.height = '100%';
    if (brewStage) {
      document.getElementById('page-title').textContent = isUninstall ? 'Uninstall Complete' : 'Brewing Complete';
      document.getElementById('progress-label').textContent = isUninstall ? 'Cup cleared.' : 'Your latte is ready.';
      brewStage.textContent = isUninstall ? 'Cleanup complete' : 'Latte served';
      if (brewLeft) brewLeft.textContent = isUninstall ? 'Cleanup complete.' : 'Fresh cup served.';
      if (brewRight) brewRight.textContent = '100%';
    } else {
      document.getElementById('page-title').textContent = isUninstall ? 'Uninstall Complete' : 'Installation Complete';
    }
    document.getElementById('status-text').textContent = isUninstall ? 'All uninstall steps completed.' : 'All steps completed.';
    btnCancel.style.display = 'none';
    btnNext.style.display = 'inline-flex';
    addLog(isUninstall ? 'Uninstall completed successfully.' : 'Installation completed successfully.', false);
  } else {
    document.getElementById('page-title').textContent = isUninstall ? 'Uninstall Failed' : 'Installation Failed';
    btnCancel.textContent = 'Close';
    btnCancel.disabled = false;
    addLog('FAILED: ' + (e.error || 'Unknown error'), true);
  }
}

function send(t,p={}){const m=JSON.stringify({type:t,...p});if(window.chrome&&window.chrome.webview)window.chrome.webview.postMessage(m);else console.log('[->]',m)}
function showCancelModal(){if(document.getElementById('cancel-modal-overlay'))return;const o=document.createElement('div');o.id='cancel-modal-overlay';o.style.cssText='position:fixed;inset:0;background:rgba(0,0,0,.35);display:flex;align-items:center;justify-content:center;z-index:9999';const c=document.createElement('div');c.style.cssText='width:min(420px,calc(100% - 32px));background:var(--bg);border:1px solid var(--border);border-radius:var(--radius);box-shadow:0 16px 36px rgba(0,0,0,.24);padding:18px';c.innerHTML='<div style="font-size:16px;font-weight:600;margin-bottom:6px">Cancel installation?</div><div style="font-size:13px;color:var(--text-muted);line-height:1.5">Installation is in progress. Exiting now will trigger rollback of applied steps.</div>';const a=document.createElement('div');a.style.cssText='display:flex;justify-content:flex-end;gap:8px;margin-top:16px';const k=document.createElement('button');k.className='btn btn-secondary';k.textContent='Continue setup';const x=document.createElement('button');x.className='btn btn-primary';x.textContent='Exit setup';a.appendChild(k);a.appendChild(x);c.appendChild(a);o.appendChild(c);document.body.appendChild(o);const close=()=>o.remove();k.onclick=close;o.onclick=(e)=>{if(e.target===o)close()};x.onclick=()=>{close();send('cancel')}}
window.__engineEvent=function(e){
  switch(e.event){
    case 'state_update':     window.__lastState=e.state; onPageStateUpdate(e.state); break;
    case 'progress':         updateProgress(e.percent,e.label,e.current,e.total); break;
    case 'log_line':         break;
    case 'install_complete': handleInstallComplete(e); break;
  }
};
document.addEventListener('DOMContentLoaded',()=>{
  send('ready');
  _pollTimer = setInterval(() => {
    if (!_installDone) send('get_state');
  }, 300);
  document.getElementById('btn-next').onclick=()=>send('next');
  document.getElementById('btn-cancel').onclick=()=>showCancelModal();
});