import { useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Badge } from '../../components/ui/Badge';
import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import {
  invokeDeleteSkillBackup,
  invokeImportSkill,
  invokeLoadCodexPlugins,
  invokeLoadInstalledSkills,
  invokeLoadSkillBackups,
  invokeRemoveSkill,
  invokeRestoreSkillBackup,
  invokeSetCodexPluginEnabled,
  invokeSetCodexPluginSkillEnabled,
} from '../../lib/tauriBridge';
import type { CodexPluginSummary, InstalledSkillSummary, PluginListResult, SkillBackupSummary } from '../../types';

type MainTabKey = 'plugins' | 'skills';
type SkillTabKey = 'installed' | 'backups';

export function SkillsManagementPage() {
  const [activeMainTab, setActiveMainTab] = useState<MainTabKey>('plugins');
  const [activeSkillTab, setActiveSkillTab] = useState<SkillTabKey>('installed');
  const [plugins, setPlugins] = useState<CodexPluginSummary[]>([]);
  const [selectedPluginKey, setSelectedPluginKey] = useState('');
  const [pluginKeyword, setPluginKeyword] = useState('');
  const [skills, setSkills] = useState<InstalledSkillSummary[]>([]);
  const [backups, setBackups] = useState<SkillBackupSummary[]>([]);
  const [pluginsRootPath, setPluginsRootPath] = useState('');
  const [skillsRootPath, setSkillsRootPath] = useState('');
  const [backupRootPath, setBackupRootPath] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');

  const selectedPlugin = useMemo(
    () => plugins.find((plugin) => pluginListKey(plugin) === selectedPluginKey) || plugins[0],
    [plugins, selectedPluginKey],
  );
  const visiblePlugins = useMemo(() => filterPlugins(plugins, pluginKeyword), [plugins, pluginKeyword]);

  const refresh = async () => {
    setLoading(true);
    try {
      const [pluginResult, skillResult, backupResult] = await Promise.all([
        invokeLoadCodexPlugins(),
        invokeLoadInstalledSkills(),
        invokeLoadSkillBackups(),
      ]);
      setPlugins(pluginResult.items);
      setPluginsRootPath(pluginResult.rootPath);
      setSkills(skillResult.items);
      setBackups(backupResult.items);
      setSkillsRootPath(skillResult.rootPath);
      setBackupRootPath(backupResult.rootPath);
      setSelectedPluginKey((current) => {
        if (current && pluginResult.items.some((plugin) => pluginListKey(plugin) === current)) {
          return current;
        }
        return pluginResult.items[0] ? pluginListKey(pluginResult.items[0]) : '';
      });
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

  const runAction = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
      await refresh();
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  };

  const applyPluginListResult = (result: PluginListResult) => {
    setPlugins(result.items);
    setPluginsRootPath(result.rootPath);
    setSelectedPluginKey((current) => {
      if (current && result.items.some((plugin) => pluginListKey(plugin) === current)) {
        return current;
      }
      return result.items[0] ? pluginListKey(result.items[0]) : '';
    });
  };

  const handleImport = async () => {
    const selected = await open({ directory: true, multiple: false, title: '选择 Skill 目录' });
    if (typeof selected !== 'string') return;

    await runAction(async () => {
      const result = await invokeImportSkill(selected);
      setMessage(result.replacedExisting ? `已导入并替换 ${result.skill.name}` : `已导入 ${result.skill.name}`);
    });
  };

  const handleRemove = async (skill: InstalledSkillSummary) => {
    if (!window.confirm(`确定移除 Skill ${skill.title || skill.name}？移除前会自动备份。`)) return;
    await runAction(async () => {
      await invokeRemoveSkill(skill.id);
      setMessage(`已移除 ${skill.title || skill.name}，并创建备份。`);
    });
  };

  const handleRestore = async (backup: SkillBackupSummary) => {
    await runAction(async () => {
      await invokeRestoreSkillBackup(backup.id);
      setMessage(`已恢复 ${backup.title || backup.name}`);
    });
  };

  const handleDeleteBackup = async (backup: SkillBackupSummary) => {
    if (!window.confirm(`确定删除备份 ${backup.id}？`)) return;
    await runAction(async () => {
      await invokeDeleteSkillBackup(backup.id);
      setMessage(`已删除备份 ${backup.id}`);
    });
  };

  const handleTogglePlugin = async (plugin: CodexPluginSummary) => {
    setBusy(true);
    try {
      const result = await invokeSetCodexPluginEnabled(plugin.id, !plugin.enabled);
      applyPluginListResult(result);
      setMessage('');
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleTogglePluginSkill = async (fullName: string, enabled: boolean) => {
    setBusy(true);
    try {
      const result = await invokeSetCodexPluginSkillEnabled(fullName, !enabled);
      applyPluginListResult(result);
      setMessage('');
    } catch (error) {
      setMessage(formatError(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <CardHeader className="py-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex gap-2">
              <Button variant={activeMainTab === 'plugins' ? 'primary' : 'secondary'} onClick={() => setActiveMainTab('plugins')}>插件</Button>
              <Button variant={activeMainTab === 'skills' ? 'primary' : 'secondary'} onClick={() => setActiveMainTab('skills')}>技能</Button>
            </div>
            <div className="flex gap-2">
              <Button variant="secondary" onClick={refresh} disabled={loading || busy}>刷新</Button>
              {activeMainTab === 'skills' && activeSkillTab === 'installed' && (
                <Button onClick={handleImport} disabled={busy}>导入 Skill</Button>
              )}
            </div>
          </div>
          <div className="mt-4 grid gap-2 text-sm text-slate-500">
            {activeMainTab === 'plugins' ? (
              <div>插件缓存目录：<span className="break-all font-medium text-slate-700">{pluginsRootPath || '-'}</span></div>
            ) : (
              <>
                <div>Skills 目录：<span className="break-all font-medium text-slate-700">{skillsRootPath || '-'}</span></div>
                <div>备份目录：<span className="break-all font-medium text-slate-700">{backupRootPath || '-'}</span></div>
              </>
            )}
          </div>
        </CardHeader>
        <CardContent className={`min-h-0 flex-1 ${activeMainTab === 'plugins' ? 'overflow-hidden' : 'overflow-y-auto'}`}>
          {message && <div className="mb-4 rounded-lg bg-slate-50 px-4 py-3 text-sm text-slate-600">{message}</div>}
          {loading ? (
            <div className="py-12 text-center text-sm text-slate-400">正在加载...</div>
          ) : activeMainTab === 'plugins' ? (
            <PluginsPanel
              plugins={visiblePlugins}
              keyword={pluginKeyword}
              selectedPlugin={selectedPlugin}
              busy={busy}
              onKeywordChange={setPluginKeyword}
              onSelectPlugin={setSelectedPluginKey}
              onTogglePlugin={handleTogglePlugin}
              onTogglePluginSkill={handleTogglePluginSkill}
            />
          ) : (
            <SkillsPanel
              activeSkillTab={activeSkillTab}
              setActiveSkillTab={setActiveSkillTab}
              skills={skills}
              backups={backups}
              busy={busy}
              handleRemove={handleRemove}
              handleRestore={handleRestore}
              handleDeleteBackup={handleDeleteBackup}
            />
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function PluginsPanel({
  plugins,
  keyword,
  selectedPlugin,
  busy,
  onKeywordChange,
  onSelectPlugin,
  onTogglePlugin,
  onTogglePluginSkill,
}: {
  plugins: CodexPluginSummary[];
  keyword: string;
  selectedPlugin?: CodexPluginSummary;
  busy: boolean;
  onKeywordChange: (value: string) => void;
  onSelectPlugin: (id: string) => void;
  onTogglePlugin: (plugin: CodexPluginSummary) => void;
  onTogglePluginSkill: (fullName: string, enabled: boolean) => void;
}) {
  return (
    <div className="grid h-full min-h-0 gap-4 lg:grid-cols-[320px_minmax(0,1fr)]">
      <div className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-slate-200">
        <div className="border-b border-slate-100 bg-white p-3">
          <input
            value={keyword}
            onChange={(event) => onKeywordChange(event.target.value)}
            placeholder="搜索插件、来源或技能"
            className="h-10 w-full rounded-lg border border-slate-200 px-3 text-sm outline-none transition focus:border-indigo-300 focus:ring-2 focus:ring-indigo-100"
          />
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">
          {plugins.length === 0 ? (
            <div className="px-4 py-10 text-center text-sm text-slate-400">{keyword.trim() ? '没有匹配的插件。' : '还没有扫描到插件。'}</div>
          ) : (
            plugins.map((plugin) => (
              <button
                key={pluginListKey(plugin)}
                type="button"
                onClick={() => onSelectPlugin(pluginListKey(plugin))}
                className={`block w-full border-b border-slate-100 px-4 py-3 text-left last:border-b-0 ${selectedPlugin && pluginListKey(selectedPlugin) === pluginListKey(plugin) ? 'bg-indigo-50' : 'hover:bg-slate-50'}`}
              >
                <div className="flex items-center justify-between gap-3">
                  <span className="truncate text-sm font-semibold text-slate-950">{plugin.displayName}</span>
                  <Badge tone={plugin.enabled ? 'green' : 'slate'}>{plugin.enabled ? '启用' : '禁用'}</Badge>
                </div>
                <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-slate-500">
                  <Badge tone="blue">{formatSource(plugin.source)}</Badge>
                  <span>{plugin.skillCount} 个技能</span>
                  <span>{plugin.version || '无版本'}</span>
                </div>
              </button>
            ))
          )}
        </div>
      </div>
      {selectedPlugin && (
        <PluginDetail plugin={selectedPlugin} busy={busy} onTogglePlugin={onTogglePlugin} onTogglePluginSkill={onTogglePluginSkill} />
      )}
    </div>
  );
}

function PluginDetail({
  plugin,
  busy,
  onTogglePlugin,
  onTogglePluginSkill,
}: {
  plugin: CodexPluginSummary;
  busy: boolean;
  onTogglePlugin: (plugin: CodexPluginSummary) => void;
  onTogglePluginSkill: (fullName: string, enabled: boolean) => void;
}) {
  return (
    <div className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-slate-200">
      <div className="shrink-0 border-b border-slate-100 px-5 py-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h3 className="text-base font-semibold text-slate-950">{plugin.displayName}</h3>
              <Badge tone="blue">{plugin.id}</Badge>
            </div>
            <div className="mt-2 text-sm text-slate-500">{plugin.shortDescription || plugin.description || '这个插件没有描述。'}</div>
          </div>
          <SwitchWithLabel enabled={plugin.enabled} disabled={busy} onClick={() => onTogglePlugin(plugin)} />
        </div>
        <div className="mt-4 grid gap-2 text-xs text-slate-500">
          <div className="flex flex-wrap gap-x-6 gap-y-1">
            <span>来源：<span className="font-medium text-slate-700">{formatSource(plugin.source)}</span></span>
            <span>开发者：<span className="font-medium text-slate-700">{plugin.developerName || '-'}</span></span>
            <span>分类：<span className="font-medium text-slate-700">{plugin.category || '-'}</span></span>
          </div>
          <div>路径：<span className="break-all font-medium text-slate-700">{plugin.directoryPath}</span></div>
        </div>
      </div>
      <div className="flex min-h-0 flex-1 flex-col px-5 py-4">
        <div className="mb-3 shrink-0 text-sm font-semibold text-slate-950">插件内置 Skills</div>
        {plugin.skills.length === 0 ? (
          <div className="rounded-lg border border-dashed border-slate-200 px-4 py-8 text-center text-sm text-slate-400">这个插件没有声明 Skills。</div>
        ) : (
          <div className="min-h-0 flex-1 overflow-y-auto rounded-lg border border-slate-200">
            {plugin.skills.map((skill) => (
              <div key={skill.fullName} className="flex items-center justify-between gap-4 border-b border-slate-100 px-4 py-3 last:border-b-0">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-slate-950">{skill.title || skill.name}</span>
                    <Badge tone="slate">{skill.fullName}</Badge>
                  </div>
                  {skill.summary && <div className="mt-1 truncate text-sm text-slate-500">{skill.summary}</div>}
                </div>
                <SwitchWithLabel enabled={skill.enabled} disabled={busy || !plugin.enabled} onClick={() => onTogglePluginSkill(skill.fullName, skill.enabled)} />
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function SwitchWithLabel({ enabled, disabled, onClick }: { enabled: boolean; disabled?: boolean; onClick: () => void }) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      <span className={`w-8 text-right text-xs font-medium ${enabled ? 'text-emerald-700' : 'text-slate-500'}`}>
        {enabled ? '启用' : '禁用'}
      </span>
      <SwitchButton enabled={enabled} disabled={disabled} onClick={onClick} />
    </div>
  );
}

function SwitchButton({ enabled, disabled, onClick }: { enabled: boolean; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`inline-flex h-7 w-14 shrink-0 items-center rounded-full p-0.5 transition disabled:cursor-not-allowed disabled:opacity-50 ${enabled ? 'bg-emerald-500' : 'bg-slate-300'}`}
      aria-pressed={enabled}
      title={enabled ? '已启用，点击禁用' : '已禁用，点击启用'}
    >
      <span className={`h-6 w-6 rounded-full bg-white shadow-sm transition ${enabled ? 'translate-x-7' : 'translate-x-0'}`} />
    </button>
  );
}

function SkillsPanel({
  activeSkillTab,
  setActiveSkillTab,
  skills,
  backups,
  busy,
  handleRemove,
  handleRestore,
  handleDeleteBackup,
}: {
  activeSkillTab: SkillTabKey;
  setActiveSkillTab: (tab: SkillTabKey) => void;
  skills: InstalledSkillSummary[];
  backups: SkillBackupSummary[];
  busy: boolean;
  handleRemove: (skill: InstalledSkillSummary) => void;
  handleRestore: (backup: SkillBackupSummary) => void;
  handleDeleteBackup: (backup: SkillBackupSummary) => void;
}) {
  return (
    <div>
      <div className="mb-4 flex gap-2">
        <Button variant={activeSkillTab === 'installed' ? 'primary' : 'secondary'} onClick={() => setActiveSkillTab('installed')}>个人技能</Button>
        <Button variant={activeSkillTab === 'backups' ? 'primary' : 'secondary'} onClick={() => setActiveSkillTab('backups')}>备份</Button>
      </div>
      {activeSkillTab === 'installed' ? (
        <InstalledSkillsList skills={skills} busy={busy} handleRemove={handleRemove} />
      ) : (
        <SkillBackupsList backups={backups} busy={busy} handleRestore={handleRestore} handleDeleteBackup={handleDeleteBackup} />
      )}
    </div>
  );
}

function InstalledSkillsList({ skills, busy, handleRemove }: { skills: InstalledSkillSummary[]; busy: boolean; handleRemove: (skill: InstalledSkillSummary) => void }) {
  if (skills.length === 0) {
    return <div className="py-12 text-center text-sm text-slate-400">还没有安装个人 Skill。</div>;
  }

  return (
    <div className="overflow-hidden rounded-lg border border-slate-200">
      {skills.map((skill) => (
        <div key={skill.id} className="flex items-center justify-between gap-4 border-b border-slate-100 px-5 py-4 last:border-b-0">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-semibold text-slate-950">{skill.title || skill.name}</span>
              <Badge tone="blue">{skill.name}</Badge>
            </div>
            {skill.summary && <div className="mt-2 truncate text-sm text-slate-500">{skill.summary}</div>}
          </div>
          <Button variant="danger" disabled={busy} onClick={() => handleRemove(skill)}>移除</Button>
        </div>
      ))}
    </div>
  );
}

function SkillBackupsList({
  backups,
  busy,
  handleRestore,
  handleDeleteBackup,
}: {
  backups: SkillBackupSummary[];
  busy: boolean;
  handleRestore: (backup: SkillBackupSummary) => void;
  handleDeleteBackup: (backup: SkillBackupSummary) => void;
}) {
  if (backups.length === 0) {
    return <div className="py-12 text-center text-sm text-slate-400">还没有 Skill 备份。</div>;
  }

  return (
    <div className="overflow-hidden rounded-lg border border-slate-200">
      {backups.map((backup) => (
        <div key={backup.id} className="flex items-center justify-between gap-4 border-b border-slate-100 px-5 py-4 last:border-b-0">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-semibold text-slate-950">{backup.title || backup.name}</span>
              <Badge tone="slate">{formatTime(backup.createdAt)}</Badge>
            </div>
            <div className="mt-2 truncate text-sm text-slate-500">{backup.relativePath}</div>
            <div className="mt-1 truncate text-xs text-slate-400">{backup.backupPath}</div>
          </div>
          <div className="flex shrink-0 gap-2">
            <Button variant="secondary" disabled={busy} onClick={() => handleRestore(backup)}>恢复</Button>
            <Button variant="danger" disabled={busy} onClick={() => handleDeleteBackup(backup)}>删除</Button>
          </div>
        </div>
      ))}
    </div>
  );
}

function filterPlugins(plugins: CodexPluginSummary[], keyword: string) {
  const normalized = keyword.trim().toLowerCase();
  const sorted = [...plugins].sort((left, right) => (
    left.source.localeCompare(right.source, 'zh-CN')
    || left.displayName.localeCompare(right.displayName, 'zh-CN')
  ));
  if (!normalized) return sorted;

  return sorted.filter((plugin) => {
    const skillText = plugin.skills.map((skill) => `${skill.name} ${skill.fullName} ${skill.summary || ''}`).join(' ');
    const haystack = [
      plugin.displayName,
      plugin.name,
      plugin.id,
      plugin.source,
      formatSource(plugin.source),
      plugin.version,
      plugin.description || '',
      plugin.shortDescription || '',
      plugin.developerName || '',
      plugin.category || '',
      skillText,
    ].join(' ').toLowerCase();
    return haystack.includes(normalized);
  });
}

function pluginListKey(plugin: CodexPluginSummary) {
  return plugin.directoryPath || plugin.manifestPath || `${plugin.id}:${plugin.version}`;
}

function formatSource(source: string) {
  const labels: Record<string, string> = {
    'openai-bundled': 'OpenAI Bundled',
    'openai-curated': 'OpenAI Curated',
    'openai-curated-remote': 'OpenAI Curated Remote',
    'openai-primary-runtime': 'OpenAI Primary Runtime',
  };
  return labels[source] || source;
}

function formatTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString('zh-CN', { hour12: false });
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
