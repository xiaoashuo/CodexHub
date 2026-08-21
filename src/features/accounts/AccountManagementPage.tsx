import { useEffect, useMemo, useRef, useState } from 'react';
import { Calendar, RefreshCw } from 'lucide-react';
import { Badge } from '../../components/ui/Badge';
import { invokeImportCpaAccount } from '../../lib/tauriBridge';
import { Button } from '../../components/ui/Button';
import { Card, CardContent } from '../../components/ui/Card';
import {
  invokeCodexOAuthCallbackListenerStatus,
  invokeCodexOAuthLoginStatus,
  invokeExportCodexAccounts,
  invokeImportChatGptSessionAccount,
  invokeImportCurrentCodexAccount,
  invokeOpenExternalUrl,
  invokeRefreshCodexAccountUsage,
  invokeRefreshCodexAccountToken,
  invokeRestartCodexApp,
  invokeRemoveCodexAccountSnapshot,
  invokeScanCodexAccounts,
  invokeStartCodexAccountLogin,
  invokeStartCodexClientLogin,
  invokeSwitchCodexAccount,
  invokeUpdateCodexAccountExpiration,
} from '../../lib/tauriBridge';
import type { AppSettings, CodexAccount, CodexAccountScanResult, CodexAccountUsageWindow } from '../../types';

const EMPTY_SCAN: CodexAccountScanResult = {
  accounts: [],
  currentAccountId: undefined,
  apiHealthy: false,
  scannedAt: '',
};

let cachedScanResult: CodexAccountScanResult | null = null;
let cachedSelectedAccountId = '';
let initialScanPromise: Promise<CodexAccountScanResult> | null = null;

type LoginTab = 'web' | 'client' | 'cpa';
type LoginStatus = 'idle' | 'waiting' | 'success' | 'error';
type SwitchStepStatus = 'pending' | 'running' | 'success' | 'error';
type OAuthCallbackListenerStatus = {
  running: boolean;
  host: string;
  port: number;
  callbackUrl: string;
  message: string;
};
type ToastTone = 'success' | 'error';
type ToastState = {
  message: string;
  tone: ToastTone;
};
type AccountConnectionStatus = 'online' | 'error' | 'offline';
type AccountListItem = CodexAccount & {
  connectionStatus: AccountConnectionStatus;
};

type SwitchProgressState = {
  account: CodexAccount;
  switchStatus: SwitchStepStatus;
  restartStatus: SwitchStepStatus;
  message: string;
  restartMessage: string;
};

type ExportSuccessState = {
  message: string;
  path: string;
};

const CHATGPT_LOGIN_URL = 'https://chatgpt.com/auth/login';
const CHATGPT_SESSION_URL = 'https://chatgpt.com/api/auth/session';

function waitForNextFrame() {
  return new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
}

