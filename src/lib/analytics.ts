/**
 * Event names for tracking
 */
export const ANALYTICS_EVENTS = {
  // App Lifecycle
  APP_STARTED: "app_started",
  // License Events
  GET_LICENSE: "get_license",
} as const;

/**
 * Capture an analytics event (Neutralized)
 */
export const captureEvent = async (
  _eventName: string,
  _properties?: Record<string, any>
) => {
  // Telemetry removed
};

/**
 * Track app initialization (Neutralized)
 */
export const trackAppStart = async (_appVersion: string, _instanceId: string) => {
  // Telemetry removed
};
