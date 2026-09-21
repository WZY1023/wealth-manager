import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  Activity,
  Archive,
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
  Landmark,
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

type View = "overview" | "holdings" | "deposits" | "history" | "transactions" | "calendar" | "analytics" | "masterData" | "reconciliation";
type CurrencyCode = "CNY" | "USD";
type EntryOperation = "BUY" | "SELL" | "PRODUCT_MATURITY" | "VALUATION" | "DIVIDEND" | "FEE" | "TRANSFER" | "DEPOSIT_OPEN" | "DEPOSIT_MATURITY";
type SortDirection = "asc" | "desc";
type SortState<Key extends string> = { key: Key; direction: SortDirection };
type SortValue = string | number | boolean | null | undefined;

type HoldingsSortKey = "productName" | "productCode" | "channel" | "holdingDays" | "cost" | "marketValue" | "gain" | "thirtyDayReturn" | "valuationDate" | "signal";
type HistorySortKey = "productName" | "productCode" | "closeType" | "holdingDays" | "investedCost" | "proceeds" | "dividendFees" | "realizedGain" | "returnRate";
type ProductSortKey = "name" | "code" | "currency" | "issuer" | "purchaseBanks" | "riskLevel" | "source";
type AccountSortKey = "account" | "currency" | "source";
type DepositSortKey = "name" | "institution" | "currency" | "principal" | "maturityDate" | "annualRate" | "status";
type ReconciliationSortKey = "institution" | "usdCnyRate" | "trackedWealthCny" | "actualWealthCny" | "trackedDepositCny" | "actualDepositCny" | "demandCny" | "actualTotalCny" | "status";
type TransactionSortKey = "tradeDate" | "operation" | "productName" | "productCode" | "currency" | "amount" | "costBasis" | "realizedGain" | "note";
type BatchValuationSortKey = "productName" | "productCode" | "marketValue" | "nextMarketValue" | "difference";
type ValuationHistorySortKey = "date" | "marketValue" | "changeRate" | "source";

type CurrencySummary = {
  currency: CurrencyCode;
  wealthValue: number;
  depositValue: number;
  demandValue: number;
  totalValue: number;
  investedCost: number;
  unrealizedGain: number;
};

type Dashboard = {
  asOfDate: string;
  lastImportAt: string | null;
  currencies: CurrencySummary[];
  convertedTotalCny: number;
  hasEstimatedExchangeRate: boolean;
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
  longestHoldingDays: number | null;
  shortestHoldingDays: number | null;
  activeLotCount: number;
  signal: string;
  signalLabel: string;
  signalReason: string;
};

type ValuationHistoryPoint = { id: number; date: string; marketValue: number; changeAmount: number | null; changeRate: number | null; source: string };
type HoldingDetail = Omit<Holding, "id" | "status" | "longestHoldingDays" | "shortestHoldingDays" | "activeLotCount"> & { riskLevel: string | null; history: ValuationHistoryPoint[] };
type ClosedPosition = {
  id: number;
  productId: number;
  accountId: number;
  name: string;
  code: string;
  institution: string;
  currency: CurrencyCode;
  purchaseDate: string | null;
  closeDate: string;
  holdingDays: number | null;
  investedCost: number;
  proceeds: number;
  dividends: number;
  fees: number;
  realizedGain: number;
  returnRate: number;
  annualizedReturn: number | null;
  closeType: "SELL" | "REDEEM" | "PRODUCT_MATURITY";
  source: string;
  needsReview: boolean;
  buyCount: number;
  exitCount: number;
};
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

type InstitutionReconciliation = { institution: string; cnyWealthValue: number; usdWealthValue: number; cnyDepositValue: number; usdDepositValue: number; usdCnyRate: number; trackedWealthCny: number; trackedDepositCny: number; demandCny: number; trackedTotalCny: number; actualWealthCny: number | null; actualDepositCny: number | null; actualTotalCny: number | null; wealthDifference: number | null; depositDifference: number | null; difference: number | null; balanceDate: string | null; note: string | null; hasUsdAssets: boolean; status: "missing" | "matched" | "difference" };
type QualityIssue = { key: string; severity: "high" | "medium" | "low"; category: string; title: string; detail: string; targetView: View; targetId: number | null };
type ReconciliationCenter = { institutions: InstitutionReconciliation[]; issues: QualityIssue[]; issueCount: number; highPriorityCount: number };
type InstitutionSnapshotInput = { institution: string; balanceDate: string; usdCnyRate: number; actualWealthCny: number; actualDepositCny: number; demandCny: number; note: string | null };

type AccountRecord = { id: number; institution: string; name: string; currency: CurrencyCode; source: string };
type ProductRecord = { id: number; code: string; name: string; currency: CurrencyCode; issuer: string | null; purchaseBanks: string[]; riskLevel: string | null; source: string };
type DepositRecord = { id: number; accountId: number; institution: string; name: string; currency: CurrencyCode; principal: number; startDate: string | null; maturityDate: string; annualRate: number; status: string; maturedAt: string | null; proceeds: number | null; source: string };
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
    { currency: "CNY", wealthValue: 0, depositValue: 0, demandValue: 0, totalValue: 0, investedCost: 0, unrealizedGain: 0 },
    { currency: "USD", wealthValue: 0, depositValue: 0, demandValue: 0, totalValue: 0, investedCost: 0, unrealizedGain: 0 },
  ],
  convertedTotalCny: 0,
  hasEstimatedExchangeRate: false,
  holdingCount: 0,
  warningCount: 0,
};

const EMPTY_MASTER_DATA: MasterData = { accounts: [], products: [], deposits: [] };
const EMPTY_ANALYTICS: Analytics = { asOfDate: new Date().toISOString().slice(0, 10), currencies: [] };
const EMPTY_RECONCILIATION: ReconciliationCenter = { institutions: [], issues: [], issueCount: 0, highPriorityCount: 0 };

