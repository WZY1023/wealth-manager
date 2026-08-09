import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  Activity,
  ArrowDown,
  ArrowUp,
  BarChart3,
  BellRing,
  CalendarDays,
  CheckCircle2,
  ClipboardList,
  Database,
  Download,
  FileSpreadsheet,
  HardDriveDownload,
  LayoutDashboard,
  ListFilter,
  Pencil,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  Scale,
  Search,
  ShieldCheck,
  Settings2,
  Sparkles,
  WalletCards,
  X,
} from "lucide-react";
import "./App.css";

type View = "overview" | "holdings" | "transactions" | "calendar" | "analytics" | "masterData" | "reconciliation";
type CurrencyCode = "CNY" | "USD";
type EntryOperation = "BUY" | "SELL" | "PRODUCT_MATURITY" | "VALUATION" | "DIVIDEND" | "FEE" | "TRANSFER" | "DEPOSIT_OPEN" | "DEPOSIT_MATURITY";
type ProductSortKey = "name" | "code" | "riskLevel";
type SortDirection = "asc" | "desc";

type CurrencySummary = {
  currency: CurrencyCode;
  wealthValue: number;
  depositValue: number;
  totalValue: number;
  investedCost: number;
  unrealizedGain: number;
};

type Dashboard = {
  asOfDate: string;
  lastImportAt: string | null;
  currencies: CurrencySummary[];
  holdingCount: number;
  warningCount: number;
};

type Holding = {
  id: number;
  productId: number;
  accountId: number;
  name: string;
  code: string;
  currency: CurrencyCode;
  channel: string;
  cost: number;
  marketValue: number;
  gain: number;
  gainRate: number;
  valuationDate: string;
  status: string;
  daysSinceValuation: number;
  sevenDayReturn: number | null;
  thirtyDayReturn: number | null;
  signal: string;
  signalLabel: string;
  signalReason: string;
};

type ValuationHistoryPoint = { id: number; date: string; marketValue: number; changeAmount: number | null; changeRate: number | null; source: string };
type HoldingDetail = Omit<Holding, "id" | "status"> & { history: ValuationHistoryPoint[] };
type BatchValuationInput = { valuationDate: string; items: { productId: number; accountId: number; marketValue: number }[]; note: string | null };
type BatchValuationResult = { updated: number; message: string };

type TransactionRecord = {
  id: number;
  title: string;
  code: string | null;
  institution: string | null;
  operation: string;
  tradeDate: string;
  amount: number;
  costBasis: number;
  realizedGain: number;
  currency: CurrencyCode;
  note: string | null;
  source: string;
  reversedBy: number | null;
  reversalOf: number | null;
  canReverse: boolean;
};

type TransactionDetail = TransactionRecord & {
  institution: string | null;
  valuationBefore: number | null;
  valuationAfter: number | null;
  canEdit: boolean;
  editHint: string | null;
  canConfirmHistoricalRedemption: boolean;
  needsReview: boolean;
};

type HistoricalRedemptionInput = { transactionId: number; tradeDate: string; proceeds: number; note: string | null };
type TransactionEditInput = { transactionId: number; tradeDate: string; amount: number; note: string | null };

type EntryInput = {
  operation: EntryOperation;
  productId: number | null;
  accountId: number | null;
  transferAccountId: number | null;
  depositId: number | null;
  productName: string | null;
  productCode: string | null;
  institution: string | null;
  currency: CurrencyCode | null;
  amount: number | null;
  marketValue: number | null;
  tradeDate: string;
  maturityDate: string | null;
  annualRate: number | null;
  note: string | null;
};

type EntryResult = { message: string; realizedGain: number | null };
type FileOperationResult = { path: string; message: string };

type AccountReconciliation = { accountId: number; institution: string; name: string; currency: CurrencyCode; wealthValue: number; depositValue: number; trackedTotal: number; actualBalance: number | null; difference: number | null; balanceDate: string | null; note: string | null; status: "missing" | "matched" | "difference" };
type QualityIssue = { key: string; severity: "high" | "medium" | "low"; category: string; title: string; detail: string; targetView: View; targetId: number | null };
type ReconciliationCenter = { accounts: AccountReconciliation[]; issues: QualityIssue[]; issueCount: number; highPriorityCount: number };
type BalanceSnapshotInput = { accountId: number; balanceDate: string; actualBalance: number; note: string | null };

type AccountRecord = { id: number; institution: string; name: string; currency: CurrencyCode; source: string };
type ProductRecord = { id: number; code: string; name: string; currency: CurrencyCode; issuer: string | null; purchaseBanks: string[]; riskLevel: string | null; source: string };
type DepositRecord = { id: number; accountId: number; institution: string; name: string; currency: CurrencyCode; principal: number; startDate: string | null; maturityDate: string; annualRate: number; status: string; source: string };
type MasterData = { accounts: AccountRecord[]; products: ProductRecord[]; deposits: DepositRecord[] };
type EditableRecord = { entityType: "account"; record: AccountRecord } | { entityType: "product"; record: ProductRecord } | { entityType: "deposit"; record: DepositRecord };
type MasterDataUpdate = {
  entityType: EditableRecord["entityType"];
  id: number;
  name: string;
  code: string | null;
  institution: string | null;
  accountId: number | null;
  currency: CurrencyCode;
  issuer: string | null;
  riskLevel: string | null;
  principal: number | null;
  startDate: string | null;
  maturityDate: string | null;
  annualRate: number | null;
};

type TrendPoint = { date: string; label: string; assetValue: number; cumulativeRealizedGain: number; netCashFlow: number };
type CurrencyAnalytics = { currency: CurrencyCode; xirr: number | null; currentValue: number; unrealizedGain: number; realizedGain: number; income: number; fees: number; trend: TrendPoint[] };
type Analytics = { asOfDate: string; currencies: CurrencyAnalytics[] };

type MaturityEvent = {
  id: number;
  name: string;
  institution: string;
  currency: CurrencyCode;
  amount: number;
  maturityDate: string;
  daysRemaining: number;
  annualRate: number;
};

type ImportReport = {
  sourcePath: string;
  importedAt: string;
  lotsImported: number;
  holdingsImported: number;
  depositsImported: number;
  warnings: string[];
};

const EMPTY_DASHBOARD: Dashboard = {
  asOfDate: new Date().toISOString().slice(0, 10),
  lastImportAt: null,
  currencies: [
    { currency: "CNY", wealthValue: 0, depositValue: 0, totalValue: 0, investedCost: 0, unrealizedGain: 0 },
    { currency: "USD", wealthValue: 0, depositValue: 0, totalValue: 0, investedCost: 0, unrealizedGain: 0 },
  ],
  holdingCount: 0,
  warningCount: 0,
};

const EMPTY_MASTER_DATA: MasterData = { accounts: [], products: [], deposits: [] };
const EMPTY_ANALYTICS: Analytics = { asOfDate: new Date().toISOString().slice(0, 10), currencies: [] };
const EMPTY_RECONCILIATION: ReconciliationCenter = { accounts: [], issues: [], issueCount: 0, highPriorityCount: 0 };

const viewTitles: Record<View, string> = {
  overview: "资产总览",
  holdings: "当前持仓",
  transactions: "交易流水",
  calendar: "到期日历",
  analytics: "收益分析",
  masterData: "资料管理",
  reconciliation: "对账与数据质量",
};

function formatMoney(value: number, currency: CurrencyCode) {
  return new Intl.NumberFormat("zh-CN", {
    style: "currency",
    currency,
    maximumFractionDigits: currency === "CNY" ? 0 : 2,
  }).format(value);
}

function formatPercent(value: number) {
  return new Intl.NumberFormat("zh-CN", { style: "percent", maximumFractionDigits: 2 }).format(value);
}

