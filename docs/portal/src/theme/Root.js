import React from 'react';

import AnalyticsTracker from '../components/AnalyticsTracker.jsx';

export default function Root({children}) {
  return (
    <>
      <AnalyticsTracker />
      {children}
    </>
  );
}
