import { useState } from 'react';
import { McpManagementPage } from '../mcp/McpManagementPage';
import { SkillsManagementPage } from '../skills/SkillsManagementPage';

type ServiceTab = 'skills' | 'mcp';

export function ServicesPage() {
  const [activeTab, setActiveTab] = useState<ServiceTab>('skills');

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <div className="mb-5 shrink-0">
        <div className="flex items-center justify-between gap-4">
          <h2 className="text-2xl font-bold leading-tight text-slate-950">服务</h2>
        <div className="flex gap-1 rounded-2xl bg-slate-100 p-1">
          <button
            type="button"
            onClick={() => setActiveTab('skills')}
            className={`rounded-xl px-5 py-2.5 text-sm font-semibold transition ${activeTab === 'skills' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-800'}`}
          >
            技能
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('mcp')}
            className={`rounded-xl px-5 py-2.5 text-sm font-semibold transition ${activeTab === 'mcp' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500 hover:text-slate-800'}`}
          >
            MCP
          </button>
        </div>
        </div>
        <p className="mt-1.5 text-sm text-slate-500">管理 Codex 技能与 MCP 服务，扩展桌面端能力。</p>
      </div>
      <div className="min-h-0 flex-1 overflow-hidden">
        {activeTab === 'skills' ? <SkillsManagementPage /> : <McpManagementPage />}
      </div>
    </div>
  );
}