function App() {
  const [view, setView] = useState<View>("overview");
  const [currency, setCurrency] = useState<CurrencyCode>("CNY");
  const [dashboard, setDashboard] = useState<Dashboard>(EMPTY_DASHBOARD);
  const [holdings, setHoldings] = useState<Holding[]>([]);
  const [maturities, setMaturities] = useState<MaturityEvent[]>([]);
  const [transactions, setTransactions] = useState<TransactionRecord[]>([]);
  const [masterData, setMasterData] = useState<MasterData>(EMPTY_MASTER_DATA);
  const [analytics, setAnalytics] = useState<Analytics>(EMPTY_ANALYTICS);
  const [reconciliation, setReconciliation] = useState<ReconciliationCenter>(EMPTY_RECONCILIATION);
  const [loading, setLoading] = useState(true);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [entryOpen, setEntryOpen] = useState(false);
  const [batchValuationOpen, setBatchValuationOpen] = useState(false);
  const [dataOpen, setDataOpen] = useState(false);
  const [transactionDetail, setTransactionDetail] = useState<TransactionDetail | null>(null);
  const [transactionToEdit, setTransactionToEdit] = useState<TransactionDetail | null>(null);
  const [redemptionToConfirm, setRedemptionToConfirm] = useState<TransactionDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [editingRecord, setEditingRecord] = useState<EditableRecord | null>(null);
  const [holdingDetail, setHoldingDetail] = useState<HoldingDetail | null>(null);
  const [holdingDetailLoading, setHoldingDetailLoading] = useState(false);
  const [reconcilingAccount, setReconcilingAccount] = useState<AccountReconciliation | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextDashboard, nextHoldings, nextMaturities, nextTransactions, nextMasterData, nextAnalytics, nextReconciliation] = await Promise.all([
        invoke<Dashboard>("get_dashboard"),
        invoke<Holding[]>("list_holdings"),
        invoke<MaturityEvent[]>("list_maturities"),
        invoke<TransactionRecord[]>("list_transactions"),
        invoke<MasterData>("list_master_data"),
        invoke<Analytics>("get_analytics"),
        invoke<ReconciliationCenter>("get_reconciliation_center"),
      ]);
      setDashboard(nextDashboard);
      setHoldings(nextHoldings);
      setMaturities(nextMaturities);
      setTransactions(nextTransactions);
      setMasterData(nextMasterData);
      setAnalytics(nextAnalytics);
      setReconciliation(nextReconciliation);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const importExcel = async () => {
    setError(null);
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Excel 工作簿", extensions: ["xlsx", "xls", "xlsm"] }],
    });
    if (!selected) return;
    setLoading(true);
    try {
      const nextReport = await invoke<ImportReport>("import_workbook", { path: selected });
      setReport(nextReport);
      setNotice(
        `已导入 ${nextReport.lotsImported} 笔买入、${nextReport.holdingsImported} 个持仓和 ${nextReport.depositsImported} 笔存款`,
      );
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
    }
  };

  const exportExcel = async () => {
    const selected = await save({
      defaultPath: `稳盈数据-${new Date().toISOString().slice(0, 10)}.xlsx`,
      filters: [{ name: "Excel 工作簿", extensions: ["xlsx"] }],
    });
    if (!selected) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<FileOperationResult>("export_excel_file", { path: selected });
      setNotice(`${result.message}：${result.path}`);
      setDataOpen(false);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  };

  const backupDatabase = async () => {
    const selected = await save({
      defaultPath: `稳盈备份-${new Date().toISOString().slice(0, 10)}.sqlite3`,
      filters: [{ name: "稳盈数据库备份", extensions: ["sqlite3", "db"] }],
    });
    if (!selected) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<FileOperationResult>("backup_database", { path: selected });
      setNotice(`${result.message}：${result.path}`);
      setDataOpen(false);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  };

  const restoreDatabase = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "稳盈数据库备份", extensions: ["sqlite3", "db"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    if (!window.confirm("恢复会用所选备份替换当前数据。程序会先自动保存当前数据库，确定继续吗？")) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<FileOperationResult>("restore_database", { path: selected });
      setNotice(result.message);
      setDataOpen(false);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
    }
  };

  const openTransaction = async (transactionId: number) => {
    setDetailLoading(true);
    setError(null);
    try {
      setTransactionDetail(await invoke<TransactionDetail>("get_transaction_detail", { transactionId }));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setDetailLoading(false);
    }
  };

  const reverseTransaction = async (transactionId: number) => {
    if (!window.confirm("撤销不会删除原流水，而是生成冲销记录并恢复相关持仓。确定继续吗？")) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<EntryResult>("reverse_transaction", { transactionId });
      setNotice(result.message);
      setTransactionDetail(null);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
    }
  };

  const selectedSummary =
    dashboard.currencies.find((item) => item.currency === currency) ?? EMPTY_DASHBOARD.currencies[0];

  const upcoming = maturities.filter((item) => item.daysRemaining >= 0 && item.daysRemaining <= 30);
  const groupedMaturities = useMemo(() => {
    return maturities.reduce<Record<string, MaturityEvent[]>>((groups, item) => {
      const month = item.maturityDate.slice(0, 7);
      groups[month] = [...(groups[month] ?? []), item];
      return groups;
    }, {});
  }, [maturities]);
  const hasData = dashboard.currencies.some((item) => item.totalValue > 0) || transactions.length > 0;

  const saveEntry = async (input: EntryInput) => {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<EntryResult>("record_entry", { input });
      const gain = result.realizedGain;
      setNotice(
        gain === null
          ? result.message
          : `${result.message}，已实现收益 ${formatMoney(gain, input.currency ?? "CNY")}`,
      );
      setEntryOpen(false);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
      throw reason;
    }
  };

  const saveMasterData = async (input: MasterDataUpdate) => {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<EntryResult>("update_master_data", { input });
      setNotice(result.message);
      setEditingRecord(null);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
      throw reason;
    }
  };

  const saveBatchValuations = async (input: BatchValuationInput) => {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<BatchValuationResult>("record_batch_valuations", { input });
      setNotice(result.message);
      setBatchValuationOpen(false);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
      throw reason;
    }
  };

  const openHoldingDetail = async (holding: Holding) => {
    setHoldingDetailLoading(true);
    setError(null);
    try {
      setHoldingDetail(await invoke<HoldingDetail>("get_holding_detail", { productId: holding.productId, accountId: holding.accountId }));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setHoldingDetailLoading(false);
    }
  };

  const saveBalanceSnapshot = async (input: BalanceSnapshotInput) => {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<EntryResult>("save_balance_snapshot", { input });
      setNotice(result.message);
      setReconcilingAccount(null);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
      throw reason;
    }
  };

  const saveHistoricalRedemption = async (input: HistoricalRedemptionInput) => {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<EntryResult>("confirm_historical_redemption", { input });
      setNotice(`${result.message}${result.realizedGain === null ? "" : `，本批次已实现收益 ${formatMoney(result.realizedGain, redemptionToConfirm?.currency ?? "CNY")}`}`);
      setRedemptionToConfirm(null);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
      throw reason;
    }
  };

  const saveTransactionEdit = async (input: TransactionEditInput) => {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<EntryResult>("edit_transaction", { input });
      setNotice(`${result.message}${result.realizedGain === null ? "" : `，已实现收益 ${formatMoney(result.realizedGain, transactionToEdit?.currency ?? "CNY")}`}`);
      setTransactionToEdit(null);
      await refresh();
    } catch (reason) {
      setError(String(reason));
      setLoading(false);
      throw reason;
    }
  };

  const openQualityIssue = (issue: QualityIssue) => {
    setView(issue.targetView);
    if (issue.targetView === "transactions" && issue.targetId !== null) void openTransaction(issue.targetId);
  };

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">盈</div>
          <div><strong>稳盈</strong><span>本地理财管家</span></div>
        </div>
        <nav aria-label="主导航">
          <NavButton active={view === "overview"} onClick={() => setView("overview")} icon={<LayoutDashboard />} label="总览" />
          <NavButton active={view === "holdings"} onClick={() => setView("holdings")} icon={<WalletCards />} label="持仓" />
          <NavButton active={view === "transactions"} onClick={() => setView("transactions")} icon={<ClipboardList />} label="交易流水" />
          <NavButton active={view === "calendar"} onClick={() => setView("calendar")} icon={<CalendarDays />} label="到期日历" />
          <NavButton active={view === "analytics"} onClick={() => setView("analytics")} icon={<BarChart3 />} label="收益分析" />
          <NavButton active={view === "masterData"} onClick={() => setView("masterData")} icon={<Settings2 />} label="资料管理" />
          <NavButton active={view === "reconciliation"} onClick={() => setView("reconciliation")} icon={<Scale />} label={`对账中心${reconciliation.highPriorityCount ? ` · ${reconciliation.highPriorityCount}` : ""}`} />
        </nav>
        <div className="privacy-note"><ShieldCheck /><span>数据仅保存在本机<br />SQLite 自动持久化</span></div>
      </aside>

      <main className="main-content">
        <header className="page-header">
          <div>
            <h1>{viewTitles[view]}</h1>
            <p>截至 {dashboard.asOfDate} · {dashboard.lastImportAt ? `最近导入 ${dashboard.lastImportAt}` : "尚未导入数据"}</p>
          </div>
          <div className="header-actions">
            <button className="button secondary" type="button" onClick={() => void refresh()} disabled={loading}><RefreshCw />刷新</button>
            <button className="button secondary" type="button" onClick={() => void importExcel()} disabled={loading}><FileSpreadsheet />导入表格</button>
            <button className="button secondary" type="button" onClick={() => setDataOpen(true)} disabled={loading}><Database />数据安全</button>
            <button className="button primary" type="button" onClick={() => setEntryOpen(true)}><Plus />记一笔</button>
          </div>
        </header>

        {notice && <div className="banner success"><CheckCircle2 /><span>{notice}</span><button onClick={() => setNotice(null)} aria-label="关闭"><X /></button></div>}
        {error && <div className="banner error"><AlertTriangle /><span>{error}</span><button onClick={() => setError(null)} aria-label="关闭"><X /></button></div>}

        {loading && !hasData ? (
          <div className="loading-state"><RefreshCw className="spin" />正在读取本地数据…</div>
        ) : !hasData ? (
          <EmptyState onImport={() => void importExcel()} />
        ) : (
          <>
            {view === "overview" && (
              <Overview
                dashboard={dashboard}
                currency={currency}
                onCurrencyChange={setCurrency}
                selectedSummary={selectedSummary}
                upcoming={upcoming}
              />
            )}
            {view === "holdings" && <HoldingsView holdings={holdings} onBatchUpdate={() => setBatchValuationOpen(true)} onSelect={(holding) => void openHoldingDetail(holding)} loading={holdingDetailLoading} />}
            {view === "transactions" && <TransactionsView transactions={transactions} onSelect={(id) => void openTransaction(id)} loading={detailLoading} />}
            {view === "calendar" && <CalendarView groups={groupedMaturities} />}
            {view === "analytics" && <AnalyticsView analytics={analytics} currency={currency} onCurrencyChange={setCurrency} />}
            {view === "masterData" && <MasterDataView data={masterData} onEdit={setEditingRecord} />}
            {view === "reconciliation" && <ReconciliationView data={reconciliation} onReconcile={setReconcilingAccount} onOpenIssue={openQualityIssue} />}
          </>
        )}
      </main>

      {report && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => setReport(null)}>
          <section className="modal" role="dialog" aria-modal="true" aria-labelledby="report-title" onMouseDown={(event) => event.stopPropagation()}>
            <div className="modal-header"><div><h2 id="report-title">导入完成</h2><p>{report.sourcePath}</p></div><button onClick={() => setReport(null)} aria-label="关闭"><X /></button></div>
            <div className="import-counts">
              <div><span>买入批次</span><strong>{report.lotsImported}</strong></div>
              <div><span>当前持仓</span><strong>{report.holdingsImported}</strong></div>
              <div><span>定期存款</span><strong>{report.depositsImported}</strong></div>
            </div>
            <div className="warning-list">
              <h3>{report.warnings.length ? `${report.warnings.length} 项需要核对` : "数据校验通过"}</h3>
              {report.warnings.map((warning) => <div key={warning}><AlertTriangle />{warning}</div>)}
            </div>
            <button className="button primary full" onClick={() => setReport(null)}>查看资产总览</button>
          </section>
        </div>
      )}
      {entryOpen && (
        <EntryModal
          holdings={holdings}
          deposits={maturities}
          accounts={masterData.accounts}
          onClose={() => setEntryOpen(false)}
          onSave={saveEntry}
        />
      )}
      {batchValuationOpen && (
        <BatchValuationModal holdings={holdings} onClose={() => setBatchValuationOpen(false)} onSave={saveBatchValuations} />
      )}
      {dataOpen && (
        <DataSafetyModal
          busy={loading}
          onClose={() => setDataOpen(false)}
          onExport={() => void exportExcel()}
          onBackup={() => void backupDatabase()}
          onRestore={() => void restoreDatabase()}
        />
      )}
      {transactionDetail && (
        <TransactionDetailModal
          detail={transactionDetail}
          onClose={() => setTransactionDetail(null)}
          onReverse={(id) => void reverseTransaction(id)}
          onEdit={(detail) => { setTransactionDetail(null); setTransactionToEdit(detail); }}
          onConfirm={(detail) => { setTransactionDetail(null); setRedemptionToConfirm(detail); }}
        />
      )}
      {transactionToEdit && (
        <TransactionEditModal detail={transactionToEdit} onClose={() => setTransactionToEdit(null)} onSave={saveTransactionEdit} />
      )}
      {redemptionToConfirm && (
        <HistoricalRedemptionModal detail={redemptionToConfirm} onClose={() => setRedemptionToConfirm(null)} onSave={saveHistoricalRedemption} />
      )}
      {editingRecord && (
        <MasterDataEditModal
          target={editingRecord}
          accounts={masterData.accounts}
          onClose={() => setEditingRecord(null)}
          onSave={saveMasterData}
        />
      )}
      {holdingDetail && (
        <HoldingDetailModal detail={holdingDetail} onClose={() => setHoldingDetail(null)} />
      )}
      {reconcilingAccount && (
        <BalanceReconciliationModal account={reconcilingAccount} onClose={() => setReconcilingAccount(null)} onSave={saveBalanceSnapshot} />
      )}
    </div>
  );
}

