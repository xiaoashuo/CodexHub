import { getCurrentWindow } from '@tauri-apps/api/window';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { invokeOpenExternalUrl } from '../../lib/tauriBridge';
import type { CodexRestartMode, RouterStatus } from '../../types';
import type { NavItem } from '../../data/seedData';
import type { MouseEvent } from 'react';

const GITHUB_REPOSITORY_URL = 'https://github.com/xiaoashuo/CodexHub';

function GitHubMark() {
  return (
    <svg aria-hidden="true" className="h-4 w-4" viewBox="0 0 24 24" fill="currentColor">
      <path d="M12 .5C5.65.5.85 5.3.85 11.6c0 4.9 3.15 9.05 7.5 10.5.55.1.75-.25.75-.55v-2c-3.05.65-3.7-1.3-3.7-1.3-.5-1.25-1.2-1.6-1.2-1.6-1-.7.1-.7.1-.7 1.1.1 1.7 1.15 1.7 1.15 1 .1.9 2.3 2.9 1.65.1-.75.4-1.25.7-1.55-2.45-.3-5-1.2-5-5.4 0-1.2.4-2.15 1.15-2.9-.1-.3-.5-1.45.1-2.95 0 0 .95-.3 3.1 1.1.9-.25 1.85-.35 2.8-.35s1.9.1 2.8.35c2.15-1.45 3.1-1.1 3.1-1.1.6 1.5.2 2.65.1 2.95.7.75 1.15 1.8 1.15 2.9 0 4.2-2.55 5.1-5 5.4.4.35.75 1.05.75 2.1v3.15c0 .3.2.65.75.55 4.35-1.45 7.5-5.6 7.5-10.5C23.15 5.3 18.35.5 12 .5Z" />
    </svg>
  );
}

function getSafeCurrentWindow() {
  if (!('__TAURI_INTERNALS__' in window)) {
    return null;
  }

  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

export function TitleBar({ checkingVersion, handleVersionCheck }: { checkingVersion: boolean; handleVersionCheck: () => Promise<void> }) {
  const appWindow = getSafeCurrentWindow();
  const handleTitleMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (!appWindow) return;
    if (event.button !== 0) return;
    if (event.detail >= 2) {
      void appWindow.toggleMaximize();
      return;
    }
    void appWindow.startDragging();
  };
  const handleGitHubOpen = async () => {
    try {
      await invokeOpenExternalUrl(GITHUB_REPOSITORY_URL);
    } catch {
      window.open(GITHUB_REPOSITORY_URL, '_blank', 'noopener,noreferrer');
    }
  };

  return (
    <div className="flex h-10 select-none items-center justify-between border-b border-white/70 bg-white/80 pl-4 backdrop-blur">
      <div
        className="flex h-full flex-1 items-center text-sm font-semibold text-slate-700"
        onMouseDown={handleTitleMouseDown}
      >
        Codex伴侣
      </div>
      <div className="flex h-full items-stretch" onMouseDown={(event) => event.stopPropagation()}>
        <button className="inline-flex w-10 items-center justify-center text-slate-500 hover:bg-indigo-50 hover:text-indigo-700" type="button" title="GitHub" aria-label="打开 GitHub 仓库" onClick={() => void handleGitHubOpen()}>
          <GitHubMark />
        </button>
        <button className="w-10 text-slate-500 hover:bg-indigo-50 hover:text-indigo-700 disabled:opacity-60" type="button" title="\u68c0\u67e5\u66f4\u65b0" aria-label="\u68c0\u67e5\u66f4\u65b0" disabled={checkingVersion} onClick={handleVersionCheck}>
          {checkingVersion ? <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-indigo-200 border-t-indigo-600 align-[-2px]" /> : <span className="text-base leading-none">{'\u21bb'}</span>}
        </button>
        <button className="w-11 text-slate-500 hover:bg-slate-100 disabled:opacity-40" type="button" aria-label="\u6700\u5c0f\u5316" disabled={!appWindow} onClick={() => appWindow?.minimize()}>&minus;</button>
        <button className="w-11 text-slate-500 hover:bg-slate-100 disabled:opacity-40" type="button" aria-label="\u6700\u5927\u5316" disabled={!appWindow} onClick={() => appWindow?.toggleMaximize()}>{'\u25a1'}</button>
        <button className="w-11 text-slate-500 hover:bg-rose-500 hover:text-white disabled:opacity-40" type="button" aria-label="\u5173\u95ed" disabled={!appWindow} onClick={() => appWindow?.close()}>&times;</button>
      </div>
    </div>
  );
}

export function Header({ routerStatus, routerActionRunning, handleRouterToggle }: { routerStatus: RouterStatus; routerActionRunning: boolean; handleRouterToggle: (codexRestartMode?: CodexRestartMode) => Promise<void> }) {
  const actionText = routerStatus === 'running' ? '停止 Router' : '启动 Router';

  return (
    <header className="mb-6 flex items-center justify-between">
      <div>
        <h2 className="text-3xl font-bold text-slate-950">Codex伴侣</h2>
        <p className="mt-2 text-slate-500">面向 Codex 本地使用场景的统一管理与增强工具。</p>
      </div>
      <div className="flex items-center gap-3">
        <Badge tone={routerStatus === 'running' ? 'green' : 'amber'}>{routerStatus === 'running' ? 'Router 运行中' : 'Router 已停止'}</Badge>
        <Button onClick={() => handleRouterToggle()} disabled={routerActionRunning}>
          {routerActionRunning && <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/50 border-t-white" />}
          {routerActionRunning ? '处理中...' : actionText}
        </Button>
      </div>
    </header>
  );
}

export function Sidebar({ activeNav, navItems, setActiveNav }: { activeNav: NavItem; navItems: readonly NavItem[]; setActiveNav: (nav: NavItem) => void }) {
  return (
    <aside className="w-72 shrink-0 border-r border-white/70 bg-white/75 px-5 py-6 shadow-xl shadow-indigo-100/40 backdrop-blur">
      <div className="mb-8 rounded-3xl bg-slate-950 p-5 text-white shadow-lg">
        <div className="flex items-center gap-3">
          <div className="relative flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-white text-lg font-black text-slate-950 shadow-md">
            C
            <span className="absolute -right-1 -bottom-1 h-4 w-4 rounded-full border-[3px] border-slate-950 bg-emerald-400" />
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="mt-1 truncate text-2xl font-bold">Codex伴侣</h1>
          </div>
        </div>
        <p className="mt-4 text-sm leading-6 text-slate-300">Codex 本地工作流的桌面控制台。</p>
      </div>
      <nav className="space-y-1">
        {navItems.map((item) => (
          <button
            key={item}
            className={`w-full rounded-xl px-4 py-2.5 text-left text-sm font-semibold transition ${
              activeNav === item ? 'bg-indigo-600 text-white shadow-lg shadow-indigo-200' : 'text-slate-600 hover:bg-white hover:text-slate-950'
            }`}
            onClick={() => setActiveNav(item)}
          >
            {item}
          </button>
        ))}
      </nav>
    </aside>
  );
}
