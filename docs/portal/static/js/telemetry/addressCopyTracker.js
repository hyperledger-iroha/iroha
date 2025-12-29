(function () {
  if (typeof window === 'undefined') {
    return;
  }

  function track(detail) {
    if (!detail || !detail.mode) {
      return;
    }
    // Example: push to a global analytics queue; replace with Segment/OTEL as needed.
    window.dataLayer = window.dataLayer || [];
    window.dataLayer.push({
      event: 'iroha_address_copy',
      copyMode: detail.mode,
      timestamp: detail.timestamp,
    });
  }

  window.irohaCopyTelemetry = {track};
  window.addEventListener('iroha:address-copy', (event) => {
    track(event.detail);
  });
})();
