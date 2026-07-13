import type { ModelConfig, RouterLogEntry } from '../types';

export const initialModels: ModelConfig[] = [
  {
    slug: 'test123',
    displayName: 'GPT5.5 中转',
    baseUrl: 'https://api.example-a.com/v1',
    apiKey: 'sk-****-****-8f2a',
    apiKeyMask: 'sk-****-****-8f2a',
    realModel: 'gpt-5.5',
    contextWindow: 272000,
    maxContextWindow: 272000,
    effectiveContextWindowPercent: 95,
    proxyMode: 'default',
    proxyUrl: '',
    protocolType: 'cpamc',
    endpointPath: '/responses',
    enabled: true,
    active: true,
    latency: '128ms',
    status: 'ready',
  },
  {
    slug: 'deepseek456',
    displayName: 'DeepSeek Pro',
    baseUrl: 'https://api.example-b.com/v1',
    apiKey: 'sk-****-****-19db',
    apiKeyMask: 'sk-****-****-19db',
    realModel: 'deepseek-pro',
    contextWindow: null,
    maxContextWindow: null,
    effectiveContextWindowPercent: null,
    proxyMode: 'default',
    proxyUrl: '',
    protocolType: 'other',
    endpointPath: '/chat/completions',
    enabled: true,
    active: false,
    latency: '96ms',
    status: 'ready',
  },
  {
    slug: 'claude789',
    displayName: 'Claude 兼容通道',
    baseUrl: 'https://api.example-c.com/v1',
    apiKey: 'sk-****-****-52ca',
    apiKeyMask: 'sk-****-****-52ca',
    realModel: 'claude-sonnet-compatible',
    contextWindow: null,
    maxContextWindow: null,
    effectiveContextWindowPercent: null,
    proxyMode: 'default',
    proxyUrl: '',
    protocolType: 'anthropic',
    endpointPath: '/messages',
    enabled: false,
    active: false,
    latency: '-',
    status: 'disabled',
  },
];

export const initialRouterLogs: RouterLogEntry[] = [
  { time: '14:21:16', source_ip: '127.0.0.1', method: 'POST', path: '/codex/router/v1/responses', status: '200', target_provider: 'example-a', cost: '1.4s', error_detail: '-' },
  { time: '14:18:03', source_ip: '127.0.0.1', method: 'POST', path: '/codex/router/v1/responses', status: '200', target_provider: 'example-b', cost: '920ms', error_detail: '-' },
  { time: '14:10:42', source_ip: '127.0.0.1', method: 'GET', path: '/health', status: 'Stopped', target_provider: '-', cost: '-', error_detail: '-' },
];

export const navItems = ['总览', '账号管理', '模型管理', '会话管理', '路由管理', '技能管理', 'MCP管理', '维护工具', '设置'] as const;

export type NavItem = (typeof navItems)[number];