function NavButton({ active, onClick, icon, label }: { active: boolean; onClick: () => void; icon: React.ReactNode; label: string }) {
  return <button type="button" className={active ? "active" : ""} aria-pressed={active} onClick={onClick}>{icon}{label}</button>;
}

function EmptyState({ onImport }: { onImport: () => void }) {
  return (
    <section className="empty-state">
      <div className="empty-icon"><Database /></div>
      <h2>建立你的本地资产库</h2>
      <p>从现有的“理财.xlsx”开始。导入不会修改原文件，发现币种或产品信息冲突时会先提示你核对。</p>
      <button className="button primary" onClick={onImport}><FileSpreadsheet />选择 Excel 文件</button>
    </section>
  );
}

function Overview({ dashboard, currency, onCurrencyChange, selectedSummary, upcoming }: {
  dashboard: Dashboard;
  currency: CurrencyCode;
  onCurrencyChange: (value: CurrencyCode) => void;
  selectedSummary: CurrencySummary;
  upcoming: MaturityEvent[];
}) {
  const cny = dashboard.currencies.find((item) => item.currency === "CNY")!;
  const usd = dashboard.currencies.find((item) => item.currency === "USD")!;
  const total = selectedSummary.totalValue || 1;
  const allocations = [
    { label: "定期存款", value: selectedSummary.depositValue },
    { label: "银行理财", value: selectedSummary.wealthValue },
  ];
  return (
    <>
      <section className="metrics-grid">
        <Metric label="人民币资产" value={formatMoney(cny.totalValue, "CNY")} detail={`理财 ${formatMoney(cny.wealthValue, "CNY")} · 存款 ${formatMoney(cny.depositValue, "CNY")}`} />
        <Metric label="美元资产" value={formatMoney(usd.totalValue, "USD")} detail={`理财 ${formatMoney(usd.wealthValue, "USD")} · 存款 ${formatMoney(usd.depositValue, "USD")}`} />
        <Metric label="当前持仓收益" value={`${formatMoney(cny.unrealizedGain, "CNY")} / ${formatMoney(usd.unrealizedGain, "USD")}`} detail="未包含汇率变动" positive={cny.unrealizedGain + usd.unrealizedGain >= 0} />
      </section>
      <section className="overview-grid">
        <article className="panel">
          <div className="panel-header"><h2>资产配置</h2><div className="segmented"><button className={currency === "CNY" ? "active" : ""} onClick={() => onCurrencyChange("CNY")}>人民币</button><button className={currency === "USD" ? "active" : ""} onClick={() => onCurrencyChange("USD")}>美元</button></div></div>
          <div className="allocation-list">
            {allocations.map((item) => {
              const percent = item.value / total;
              return <div className="allocation-row" key={item.label}><span>{item.label}</span><div className="bar-track"><div className="bar-fill" style={{ width: `${percent * 100}%` }} /></div><strong>{formatMoney(item.value, currency)} · {formatPercent(percent)}</strong></div>;
            })}
          </div>
        </article>
        <article className="panel">
          <div className="panel-header"><h2>近期可用资金</h2><span>未来30天</span></div>
          <div className="upcoming-list">
            {upcoming.length ? upcoming.map((item) => <div className="upcoming-item" key={item.id}><div><strong>{item.maturityDate.slice(5).replace("-", "月")}日</strong><span>{item.daysRemaining}天后</span></div><p>{item.name}</p><b>{formatMoney(item.amount, item.currency)}</b></div>) : <p className="muted">未来30天没有到期存款</p>}
          </div>
          {upcoming.length > 0 && <div className="reminder"><BellRing />有 {upcoming.length} 笔资金将在30天内到期，可提前安排续存或再配置。</div>}
        </article>
      </section>
    </>
  );
}

function Metric({ label, value, detail, positive }: { label: string; value: string; detail: string; positive?: boolean }) {
  return <article className="metric-card"><span>{label}</span><strong className={positive === undefined ? "" : positive ? "gain" : "loss"}>{value}</strong><small>{detail}</small></article>;
}

function HoldingsView({ holdings, onBatchUpdate, onSelect, loading }: { holdings: Holding[]; onBatchUpdate: () => void; onSelect: (holding: Holding) => void; loading: boolean }) {
  const [searchText, setSearchText] = useState("");
  const [currencyFilter, setCurrencyFilter] = useState("all");
  const [channelFilter, setChannelFilter] = useState("all");
  const [signalFilter, setSignalFilter] = useState("all");
  const channels = useMemo(() => [...new Set(holdings.map((item) => item.channel))].sort(), [holdings]);
  const filtered = useMemo(() => {
    const keyword = searchText.trim().toLowerCase();
    return holdings.filter((item) => {
      const matchesKeyword = !keyword || `${item.name} ${item.code} ${item.channel}`.toLowerCase().includes(keyword);
      const matchesCurrency = currencyFilter === "all" || item.currency === currencyFilter;
      const matchesChannel = channelFilter === "all" || item.channel === channelFilter;
      const matchesSignal = signalFilter === "all" || (signalFilter === "stale" ? item.daysSinceValuation > 10 : item.signal === signalFilter);
      return matchesKeyword && matchesCurrency && matchesChannel && matchesSignal;
    });
  }, [holdings, searchText, currencyFilter, channelFilter, signalFilter]);
  return (
    <article className="panel table-panel">
      <div className="panel-header holdings-header"><div><h2>当前持仓</h2><span>显示 {filtered.length}/{holdings.length} 个产品 · 点击持仓查看市值与收益历史</span></div><button className="button primary valuation-cta" type="button" onClick={onBatchUpdate}><Activity />批量更新市值</button></div>
      <div className="filter-bar holdings-filter"><label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索产品、代码或渠道" /></label><select value={currencyFilter} onChange={(event) => setCurrencyFilter(event.target.value)}><option value="all">全部币种</option><option value="CNY">人民币</option><option value="USD">美元</option></select><select value={channelFilter} onChange={(event) => setChannelFilter(event.target.value)}><option value="all">全部渠道</option>{channels.map((channel) => <option key={channel} value={channel}>{channel}</option>)}</select><select value={signalFilter} onChange={(event) => setSignalFilter(event.target.value)}><option value="all">全部参考</option><option value="stale">市值待更新</option><option value="REVIEW">评估赎回</option><option value="TAKE_PROFIT">考虑止盈</option><option value="ADD_WATCH">关注加仓</option><option value="HOLD">继续观察</option><option value="OBSERVE">积累数据</option></select></div>
      <div className="table-scroll"><table><thead><tr><th>产品</th><th>渠道</th><th className="number">成本</th><th className="number">市值</th><th className="number">持有收益</th><th className="number">近30日</th><th>最近更新</th><th>操作参考</th></tr></thead><tbody>
        {filtered.map((item) => <tr className="holding-row" aria-busy={loading} key={`${item.id}-${item.currency}`} onClick={() => onSelect(item)}><td><strong>{item.name}</strong><span>{item.code} · {item.currency}</span></td><td>{item.channel}</td><td className="number">{formatMoney(item.cost, item.currency)}</td><td className="number"><strong>{formatMoney(item.marketValue, item.currency)}</strong></td><td className={`number ${item.gain >= 0 ? "gain" : "loss"}`}>{formatMoney(item.gain, item.currency)}<span>{formatPercent(item.gainRate)}</span></td><td className={`number ${item.thirtyDayReturn === null ? "" : item.thirtyDayReturn >= 0 ? "gain" : "loss"}`}>{item.thirtyDayReturn === null ? "—" : formatPercent(item.thirtyDayReturn)}</td><td><span className={item.daysSinceValuation > 10 ? "stale-date" : "fresh-date"}>{item.valuationDate}</span><small>{item.daysSinceValuation === 0 ? "今天" : `${item.daysSinceValuation}天前`}</small></td><td><span className={`signal-badge signal-${item.signal.toLowerCase()}`}>{item.signalLabel}</span><small className="signal-summary">{item.signalReason}</small></td></tr>)}
      </tbody></table></div>
      <div className="advice-disclaimer"><Sparkles />操作参考仅根据你记录的估值、收益、产品风险等级和持仓占比生成；历史表现不能预测未来，请同时核对期限、赎回规则、费用和个人风险承受能力。</div>
    </article>
  );
}

function BatchValuationModal({ holdings, onClose, onSave }: { holdings: Holding[]; onClose: () => void; onSave: (input: BatchValuationInput) => Promise<void> }) {
  const today = new Date().toISOString().slice(0, 10);
  const sortedHoldings = useMemo(() => {
    const collator = new Intl.Collator("zh-CN", { numeric: true, sensitivity: "base" });
    return [...holdings].sort((left, right) => {
      const leftCode = left.code.trim();
      const rightCode = right.code.trim();
      if (!leftCode && rightCode) return 1;
      if (leftCode && !rightCode) return -1;
      const byCode = collator.compare(leftCode, rightCode);
      if (byCode !== 0) return byCode;
      const byChannel = collator.compare(left.channel, right.channel);
      return byChannel !== 0 ? byChannel : left.accountId - right.accountId;
    });
  }, [holdings]);
  const [valuationDate, setValuationDate] = useState(today);
  const [note, setNote] = useState("");
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(sortedHoldings.map((item) => [`${item.productId}:${item.accountId}`, item.marketValue.toString()])));
  const [submitting, setSubmitting] = useState(false);
  const changed = sortedHoldings.filter((item) => {
    const next = Number(values[`${item.productId}:${item.accountId}`]);
    return Number.isFinite(next) && Math.abs(next - item.marketValue) > 0.000001;
  }).length;
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const items = sortedHoldings.map((item) => ({ productId: item.productId, accountId: item.accountId, marketValue: Number(values[`${item.productId}:${item.accountId}`]) }));
    if (items.some((item) => !Number.isFinite(item.marketValue) || item.marketValue < 0)) return;
    setSubmitting(true);
    try {
      await onSave({ valuationDate, items, note: note.trim() || null });
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal batch-valuation-modal" role="dialog" aria-modal="true" aria-labelledby="batch-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="batch-title">批量更新市值</h2><p>一次完成全部持仓的周度估值；未变化的数值也会留下本次确认记录</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <form onSubmit={(event) => void submit(event)}>
          <div className="batch-meta"><label>估值日期<input required type="date" value={valuationDate} onChange={(event) => setValuationDate(event.target.value)} /></label><label>备注（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="例如：周末统一更新" /></label></div>
          <div className="batch-list"><div className="batch-list-head"><span>产品、代码与渠道</span><span>原市值</span><span>本次市值</span><span>变化</span></div>{sortedHoldings.map((item) => {
            const key = `${item.productId}:${item.accountId}`;
            const next = Number(values[key]);
            const difference = Number.isFinite(next) ? next - item.marketValue : 0;
            return <div className="batch-row" key={key}><div><strong>{item.name}</strong><span>{item.code || "无代码"} · {item.channel} · {item.currency}</span></div><span>{formatMoney(item.marketValue, item.currency)}</span><input required aria-label={`${item.name}本次市值`} type="number" min="0" step="0.01" value={values[key]} onChange={(event) => setValues((current) => ({ ...current, [key]: event.target.value }))} /><b className={difference >= 0 ? "gain" : "loss"}>{difference === 0 ? "—" : `${difference > 0 ? "+" : ""}${formatMoney(difference, item.currency)}`}</b></div>;
          })}</div>
          <div className="batch-footer"><div><strong>{sortedHoldings.length}</strong> 个持仓将记录本次估值，<strong>{changed}</strong> 个市值发生变化</div><div className="entry-actions"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><Save />{submitting ? "保存中…" : "保存全部市值"}</button></div></div>
        </form>
      </section>
    </div>
  );
}

