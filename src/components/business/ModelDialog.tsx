import { useState } from 'react';
import type { ReactNode } from 'react';
import { Eye, EyeOff, Tag, Plug, Gauge, Check, Boxes } from 'lucide-react';
import { Button } from '../ui/Button';

const INPUT_BASE =
  'w-full rounded-xl border border-slate-200 bg-white px-3.5 py-2.5 text-sm text-slate-800 placeholder-slate-400 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100';
import type { ModelConfig } from '../../types';
import type { ModelDialogFormValues, ModelDialogState, ModelHeaderPair } from '../../lib/appTypes';
import { invokeFetchProviderModels } from '../../lib/tauriBridge';

const DEFAULT_BASE_URL = 'https://api.example.com/v1';
const DEFAULT_REAL_MODEL = 'gpt-5.5';
const DEFAULT_PROTOCOL_TYPE = 'cpamc';
const MODEL_SELECT_PLACEHOLDER = '';

const PROTOCOL_OPTIONS = [
  { value: 'cpamc', label: 'response', endpoint: '/responses' },
  { value: 'openai', label: 'openai', endpoint: '/chat/completions' },
  { value: 'anthropic', label: 'anthropic', endpoint: '/messages' },
  { value: 'other', label: '其他', endpoint: '/chat/completions' },
];

const headersRecordToPairs = (headers?: Record<string, string>): ModelHeaderPair[] => {
  if (!headers) return [];
  return Object.entries(headers)
    .filter(([key]) => key.trim() !== '')
    .map(([key, value]) => ({ key, value }));
};

const headersPairsToRecord = (pairs: ModelHeaderPair[]): Record<string, string> => {
  const record: Record<string, string> = {};
  for (const pair of pairs) {
    const key = pair.key.trim();
    if (key !== '') {
      record[key] = pair.value;
    }
  }
  return record;
};