export function AccountManagementPage({ appSettings }: { appSettings: AppSettings }) {
  const [scanResult, setScanResult] = useState<CodexAccountScanResult>(cachedScanResult ?? EMPTY_SCAN);
  const [selectedAccountId, setSelectedAccountId] = useState(cachedSelectedAccountId);
  const [keyword, setKeyword] = useState('');
  const [loading, setLoading] = useState(false);
  const [usageLoading, setUsageLoading] = useState(false);
  const [tokenRefreshLoading, setTokenRefreshLoading] = useState(false);
  const [error, setError] = useState('');
  const [operationMessage, setOperationMessage] = useState('');
  const [usageFailedAccountKey, setUsageFailedAccountKey] = useState('');
  const [loginDialogOpen, setLoginDialogOpen] = useState(false);
  const [loginTab, setLoginTab] = useState<LoginTab>('web');
  const [loginUrl, setLoginUrl] = useState('');
  const [loginStatus, setLoginStatus] = useState<LoginStatus>('idle');
  const [loginMessage, setLoginMessage] = useState('');
  const [loginBusy, setLoginBusy] = useState(false);
  const [reauthorizeAccount, setReauthorizeAccount] = useState<CodexAccount | null>(null);
  const [reauthorizeUrl, setReauthorizeUrl] = useState('');
  const [reauthorizeStatus, setReauthorizeStatus] = useState<LoginStatus>('idle');
  const [reauthorizeMessage, setReauthorizeMessage] = useState('');
  const [reauthorizeBusy, setReauthorizeBusy] = useState(false);
  const [clientLoginStarted, setClientLoginStarted] = useState(false);
  const [sessionLoginStarted, setSessionLoginStarted] = useState(false);
  const [cpaLoginStarted, setCpaLoginStarted] = useState(false);
  const [cpaText, setCpaText] = useState('');
  const [sessionJson, setSessionJson] = useState('');
  const [callbackListenerStatus, setCallbackListenerStatus] = useState<OAuthCallbackListenerStatus | null>(null);
  const [switchProgress, setSwitchProgress] = useState<SwitchProgressState | null>(null);
  const [exportSuccess, setExportSuccess] = useState<ExportSuccessState | null>(null);
  const [toast, setToast] = useState<ToastState | null>(null);
  const copyToastTimerRef = useRef<number | null>(null);
  const usageRefreshInFlightRef = useRef(false);

  useEffect(() => {
    refreshCallbackListenerStatus().catch((statusError) => {
      setCallbackListenerStatus({
        running: false,
        host: '127.0.0.1',
        port: 1455,
        callbackUrl: 'http://localhost:1455/auth/callback',
        message: formatUnknownError(statusError),
      });
    });

    if (cachedScanResult) {
      void refreshCurrentAccountUsageInBackground(cachedScanResult);
      return;
    }

    handleInitialScan();
  }, []);

  useEffect(() => {
    if (!loginDialogOpen || loginTab !== 'web' || loginStatus !== 'waiting') {
      return;
    }

    const timer = window.setInterval(async () => {
      try {
        const status = await invokeCodexOAuthLoginStatus();
        if (status.status === 'success') {
          setLoginStatus('success');
          setLoginMessage(status.message);
          setLoginUrl('');
          const result = await invokeScanCodexAccounts();
          applyScanResult(result, status.accountKey);
        } else if (status.status === 'error') {
          setLoginStatus('error');
          setLoginMessage(status.message);
          setLoginUrl('');
        }
      } catch {
        // Keep waiting; the user can retry or close the dialog.
      }
    }, 2000);

    return () => window.clearInterval(timer);
  }, [loginDialogOpen, loginStatus, loginTab]);

  useEffect(() => {
    if (!reauthorizeAccount || reauthorizeStatus !== 'waiting') {
      return;
    }

    const timer = window.setInterval(async () => {
      try {
        const status = await invokeCodexOAuthLoginStatus();
        if (status.status === 'success') {
          const result = await invokeScanCodexAccounts();
          const authorizedAccount = result.accounts.find((account) => account.accountKey === status.accountKey);
          const returnedEmail = status.accountEmail || authorizedAccount?.email || '';
          const sameAccount =
            authorizedAccount?.accountKey === reauthorizeAccount.accountKey ||
            returnedEmail.toLowerCase() === reauthorizeAccount.email.toLowerCase();

          applyScanResult(result, sameAccount ? reauthorizeAccount.accountKey : status.accountKey);
          setReauthorizeStatus(sameAccount ? 'success' : 'error');
          setReauthorizeUrl('');
          setReauthorizeMessage(
            sameAccount
              ? '已重新授权该账号，新的登录快照已保存。'
              : `授权完成，但返回账号不是 ${reauthorizeAccount.email}。已按新账号保存，原账号未覆盖。`,
          );
        } else if (status.status === 'error') {
          setReauthorizeStatus('error');
          setReauthorizeMessage(status.message);
          setReauthorizeUrl('');
        }
      } catch {
        // Keep waiting; the user can retry or close the dialog.
      }
    }, 2000);

    return () => window.clearInterval(timer);
  }, [reauthorizeAccount, reauthorizeStatus]);

  useEffect(() => () => {
    if (copyToastTimerRef.current) {
      window.clearTimeout(copyToastTimerRef.current);
    }
  }, []);

  const accounts = useMemo<AccountListItem[]>(() => {
    const normalizedKeyword = keyword.trim().toLowerCase();
    const filteredAccounts = normalizedKeyword
      ? scanResult.accounts.filter((account) =>
        `${account.email} ${account.name} ${account.workspaceName} ${account.plan}`.toLowerCase().includes(normalizedKeyword),
      )
      : scanResult.accounts;

    return filteredAccounts.map((account) => ({
      ...account,
      connectionStatus: resolveAccountConnectionStatus(account, usageFailedAccountKey),
    }));
  }, [keyword, scanResult.accounts, usageFailedAccountKey]);

  const selectedAccount = accounts.find((account) => account.accountKey === selectedAccountId) ?? accounts[0];

  const applyScanResult = (result: CodexAccountScanResult, preferredAccountKey?: string) => {
    const nextSelectedAccountId = preferredAccountKey || result.currentAccountId || result.accounts[0]?.accountKey || '';

    cachedScanResult = result;
    cachedSelectedAccountId = nextSelectedAccountId;
    setScanResult(result);
    setSelectedAccountId(nextSelectedAccountId);
  };

  const handleSelectAccount = (accountKey: string) => {
    cachedSelectedAccountId = accountKey;
    setSelectedAccountId(accountKey);
  };

  const shouldRefreshCurrentAccountUsage = (result: CodexAccountScanResult) => {
    const accountKey = result.currentAccountId;
    if (!accountKey) {
      return false;
    }

    const currentAccount = result.accounts.find((account) => account.accountKey === accountKey);
    const lastUsageAt = Number(currentAccount?.lastUsageAt);
    if (!Number.isFinite(lastUsageAt) || lastUsageAt <= 0) {
      return true;
    }

    const refreshSeconds = appSettings.account_usage_refresh_seconds || 60;
    return Math.floor(Date.now() / 1000) - lastUsageAt >= refreshSeconds;
  };

  const refreshCurrentAccountUsageInBackground = async (result: CodexAccountScanResult) => {
    const accountKey = result.currentAccountId;

    if (!accountKey || usageRefreshInFlightRef.current || !shouldRefreshCurrentAccountUsage(result)) {
      return;
    }

    usageRefreshInFlightRef.current = true;
    setUsageLoading(true);
    try {
      const usageResult = await invokeRefreshCodexAccountUsage(accountKey, false);
      setUsageFailedAccountKey((failedAccountKey) => failedAccountKey === accountKey ? '' : failedAccountKey);
      applyScanResult(usageResult.scan, cachedSelectedAccountId || accountKey);
    } catch {
      setUsageFailedAccountKey(accountKey);
      // Network quota refresh is best-effort and must not block local account scanning.
    } finally {
      usageRefreshInFlightRef.current = false;
      setUsageLoading(false);
    }
  };

  const showToast = (message: string, tone: ToastTone = 'success') => {
    setToast({ message, tone });
    if (copyToastTimerRef.current) {
      window.clearTimeout(copyToastTimerRef.current);
    }
    copyToastTimerRef.current = window.setTimeout(() => {
      setToast(null);
      copyToastTimerRef.current = null;
    }, 1600);
  };

  async function handleInitialScan() {
    setLoading(true);
    setError('');

    try {
      if (!initialScanPromise) {
        initialScanPromise = invokeScanCodexAccounts().finally(() => {
          initialScanPromise = null;
        });
      }

      const result = await initialScanPromise;
      applyScanResult(result, cachedSelectedAccountId);
      void refreshCurrentAccountUsageInBackground(result);
    } catch (scanError) {
      setError(formatUnknownError(scanError));
    } finally {
      setLoading(false);
    }
  }

  const runAccountOperation = async (
    operation: () => Promise<{ message: string; path?: string; scan: CodexAccountScanResult }>,
    preferredAccountKey?: string,
    options?: { silent?: boolean },
  ) => {
    setLoading(true);
    setError('');

    try {
      const result = await operation();
      applyScanResult(result.scan, preferredAccountKey);
      setOperationMessage(options?.silent ? '' : result.path ? `${result.message} ${result.path}` : result.message);
      return true;
    } catch (operationError) {
      setError(formatUnknownError(operationError));
      return false;
    } finally {
      setLoading(false);
    }
  };

  const handleExportAccounts = async () => {
    setLoading(true);
    setError('');

    try {
      const result = await invokeExportCodexAccounts();
      applyScanResult(result.scan, selectedAccountId);
      setOperationMessage('');
      setExportSuccess({
        message: '账号导出成功',
        path: result.path || '',
      });
    } catch (operationError) {
      setError(formatUnknownError(operationError));
    } finally {
      setLoading(false);
    }
  };

  async function handleListRefresh() {
    setLoading(true);
    setError('');

    try {
      const result = await invokeScanCodexAccounts();
      applyScanResult(result, selectedAccountId);
      setOperationMessage('');
      void refreshCurrentAccountUsageInBackground(result);
      showToast('账号列表已刷新');
    } catch (scanError) {
      setError(formatUnknownError(scanError));
    } finally {
      setLoading(false);
    }
  }

  const handleCurrentUsageRefresh = async () => {
    if (!selectedAccount) {
      setOperationMessage('请先选择一个账号。');
      return;
    }

    setUsageLoading(true);
    setError('');

    try {
      const result = await invokeRefreshCodexAccountUsage(selectedAccount.accountKey);
      setUsageFailedAccountKey((failedAccountKey) => failedAccountKey === selectedAccount.accountKey ? '' : failedAccountKey);
      applyScanResult(result.scan, selectedAccount.accountKey);
      setOperationMessage('');
      showToast(result.message || '额度刷新成功');
    } catch (usageError) {
      setUsageFailedAccountKey(selectedAccount.accountKey);
      showToast(formatUnknownError(usageError), 'error');
    } finally {
      setUsageLoading(false);
    }
  };

  const handleRefreshToken = async (account: CodexAccount) => {
    if (tokenRefreshLoading) return;
    setTokenRefreshLoading(true);
    setError('');
    try {
      const result = await invokeRefreshCodexAccountToken(account.accountKey);
      applyScanResult(result.scan, account.accountKey);
      showToast(result.message || 'Token 已刷新');
    } catch (tokenError) {
      try {
        const scan = await invokeScanCodexAccounts();
        applyScanResult(scan, account.accountKey);
      } catch {
        // Keep the original refresh error visible when the follow-up scan fails.
      }
      showToast(formatUnknownError(tokenError), 'error');
    } finally {
      setTokenRefreshLoading(false);
    }
  };

  const handleExpirationSave = async (account: CodexAccount, expiresAt: string | null) => {
    setLoading(true);
    setError('');

    try {
      const result = await invokeUpdateCodexAccountExpiration(account.accountKey, account.email, expiresAt);
      applyScanResult(result.scan, account.accountKey);
      setOperationMessage('');
      showToast(result.message || '账号到期时间已保存');
    } catch (expirationError) {
      showToast(formatUnknownError(expirationError), 'error');
    } finally {
      setLoading(false);
    }
  };

  const handleStartWebLogin = async () => {
    setLoginBusy(true);
    setError('');
    setLoginStatus('idle');
    setLoginMessage('');
    setLoginUrl('');

    try {
      const listenerStatus = await refreshCallbackListenerStatus();
      if (!listenerStatus.running) {
        setLoginStatus('error');
        setLoginMessage(listenerStatus.message);
        return;
      }

      const result = await invokeStartCodexAccountLogin();
      applyScanResult(result.scan, selectedAccountId);
      setLoginStatus('waiting');
      setLoginMessage(result.message);
      if (result.path) {
        setLoginUrl(result.path);
        await invokeOpenExternalUrl(result.path);
      }
    } catch (loginError) {
      setLoginStatus('error');
      setLoginMessage(formatUnknownError(loginError));
    } finally {
      setLoginBusy(false);
    }
  };

  const refreshCallbackListenerStatus = async () => {
    const status = await invokeCodexOAuthCallbackListenerStatus();
    setCallbackListenerStatus(status);
    return status;
  };

  const handleClientLoginStart = async () => {
    setLoginBusy(true);
    setError('');

    try {
      const result = await invokeStartCodexClientLogin();
      applyScanResult(result.scan, selectedAccountId);
      setClientLoginStarted(true);
      setLoginStatus('waiting');
      setLoginMessage(result.message);
    } catch (clientStartError) {
      setLoginStatus('error');
      setLoginMessage(formatUnknownError(clientStartError));
    } finally {
      setLoginBusy(false);
    }
  };

  const handleClientLoginSuccess = async () => {
    setLoginBusy(true);
    setError('');

    try {
      const result = await invokeImportCurrentCodexAccount();
      applyScanResult(result.scan, result.scan.currentAccountId);
      setLoginStatus('success');
      setLoginMessage(result.message);
      setOperationMessage(result.message);
      setClientLoginStarted(true);
      setLoginTab('client');
      setLoginDialogOpen(true);
    } catch (clientLoginError) {
      setLoginStatus('error');
      setLoginMessage(formatUnknownError(clientLoginError));
    } finally {
      setLoginBusy(false);
    }
  };

  const handleSessionLoginStart = async () => {
    setLoginBusy(true);
    setError('');
    setSessionJson('');

    try {
      await invokeOpenExternalUrl(CHATGPT_LOGIN_URL);
      setSessionLoginStarted(true);
      setLoginStatus('waiting');
      setLoginMessage('请在浏览器完成 ChatGPT 登录，然后打开 Session 接口并粘贴返回的 JSON。');
    } catch (sessionStartError) {
      setLoginStatus('error');
      setLoginMessage(formatUnknownError(sessionStartError));
    } finally {
      setLoginBusy(false);
    }
  };

  const handleOpenSessionUrl = async () => {
    try {
      await invokeOpenExternalUrl(CHATGPT_SESSION_URL);
    } catch (openError) {
      setLoginStatus('error');
      setLoginMessage(formatUnknownError(openError));
    }
  };

  const handleSessionLoginSuccess = async () => {
    if (!sessionJson.trim()) {
      setLoginStatus('error');
      setLoginMessage('请先粘贴 https://chatgpt.com/api/auth/session 返回的 JSON。');
      return;
    }

    setLoginBusy(true);
    setError('');
    setLoginStatus('waiting');
    setLoginMessage('正在保存 ChatGPT Session 登录信息...');

    try {
      await waitForNextFrame();
      const result = await invokeImportChatGptSessionAccount(sessionJson);
      applyScanResult(result.scan, result.scan.currentAccountId);
      setLoginStatus('success');
      setLoginMessage(result.message);
      setOperationMessage(result.message);
      setSessionLoginStarted(true);
    } catch (sessionLoginError) {
      setLoginStatus('error');
      setLoginMessage(formatUnknownError(sessionLoginError));
    } finally {
      setLoginBusy(false);
    }
  };

  const handleSessionLoginFail = () => {
    setSessionLoginStarted(false);
    setSessionJson('');
    setLoginStatus('idle');
    setLoginMessage('');
  };

  const handleClearSessionJson = () => {
    setSessionJson('');
    setLoginStatus('idle');
    setLoginMessage('');
    setSessionLoginStarted(false);
  };

  const handleCpaLoginStart = async () => {
    setLoginBusy(true);
    setError('');
    setCpaLoginStarted(true);
    setLoginStatus('idle');
    setLoginMessage('');
    setCpaText('');
    setLoginBusy(false);
  };

  const handleCpaLoginSuccess = async () => {
    if (!cpaText.trim()) {
      setLoginStatus('error');
      setLoginMessage('请先粘贴 CPA JSON。');
      return;
    }
    setLoginBusy(true);
    setError('');
    try {
      const result = await invokeImportCpaAccount(cpaText);
      const newScan = result.scan;
      cachedScanResult = newScan;
      setScanResult(newScan);
      setLoginStatus('success');
      setLoginMessage(result.message);
      setOperationMessage(result.message);
    } catch (cpaError) {
      setLoginStatus('error');
      setLoginMessage(formatUnknownError(cpaError));
    } finally {
      setLoginBusy(false);
    }
  };

  const handleCpaLoginFail = () => {
    setCpaLoginStarted(false);
    setCpaText('');
    setLoginStatus('idle');
    setLoginMessage('');
  };

const openLoginDialog = () => {
    setLoginDialogOpen(true);
    setLoginTab('web');
    setLoginUrl('');
    setLoginStatus('idle');
    setLoginMessage('');
    setClientLoginStarted(false);
    setSessionLoginStarted(false);
    setCpaLoginStarted(false);
    setCpaText('');
    setSessionJson('');
  };

  const openReauthorizeDialog = (account: CodexAccount) => {
    setReauthorizeAccount(account);
    setReauthorizeUrl('');
    setReauthorizeStatus('idle');
    setReauthorizeMessage('');
    setReauthorizeBusy(false);
  };

  const handleStartReauthorize = async () => {
    if (!reauthorizeAccount) {
      return;
    }

    setReauthorizeBusy(true);
    setError('');
    setReauthorizeStatus('idle');
    setReauthorizeMessage('');
    setReauthorizeUrl('');

    try {
      const listenerStatus = await refreshCallbackListenerStatus();
      if (!listenerStatus.running) {
        setReauthorizeStatus('error');
        setReauthorizeMessage(listenerStatus.message);
        return;
      }

      const result = await invokeStartCodexAccountLogin();
      applyScanResult(result.scan, reauthorizeAccount.accountKey);
      setReauthorizeStatus('waiting');
      setReauthorizeMessage(`请在浏览器中重新授权 ${reauthorizeAccount.email}。`);
      if (result.path) {
        setReauthorizeUrl(result.path);
        await invokeOpenExternalUrl(result.path);
      }
    } catch (loginError) {
      setReauthorizeStatus('error');
      setReauthorizeMessage(formatUnknownError(loginError));
    } finally {
      setReauthorizeBusy(false);
    }
  };

  const handleRemoveSnapshot = async (account: CodexAccount) => {
    const confirmed = window.confirm(`确认移除账号 ${account.email} 的本地快照吗？此操作会从账号列表删除该项。`);
    if (!confirmed) {
      return;
    }

    const removed = await runAccountOperation(() => invokeRemoveCodexAccountSnapshot(account.accountKey), undefined, { silent: true });
    if (removed) {
      showToast('已成功移除');
    }
  };

  const handleCopyEmail = async (account: CodexAccount) => {
    await navigator.clipboard.writeText(account.email);
    showToast('邮箱已复制');
  };

  const handleSwitchAccount = async (account: CodexAccount) => {
    setSwitchProgress({
      account,
      switchStatus: 'running',
      restartStatus: 'pending',
      message: '正在切换账号...',
      restartMessage: '',
    });
    setLoading(true);
    setError('');
    setOperationMessage('');

    try {
      await waitForNextFrame();
      const switchResult = await invokeSwitchCodexAccount(account.accountKey);
      applyScanResult(switchResult.scan, account.accountKey);
      setSwitchProgress((current) => current && {
        ...current,
        switchStatus: 'success',
        restartStatus: 'running',
        message: '切换完成',
        restartMessage: '正在重启应用...',
      });

      const restartResult = await invokeRestartCodexApp();
      setSwitchProgress(null);
      showToast(restartResult.success ? '账号切换成功，Codex 已重启' : '账号切换成功，请手动重启 Codex', restartResult.success ? 'success' : 'error');
    } catch (switchError) {
      const message = formatUnknownError(switchError);
      setSwitchProgress((current) => current && {
        ...current,
        switchStatus: 'error',
        restartStatus: 'pending',
        message,
      });
      setError(message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
      <section className="shrink-0 flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-slate-500">管理 Codex OAuth 账号、切换本地登录快照，并按账号刷新真实额度。</p>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="secondary" onClick={openLoginDialog} disabled={loading}>
            添加账号
          </Button>
          <Button onClick={handleListRefresh} disabled={loading}>
            {loading ? '扫描中' : '刷新'}
          </Button>
        </div>
      </section>

      <div className="shrink-0 flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap gap-3">
          <input
            className="w-56 rounded-lg border border-slate-200 bg-white px-4 py-2 text-sm outline-none focus:border-indigo-400"
            placeholder="搜索账号"
            value={keyword}
            onChange={(event) => setKeyword(event.target.value)}
          />
          <Button variant="secondary" onClick={handleExportAccounts} disabled={loading || scanResult.accounts.length === 0}>
            导出账号
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span className={scanResult.apiHealthy ? 'text-emerald-600' : 'text-slate-500'}>{scanResult.apiHealthy ? 'API 通信正常' : '等待账号数据'}</span>
          <Button variant="secondary" onClick={handleCurrentUsageRefresh} disabled={usageLoading || !selectedAccount} className="px-3 py-1 text-xs">
            {usageLoading ? '拉取中' : '刷新额度'}
          </Button>
        </div>
      </div>

      {error && <div className="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">{error}</div>}
      {operationMessage && <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">{operationMessage}</div>}

      <div className="grid min-h-0 flex-1 grid-cols-[300px_minmax(0,1fr)] gap-4 overflow-hidden">
        <Card className="min-h-0 overflow-hidden">
          <CardContent className="flex h-full min-h-0 flex-col py-4">
            <div className="mb-3 flex shrink-0 items-center justify-between">
              <div className="text-sm font-semibold text-slate-700">账号列表</div>
              <div className="text-xs text-slate-400">{scanResult.accounts.length} 个</div>
            </div>
            <div className="min-h-0 flex-1 space-y-2 overflow-y-auto pr-1">
              {accounts.map((account) => (
                <button
                  key={account.accountKey}
                  className={`w-full rounded-lg border px-3 py-3 text-left transition ${
                    selectedAccount?.accountKey === account.accountKey
                      ? 'border-indigo-500 bg-indigo-600 text-white'
                      : 'border-slate-200 bg-white text-slate-700 hover:border-indigo-200 hover:bg-slate-50'
                  }`}
                  onClick={() => handleSelectAccount(account.accountKey)}
                >
                  <div className="flex items-center gap-2">
                    <span className={`h-2.5 w-2.5 rounded-full ${accountConnectionDotClass(account.connectionStatus)}`} title={accountConnectionStatusLabel(account.connectionStatus)} />
                    <span className="min-w-0 flex-1 truncate text-sm font-bold">{account.email}</span>
                    {account.isCurrent && <span className="rounded-full bg-white/20 px-2 py-0.5 text-xs font-semibold">当前</span>}
                  </div>
                  <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs opacity-90">
                    <span>{account.plan}</span>
                    {getAccountUsageWindows(account).slice(0, 2).map((window, index) => (
                      <span key={`${window.limitWindowSeconds ?? 'window'}-${index}`}>
                        {formatUsageWindowLabel(window.limitWindowSeconds)}剩余 {formatQuotaPercent(window.remainingPercent)}
                      </span>
                    ))}
                  </div>
                </button>
              ))}
              {accounts.length === 0 && <div className="rounded-lg bg-slate-50 px-4 py-8 text-center text-sm text-slate-400">未检测到 Codex 账号。</div>}
            </div>
          </CardContent>
        </Card>

        <Card className="min-h-0 overflow-hidden">
          <CardContent className="h-full py-4">
            {selectedAccount ? (
              <AccountDetail
                account={selectedAccount}
                loading={loading}
                tokenRefreshLoading={tokenRefreshLoading}
                onCopyEmail={handleCopyEmail}
                onRemoveSnapshot={handleRemoveSnapshot}
                onReauthorize={openReauthorizeDialog}
                onRefreshToken={handleRefreshToken}
                onSaveExpiration={handleExpirationSave}
                onSwitchAccount={handleSwitchAccount}
              />
            ) : (
              <div className="py-20 text-center text-sm text-slate-400">请选择账号。</div>
            )}
          </CardContent>
        </Card>
      </div>

      {loginDialogOpen && (
        <AccountLoginDialog
          activeTab={loginTab}
          busy={loginBusy}
          clientLoginStarted={clientLoginStarted}
          loginMessage={loginMessage}
          loginStatus={loginStatus}
          loginUrl={loginUrl}
          sessionJson={sessionJson}
          cpaLoginStarted={cpaLoginStarted}
          cpaText={cpaText}
          sessionLoginStarted={sessionLoginStarted}

          onCpaFail={handleCpaLoginFail}
          onCpaStart={handleCpaLoginStart}
          onCpaSuccess={handleCpaLoginSuccess}
          onCpaTextChange={setCpaText}
          onClientFail={() => { setLoginStatus('error'); setLoginMessage('客户端登录未完成。'); }}
          onClientStart={handleClientLoginStart}
          onClientSuccess={handleClientLoginSuccess}
          onClose={() => setLoginDialogOpen(false)}
          onOpenUrl={() => loginUrl && invokeOpenExternalUrl(loginUrl)}
          onOpenSessionUrl={handleOpenSessionUrl}
          onSessionClear={handleClearSessionJson}
          onSessionFail={handleSessionLoginFail}
          onSessionJsonChange={setSessionJson}
          onSessionStart={handleSessionLoginStart}
          onSessionSuccess={handleSessionLoginSuccess}
          onStartWebLogin={handleStartWebLogin}
          onTabChange={(tab) => {
            setLoginTab(tab);
            setLoginStatus('idle');
            setLoginMessage('');
            setClientLoginStarted(false);
            setSessionLoginStarted(false);
            setSessionJson('');
            setCpaLoginStarted(false);
            setCpaText('');
          }}
        />
      )}
      {reauthorizeAccount && (
        <AccountReauthorizeDialog
          account={reauthorizeAccount}
          busy={reauthorizeBusy}
          loginMessage={reauthorizeMessage}
          loginStatus={reauthorizeStatus}
          loginUrl={reauthorizeUrl}
          onClose={() => setReauthorizeAccount(null)}
          onOpenUrl={() => reauthorizeUrl && invokeOpenExternalUrl(reauthorizeUrl)}
          onStart={handleStartReauthorize}
        />
      )}
      {exportSuccess && <ExportSuccessDialog state={exportSuccess} onClose={() => setExportSuccess(null)} />}
      {switchProgress && (
        <SwitchProgressDialog
          state={switchProgress}
          onClose={() => setSwitchProgress(null)}
        />
      )}
      {toast && (
        <div className={`fixed left-1/2 top-6 z-50 -translate-x-1/2 rounded-lg border px-4 py-2 text-sm font-semibold shadow-lg shadow-slate-950/10 ${
          toast.tone === 'error'
            ? 'border-rose-200 bg-rose-50 text-rose-700'
            : 'border-emerald-200 bg-white text-emerald-700'
        }`}>
          {toast.message}
        </div>
      )}
    </div>
  );
}

function SwitchProgressDialog({ state, onClose }: { state: SwitchProgressState; onClose: () => void }) {
  const running = state.switchStatus === 'running' || state.restartStatus === 'running';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-lg rounded-xl bg-white p-6 shadow-2xl shadow-slate-950/20">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-xl font-bold text-slate-950">切换账号</h3>
            <p className="mt-1 text-sm text-slate-500">{state.account.email}</p>
          </div>
          <Button variant="ghost" onClick={onClose} disabled={running}>关闭</Button>
        </div>
        <div className="mt-5 space-y-4">
          <ProgressStep index={1} title="切换完成" status={state.switchStatus} message={state.message} />
          <ProgressStep index={2} title="重启应用" status={state.restartStatus} message={state.restartMessage || '等待切换完成'} />
        </div>
      </div>
    </div>
  );
}

function resolveAccountConnectionStatus(account: CodexAccount, usageFailedAccountKey: string): AccountConnectionStatus {
  if (usageFailedAccountKey === account.accountKey) {
    return 'error';
  }

  return account.isCurrent ? 'online' : 'offline';
}

function accountConnectionDotClass(status: AccountConnectionStatus) {
  if (status === 'error') {
    return 'bg-rose-500';
  }

  if (status === 'online') {
    return 'bg-emerald-400';
  }

  return 'bg-slate-300';
}

function accountConnectionStatusLabel(status: AccountConnectionStatus) {
  if (status === 'error') {
    return '异常';
  }

  if (status === 'online') {
    return '在线';
  }

  return '离线';
}

function ProgressStep({ index, title, status, message }: { index: number; title: string; status: SwitchStepStatus; message: string }) {
  const tone = status === 'success' ? 'bg-emerald-500 text-white' : status === 'error' ? 'bg-rose-500 text-white' : status === 'running' ? 'bg-indigo-600 text-white' : 'bg-slate-100 text-slate-500';
  const label = status === 'success' ? '完成' : status === 'error' ? '失败' : status === 'running' ? '处理中' : '等待';

  return (
    <div className="flex gap-3 rounded-lg border border-slate-200 bg-slate-50 p-4">
      <span className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-xs font-bold ${tone}`}>{status === 'success' ? '✓' : index}</span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between gap-3">
          <div className="font-semibold text-slate-900">{title}</div>
          <span className="text-xs text-slate-500">{label}</span>
        </div>
        <div className={`mt-1 text-sm ${status === 'error' ? 'text-rose-600' : 'text-slate-500'}`}>{message}</div>
      </div>
    </div>
  );
}

function ExportSuccessDialog({ state, onClose }: { state: ExportSuccessState; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-lg rounded-xl bg-white p-6 shadow-2xl shadow-slate-950/20">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-xl font-bold text-slate-950">{state.message}</h3>
            <p className="mt-1 text-sm text-slate-500">导出目录路径如下。</p>
          </div>
          <Button variant="ghost" onClick={onClose}>关闭</Button>
        </div>
        <div className="mt-5 rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 font-mono text-xs text-slate-700 break-all">
          {state.path || '未返回导出目录'}
        </div>
      </div>
    </div>
  );
}

function AccountLoginDialog({
  activeTab,
  busy,
  clientLoginStarted,
  loginMessage,
  loginStatus,
  loginUrl,
  sessionJson,
  cpaLoginStarted,
  cpaText,
  sessionLoginStarted,
  onCpaFail,
  onCpaStart,
  onCpaSuccess,
  onCpaTextChange,
  onClientFail,
  onClientStart,
  onClientSuccess,
  onClose,
  onOpenUrl,
  onOpenSessionUrl,
  onSessionClear,
  onSessionFail,
  onSessionJsonChange,
  onSessionStart,
  onSessionSuccess,
  onStartWebLogin,
  onTabChange,
}: {
  activeTab: LoginTab;
  busy: boolean;
  clientLoginStarted: boolean;
  cpaLoginStarted: boolean;
  cpaText: string;
  loginMessage: string;
  loginStatus: LoginStatus;
  loginUrl: string;
  sessionJson: string;
  sessionLoginStarted: boolean;
  onCpaFail: () => void;
  onCpaStart: () => void;
  onCpaSuccess: () => void;
  onCpaTextChange: (value: string) => void;
  onClientFail: () => void;
  onClientStart: () => void;
  onClientSuccess: () => void;
  onClose: () => void;
  onOpenUrl: () => void;
  onOpenSessionUrl: () => void;
  onSessionClear: () => void;
  onSessionFail: () => void;
  onSessionJsonChange: (value: string) => void;
  onSessionStart: () => void;
  onSessionSuccess: () => void;
  onStartWebLogin: () => void;
  onTabChange: (tab: LoginTab) => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-2xl overflow-hidden rounded-xl bg-white shadow-2xl shadow-slate-950/20">
        <div className="border-b border-slate-100 px-6 py-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h3 className="text-xl font-bold text-slate-950">添加账号</h3>
              <p className="mt-1 text-sm text-slate-500">选择登录方式后按步骤完成授权。</p>
            </div>
            <Button variant="ghost" onClick={onClose}>关闭</Button>
          </div>
          <div className="mt-4 inline-flex rounded-lg bg-slate-100 p-1">
            <button className={`rounded-md px-4 py-2 text-sm font-semibold ${activeTab === 'web' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500'}`} onClick={() => onTabChange('web')}>
              网页 Codex 登录
            </button>
            <button className={`rounded-md px-4 py-2 text-sm font-semibold ${activeTab === 'client' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500'}`} onClick={() => onTabChange('client')}>
              客户端登录
            </button>
            <button className={`rounded-md px-4 py-2 text-sm font-semibold ${activeTab === 'cpa' ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-500'}`} onClick={() => onTabChange('cpa')}>
              CPA 登录
            </button>
          </div>
        </div>

        <div className="space-y-4 px-6 py-5">
          {activeTab === 'web' ? (
            <>
              <LoginStep index={1} title="打开授权链接" active={loginStatus === 'idle'} done={loginStatus !== 'idle'} />
              <div className="rounded-lg border border-slate-200 bg-slate-50 p-3">
                <div className="flex flex-wrap gap-2">
                  <Button onClick={onStartWebLogin} disabled={busy}>{busy ? '准备中' : loginStatus === 'idle' ? '打开授权链接' : '重新生成链接'}</Button>
                  <Button variant="secondary" onClick={onOpenUrl} disabled={!loginUrl}>重新打开</Button>
                </div>
                {loginUrl && <div className="mt-3 break-all rounded-md bg-white px-3 py-2 font-mono text-xs text-slate-600">{loginUrl}</div>}
              </div>
              <LoginStep index={2} title="等待回调接收..." active={loginStatus === 'waiting'} done={loginStatus === 'success'} />
              <LoginStep index={3} title="接收到回调，保存账号快照" active={loginStatus === 'success'} done={loginStatus === 'success'} />
            </>
          ) : activeTab === 'client' ? (
            <>
              {!clientLoginStarted ? (
                <div className="rounded-lg border border-slate-200 bg-slate-50 p-3">
                  <Button onClick={onClientStart} disabled={busy}>开始登录</Button>
                </div>
              ) : (
                <>
                  <LoginStep index={1} title="退出客户端账号" active={loginStatus === 'waiting'} done={loginStatus === 'success'} />
                  <LoginStep index={2} title="等待你在 Codex 客户端重新登录" active={loginStatus === 'waiting'} done={loginStatus === 'success'} />
                  <div className="rounded-lg border border-slate-200 bg-slate-50 p-3">
                    <div className="flex flex-wrap gap-2">
                      <Button onClick={onClientSuccess} disabled={busy}>{busy ? '保存中' : '登录成功'}</Button>
                      <Button variant="secondary" onClick={onClientFail} disabled={busy}>登录失败</Button>
                    </div>
                  </div>
                </>
              )}
            </>
          ) : (
            <>
              {!cpaLoginStarted ? (
                <div className="rounded-lg border border-slate-200 bg-slate-50 p-3">
                  <Button onClick={onCpaStart} disabled={busy}>开始登录</Button>
                </div>
              ) : (
                <>
                  <LoginStep index={1} title="粘贴 CPA JSON 并保存账号" active={loginStatus === 'waiting'} done={loginStatus === 'success'} />
                  <div className="space-y-3 rounded-lg border border-slate-200 bg-slate-50 p-3">
                    <div className="flex flex-wrap gap-2">
                      <Button onClick={onCpaSuccess} disabled={busy}>{busy ? '保存中' : '登录成功'}</Button>
                      <Button variant="secondary" onClick={onCpaFail} disabled={busy}>登录失败</Button>
                    </div>
                    <textarea
                      className="min-h-40 w-full resize-y rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-xs text-slate-700 outline-none focus:border-indigo-400"
                      onChange={(event) => onCpaTextChange(event.target.value)}
                      placeholder="粘贴 CPA JSON 内容"
                      value={cpaText}
                    />
                  </div>
                </>
              )}
            </>
          )}

          {loginMessage && (
            <div className={`rounded-lg border px-4 py-3 text-sm ${loginStatus === 'error' ? 'border-rose-200 bg-rose-50 text-rose-700' : 'border-emerald-200 bg-emerald-50 text-emerald-700'}`}>
              {loginMessage}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function AccountReauthorizeDialog({
  account,
  busy,
  loginMessage,
  loginStatus,
  loginUrl,
  onClose,
  onOpenUrl,
  onStart,
}: {
  account: CodexAccount;
  busy: boolean;
  loginMessage: string;
  loginStatus: LoginStatus;
  loginUrl: string;
  onClose: () => void;
  onOpenUrl: () => void;
  onStart: () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="w-full max-w-2xl overflow-hidden rounded-xl bg-white shadow-2xl shadow-slate-950/20">
        <div className="border-b border-slate-100 px-6 py-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h3 className="text-xl font-bold text-slate-950">重新授权账号</h3>
              <p className="mt-1 text-sm text-slate-500">目标邮箱：{account.email}</p>
            </div>
            <Button variant="ghost" onClick={onClose}>关闭</Button>
          </div>
        </div>

        <div className="space-y-4 px-6 py-5">
          <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
            这是一套独立于“添加账号”的重新授权流程，用于修复该账号 401 或 token 失效问题。请确认浏览器中选择的是上方目标邮箱。
          </div>
          <LoginStep index={1} title="打开授权链接" active={loginStatus === 'idle'} done={loginStatus !== 'idle'} />
          <div className="rounded-lg border border-slate-200 bg-slate-50 p-3">
            <div className="flex flex-wrap gap-2">
              <Button onClick={onStart} disabled={busy}>{busy ? '准备中' : '重新授权'}</Button>
              <Button variant="secondary" onClick={onOpenUrl} disabled={!loginUrl}>重新打开</Button>
            </div>
            {loginUrl && <div className="mt-3 break-all rounded-md bg-white px-3 py-2 font-mono text-xs text-slate-600">{loginUrl}</div>}
          </div>
          <LoginStep index={2} title="等待浏览器授权回调" active={loginStatus === 'waiting'} done={loginStatus === 'success'} />
          <LoginStep index={3} title="保存新的账号快照" active={loginStatus === 'success'} done={loginStatus === 'success'} />
          {loginMessage && (
            <div className={`rounded-lg border px-4 py-3 text-sm ${loginStatus === 'error' ? 'border-rose-200 bg-rose-50 text-rose-700' : 'border-emerald-200 bg-emerald-50 text-emerald-700'}`}>
              {loginMessage}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function LoginStep({ index, title, active, done }: { index: number; title: string; active: boolean; done: boolean }) {
  return (
    <div className="flex items-center gap-3 text-sm">
      <span className={`flex h-7 w-7 items-center justify-center rounded-full text-xs font-bold ${done ? 'bg-emerald-500 text-white' : active ? 'bg-indigo-600 text-white' : 'bg-slate-100 text-slate-500'}`}>{done ? '✓' : index}</span>
      <span className={done ? 'text-slate-700' : active ? 'font-semibold text-slate-900' : 'text-slate-500'}>{title}</span>
    </div>
  );
}

function AccountDetail({
  account,
  loading,
  tokenRefreshLoading,
  onCopyEmail,
  onRemoveSnapshot,
  onReauthorize,
  onRefreshToken,
  onSaveExpiration,
  onSwitchAccount,
}: {
  account: CodexAccount;
  loading: boolean;
  tokenRefreshLoading: boolean;
  onCopyEmail: (account: CodexAccount) => void;
  onRemoveSnapshot: (account: CodexAccount) => void;
  onReauthorize: (account: CodexAccount) => void;
  onRefreshToken: (account: CodexAccount) => void;
  onSaveExpiration: (account: CodexAccount, expiresAt: string | null) => Promise<void>;
  onSwitchAccount: (account: CodexAccount) => void;
}) {
  const [expirationDialogOpen, setExpirationDialogOpen] = useState(false);
  const [expirationValue, setExpirationValue] = useState(accountExpirationDateInput(account.expiresAt));
  const [expirationSaving, setExpirationSaving] = useState(false);
  const expirationDisplay = getExpirationDisplay(account.expiresAt);
  const openExpirationDialog = () => {
    setExpirationValue(accountExpirationDateInput(account.expiresAt));
    setExpirationDialogOpen(true);
  };
  const handleExpirationSave = async () => {
    setExpirationSaving(true);
    try {
      await onSaveExpiration(account, expirationValue ? expirationDateInputToIso(expirationValue) : null);
      setExpirationDialogOpen(false);
    } finally {
      setExpirationSaving(false);
    }
  };
  const handleExpirationClear = async () => {
    setExpirationValue('');
    setExpirationSaving(true);
    try {
      await onSaveExpiration(account, null);
      setExpirationDialogOpen(false);
    } finally {
      setExpirationSaving(false);
    }
  };
  const tokenExpiryDisplay = account.tokenExpiresAt
    ? formatDateTime(account.tokenExpiresAt) + (account.tokenExpired ? ' (已过期)' : account.tokenNeedsRefresh ? ' (即将到期)' : '')
    : '-';
  const detailItems = [
    { label: '工作区', value: account.workspaceName },
    { label: '认证方式', value: account.authMode },
    { label: '订阅状态', value: normalizePlanLabel(account.plan) || account.plan || 'Unknown' },
    { label: '自动续费', value: account.autoRenew ? '已开启' : '未开启' },
  ];
  const quotaWindows = getAccountUsageWindows(account);

  return (
    <div className="space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-sm text-slate-500">已选账号</div>
          <h3 className="mt-0.5 truncate text-2xl font-bold leading-7 text-slate-950">{account.email}</h3>
          <div className="mt-1.5 flex items-center gap-2 text-xs text-slate-400">
            <span>快照密钥</span>
            <span className="max-w-[520px] truncate font-mono">{account.accessTokenMask}</span>
          </div>
        </div>
        <Badge tone="blue">{account.plan}</Badge>
      </div>

      <div className="grid grid-cols-2 gap-3">
        {quotaWindows.map((window, index) => (
          <QuotaCard
            key={`${window.limitWindowSeconds ?? 'window'}-${index}`}
            label={`${formatUsageWindowLabel(window.limitWindowSeconds)}剩余`}
            percent={window.remainingPercent}
            resetAt={window.resetAt}
          />
        ))}
      </div>

      <div className="grid grid-cols-2 gap-2 rounded-lg border border-slate-200 p-3">
        <InfoField label="用户" value={account.name} />
        {detailItems.slice(0, 1).map((item) => (
          <InfoField key={item.label} label={item.label} value={item.value} />
        ))}
        <ExpirationInfoField display={expirationDisplay} disabled={loading} onOpen={openExpirationDialog} />
        <TokenInfoField label="Token 有效期" value={tokenExpiryDisplay} needsRefresh={account.tokenNeedsRefresh} expired={account.tokenExpired} permanentlyFailed={account.tokenRefreshPermanentlyFailed} />
        {detailItems.slice(1).map((item) => (
          <InfoField key={item.label} label={item.label} value={item.value} />
        ))}
      </div>

      <div className="flex flex-wrap gap-2 pt-1">
        <Button variant="secondary" onClick={() => onSwitchAccount(account)} disabled={loading || account.isCurrent}>
          切换到此账号
        </Button>
        <Button variant="secondary" onClick={() => onCopyEmail(account)}>
          复制邮箱
        </Button>
        {!account.isCurrent && (account.tokenNeedsRefresh || account.tokenExpired || account.tokenRefreshPermanentlyFailed) && (
          <Button
            variant="secondary"
            onClick={() => onRefreshToken(account)}
            disabled={loading || tokenRefreshLoading || account.tokenRefreshPermanentlyFailed}
          >
            <RefreshCw className={`mr-1.5 h-3.5 w-3.5 ${tokenRefreshLoading ? 'animate-spin' : ''}`} />
            {tokenRefreshLoading ? '刷新中' : '刷新 Token'}
          </Button>
        )}
        <Button variant="secondary" onClick={() => onReauthorize(account)} disabled={loading}>
          重新授权
        </Button>
        <Button variant="danger" onClick={() => onRemoveSnapshot(account)} disabled={loading}>
          移除快照
        </Button>
      </div>

      {expirationDialogOpen && (
        <ExpirationEditDialog
          account={account}
          value={expirationValue}
          saving={expirationSaving}
          onChange={setExpirationValue}
          onClear={handleExpirationClear}
          onClose={() => setExpirationDialogOpen(false)}
          onSave={handleExpirationSave}
        />
      )}
    </div>
  );
}

function ExpirationInfoField({
  display,
  disabled,
  onOpen,
}: {
  display: ExpirationDisplay;
  disabled: boolean;
  onOpen: () => void;
}) {
  const textClassName = display.tone === 'danger' ? 'text-rose-600' : display.tone === 'muted' ? 'text-slate-500' : 'text-slate-800';

  return (
    <div className="min-w-0 rounded-lg bg-slate-50 px-3 py-2">
      <div className="text-xs text-slate-400">到期时间</div>
      <div className="mt-0.5 flex min-w-0 items-center gap-2">
        <div className={`min-w-0 flex-1 truncate text-sm font-medium ${textClassName}`} title={display.title}>
          {display.text}
        </div>
        <button
          type="button"
          className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white hover:text-indigo-600 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={onOpen}
          disabled={disabled}
          title="设置到期时间"
          aria-label="设置到期时间"
        >
          <Calendar className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}

function ExpirationEditDialog({
  account,
  value,
  saving,
  onChange,
  onClear,
  onClose,
  onSave,
}: {
  account: CodexAccount;
  value: string;
  saving: boolean;
  onChange: (value: string) => void;
  onClear: () => Promise<void>;
  onClose: () => void;
  onSave: () => Promise<void>;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 px-4 py-6 backdrop-blur-sm">
      <div className="box-border w-full max-w-sm rounded-2xl bg-white px-5 py-5 shadow-2xl">
        <div className="min-w-0">
          <h3 className="text-lg font-bold text-slate-950">设置到期时间</h3>
          <p className="mt-1 truncate text-sm text-slate-500">{account.email}</p>
        </div>
        <label className="mt-5 block">
          <span className="text-sm font-semibold text-slate-700">到期日期</span>
          <input
            className="mt-2 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-700 outline-none focus:border-indigo-400"
            type="date"
            value={value}
            onChange={(event) => onChange(event.target.value)}
            disabled={saving}
          />
        </label>
        <div className="mt-6 flex flex-wrap items-center justify-end gap-2">
          <Button variant="ghost" onClick={onClose} disabled={saving}>取消</Button>
          <Button variant="secondary" onClick={onClear} disabled={saving}>清空</Button>
          <Button onClick={onSave} disabled={saving}>{saving ? '保存中' : '保存'}</Button>
        </div>
      </div>
    </div>
  );
}

function QuotaCard({ label, percent, resetAt }: { label: string; percent?: number | null; resetAt?: string }) {
  const hasPercent = typeof percent === 'number';

  return (
    <div className="rounded-lg border border-slate-200 px-4 py-2.5">
      <div className="flex items-center justify-between text-sm">
        <span className="text-slate-600">{label}</span>
        <span className="font-bold text-emerald-600">{formatQuotaPercent(percent)}</span>
      </div>
      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-100">
        <div className="h-full rounded-full bg-emerald-500" style={{ width: `${hasPercent ? Math.min(percent, 100) : 0}%` }} />
      </div>
      <div className="mt-1.5 text-xs text-slate-500">{resetAt ? `${formatDateTime(resetAt)} 重置` : hasPercent ? '暂无额度重置信息' : '当前套餐未返回该额度窗口'}</div>
    </div>
  );
}

function getAccountUsageWindows(account: CodexAccount): CodexAccountUsageWindow[] {
  if (account.usageWindows?.length) {
    return account.usageWindows;
  }

  return [
    {
      remainingPercent: account.fiveHourPercent,
      resetAt: account.fiveHourResetAt,
      limitWindowSeconds: 18_000,
    },
    {
      remainingPercent: account.weeklyPercent,
      resetAt: account.weeklyResetAt,
      limitWindowSeconds: 604_800,
    },
  ];
}

function formatUsageWindowLabel(seconds?: number | null) {
  if (!seconds || seconds <= 0) {
    return '额度';
  }

  const hour = 3_600;
  const day = 86_400;
  const week = 604_800;
  const month = 2_592_000;

  if (seconds % month === 0) {
    return `${seconds / month}月`;
  }
  if (seconds % week === 0) {
    return `${seconds / week}周`;
  }
  if (seconds % day === 0) {
    return `${seconds / day}天`;
  }
  if (seconds % hour === 0) {
    return `${seconds / hour}小时`;
  }
  return `${Math.round(seconds / hour)}小时`;
}

function formatQuotaPercent(percent?: number | null) {
  return typeof percent === 'number' ? `${percent}%` : '-';
}

function InfoField({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-lg bg-slate-50 px-3 py-2">
      <div className="text-xs text-slate-400">{label}</div>
      <div className="mt-0.5 truncate text-sm font-medium text-slate-800" title={value}>{value}</div>
    </div>
  );
}

function TokenInfoField({ label, value, needsRefresh, expired, permanentlyFailed }: { label: string; value: string; needsRefresh: boolean; expired?: boolean; permanentlyFailed?: boolean }) {
  const toneClass = permanentlyFailed ? 'text-rose-600' : expired ? 'text-rose-600' : needsRefresh ? 'text-amber-600' : 'text-emerald-600';
  return (
    <div className="min-w-0 rounded-lg bg-slate-50 px-3 py-2">
      <div className="text-xs text-slate-400">{label}</div>
      <div className={`mt-0.5 truncate text-sm font-medium ${toneClass}`} title={value}>{value}</div>
    </div>
  );
}

type ExpirationDisplay = {
  text: string;
  title: string;
  tone: 'normal' | 'muted' | 'danger';
};

function getExpirationDisplay(value?: string): ExpirationDisplay {
  if (!value) {
    return { text: '未设置', title: '未设置', tone: 'muted' };
  }

  const date = parseDateTimeValue(value);
  if (!date) {
    return { text: value, title: value, tone: 'normal' };
  }

  const text = formatDateTime(value);
  const today = startOfLocalDay(new Date());
  const expiresDay = startOfLocalDay(date);
  const daysUntilExpiration = Math.floor((expiresDay.getTime() - today.getTime()) / 86_400_000);
  const shouldWarn = daysUntilExpiration <= 7;

  return {
    text,
    title: text,
    tone: shouldWarn ? 'danger' : 'normal',
  };
}

function accountExpirationDateInput(value?: string) {
  const date = value ? parseDateTimeValue(value) : null;
  if (!date) {
    return '';
  }

  return formatLocalDateInput(date);
}

function expirationDateInputToIso(value: string) {
  const [year, month, day] = value.split('-').map(Number);
  if (!year || !month || !day) {
    return value;
  }

  return new Date(year, month - 1, day, 23, 59, 59).toISOString();
}

function parseDateTimeValue(value: string) {
  const date = /^\d+$/.test(value) ? new Date(Number(value) * 1000) : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function startOfLocalDay(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function formatLocalDateInput(date: Date) {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function formatDateTime(value: string) {
  const date = parseDateTimeValue(value);

  if (!date) {
    return value;
  }

  return date.toLocaleString('zh-CN', { hour12: false });
}

function normalizeSubscriptionStatus(value: string, plan?: string) {
  const planLabel = normalizePlanLabel(plan);
  if (planLabel) {
    return planLabel;
  }
  if (value === '鏈夋晥璁㈤槄') {
    return '有效订阅';
  }
  if (value === '鏈煡') {
    return '未知';
  }
  return value || '未知';
}

function normalizePlanLabel(plan?: string) {
  const normalized = (plan || '').trim().toLowerCase();
  if (!normalized || normalized === 'unknown' || normalized === 'free') {
    return '';
  }
  if (normalized.includes('plus')) {
    return 'Plus';
  }
  if (normalized.includes('pro')) {
    return 'Pro';
  }
  if (normalized.includes('team')) {
    return 'Team';
  }
  if (normalized.includes('enterprise')) {
    return 'Enterprise';
  }
  return plan?.trim() || '';
}

function formatUnknownError(error: unknown) {
  if (error instanceof Error) {
    if (error.message.includes('invoke')) {
      return '当前浏览器预览无法访问 Tauri 后端，请在桌面应用中查看账号。';
    }

    return error.message;
  }

  return typeof error === 'string' ? error : String(error);
}