function ValuationHistoryChart({ points, currency }: { points: ValuationHistoryPoint[]; currency: CurrencyCode }) {
  const visible = points.slice(-24);
  if (visible.length < 2) return <div className="history-empty">至少完成两次市值记录后显示变化曲线</div>;
  const width = 760;
  const height = 230;
  const padX = 28;
  const padTop = 18;
  const padBottom = 34;
  const values = visible.map((item) => item.marketValue);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const padding = Math.max((max - min) * 0.12, max * 0.005, 1);
  const low = min - padding;
  const high = max + padding;
  const x = (index: number) => padX + index * ((width - padX * 2) / Math.max(visible.length - 1, 1));
  const y = (value: number) => padTop + (high - value) / Math.max(high - low, 1) * (height - padTop - padBottom);
  const path = visible.map((item, index) => `${x(index)},${y(item.marketValue)}`).join(" ");
  return <div className="valuation-chart"><svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label="持仓市值历史"><polyline points={path} fill="none" stroke="#2e7d55" strokeWidth="3" strokeLinejoin="round" strokeLinecap="round" />{visible.map((item, index) => <g key={item.id}><circle cx={x(index)} cy={y(item.marketValue)} r="3.5" fill="#2e7d55"><title>{item.date} · {formatMoney(item.marketValue, currency)}</title></circle>{(index === 0 || index === visible.length - 1 || index % Math.ceil(visible.length / 6) === 0) && <text x={x(index)} y={height - 9} textAnchor="middle">{item.date.slice(5)}</text>}</g>)}</svg><div className="chart-range"><span>{formatMoney(low, currency)}</span><span>{formatMoney(high, currency)}</span></div></div>;
}

function HoldingDetailModal({ detail, onClose }: { detail: HoldingDetail; onClose: () => void }) {
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal holding-detail-modal" role="dialog" aria-modal="true" aria-labelledby="holding-detail-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="holding-detail-title">{detail.name}</h2><p>{detail.code} · {detail.channel} · {detail.currency}</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <section className="holding-detail-metrics"><div><span>当前市值</span><strong>{formatMoney(detail.marketValue, detail.currency)}</strong></div><div><span>持有收益</span><strong className={detail.gain >= 0 ? "gain" : "loss"}>{formatMoney(detail.gain, detail.currency)} · {formatPercent(detail.gainRate)}</strong></div><div><span>近7日估值收益</span><strong className={detail.sevenDayReturn === null ? "" : detail.sevenDayReturn >= 0 ? "gain" : "loss"}>{detail.sevenDayReturn === null ? "—" : formatPercent(detail.sevenDayReturn)}</strong></div><div><span>近30日估值收益</span><strong className={detail.thirtyDayReturn === null ? "" : detail.thirtyDayReturn >= 0 ? "gain" : "loss"}>{detail.thirtyDayReturn === null ? "—" : formatPercent(detail.thirtyDayReturn)}</strong></div></section>
        <div className={`holding-signal signal-card-${detail.signal.toLowerCase()}`}><div><Sparkles /><strong>{detail.signalLabel}</strong></div><p>{detail.signalReason}</p></div>
        <div className="history-section"><div className="panel-header"><h2>市值变化</h2><span>最近更新 {detail.valuationDate} · {detail.daysSinceValuation === 0 ? "今天" : `${detail.daysSinceValuation}天前`}</span></div><ValuationHistoryChart points={detail.history} currency={detail.currency} /></div>
        <div className="history-table"><div className="batch-list-head"><span>日期</span><span>市值</span><span>估值收益</span><span>来源</span></div>{detail.history.slice().reverse().slice(0, 12).map((item) => <div key={item.id}><span>{item.date}</span><strong>{formatMoney(item.marketValue, detail.currency)}</strong><span className={item.changeRate === null ? "" : item.changeRate >= 0 ? "gain" : "loss"}>{item.changeRate === null ? "—" : `${formatMoney(item.changeAmount ?? 0, detail.currency)} · ${formatPercent(item.changeRate)}`}</span><span>{item.source}</span></div>)}</div>
        <div className="advice-disclaimer"><AlertTriangle />这些信号是记录整理和复核提示，不是收益保证或个性化投资顾问意见。历史表现不能预测未来。</div>
        <div className="entry-actions"><button className="button secondary" onClick={onClose}>关闭</button></div>
      </section>
    </div>
  );
}

function CalendarView({ groups }: { groups: Record<string, MaturityEvent[]> }) {
  return <section className="calendar-list">{Object.entries(groups).map(([month, events]) => <article className="panel month-panel" key={month}><div className="month-title"><h2>{month.replace("-", "年")}月</h2><span>{events.length}笔到期</span></div><div className="timeline">{events.map((item) => <div className="timeline-item" key={item.id}><time>{item.maturityDate.slice(5)}</time><div><strong>{item.name}</strong><span>{item.institution} · 年利率 {formatPercent(item.annualRate)}</span></div><b>{formatMoney(item.amount, item.currency)}</b></div>)}</div></article>)}</section>;
}

function TrendChart({ points, field, currency, color }: { points: TrendPoint[]; field: "assetValue" | "cumulativeRealizedGain"; currency: CurrencyCode; color: string }) {
  const width = 760;
  const height = 220;
  const padX = 28;
  const padTop = 18;
  const padBottom = 35;
  const values = points.map((item) => item[field]);
  const rawMin = Math.min(0, ...values);
  const rawMax = Math.max(0, ...values);
  const span = Math.max(rawMax - rawMin, 1);
  const x = (index: number) => padX + index * ((width - padX * 2) / Math.max(points.length - 1, 1));
  const y = (value: number) => padTop + (rawMax - value) / span * (height - padTop - padBottom);
  const path = points.map((item, index) => `${x(index)},${y(item[field])}`).join(" ");
  return (
    <div className="trend-chart">
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={field === "assetValue" ? "资产变化趋势" : "累计收益趋势"}>
        <line x1={padX} y1={y(0)} x2={width - padX} y2={y(0)} className="chart-zero" />
        <polyline points={path} fill="none" stroke={color} strokeWidth="3" strokeLinejoin="round" strokeLinecap="round" />
        {points.map((item, index) => <g key={item.date}><circle cx={x(index)} cy={y(item[field])} r="3.5" fill={color}><title>{item.date} · {formatMoney(item[field], currency)}</title></circle><text x={x(index)} y={height - 10} textAnchor="middle">{item.label}</text></g>)}
      </svg>
      <div className="chart-range"><span>{formatMoney(rawMin, currency)}</span><span>{formatMoney(rawMax, currency)}</span></div>
    </div>
  );
}

function AnalyticsView({ analytics, currency, onCurrencyChange }: { analytics: Analytics; currency: CurrencyCode; onCurrencyChange: (value: CurrencyCode) => void }) {
  const data = analytics.currencies.find((item) => item.currency === currency);
  if (!data) return <article className="panel"><p className="muted">暂无可分析数据</p></article>;
  return (
    <>
      <div className="analysis-toolbar"><div className="segmented"><button className={currency === "CNY" ? "active" : ""} onClick={() => onCurrencyChange("CNY")}>人民币</button><button className={currency === "USD" ? "active" : ""} onClick={() => onCurrencyChange("USD")}>美元</button></div><span>统计截至 {analytics.asOfDate} · XIRR 按实际日期现金流计算</span></div>
      <section className="metrics-grid analysis-metrics">
        <Metric label="当前资产" value={formatMoney(data.currentValue, currency)} detail="理财市值 + 有效定存本金" />
        <Metric label="年化收益率 XIRR" value={data.xirr === null ? "数据不足" : formatPercent(data.xirr)} detail="不完整历史不会强行估算" positive={data.xirr === null ? undefined : data.xirr >= 0} />
        <Metric label="累计已实现收益" value={formatMoney(data.realizedGain, currency)} detail={`分红 ${formatMoney(data.income, currency)} · 费用 ${formatMoney(data.fees, currency)}`} positive={data.realizedGain >= 0} />
        <Metric label="当前持有收益" value={formatMoney(data.unrealizedGain, currency)} detail="当前市值减剩余持仓成本" positive={data.unrealizedGain >= 0} />
      </section>
      <section className="analysis-grid">
        <article className="panel"><div className="panel-header"><h2>资产变化</h2><span>近12个月月末值</span></div><TrendChart points={data.trend} field="assetValue" currency={currency} color="#2e7d55" /></article>
        <article className="panel"><div className="panel-header"><h2>累计已实现收益</h2><span>赎回收益、利息、分红与费用</span></div><TrendChart points={data.trend} field="cumulativeRealizedGain" currency={currency} color="#bd7c2e" /></article>
      </section>
      <article className="panel cash-flow-panel"><div className="panel-header"><h2>月度现金流</h2><span>正数为回款，负数为投入或费用；内部转账已排除</span></div><div className="cash-flow-list">{data.trend.map((item) => <div key={item.date}><span>{item.label}</span><div className="flow-track"><i className={item.netCashFlow >= 0 ? "positive" : "negative"} style={{ width: `${Math.min(100, Math.abs(item.netCashFlow) / Math.max(...data.trend.map((point) => Math.abs(point.netCashFlow)), 1) * 100)}%` }} /></div><strong className={item.netCashFlow >= 0 ? "gain" : "loss"}>{formatMoney(item.netCashFlow, currency)}</strong></div>)}</div></article>
      <div className="calculation-note analysis-note">XIRR 仅使用日期和方向明确的买入、卖出、到期、分红及费用流水，并加入当前理财市值。缺少开户日期的历史导入定存不会加入 XIRR，以避免虚高结果。</div>
    </>
  );
}

