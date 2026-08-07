import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  BellRing,
  CalendarDays,
  CheckCircle2,
  Database,
  FileSpreadsheet,
  LayoutDashboard,
  Plus,
  RefreshCw,
  ShieldCheck,
  WalletCards,
  X,
} from "lucide-react";
import "./App.css";

type View = "overview" | "holdings" | "calendar";
type CurrencyCode = "CNY" | "USD";

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
  const [loading, setLoading] = useState(true);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextDashboard, nextHoldings, nextMaturities] = await Promise.all([
        invoke<Dashboard>("get_dashboard"),
        invoke<Holding[]>("list_holdings"),
        invoke<MaturityEvent[]>("list_maturities"),
      ]);
      setDashboard(nextDashboard);
      setHoldings(nextHoldings);
      setMaturities(nextMaturities);
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
            <button className="button primary" type="button" onClick={() => setNotice("手工记账将在第二阶段接入数据库")}><Plus />记一笔</button>
          </div>
        </header>

        {notice && <div className="banner success"><CheckCircle2 /><span>{notice}</span><button onClick={() => setNotice(null)} aria-label="关闭"><X /></button></div>}
        {error && <div className="banner error"><AlertTriangle /><span>{error}</span><button onClick={() => setError(null)} aria-label="关闭"><X /></button></div>}

        {loading && dashboard.holdingCount === 0 ? (
          <div className="loading-state"><RefreshCw className="spin" />正在读取本地数据…</div>
        ) : dashboard.holdingCount === 0 ? (
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

export default App;
