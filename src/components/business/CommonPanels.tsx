import { Card, CardContent, CardHeader } from '../ui/Card';
import { Button } from '../ui/Button';
import { PathPreview } from './PreviewParts';
import { CODEX_CONFIG_PATH, ROUTER_MAPPING_PATH } from '../../lib/constants';

export function QuickActions({ handlePreviewAction }: { handlePreviewAction: (action: string) => void }) {
  return (
    <Card>
      <CardHeader>
        <h3 className="text-lg font-bold text-slate-950">快捷操作</h3>
      </CardHeader>
      <CardContent className="space-y-3">
        <Button className="w-full" onClick={() => handlePreviewAction('生成 Catalog JSON')}>生成 Catalog JSON</Button>
        <Button className="w-full" variant="secondary" onClick={() => handlePreviewAction('生成 Router Mapping')}>生成 Router Mapping</Button>
        <Button className="w-full" variant="secondary" onClick={() => handlePreviewAction('备份 config.toml')}>备份 config.toml</Button>
        <Button className="w-full" variant="secondary" onClick={() => handlePreviewAction('写入 Codex V1 配置')}>写入 Codex V1 配置</Button>
        <Button className="w-full" variant="ghost" onClick={() => handlePreviewAction('打开文件位置')}>打开文件位置</Button>
      </CardContent>
    </Card>
  );
}

export function ConfigPreview({ routerUrl }: { routerUrl: string }) {
  return (
    <Card>
      <CardHeader>
        <h3 className="text-lg font-bold text-slate-950">配置预览</h3>
      </CardHeader>
      <CardContent className="space-y-4 text-sm">
        <PathPreview label="config.toml" value={CODEX_CONFIG_PATH} />
        <PathPreview label="mapping" value={ROUTER_MAPPING_PATH} />
        <PathPreview label="router" value={routerUrl} />
      </CardContent>
    </Card>
  );
}


export function StatusChecklist() {
  const items = ['检测 Codex 配置文件', '生成 Catalog JSON', '生成 Router Mapping', '启动本地 Router', '重启 Codex Desktop'];

  return (
    <Card>
      <CardHeader>
        <h3 className="text-lg font-bold text-slate-950">实施检查清单</h3>
      </CardHeader>
      <CardContent className="space-y-3">
        {items.map((item, index) => (
          <div key={item} className="flex items-center gap-3 rounded-2xl bg-slate-50 px-4 py-3 text-sm text-slate-700">
            <span className="flex h-7 w-7 items-center justify-center rounded-full bg-indigo-100 font-bold text-indigo-700">{index + 1}</span>
            <span>{item}</span>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
