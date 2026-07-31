import { useState } from 'react';
import type { ReactNode } from 'react';
import { Eye, EyeOff } from 'lucide-react';
import { Button } from '../ui/Button';
import type { ModelConfig } from '../../types';
import type { ModelDialogFormValues, ModelDialogState } from '../../lib/appTypes';
import { invokeFetchProviderModels } from '../../lib/tauriBridge';

const DEFAULT_BASE_URL = 'https://api.example.com/v1';
const DEFAULT_REAL_MODEL = 'gpt-5.5';
const DEFAULT_PROTOCOL_TYPE = 'cpamc';
const MODEL_SELECT_PLACEHOLDER = '';

const PROTOCOL_OPTIONS = [
  { value: 'cpamc', label: 'cpamc', endpoint: '/responses' },
  { value: 'openai', label: 'openai', endpoint: '/chat/completions' },
  { value: 'anthropic', label: 'anthropic', endpoint: '/messages' },
  { value: 'other', label: '其他', endpoint: '/chat/completions' },
];

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
      <div className="flex max-h-[calc(100vh-2rem)] w-full max-w-5xl flex-col overflow-hidden rounded-3xl border border-white/70 bg-slate-50 shadow-2xl shadow-slate-950/20 sm:max-h-[calc(100vh-3rem)] lg:max-h-[calc(100vh-4rem)]">
        <div className="flex shrink-0 items-start justify-between gap-4 border-b border-slate-200/80 bg-white px-5 py-4 sm:px-7 sm:py-5">
          <h3 className="text-xl font-bold text-slate-950">{isCreateMode ? '新增模型' : '编辑模型'}</h3>
          <Button variant="ghost" onClick={handleModelDialogClose} disabled={saving}>关闭</Button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 sm:px-7 sm:py-6">
          <div className="mx-auto max-w-4xl">
            <FormSection title="模型配置" description="按模块配置模型识别、上游连接、路由策略和运行参数。">
          <EditableInput label="显示名称" value={formValues.displayName} onValueChange={(value) => handleFormValueChange('displayName', value)} placeholder="例如：GPT5.5 中转" />
          {!isCreateMode && <ReadonlySlugInput value={formValues.slug} />}
          <EditableInput label="Base URL" value={formValues.baseUrl} onValueChange={(value) => handleFormValueChange('baseUrl', value)} />
          <ProtocolTypeInput value={formValues.protocolType} endpointPath={formValues.endpointPath} onValueChange={handleProtocolTypeChange} onEndpointPathChange={(value) => handleFormValueChange('endpointPath', value)} />
          <RoutingInput
            mappings={formValues.modelMappings}
            priority={formValues.priority}
            weight={formValues.weight}
            onMappingsChange={(value) => handleFormValueChange('modelMappings', value)}
            onPriorityChange={(value) => handleFormValueChange('priority', value)}
            onWeightChange={(value) => handleFormValueChange('weight', value)}
          />
          <ApiKeyInput value={formValues.apiKey} visible={apiKeyVisible} onToggleVisible={() => setApiKeyVisible((visible) => !visible)} onValueChange={(value) => handleFormValueChange('apiKey', value)} />
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
          <NumberInput label="上下文窗口" value={formValues.contextWindow} placeholder="例如 128000" description="用于设置 Codex 识别的模型上下文容量。" onValueChange={(value) => handleFormValueChange('contextWindow', value)} />
          <NumberInput label="有效上下文比例" value={formValues.effectiveContextWindowPercent} placeholder="1-100" description="用于控制 Codex 实际使用上下文窗口的保守比例。" max={100} onValueChange={(value) => handleFormValueChange('effectiveContextWindowPercent', value)} />
          <label className="block">
            <span className="mb-2 block text-sm font-semibold text-slate-700">启用状态</span>
            <select className="w-full rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700 outline-none" value={formValues.enabled ? 'enabled' : 'disabled'} onChange={(event) => handleFormValueChange('enabled', event.target.value === 'enabled')}>
              <option value="enabled">启用</option>
              <option value="disabled">禁用</option>
            </select>
          </label>
            </FormSection>
          </div>
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

function FormSection({ title, description, children }: { title: string; description: string; children: ReactNode }) {
  return (
    <section className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm">
      <div className="border-b border-slate-100 bg-slate-50/70 px-5 py-4 sm:px-6">
        <h4 className="text-sm font-bold text-slate-900">{title}</h4>
        <p className="mt-1 text-xs leading-5 text-slate-500">{description}</p>
      </div>
      <div className="grid grid-cols-1 gap-x-5 gap-y-4 px-5 py-5 sm:grid-cols-2 sm:px-6">
        {children}
      </div>
    </section>
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
    <fieldset className="block">
      <span className="mb-2 block text-sm font-semibold text-slate-700">API Protocol</span>
      <select className="w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-700 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100" value={selectedProtocol.value} onChange={(event) => onValueChange(event.target.value)}>
        {PROTOCOL_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>{option.label}</option>
        ))}
      </select>
      <label className="mt-2 block">
        <span className="mb-2 block text-xs font-semibold text-slate-500">Endpoint Path</span>
        <input className="w-full rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 font-mono text-xs text-slate-800 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100" value={endpointPath || selectedProtocol.endpoint} placeholder={selectedProtocol.endpoint} onChange={(event) => onEndpointPathChange(event.target.value)} />
      </label>
    </fieldset>
  );
}

