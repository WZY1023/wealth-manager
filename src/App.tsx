import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  BellRing,
  CalendarDays,
  CheckCircle2,
  ClipboardList,
  Database,
  FileSpreadsheet,
  LayoutDashboard,
  Plus,
  RefreshCw,
  Save,
  ShieldCheck,
  WalletCards,
  X,
} from "lucide-react";
import "./App.css";

type View = "overview" | "holdings" | "transactions" | "calendar";
type CurrencyCode = "CNY" | "USD";
type EntryOperation = "BUY" | "SELL" | "PRODUCT_MATURITY" | "VALUATION" | "DEPOSIT_OPEN" | "DEPOSIT_MATURITY";

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
};

type TransactionRecord = {
  id: number;
  title: string;
  code: string | null;
  operation: string;
  tradeDate: string;
  amount: number;
  costBasis: number;
  realizedGain: number;
  currency: CurrencyCode;
  note: string | null;
};

type EntryInput = {
  operation: EntryOperation;
  productId: number | null;
  accountId: number | null;
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

const viewTitles: Record<View, string> = {
  overview: "资产总览",
  holdings: "当前持仓",
  transactions: "交易流水",
  calendar: "到期日历",
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
  const [loading, setLoading] = useState(true);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [entryOpen, setEntryOpen] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextDashboard, nextHoldings, nextMaturities, nextTransactions] = await Promise.all([
        invoke<Dashboard>("get_dashboard"),
        invoke<Holding[]>("list_holdings"),
        invoke<MaturityEvent[]>("list_maturities"),
        invoke<TransactionRecord[]>("list_transactions"),
      ]);
      setDashboard(nextDashboard);
      setHoldings(nextHoldings);
      setMaturities(nextMaturities);
      setTransactions(nextTransactions);
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
            {view === "holdings" && <HoldingsView holdings={holdings} />}
            {view === "transactions" && <TransactionsView transactions={transactions} />}
            {view === "calendar" && <CalendarView groups={groupedMaturities} />}
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
          onClose={() => setEntryOpen(false)}
          onSave={saveEntry}
        />
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

function HoldingsView({ holdings }: { holdings: Holding[] }) {
  return (
    <article className="panel table-panel">
      <div className="panel-header"><h2>当前持仓</h2><span>{holdings.length}个产品 · 已按产品代码合并分批买入</span></div>
      <div className="table-scroll"><table><thead><tr><th>产品</th><th>渠道</th><th>币种</th><th className="number">成本</th><th className="number">市值</th><th className="number">收益</th><th className="number">收益率</th><th>状态</th></tr></thead><tbody>
        {holdings.map((item) => <tr key={`${item.id}-${item.currency}`}><td><strong>{item.name}</strong><span>{item.code}</span></td><td>{item.channel}</td><td>{item.currency}</td><td className="number">{formatMoney(item.cost, item.currency)}</td><td className="number">{formatMoney(item.marketValue, item.currency)}</td><td className={`number ${item.gain >= 0 ? "gain" : "loss"}`}>{formatMoney(item.gain, item.currency)}</td><td className={`number ${item.gainRate >= 0 ? "gain" : "loss"}`}>{formatPercent(item.gainRate)}</td><td><span className="status">{item.status}</span></td></tr>)}
      </tbody></table></div>
    </article>
  );
}

function CalendarView({ groups }: { groups: Record<string, MaturityEvent[]> }) {
  return <section className="calendar-list">{Object.entries(groups).map(([month, events]) => <article className="panel month-panel" key={month}><div className="month-title"><h2>{month.replace("-", "年")}月</h2><span>{events.length}笔到期</span></div><div className="timeline">{events.map((item) => <div className="timeline-item" key={item.id}><time>{item.maturityDate.slice(5)}</time><div><strong>{item.name}</strong><span>{item.institution} · 年利率 {formatPercent(item.annualRate)}</span></div><b>{formatMoney(item.amount, item.currency)}</b></div>)}</div></article>)}</section>;
}

const operationLabels: Record<string, string> = {
  BUY: "买入",
  SELL: "卖出",
  REDEEM: "历史赎回",
  PRODUCT_MATURITY: "理财到期",
  DEPOSIT_OPEN: "定存开户",
  DEPOSIT_MATURITY: "定存到期",
};

function TransactionsView({ transactions }: { transactions: TransactionRecord[] }) {
  return (
    <article className="panel table-panel">
      <div className="panel-header"><h2>交易流水</h2><span>最近{transactions.length}笔 · 买卖成本和已实现收益可追溯</span></div>
      <div className="table-scroll"><table><thead><tr><th>日期</th><th>操作</th><th>产品</th><th>币种</th><th className="number">现金金额</th><th className="number">核销成本</th><th className="number">已实现收益</th><th>备注</th></tr></thead><tbody>
        {transactions.map((item) => {
          const incoming = ["SELL", "REDEEM", "PRODUCT_MATURITY", "DEPOSIT_MATURITY"].includes(item.operation);
          const hasRealized = incoming && (item.operation !== "REDEEM" || Math.abs(item.realizedGain) > 0.000001);
          return <tr key={item.id}><td className="nowrap">{item.tradeDate}</td><td><span className={`operation-tag ${incoming ? "incoming" : "outgoing"}`}>{operationLabels[item.operation] ?? item.operation}</span></td><td><strong>{item.title}</strong>{item.code && <span>{item.code}</span>}</td><td>{item.currency}</td><td className={`number ${incoming ? "gain" : ""}`}>{incoming ? "+" : "-"}{formatMoney(item.amount, item.currency)}</td><td className="number">{item.costBasis > 0 ? formatMoney(item.costBasis, item.currency) : "—"}</td><td className={`number ${item.realizedGain >= 0 ? "gain" : "loss"}`}>{hasRealized ? formatMoney(item.realizedGain, item.currency) : "—"}</td><td className="note-cell">{item.note ?? "—"}</td></tr>;
        })}
      </tbody></table></div>
    </article>
  );
}

const entryOperations: { value: EntryOperation; label: string; help: string }[] = [
  { value: "BUY", label: "买入理财", help: "自动新增批次" },
  { value: "SELL", label: "卖出理财", help: "自动 FIFO 核销" },
  { value: "PRODUCT_MATURITY", label: "理财到期", help: "全部结清" },
  { value: "VALUATION", label: "更新市值", help: "保留历史快照" },
  { value: "DEPOSIT_OPEN", label: "新增定存", help: "加入到期日历" },
  { value: "DEPOSIT_MATURITY", label: "定存到期", help: "拆分本金利息" },
];

function EntryModal({ holdings, deposits, onClose, onSave }: {
  holdings: Holding[];
  deposits: MaturityEvent[];
  onClose: () => void;
  onSave: (input: EntryInput) => Promise<void>;
}) {
  const today = new Date().toISOString().slice(0, 10);
  const [operation, setOperation] = useState<EntryOperation>("BUY");
  const [holdingChoice, setHoldingChoice] = useState("new");
  const [depositChoice, setDepositChoice] = useState(deposits[0]?.id.toString() ?? "");
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
  const needsHolding = ["SELL", "PRODUCT_MATURITY", "VALUATION"].includes(operation);
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
      accountId: needsHolding ? operationHolding?.accountId ?? null : null,
      depositId: operation === "DEPOSIT_MATURITY" ? selectedDeposit?.id ?? null : null,
      productName: operation === "DEPOSIT_OPEN" || (operation === "BUY" && !isExistingBuy) ? name : null,
      productCode: operation === "BUY" && !isExistingBuy ? code : null,
      institution: operation === "BUY" ? (isExistingBuy ? selectedHolding.channel : institution) : operation === "DEPOSIT_OPEN" ? institution : null,
      currency: operation === "BUY" ? (isExistingBuy ? selectedHolding.currency : currency) : operation === "DEPOSIT_OPEN" ? currency : operation === "DEPOSIT_MATURITY" ? selectedDeposit?.currency ?? null : selectedHolding?.currency ?? null,
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

          {operation === "BUY" && !isExistingBuy && <><label className="full-field">产品名称<input required value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：稳富固收增强" /></label><label>产品代码<input required value={code} onChange={(event) => setCode(event.target.value)} /></label><label>购买渠道<input required value={institution} onChange={(event) => setInstitution(event.target.value)} placeholder="例如：中国银行" /></label><label>币种<select value={currency} onChange={(event) => setCurrency(event.target.value as CurrencyCode)}><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select></label></>}
          {operation === "DEPOSIT_OPEN" && <><label className="full-field">存款名称<input required value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：整存整取-3年" /></label><label>存款银行<input required value={institution} onChange={(event) => setInstitution(event.target.value)} /></label><label>币种<select value={currency} onChange={(event) => setCurrency(event.target.value as CurrencyCode)}><option value="CNY">人民币 CNY</option><option value="USD">美元 USD</option></select></label><label>到期日<input required type="date" value={maturityDate} onChange={(event) => setMaturityDate(event.target.value)} /></label><label>年利率（%）<input required type="number" min="0" max="100" step="0.01" value={annualRate} onChange={(event) => setAnnualRate(event.target.value)} placeholder="2.85" /></label></>}

          {operation !== "VALUATION" && <label>{operation === "BUY" ? "买入金额" : operation === "DEPOSIT_OPEN" ? "存款本金" : "实际到账金额"}<input required type="number" min="0.01" step="0.01" value={amount} onChange={(event) => setAmount(event.target.value)} /></label>}
          {operation === "VALUATION" && <label>当前总市值<input required type="number" min="0.01" step="0.01" value={marketValue} onChange={(event) => setMarketValue(event.target.value)} /></label>}
          <label>操作日期<input required type="date" value={tradeDate} onChange={(event) => setTradeDate(event.target.value)} /></label>
          <label className="full-field">备注（可选）<input value={note} onChange={(event) => setNote(event.target.value)} placeholder="银行流水说明、确认号等" /></label>

          {operation === "SELL" && selectedHolding && <div className="calculation-note full-field">将以当前市值 {formatMoney(selectedHolding.marketValue, selectedHolding.currency)} 为基准，按卖出比例计算应核销成本，并从最早的买入批次开始自动扣减。</div>}
          {operation === "PRODUCT_MATURITY" && selectedHolding && <div className="calculation-note full-field">将结清全部剩余成本 {formatMoney(selectedHolding.cost, selectedHolding.currency)}，到账金额与成本的差额计入已实现收益。</div>}
          {operation === "VALUATION" && <div className="calculation-note full-field">这里只更新产品总市值，不改变买入成本；旧市值会保留为历史快照。</div>}

          <div className="entry-actions full-field"><button className="button secondary" type="button" onClick={onClose}>取消</button><button className="button primary" type="submit" disabled={submitting}><Save />{submitting ? "保存中…" : "保存操作"}</button></div>
        </form>
      </section>
    </div>
  );
}

export default App;
