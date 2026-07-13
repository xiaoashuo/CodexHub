import { useEffect, useMemo, useState } from 'react';
import { Badge } from '../../components/ui/Badge';
import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import { invokeLoadMcpServers, invokeRemoveMcpServer, invokeSetMcpServerEnabled, invokeUpsertMcpServer } from '../../lib/tauriBridge';
import type { McpServerSummary, McpTransport, UpsertMcpServerRequest } from '../../types';

const emptyForm: UpsertMcpServerRequest = {
  name: '',
  transport: 'stdio',
  enabled: true,
  command: '',
  args: [],
  url: '',
  headers: {},
  environment: {},
};

export function McpManagementPage() {
  const [servers, setServers] = useState<McpServerSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');
  const [editing, setEditing] = useState<McpServerSummary | null>(null);
  const enabledCount = useMemo(() => servers.filter((server) => server.enabled).length, [servers]);

  const refresh = async () => {
    setLoading(true);
    try {
      const result = await invokeLoadMcpServers();
      setServers(result.items);
      setMessage('');
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const handleToggle = async (server: McpServerSummary) => {
    try {
      await invokeSetMcpServerEnabled(server.name, !server.enabled);
      await refresh();
    } catch (error) {
      setMessage(formatError(error));
    }
  };

  const handleRemove = async (server: McpServerSummary) => {
    if (!window.confirm(`确定删除 MCP 服务 ${server.name}？`)) {
      return;
    }
    try {
      const result = await invokeRemoveMcpServer(server.name);
      setServers(result.items);
      setMessage(`已删除 ${server.name}`);
    } catch (error) {
      setMessage(formatError(error));
    }
  };

  const handleSave = async (request: UpsertMcpServerRequest) => {
    setSaving(true);
    try {
      await invokeUpsertMcpServer(request);
      setEditing(null);
      await refresh();
      setMessage(`已保存 ${request.name}`);
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <Card className="min-h-0 flex-1 overflow-hidden">
        <CardHeader className="py-4">
          <div className="flex items-center justify-between gap-4">
            <div className="text-sm text-slate-600">
            服务数 <span className="font-semibold text-slate-950">{servers.length}</span>，已启用 <span className="font-semibold text-slate-950">{enabledCount}</span>
            </div>
            <div className="ml-auto flex shrink-0 gap-2">
              <Button variant="secondary" onClick={refresh} disabled={loading}>刷新</Button>
              <Button onClick={() => setEditing({ ...emptyForm, sourcePath: '' } as McpServerSummary)}>添加</Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="h-full overflow-y-auto">
          {message && <div className="mb-4 rounded-xl bg-slate-50 px-4 py-3 text-sm text-slate-600">{message}</div>}
          {loading ? (
            <div className="py-12 text-center text-sm text-slate-400">正在加载 MCP 服务...</div>
          ) : servers.length === 0 ? (
            <div className="py-12 text-center text-sm text-slate-400">还没有配置 MCP 服务。</div>
          ) : (
            <div className="overflow-hidden rounded-2xl border border-slate-200">
              {servers.map((server) => (
                <div key={server.name} className="flex items-center justify-between gap-4 border-b border-slate-100 px-5 py-4 last:border-b-0">
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-semibold text-slate-950">{server.name}</span>
                      <Badge tone="blue">{server.transport}</Badge>
                      <Badge tone={server.enabled ? 'green' : 'rose'}>{server.enabled ? '已启用' : '已禁用'}</Badge>
                    </div>
                    <div className="mt-2 truncate text-sm text-slate-500">
                      {formatMcpEndpoint(server)}
                    </div>
                  </div>
                  <div className="flex shrink-0 gap-2">
                    <Button variant="secondary" onClick={() => handleToggle(server)}>{server.enabled ? '禁用' : '启用'}</Button>
                    <Button variant="secondary" onClick={() => setEditing(server)}>编辑</Button>
                    <Button variant="danger" onClick={() => handleRemove(server)}>删除</Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {editing && (
        <McpDialog
          server={editing.name ? editing : null}
          saving={saving}
          handleClose={() => setEditing(null)}
          handleSave={handleSave}
        />
      )}
    </div>
  );
}

function McpDialog({
  server,
  saving,
  handleClose,
  handleSave,
}: {
  server: McpServerSummary | null;
  saving: boolean;
  handleClose: () => void;
  handleSave: (request: UpsertMcpServerRequest) => Promise<void>;
}) {
  const [name, setName] = useState(server?.name ?? '');
  const [transport, setTransport] = useState<McpTransport>(server?.transport ?? 'stdio');
  const [enabled, setEnabled] = useState(server?.enabled ?? true);
  const [command, setCommand] = useState(stripWrappingQuotes(server?.command ?? ''));
  const [args, setArgs] = useState((server?.args ?? []).map(stripWrappingQuotes).join(', '));
  const [url, setUrl] = useState(stripWrappingQuotes(server?.url ?? ''));
  const [envText, setEnvText] = useState(formatKeyValue(server?.environment, '='));
  const [headersText, setHeadersText] = useState(formatKeyValue(server?.headers, ':'));

  const submit = async () => {
    await handleSave({
      name,
      transport,
      enabled,
      command: stripWrappingQuotes(command),
      args: args.split(',').map((item) => stripWrappingQuotes(item)).filter(Boolean),
      url: stripWrappingQuotes(url),
      environment: parseKeyValue(envText, '='),
      headers: parseKeyValue(headersText, ':'),
    });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-2xl rounded-2xl bg-white p-6 shadow-2xl">
        <h3 className="text-xl font-bold text-slate-950">{server ? '编辑 MCP' : '添加 MCP'}</h3>
        <div className="mt-5 grid gap-4">
          <Input label="名称" value={name} onChange={setName} disabled={!!server} />
          <label className="grid gap-1 text-sm font-medium text-slate-700">
            Transport
            <select className="rounded-xl border border-slate-200 px-3 py-2" value={transport} onChange={(event) => setTransport(event.target.value as McpTransport)}>
              <option value="stdio">stdio</option>
              <option value="http">http</option>
              <option value="sse">sse</option>
            </select>
          </label>
          <label className="flex items-center gap-2 text-sm text-slate-700">
            <input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />
            启用
          </label>
          {transport === 'stdio' ? (
            <>
              <Input label="Command" value={command ?? ''} onChange={setCommand} />
              <Input label="Args（逗号分隔）" value={args} onChange={setArgs} />
            </>
          ) : (
            <Input label="URL" value={url ?? ''} onChange={setUrl} />
          )}
          <Textarea label="环境变量（每行 KEY=value）" value={envText} onChange={setEnvText} />
          <Textarea label="Headers（每行 Key: Value）" value={headersText} onChange={setHeadersText} />
        </div>
        <div className="mt-6 flex justify-end gap-2">
          <Button variant="secondary" onClick={handleClose} disabled={saving}>取消</Button>
          <Button onClick={submit} disabled={saving}>{saving ? '保存中...' : '保存'}</Button>
        </div>
      </div>
    </div>
  );
}

function Input({ label, value, onChange, disabled = false }: { label: string; value: string; onChange: (value: string) => void; disabled?: boolean }) {
  return (
    <label className="grid gap-1 text-sm font-medium text-slate-700">
      {label}
      <input className="rounded-xl border border-slate-200 px-3 py-2 disabled:bg-slate-50" value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}

function Textarea({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label className="grid gap-1 text-sm font-medium text-slate-700">
      {label}
      <textarea className="min-h-20 rounded-xl border border-slate-200 px-3 py-2 font-mono text-sm" value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}

function parseKeyValue(text: string, separator: '=' | ':') {
  return Object.fromEntries(text.split('\n').map((line) => {
    const index = line.indexOf(separator);
    return index > 0 ? [line.slice(0, index).trim(), line.slice(index + 1).trim()] : ['', ''];
  }).filter(([key]) => key));
}

function formatKeyValue(value: Record<string, string> | undefined, separator: '=' | ':') {
  return Object.entries(value ?? {}).map(([key, item]) => `${key}${separator} ${item}`).join('\n');
}

function formatMcpEndpoint(server: McpServerSummary) {
  if (server.transport !== 'stdio') {
    return stripWrappingQuotes(server.url ?? '');
  }

  return [server.command, ...server.args]
    .filter(Boolean)
    .map((value) => stripWrappingQuotes(value ?? ''))
    .join(' ');
}

function stripWrappingQuotes(value: string) {
  let result = value.trim();
  while (result.length >= 2) {
    const first = result[0];
    const last = result[result.length - 1];
    if ((first === '"' && last === '"') || (first === '\'' && last === '\'') || (first === '`' && last === '`')) {
      result = result.slice(1, -1).trim();
      continue;
    }
    break;
  }
  return result;
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