function ProductSortHeader({ label, sortKey, activeKey, direction, onSort }: { label: string; sortKey: ProductSortKey; activeKey: ProductSortKey; direction: SortDirection; onSort: (key: ProductSortKey) => void }) {
  const active = sortKey === activeKey;
  return <th aria-sort={active ? direction === "asc" ? "ascending" : "descending" : "none"}><button className={`sortable-header ${active ? "active" : ""}`} type="button" onClick={() => onSort(sortKey)}>{label}{active ? direction === "asc" ? <ArrowUp /> : <ArrowDown /> : <span className="sort-placeholder">↕</span>}</button></th>;
}

function MasterDataView({ data, onEdit }: { data: MasterData; onEdit: (target: EditableRecord) => void }) {
  const [section, setSection] = useState<"products" | "accounts" | "deposits">("products");
  const [productSort, setProductSort] = useState<{ key: ProductSortKey; direction: SortDirection }>({ key: "code", direction: "asc" });
  const sortedProducts = useMemo(() => {
    const collator = new Intl.Collator("zh-CN", { numeric: true, sensitivity: "base" });
    return [...data.products].sort((left, right) => {
      const leftValue = (left[productSort.key] ?? "").trim();
      const rightValue = (right[productSort.key] ?? "").trim();
      if (!leftValue && rightValue) return 1;
      if (leftValue && !rightValue) return -1;
      const primary = collator.compare(leftValue, rightValue) * (productSort.direction === "asc" ? 1 : -1);
      if (primary !== 0) return primary;
      const byCode = collator.compare(left.code, right.code);
      return byCode !== 0 ? byCode : left.id - right.id;
    });
  }, [data.products, productSort]);
  const changeProductSort = (key: ProductSortKey) => {
    setProductSort((current) => current.key === key ? { key, direction: current.direction === "asc" ? "desc" : "asc" } : { key, direction: "asc" });
  };
  return (
    <article className="panel table-panel master-panel">
      <div className="panel-header"><div><h2>资料管理</h2><span>修改会自动备份并保留变更审计；手工修正不会被下次导入覆盖</span></div><div className="segmented"><button className={section === "products" ? "active" : ""} onClick={() => setSection("products")}>产品 {data.products.length}</button><button className={section === "accounts" ? "active" : ""} onClick={() => setSection("accounts")}>账户 {data.accounts.length}</button><button className={section === "deposits" ? "active" : ""} onClick={() => setSection("deposits")}>存款 {data.deposits.length}</button></div></div>
      {section === "products" && <div className="table-scroll"><table className="product-table"><thead><tr><ProductSortHeader label="名称" sortKey="name" activeKey={productSort.key} direction={productSort.direction} onSort={changeProductSort} /><ProductSortHeader label="代码" sortKey="code" activeKey={productSort.key} direction={productSort.direction} onSort={changeProductSort} /><th>币种</th><th>发行机构</th><th>购买银行</th><ProductSortHeader label="风险等级" sortKey="riskLevel" activeKey={productSort.key} direction={productSort.direction} onSort={changeProductSort} /><th>来源</th><th /></tr></thead><tbody>{sortedProducts.map((item) => <tr key={item.id}><td><strong>{item.name}</strong></td><td className="product-code">{item.code}</td><td>{item.currency}</td><td>{item.issuer ?? "—"}</td><td className="purchase-banks">{item.purchaseBanks.length ? item.purchaseBanks.join("、") : "—"}</td><td>{item.riskLevel ?? "—"}</td><td>{item.source === "excel" ? "Excel" : "手工"}</td><td><button className="icon-button" onClick={() => onEdit({ entityType: "product", record: item })} aria-label={`编辑${item.name}`}><Pencil /></button></td></tr>)}</tbody></table></div>}
      {section === "accounts" && <div className="table-scroll"><table><thead><tr><th>账户</th><th>币种</th><th>来源</th><th /></tr></thead><tbody>{data.accounts.map((item) => <tr key={item.id}><td><strong>{item.institution}</strong><span>{item.name}</span></td><td>{item.currency}</td><td>{item.source === "excel" ? "Excel" : "手工"}</td><td><button className="icon-button" onClick={() => onEdit({ entityType: "account", record: item })} aria-label={`编辑${item.name}`}><Pencil /></button></td></tr>)}</tbody></table></div>}
      {section === "deposits" && <div className="table-scroll"><table><thead><tr><th>存款</th><th>账户</th><th>币种</th><th className="number">本金</th><th>到期日</th><th>年利率</th><th>状态</th><th /></tr></thead><tbody>{data.deposits.map((item) => <tr key={item.id}><td><strong>{item.name}</strong><span>{item.source === "excel" ? "Excel 导入" : "手工录入"}</span></td><td>{item.institution}</td><td>{item.currency}</td><td className="number">{formatMoney(item.principal, item.currency)}</td><td>{item.maturityDate}</td><td>{formatPercent(item.annualRate)}</td><td><span className="status">{item.status === "active" ? "持有中" : item.status === "matured" ? "已到期" : "已取消"}</span></td><td><button className="icon-button" onClick={() => onEdit({ entityType: "deposit", record: item })} aria-label={`编辑${item.name}`}><Pencil /></button></td></tr>)}</tbody></table></div>}
    </article>
  );
}