export function ModelDialog({
  state,
  handleModelDialogClose,
  handleModelDialogSave,
  handleModelChatTest,
}: {
  state: ModelDialogState;
  handleModelDialogClose: () => void;
  handleModelDialogSave: (values: ModelDialogFormValues) => Promise<void>;
  handleModelChatTest: (model: ModelConfig) => Promise<void>;
}) {
  const isCreateMode = state.mode === 'create';
  const model = state.model;
  const [formValues, setFormValues] = useState<ModelDialogFormValues>({
    displayName: model?.displayName ?? '',
    slug: model?.slug ?? '',
    baseUrl: model?.baseUrl ?? DEFAULT_BASE_URL,
    apiKey: model?.apiKey ?? '',
    realModel: model?.realModel ?? DEFAULT_REAL_MODEL,
    contextWindow: formatOptionalNumber(model?.contextWindow, ''),
    effectiveContextWindowPercent: formatOptionalNumber(model?.effectiveContextWindowPercent, ''),
    proxyMode: model?.proxyMode ?? 'default',
    proxyUrl: model?.proxyUrl ?? '',
    protocolType: model?.protocolType ?? DEFAULT_PROTOCOL_TYPE,
    endpointPath: model?.endpointPath ?? resolveDefaultEndpointPath(model?.protocolType ?? DEFAULT_PROTOCOL_TYPE),
    modelMappings: model?.modelMappings.join(', ') ?? '',
    customHeaders: headersRecordToPairs(model?.customHeaders),
    priority: String(model?.priority ?? 0),
    weight: String(model?.weight ?? 1),
    enabled: model?.enabled ?? true,
  });
  const [providerModels, setProviderModels] = useState<string[]>([]);
  const [providerModelsUrl, setProviderModelsUrl] = useState('');
  const [modelFetchLoading, setModelFetchLoading] = useState(false);
  const [modelFetchError, setModelFetchError] = useState('');
  const [chatTesting, setChatTesting] = useState(false);
  const [apiKeyVisible, setApiKeyVisible] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState('');

  const handleFormValueChange = (field: keyof ModelDialogFormValues, value: string | boolean) => {
    setFormValues((currentValues) => ({
      ...currentValues,
      [field]: value,
    }));
  };

  const handleProtocolTypeChange = (protocolType: string) => {
    setFormValues((currentValues) => ({
      ...currentValues,
      protocolType,
      endpointPath: resolveDefaultEndpointPath(protocolType),
    }));
  };

  const handleCustomHeaderChange = (index: number, field: 'key' | 'value', value: string) => {
    setFormValues((currentValues) => ({
      ...currentValues,
      customHeaders: currentValues.customHeaders.map((pair, pairIndex) =>
        pairIndex === index ? { ...pair, [field]: value } : pair,
      ),
    }));
  };

  const handleAddCustomHeader = () => {
    setFormValues((currentValues) => ({
      ...currentValues,
      customHeaders: [...currentValues.customHeaders, { key: '', value: '' }],
    }));
  };

  const handleRemoveCustomHeader = (index: number) => {
    setFormValues((currentValues) => ({
      ...currentValues,
      customHeaders: currentValues.customHeaders.filter((_, pairIndex) => pairIndex !== index),
    }));
  };

  const handleFetchProviderModels = async () => {
    const baseUrl = formValues.baseUrl.trim();
    const apiKey = formValues.apiKey.trim();
    setModelFetchError('');
    setProviderModels([]);
    setProviderModelsUrl('');

    if (!baseUrl) {
      setModelFetchError('请先填写 Base URL。');
      return;
    }
    if (!apiKey) {
      setModelFetchError('请先填写 API Key。');
      return;
    }

    setModelFetchLoading(true);
    try {
      const result = await invokeFetchProviderModels(baseUrl, apiKey, formValues.protocolType, formValues.proxyUrl);
      setProviderModels(result.models);
      setProviderModelsUrl(result.url);
    } catch (error) {
      setModelFetchError(error instanceof Error ? error.message : String(error));
    } finally {
      setModelFetchLoading(false);
    }
  };

  const handleChatTest = async () => {
    if (chatTesting) return;
    const testModel = model ?? {
      slug: formValues.slug || `temp_${Date.now()}`,
      displayName: formValues.displayName || '临时模型',
      baseUrl: formValues.baseUrl,
      apiKey: formValues.apiKey,
      apiKeyMask: formValues.apiKey ? `${formValues.apiKey.slice(0, 6)}****` : '',
      realModel: formValues.realModel,
      contextWindow: null,
      maxContextWindow: null,
      effectiveContextWindowPercent: null,
      proxyMode: formValues.proxyMode,
      proxyUrl: formValues.proxyUrl,
      protocolType: formValues.protocolType,
      endpointPath: formValues.endpointPath,
      modelMappings: parseModelMappings(formValues.modelMappings),
      customHeaders: headersPairsToRecord(formValues.customHeaders),
      priority: parseInteger(formValues.priority, 0),
      weight: Math.max(1, parseInteger(formValues.weight, 1)),
      enabled: formValues.enabled,
      active: false,
      latency: '-',
      status: 'ready' as const,
    };

    setChatTesting(true);
    try {
      await handleModelChatTest(testModel);
    } finally {
      setChatTesting(false);
    }
  };

  const handleSave = async () => {
    if (saving) return;
    const cleanedValues = {
      ...formValues,
      displayName: formValues.displayName.trim(),
      slug: formValues.slug.trim(),
      baseUrl: formValues.baseUrl.trim(),
      apiKey: formValues.apiKey.trim(),
      realModel: formValues.realModel.trim(),
      endpointPath: formValues.endpointPath.trim() || resolveDefaultEndpointPath(formValues.protocolType),
      proxyUrl: formValues.proxyUrl.trim(),
    };

    if (!cleanedValues.displayName || !cleanedValues.baseUrl || !cleanedValues.realModel) {
      setSaveError('请填写显示名称、Base URL 和真实模型。');
      return;
    }

    setSaving(true);
    setSaveError('');
    try {
      await handleModelDialogSave(cleanedValues);
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-slate-950/50 p-4 backdrop-blur-sm sm:p-6 lg:p-8">
      <div className="flex max-h-[calc(100vh-2rem)] w-full max-w-3xl flex-col overflow-hidden rounded-3xl border border-white/70 bg-slate-50 shadow-2xl shadow-slate-950/20 sm:max-h-[calc(100vh-3rem)] lg:max-h-[calc(100vh-4rem)]">
        <div className="flex shrink-0 items-start justify-between gap-4 border-b border-slate-200/80 bg-white px-5 py-4 sm:px-7 sm:py-5">
          <div className="flex items-center gap-3">
            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600">
              <Boxes size={20} />
            </span>
            <div>
              <h3 className="text-lg font-bold text-slate-950">{isCreateMode ? '新增模型' : '编辑模型'}</h3>
              <p className="text-xs text-slate-400">配置第三方模型 Provider 与路由策略</p>
            </div>
          </div>
          <Button variant="ghost" onClick={handleModelDialogClose} disabled={saving}>关闭</Button>
        </div>

        <div className="min-h-0 flex-1 space-y-5 overflow-y-auto px-5 py-5 sm:px-7 sm:py-6">
          <FormSection icon={Tag} title="基础信息" description="模型展示名称与运行状态。">
            <Field label="显示名称" className="sm:col-span-2">
              <input className={INPUT_BASE} value={formValues.displayName} placeholder="例如：GPT5.5 中转" onChange={(event) => handleFormValueChange('displayName', event.target.value)} />
            </Field>
            {!isCreateMode && (
              <Field label="Slug" className="sm:col-span-2">
                <input className="w-full cursor-not-allowed rounded-xl border border-slate-200 bg-slate-100 px-3.5 py-2.5 font-mono text-sm text-slate-500 outline-none" value={formValues.slug || '保存时自动生成'} readOnly />
                <p className="mt-1.5 text-xs text-slate-400">Slug 创建后固定，不支持修改。</p>
              </Field>
            )}
            <Field label="启用状态" className="sm:col-span-2">
              <Toggle checked={formValues.enabled} onChange={(checked) => handleFormValueChange('enabled', checked)} />
            </Field>
          </FormSection>

          <FormSection icon={Plug} title="上游连接" description="Provider 协议、地址与凭据。">
            <Field label="Base URL" className="sm:col-span-2">
              <input className={INPUT_BASE} value={formValues.baseUrl} placeholder="https://api.example.com/v1" onChange={(event) => handleFormValueChange('baseUrl', event.target.value)} />
            </Field>
            <ProtocolTypeInput value={formValues.protocolType} endpointPath={formValues.endpointPath} onValueChange={handleProtocolTypeChange} onEndpointPathChange={(value) => handleFormValueChange('endpointPath', value)} />
            <Field label="API Key" className="sm:col-span-2">
              <div className="relative flex items-center">
                <input className={`${INPUT_BASE} pr-10`} type={apiKeyVisible ? 'text' : 'password'} value={formValues.apiKey} placeholder="sk-..." onChange={(event) => handleFormValueChange('apiKey', event.target.value)} />
                <button type="button" className="absolute right-3 text-slate-400 hover:text-slate-700" onClick={() => setApiKeyVisible((visible) => !visible)} title={apiKeyVisible ? '隐藏' : '显示'}>
                  {apiKeyVisible ? <EyeOff size={18} /> : <Eye size={18} />}
                </button>
              </div>
            </Field>
            <ModelInput
              value={formValues.realModel}
              providerModels={providerModels}
              providerModelsUrl={providerModelsUrl}
              modelFetchError={modelFetchError}
              modelFetchLoading={modelFetchLoading}
              chatTesting={chatTesting}
              onValueChange={(value) => handleFormValueChange('realModel', value)}
              onFetchProviderModels={handleFetchProviderModels}
              onProviderModelSelect={(value) => value && handleFormValueChange('realModel', value)}
              onChatTest={handleChatTest}
            />
            <Field label="自定义请求头" className="sm:col-span-2">
              <div className="space-y-2">
                {formValues.customHeaders.length === 0 && (
                  <p className="text-xs text-slate-400">未配置自定义请求头，将只发送协议默认请求头。</p>
                )}
                {formValues.customHeaders.map((pair, pairIndex) => (
                  <div key={pairIndex} className="flex items-center gap-2">
                    <input
                      className={`${INPUT_BASE} font-mono`}
                      value={pair.key}
                      placeholder="Header-Name"
                      onChange={(event) => handleCustomHeaderChange(pairIndex, 'key', event.target.value)}
                    />
                    <input
                      className={INPUT_BASE}
                      value={pair.value}
                      placeholder="value"
                      onChange={(event) => handleCustomHeaderChange(pairIndex, 'value', event.target.value)}
                    />
                    <button
                      type="button"
                      className="shrink-0 rounded-lg border border-slate-200 px-2.5 py-2 text-slate-400 transition hover:border-rose-200 hover:text-rose-600"
                      onClick={() => handleRemoveCustomHeader(pairIndex)}
                      title="删除请求头"
                    >
                      ✕
                    </button>
                  </div>
                ))}
                <button
                  type="button"
                  className="inline-flex items-center gap-1.5 rounded-xl border border-dashed border-slate-300 px-3.5 py-2 text-sm font-medium text-slate-600 transition hover:border-indigo-300 hover:text-indigo-600"
                  onClick={handleAddCustomHeader}
                >
                  添加请求头
                </button>
              </div>
              <p className="mt-1.5 text-xs text-slate-400">
                自定义请求头会附加到发往上游 Provider 的请求中，可用于传递私有网关所需的鉴权或路由标识。
              </p>
            </Field>
          </FormSection>

          <FormSection icon={Gauge} title="运行参数" description="上下文窗口与保守使用比例。">
            <Field label="上下文窗口">
              <input className={`${INPUT_BASE} font-mono`} type="number" min="1" value={formValues.contextWindow} placeholder="例如 128000" onChange={(event) => handleFormValueChange('contextWindow', clampNumberInput(event.target.value))} />
              <p className="mt-1.5 text-xs text-slate-400">用于设置 Codex 识别的模型上下文容量。</p>
            </Field>
            <Field label="有效上下文比例">
              <input className={`${INPUT_BASE} font-mono`} type="number" min="1" max={100} value={formValues.effectiveContextWindowPercent} placeholder="1-100" onChange={(event) => handleFormValueChange('effectiveContextWindowPercent', clampNumberInput(event.target.value, 100))} />
              <p className="mt-1.5 text-xs text-slate-400">用于控制 Codex 实际使用上下文窗口的保守比例。</p>
            </Field>
          </FormSection>
        </div>

        <div className="shrink-0 border-t border-slate-200/80 bg-white px-5 py-4 sm:px-7 sm:py-5">
          {saveError && <div className="mb-3 rounded-xl bg-rose-50 px-4 py-3 text-sm leading-6 text-rose-700">{saveError}</div>}
          <div className="flex justify-end gap-3">
            <Button variant="secondary" onClick={handleModelDialogClose} disabled={saving}>取消</Button>
            <Button onClick={handleSave} disabled={saving}>{saving ? '保存中' : isCreateMode ? '保存新增' : '保存编辑'}</Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function FormSection({ icon: Icon, title, description, children }: { icon: typeof Tag; title: string; description: string; children: ReactNode }) {
  return (
    <section className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm">
      <div className="flex items-center gap-3 border-b border-slate-100 bg-slate-50/70 px-5 py-3.5 sm:px-6">
        <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-white text-indigo-600 shadow-sm ring-1 ring-slate-200">
          <Icon size={16} />
        </span>
        <div>
          <h4 className="text-sm font-bold text-slate-900">{title}</h4>
          <p className="text-xs leading-5 text-slate-500">{description}</p>
        </div>
      </div>
      <div className="grid grid-cols-1 gap-x-5 gap-y-4 px-5 py-5 sm:grid-cols-2 sm:px-6">
        {children}
      </div>
    </section>
  );
}

function Field({ label, className = '', children }: { label: string; className?: string; children: ReactNode }) {
  return (
    <label className={`block ${className}`}>
      <span className="mb-1.5 block text-sm font-semibold text-slate-700">{label}</span>
      {children}
    </label>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <button type="button" role="switch" aria-checked={checked} onClick={() => onChange(!checked)} className={`inline-flex items-center gap-2 rounded-xl border px-3 py-2 text-sm font-medium transition ${checked ? 'border-emerald-200 bg-emerald-50 text-emerald-700' : 'border-slate-200 bg-slate-50 text-slate-500'}`}>
      <span className={`relative inline-flex h-5 w-9 items-center rounded-full transition ${checked ? 'bg-emerald-500' : 'bg-slate-300'}`}>
        <span className={`absolute h-4 w-4 rounded-full bg-white shadow transition-all ${checked ? 'left-4' : 'left-0.5'}`} />
      </span>
      {checked ? '已启用' : '已禁用'}
    </button>
  );
}

function ProtocolTypeInput({
  value,
  endpointPath,
  onValueChange,
  onEndpointPathChange,
}: {
  value: string;
  endpointPath: string;
  onValueChange: (value: string) => void;
  onEndpointPathChange: (value: string) => void;
}) {
  const selectedProtocol = PROTOCOL_OPTIONS.find((option) => option.value === value) ?? PROTOCOL_OPTIONS[0];

  return (
    <fieldset className="block sm:col-span-2">
      <span className="mb-1.5 block text-sm font-semibold text-slate-700">API Protocol</span>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {PROTOCOL_OPTIONS.map((option) => {
          const active = option.value === selectedProtocol.value;
          return (
            <button key={option.value} type="button" onClick={() => onValueChange(option.value)} className={`flex items-center justify-center gap-1.5 rounded-xl border px-3 py-2.5 text-sm font-medium transition ${active ? 'border-indigo-300 bg-indigo-50 text-indigo-700 ring-2 ring-indigo-100' : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300'}`}>
              {active && <Check size={14} />}
              {option.label}
            </button>
          );
        })}
      </div>
      <div className="mt-3">
        <span className="mb-1.5 block text-xs font-semibold text-slate-500">Endpoint Path</span>
        <input className={`${INPUT_BASE} font-mono text-xs`} value={endpointPath || selectedProtocol.endpoint} placeholder={selectedProtocol.endpoint} onChange={(event) => onEndpointPathChange(event.target.value)} />
      </div>
    </fieldset>
  );
}

function ModelInput({
  value,
  providerModels,
  providerModelsUrl,
  modelFetchError,
  modelFetchLoading,
  chatTesting,
  onValueChange,
  onFetchProviderModels,
  onProviderModelSelect,
  onChatTest,
}: {
  value: string;
  providerModels: string[];
  providerModelsUrl: string;
  modelFetchError: string;
  modelFetchLoading: boolean;
  chatTesting: boolean;
  onValueChange: (value: string) => void;
  onFetchProviderModels: () => Promise<void>;
  onProviderModelSelect: (value: string) => void;
  onChatTest: () => void;
}) {
  return (
    <Field label="真实模型" className="sm:col-span-2">
      <div className="flex flex-col gap-2">
        <div className="flex gap-2">
          <input className={`${INPUT_BASE} min-w-0 flex-1`} value={value} onChange={(event) => onValueChange(event.target.value)} />
          <Button variant="secondary" onClick={onFetchProviderModels} disabled={modelFetchLoading}>{modelFetchLoading ? '获取中' : '获取模型'}</Button>
          <Button variant="secondary" onClick={onChatTest} disabled={chatTesting}>{chatTesting ? '测试中' : '模型测试'}</Button>
        </div>
        {providerModels.length > 0 && (
          <select className="w-full rounded-xl border border-indigo-100 bg-indigo-50 px-3.5 py-2.5 text-sm text-indigo-900 outline-none" value={providerModels.includes(value) ? value : MODEL_SELECT_PLACEHOLDER} onChange={(event) => onProviderModelSelect(event.target.value)}>
            <option value={MODEL_SELECT_PLACEHOLDER}>选择上游模型</option>
            {providerModels.map((modelId) => (
              <option key={modelId} value={modelId}>{modelId}</option>
            ))}
          </select>
        )}
        {providerModelsUrl && <div className="break-all text-xs text-slate-400">来源：{providerModelsUrl}</div>}
        {modelFetchError && <div className="break-all text-xs text-rose-600">{modelFetchError}</div>}
      </div>
    </Field>
  );
}

function resolveDefaultEndpointPath(protocolType: string) {
  return PROTOCOL_OPTIONS.find((option) => option.value === protocolType)?.endpoint ?? PROTOCOL_OPTIONS[0].endpoint;
}

function clampNumberInput(value: string, max?: number) {
  if (!value) return '';
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return value;
  const normalized = Math.max(1, Math.trunc(parsed));
  return String(typeof max === 'number' ? Math.min(max, normalized) : normalized);
}

function formatOptionalNumber(value: number | null | undefined, fallback: string) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? String(value) : fallback;
}

function parseModelMappings(value: string) {
  return [...new Set(value.split(',').map((item) => item.trim()).filter(Boolean))];
}

function parseInteger(value: string, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.trunc(parsed) : fallback;
}

export type ModelDialogOpenHandler = (mode: ModelDialogState['mode'], model?: ModelConfig) => void;
