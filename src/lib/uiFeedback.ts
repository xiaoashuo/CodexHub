import type { ToastState } from '../types';

export function showActionToast(action: string): ToastState {
  return {
    title: `${action} 已触发`,
    description: `${action} 已记录。`,
  };
}

export function maskApiKey(apiKey: string) {
  const trimmedApiKey = apiKey.trim();
  const visibleLength = 4;
  const emptyApiKeyMask = 'sk-****-****-empty';

  if (!trimmedApiKey) {
    return emptyApiKeyMask;
  }

  return `sk-****-****-${trimmedApiKey.slice(-visibleLength)}`;
}