function ReconciliationView({ data, onReconcile, onOpenIssue }: { data: ReconciliationCenter; onReconcile: (account: AccountReconciliation) => void; onOpenIssue: (issue: QualityIssue) => void }) {
  const [section, setSection] = useState<"accounts" | "quality">("accounts");
  const [severity, setSeverity] = useState("all");
  const [searchText, setSearchText] = useState("");
  const matched = data.accounts.filter((item) => item.status === "matched").length;
  const differences = data.accounts.filter((item) => item.status === "difference").length;
  const missing = data.accounts.filter((item) => item.status === "missing" && item.trackedTotal > 0.000001).length;
  const filteredIssues = data.issues.filter((issue) => {
    const keyword = searchText.trim().toLowerCase();
    return (severity === "all" || issue.severity === severity) && (!keyword || `${issue.category} ${issue.title} ${issue.detail}`.toLowerCase().includes(keyword));
  });
  return (
    <>
      <section className="metrics-grid reconciliation-metrics"><Metric label="已核对账户" value={`${matched}`} detail={`共 ${data.accounts.length} 个账户`} positive={matched > 0} /><Metric label="存在差额" value={`${differences}`} detail="银行总资产与账本不一致" positive={differences === 0} /><Metric label="尚未核对" value={`${missing}`} detail="有账本资产但没有余额快照" positive={missing === 0} /><Metric label="数据质量问题" value={`${data.issueCount}`} detail={`${data.highPriorityCount} 项高优先级`} positive={data.highPriorityCount === 0} /></section>
      <article className="panel reconciliation-panel">
        <div className="panel-header"><div><h2>对账与数据质量</h2><span>银行显示总资产 − 软件中的理财市值与有效定存本金 = 对账差额</span></div><div className="segmented"><button className={section === "accounts" ? "active" : ""} onClick={() => setSection("accounts")}>账户对账</button><button className={section === "quality" ? "active" : ""} onClick={() => setSection("quality")}>问题清单 {data.issueCount}</button></div></div>
        {section === "accounts" && <div className="table-scroll"><table className="reconciliation-table"><thead><tr><th>账户</th><th>币种</th><th className="number">理财市值</th><th className="number">定存本金</th><th className="number">账本资产</th><th className="number">银行总资产</th><th className="number">差额</th><th>状态</th><th /></tr></thead><tbody>{data.accounts.map((item) => <tr key={item.accountId}><td><strong>{item.institution}</strong><span>{item.name}{item.balanceDate ? ` · 核对于 ${item.balanceDate}` : ""}</span></td><td>{item.currency}</td><td className="number">{formatMoney(item.wealthValue, item.currency)}</td><td className="number">{formatMoney(item.depositValue, item.currency)}</td><td className="number"><strong>{formatMoney(item.trackedTotal, item.currency)}</strong></td><td className="number">{item.actualBalance === null ? "—" : formatMoney(item.actualBalance, item.currency)}</td><td className={`number ${item.difference === null ? "" : Math.abs(item.difference) <= (item.currency === "USD" ? 0.01 : 1) ? "gain" : "loss"}`}>{item.difference === null ? "—" : formatMoney(item.difference, item.currency)}</td><td><span className={`reconciliation-status status-${item.status}`}>{item.status === "matched" ? "已一致" : item.status === "difference" ? "有差额" : "未核对"}</span></td><td><button className="button secondary compact" onClick={() => onReconcile(item)}>录入余额</button></td></tr>)}</tbody></table></div>}
        {section === "quality" && <><div className="quality-toolbar"><label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索问题" /></label><select value={severity} onChange={(event) => setSeverity(event.target.value)}><option value="all">全部优先级</option><option value="high">高优先级</option><option value="medium">中优先级</option><option value="low">低优先级</option></select><span>显示 {filteredIssues.length}/{data.issues.length} 项</span></div><div className="quality-list">{filteredIssues.map((issue) => <article key={issue.key} className={`quality-item severity-${issue.severity}`}><div className="quality-severity">{issue.severity === "high" ? "高" : issue.severity === "medium" ? "中" : "低"}</div><div><span>{issue.category}</span><strong>{issue.title}</strong><p>{issue.detail}</p></div><button className="button secondary compact" onClick={() => onOpenIssue(issue)}>前往处理</button></article>)}{!filteredIssues.length && <div className="quality-empty"><CheckCircle2 />当前筛选下没有待处理问题</div>}</div></>}
      </article>
    </>
  );
}

function BalanceReconciliationModal({ account, onClose, onSave }: { account: AccountReconciliation; onClose: () => void; onSave: (input: BalanceSnapshotInput) => Promise<void> }) {
  const [balanceDate, setBalanceDate] = useState(account.balanceDate ?? new Date().toISOString().slice(0, 10));
  const [actualBalance, setActualBalance] = useState(account.actualBalance?.toString() ?? "");
  const [note, setNote] = useState(account.note ?? "");
  const [submitting, setSubmitting] = useState(false);
  const actual = actualBalance.trim() === "" ? null : Number(actualBalance);
  const difference = actual !== null && Number.isFinite(actual) ? actual - account.trackedTotal : null;
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (actual === null || !Number.isFinite(actual) || actual < 0) return;
    setSubmitting(true);
    try {
      await onSave({ accountId: account.accountId, balanceDate, actualBalance: actual, note: note.trim() || null });
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal reconciliation-modal" role="dialog" aria-modal="true" aria-labelledby="reconciliation-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="reconciliation-title">核对 {account.institution}</h2><p>{account.name} · {account.currency}</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="reconciliation-breakdown"><div><span>理财市值</span><strong>{formatMoney(account.wealthValue, account.currency)}</strong></div><div><span>有效定存本金</span><strong>{formatMoney(account.depositValue, account.currency)}</strong></div><div><span>账本资产合计</span><strong>{formatMoney(account.trackedTotal, account.currency)}</strong></div></div>
        <form className="entry-form reconciliation-form" onSubmit={(event) => void submit(event)}><label>对账日期<input required type="date" value={balanceDate} onChange={(event) => setBalanceDate(event.target.value)} /></label><label>银行显示总资产<input required autoFocus type="number" min="0" step="0.01" value={actualBalance} onChange={(event) => setActualBalance(event.target.value)} placeholder="输入银行页面的账户总资产" /></label><label className="full-field">备注（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="例如：包含活期余额、待入账收益" /></label><div className={`reconciliation-difference full-field ${difference === null ? "" : Math.abs(difference) <= (account.currency === "USD" ? 0.01 : 1) ? "matched" : "unmatched"}`}><span>预计差额</span><strong>{difference === null ? "—" : formatMoney(difference, account.currency)}</strong><small>{difference === null ? "录入银行总资产后自动计算" : Math.abs(difference) <= (account.currency === "USD" ? 0.01 : 1) ? "与账本一致" : "差额可能来自未录入现金、漏记产品或入账时间差"}</small></div><div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><Scale />{submitting ? "保存中…" : "保存对账"}</button></div></form>
      </section>
    </div>
  );
}

const operationLabels: Record<string, string> = {
  BUY: "买入",
  SELL: "卖出",
  REDEEM: "历史赎回",
  PRODUCT_MATURITY: "理财到期",
  VALUATION: "更新市值",
  DEPOSIT_OPEN: "定存开户",
  DEPOSIT_MATURITY: "定存到期",
  DIVIDEND: "分红",
  FEE: "费用",
  TRANSFER: "账户转账",
  REVERSAL: "冲销",
};

function TransactionsView({ transactions, onSelect, loading }: {
  transactions: TransactionRecord[];
  onSelect: (id: number) => void;
  loading: boolean;
}) {
  const [searchText, setSearchText] = useState("");
  const [operationFilter, setOperationFilter] = useState("all");
  const [currencyFilter, setCurrencyFilter] = useState("all");
  const [institutionFilter, setInstitutionFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const institutions = useMemo(() => [...new Set(transactions.map((item) => item.institution).filter((item): item is string => Boolean(item)))].sort(), [transactions]);
  const filtered = useMemo(() => {
    const keyword = searchText.trim().toLowerCase();
    return transactions.filter((item) => {
      const matchesKeyword = !keyword || `${item.title} ${item.code ?? ""} ${item.institution ?? ""} ${item.note ?? ""}`.toLowerCase().includes(keyword);
      const matchesOperation = operationFilter === "all" || item.operation === operationFilter;
      const matchesCurrency = currencyFilter === "all" || item.currency === currencyFilter;
      const matchesInstitution = institutionFilter === "all" || item.institution === institutionFilter;
      const matchesDate = (!dateFrom || item.tradeDate >= dateFrom) && (!dateTo || item.tradeDate <= dateTo);
      const matchesStatus = statusFilter === "all" || (statusFilter === "effective" && item.reversedBy === null && item.operation !== "REVERSAL") || (statusFilter === "reversed" && (item.reversedBy !== null || item.operation === "REVERSAL")) || (statusFilter === "review" && item.note?.includes("待核对"));
      return matchesKeyword && matchesOperation && matchesCurrency && matchesInstitution && matchesDate && matchesStatus;
    });
  }, [transactions, searchText, operationFilter, currencyFilter, institutionFilter, statusFilter, dateFrom, dateTo]);
  const gains = filtered.reduce<Record<CurrencyCode, number>>((totals, item) => {
    if (item.reversedBy === null && item.operation !== "REVERSAL") totals[item.currency] += item.realizedGain;
    return totals;
  }, { CNY: 0, USD: 0 });
  return (
    <article className="panel table-panel">
      <div className="panel-header"><div><h2>交易流水</h2><span>显示 {filtered.length}/{transactions.length} 笔 · 筛选结果已实现收益 {formatMoney(gains.CNY, "CNY")} / {formatMoney(gains.USD, "USD")}</span></div><button className="button secondary compact" type="button" onClick={() => { setSearchText(""); setOperationFilter("all"); setCurrencyFilter("all"); setInstitutionFilter("all"); setStatusFilter("all"); setDateFrom(""); setDateTo(""); }}><ListFilter />清除筛选</button></div>
      <div className="filter-bar transaction-filters"><label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索产品、代码、银行或备注" /></label><select value={operationFilter} onChange={(event) => setOperationFilter(event.target.value)}><option value="all">全部操作</option>{Object.entries(operationLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select><select value={currencyFilter} onChange={(event) => setCurrencyFilter(event.target.value)}><option value="all">全部币种</option><option value="CNY">人民币</option><option value="USD">美元</option></select><select value={institutionFilter} onChange={(event) => setInstitutionFilter(event.target.value)}><option value="all">全部银行</option>{institutions.map((institution) => <option key={institution} value={institution}>{institution}</option>)}</select><select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}><option value="all">全部状态</option><option value="effective">有效流水</option><option value="reversed">冲销记录</option><option value="review">待核对</option></select><div className="transaction-date-range"><label className="date-filter">从<input type="date" value={dateFrom} onChange={(event) => setDateFrom(event.target.value)} /></label><label className="date-filter">至<input type="date" value={dateTo} onChange={(event) => setDateTo(event.target.value)} /></label></div></div>
      <div className="table-scroll"><table className="transaction-table"><colgroup><col className="transaction-date-col" /><col className="transaction-operation-col" /><col /><col className="transaction-currency-col" /><col className="transaction-money-col" /><col className="transaction-money-col" /><col className="transaction-money-col" /><col className="transaction-note-col" /></colgroup><thead><tr><th>日期</th><th>操作</th><th>产品</th><th>币种</th><th className="number">现金金额</th><th className="number">核销成本</th><th className="number">已实现收益</th><th>备注</th></tr></thead><tbody>
        {filtered.map((item) => {
          const incoming = ["SELL", "REDEEM", "PRODUCT_MATURITY", "DEPOSIT_MATURITY", "DIVIDEND"].includes(item.operation);
          const cashless = item.operation === "VALUATION";
          const neutral = ["REVERSAL", "TRANSFER"].includes(item.operation);
          const reversed = item.reversedBy !== null;
          const hasRealized = (incoming || item.operation === "FEE" || item.operation === "REVERSAL") && (item.operation !== "REDEEM" || Math.abs(item.realizedGain) > 0.000001);
          return <tr className={`transaction-row ${reversed ? "reversed-row" : ""}`} key={item.id} onClick={() => onSelect(item.id)} aria-busy={loading}><td className="nowrap">{item.tradeDate}</td><td><span className={`operation-tag ${neutral ? "neutral" : incoming ? "incoming" : "outgoing"}`}>{operationLabels[item.operation] ?? item.operation}</span>{reversed && <span className="reversed-label">已冲销</span>}</td><td><strong>{item.title}</strong><span>{[item.code, item.institution].filter(Boolean).join(" · ")}</span></td><td>{item.currency}</td><td className={`number ${incoming ? "gain" : ""}`}>{cashless ? "—" : <>{incoming ? "+" : neutral ? "↔" : "-"}{formatMoney(item.amount, item.currency)}</>}</td><td className="number">{Math.abs(item.costBasis) > 0.000001 ? formatMoney(item.costBasis, item.currency) : "—"}</td><td className={`number ${item.realizedGain >= 0 ? "gain" : "loss"}`}>{hasRealized ? formatMoney(item.realizedGain, item.currency) : "—"}</td><td className="note-cell">{item.note ?? "—"}</td></tr>;
        })}
        {!filtered.length && <tr><td colSpan={8}><div className="table-empty">没有符合当前筛选条件的流水</div></td></tr>}
      </tbody></table></div>
    </article>
  );
}

function DataSafetyModal({ busy, onClose, onExport, onBackup, onRestore }: {
  busy: boolean;
  onClose: () => void;
  onExport: () => void;
  onBackup: () => void;
  onRestore: () => void;
}) {
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal data-modal" role="dialog" aria-modal="true" aria-labelledby="data-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="data-title">数据安全</h2><p>导出可阅读数据，并为本地数据库建立可恢复快照</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="data-actions">
          <article><div className="data-action-icon"><Download /></div><div><strong>导出 Excel 数据包</strong><p>包含资产概览、持仓、流水、定存、买入批次和市值历史。</p></div><button className="button secondary" disabled={busy} onClick={onExport}>选择保存位置</button></article>
          <article><div className="data-action-icon"><HardDriveDownload /></div><div><strong>备份完整数据库</strong><p>保留全部交易关联、冲销记录和历史快照，可用于完整恢复。</p></div><button className="button secondary" disabled={busy} onClick={onBackup}>创建备份</button></article>
          <article className="restore-action"><div className="data-action-icon"><RotateCcw /></div><div><strong>从备份恢复</strong><p>恢复前会自动备份当前数据库，并检查所选文件完整性。</p></div><button className="button danger-ghost" disabled={busy} onClick={onRestore}>选择备份</button></article>
        </div>
        <div className="calculation-note">程序每天首次启动、重新导入 Excel、撤销交易和恢复数据库前，都会自动创建安全快照。</div>
      </section>
    </div>
  );
}

function TransactionDetailModal({ detail, onClose, onReverse, onEdit, onConfirm }: {
  detail: TransactionDetail;
  onClose: () => void;
  onReverse: (id: number) => void;
  onEdit: (detail: TransactionDetail) => void;
  onConfirm: (detail: TransactionDetail) => void;
}) {
  const status = detail.reversedBy !== null ? `已由交易 #${detail.reversedBy} 冲销` : detail.reversalOf !== null ? `冲销交易 #${detail.reversalOf}` : "有效";
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal detail-modal" role="dialog" aria-modal="true" aria-labelledby="detail-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="detail-title">交易 #{detail.id}</h2><p>{detail.title}{detail.code ? ` · ${detail.code}` : ""}</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="detail-status"><span className={`operation-tag ${detail.operation === "REVERSAL" ? "neutral" : ""}`}>{operationLabels[detail.operation] ?? detail.operation}</span><span>{status}</span><span>{detail.source === "manual" ? "手工录入" : "Excel 导入"}</span></div>
        <dl className="detail-grid">
          <div><dt>操作日期</dt><dd>{detail.tradeDate}</dd></div>
          <div><dt>银行/渠道</dt><dd>{detail.institution ?? "—"}</dd></div>
          <div><dt>现金金额</dt><dd>{detail.operation === "VALUATION" ? "—" : formatMoney(detail.amount, detail.currency)}</dd></div>
          <div><dt>核销成本</dt><dd>{Math.abs(detail.costBasis) > 0.000001 ? formatMoney(detail.costBasis, detail.currency) : "—"}</dd></div>
          <div><dt>已实现收益</dt><dd>{Math.abs(detail.realizedGain) > 0.000001 ? formatMoney(detail.realizedGain, detail.currency) : "—"}</dd></div>
          <div><dt>币种</dt><dd>{detail.currency}</dd></div>
          {detail.valuationBefore !== null && <div><dt>操作前市值</dt><dd>{formatMoney(detail.valuationBefore, detail.currency)}</dd></div>}
          {detail.valuationAfter !== null && <div><dt>操作后市值</dt><dd>{formatMoney(detail.valuationAfter, detail.currency)}</dd></div>}
          <div className="detail-note"><dt>备注</dt><dd>{detail.note ?? "—"}</dd></div>
        </dl>
        {detail.needsReview && <div className="calculation-note warning-note"><AlertTriangle />这笔流水根据 Excel 中的结束日期自动生成，目前到账金额只是按原买入金额占位。请根据银行流水核对实际日期和到账金额。</div>}
        {!detail.canEdit && detail.editHint && !detail.canConfirmHistoricalRedemption && detail.operation !== "REVERSAL" && detail.reversedBy === null && <div className="calculation-note">{detail.editHint}</div>}
        <div className="entry-actions"><button className="button secondary" onClick={onClose}>关闭</button>{detail.canEdit && <button className="button primary" onClick={() => onEdit(detail)}><Pencil />修改记录</button>}{detail.canConfirmHistoricalRedemption && <button className="button primary" onClick={() => onConfirm(detail)}><CheckCircle2 />{detail.needsReview ? "核对赎回" : "修改核对"}</button>}{detail.canReverse && <button className="button danger" onClick={() => onReverse(detail.id)}><RotateCcw />安全撤销</button>}</div>
      </section>
    </div>
  );
}

