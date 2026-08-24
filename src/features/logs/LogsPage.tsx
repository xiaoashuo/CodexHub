import { useMemo, useState, type ReactNode } from 'react';
import { Search, Trash2, RefreshCw, Server, Box, CalendarDays, ShieldCheck, ChevronRight, X, ArrowUp, ArrowDown } from 'lucide-react';
import { Button } from '../../components/ui/Button';
import type { AppOperationLogEntry, ModelConfig, RouterLogEntry } from '../../types';

type LogsTab = 'router' | 'app';

const PAGE_SIZE = 8;

interface AuditRow {
  id: string;
  displayNo: number;
  time: string;
  upstream: string;
  model: string;
  status: string;
  secure: boolean;
  token: number;
  duration: string;
  raw: RouterLogEntry;
}

export function LogsPage({
  routerLogs,
  models,
  appOperationLogs,
  handleRouterLogsRefresh,
  handleRouterLogsClear,
  handleAppLogsSearch,
  handleAppLogsClear,
}: {
  routerLogs: RouterLogEntry[];
  models: ModelConfig[];
  appOperationLogs: AppOperationLogEntry[];
  handleRouterLogsRefresh: () => Promise<void>;
  handleRouterLogsClear: () => Promise<void>;
  handleAppLogsSearch: (keyword: string, level: AppOperationLogEntry['level'] | 'all') => Promise<void>;
  handleAppLogsClear: () => Promise<void>;
}) {
  const [tab, setTab] = useState<LogsTab>('router');
  const [filterOpen, setFilterOpen] = useState(false);
  const [clearModalOpen, setClearModalOpen] = useState(false);

  const handleRefresh = async () => {
    if (tab === 'app') {
      await handleAppLogsSearch('', 'all');
    } else {
      await handleRouterLogsRefresh();
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-[#f5f7fb]">
      <style>{`
        .audit-scroll::-webkit-scrollbar{width:6px;height:6px}
        .audit-scroll::-webkit-scrollbar-thumb{background:#d8dde6;border-radius:999px}
        .audit-scroll::-webkit-scrollbar-track{background:transparent}
      `}</style>

      <div className="flex shrink-0 flex-wrap items-start justify-between gap-4 px-1 pb-4">
        <div>
          <h2 className="text-[28px] font-bold leading-tight text-[#0f172a]">审计日志</h2>
          <p className="mt-1 text-sm text-[#64748b]">查看请求结果、Token 消耗、工具调用与网关路由详情</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className="flex h-10 w-11 items-center justify-center rounded-[11px] border border-[#e2e8f0] bg-white text-[#0f172a] transition hover:bg-slate-50"
            title="搜索"
            onClick={() => setFilterOpen((open) => !open)}
          >
            <Search size={18} />
          </button>
          <Button variant="secondary" onClick={() => setClearModalOpen(true)} className="h-10 gap-1.5 rounded-[11px] border border-[#e2e8f0] bg-white px-4 text-[#0f172a] hover:bg-slate-50">
            <Trash2 size={16} />
            清理
          </Button>
          <Button variant="secondary" onClick={() => void handleRefresh()} className="h-10 gap-1.5 rounded-[11px] border border-[#e2e8f0] bg-white px-4 text-[#0f172a] hover:bg-slate-50">
            <RefreshCw size={16} />
            刷新
          </Button>
        </div>
      </div>

      <div className="mb-3 flex shrink-0 gap-1 rounded-2xl bg-slate-100 p-1">
        <TabButton active={tab === 'router'} onClick={() => setTab('router')}>审计日志</TabButton>
        <TabButton active={tab === 'app'} onClick={() => setTab('app')}>应用日志</TabButton>
      </div>

      {tab === 'router' ? (
        <RouterAuditLog logs={routerLogs} models={models} onRefresh={handleRouterLogsRefresh} onClear={handleRouterLogsClear} filterOpen={filterOpen} setFilterOpen={setFilterOpen} />
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto pb-2">
          <AppLogsPanel logs={appOperationLogs} handleAppLogsSearch={handleAppLogsSearch} handleAppLogsClear={handleAppLogsClear} filterOpen={filterOpen} setFilterOpen={setFilterOpen} />
        </div>
      )}

      <ClearLogModal
        open={clearModalOpen}
        onClose={() => setClearModalOpen(false)}
        onConfirm={(scope) => {
          void handleRouterLogsClear();
          setClearModalOpen(false);
        }}
      />
    </div>
  );
}

function ClearLogModal({ open, onClose, onConfirm }: { open: boolean; onClose: () => void; onConfirm: (scope: '7d' | '30d' | 'all') => void }) {
  if (!open) return null;
  const options: { scope: '7d' | '30d' | 'all'; label: string }[] = [
    { scope: '7d', label: '清理 7 天前的日志' },
    { scope: '30d', label: '清理 30 天前的日志' },
    { scope: 'all', label: '清理全部日志' },
  ];
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 px-4" onClick={onClose}>
      <div className="w-[420px] max-w-full rounded-2xl bg-white p-6 shadow-xl" onClick={(e) => e.stopPropagation()}>
        <h3 className="text-lg font-bold text-slate-950">清理日志</h3>
        <p className="mt-1 text-sm text-slate-500">选择要清理的日志范围，此操作不可撤销</p>

        <div className="mt-5 flex flex-col gap-2">
          {options.map((opt) => (
            <button
              key={opt.scope}
              type="button"
              onClick={() => onConfirm(opt.scope)}
              className="flex h-11 items-center rounded-xl border border-slate-200 bg-white px-4 text-left text-sm font-medium text-slate-700 transition hover:border-rose-300 hover:bg-rose-50 hover:text-rose-600"
            >
              {opt.label}
            </button>
          ))}
        </div>

        <div className="mt-5 flex justify-end">
          <Button variant="ghost" onClick={onClose}>取消</Button>
        </div>
      </div>
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-xl px-4 py-2 text-sm font-semibold transition ${active ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-800'}`}
    >
      {children}
    </button>
  );
}

function RouterAuditLog({
  logs,
  models,
  onRefresh,
  onClear,
  filterOpen,
  setFilterOpen,
}: {
  logs: RouterLogEntry[];
  models: ModelConfig[];
  onRefresh: () => Promise<void>;
  onClear: () => Promise<void>;
  filterOpen: boolean;
  setFilterOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
}) {
  const [keyword, setKeyword] = useState('');
  const [channel, setChannel] = useState('');
  const [model, setModel] = useState('');
  const [source, setSource] = useState('全部来源');
  const [traceId, setTraceId] = useState('');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [deletedIds, setDeletedIds] = useState<Set<string>>(new Set());
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [page, setPage] = useState(1);

  const channelModelMap = useMemo(() => {
    const map: Record<string, string> = {};
    for (const item of models) {
      if (item.slug) {
        map[item.slug] = item.realModel || item.slug;
      }
    }
    return map;
  }, [models]);

  const rows = useMemo<AuditRow[]>(() => {
    const list = logs
      .map((log, index) => ({
        id: `${log.time}|${log.path}|${log.status}|${log.target_provider}|${index}`,
        time: formatLogTime(log.time),
        upstream: log.target_provider,
        model: log.target_provider ? (channelModelMap[log.target_provider] ?? log.target_provider) : '-',
        status: log.status,
        secure: true,
        token: log.total_tokens ?? (log.input_tokens ?? 0) + (log.output_tokens ?? 0),
        duration: log.cost,
        raw: log,
      }))
      .filter((row) => !deletedIds.has(row.id));

    const kw = keyword.trim().toLowerCase();
    const ch = channel.trim().toLowerCase();
    const src = source === '全部来源' ? '' : source.trim().toLowerCase();
    const ti = traceId.trim().toLowerCase();

    return list
      .filter((row) => {
        if (kw && !`${row.time} ${row.raw.path} ${row.upstream} ${row.model} ${row.status}`.toLowerCase().includes(kw)) return false;
        if (ch && !row.upstream.toLowerCase().includes(ch)) return false;
        if (model.trim() && !row.model.includes(model.trim())) return false;
        if (src && !row.upstream.toLowerCase().includes(src)) return false;
        if (ti && !row.raw.path.toLowerCase().includes(ti)) return false;
        if (startDate && row.time < startDate.replace(/\//g, '-')) return false;
        if (endDate && row.time > endDate.replace(/\//g, '-')) return false;
        return true;
      })
      .map((row, index) => ({ ...row, displayNo: list.length - index }));
  }, [logs, deletedIds, keyword, channel, model, source, traceId, startDate, endDate, channelModelMap]);

  const totalPages = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const safePage = Math.min(page, totalPages);
  const pageRows = rows.slice((safePage - 1) * PAGE_SIZE, safePage * PAGE_SIZE);

  const handleDelete = (id: string) => {
    setDeletedIds((current) => {
      const next = new Set(current);
      next.add(id);
      return next;
    });
  };

  const resetFilters = () => {
    setKeyword('');
    setChannel('');
    setModel('');
    setSource('全部来源');
    setTraceId('');
    setStartDate('');
    setEndDate('');
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-[18px] overflow-hidden">
      {filterOpen && (
        <div className="shrink-0 rounded-[17px] border border-[#e2e8f0] bg-white p-4 shadow-sm">
          <div className="mb-3 flex items-center justify-between">
            <div className="flex items-center gap-2 text-sm font-semibold text-[#0f172a]">
              <Search size={16} className="text-[#64748b]" />
              搜索筛选
            </div>
            <button type="button" className="text-[#94a3b8] transition hover:text-[#64748b]" onClick={() => setFilterOpen(false)} title="关闭">
              <X size={16} />
            </button>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <FilterInput icon={<Search size={15} className="text-[#94a3b8]" />} value={keyword} onChange={setKeyword} placeholder="关键词搜索（渠道/模型/Trace）" />
            <FilterInput icon={<Server size={15} className="text-[#94a3b8]" />} value={channel} onChange={setChannel} placeholder="渠道名称" />
            <FilterInput icon={<Box size={15} className="text-[#94a3b8]" />} value={model} onChange={setModel} placeholder="模型名称" />
            <SelectInput icon={<Server size={15} className="text-[#94a3b8]" />} value={source} onChange={setSource} options={[{ label: '全部来源', value: '全部来源' }, { label: 'grok公益', value: 'grok公益' }, { label: '小米公益', value: '小米公益' }, { label: 'cmy', value: 'cmy' }, { label: 'local', value: 'local' }]} />
            <FilterInput icon={<Search size={15} className="text-[#94a3b8]" />} value={traceId} onChange={setTraceId} placeholder="Trace ID" />
            <DateInput value={startDate} onChange={setStartDate} placeholder="yyyy/mm/日" />
            <DateInput value={endDate} onChange={setEndDate} placeholder="yyyy/mm/日" />
          </div>
          <div className="mt-3 flex justify-end">
            <Button variant="ghost" onClick={resetFilters}>重置筛选</Button>
          </div>
        </div>
      )}

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[20px] border border-[#e2e8f0] bg-white shadow-sm">
        <div className="audit-scroll min-h-0 flex-1 overflow-auto">
          <table className="w-full border-collapse text-left">
            <thead className="sticky top-0 z-10 bg-[#fafafa]">
              <tr className="text-[13px] font-semibold text-[#64748b]">
                <th className="w-10 px-3" />
                <th className="px-3 py-3 font-semibold">#</th>
                <th className="px-3 py-3 font-semibold">时间</th>
                <th className="px-3 py-3 font-semibold">上游</th>
                <th className="px-3 py-3 font-semibold">模型</th>
                <th className="px-3 py-3 font-semibold">状态</th>
                <th className="whitespace-nowrap px-3 py-3 font-semibold">安全</th>
                <th className="px-3 py-3 font-semibold">Token</th>
                <th className="px-3 py-3 font-semibold">耗时</th>
                <th className="px-3 py-3 font-semibold">
                  <span className="inline-flex items-center gap-1 whitespace-nowrap"><ChevronRight size={13} />更多</span>
                </th>
                <th className="w-12 px-3" />
              </tr>
            </thead>
            <tbody>
              {pageRows.map((row) => (
                <RowFragment
                  key={row.id}
                  row={row}
                  expanded={expandedId === row.id}
                  onToggle={() => setExpandedId((current) => (current === row.id ? null : row.id))}
                  onDelete={() => handleDelete(row.id)}
                />
              ))}
              {pageRows.length === 0 && (
                <tr>
                  <td colSpan={11} className="px-3 py-16 text-center text-sm text-[#94a3b8]">暂无审计日志</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <div className="flex h-[46px] shrink-0 items-center justify-between border-t border-[#eef2f7] px-4">
          <button
            type="button"
            disabled={safePage <= 1}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            className="rounded-[12px] border border-[#e2e8f0] bg-white px-3 py-1.5 text-sm text-[#94a3b8] transition hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-60"
          >
            上一页
          </button>
          <span className="text-sm text-[#64748b]">第 {safePage} 页</span>
          <button
            type="button"
            disabled={safePage >= totalPages}
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            className="rounded-[12px] border border-[#e2e8f0] bg-white px-3 py-1.5 text-sm text-[#0f172a] transition hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-60"
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}

function RowFragment({ row, expanded, onToggle, onDelete }: { row: AuditRow; expanded: boolean; onToggle: () => void; onDelete: () => void }) {
  return (
    <>
      <tr className="group h-[58px] border-b border-[#eef2f7] text-[13px] text-[#334155] transition hover:bg-[#f8fafc]">
        <td className="px-3">
          <button type="button" className="text-[#64748b] transition hover:text-[#0f172a]" onClick={onToggle}>
            <ChevronRight size={16} className={`transition-transform ${expanded ? 'rotate-90' : ''}`} />
          </button>
        </td>
        <td className="px-3 font-mono text-[#94a3b8]">#{row.displayNo}</td>
        <td className="px-3 whitespace-nowrap">{row.time}</td>
        <td className="px-3">
          <div className="flex flex-col gap-1">
            <span className="whitespace-nowrap">{row.upstream}</span>
            <ApiBadge />
          </div>
        </td>
        <td className="px-3 max-w-[140px] truncate" title={row.model}>{row.model}</td>
        <td className="px-3"><StatusBadge status={row.status} /></td>
        <td className="px-3"><SecurityBadge /></td>
        <td className={`px-3 font-mono tabular-nums ${row.token === 0 ? 'text-[#94a3b8]' : 'text-[#334155]'}`}>{row.token}</td>
        <td className="px-3 font-mono whitespace-nowrap text-[#64748b]">{row.duration}</td>
        <td className="px-3">
          <button type="button" className="inline-flex items-center gap-1 whitespace-nowrap text-[#94a3b8] transition hover:text-[#0f172a]" onClick={onToggle}>
            <ChevronRight size={13} />
            更多
          </button>
        </td>
        <td className="px-3">
          <button type="button" className="text-[#cbd5e1] transition hover:text-[#ef4444]" onClick={onDelete} title="删除">
            <Trash2 size={16} />
          </button>
        </td>
      </tr>
      {expanded && (
        <tr className="border-b border-[#eef2f7] bg-[#fafafa]">
          <td colSpan={11} className="px-6 py-4">
            <AuditDetail row={row} />
          </td>
        </tr>
      )}
    </>
  );
}

function AuditDetail({ row }: { row: AuditRow }) {
  return <RouterLogDetail row={row} />;
}

type LogDetailTab = 'request' | 'response';

function RouterLogDetail({ row }: { row: AuditRow }) {
  const [tab, setTab] = useState<LogDetailTab>('request');
  const [showRaw, setShowRaw] = useState(false);
  const raw = row.raw;
  const input = raw.input_tokens ?? 0;
  const output = raw.output_tokens ?? 0;
  const cached = raw.cached_input_tokens ?? 0;
  const total = row.token;
  const isStream = raw.stream ?? false;

  const requestBody = raw.request_body ?? '';
  const responseBody = raw.response_body ?? '';

  const requestBytes = byteLength(requestBody);
  const requestChars = [...requestBody].length;
  const cost = (input * 1.25 + output * 10 + cached * 0.125) / 1_000_000;

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <div className="rounded-2xl border border-[#eef2f7] bg-white px-4 py-3">
          <div className="text-xs font-medium text-[#94a3b8]">Token 消耗</div>
          <div className="mt-1 text-2xl font-bold text-[#0f172a]">{formatTokenAmount(total)}</div>
          <div className="mt-1 text-xs text-[#64748b]">
            输入 {formatTokenAmount(input)} <span className="px-1 text-[#cbd5e1]">|</span> 输出 {formatTokenAmount(output)}
            {cached > 0 && (
              <span className="text-[#94a3b8]"> <span className="px-1 text-[#cbd5e1]">|</span> 缓存 {formatTokenAmount(cached)}</span>
            )}
          </div>
        </div>

        <div className="rounded-2xl border border-[#eef2f7] bg-white px-4 py-3">
          <div className="text-xs font-medium text-[#94a3b8]">请求大小</div>
          <div className="mt-1 text-2xl font-bold text-[#0f172a]">{formatSize(requestBytes)}</div>
          <div className="mt-1 text-xs text-[#64748b]">{requestChars.toLocaleString('zh-CN')} 字符</div>
        </div>

        <div className="rounded-2xl border border-[#eef2f7] bg-white px-4 py-3">
          <div className="text-xs font-medium text-[#94a3b8]">请求耗时</div>
          <div className="mt-1 text-2xl font-bold text-[#0f172a]">{row.duration}</div>
          <div className="mt-1 text-xs text-[#64748b]">{isStream ? '流式' : '非流式'}{raw.usage_source ? <span><span className="px-1 text-[#cbd5e1]">|</span>用量来源 {raw.usage_source}</span> : null}</div>
        </div>

        <div className="rounded-2xl border border-[#eef2f7] bg-white px-4 py-3">
          <div className="text-xs font-medium text-[#94a3b8]">成本估算</div>
          <div className="mt-1 text-2xl font-bold text-[#0f172a]">${cost.toFixed(4)}</div>
          <div className="mt-1 text-xs text-[#64748b]">参考 GPT-5 定价</div>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-x-8 gap-y-2 rounded-2xl border border-[#eef2f7] bg-white px-4 py-3 sm:grid-cols-2">
        <DetailLine label="请求地址" value={raw.path} />
        <DetailLine label="方法" value={raw.method} />
        <DetailLine label="上游渠道" value={raw.target_provider} />
        <DetailLine label="来源 IP" value={raw.source_ip} />
      </div>

      <div className="overflow-hidden rounded-2xl border border-[#eef2f7] bg-white">
        <div className="flex flex-wrap items-center gap-1 border-b border-[#eef2f7] px-2 py-2">
          <DetailTabButton active={tab === 'request'} onClick={() => setTab('request')}>
            <ArrowUp size={13} /> 请求
          </DetailTabButton>
          <DetailTabButton active={tab === 'response'} onClick={() => setTab('response')}>
            <ArrowDown size={13} /> 响应
          </DetailTabButton>

          {tab === 'request' && (
            <button
              type="button"
              onClick={() => setShowRaw((current) => !current)}
              className="ml-auto rounded-[11px] border border-[#e2e8f0] bg-white px-3 py-1.5 text-sm font-medium text-[#0f172a] transition hover:bg-slate-50"
            >
              {showRaw ? '收起原始 JSON' : '查看原始 JSON'}
            </button>
          )}
        </div>

        <div className="px-4 py-3">
          {tab === 'request' && (
            <div className="flex flex-col gap-3">
              {showRaw ? <BodyBlock title="请求体" body={requestBody} /> : <RequestBodySummary body={requestBody} />}
            </div>
          )}

          {tab === 'response' && (
            <BodyBlock title="响应体" body={responseBody} />
          )}
        </div>
      </div>
    </div>
  );
}

function RequestBodySummary({ body }: { body: string }) {
  const summary = useMemo(() => {
    const EMPTY_HINT = '-';
    if (!body || body === EMPTY_HINT) {
      return null;
    }

    const asStr = (value: unknown): string => (typeof value === 'string' ? value : '');
    const asArr = (value: unknown): unknown[] => (Array.isArray(value) ? value : []);
    const countMatches = (source: string, re: RegExp): number => (source.match(re) || []).length;

    const computeFromParsed = (parsed: Record<string, unknown>) => {
      const model = asStr(parsed.model) || EMPTY_HINT;
      const stream = parsed.stream === true ? '是' : parsed.stream === false ? '否' : EMPTY_HINT;
      const maxOut = typeof parsed.max_output_tokens === 'number' ? String(parsed.max_output_tokens) : EMPTY_HINT;
      const temperature = typeof parsed.temperature === 'number' ? String(parsed.temperature) : EMPTY_HINT;
      const incremental = asStr(parsed.previous_response_id) ? '增量' : '全量';

      const input = asArr(parsed.input);
      const messageCount = input.filter((item) => {
        const type = asStr((item as Record<string, unknown>)?.type);
        return type !== 'function_call' && type !== 'function_call_output' && type !== 'custom_tool_call' && type !== 'custom_tool_call_output' && type !== 'item_reference';
      }).length;
      const hasToolCall = input.some((item) => {
        const type = asStr((item as Record<string, unknown>)?.type);
        return type === 'function_call' || type === 'custom_tool_call';
      });

      const instructions = asStr(parsed.instructions);
      const tools = asArr(parsed.tools).map((tool) => {
        const t = tool as Record<string, unknown>;
        const type = asStr(t.type) || 'function';
        const name = asStr(t.name) || asStr((t.function as Record<string, unknown>)?.name) || EMPTY_HINT;
        return type === 'namespace' ? `namespace(${name})` : name;
      });
      const toolText = tools.length ? `${tools.length} 个 · ${tools.join(', ')}` : '无';

      return { model, stream, maxOut, temperature, incremental, messageCount, hasToolCall, inputChars: [...body].length, instructions, toolText, partial: false };
    };

    let parsed: Record<string, unknown> | null = null;
    try {
      parsed = JSON.parse(body) as Record<string, unknown>;
    } catch {
      parsed = null;
    }
    if (parsed && typeof parsed === 'object') {
      return computeFromParsed(parsed);
    }

    const pick = (re: RegExp): string | null => {
      const m = body.match(re);
      return m ? (m[1] ?? null) : null;
    };
    const model = pick(/"model"\s*:\s*"([^"]*)"/) ?? EMPTY_HINT;
    const streamRaw = pick(/"stream"\s*:\s*(true|false)/);
    const stream = streamRaw === 'true' ? '是' : streamRaw === 'false' ? '否' : EMPTY_HINT;
    const maxOut = pick(/"max_output_tokens"\s*:\s*(\d+)/) ?? EMPTY_HINT;
    const temperature = pick(/"temperature"\s*:\s*([\d.]+)/) ?? EMPTY_HINT;
    const incremental = pick(/"previous_response_id"\s*:\s*"([^"]*)"/) ? '增量' : '全量';
    const messageCount = countMatches(body, /"type"\s*:\s*"message"/g) || 0;
    const hasToolCall = /"type"\s*:\s*"(function_call|custom_tool_call)"/.test(body);
    const instructions = /"instructions"\s*:\s*"/.test(body) ? '（存在）' : '无';
    const toolDefs = countMatches(body, /"type"\s*:\s*"(function|custom|namespace)"/g);
    const toolText = toolDefs ? `${toolDefs} 个（部分解析）` : '无';

    return { model, stream, maxOut, temperature, incremental, messageCount, hasToolCall, inputChars: [...body].length, instructions, toolText, partial: true };
  }, [body]);

  if (!summary) {
    return <div className="rounded-2xl border border-[#eef2f7] bg-white px-4 py-3 text-sm text-[#94a3b8]">无请求体</div>;
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <SummaryCard label="会话模式" value={summary.incremental} />
        <SummaryCard label="消息轮数" value={String(summary.messageCount)} />
        <SummaryCard label="工具调用" value={summary.hasToolCall ? '有' : '无'} />
        <SummaryCard label="输入规模" value={`${summary.inputChars} 字符`} />
      </div>

      {summary.partial && (
        <div className="rounded-2xl border border-amber-200 bg-amber-50 px-4 py-2 text-xs text-amber-700">
          请求体被截断或非标准格式，以下为尽力解析的摘要；点「查看原始 JSON」可看完整内容。
        </div>
      )}

      <div className="grid grid-cols-1 gap-x-8 gap-y-2 rounded-2xl border border-[#eef2f7] bg-white px-4 py-3 sm:grid-cols-2">
        <DetailLine label="系统指令" value={summary.partial ? summary.instructions : summary.instructions ? `${summary.instructions.length} 字符` : '无'} />
        <DetailLine label="工具" value={summary.toolText} />
      </div>
    </div>
  );
}

function SummaryCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-2xl border border-[#eef2f7] bg-white px-4 py-3">
      <div className="text-xs font-medium text-[#94a3b8]">{label}</div>
      <div className="mt-1 truncate text-lg font-bold text-[#0f172a]" title={value}>{value}</div>
    </div>
  );
}

function BodyBlock({ title, body }: { title: string; body: string }) {
  const [copied, setCopied] = useState(false);
  if (!body) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-[#e2e8f0] bg-[#fafafa] py-8 text-center text-sm text-[#94a3b8]">
        无{title}
      </div>
    );
  }
  let pretty = body;
  try {
    pretty = JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    pretty = body;
  }
  const handleCopy = async () => {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(body);
      } else {
        const ta = document.createElement('textarea');
        ta.value = body;
        ta.style.position = 'fixed';
        ta.style.opacity = '0';
        document.body.appendChild(ta);
        ta.select();
        document.execCommand('copy');
        document.body.removeChild(ta);
      }
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };
  return (
    <div className="min-w-0">
      <div className="mb-1.5 flex items-center justify-between">
        <span className="text-xs font-medium text-[#94a3b8]">{title}</span>
        <button
          type="button"
          onClick={handleCopy}
          className="inline-flex items-center gap-1 rounded-[11px] border border-[#e2e8f0] bg-white px-2.5 py-1 text-xs font-medium text-[#0f172a] transition hover:bg-slate-50"
        >
          {copied ? '已复制' : '复制'}
        </button>
      </div>
      <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-all rounded-xl bg-[#0f172a] px-4 py-3 text-xs leading-relaxed text-[#e2e8f0]">
{pretty}
      </pre>
    </div>
  );
}

function byteLength(value: string) {
  if (typeof TextEncoder !== 'undefined') {
    return new TextEncoder().encode(value).length;
  }
  return value.length;
}

function formatSize(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

function DetailLine({ label, value, valueClass = '' }: { label: string; value: ReactNode; valueClass?: string }) {
  return (
    <div className="min-w-0">
      <div className="text-xs font-medium text-[#94a3b8]">{label}</div>
      <div className={`mt-1 break-all text-sm text-[#334155] ${valueClass}`}>{value}</div>
    </div>
  );
}

function DetailTabButton({ active, onClick, children, className = '' }: { active: boolean; onClick: () => void; children: ReactNode; className?: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex items-center gap-1 rounded-lg px-3 py-1.5 text-sm font-medium transition ${
        active ? 'bg-indigo-50 text-indigo-700' : 'text-[#64748b] hover:bg-slate-50 hover:text-[#0f172a]'
      } ${className}`}
    >
      {children}
    </button>
  );
}

function formatTokenAmount(value: number) {
  const v = value || 0;
  if (v >= 1000000) return `${trimTrailingZero(v / 1000000)}M`;
  if (v >= 10000) return `${trimTrailingZero(v / 1000)}k`;
  return new Intl.NumberFormat('zh-CN').format(v);
}

function trimTrailingZero(n: number) {
  return n
    .toFixed(1)
    .replace(/\.0$/, '');
}

function StatusBadge({ status }: { status: string }) {
  const ok = status.startsWith('2');
  return (
    <span
      className="inline-block rounded-full px-[9px] py-[3px] text-[12px] font-semibold"
      style={ok ? { color: '#34d399', background: '#d1fae5' } : { color: '#fb7185', background: '#ffe4e6' }}
    >
      {status}
    </span>
  );
}

function SecurityBadge() {
  return (
    <span className="inline-flex h-[23px] items-center gap-1 whitespace-nowrap rounded-full border border-[#a7f3d0] bg-[#ecfdf5] px-2.5 text-[12px] font-medium text-[#059669]">
      <ShieldCheck size={13} />
      安全
    </span>
  );
}

function ApiBadge() {
  return (
    <span className="inline-block w-fit rounded-[5px] bg-[#eff6ff] px-[5px] py-[2px] text-[10px] font-medium text-[#2563eb]">
      API
    </span>
  );
}

function FilterInput({ icon, value, onChange, placeholder }: { icon: ReactNode; value: string; onChange: (v: string) => void; placeholder: string }) {
  return (
    <div className="flex h-10 items-center gap-2 rounded-[11px] border border-[#e2e8f0] bg-white px-3 transition focus-within:border-[#93c5fd] focus-within:shadow-[0_0_0_3px_rgba(147,197,253,0.35)]">
      {icon}
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full bg-transparent text-sm text-[#334155] outline-none placeholder:text-[#94a3b8]"
      />
    </div>
  );
}

function SelectInput({ icon, value, onChange, options }: { icon: ReactNode; value: string; onChange: (v: string) => void; options: { label: string; value: string }[] }) {
  return (
    <div className="flex h-10 items-center gap-2 rounded-[11px] border border-[#e2e8f0] bg-white px-3 transition focus-within:border-[#93c5fd] focus-within:shadow-[0_0_0_3px_rgba(147,197,253,0.35)]">
      {icon}
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full bg-transparent text-sm text-[#334155] outline-none"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>{opt.label}</option>
        ))}
      </select>
    </div>
  );
}

function DateInput({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder: string }) {
  return (
    <div className="flex h-10 items-center gap-2 rounded-[11px] border border-[#e2e8f0] bg-white px-3 transition focus-within:border-[#93c5fd] focus-within:shadow-[0_0_0_3px_rgba(147,197,253,0.35)]">
      <CalendarDays size={15} className="text-[#94a3b8]" />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full bg-transparent text-sm text-[#334155] outline-none placeholder:text-[#94a3b8]"
      />
      <button type="button" className="text-[#94a3b8] transition hover:text-[#64748b]" title="选择日期">
        <CalendarDays size={15} />
      </button>
    </div>
  );
}

function AppLogsPanel({
  logs,
  handleAppLogsSearch,
  handleAppLogsClear,
  filterOpen,
  setFilterOpen,
}: {
  logs: AppOperationLogEntry[];
  handleAppLogsSearch: (keyword: string, level: AppOperationLogEntry['level'] | 'all') => Promise<void>;
  handleAppLogsClear: () => Promise<void>;
  filterOpen: boolean;
  setFilterOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
}) {
  const [keyword, setKeyword] = useState('');
  const [level, setLevel] = useState<AppOperationLogEntry['level'] | 'all'>('all');

  const levelColor: Record<string, string> = {
    info: 'bg-sky-50 text-sky-600 border-sky-200',
    warn: 'bg-amber-50 text-amber-600 border-amber-200',
    error: 'bg-rose-50 text-rose-600 border-rose-200',
  };

  const kw = keyword.trim().toLowerCase();
  const filtered = useMemo(
    () =>
      logs.filter((log) => {
        if (kw && !`${log.module} ${log.detail ?? ''} ${log.message}`.toLowerCase().includes(kw)) return false;
        if (level !== 'all' && log.level !== level) return false;
        return true;
      }),
    [logs, kw, level],
  );

  return (
    <div className="flex flex-col gap-4">
      {filterOpen && (
      <div className="shrink-0 rounded-[17px] border border-[#e2e8f0] bg-white p-4 shadow-sm">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2 text-sm font-semibold text-[#0f172a]">
            <Search size={16} className="text-[#64748b]" />
            搜索筛选
          </div>
          <button type="button" className="text-[#94a3b8] transition hover:text-[#64748b]" onClick={() => setFilterOpen(false)} title="关闭">
            <X size={16} />
          </button>
        </div>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <FilterInput icon={<Search size={15} className="text-[#94a3b8]" />} value={keyword} onChange={setKeyword} placeholder="搜索日志内容或操作来源" />
          <SelectInput
            icon={<Server size={15} className="text-[#94a3b8]" />}
            value={level}
            onChange={(v) => setLevel(v as AppOperationLogEntry['level'] | 'all')}
            options={[{ label: '全部', value: 'all' }, { label: '信息', value: 'info' }, { label: '警告', value: 'warn' }, { label: '错误', value: 'error' }]}
          />
        </div>
        <div className="mt-3 flex justify-end">
          <Button variant="ghost" onClick={() => { setKeyword(''); setLevel('all'); }}>重置筛选</Button>
        </div>
      </div>
      )}

      <div className="overflow-hidden rounded-xl border border-slate-200">
        <table className="w-full text-left text-sm">
          <thead className="bg-slate-50 text-xs font-semibold text-slate-500">
            <tr>
              <th className="px-3 py-2.5">时间</th>
              <th className="px-3 py-2.5">级别</th>
              <th className="px-3 py-2.5">来源</th>
              <th className="px-3 py-2.5">消息</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((log, i) => (
              <tr key={i} className="border-t border-slate-100 text-slate-700">
                <td className="whitespace-nowrap px-3 py-2.5 text-slate-400">{formatLogTime(log.time)}</td>
                <td className="px-3 py-2.5">
                  <span className={`inline-block rounded-full border px-2 py-0.5 text-xs ${levelColor[log.level] ?? 'bg-slate-50 text-slate-500 border-slate-200'}`}>
                    {log.level}
                  </span>
                </td>
                <td className="px-3 py-2.5 text-slate-500">{log.module}</td>
                <td className="px-3 py-2.5">{log.detail}</td>
              </tr>
            ))}
            {filtered.length === 0 && (
              <tr>
                <td colSpan={4} className="px-3 py-12 text-center text-slate-400">暂无应用日志</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function formatLogTime(value: string | number): string {
  const raw = String(value).trim();
  if (!raw) return '-';

  const numeric = Number(raw);
  const date = Number.isFinite(numeric)
    ? new Date(numeric < 10_000_000_000 ? numeric * 1000 : numeric)
    : new Date(raw);
  if (Number.isNaN(date.getTime())) return raw;

  const pad = (part: number) => String(part).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}
