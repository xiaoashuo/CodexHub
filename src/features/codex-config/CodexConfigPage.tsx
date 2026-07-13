import { Button } from '../../components/ui/Button';
import { Card, CardContent, CardHeader } from '../../components/ui/Card';
import { ConfigPreview, StatusChecklist } from '../../components/business/CommonPanels';
import { DEFAULT_MODEL_SLUG, DEFAULT_PROVIDER_NAME } from '../../lib/constants';

export function CodexConfigPage({ handlePreviewAction, routerUrl }: { handlePreviewAction: (action: string) => void; routerUrl: string }) {
  return (
    <div className="grid grid-cols-12 gap-6">
      <section className="col-span-7 space-y-6">
        <Card>
          <CardHeader>
            <h3 className="text-lg font-bold text-slate-950">Codex V1 配置写入预览</h3>
            <p className="mt-1 text-sm text-slate-500">后续真实实现时会先备份，再局部更新 config.toml。</p>
          </CardHeader>
          <CardContent>
            <pre className="overflow-auto rounded-2xl bg-slate-950 p-5 text-sm leading-6 text-indigo-100">{`model_provider = "ai-router"
model = "${DEFAULT_MODEL_SLUG}"

[model_providers.ai-router]
name = "${DEFAULT_PROVIDER_NAME}"
base_url = "${routerUrl}"
wire_api = "responses"
requires_openai_auth = true
`}</pre>
          </CardContent>
        </Card>
        <ConfigPreview routerUrl={routerUrl} />
      </section>
      <section className="col-span-5 space-y-6">
        <Card>
          <CardHeader>
            <h3 className="text-lg font-bold text-slate-950">配置操作</h3>
          </CardHeader>
          <CardContent className="space-y-3">
            <Button className="w-full" onClick={() => handlePreviewAction('检测 Codex 配置')}>检测 Codex 配置</Button>
            <Button className="w-full" variant="secondary" onClick={() => handlePreviewAction('备份 config.toml')}>备份 config.toml</Button>
            <Button className="w-full" variant="secondary" onClick={() => handlePreviewAction('写入 Codex V1 配置')}>写入 Codex V1 配置</Button>
            <Button className="w-full" variant="ghost" onClick={() => handlePreviewAction('恢复上次备份')}>恢复上次备份</Button>
          </CardContent>
        </Card>
        <StatusChecklist />
      </section>
    </div>
  );
}