const viewTitles: Record<View, string> = {
  overview: "资产总览",
  holdings: "当前持仓",
  deposits: "定期存款",
  history: "历史理财",
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
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

function formatPercent(value: number) {
  return new Intl.NumberFormat("zh-CN", { style: "percent", maximumFractionDigits: 2 }).format(value);
}

function formatHoldingDays(value: number | null) {
  return value === null ? "—" : `${value}天`;
}

const sortCollator = new Intl.Collator("zh-CN", { numeric: true, sensitivity: "base" });

function toggleSort<Key extends string>(current: SortState<Key> | null, key: Key): SortState<Key> {
  return current?.key === key
    ? { key, direction: current.direction === "asc" ? "desc" : "asc" }
    : { key, direction: "asc" };
}

function cycleProductSort<Key extends string>(current: SortState<Key> | null, nameKey: Key, codeKey: Key): SortState<Key> {
  if (current?.key === nameKey) {
    return current.direction === "asc" ? { key: nameKey, direction: "desc" } : { key: codeKey, direction: "asc" };
  }
  if (current?.key === codeKey) {
    return current.direction === "asc" ? { key: codeKey, direction: "desc" } : { key: nameKey, direction: "asc" };
  }
  return { key: nameKey, direction: "asc" };
}

function sortRows<Item, Key extends string>(items: Item[], sort: SortState<Key> | null, getValue: (item: Item, key: Key) => SortValue) {
  if (!sort) return items;
  const multiplier = sort.direction === "asc" ? 1 : -1;
  return items.map((item, index) => ({ item, index })).sort((left, right) => {
    const leftValue = getValue(left.item, sort.key);
    const rightValue = getValue(right.item, sort.key);
    const leftMissing = leftValue === null || leftValue === undefined || leftValue === "";
    const rightMissing = rightValue === null || rightValue === undefined || rightValue === "";
    if (leftMissing !== rightMissing) return leftMissing ? 1 : -1;
    if (leftMissing && rightMissing) return left.index - right.index;
    const comparison = typeof leftValue === "number" && typeof rightValue === "number"
      ? leftValue - rightValue
      : sortCollator.compare(String(leftValue), String(rightValue));
    return comparison === 0 ? left.index - right.index : comparison * multiplier;
  }).map(({ item }) => item);
}

function SortableHeader({ label, active, direction, onSort, className, activeDetail }: {
  label: string;
  active: boolean;
  direction: SortDirection;
  onSort: () => void;
  className?: string;
  activeDetail?: string;
}) {
  return <th className={className} aria-sort={active ? direction === "asc" ? "ascending" : "descending" : "none"}><SortControl label={label} active={active} direction={direction} onSort={onSort} activeDetail={activeDetail} /></th>;
}

function SortControl({ label, active, direction, onSort, activeDetail }: {
  label: string;
  active: boolean;
  direction: SortDirection;
  onSort: () => void;
  activeDetail?: string;
}) {
  const sortLabel = active ? `${activeDetail ? `${activeDetail}、` : ""}${direction === "asc" ? "升序" : "降序"}` : "未排序";
  return <button className={`sortable-header ${active ? "active" : ""}`} type="button" onClick={onSort} title={`${label}：${sortLabel}`}><span>{label}</span>{activeDetail && active && <small>{activeDetail}</small>}{active ? direction === "asc" ? <ArrowUp /> : <ArrowDown /> : <span className="sort-placeholder">↕</span>}</button>;
}

type DateInputProps = {
  value: string;
  onChange: (value: string) => void;
  required?: boolean;
  ariaLabel?: string;
};

function DateInput({ value, onChange, required = false, ariaLabel = "日期" }: DateInputProps) {
  const yearRef = useRef<HTMLInputElement>(null);
  const monthRef = useRef<HTMLInputElement>(null);
  const dayRef = useRef<HTMLInputElement>(null);
  const [year = "", month = "", day = ""] = value ? value.split("-", 3) : [];
  const parts = [year, month, day];
  const limits = [4, 2, 2];
  const refs = [yearRef, monthRef, dayRef];
  const labels = ["年", "月", "日"];
  const patterns = ["[0-9]{4}", "(0[1-9]|1[0-2])", "(0[1-9]|[12][0-9]|3[01])"];
  const segmentRequired = required || parts.some((part) => part.length > 0);

  const focusSegment = (index: number) => {
    requestAnimationFrame(() => {
      refs[index]?.current?.focus();
      refs[index]?.current?.select();
    });
  };
  const updatePart = (index: number, rawValue: string) => {
    const nextPart = rawValue.replace(/\D/g, "").slice(0, limits[index]);
    const nextParts = [...parts];
    nextParts[index] = nextPart;
    onChange(nextParts.every((part) => !part) ? "" : nextParts.join("-"));
    if (nextPart.length === limits[index] && index < refs.length - 1) focusSegment(index + 1);
  };
  const handlePaste = (event: React.ClipboardEvent<HTMLInputElement>) => {
    const digits = event.clipboardData.getData("text").replace(/\D/g, "");
    if (digits.length !== 8) return;
    event.preventDefault();
    onChange(`${digits.slice(0, 4)}-${digits.slice(4, 6)}-${digits.slice(6, 8)}`);
    focusSegment(2);
  };

  return (
    <span className="segmented-date" role="group" aria-label={ariaLabel}>
      {parts.map((part, index) => <span className="date-segment-part" key={labels[index]}>{index > 0 && <i aria-hidden="true">/</i>}<input ref={refs[index]} required={segmentRequired} aria-label={`${ariaLabel}${labels[index]}`} inputMode="numeric" autoComplete="off" maxLength={limits[index]} pattern={patterns[index]} placeholder={labels[index]} value={part} onFocus={(event) => event.currentTarget.select()} onPaste={handlePaste} onKeyDown={(event) => { if (event.key === "Backspace" && !part && index > 0) { event.preventDefault(); focusSegment(index - 1); } }} onChange={(event) => updatePart(index, event.target.value)} /></span>)}
    </span>
  );
}

function App() {
  const [view, setView] = useState<View>("overview");
  const [currency, setCurrency] = useState<CurrencyCode>("CNY");
  const [dashboard, setDashboard] = useState<Dashboard>(EMPTY_DASHBOARD);
  const [holdings, setHoldings] = useState<Holding[]>([]);
  const [closedPositions, setClosedPositions] = useState<ClosedPosition[]>([]);
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
  const [closedPositionDetail, setClosedPositionDetail] = useState<ClosedPosition | null>(null);
  const [holdingDetailLoading, setHoldingDetailLoading] = useState(false);
  const [reconcilingInstitution, setReconcilingInstitution] = useState<InstitutionReconciliation | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      await invoke<number>("sync_deposit_statuses");
      const [nextDashboard, nextHoldings, nextClosedPositions, nextMaturities, nextTransactions, nextMasterData, nextAnalytics, nextReconciliation] = await Promise.all([
        invoke<Dashboard>("get_dashboard"),
        invoke<Holding[]>("list_holdings"),
        invoke<ClosedPosition[]>("list_closed_positions"),
        invoke<MaturityEvent[]>("list_maturities"),
        invoke<TransactionRecord[]>("list_transactions"),
        invoke<MasterData>("list_master_data"),
        invoke<Analytics>("get_analytics"),
        invoke<ReconciliationCenter>("get_reconciliation_center"),
      ]);
      setDashboard(nextDashboard);
      setHoldings(nextHoldings);
      setClosedPositions(nextClosedPositions);
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

  useEffect(() => {
    const blockNumberWheel = (event: WheelEvent) => {
      const activeElement = document.activeElement;
      if (!(activeElement instanceof HTMLInputElement) || activeElement.type !== "number") return;

      const bounds = activeElement.getBoundingClientRect();
      const pointerIsInsideInput =
        event.clientX >= bounds.left &&
        event.clientX <= bounds.right &&
        event.clientY >= bounds.top &&
        event.clientY <= bounds.bottom;

      // A focused native number input applies the wheel delta as a value step.
      // Blurring it before the default action keeps the value stable while still
      // allowing the surrounding page or dialog to scroll normally.
      if (pointerIsInsideInput) activeElement.blur();
    };
    document.addEventListener("wheel", blockNumberWheel, { capture: true });
    return () => document.removeEventListener("wheel", blockNumberWheel, { capture: true });
  }, []);

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
  const depositMaturityOptions = useMemo<MaturityEvent[]>(() => masterData.deposits
    .filter((item) => item.status === "active" || (item.status === "matured" && item.maturedAt === null))
    .map((item) => ({
      id: item.id,
      name: item.name,
      institution: item.institution,
      currency: item.currency,
      amount: item.principal,
      maturityDate: item.maturityDate,
      daysRemaining: 0,
      annualRate: item.annualRate,
    })), [masterData.deposits]);
  const hasData = dashboard.currencies.some((item) => item.totalValue > 0) || transactions.length > 0 || masterData.deposits.length > 0;

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

  const saveInstitutionSnapshot = async (input: InstitutionSnapshotInput) => {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<EntryResult>("save_institution_snapshot", { input });
      setNotice(result.message);
      setReconcilingInstitution(null);
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
          <NavButton active={view === "deposits"} onClick={() => setView("deposits")} icon={<Landmark />} label="定期存款" />
          <NavButton active={view === "history"} onClick={() => setView("history")} icon={<Archive />} label="历史" />
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
            {view === "deposits" && <DepositsView deposits={masterData.deposits} asOfDate={dashboard.asOfDate} onEdit={(record) => setEditingRecord({ entityType: "deposit", record })} />}
            {view === "history" && <HistoryView positions={closedPositions} onSelect={setClosedPositionDetail} />}
            {view === "transactions" && <TransactionsView transactions={transactions} onSelect={(id) => void openTransaction(id)} loading={detailLoading} />}
            {view === "calendar" && <CalendarView groups={groupedMaturities} />}
            {view === "analytics" && <AnalyticsView analytics={analytics} currency={currency} onCurrencyChange={setCurrency} />}
            {view === "masterData" && <MasterDataView data={masterData} onEdit={setEditingRecord} />}
            {view === "reconciliation" && <ReconciliationView data={reconciliation} onReconcile={setReconcilingInstitution} onOpenIssue={openQualityIssue} />}
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
          deposits={depositMaturityOptions}
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
      {closedPositionDetail && (
        <ClosedPositionDetailModal detail={closedPositionDetail} onClose={() => setClosedPositionDetail(null)} />
      )}
      {reconcilingInstitution && (
        <InstitutionReconciliationModal institution={reconcilingInstitution} onClose={() => setReconcilingInstitution(null)} onSave={saveInstitutionSnapshot} />
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
    ...(selectedSummary.demandValue > 0 ? [{ label: "活期", value: selectedSummary.demandValue }] : []),
  ];
  return (
    <>
      <section className="metrics-grid overview-metrics">
        <Metric label="折算人民币总资产" value={formatMoney(dashboard.convertedTotalCny, "CNY")} detail={dashboard.hasEstimatedExchangeRate ? "含活期；未核对银行的美元资产按参考汇率暂估" : "含人民币资产、美元折算资产和活期"} />
        <Metric label="人民币资产" value={formatMoney(cny.totalValue, "CNY")} detail={`理财 ${formatMoney(cny.wealthValue, "CNY")} · 存款 ${formatMoney(cny.depositValue, "CNY")} · 活期 ${formatMoney(cny.demandValue, "CNY")}`} />
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
  const [sort, setSort] = useState<SortState<HoldingsSortKey> | null>(null);
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
  const sorted = useMemo(() => sortRows(filtered, sort, (item, key) => {
    switch (key) {
      case "productName": return item.name;
      case "productCode": return item.code;
      case "channel": return item.channel;
      case "holdingDays": return item.longestHoldingDays;
      case "cost": return item.cost;
      case "marketValue": return item.marketValue;
      case "gain": return item.gain;
      case "thirtyDayReturn": return item.thirtyDayReturn;
      case "valuationDate": return item.valuationDate;
      case "signal": return item.signalLabel;
    }
  }), [filtered, sort]);
  const productSortActive = sort?.key === "productName" || sort?.key === "productCode";
  const changeSort = (key: HoldingsSortKey) => setSort((current) => toggleSort(current, key));
  return (
    <article className="panel table-panel">
      <div className="panel-header holdings-header"><div><h2>当前持仓</h2><span>显示 {filtered.length}/{holdings.length} 个产品 · 点击持仓查看市值与收益历史</span></div><button className="button primary valuation-cta" type="button" onClick={onBatchUpdate}><Activity />批量更新市值</button></div>
      <div className="filter-bar holdings-filter"><label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索产品、代码或渠道" /></label><select value={currencyFilter} onChange={(event) => setCurrencyFilter(event.target.value)}><option value="all">全部币种</option><option value="CNY">人民币</option><option value="USD">美元</option></select><select value={channelFilter} onChange={(event) => setChannelFilter(event.target.value)}><option value="all">全部渠道</option>{channels.map((channel) => <option key={channel} value={channel}>{channel}</option>)}</select><select value={signalFilter} onChange={(event) => setSignalFilter(event.target.value)}><option value="all">全部参考</option><option value="stale">市值待更新</option><option value="REVIEW">评估赎回</option><option value="TAKE_PROFIT">考虑止盈</option><option value="ADD_WATCH">关注加仓</option><option value="HOLD">继续观察</option><option value="OBSERVE">积累数据</option></select></div>
      <div className="table-scroll"><table className="holdings-table"><thead><tr><SortableHeader label="产品" active={productSortActive} direction={sort?.direction ?? "asc"} activeDetail={sort?.key === "productCode" ? "代码" : "名称"} onSort={() => setSort((current) => cycleProductSort(current, "productName", "productCode"))} /><SortableHeader label="渠道" active={sort?.key === "channel"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("channel")} /><SortableHeader label="持有期" active={sort?.key === "holdingDays"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("holdingDays")} /><SortableHeader label="成本" className="number" active={sort?.key === "cost"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("cost")} /><SortableHeader label="市值" className="number" active={sort?.key === "marketValue"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("marketValue")} /><SortableHeader label="持有收益" className="number" active={sort?.key === "gain"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("gain")} /><SortableHeader label="近30日" className="number" active={sort?.key === "thirtyDayReturn"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("thirtyDayReturn")} /><SortableHeader label="最近更新" active={sort?.key === "valuationDate"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("valuationDate")} /><SortableHeader label="操作参考" active={sort?.key === "signal"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("signal")} /></tr></thead><tbody>
        {sorted.map((item) => <tr className="holding-row" aria-busy={loading} key={`${item.id}-${item.currency}`} onClick={() => onSelect(item)}><td><strong>{item.name}</strong><span>{item.code} · {item.currency}</span></td><td>{item.channel}</td><td className="holding-period"><strong>最长 {formatHoldingDays(item.longestHoldingDays)}</strong><span>{item.shortestHoldingDays === null ? "批次日期待补全" : `最短 ${formatHoldingDays(item.shortestHoldingDays)} · ${item.activeLotCount}个批次`}</span></td><td className="number">{formatMoney(item.cost, item.currency)}</td><td className="number"><strong>{formatMoney(item.marketValue, item.currency)}</strong></td><td className={`number ${item.gain >= 0 ? "gain" : "loss"}`}>{formatMoney(item.gain, item.currency)}<span>{formatPercent(item.gainRate)}</span></td><td className={`number ${item.thirtyDayReturn === null ? "" : item.thirtyDayReturn >= 0 ? "gain" : "loss"}`}>{item.thirtyDayReturn === null ? "—" : formatPercent(item.thirtyDayReturn)}</td><td><span className={item.daysSinceValuation > 10 ? "stale-date" : "fresh-date"}>{item.valuationDate}</span><small>{item.daysSinceValuation === 0 ? "今天" : `${item.daysSinceValuation}天前`}</small></td><td><span className={`signal-badge signal-${item.signal.toLowerCase()}`}>{item.signalLabel}</span><small className="signal-summary">{item.signalReason}</small></td></tr>)}
      </tbody></table></div>
      <div className="advice-disclaimer"><Sparkles />操作参考仅根据你记录的估值、收益、产品风险等级和持仓占比生成；历史表现不能预测未来，请同时核对期限、赎回规则、费用和个人风险承受能力。</div>
    </article>
  );
}

const closedPositionLabels: Record<ClosedPosition["closeType"], string> = {
  SELL: "全部卖出",
  REDEEM: "历史赎回",
  PRODUCT_MATURITY: "理财到期",
};

function HistoryView({ positions, onSelect }: { positions: ClosedPosition[]; onSelect: (position: ClosedPosition) => void }) {
  const [searchText, setSearchText] = useState("");
  const [currencyFilter, setCurrencyFilter] = useState("all");
  const [institutionFilter, setInstitutionFilter] = useState("all");
  const [typeFilter, setTypeFilter] = useState("all");
  const [yearFilter, setYearFilter] = useState("all");
  const [sort, setSort] = useState<SortState<HistorySortKey> | null>(null);
  const institutions = useMemo(() => [...new Set(positions.map((item) => item.institution))].sort(), [positions]);
  const years = useMemo(() => [...new Set(positions.map((item) => item.closeDate.slice(0, 4)))].sort().reverse(), [positions]);
  const filtered = useMemo(() => {
    const keyword = searchText.trim().toLowerCase();
    return positions.filter((item) => {
      const matchesKeyword = !keyword || `${item.name} ${item.code} ${item.institution}`.toLowerCase().includes(keyword);
      const matchesCurrency = currencyFilter === "all" || item.currency === currencyFilter;
      const matchesInstitution = institutionFilter === "all" || item.institution === institutionFilter;
      const matchesType = typeFilter === "all" || item.closeType === typeFilter;
      const matchesYear = yearFilter === "all" || item.closeDate.startsWith(yearFilter);
      return matchesKeyword && matchesCurrency && matchesInstitution && matchesType && matchesYear;
    });
  }, [positions, searchText, currencyFilter, institutionFilter, typeFilter, yearFilter]);
  const sorted = useMemo(() => sortRows(filtered, sort, (item, key) => {
    switch (key) {
      case "productName": return item.name;
      case "productCode": return item.code;
      case "closeType": return closedPositionLabels[item.closeType];
      case "holdingDays": return item.holdingDays;
      case "investedCost": return item.investedCost;
      case "proceeds": return item.proceeds;
      case "dividendFees": return item.dividends - item.fees;
      case "realizedGain": return item.realizedGain;
      case "returnRate": return item.returnRate;
    }
  }), [filtered, sort]);
  const productSortActive = sort?.key === "productName" || sort?.key === "productCode";
  const changeSort = (key: HistorySortKey) => setSort((current) => toggleSort(current, key));
  const cnyGain = filtered.filter((item) => item.currency === "CNY").reduce((sum, item) => sum + item.realizedGain, 0);
  const usdGain = filtered.filter((item) => item.currency === "USD").reduce((sum, item) => sum + item.realizedGain, 0);
  const profitable = filtered.filter((item) => item.realizedGain > 0.000001).length;
  return (
    <>
      <section className="metrics-grid history-metrics">
        <Metric label="已结清理财" value={`${filtered.length} 笔`} detail={`Excel 历史与 App 卖出自动汇总`} />
        <Metric label="人民币已实现收益" value={formatMoney(cnyGain, "CNY")} detail="包含卖出、分红与费用" positive={cnyGain >= 0} />
        <Metric label="美元已实现收益" value={formatMoney(usdGain, "USD")} detail="按原币统计" positive={usdGain >= 0} />
        <Metric label="盈利笔数" value={filtered.length ? formatPercent(profitable / filtered.length) : "—"} detail={`${profitable}/${filtered.length} 笔结清收益为正`} />
      </section>
      <article className="panel table-panel history-panel">
        <div className="panel-header"><div><h2>已清仓 / 已到期</h2><span>显示 {filtered.length}/{positions.length} 笔 · 点击查看完整信息</span></div></div>
        <div className="filter-bar history-filter"><label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索产品、代码或银行" /></label><select value={currencyFilter} onChange={(event) => setCurrencyFilter(event.target.value)}><option value="all">全部币种</option><option value="CNY">人民币</option><option value="USD">美元</option></select><select value={institutionFilter} onChange={(event) => setInstitutionFilter(event.target.value)}><option value="all">全部银行</option>{institutions.map((institution) => <option key={institution} value={institution}>{institution}</option>)}</select><select value={typeFilter} onChange={(event) => setTypeFilter(event.target.value)}><option value="all">全部方式</option>{Object.entries(closedPositionLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select><select value={yearFilter} onChange={(event) => setYearFilter(event.target.value)}><option value="all">全部年份</option>{years.map((year) => <option key={year} value={year}>{year} 年</option>)}</select></div>
        <div className="table-scroll"><table className="history-position-table"><thead><tr><SortableHeader label="产品" active={productSortActive} direction={sort?.direction ?? "asc"} activeDetail={sort?.key === "productCode" ? "代码" : "名称"} onSort={() => setSort((current) => cycleProductSort(current, "productName", "productCode"))} /><SortableHeader label="结清方式" active={sort?.key === "closeType"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("closeType")} /><SortableHeader label="持有期间" active={sort?.key === "holdingDays"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("holdingDays")} /><SortableHeader label="投入成本" className="number" active={sort?.key === "investedCost"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("investedCost")} /><SortableHeader label="赎回到账" className="number" active={sort?.key === "proceeds"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("proceeds")} /><SortableHeader label="分红 / 费用" className="number" active={sort?.key === "dividendFees"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("dividendFees")} /><SortableHeader label="已实现收益" className="number" active={sort?.key === "realizedGain"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("realizedGain")} /><SortableHeader label="收益率 / 年化" className="number" active={sort?.key === "returnRate"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("returnRate")} /></tr></thead><tbody>
          {sorted.map((item) => <tr className="history-row" key={item.id} onClick={() => onSelect(item)}><td><strong>{item.name}</strong><span>{item.code} · {item.institution} · {item.currency}</span></td><td><span className="status">{closedPositionLabels[item.closeType]}</span>{item.needsReview && <small className="review-label">待核对</small>}</td><td className="history-period"><strong>{item.purchaseDate ?? "—"} → {item.closeDate}</strong><span>{item.holdingDays === null ? "持有天数待补全" : `持有 ${item.holdingDays} 天`}</span></td><td className="number">{formatMoney(item.investedCost, item.currency)}</td><td className="number">{formatMoney(item.proceeds, item.currency)}</td><td className="number"><strong>{formatMoney(item.dividends, item.currency)}</strong><span className={item.fees > 0 ? "loss" : ""}>{item.fees > 0 ? `- ${formatMoney(item.fees, item.currency)}` : "—"}</span></td><td className={`number ${item.realizedGain >= 0 ? "gain" : "loss"}`}><strong>{formatMoney(item.realizedGain, item.currency)}</strong></td><td className={`number ${item.returnRate >= 0 ? "gain" : "loss"}`}><strong>{formatPercent(item.returnRate)}</strong><span>{item.annualizedReturn === null ? "年化—" : `年化 ${formatPercent(item.annualizedReturn)}`}</span></td></tr>)}
          {!filtered.length && <tr><td colSpan={8}><div className="table-empty">没有符合条件的历史理财</div></td></tr>}
        </tbody></table></div>
      </article>
    </>
  );
}

function ClosedPositionDetailModal({ detail, onClose }: { detail: ClosedPosition; onClose: () => void }) {
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal detail-modal closed-position-modal" role="dialog" aria-modal="true" aria-labelledby="closed-position-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="closed-position-title">{detail.name}</h2><p>{detail.code} · {detail.institution} · {detail.currency}</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="detail-status"><span className="operation-tag incoming">{closedPositionLabels[detail.closeType]}</span><span>{detail.source === "excel" ? "Excel 导入" : "App 记录"}</span>{detail.needsReview && <span className="loss">金额待核对</span>}</div>
        <dl className="detail-grid">
          <div><dt>首次买入日</dt><dd>{detail.purchaseDate ?? "—"}</dd></div><div><dt>结清日</dt><dd>{detail.closeDate}</dd></div>
          <div><dt>持有时间</dt><dd>{detail.holdingDays === null ? "—" : `${detail.holdingDays} 天`}</dd></div><div><dt>批次 / 卖出次数</dt><dd>{detail.buyCount} 个买入批次 / {detail.exitCount} 次</dd></div>
          <div><dt>核销投入成本</dt><dd>{formatMoney(detail.investedCost, detail.currency)}</dd></div><div><dt>赎回到账</dt><dd>{formatMoney(detail.proceeds, detail.currency)}</dd></div>
          <div><dt>累计分红</dt><dd>{formatMoney(detail.dividends, detail.currency)}</dd></div><div><dt>累计费用</dt><dd>{formatMoney(detail.fees, detail.currency)}</dd></div>
          <div><dt>已实现收益</dt><dd className={detail.realizedGain >= 0 ? "gain" : "loss"}>{formatMoney(detail.realizedGain, detail.currency)}</dd></div><div><dt>总收益率</dt><dd className={detail.returnRate >= 0 ? "gain" : "loss"}>{formatPercent(detail.returnRate)}</dd></div>
          <div><dt>年化收益率</dt><dd>{detail.annualizedReturn === null ? "—" : formatPercent(detail.annualizedReturn)}</dd></div><div><dt>数据来源</dt><dd>{detail.source === "excel" ? "理财.xlsx 历史赎回" : "App 卖出 / 到期记录"}</dd></div>
        </dl>
        <div className="calculation-note">总收益率 = 已实现收益 ÷ 核销成本；年化收益率根据首次买入日和结清日折算。若期间有多次买入或部分卖出，该年化值为简化口径。</div>
        <div className="entry-actions"><button className="button primary" onClick={onClose}>关闭</button></div>
      </section>
    </div>
  );
}

function BatchValuationModal({ holdings, onClose, onSave }: { holdings: Holding[]; onClose: () => void; onSave: (input: BatchValuationInput) => Promise<void> }) {
  const today = new Date().toISOString().slice(0, 10);
  const [valuationDate, setValuationDate] = useState(today);
  const [note, setNote] = useState("");
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(holdings.map((item) => [`${item.productId}:${item.accountId}`, item.marketValue.toString()])));
  const [submitting, setSubmitting] = useState(false);
  const [sort, setSort] = useState<SortState<BatchValuationSortKey>>({ key: "productCode", direction: "asc" });
  const sortedHoldings = useMemo(() => sortRows(holdings, sort, (item, key) => {
    const valueKey = `${item.productId}:${item.accountId}`;
    const nextValue = Number(values[valueKey]);
    switch (key) {
      case "productName": return item.name;
      case "productCode": return item.code;
      case "marketValue": return item.marketValue;
      case "nextMarketValue": return Number.isFinite(nextValue) ? nextValue : null;
      case "difference": return Number.isFinite(nextValue) ? nextValue - item.marketValue : null;
    }
  }), [holdings, sort, values]);
  const changed = sortedHoldings.filter((item) => {
    const next = Number(values[`${item.productId}:${item.accountId}`]);
    return Number.isFinite(next) && Math.abs(next - item.marketValue) > 0.000001;
  }).length;
  const adjustValue = (key: string, delta: number) => {
    setValues((current) => {
      const parsed = Number(current[key]);
      const base = Number.isFinite(parsed) ? parsed : 0;
      const next = Math.max(0, Math.round((base + delta) * 100) / 100);
      return { ...current, [key]: next.toFixed(2) };
    });
  };
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
          <div className="batch-meta"><label>估值日期<DateInput required ariaLabel="估值日期" value={valuationDate} onChange={setValuationDate} /></label><label>备注（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="例如：周末统一更新" /></label></div>
          <div className="batch-list"><div className="batch-list-head"><SortControl label="产品、代码与渠道" active={sort.key === "productName" || sort.key === "productCode"} direction={sort.direction} activeDetail={sort.key === "productCode" ? "代码" : "名称"} onSort={() => setSort((current) => cycleProductSort(current, "productName", "productCode"))} /><SortControl label="原市值" active={sort.key === "marketValue"} direction={sort.direction} onSort={() => setSort((current) => toggleSort(current, "marketValue"))} /><SortControl label="本次市值" active={sort.key === "nextMarketValue"} direction={sort.direction} onSort={() => setSort((current) => toggleSort(current, "nextMarketValue"))} /><SortControl label="变化" active={sort.key === "difference"} direction={sort.direction} onSort={() => setSort((current) => toggleSort(current, "difference"))} /></div>{sortedHoldings.map((item) => {
            const key = `${item.productId}:${item.accountId}`;
            const next = Number(values[key]);
            const difference = Number.isFinite(next) ? next - item.marketValue : 0;
            return <div className="batch-row" key={key}><div><strong>{item.name}</strong><span>{item.code || "无代码"} · {item.channel} · {item.currency}</span></div><span>{formatMoney(item.marketValue, item.currency)}</span><div className="batch-number-control"><input required aria-label={`${item.name}本次市值`} type="number" min="0" step="0.01" value={values[key]} onKeyDown={(event) => { if (event.key === "ArrowUp" || event.key === "ArrowDown") { event.preventDefault(); adjustValue(key, event.key === "ArrowUp" ? 1 : -1); } }} onChange={(event) => setValues((current) => ({ ...current, [key]: event.target.value }))} /><div className="batch-stepper"><button type="button" tabIndex={-1} aria-label={`${item.name}市值增加1`} onMouseDown={(event) => event.preventDefault()} onClick={() => adjustValue(key, 1)}><ArrowUp /></button><button type="button" tabIndex={-1} aria-label={`${item.name}市值减少1`} onMouseDown={(event) => event.preventDefault()} onClick={() => adjustValue(key, -1)}><ArrowDown /></button></div></div><b className={difference >= 0 ? "gain" : "loss"}>{difference === 0 ? "—" : `${difference > 0 ? "+" : ""}${formatMoney(difference, item.currency)}`}</b></div>;
          })}</div>
          <div className="batch-footer"><div><strong>{sortedHoldings.length}</strong> 个持仓将记录本次估值，<strong>{changed}</strong> 个市值发生变化</div><div className="entry-actions"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><Save />{submitting ? "保存中…" : "保存全部市值"}</button></div></div>
        </form>
      </section>
    </div>
  );
}

function niceChartStep(range: number, targetIntervals: number) {
  const roughStep = range / Math.max(targetIntervals, 1);
  const magnitude = 10 ** Math.floor(Math.log10(Math.max(roughStep, Number.EPSILON)));
  const normalized = roughStep / magnitude;
  const factors = [1, 1.25, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10];
  const factor = factors.reduce((closest, candidate) => Math.abs(Math.log(candidate / normalized)) < Math.abs(Math.log(closest / normalized)) ? candidate : closest);
  return factor * magnitude;
}

function ValuationHistoryChart({ points, currency }: { points: ValuationHistoryPoint[]; currency: CurrencyCode }) {
  const visible = points.slice(-24);
  if (visible.length < 2) return <div className="history-empty">至少完成两次市值记录后显示变化曲线</div>;
  const width = 760;
  const height = 230;
  const padLeft = 62;
  const padRight = 28;
  const padTop = 18;
  const padBottom = 34;
  const values = visible.map((item) => item.marketValue);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const valueRange = max - min;
  const valueMagnitude = Math.max(Math.abs(min), Math.abs(max), 1);
  const padding = valueRange > 0
    ? Math.max(valueRange * 0.08, valueMagnitude * 0.00001, 0.01)
    : Math.max(valueMagnitude * 0.001, 1);
  const yStep = niceChartStep(valueRange + padding * 2, 7);
  const low = Math.floor((min - padding) / yStep) * yStep;
  const high = Math.ceil((max + padding) / yStep) * yStep;
  const yTicks = Array.from({ length: Math.round((high - low) / yStep) + 1 }, (_, index) => low + index * yStep);
  const timestamps = visible.map((item) => Date.parse(`${item.date}T00:00:00Z`));
  const firstTimestamp = timestamps[0];
  const lastTimestamp = timestamps[timestamps.length - 1];
  const hasDateRange = timestamps.every(Number.isFinite) && lastTimestamp > firstTimestamp;
  const x = (timestamp: number, index: number) => hasDateRange
    ? padLeft + (timestamp - firstTimestamp) / (lastTimestamp - firstTimestamp) * (width - padLeft - padRight)
    : padLeft + index * ((width - padLeft - padRight) / Math.max(visible.length - 1, 1));
  const y = (value: number) => padTop + (high - value) / Math.max(high - low, 1) * (height - padTop - padBottom);
  const path = visible.map((item, index) => `${x(timestamps[index], index)},${y(item.marketValue)}`).join(" ");
  const spanDays = hasDateRange ? Math.round((lastTimestamp - firstTimestamp) / 86_400_000) : 0;
  const tickCount = hasDateRange ? Math.min(5, spanDays + 1) : 0;
  const xTicks = Array.from({ length: tickCount }, (_, index) => {
    const ratio = tickCount === 1 ? 0 : index / (tickCount - 1);
    const timestamp = firstTimestamp + ratio * (lastTimestamp - firstTimestamp);
    return { x: padLeft + ratio * (width - padLeft - padRight), label: new Date(timestamp).toISOString().slice(5, 10) };
  });
  return (
    <div className="valuation-chart">
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label="持仓市值历史">
        <g className="chart-grid" aria-hidden="true">
          {yTicks.map((tick) => <line key={`y-${tick}`} x1={padLeft} x2={width - padRight} y1={y(tick)} y2={y(tick)} />)}
          {xTicks.map((tick) => <line key={`x-${tick.x}`} x1={tick.x} x2={tick.x} y1={padTop} y2={height - padBottom} />)}
        </g>
        {yTicks.map((tick) => <text className="chart-y-label" key={`label-${tick}`} x={padLeft - 7} y={y(tick)} textAnchor="end" dominantBaseline="middle">{formatMoney(tick, currency)}</text>)}
        <polyline points={path} fill="none" stroke="#2e7d55" strokeWidth="3" strokeLinejoin="round" strokeLinecap="round" />
        {visible.map((item, index) => <circle key={item.id} cx={x(timestamps[index], index)} cy={y(item.marketValue)} r="3.5" fill="#2e7d55"><title>{item.date} · {formatMoney(item.marketValue, currency)}</title></circle>)}
        {hasDateRange
          ? xTicks.map((tick) => <text key={`${tick.x}:${tick.label}`} x={tick.x} y={height - 9} textAnchor="middle">{tick.label}</text>)
          : visible.map((item, index) => (index === 0 || index === visible.length - 1) && <text key={item.id} x={x(timestamps[index], index)} y={height - 9} textAnchor="middle">{item.date.slice(5)}</text>)}
      </svg>
    </div>
  );
}

function HoldingDetailModal({ detail, onClose }: { detail: HoldingDetail; onClose: () => void }) {
  const [sort, setSort] = useState<SortState<ValuationHistorySortKey> | null>(null);
  const recentHistory = useMemo(() => detail.history.slice().reverse().slice(0, 12), [detail.history]);
  const sortedHistory = useMemo(() => sortRows(recentHistory, sort, (item, key) => {
    switch (key) {
      case "date": return item.date;
      case "marketValue": return item.marketValue;
      case "changeRate": return item.changeRate;
      case "source": return item.source;
    }
  }), [recentHistory, sort]);
  const header = (label: string, key: ValuationHistorySortKey) => <SortControl label={label} active={sort?.key === key} direction={sort?.direction ?? "asc"} onSort={() => setSort((current) => toggleSort(current, key))} />;
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal holding-detail-modal" role="dialog" aria-modal="true" aria-labelledby="holding-detail-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="holding-detail-title">{detail.name}</h2><p>{detail.code} · {detail.channel} · {detail.currency}{detail.riskLevel ? ` · ${detail.riskLevel}` : ""}</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <section className="holding-detail-metrics"><div><span>当前市值</span><strong>{formatMoney(detail.marketValue, detail.currency)}</strong></div><div><span>持有收益</span><strong className={detail.gain >= 0 ? "gain" : "loss"}>{formatMoney(detail.gain, detail.currency)} · {formatPercent(detail.gainRate)}</strong></div><div><span>近7日估值收益</span><strong className={detail.sevenDayReturn === null ? "" : detail.sevenDayReturn >= 0 ? "gain" : "loss"}>{detail.sevenDayReturn === null ? "—" : formatPercent(detail.sevenDayReturn)}</strong></div><div><span>近30日估值收益</span><strong className={detail.thirtyDayReturn === null ? "" : detail.thirtyDayReturn >= 0 ? "gain" : "loss"}>{detail.thirtyDayReturn === null ? "—" : formatPercent(detail.thirtyDayReturn)}</strong></div></section>
        <div className={`holding-signal signal-card-${detail.signal.toLowerCase()}`}><div><Sparkles /><strong>{detail.signalLabel}</strong></div><p>{detail.signalReason}</p></div>
        <div className="history-section"><div className="panel-header"><h2>市值变化</h2><span>最近更新 {detail.valuationDate} · {detail.daysSinceValuation === 0 ? "今天" : `${detail.daysSinceValuation}天前`}</span></div><ValuationHistoryChart points={detail.history} currency={detail.currency} /></div>
        <div className="history-table"><div className="batch-list-head">{header("日期", "date")}{header("市值", "marketValue")}{header("估值收益", "changeRate")}{header("来源", "source")}</div>{sortedHistory.map((item) => <div key={item.id}><span>{item.date}</span><strong>{formatMoney(item.marketValue, detail.currency)}</strong><span className={item.changeRate === null ? "" : item.changeRate >= 0 ? "gain" : "loss"}>{item.changeRate === null ? "—" : `${formatMoney(item.changeAmount ?? 0, detail.currency)} · ${formatPercent(item.changeRate)}`}</span><span>{item.source}</span></div>)}</div>
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

function depositStatusLabel(status: string) {
  return status === "active" ? "持有中" : status === "matured" ? "已取出" : "已取消";
}

function daysBetween(date: string, asOfDate: string) {
  const target = new Date(`${date}T00:00:00`);
  const today = new Date(`${asOfDate}T00:00:00`);
  return Math.round((target.getTime() - today.getTime()) / 86_400_000);
}

function DepositsView({ deposits, asOfDate, onEdit }: { deposits: DepositRecord[]; asOfDate: string; onEdit: (record: DepositRecord) => void }) {
  const [searchText, setSearchText] = useState("");
  const [status, setStatus] = useState("all");
  const [currency, setCurrency] = useState("all");
  const [sort, setSort] = useState<SortState<DepositSortKey>>({ key: "maturityDate", direction: "asc" });
  const active = deposits.filter((item) => item.status === "active");
  const cnyPrincipal = active.filter((item) => item.currency === "CNY").reduce((sum, item) => sum + item.principal, 0);
  const usdPrincipal = active.filter((item) => item.currency === "USD").reduce((sum, item) => sum + item.principal, 0);
  const withdrawnCount = deposits.filter((item) => item.status === "matured").length;
  const dueSoon = active.filter((item) => {
    const days = daysBetween(item.maturityDate, asOfDate);
    return days >= 0 && days <= 30;
  }).length;
  const filtered = deposits.filter((item) => {
    const keyword = searchText.trim().toLowerCase();
    return (status === "all" || item.status === status)
      && (currency === "all" || item.currency === currency)
      && (!keyword || `${item.name} ${item.institution}`.toLowerCase().includes(keyword));
  });
  const sorted = useMemo(() => sortRows(filtered, sort, (item, key) => {
    switch (key) {
      case "name": return item.name;
      case "institution": return item.institution;
      case "currency": return item.currency;
      case "principal": return item.principal;
      case "maturityDate": return item.maturityDate;
      case "annualRate": return item.annualRate;
      case "status": return depositStatusLabel(item.status);
    }
  }), [filtered, sort]);
  const header = (label: string, key: DepositSortKey, className?: string) => <SortableHeader label={label} className={className} active={sort.key === key} direction={sort.direction} onSort={() => setSort((current) => toggleSort(current, key))} />;
  return (
    <>
      <section className="metrics-grid deposit-metrics">
        <Metric label="持有中" value={`${active.length} 笔`} detail={`${dueSoon} 笔将在 30 天内到期`} />
        <Metric label="人民币本金" value={formatMoney(cnyPrincipal, "CNY")} detail="仅统计持有中存款" />
        <Metric label="美元本金" value={formatMoney(usdPrincipal, "USD")} detail="仅统计持有中存款" />
        <Metric label="已取出" value={`${withdrawnCount} 笔`} detail="到期日已到后自动转入" />
      </section>
      <article className="panel table-panel deposit-panel">
        <div className="panel-header"><div><h2>存款项目</h2><span>共 {deposits.length} 笔 · 到期日已到的项目自动标记为已取出</span></div></div>
        <div className="filter-bar deposit-filter">
          <label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索存款名称或银行" /></label>
          <select aria-label="存款状态" value={status} onChange={(event) => setStatus(event.target.value)}><option value="all">全部状态</option><option value="active">持有中</option><option value="matured">已取出</option><option value="cancelled">已取消</option></select>
          <select aria-label="存款币种" value={currency} onChange={(event) => setCurrency(event.target.value)}><option value="all">全部币种</option><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select>
          <span>显示 {sorted.length}/{deposits.length} 笔</span>
        </div>
        <div className="table-scroll"><table className="deposit-overview-table"><thead><tr>{header("存款项目", "name")}{header("存款银行", "institution")}{header("币种", "currency")}{header("本金", "principal", "number")}<th>起息日</th>{header("到期日", "maturityDate")}{header("年利率", "annualRate")}{header("状态", "status")}<th /></tr></thead><tbody>
          {sorted.map((item) => {
            const days = daysBetween(item.maturityDate, asOfDate);
            const timing = item.status === "active" ? (days === 0 ? "今天到期" : `${days} 天后`) : item.maturedAt ? `取出于 ${item.maturedAt}` : "到期自动取出";
            return <tr key={item.id}><td><strong>{item.name}</strong><span>{item.source === "excel" ? "Excel 导入" : "手工录入"}</span></td><td>{item.institution}</td><td>{item.currency}</td><td className="number">{formatMoney(item.principal, item.currency)}{item.proceeds !== null && <span>到账 {formatMoney(item.proceeds, item.currency)}</span>}</td><td className="nowrap">{item.startDate ?? "—"}</td><td className="deposit-maturity-cell"><strong>{item.maturityDate}</strong><span>{timing}</span></td><td>{formatPercent(item.annualRate)}</td><td><span className={`status deposit-status-${item.status}`}>{depositStatusLabel(item.status)}</span></td><td><button className="icon-button" onClick={() => onEdit(item)} aria-label={`编辑${item.name}`}><Pencil /></button></td></tr>;
          })}
          {!sorted.length && <tr><td className="table-empty" colSpan={9}>没有符合条件的存款项目</td></tr>}
        </tbody></table></div>
      </article>
    </>
  );
}

function MasterDataView({ data, onEdit }: { data: MasterData; onEdit: (target: EditableRecord) => void }) {
  const [section, setSection] = useState<"products" | "accounts" | "deposits">("products");
  const [productSort, setProductSort] = useState<SortState<ProductSortKey>>({ key: "code", direction: "asc" });
  const [accountSort, setAccountSort] = useState<SortState<AccountSortKey> | null>(null);
  const [depositSort, setDepositSort] = useState<SortState<DepositSortKey> | null>(null);
  const sortedProducts = useMemo(() => sortRows(data.products, productSort, (item, key) => {
    switch (key) {
      case "name": return item.name;
      case "code": return item.code;
      case "currency": return item.currency;
      case "issuer": return item.issuer;
      case "purchaseBanks": return item.purchaseBanks.join("、");
      case "riskLevel": return item.riskLevel;
      case "source": return item.source === "excel" ? "Excel" : "手工";
    }
  }), [data.products, productSort]);
  const sortedAccounts = useMemo(() => sortRows(data.accounts, accountSort, (item, key) => {
    switch (key) {
      case "account": return `${item.institution} ${item.name}`;
      case "currency": return item.currency;
      case "source": return item.source === "excel" ? "Excel" : "手工";
    }
  }), [data.accounts, accountSort]);
  const sortedDeposits = useMemo(() => sortRows(data.deposits, depositSort, (item, key) => {
    switch (key) {
      case "name": return item.name;
      case "institution": return item.institution;
      case "currency": return item.currency;
      case "principal": return item.principal;
      case "maturityDate": return item.maturityDate;
      case "annualRate": return item.annualRate;
      case "status": return depositStatusLabel(item.status);
    }
  }), [data.deposits, depositSort]);
  const productHeader = (label: string, key: ProductSortKey) => <SortableHeader label={label} active={productSort.key === key} direction={productSort.direction} onSort={() => setProductSort((current) => toggleSort(current, key))} />;
  const accountHeader = (label: string, key: AccountSortKey) => <SortableHeader label={label} active={accountSort?.key === key} direction={accountSort?.direction ?? "asc"} onSort={() => setAccountSort((current) => toggleSort(current, key))} />;
  const depositHeader = (label: string, key: DepositSortKey, className?: string) => <SortableHeader label={label} className={className} active={depositSort?.key === key} direction={depositSort?.direction ?? "asc"} onSort={() => setDepositSort((current) => toggleSort(current, key))} />;
  return (
    <article className="panel table-panel master-panel">
      <div className="panel-header"><div><h2>资料管理</h2><span>修改会自动备份并保留变更审计；手工修正不会被下次导入覆盖</span></div><div className="segmented"><button className={section === "products" ? "active" : ""} onClick={() => setSection("products")}>产品 {data.products.length}</button><button className={section === "accounts" ? "active" : ""} onClick={() => setSection("accounts")}>账户 {data.accounts.length}</button><button className={section === "deposits" ? "active" : ""} onClick={() => setSection("deposits")}>存款 {data.deposits.length}</button></div></div>
      {section === "products" && <div className="table-scroll"><table className="product-table"><thead><tr>{productHeader("名称", "name")}{productHeader("代码", "code")}{productHeader("币种", "currency")}{productHeader("发行机构", "issuer")}{productHeader("购买银行", "purchaseBanks")}{productHeader("风险等级", "riskLevel")}{productHeader("来源", "source")}<th /></tr></thead><tbody>{sortedProducts.map((item) => <tr key={item.id}><td><strong>{item.name}</strong></td><td className="product-code">{item.code}</td><td>{item.currency}</td><td>{item.issuer ?? "—"}</td><td className="purchase-banks">{item.purchaseBanks.length ? item.purchaseBanks.join("、") : "—"}</td><td>{item.riskLevel ?? "—"}</td><td>{item.source === "excel" ? "Excel" : "手工"}</td><td><button className="icon-button" onClick={() => onEdit({ entityType: "product", record: item })} aria-label={`编辑${item.name}`}><Pencil /></button></td></tr>)}</tbody></table></div>}
      {section === "accounts" && <div className="table-scroll"><table><thead><tr>{accountHeader("账户", "account")}{accountHeader("币种", "currency")}{accountHeader("来源", "source")}<th /></tr></thead><tbody>{sortedAccounts.map((item) => <tr key={item.id}><td><strong>{item.institution}</strong><span>{item.name}</span></td><td>{item.currency}</td><td>{item.source === "excel" ? "Excel" : "手工"}</td><td><button className="icon-button" onClick={() => onEdit({ entityType: "account", record: item })} aria-label={`编辑${item.name}`}><Pencil /></button></td></tr>)}</tbody></table></div>}
      {section === "deposits" && <div className="table-scroll"><table className="deposit-table"><thead><tr>{depositHeader("存款", "name")}{depositHeader("账户", "institution")}{depositHeader("币种", "currency")}{depositHeader("本金", "principal", "number")}{depositHeader("到期日", "maturityDate")}{depositHeader("年利率", "annualRate")}{depositHeader("状态", "status")}<th /></tr></thead><tbody>{sortedDeposits.map((item) => <tr key={item.id}><td><strong>{item.name}</strong><span>{item.source === "excel" ? "Excel 导入" : "手工录入"}</span></td><td>{item.institution}</td><td>{item.currency}</td><td className="number">{formatMoney(item.principal, item.currency)}</td><td>{item.maturityDate}</td><td>{formatPercent(item.annualRate)}</td><td><span className="status">{depositStatusLabel(item.status)}</span></td><td><button className="icon-button" onClick={() => onEdit({ entityType: "deposit", record: item })} aria-label={`编辑${item.name}`}><Pencil /></button></td></tr>)}</tbody></table></div>}
    </article>
  );
}

function ReconciliationView({ data, onReconcile, onOpenIssue }: { data: ReconciliationCenter; onReconcile: (institution: InstitutionReconciliation) => void; onOpenIssue: (issue: QualityIssue) => void }) {
  const [section, setSection] = useState<"banks" | "quality">("banks");
  const [severity, setSeverity] = useState("all");
  const [searchText, setSearchText] = useState("");
  const [sort, setSort] = useState<SortState<ReconciliationSortKey> | null>(null);
  const matched = data.institutions.filter((item) => item.status === "matched").length;
  const differences = data.institutions.filter((item) => item.status === "difference").length;
  const missing = data.institutions.filter((item) => item.status === "missing" && item.trackedTotalCny > 0.000001).length;
  const filteredIssues = data.issues.filter((issue) => {
    const keyword = searchText.trim().toLowerCase();
    return (severity === "all" || issue.severity === severity) && (!keyword || `${issue.category} ${issue.title} ${issue.detail}`.toLowerCase().includes(keyword));
  });
  const sortedInstitutions = useMemo(() => sortRows(data.institutions, sort, (item, key) => {
    switch (key) {
      case "institution": return item.institution;
      case "usdCnyRate": return item.hasUsdAssets ? item.usdCnyRate : null;
      case "trackedWealthCny": return item.trackedWealthCny;
      case "actualWealthCny": return item.actualWealthCny;
      case "trackedDepositCny": return item.trackedDepositCny;
      case "actualDepositCny": return item.actualDepositCny;
      case "demandCny": return item.balanceDate ? item.demandCny : null;
      case "actualTotalCny": return item.actualTotalCny;
      case "status": return item.status === "matched" ? "已一致" : item.status === "difference" ? "有差额" : "未核对";
    }
  }), [data.institutions, sort]);
  const header = (label: string, key: ReconciliationSortKey, className?: string) => <SortableHeader label={label} className={className} active={sort?.key === key} direction={sort?.direction ?? "asc"} onSort={() => setSort((current) => toggleSort(current, key))} />;
  return (
    <>
      <section className="metrics-grid reconciliation-metrics"><Metric label="已核对银行" value={`${matched}`} detail={`共 ${data.institutions.length} 家银行`} positive={matched > 0} /><Metric label="存在差额" value={`${differences}`} detail="理财或存款分项与软件不一致" positive={differences === 0} /><Metric label="尚未核对" value={`${missing}`} detail="有资产但没有银行分项快照" positive={missing === 0} /><Metric label="数据质量问题" value={`${data.issueCount}`} detail={`${data.highPriorityCount} 项高优先级`} positive={data.highPriorityCount === 0} /></section>
      <article className="panel reconciliation-panel">
        <div className="panel-header"><div><h2>对账与数据质量</h2><span>同一家银行的人民币与美元资产合并；银行 App 三个分项均按人民币录入</span></div><div className="segmented"><button className={section === "banks" ? "active" : ""} onClick={() => setSection("banks")}>银行对账</button><button className={section === "quality" ? "active" : ""} onClick={() => setSection("quality")}>问题清单 {data.issueCount}</button></div></div>
        {section === "banks" && <div className="table-scroll"><table className="reconciliation-table"><thead><tr>{header("银行", "institution")}{header("USD/CNY", "usdCnyRate", "number")}{header("软件理财", "trackedWealthCny", "number")}{header("银行理财 / 差额", "actualWealthCny", "number")}{header("软件存款", "trackedDepositCny", "number")}{header("银行存款 / 差额", "actualDepositCny", "number")}{header("银行活期", "demandCny", "number")}{header("银行总资产", "actualTotalCny", "number")}{header("状态", "status")}<th /></tr></thead><tbody>{sortedInstitutions.map((item) => <tr key={item.institution}><td><strong>{item.institution}</strong><span>{item.balanceDate ? `核对于 ${item.balanceDate}` : "尚无对账快照"}</span></td><td className="number">{item.hasUsdAssets ? item.usdCnyRate.toFixed(4) : "—"}</td><td className="number"><strong>{formatMoney(item.trackedWealthCny, "CNY")}</strong><span>{item.usdWealthValue > 0 ? `含 ${formatMoney(item.usdWealthValue, "USD")}` : "仅人民币"}</span></td><td className="number">{item.actualWealthCny === null ? "—" : formatMoney(item.actualWealthCny, "CNY")}<span className={item.wealthDifference === null ? "" : Math.abs(item.wealthDifference) <= 1 ? "gain" : "loss"}>{item.wealthDifference === null ? "" : `差 ${formatMoney(item.wealthDifference, "CNY")}`}</span></td><td className="number"><strong>{formatMoney(item.trackedDepositCny, "CNY")}</strong><span>{item.usdDepositValue > 0 ? `含 ${formatMoney(item.usdDepositValue, "USD")}` : "仅人民币"}</span></td><td className="number">{item.actualDepositCny === null ? "—" : formatMoney(item.actualDepositCny, "CNY")}<span className={item.depositDifference === null ? "" : Math.abs(item.depositDifference) <= 1 ? "gain" : "loss"}>{item.depositDifference === null ? "" : `差 ${formatMoney(item.depositDifference, "CNY")}`}</span></td><td className="number">{item.balanceDate ? formatMoney(item.demandCny, "CNY") : "—"}</td><td className="number"><strong>{item.actualTotalCny === null ? "—" : formatMoney(item.actualTotalCny, "CNY")}</strong><span>{item.difference === null ? "" : `总差 ${formatMoney(item.difference, "CNY")}`}</span></td><td><span className={`reconciliation-status status-${item.status}`}>{item.status === "matched" ? "已一致" : item.status === "difference" ? "有差额" : "未核对"}</span></td><td><button className="button secondary compact" onClick={() => onReconcile(item)}>录入分项</button></td></tr>)}</tbody></table></div>}
        {section === "quality" && <><div className="quality-toolbar"><label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索问题" /></label><select value={severity} onChange={(event) => setSeverity(event.target.value)}><option value="all">全部优先级</option><option value="high">高优先级</option><option value="medium">中优先级</option><option value="low">低优先级</option></select><span>显示 {filteredIssues.length}/{data.issues.length} 项</span></div><div className="quality-list">{filteredIssues.map((issue) => <article key={issue.key} className={`quality-item severity-${issue.severity}`}><div className="quality-severity">{issue.severity === "high" ? "高" : issue.severity === "medium" ? "中" : "低"}</div><div><span>{issue.category}</span><strong>{issue.title}</strong><p>{issue.detail}</p></div><button className="button secondary compact" onClick={() => onOpenIssue(issue)}>前往处理</button></article>)}{!filteredIssues.length && <div className="quality-empty"><CheckCircle2 />当前筛选下没有待处理问题</div>}</div></>}
      </article>
    </>
  );
}

function InstitutionReconciliationModal({ institution, onClose, onSave }: { institution: InstitutionReconciliation; onClose: () => void; onSave: (input: InstitutionSnapshotInput) => Promise<void> }) {
  const [balanceDate, setBalanceDate] = useState(institution.balanceDate ?? new Date().toISOString().slice(0, 10));
  const [usdCnyRate, setUsdCnyRate] = useState(institution.usdCnyRate.toString());
  const [actualWealthCny, setActualWealthCny] = useState(institution.actualWealthCny?.toString() ?? "");
  const [actualDepositCny, setActualDepositCny] = useState(institution.actualDepositCny?.toString() ?? "");
  const [demandCny, setDemandCny] = useState(institution.balanceDate ? institution.demandCny.toString() : "");
  const [note, setNote] = useState(institution.note ?? "");
  const [submitting, setSubmitting] = useState(false);
  const rate = Number(usdCnyRate);
  const wealth = Number(actualWealthCny);
  const deposit = Number(actualDepositCny);
  const demand = Number(demandCny);
  const validInputs = [rate, wealth, deposit, demand].every((value) => Number.isFinite(value) && value >= 0) && rate > 0 && actualWealthCny !== "" && actualDepositCny !== "" && demandCny !== "";
  const conversionRate = Number.isFinite(rate) && rate > 0 ? rate : institution.usdCnyRate;
  const trackedWealth = institution.cnyWealthValue + institution.usdWealthValue * conversionRate;
  const trackedDeposit = institution.cnyDepositValue + institution.usdDepositValue * conversionRate;
  const wealthDifference = validInputs ? wealth - trackedWealth : null;
  const depositDifference = validInputs ? deposit - trackedDeposit : null;
  const actualTotal = validInputs ? wealth + deposit + demand : null;
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!validInputs) return;
    setSubmitting(true);
    try {
      await onSave({ institution: institution.institution, balanceDate, usdCnyRate: rate, actualWealthCny: wealth, actualDepositCny: deposit, demandCny: demand, note: note.trim() || null });
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal reconciliation-modal" role="dialog" aria-modal="true" aria-labelledby="reconciliation-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header"><div><h2 id="reconciliation-title">核对 {institution.institution}</h2><p>银行 App 的理财、存款、活期均按人民币录入</p></div><button onClick={onClose} aria-label="关闭"><X /></button></div>
        <div className="native-assets"><span>软件原币资产</span><p>理财：{formatMoney(institution.cnyWealthValue, "CNY")}{institution.usdWealthValue > 0 ? ` + ${formatMoney(institution.usdWealthValue, "USD")}` : ""}</p><p>存款：{formatMoney(institution.cnyDepositValue, "CNY")}{institution.usdDepositValue > 0 ? ` + ${formatMoney(institution.usdDepositValue, "USD")}` : ""}</p></div>
        <form className="entry-form reconciliation-form" onSubmit={(event) => void submit(event)}>
          <label>对账日期<DateInput required ariaLabel="对账日期" value={balanceDate} onChange={setBalanceDate} /></label>
          <label>美元兑人民币汇率<input required type="number" min="0.0001" step="0.0001" value={usdCnyRate} onChange={(event) => setUsdCnyRate(event.target.value)} disabled={!institution.hasUsdAssets} /><small>{institution.hasUsdAssets ? "用于折算该银行的美元理财和美元存款" : "该银行目前没有美元资产，无需折算"}</small></label>
          <label>银行 App · 理财（人民币）<input required autoFocus type="number" min="0" step="0.01" value={actualWealthCny} onChange={(event) => setActualWealthCny(event.target.value)} placeholder="银行显示的理财分项" /></label>
          <label>银行 App · 存款（人民币）<input required type="number" min="0" step="0.01" value={actualDepositCny} onChange={(event) => setActualDepositCny(event.target.value)} placeholder="银行显示的存款分项" /></label>
          <label className="full-field">银行 App · 活期（人民币）<input required type="number" min="0" step="0.01" value={demandCny} onChange={(event) => setDemandCny(event.target.value)} placeholder="含已折算的人民币与外币活期" /><small>活期会作为独立资产计入资产总览，不会被当作对账差额</small></label>
          <div className="reconciliation-breakdown full-field"><div><span>软件理财（折算）</span><strong>{formatMoney(trackedWealth, "CNY")}</strong><small className={wealthDifference === null ? "" : Math.abs(wealthDifference) <= 1 ? "gain" : "loss"}>{wealthDifference === null ? "等待录入" : `差 ${formatMoney(wealthDifference, "CNY")}`}</small></div><div><span>软件存款（折算）</span><strong>{formatMoney(trackedDeposit, "CNY")}</strong><small className={depositDifference === null ? "" : Math.abs(depositDifference) <= 1 ? "gain" : "loss"}>{depositDifference === null ? "等待录入" : `差 ${formatMoney(depositDifference, "CNY")}`}</small></div><div><span>银行总资产</span><strong>{actualTotal === null ? "—" : formatMoney(actualTotal, "CNY")}</strong><small>理财 + 存款 + 活期</small></div></div>
          <label className="full-field">备注（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="例如：银行 App 数据截至时间、汇率来源" /></label>
          <div className={`reconciliation-difference full-field ${wealthDifference === null || depositDifference === null ? "" : Math.abs(wealthDifference) <= 1 && Math.abs(depositDifference) <= 1 ? "matched" : "unmatched"}`}><span>分项核对结果</span><strong>{wealthDifference === null || depositDifference === null ? "—" : Math.abs(wealthDifference) <= 1 && Math.abs(depositDifference) <= 1 ? "一致" : "有差额"}</strong><small>理财和存款分别核对，避免两个分项的差额相互抵消</small></div>
          <div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting || !validInputs}><Scale />{submitting ? "保存中…" : "保存分项对账"}</button></div>
        </form>
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
  const [sort, setSort] = useState<SortState<TransactionSortKey> | null>(null);
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
  const sorted = useMemo(() => sortRows(filtered, sort, (item, key) => {
    switch (key) {
      case "tradeDate": return item.tradeDate;
      case "operation": return operationLabels[item.operation] ?? item.operation;
      case "productName": return item.title;
      case "productCode": return item.code;
      case "currency": return item.currency;
      case "amount": return item.amount;
      case "costBasis": return item.costBasis;
      case "realizedGain": return item.realizedGain;
      case "note": return item.note;
    }
  }), [filtered, sort]);
  const productSortActive = sort?.key === "productName" || sort?.key === "productCode";
  const changeSort = (key: TransactionSortKey) => setSort((current) => toggleSort(current, key));
  const gains = filtered.reduce<Record<CurrencyCode, number>>((totals, item) => {
    if (item.reversedBy === null && item.operation !== "REVERSAL") totals[item.currency] += item.realizedGain;
    return totals;
  }, { CNY: 0, USD: 0 });
  return (
    <article className="panel table-panel">
      <div className="panel-header"><div><h2>交易流水</h2><span>显示 {filtered.length}/{transactions.length} 笔 · 筛选结果已实现收益 {formatMoney(gains.CNY, "CNY")} / {formatMoney(gains.USD, "USD")}</span></div><button className="button secondary compact" type="button" onClick={() => { setSearchText(""); setOperationFilter("all"); setCurrencyFilter("all"); setInstitutionFilter("all"); setStatusFilter("all"); setDateFrom(""); setDateTo(""); }}><ListFilter />清除筛选</button></div>
      <div className="filter-bar transaction-filters"><label className="search-field"><Search /><input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="搜索产品、代码、银行或备注" /></label><select value={operationFilter} onChange={(event) => setOperationFilter(event.target.value)}><option value="all">全部操作</option>{Object.entries(operationLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select><select value={currencyFilter} onChange={(event) => setCurrencyFilter(event.target.value)}><option value="all">全部币种</option><option value="CNY">人民币</option><option value="USD">美元</option></select><select value={institutionFilter} onChange={(event) => setInstitutionFilter(event.target.value)}><option value="all">全部银行</option>{institutions.map((institution) => <option key={institution} value={institution}>{institution}</option>)}</select><select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}><option value="all">全部状态</option><option value="effective">有效流水</option><option value="reversed">冲销记录</option><option value="review">待核对</option></select><div className="transaction-date-range"><label className="date-filter">从<DateInput ariaLabel="开始日期" value={dateFrom} onChange={setDateFrom} /></label><label className="date-filter">至<DateInput ariaLabel="结束日期" value={dateTo} onChange={setDateTo} /></label></div></div>
      <div className="table-scroll"><table className="transaction-table"><colgroup><col className="transaction-date-col" /><col className="transaction-operation-col" /><col /><col className="transaction-currency-col" /><col className="transaction-money-col" /><col className="transaction-money-col" /><col className="transaction-money-col" /><col className="transaction-note-col" /></colgroup><thead><tr><SortableHeader label="日期" active={sort?.key === "tradeDate"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("tradeDate")} /><SortableHeader label="操作" active={sort?.key === "operation"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("operation")} /><SortableHeader label="产品" active={productSortActive} direction={sort?.direction ?? "asc"} activeDetail={sort?.key === "productCode" ? "代码" : "名称"} onSort={() => setSort((current) => cycleProductSort(current, "productName", "productCode"))} /><SortableHeader label="币种" active={sort?.key === "currency"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("currency")} /><SortableHeader label="现金金额" className="number" active={sort?.key === "amount"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("amount")} /><SortableHeader label="核销成本" className="number" active={sort?.key === "costBasis"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("costBasis")} /><SortableHeader label="已实现收益" className="number" active={sort?.key === "realizedGain"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("realizedGain")} /><SortableHeader label="备注" active={sort?.key === "note"} direction={sort?.direction ?? "asc"} onSort={() => changeSort("note")} /></tr></thead><tbody>
        {sorted.map((item) => {
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
          <label>操作日期<DateInput required ariaLabel="操作日期" value={tradeDate} onChange={setTradeDate} /></label>
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
        <form className="entry-form redemption-form" onSubmit={(event) => void submit(event)}><label>实际赎回日期<DateInput required ariaLabel="实际赎回日期" value={tradeDate} onChange={setTradeDate} /></label><label>银行实际到账金额<input required autoFocus type="number" min="0" step="0.01" value={proceeds} onChange={(event) => setProceeds(event.target.value)} /></label><label className="full-field">核对说明（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="例如：银行流水已核对、含分红或手续费" /></label><div className={`redemption-result full-field ${realizedGain === null ? "" : realizedGain >= 0 ? "gain" : "loss"}`}><span>本批次已实现收益</span><strong>{realizedGain === null ? "—" : formatMoney(realizedGain, detail.currency)}</strong><small>实际到账金额 − 原买入成本；保存后将更新收益分析和 XIRR</small></div><div className="calculation-note full-field">核对结果保存在软件数据库中，不修改原 Excel；以后重新导入同一批次时会自动沿用。</div><div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><CheckCircle2 />{submitting ? "保存中…" : "确认核对"}</button></div></form>
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
          {deposit && <><label>存款账户<select required value={accountId} onChange={(event) => { setAccountId(event.target.value); const selected = accounts.find((item) => item.id.toString() === event.target.value); if (selected) setCurrency(selected.currency); }}><option value="">请选择</option>{accounts.map((item) => <option value={item.id} key={item.id}>{item.institution} · {item.name} · {item.currency}</option>)}</select></label><label>币种<input value={currency} disabled /></label><label>本金<input required type="number" min="0.01" step="0.01" value={principal} onChange={(event) => setPrincipal(event.target.value)} /></label><label>年利率（%）<input required type="number" min="0" max="100" step="0.01" value={annualRate} onChange={(event) => setAnnualRate(event.target.value)} /></label><label>起息日（可选）<DateInput ariaLabel="起息日" value={startDate} onChange={setStartDate} /></label><label>到期日<DateInput required ariaLabel="到期日" value={maturityDate} onChange={setMaturityDate} /></label></>}
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
          {operation === "DEPOSIT_OPEN" && <><label className="full-field">存款名称<input required value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：整存整取-3年" /></label><label>存款银行<input required value={institution} onChange={(event) => setInstitution(event.target.value)} /></label><label>币种<select value={currency} onChange={(event) => setCurrency(event.target.value as CurrencyCode)}><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select></label><label>到期日<DateInput required ariaLabel="到期日" value={maturityDate} onChange={setMaturityDate} /></label><label>年利率（%）<input required type="number" min="0" max="100" step="0.01" value={annualRate} onChange={(event) => setAnnualRate(event.target.value)} placeholder="2.85" /></label></>}

          {operation !== "VALUATION" && <label>{operation === "BUY" ? "买入金额" : operation === "DEPOSIT_OPEN" ? "存款本金" : operation === "DIVIDEND" ? "分红金额" : operation === "FEE" ? "费用金额" : operation === "TRANSFER" ? "转账金额" : "实际到账金额"}<input required type="number" min="0.01" step="0.01" value={amount} onChange={(event) => setAmount(event.target.value)} /></label>}
          {operation === "VALUATION" && <label>当前总市值<input required type="number" min="0.01" step="0.01" value={marketValue} onChange={(event) => setMarketValue(event.target.value)} /></label>}
          <label>操作日期<DateInput required ariaLabel="操作日期" value={tradeDate} onChange={setTradeDate} /></label>
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