function RoutingInput({ mappings, priority, weight, onMappingsChange, onPriorityChange, onWeightChange }: {
  mappings: string;
  priority: string;
  weight: string;
  onMappingsChange: (value: string) => void;
  onPriorityChange: (value: string) => void;
  onWeightChange: (value: string) => void;
}) {
  return (
    <fieldset className="block sm:col-span-2">
      <span className="mb-2 block text-sm font-semibold text-slate-700">渠道路由与负载均衡</span>
      <input className="w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-700 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100" value={mappings} placeholder="对外模型名，逗号分隔；相同名称会参与负载均衡" onChange={(event) => onMappingsChange(event.target.value)} />
      <div className="mt-2 grid grid-cols-2 gap-2">
        <label><span className="mb-1 block text-xs font-semibold text-slate-500">优先级（高者优先）</span><input className="w-full rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 font-mono text-sm text-slate-700 outline-none" type="number" value={priority} onChange={(event) => onPriorityChange(event.target.value)} /></label>
        <label><span className="mb-1 block text-xs font-semibold text-slate-500">权重（同优先级）</span><input className="w-full rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 font-mono text-sm text-slate-700 outline-none" type="number" min="1" value={weight} onChange={(event) => onWeightChange(event.target.value)} /></label>
      </div>
    </fieldset>
  );
}

function ApiKeyInput({ value, visible, onToggleVisible, onValueChange }: { value: string; visible: boolean; onToggleVisible: () => void; onValueChange: (value: string) => void }) {
  return (
    <label className="block">
      <span className="mb-2 block text-sm font-semibold text-slate-700">API Key</span>
      <div className="relative flex items-center">
        <input className="w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 pr-10 text-sm text-slate-700 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100" type={visible ? 'text' : 'password'} value={value} placeholder="sk-..." onChange={(event) => onValueChange(event.target.value)} />
        <button type="button" className="absolute right-3 text-slate-400 hover:text-slate-700" onClick={onToggleVisible} title={visible ? '隐藏' : '显示'}>
          {visible ? <EyeOff size={18} /> : <Eye size={18} />}
        </button>
      </div>
    </label>
  );
}

function EditableInput({ label, value, onValueChange, placeholder }: { label: string; value: string; onValueChange: (value: string) => void; placeholder?: string }) {
  return (
    <label className={label === 'Base URL' ? 'block sm:col-span-2' : 'block'}>
      <span className="mb-2 block text-sm font-semibold text-slate-700">{label}</span>
      <input className="w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-700 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100" value={value} placeholder={placeholder} onChange={(event) => onValueChange(event.target.value)} />
    </label>
  );
}

function NumberInput({ label, value, placeholder, description, max, onValueChange }: { label: string; value: string; placeholder: string; description?: string; max?: number; onValueChange: (value: string) => void }) {
  return (
    <label className="block">
      <span className="mb-2 block text-sm font-semibold text-slate-700">{label}</span>
      <input className="w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 font-mono text-sm text-slate-700 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100" type="number" min="1" max={max} step="1" value={value} placeholder={placeholder} onChange={(event) => onValueChange(clampNumberInput(event.target.value, max))} />
      {description && <div className="mt-2 text-xs leading-5 text-slate-400">{description}</div>}
    </label>
  );
}

function ReadonlySlugInput({ value }: { value: string }) {
  return (
    <label className="block">
      <span className="mb-2 block text-sm font-semibold text-slate-700">Slug</span>
      <input className="w-full cursor-not-allowed rounded-2xl border border-slate-200 bg-slate-100 px-4 py-3 font-mono text-sm text-slate-500 outline-none" value={value || '保存时自动生成'} readOnly />
      <div className="mt-2 text-xs text-slate-400">Slug 创建后固定，不支持修改。</div>
    </label>
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
    <label className="block">
      <span className="mb-2 block text-sm font-semibold text-slate-700">真实模型</span>
      <div className="flex gap-2">
        <input className="min-w-0 flex-1 rounded-2xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-700 outline-none transition focus:border-indigo-400 focus:ring-4 focus:ring-indigo-100" value={value} onChange={(event) => onValueChange(event.target.value)} />
        <Button variant="secondary" onClick={onFetchProviderModels} disabled={modelFetchLoading}>{modelFetchLoading ? '获取中' : '获取模型'}</Button>
        <Button variant="secondary" onClick={onChatTest} disabled={chatTesting}>{chatTesting ? '测试中' : '模型测试'}</Button>
      </div>
      {providerModels.length > 0 && (
        <select className="mt-2 w-full rounded-2xl border border-indigo-100 bg-indigo-50 px-4 py-3 text-sm text-indigo-900 outline-none" value={providerModels.includes(value) ? value : MODEL_SELECT_PLACEHOLDER} onChange={(event) => onProviderModelSelect(event.target.value)}>
          <option value={MODEL_SELECT_PLACEHOLDER}>选择上游模型</option>
          {providerModels.map((modelId) => (
            <option key={modelId} value={modelId}>{modelId}</option>
          ))}
        </select>
      )}
      {providerModelsUrl && <div className="mt-2 break-all text-xs text-slate-400">来源：{providerModelsUrl}</div>}
      {modelFetchError && <div className="mt-2 break-all text-xs text-rose-600">{modelFetchError}</div>}
    </label>
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
