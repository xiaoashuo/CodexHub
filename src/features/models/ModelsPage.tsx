import { ModelTable } from '../../components/business/ModelTable';
import type { AppSettings, ModelConfig } from '../../types';
import type { ModelDialogMode } from '../../lib/appTypes';

export function ModelsPage({
  models,
  appSettings,
  handlePreviewAction,
  handleModelDialogOpen,
  handleModelDelete,
  handleModelEnabledToggle,
  handleModelSetActive,
  handleModelProxySave,
  handleModelConnectivityTest,
  handleModelChatTest,
  handleModelConfigExport,
  handleModelConfigImport,
  handleSyncModelsToCatalog,
}: {
  models: ModelConfig[];
  appSettings: AppSettings;
  handlePreviewAction: (action: string) => void;
  handleModelDialogOpen: (mode: ModelDialogMode, model?: ModelConfig) => void;
  handleModelDelete: (model: ModelConfig) => Promise<void>;
  handleModelEnabledToggle: (model: ModelConfig) => Promise<void>;
  handleModelSetActive: (model: ModelConfig) => Promise<void>;
  handleModelProxySave: (model: ModelConfig, proxyMode: ModelConfig['proxyMode'], proxyUrl: string) => Promise<void>;
  handleModelConnectivityTest: (model: ModelConfig) => Promise<void>;
  handleModelChatTest: (model: ModelConfig) => Promise<void>;
  handleModelConfigExport: () => Promise<void>;
  handleModelConfigImport: () => Promise<void>;
  handleSyncModelsToCatalog: () => Promise<void>;
}) {
  return (
    <div className="flex h-full min-h-0 w-full max-w-full flex-col overflow-hidden">
      <ModelTable
        models={models}
        appSettings={appSettings}
        handlePreviewAction={handlePreviewAction}
        handleModelDialogOpen={handleModelDialogOpen}
        handleModelDelete={handleModelDelete}
        handleModelEnabledToggle={handleModelEnabledToggle}
        handleModelSetActive={handleModelSetActive}
        handleModelProxySave={handleModelProxySave}
        handleModelConnectivityTest={handleModelConnectivityTest}
        handleModelChatTest={handleModelChatTest}
        handleModelConfigExport={handleModelConfigExport}
        handleModelConfigImport={handleModelConfigImport}
        handleSyncModelsToCatalog={handleSyncModelsToCatalog}
        />
    </div>
  );
}


