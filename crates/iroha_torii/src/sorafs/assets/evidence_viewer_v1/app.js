'use strict';
(() => {
  const expectedOrigin = window.location.origin;
  const stage = document.getElementById('stage');
  const warning = document.getElementById('warning');
  const reveal = document.getElementById('reveal');
  const status = document.getElementById('status');
  const binding = document.getElementById('case-binding');
  const canvas = document.getElementById('evidence');
  const context = canvas.getContext('2d', {alpha: false});
  const marks = Array.from(document.querySelectorAll('#watermarks span'));
  let acknowledged = false;

  const notify = (kind) => {
    if (window.parent !== window) {
      window.parent.postMessage({type: 'sorafs-evidence-viewer-event-v1', kind}, expectedOrigin);
    }
  };
  const deny = (event, kind) => {
    event.preventDefault();
    notify(kind);
  };
  document.addEventListener('contextmenu', event => deny(event, 'download_attempted'));
  document.addEventListener('dragstart', event => deny(event, 'download_attempted'));
  document.addEventListener('keydown', event => {
    const key = event.key.toLowerCase();
    if (event.key === 'PrintScreen') deny(event, 'screenshot_attempted');
    if ((event.ctrlKey || event.metaKey) && ['p', 's', 'u'].includes(key)) {
      deny(event, key === 'p' ? 'screenshot_attempted' : 'download_attempted');
    }
  });
  reveal.addEventListener('click', () => {
    acknowledged = true;
    warning.hidden = true;
    stage.hidden = false;
    notify('viewed');
  }, {once: true});
  window.addEventListener('blur', () => notify('paused'));
  window.addEventListener('pagehide', () => {
    context.clearRect(0, 0, canvas.width, canvas.height);
    notify('paused');
  });
  window.addEventListener('message', event => {
    if (event.origin !== expectedOrigin || event.source !== window.parent || !event.data) return;
    if (event.data.type === 'sorafs-evidence-manifest-v1') {
      const watermark = String(event.data.visible_watermark || 'CONFIDENTIAL');
      marks.forEach(mark => { mark.textContent = watermark; });
      binding.textContent = `Case ${String(event.data.case_id || '')} · Round ${String(event.data.round_id || '')}`;
      status.textContent = 'Case-bound manifest verified. Confirm the content warning to continue.';
      return;
    }
    if (event.data.type === 'sorafs-evidence-frame-v1' && event.data.frame instanceof ImageBitmap) {
      if (!acknowledged) {
        event.data.frame.close();
        return;
      }
      canvas.width = event.data.frame.width;
      canvas.height = event.data.frame.height;
      context.drawImage(event.data.frame, 0, 0);
      event.data.frame.close();
      status.textContent = 'Protected frame displayed.';
      notify('viewed');
      return;
    }
    if (event.data.type === 'sorafs-evidence-clear-v1') {
      context.clearRect(0, 0, canvas.width, canvas.height);
      status.textContent = 'Protected frame cleared.';
    }
  });
})();