function TransactionEditModal({ detail, onClose, onSave }: { detail: TransactionDetail; onClose: () => void; onSave: (input: TransactionEditInput) => Promise<void> }) {
  const [tradeDate, setTradeDate] = useState(detail.tradeDate);
  const [amount, setAmount] = useState(detail.amount.toString());
  const [note, setNote] = useState(detail.note ?? "");
  const [submitting, setSubmitting] = useState(false);
  const parsedAmount = amount.trim() === "" ? null : Number(amount);
  const amountLabel: Record<string, string> = {
    BUY: "买入金额",
    SELL: "实际到账金额",
    PRODUCT_MATURITY: "实际到账金额",
    VALUATION: "当前总市值",
    DIVIDEND: "分红金额",
    FEE: "费用金额",
    TRANSFER: "转账金额",
    DEPOSIT_OPEN: "存款本金",
    DEPOSIT_MATURITY: "实际到账金额",
  };
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (parsedAmount === null || !Number.isFinite(parsedAmount) || parsedAmount <= 0) return;
    setSubmitting(true);
    try {
      await onSave({ transactionId: detail.id, tradeDate, amount: parsedAmount, note: note.trim() || null });
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal redemption-modal" role="dialog" aria-modal="true" aria-labelledby="transaction-edit-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="transaction-edit-title">修改交易记录</h2><p>{detail.title}{detail.code ? ` · ${detail.code}` : ""}</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="redemption-context"><div><span>操作类型</span><strong>{operationLabels[detail.operation] ?? detail.operation}</strong></div><div><span>银行/渠道</span><strong>{detail.institution ?? "—"}</strong></div><div><span>币种</span><strong>{detail.currency}</strong></div></div>
        <form className="entry-form redemption-form" onSubmit={(event) => void submit(event)}>
          <label>操作日期<input required type="date" value={tradeDate} onChange={(event) => setTradeDate(event.target.value)} /></label>
          <label>{amountLabel[detail.operation] ?? "金额"}<input required autoFocus type="number" min="0.01" step="0.01" value={amount} onChange={(event) => setAmount(event.target.value)} /></label>
          <label className="full-field">备注（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="补充银行流水、用途或核对说明" /></label>
          <div className="calculation-note full-field">操作类型、产品、银行和币种保持不变；保存后会自动重算 FIFO 成本、持仓市值与收益。{detail.source === "excel" ? "这条 Excel 修正会在以后重新导入时继续沿用。" : "修改前会自动备份数据库，并保留审计记录。"}</div>
          <div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><Save />{submitting ? "保存中…" : "保存修改"}</button></div>
        </form>
      </section>
    </div>
  );
}

function HistoricalRedemptionModal({ detail, onClose, onSave }: { detail: TransactionDetail; onClose: () => void; onSave: (input: HistoricalRedemptionInput) => Promise<void> }) {
  const [tradeDate, setTradeDate] = useState(detail.tradeDate);
  const [proceeds, setProceeds] = useState(detail.amount.toString());
  const [note, setNote] = useState(detail.needsReview ? "" : (detail.note ?? "").replace(/^历史赎回已核对；?/, ""));
  const [submitting, setSubmitting] = useState(false);
  const actualProceeds = proceeds.trim() === "" ? null : Number(proceeds);
  const realizedGain = actualProceeds === null || !Number.isFinite(actualProceeds) ? null : actualProceeds - detail.costBasis;
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (actualProceeds === null || !Number.isFinite(actualProceeds) || actualProceeds < 0) return;
    setSubmitting(true);
    try {
      await onSave({ transactionId: detail.id, tradeDate, proceeds: actualProceeds, note: note.trim() || null });
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal redemption-modal" role="dialog" aria-modal="true" aria-labelledby="redemption-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="redemption-title">核对历史赎回</h2><p>{detail.title}{detail.code ? ` · ${detail.code}` : ""}</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="redemption-context"><div><span>银行/渠道</span><strong>{detail.institution ?? "—"}</strong></div><div><span>原买入成本</span><strong>{formatMoney(detail.costBasis, detail.currency)}</strong></div><div><span>{detail.needsReview ? "当前占位金额" : "当前到账金额"}</span><strong>{formatMoney(detail.amount, detail.currency)}</strong></div></div>
        <form className="entry-form redemption-form" onSubmit={(event) => void submit(event)}><label>实际赎回日期<input required type="date" value={tradeDate} onChange={(event) => setTradeDate(event.target.value)} /></label><label>银行实际到账金额<input required autoFocus type="number" min="0" step="0.01" value={proceeds} onChange={(event) => setProceeds(event.target.value)} /></label><label className="full-field">核对说明（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="例如：银行流水已核对、含分红或手续费" /></label><div className={`redemption-result full-field ${realizedGain === null ? "" : realizedGain >= 0 ? "gain" : "loss"}`}><span>本批次已实现收益</span><strong>{realizedGain === null ? "—" : formatMoney(realizedGain, detail.currency)}</strong><small>实际到账金额 − 原买入成本；保存后将更新收益分析和 XIRR</small></div><div className="calculation-note full-field">核对结果保存在软件数据库中，不修改原 Excel；以后重新导入同一批次时会自动沿用。</div><div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><CheckCircle2 />{submitting ? "保存中…" : "确认核对"}</button></div></form>
      </section>
    </div>
  );
}

const entryOperations: { value: EntryOperation; label: string; help: string }[] = [
  { value: "BUY", label: "买入理财", help: "自动新增批次" },
  { value: "SELL", label: "卖出理财", help: "自动 FIFO 核销" },
  { value: "PRODUCT_MATURITY", label: "理财到期", help: "全部结清" },
  { value: "DIVIDEND", label: "收到分红", help: "计入已实现收益" },
  { value: "FEE", label: "支付费用", help: "计入收益扣减" },
  { value: "TRANSFER", label: "账户转账", help: "不影响总资产" },
  { value: "DEPOSIT_OPEN", label: "新增定存", help: "加入到期日历" },
  { value: "DEPOSIT_MATURITY", label: "定存到期", help: "拆分本金利息" },
];

function MasterDataEditModal({ target, accounts, onClose, onSave }: { target: EditableRecord; accounts: AccountRecord[]; onClose: () => void; onSave: (input: MasterDataUpdate) => Promise<void> }) {
  const record = target.record;
  const product = target.entityType === "product" ? target.record : null;
  const account = target.entityType === "account" ? target.record : null;
  const deposit = target.entityType === "deposit" ? target.record : null;
  const [name, setName] = useState(record.name);
  const [code, setCode] = useState(product?.code ?? "");
  const [institution, setInstitution] = useState(account?.institution ?? "");
  const [currency, setCurrency] = useState<CurrencyCode>(record.currency);
  const [issuer, setIssuer] = useState(product?.issuer ?? "");
  const [riskLevel, setRiskLevel] = useState(product?.riskLevel ?? "");
  const [accountId, setAccountId] = useState((deposit?.accountId ?? accounts.find((item) => item.currency === record.currency)?.id ?? 0).toString());
  const [principal, setPrincipal] = useState(deposit?.principal.toString() ?? "");
  const [startDate, setStartDate] = useState(deposit?.startDate ?? "");
  const [maturityDate, setMaturityDate] = useState(deposit?.maturityDate ?? "");
  const [annualRate, setAnnualRate] = useState(deposit ? (deposit.annualRate * 100).toString() : "");
  const [submitting, setSubmitting] = useState(false);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    try {
      await onSave({
        entityType: target.entityType,
        id: record.id,
        name,
        code: product ? code : null,
        institution: account ? institution : null,
        accountId: deposit ? Number(accountId) || null : null,
        currency,
        issuer: product ? issuer.trim() || null : null,
        riskLevel: product ? riskLevel.trim() || null : null,
        principal: deposit ? Number(principal) || null : null,
        startDate: deposit ? startDate || null : null,
        maturityDate: deposit ? maturityDate || null : null,
        annualRate: deposit ? (Number(annualRate) || 0) / 100 : null,
      });
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal edit-modal" role="dialog" aria-modal="true" aria-labelledby="edit-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="edit-title">编辑{target.entityType === "product" ? "产品" : target.entityType === "account" ? "账户" : "定期存款"}</h2><p>保存前自动备份，修改内容写入本地审计记录</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <form className="entry-form edit-form" onSubmit={(event) => void submit(event)}>
          <label className="full-field">名称<input required value={name} onChange={(event) => setName(event.target.value)} /></label>
          {product && <><label>产品代码<input required value={code} onChange={(event) => setCode(event.target.value)} /></label><label>币种<select value={currency} onChange={(event) => setCurrency(event.target.value as CurrencyCode)}><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select></label><label>发行机构<input value={issuer} onChange={(event) => setIssuer(event.target.value)} /></label><label>风险等级<input value={riskLevel} onChange={(event) => setRiskLevel(event.target.value)} placeholder="例如 R2" /></label><label className="full-field">购买银行<input value={product.purchaseBanks.join("、") || "尚无购买记录"} disabled /></label></>}
          {account && <><label>银行/机构<input required value={institution} onChange={(event) => setInstitution(event.target.value)} /></label><label>币种<select value={currency} onChange={(event) => setCurrency(event.target.value as CurrencyCode)}><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select></label></>}
          {deposit && <><label>存款账户<select required value={accountId} onChange={(event) => { setAccountId(event.target.value); const selected = accounts.find((item) => item.id.toString() === event.target.value); if (selected) setCurrency(selected.currency); }}><option value="">请选择</option>{accounts.map((item) => <option value={item.id} key={item.id}>{item.institution} · {item.name} · {item.currency}</option>)}</select></label><label>币种<input value={currency} disabled /></label><label>本金<input required type="number" min="0.01" step="0.01" value={principal} onChange={(event) => setPrincipal(event.target.value)} /></label><label>年利率（%）<input required type="number" min="0" max="100" step="0.01" value={annualRate} onChange={(event) => setAnnualRate(event.target.value)} /></label><label>起息日（可选）<input type="date" value={startDate} onChange={(event) => setStartDate(event.target.value)} /></label><label>到期日<input required type="date" value={maturityDate} onChange={(event) => setMaturityDate(event.target.value)} /></label></>}
          <div className="calculation-note full-field">已有账本记录的产品和账户不能修改币种；Excel 导入项目的手工修正会在后续导入时保留。</div>
          <div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><Save />{submitting ? "保存中…" : "保存修改"}</button></div>
        </form>
      </section>
    </div>
  );
}

function EntryModal({ holdings, deposits, accounts, onClose, onSave }: {
  holdings: Holding[];
  deposits: MaturityEvent[];
  accounts: AccountRecord[];
  onClose: () => void;
  onSave: (input: EntryInput) => Promise<void>;
}) {
  const today = new Date().toISOString().slice(0, 10);
  const [operation, setOperation] = useState<EntryOperation>("BUY");
  const [holdingChoice, setHoldingChoice] = useState("new");
  const [depositChoice, setDepositChoice] = useState(deposits[0]?.id.toString() ?? "");
  const [sourceAccountChoice, setSourceAccountChoice] = useState(accounts[0]?.id.toString() ?? "");
  const [targetAccountChoice, setTargetAccountChoice] = useState(accounts[1]?.id.toString() ?? "");
  const [name, setName] = useState("");
  const [code, setCode] = useState("");
  const [institution, setInstitution] = useState("");
  const [currency, setCurrency] = useState<CurrencyCode>("CNY");
  const [amount, setAmount] = useState("");
  const [marketValue, setMarketValue] = useState("");
  const [tradeDate, setTradeDate] = useState(today);
  const [maturityDate, setMaturityDate] = useState("");
  const [annualRate, setAnnualRate] = useState("");
  const [note, setNote] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const selectedHolding = holdings.find((item) => `${item.productId}:${item.accountId}` === holdingChoice);
  const selectedDeposit = deposits.find((item) => item.id.toString() === depositChoice);
  const needsHolding = ["SELL", "PRODUCT_MATURITY", "VALUATION", "DIVIDEND", "FEE"].includes(operation);
  const isExistingBuy = operation === "BUY" && holdingChoice !== "new" && selectedHolding;

  const changeOperation = (next: EntryOperation) => {
    setOperation(next);
    setAmount("");
    setMarketValue("");
    if (["SELL", "PRODUCT_MATURITY", "VALUATION"].includes(next)) {
      setHoldingChoice(holdings[0] ? `${holdings[0].productId}:${holdings[0].accountId}` : "");
    } else if (next === "BUY") {
      setHoldingChoice("new");
    }
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const operationHolding = needsHolding || isExistingBuy ? selectedHolding : undefined;
    const input: EntryInput = {
      operation,
      productId: operationHolding?.productId ?? null,
      accountId: operation === "TRANSFER" ? Number(sourceAccountChoice) || null : needsHolding ? operationHolding?.accountId ?? null : null,
      transferAccountId: operation === "TRANSFER" ? Number(targetAccountChoice) || null : null,
      depositId: operation === "DEPOSIT_MATURITY" ? selectedDeposit?.id ?? null : null,
      productName: operation === "DEPOSIT_OPEN" || (operation === "BUY" && !isExistingBuy) ? name : null,
      productCode: operation === "BUY" && !isExistingBuy ? code : null,
      institution: operation === "BUY" ? (isExistingBuy ? selectedHolding.channel : institution) : operation === "DEPOSIT_OPEN" ? institution : null,
      currency: operation === "BUY" ? (isExistingBuy ? selectedHolding.currency : currency) : operation === "DEPOSIT_OPEN" ? currency : operation === "DEPOSIT_MATURITY" ? selectedDeposit?.currency ?? null : operation === "TRANSFER" ? accounts.find((item) => item.id.toString() === sourceAccountChoice)?.currency ?? null : selectedHolding?.currency ?? null,
      amount: operation === "VALUATION" ? null : Number(amount) || null,
      marketValue: operation === "VALUATION" ? Number(marketValue) || null : null,
      tradeDate,
      maturityDate: operation === "DEPOSIT_OPEN" ? maturityDate : null,
      annualRate: operation === "DEPOSIT_OPEN" ? (Number(annualRate) || 0) / 100 : null,
      note: note.trim() || null,
    };
    setSubmitting(true);
    try {
      await onSave(input);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal entry-modal" role="dialog" aria-modal="true" aria-labelledby="entry-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="entry-title">记一笔</h2><p>选择发生的操作，持仓批次和成本由程序自动处理</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="operation-grid">
          {entryOperations.map((item) => <button type="button" key={item.value} className={operation === item.value ? "active" : ""} onClick={() => changeOperation(item.value)}><strong>{item.label}</strong><span>{item.help}</span></button>)}
        </div>
        <form className="entry-form" onSubmit={(event) => void submit(event)}>
          {operation === "BUY" && <label className="full-field">产品<select value={holdingChoice} onChange={(event) => setHoldingChoice(event.target.value)}><option value="new">＋ 新理财产品</option>{holdings.map((item) => <option key={`${item.productId}:${item.accountId}`} value={`${item.productId}:${item.accountId}`}>{item.name} · {item.channel} · {item.currency}</option>)}</select></label>}
          {needsHolding && <label className="full-field">理财持仓<select required value={holdingChoice} onChange={(event) => setHoldingChoice(event.target.value)}><option value="">请选择</option>{holdings.map((item) => <option key={`${item.productId}:${item.accountId}`} value={`${item.productId}:${item.accountId}`}>{item.name} · {item.channel} · 市值 {formatMoney(item.marketValue, item.currency)}</option>)}</select></label>}
          {operation === "DEPOSIT_MATURITY" && <label className="full-field">到期存款<select required value={depositChoice} onChange={(event) => setDepositChoice(event.target.value)}><option value="">请选择</option>{deposits.map((item) => <option key={item.id} value={item.id}>{item.name} · {item.maturityDate} · {formatMoney(item.amount, item.currency)}</option>)}</select></label>}
          {operation === "TRANSFER" && <><label>转出账户<select required value={sourceAccountChoice} onChange={(event) => setSourceAccountChoice(event.target.value)}><option value="">请选择</option>{accounts.map((item) => <option key={item.id} value={item.id}>{item.institution} · {item.name} · {item.currency}</option>)}</select></label><label>转入账户<select required value={targetAccountChoice} onChange={(event) => setTargetAccountChoice(event.target.value)}><option value="">请选择</option>{accounts.filter((item) => item.id.toString() !== sourceAccountChoice).map((item) => <option key={item.id} value={item.id}>{item.institution} · {item.name} · {item.currency}</option>)}</select></label></>}

          {operation === "BUY" && !isExistingBuy && <><label className="full-field">产品名称<input required value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：稳富固收增强" /></label><label>产品代码<input required value={code} onChange={(event) => setCode(event.target.value)} /></label><label>购买渠道<input required value={institution} onChange={(event) => setInstitution(event.target.value)} placeholder="例如：中国银行" /></label><label>币种<select value={currency} onChange={(event) => setCurrency(event.target.value as CurrencyCode)}><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select></label></>}
          {operation === "DEPOSIT_OPEN" && <><label className="full-field">存款名称<input required value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：整存整取-3年" /></label><label>存款银行<input required value={institution} onChange={(event) => setInstitution(event.target.value)} /></label><label>币种<select value={currency} onChange={(event) => setCurrency(event.target.value as CurrencyCode)}><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select></label><label>到期日<input required type="date" value={maturityDate} onChange={(event) => setMaturityDate(event.target.value)} /></label><label>年利率（%）<input required type="number" min="0" max="100" step="0.01" value={annualRate} onChange={(event) => setAnnualRate(event.target.value)} placeholder="2.85" /></label></>}

          {operation !== "VALUATION" && <label>{operation === "BUY" ? "买入金额" : operation === "DEPOSIT_OPEN" ? "存款本金" : operation === "DIVIDEND" ? "分红金额" : operation === "FEE" ? "费用金额" : operation === "TRANSFER" ? "转账金额" : "实际到账金额"}<input required type="number" min="0.01" step="0.01" value={amount} onChange={(event) => setAmount(event.target.value)} /></label>}
          {operation === "VALUATION" && <label>当前总市值<input required type="number" min="0.01" step="0.01" value={marketValue} onChange={(event) => setMarketValue(event.target.value)} /></label>}
          <label>操作日期<input required type="date" value={tradeDate} onChange={(event) => setTradeDate(event.target.value)} /></label>
          <label className="full-field">备注（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="银行流水说明、确认号等" /></label>

          {operation === "SELL" && selectedHolding && <div className="calculation-note full-field">将以当前市值 {formatMoney(selectedHolding.marketValue, selectedHolding.currency)} 为基准，按卖出比例计算应核销成本，并从最早的买入批次开始自动扣减。</div>}
          {operation === "PRODUCT_MATURITY" && selectedHolding && <div className="calculation-note full-field">将结清全部剩余成本 {formatMoney(selectedHolding.cost, selectedHolding.currency)}，到账金额与成本的差额计入已实现收益。</div>}
          {operation === "VALUATION" && <div className="calculation-note full-field">这里只更新产品总市值，不改变买入成本；旧市值会保留为历史快照。</div>}
          {operation === "DIVIDEND" && <div className="calculation-note full-field">分红会计入已实现收益，不降低持仓成本，也不会自动修改当前市值。</div>}
          {operation === "FEE" && <div className="calculation-note full-field">费用作为负收益记录，不改变持仓成本和当前市值。</div>}
          {operation === "TRANSFER" && <div className="calculation-note full-field">账户间内部转账仅保留资金路径，不计入收益，也不改变当前统计的总资产。</div>}

          <div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><Save />{submitting ? "保存中…" : "保存操作"}</button></div>
        </form>
      </section>
    </div>
  );
}

export default App;
