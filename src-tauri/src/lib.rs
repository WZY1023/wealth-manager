use calamine::{open_workbook_auto, Data, DataType, Reader};
use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Mutex};
use tauri::{Manager, State};

struct AppState {
    db: Mutex<Connection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrencySummary {
    currency: String,
    wealth_value: f64,
    deposit_value: f64,
    total_value: f64,
    invested_cost: f64,
    unrealized_gain: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Dashboard {
    as_of_date: String,
    last_import_at: Option<String>,
    currencies: Vec<CurrencySummary>,
    holding_count: i64,
    warning_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Holding {
    id: i64,
    product_id: i64,
    account_id: i64,
    name: String,
    code: String,
    currency: String,
    channel: String,
    cost: f64,
    market_value: f64,
    gain: f64,
    gain_rate: f64,
    valuation_date: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryInput {
    operation: String,
    product_id: Option<i64>,
    account_id: Option<i64>,
    deposit_id: Option<i64>,
    product_name: Option<String>,
    product_code: Option<String>,
    institution: Option<String>,
    currency: Option<String>,
    amount: Option<f64>,
    market_value: Option<f64>,
    trade_date: String,
    maturity_date: Option<String>,
    annual_rate: Option<f64>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryResult {
    message: String,
    realized_gain: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransactionRecord {
    id: i64,
    title: String,
    code: Option<String>,
    operation: String,
    trade_date: String,
    amount: f64,
    cost_basis: f64,
    realized_gain: f64,
    currency: String,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaturityEvent {
    id: i64,
    name: String,
    institution: String,
    currency: String,
    amount: f64,
    maturity_date: String,
    days_remaining: i64,
    annual_rate: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportReport {
    source_path: String,
    imported_at: String,
    lots_imported: usize,
    holdings_imported: usize,
    deposits_imported: usize,
    warnings: Vec<String>,
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS accounts (
          id INTEGER PRIMARY KEY,
          institution TEXT NOT NULL,
          name TEXT NOT NULL,
          currency TEXT NOT NULL,
          account_type TEXT NOT NULL DEFAULT 'bank',
          source TEXT NOT NULL DEFAULT 'manual',
          UNIQUE(institution, name, currency)
        );

        CREATE TABLE IF NOT EXISTS products (
          id INTEGER PRIMARY KEY,
          code TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          issuer TEXT,
          category TEXT NOT NULL DEFAULT 'wealth',
          currency TEXT NOT NULL,
          risk_level TEXT,
          liquidity_rule TEXT,
          source TEXT NOT NULL DEFAULT 'manual'
        );

        CREATE TABLE IF NOT EXISTS transactions (
          id INTEGER PRIMARY KEY,
          product_id INTEGER,
          account_id INTEGER,
          transaction_type TEXT NOT NULL,
          trade_date TEXT NOT NULL,
          amount REAL NOT NULL,
          currency TEXT NOT NULL,
          note TEXT,
          cost_basis REAL NOT NULL DEFAULT 0,
          realized_gain REAL NOT NULL DEFAULT 0,
          deposit_id INTEGER,
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(account_id) REFERENCES accounts(id),
          FOREIGN KEY(deposit_id) REFERENCES deposits(id)
        );

        CREATE TABLE IF NOT EXISTS position_lots (
          id INTEGER PRIMARY KEY,
          product_id INTEGER NOT NULL,
          account_id INTEGER NOT NULL,
          purchase_date TEXT NOT NULL,
          original_amount REAL NOT NULL,
          remaining_amount REAL NOT NULL,
          end_date TEXT,
          status TEXT NOT NULL,
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(account_id) REFERENCES accounts(id)
        );

        CREATE TABLE IF NOT EXISTS valuations (
          id INTEGER PRIMARY KEY,
          product_id INTEGER NOT NULL,
          account_id INTEGER NOT NULL,
          valuation_date TEXT NOT NULL,
          market_value REAL NOT NULL,
          currency TEXT NOT NULL,
          is_current INTEGER NOT NULL DEFAULT 1,
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(account_id) REFERENCES accounts(id)
        );

        CREATE TABLE IF NOT EXISTS deposits (
          id INTEGER PRIMARY KEY,
          account_id INTEGER NOT NULL,
          name TEXT NOT NULL,
          currency TEXT NOT NULL,
          principal REAL NOT NULL,
          start_date TEXT,
          maturity_date TEXT NOT NULL,
          annual_rate REAL NOT NULL,
          status TEXT NOT NULL DEFAULT 'active',
          matured_at TEXT,
          proceeds REAL,
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(account_id) REFERENCES accounts(id)
        );

        CREATE TABLE IF NOT EXISTS import_runs (
          id INTEGER PRIMARY KEY,
          source_path TEXT NOT NULL,
          imported_at TEXT NOT NULL,
          lots_imported INTEGER NOT NULL,
          holdings_imported INTEGER NOT NULL,
          deposits_imported INTEGER NOT NULL,
          warnings_json TEXT NOT NULL
        );
        "#,
    )?;
    ensure_column(
        conn,
        "transactions",
        "cost_basis",
        "REAL NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "transactions",
        "realized_gain",
        "REAL NOT NULL DEFAULT 0",
    )?;
    ensure_column(conn, "transactions", "deposit_id", "INTEGER")?;
    ensure_column(conn, "position_lots", "remaining_amount", "REAL")?;
    ensure_column(conn, "deposits", "status", "TEXT NOT NULL DEFAULT 'active'")?;
    ensure_column(conn, "deposits", "matured_at", "TEXT")?;
    ensure_column(conn, "deposits", "proceeds", "REAL")?;
    conn.execute(
        "UPDATE position_lots SET remaining_amount = original_amount WHERE remaining_amount IS NULL",
        [],
    )?;
    conn.execute(
        "UPDATE valuations SET is_current = 0
         WHERE is_current = 1 AND id NOT IN (
           SELECT MAX(id) FROM valuations WHERE is_current = 1 GROUP BY product_id, account_id
         )",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS one_current_valuation_per_holding
         ON valuations(product_id, account_id) WHERE is_current = 1",
        [],
    )?;
    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !names.iter().any(|name| name == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn text(cell: Option<&Data>) -> String {
    cell.and_then(DataType::as_string)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn number(cell: Option<&Data>) -> Option<f64> {
    cell.and_then(DataType::as_f64).or_else(|| {
        cell.and_then(DataType::as_string)
            .and_then(|value| value.replace([',', '¥', '$'], "").parse::<f64>().ok())
    })
}

fn excel_date(cell: Option<&Data>) -> Option<NaiveDate> {
    let cell = cell?;
    if let Some(value) = cell.as_datetime() {
        return Some(value.date());
    }
    if let Some(value) = cell.as_f64() {
        let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?;
        return epoch.checked_add_signed(Duration::days(value.floor() as i64));
    }
    let value = cell.as_string()?;
    ["%Y-%m-%d", "%m/%d/%y", "%m/%d/%Y"]
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(value.trim(), format).ok())
}

fn account_id(tx: &Transaction<'_>, institution: &str, currency: &str) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO accounts (institution, name, currency, source) VALUES (?1, ?1, ?2, 'excel')
         ON CONFLICT(institution, name, currency) DO NOTHING",
        params![institution, currency],
    )?;
    tx.query_row(
        "SELECT id FROM accounts WHERE institution = ?1 AND name = ?1 AND currency = ?2",
        params![institution, currency],
        |row| row.get(0),
    )
}

fn product_id(
    tx: &Transaction<'_>,
    code: &str,
    name: &str,
    currency: &str,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO products (code, name, currency, source) VALUES (?1, ?2, ?3, 'excel')
         ON CONFLICT(code) DO UPDATE SET name = excluded.name",
        params![code, name, currency],
    )?;
    tx.query_row(
        "SELECT id FROM products WHERE code = ?1",
        params![code],
        |row| row.get(0),
    )
}

fn required_text(value: &Option<String>, label: &str) -> Result<String, String> {
    let value = value.as_deref().unwrap_or_default().trim();
    if value.is_empty() {
        Err(format!("请填写{label}"))
    } else {
        Ok(value.to_string())
    }
}

fn positive_amount(value: Option<f64>, label: &str) -> Result<f64, String> {
    let value = value.unwrap_or_default();
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(format!("{label}必须大于0"))
    }
}

fn valid_date(value: &str, label: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|date| date.format("%Y-%m-%d").to_string())
        .map_err(|_| format!("{label}格式不正确"))
}

fn manual_account_id(
    tx: &Transaction<'_>,
    institution: &str,
    currency: &str,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO accounts (institution, name, currency, source) VALUES (?1, ?1, ?2, 'manual')
         ON CONFLICT(institution, name, currency) DO NOTHING",
        params![institution, currency],
    )?;
    tx.query_row(
        "SELECT id FROM accounts WHERE institution = ?1 AND name = ?1 AND currency = ?2",
        params![institution, currency],
        |row| row.get(0),
    )
}

fn manual_product_id(
    tx: &Transaction<'_>,
    code: &str,
    name: &str,
    currency: &str,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO products (code, name, currency, source) VALUES (?1, ?2, ?3, 'manual')
         ON CONFLICT(code) DO UPDATE SET name = excluded.name",
        params![code, name, currency],
    )?;
    tx.query_row(
        "SELECT id FROM products WHERE code = ?1",
        params![code],
        |row| row.get(0),
    )
}

fn current_market_value(
    tx: &Transaction<'_>,
    product: i64,
    account: i64,
) -> rusqlite::Result<Option<f64>> {
    tx.query_row(
        "SELECT market_value FROM valuations
         WHERE product_id = ?1 AND account_id = ?2 AND is_current = 1
         ORDER BY valuation_date DESC, id DESC LIMIT 1",
        params![product, account],
        |row| row.get(0),
    )
    .optional()
}

fn replace_current_valuation(
    tx: &Transaction<'_>,
    product: i64,
    account: i64,
    date: &str,
    value: f64,
    currency: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE valuations SET is_current = 0 WHERE product_id = ?1 AND account_id = ?2 AND is_current = 1",
        params![product, account],
    )?;
    tx.execute(
        "INSERT INTO valuations (product_id, account_id, valuation_date, market_value, currency, is_current, source)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, 'manual')",
        params![product, account, date, value.max(0.0), currency],
    )?;
    Ok(())
}

fn holding_currency(tx: &Transaction<'_>, product: i64, account: i64) -> Result<String, String> {
    tx.query_row(
        "SELECT p.currency FROM products p
         JOIN position_lots l ON l.product_id = p.id
         WHERE p.id = ?1 AND l.account_id = ?2 LIMIT 1",
        params![product, account],
        |row| row.get(0),
    )
    .map_err(|_| "没有找到对应持仓".to_string())
}

fn active_cost(
    tx: &Transaction<'_>,
    product: i64,
    account: i64,
    through_date: &str,
) -> rusqlite::Result<f64> {
    tx.query_row(
        "SELECT COALESCE(SUM(COALESCE(remaining_amount, original_amount)), 0)
         FROM position_lots
         WHERE product_id = ?1 AND account_id = ?2 AND status = 'active'
           AND purchase_date <= ?3",
        params![product, account, through_date],
        |row| row.get(0),
    )
}

fn consume_fifo_cost(
    tx: &Transaction<'_>,
    product: i64,
    account: i64,
    through_date: &str,
    mut cost_to_remove: f64,
) -> rusqlite::Result<()> {
    let lots = {
        let mut statement = tx.prepare(
            "SELECT id, COALESCE(remaining_amount, original_amount)
             FROM position_lots
             WHERE product_id = ?1 AND account_id = ?2 AND status = 'active'
               AND purchase_date <= ?3
             ORDER BY purchase_date, id",
        )?;
        let rows = statement
            .query_map(params![product, account, through_date], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for (lot_id, remaining) in lots {
        if cost_to_remove <= 0.000_001 {
            break;
        }
        let consumed = remaining.min(cost_to_remove);
        let next = (remaining - consumed).max(0.0);
        tx.execute(
            "UPDATE position_lots SET remaining_amount = ?1, status = ?2 WHERE id = ?3",
            params![
                next,
                if next <= 0.000_001 {
                    "closed"
                } else {
                    "active"
                },
                lot_id
            ],
        )?;
        cost_to_remove -= consumed;
    }
    Ok(())
}

fn replay_manual_exits(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE position_lots
         SET remaining_amount = original_amount, status = 'active'
         WHERE source = 'manual'",
        [],
    )?;
    let exits = {
        let mut statement = tx.prepare(
            "SELECT product_id, account_id, trade_date, cost_basis
             FROM transactions
             WHERE source = 'manual'
               AND transaction_type IN ('SELL', 'PRODUCT_MATURITY')
               AND product_id IS NOT NULL AND account_id IS NOT NULL
             ORDER BY trade_date, id",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for (product, account, date, cost_basis) in exits {
        consume_fifo_cost(tx, product, account, &date, cost_basis)?;
    }
    Ok(())
}

fn apply_entry(conn: &mut Connection, input: &EntryInput) -> Result<EntryResult, String> {
    let operation = input.operation.trim().to_uppercase();
    let trade_date = valid_date(&input.trade_date, "操作日期")?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let result = match operation.as_str() {
        "BUY" => {
            let amount = positive_amount(input.amount, "买入金额")?;
            let currency = required_text(&input.currency, "币种")?.to_uppercase();
            let institution = required_text(&input.institution, "购买渠道")?;
            let (product, product_name) = if let Some(product) = input.product_id {
                let name = tx
                    .query_row(
                        "SELECT name FROM products WHERE id = ?1",
                        params![product],
                        |row| row.get(0),
                    )
                    .map_err(|_| "没有找到所选产品".to_string())?;
                (product, name)
            } else {
                let name = required_text(&input.product_name, "产品名称")?;
                let code = required_text(&input.product_code, "产品代码")?;
                let product = manual_product_id(&tx, &code, &name, &currency)
                    .map_err(|error| error.to_string())?;
                (product, name)
            };
            let account = manual_account_id(&tx, &institution, &currency)
                .map_err(|error| error.to_string())?;
            tx.execute(
                "INSERT INTO transactions (product_id, account_id, transaction_type, trade_date, amount, currency, note, cost_basis, source)
                 VALUES (?1, ?2, 'BUY', ?3, ?4, ?5, ?6, ?4, 'manual')",
                params![product, account, trade_date, amount, currency, input.note],
            )
            .map_err(|error| error.to_string())?;
            tx.execute(
                "INSERT INTO position_lots (product_id, account_id, purchase_date, original_amount, remaining_amount, status, source)
                 VALUES (?1, ?2, ?3, ?4, ?4, 'active', 'manual')",
                params![product, account, trade_date, amount],
            )
            .map_err(|error| error.to_string())?;
            let previous = current_market_value(&tx, product, account)
                .map_err(|error| error.to_string())?
                .unwrap_or_default();
            replace_current_valuation(
                &tx,
                product,
                account,
                &trade_date,
                previous + amount,
                &currency,
            )
            .map_err(|error| error.to_string())?;
            EntryResult {
                message: format!("已买入 {product_name}，并自动新增一个持仓批次"),
                realized_gain: None,
            }
        }
        "SELL" | "PRODUCT_MATURITY" => {
            let proceeds = positive_amount(input.amount, "到账金额")?;
            let product = input
                .product_id
                .ok_or_else(|| "请选择理财产品".to_string())?;
            let account = input
                .account_id
                .ok_or_else(|| "请选择购买渠道".to_string())?;
            let currency = holding_currency(&tx, product, account)?;
            let market = current_market_value(&tx, product, account)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "请先录入该产品的当前市值".to_string())?;
            let cost = active_cost(&tx, product, account, &trade_date)
                .map_err(|error| error.to_string())?;
            if cost <= 0.000_001 {
                return Err("该产品没有可卖出的持仓成本".to_string());
            }
            let full = operation == "PRODUCT_MATURITY" || proceeds >= market * 0.999_999;
            if !full && proceeds > market {
                return Err("部分卖出的到账金额不能超过当前市值".to_string());
            }
            let cost_basis = if full { cost } else { cost * proceeds / market };
            consume_fifo_cost(&tx, product, account, &trade_date, cost_basis)
                .map_err(|error| error.to_string())?;
            let realized_gain = proceeds - cost_basis;
            let next_market = if full {
                0.0
            } else {
                (market - proceeds).max(0.0)
            };
            replace_current_valuation(&tx, product, account, &trade_date, next_market, &currency)
                .map_err(|error| error.to_string())?;
            tx.execute(
                "INSERT INTO transactions (product_id, account_id, transaction_type, trade_date, amount, currency, note, cost_basis, realized_gain, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'manual')",
                params![
                    product,
                    account,
                    if operation == "PRODUCT_MATURITY" { "PRODUCT_MATURITY" } else { "SELL" },
                    trade_date,
                    proceeds,
                    currency,
                    input.note,
                    cost_basis,
                    realized_gain
                ],
            )
            .map_err(|error| error.to_string())?;
            EntryResult {
                message: if full {
                    "该持仓已全部结清，多个买入批次已按 FIFO 自动核销".to_string()
                } else {
                    "部分卖出已完成，持仓成本与多个买入批次已自动调整".to_string()
                },
                realized_gain: Some(realized_gain),
            }
        }
        "VALUATION" => {
            let value = positive_amount(input.market_value, "当前总市值")?;
            let product = input
                .product_id
                .ok_or_else(|| "请选择理财产品".to_string())?;
            let account = input
                .account_id
                .ok_or_else(|| "请选择购买渠道".to_string())?;
            let currency = holding_currency(&tx, product, account)?;
            replace_current_valuation(&tx, product, account, &trade_date, value, &currency)
                .map_err(|error| error.to_string())?;
            EntryResult {
                message: "当前市值已更新，旧市值已作为历史快照保留".to_string(),
                realized_gain: None,
            }
        }
        "DEPOSIT_OPEN" => {
            let institution = required_text(&input.institution, "存款银行")?;
            let name = required_text(&input.product_name, "存款名称")?;
            let currency = required_text(&input.currency, "币种")?.to_uppercase();
            let principal = positive_amount(input.amount, "存款本金")?;
            let maturity = valid_date(&required_text(&input.maturity_date, "到期日")?, "到期日")?;
            let annual_rate = input.annual_rate.unwrap_or_default();
            if !(0.0..=1.0).contains(&annual_rate) {
                return Err("年利率必须在0%到100%之间".to_string());
            }
            let account = manual_account_id(&tx, &institution, &currency)
                .map_err(|error| error.to_string())?;
            tx.execute(
                "INSERT INTO deposits (account_id, name, currency, principal, start_date, maturity_date, annual_rate, status, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', 'manual')",
                params![account, name, currency, principal, trade_date, maturity, annual_rate],
            )
            .map_err(|error| error.to_string())?;
            let deposit = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO transactions (account_id, deposit_id, transaction_type, trade_date, amount, currency, note, cost_basis, source)
                 VALUES (?1, ?2, 'DEPOSIT_OPEN', ?3, ?4, ?5, ?6, ?4, 'manual')",
                params![account, deposit, trade_date, principal, currency, input.note],
            )
            .map_err(|error| error.to_string())?;
            EntryResult {
                message: "定期存款已加入资产和到期日历".to_string(),
                realized_gain: None,
            }
        }
        "DEPOSIT_MATURITY" => {
            let deposit = input
                .deposit_id
                .ok_or_else(|| "请选择到期存款".to_string())?;
            let (account, currency, principal, status): (i64, String, f64, String) = tx
                .query_row(
                    "SELECT account_id, currency, principal, status FROM deposits WHERE id = ?1",
                    params![deposit],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(|_| "没有找到所选存款".to_string())?;
            if status != "active" {
                return Err("该存款已经结清".to_string());
            }
            let proceeds = positive_amount(input.amount, "到账金额")?;
            let realized_gain = proceeds - principal;
            tx.execute(
                "UPDATE deposits SET status = 'matured', matured_at = ?1, proceeds = ?2 WHERE id = ?3",
                params![trade_date, proceeds, deposit],
            )
            .map_err(|error| error.to_string())?;
            tx.execute(
                "INSERT INTO transactions (account_id, deposit_id, transaction_type, trade_date, amount, currency, note, cost_basis, realized_gain, source)
                 VALUES (?1, ?2, 'DEPOSIT_MATURITY', ?3, ?4, ?5, ?6, ?7, ?8, 'manual')",
                params![account, deposit, trade_date, proceeds, currency, input.note, principal, realized_gain],
            )
            .map_err(|error| error.to_string())?;
            EntryResult {
                message: "定期存款已结清，本金和利息收益已自动拆分".to_string(),
                realized_gain: Some(realized_gain),
            }
        }
        _ => return Err("不支持的记账操作".to_string()),
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(result)
}

fn clear_excel_data(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM valuations WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM position_lots WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM transactions WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM deposits WHERE source = 'excel'", [])?;
    tx.execute(
        "UPDATE position_lots
         SET remaining_amount = original_amount, status = 'active'
         WHERE source = 'manual'",
        [],
    )?;
    tx.execute(
        "DELETE FROM products WHERE source = 'excel'
         AND id NOT IN (SELECT DISTINCT product_id FROM transactions WHERE product_id IS NOT NULL)
         AND id NOT IN (SELECT DISTINCT product_id FROM position_lots)
         AND id NOT IN (SELECT DISTINCT product_id FROM valuations)",
        [],
    )?;
    tx.execute(
        "DELETE FROM accounts WHERE source = 'excel'
         AND id NOT IN (SELECT DISTINCT account_id FROM transactions WHERE account_id IS NOT NULL)
         AND id NOT IN (SELECT DISTINCT account_id FROM position_lots)
         AND id NOT IN (SELECT DISTINCT account_id FROM valuations)
         AND id NOT IN (SELECT DISTINCT account_id FROM deposits)",
        [],
    )?;
    Ok(())
}

fn parse_workbook(path: &Path, conn: &mut Connection) -> Result<ImportReport, String> {
    let mut workbook = open_workbook_auto(path).map_err(|error| error.to_string())?;
    let purchase_formulas = workbook
        .worksheet_formula("Sheet1")
        .map_err(|error| format!("无法读取 Sheet1 公式：{error}"))?;
    let purchases = workbook
        .worksheet_range("Sheet1")
        .map_err(|error| format!("无法读取 Sheet1：{error}"))?;
    let summaries = workbook
        .worksheet_range("Sheet2")
        .map_err(|error| format!("无法读取 Sheet2：{error}"))?;
    let deposits = workbook
        .worksheet_range("Sheet3")
        .map_err(|error| format!("无法读取 Sheet3：{error}"))?;

    let today = Local::now().date_naive();
    let workbook_as_of = purchases
        .rows()
        .enumerate()
        .skip(1)
        .find_map(|(index, row)| {
            let formula = purchase_formulas
                .get((index, 6))
                .map(|value| value.to_uppercase())
                .unwrap_or_default();
            formula
                .contains("TODAY")
                .then(|| excel_date(row.get(6)))
                .flatten()
        })
        .unwrap_or(today);
    let as_of = workbook_as_of.format("%Y-%m-%d").to_string();
    let imported_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut warnings = Vec::new();
    let mut lots_imported = 0usize;
    let mut holdings_imported = 0usize;
    let mut deposits_imported = 0usize;

    let tx = conn.transaction().map_err(|error| error.to_string())?;
    clear_excel_data(&tx).map_err(|error| error.to_string())?;

    for (index, row) in purchases.rows().enumerate().skip(1) {
        let name = text(row.first());
        let code = text(row.get(1));
        let currency = text(row.get(2)).to_uppercase();
        let institution = text(row.get(3));
        let amount = number(row.get(4));
        let purchase_date = excel_date(row.get(5));
        let end_date = excel_date(row.get(6));
        if name == "汇总" || name.is_empty() {
            continue;
        }
        if code.is_empty() || currency.is_empty() || amount.is_none() || purchase_date.is_none() {
            warnings.push(format!(
                "Sheet1 第{}行缺少代码、币种、金额或购买日期，已跳过",
                index + 1
            ));
            continue;
        }
        let amount = amount.unwrap_or_default();
        let purchase_date = purchase_date.unwrap_or(today);
        let active = end_date.map(|date| date >= workbook_as_of).unwrap_or(true);
        let product =
            product_id(&tx, &code, &name, &currency).map_err(|error| error.to_string())?;
        let account =
            account_id(&tx, &institution, &currency).map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO transactions (product_id, account_id, transaction_type, trade_date, amount, currency, note, cost_basis, source, source_row)
             VALUES (?1, ?2, 'BUY', ?3, ?4, ?5, 'Excel 导入', ?4, 'excel', ?6)",
            params![product, account, purchase_date.format("%Y-%m-%d").to_string(), amount, currency, (index + 1) as i64],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO position_lots (product_id, account_id, purchase_date, original_amount, remaining_amount, end_date, status, source, source_row)
             VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, 'excel', ?7)",
            params![
                product,
                account,
                purchase_date.format("%Y-%m-%d").to_string(),
                amount,
                end_date.map(|date| date.format("%Y-%m-%d").to_string()),
                if active { "active" } else { "closed" },
                (index + 1) as i64
            ],
        )
        .map_err(|error| error.to_string())?;
        if !active {
            tx.execute(
                "INSERT INTO transactions (product_id, account_id, transaction_type, trade_date, amount, currency, note, cost_basis, source, source_row)
                 VALUES (?1, ?2, 'REDEEM', ?3, ?4, ?5, '根据到期日期生成；赎回金额待核对', ?4, 'excel', ?6)",
                params![
                    product,
                    account,
                    end_date.unwrap_or(today).format("%Y-%m-%d").to_string(),
                    amount,
                    currency,
                    (index + 1) as i64
                ],
            )
            .map_err(|error| error.to_string())?;
        }
        lots_imported += 1;
    }

    replay_manual_exits(&tx).map_err(|error| error.to_string())?;

    for (index, row) in summaries.rows().enumerate().skip(1) {
        let name = text(row.first());
        if name.is_empty() {
            break;
        }
        let code = text(row.get(1));
        let currency = text(row.get(2)).to_uppercase();
        let institution = text(row.get(3));
        let market_value = number(row.get(4));
        if code.is_empty() || currency.is_empty() || market_value.is_none() {
            warnings.push(format!(
                "Sheet2 第{}行缺少代码、币种或市值，已跳过",
                index + 1
            ));
            continue;
        }
        let existing_currency: Option<String> = tx
            .query_row(
                "SELECT currency FROM products WHERE code = ?1",
                params![code],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some(existing) = existing_currency {
            if existing != currency {
                warnings.push(format!(
                    "产品 {code} 的币种不一致：买入记录为 {existing}，当前持仓为 {currency}"
                ));
            }
        }
        let product =
            product_id(&tx, &code, &name, &currency).map_err(|error| error.to_string())?;
        let account =
            account_id(&tx, &institution, &currency).map_err(|error| error.to_string())?;
        let has_manual_current: bool = tx
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM valuations
                   WHERE product_id = ?1 AND account_id = ?2 AND is_current = 1 AND source = 'manual'
                 )",
                params![product, account],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO valuations (product_id, account_id, valuation_date, market_value, currency, is_current, source, source_row)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'excel', ?7)",
            params![
                product,
                account,
                as_of,
                market_value.unwrap_or_default(),
                currency,
                if has_manual_current { 0 } else { 1 },
                (index + 1) as i64
            ],
        )
        .map_err(|error| error.to_string())?;
        if has_manual_current {
            warnings.push(format!(
                "产品 {code} 已有手工更新的市值，本次 Excel 市值仅作为历史记录导入"
            ));
        }
        holdings_imported += 1;
    }

    for (index, row) in deposits.rows().enumerate().skip(1) {
        let institution = text(row.first());
        let name = text(row.get(1));
        let currency = text(row.get(2)).to_uppercase();
        let principal = number(row.get(3));
        let maturity_date = excel_date(row.get(4));
        let annual_rate = number(row.get(5));
        if name.is_empty() {
            continue;
        }
        if currency.is_empty()
            || principal.is_none()
            || maturity_date.is_none()
            || annual_rate.is_none()
        {
            warnings.push(format!("Sheet3 第{}行信息不完整，已跳过", index + 1));
            continue;
        }
        let account =
            account_id(&tx, &institution, &currency).map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO deposits (account_id, name, currency, principal, maturity_date, annual_rate, source, source_row)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'excel', ?7)",
            params![
                account,
                name,
                currency,
                principal.unwrap_or_default(),
                maturity_date.unwrap_or(today).format("%Y-%m-%d").to_string(),
                annual_rate.unwrap_or_default(),
                (index + 1) as i64
            ],
        )
        .map_err(|error| error.to_string())?;
        deposits_imported += 1;
    }

    let warnings_json = serde_json::to_string(&warnings).map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO import_runs (source_path, imported_at, lots_imported, holdings_imported, deposits_imported, warnings_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            path.to_string_lossy(),
            imported_at,
            lots_imported as i64,
            holdings_imported as i64,
            deposits_imported as i64,
            warnings_json
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;

    Ok(ImportReport {
        source_path: path.to_string_lossy().to_string(),
        imported_at,
        lots_imported,
        holdings_imported,
        deposits_imported,
        warnings,
    })
}

#[tauri::command]
fn import_workbook(path: String, state: State<'_, AppState>) -> Result<ImportReport, String> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err("所选文件不存在".to_string());
    }
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    parse_workbook(path, &mut conn)
}

#[tauri::command]
fn get_dashboard(state: State<'_, AppState>) -> Result<Dashboard, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let mut currencies = Vec::new();
    for currency in ["CNY", "USD"] {
        let wealth_value: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(market_value), 0) FROM valuations WHERE is_current = 1 AND currency = ?1",
                params![currency],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let deposit_value: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(principal), 0) FROM deposits WHERE currency = ?1 AND status = 'active'",
                params![currency],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let invested_cost: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(COALESCE(remaining_amount, original_amount)), 0) FROM position_lots l
                 JOIN products p ON p.id = l.product_id WHERE l.status = 'active' AND p.currency = ?1",
                params![currency],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        currencies.push(CurrencySummary {
            currency: currency.to_string(),
            wealth_value,
            deposit_value,
            total_value: wealth_value + deposit_value,
            invested_cost,
            unrealized_gain: wealth_value - invested_cost,
        });
    }
    let holding_count = conn
        .query_row(
            "SELECT COUNT(*) FROM valuations WHERE is_current = 1 AND market_value > 0.000001",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let last_run: Option<(String, String)> = conn
        .query_row(
            "SELECT imported_at, warnings_json FROM import_runs ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let warning_count = last_run
        .as_ref()
        .and_then(|(_, json)| serde_json::from_str::<Vec<String>>(json).ok())
        .map(|items| items.len() as i64)
        .unwrap_or_default();
    Ok(Dashboard {
        as_of_date: Local::now().format("%Y-%m-%d").to_string(),
        last_import_at: last_run.map(|(date, _)| date),
        currencies,
        holding_count,
        warning_count,
    })
}

#[tauri::command]
fn list_holdings(state: State<'_, AppState>) -> Result<Vec<Holding>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT v.id, p.id, a.id, p.name, p.code, v.currency, a.institution,
                    COALESCE(SUM(COALESCE(l.remaining_amount, l.original_amount)), 0) AS cost,
                    v.market_value, v.valuation_date
             FROM valuations v
             JOIN products p ON p.id = v.product_id
             JOIN accounts a ON a.id = v.account_id
             LEFT JOIN position_lots l ON l.product_id = p.id AND l.account_id = a.id AND l.status = 'active'
             WHERE v.is_current = 1 AND (v.market_value > 0.000001 OR l.id IS NOT NULL)
             GROUP BY v.id, p.id, a.id, p.name, p.code, v.currency, a.institution, v.market_value, v.valuation_date
             ORDER BY v.currency, v.market_value DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let cost: f64 = row.get(7)?;
            let market_value: f64 = row.get(8)?;
            let gain = market_value - cost;
            Ok(Holding {
                id: row.get(0)?,
                product_id: row.get(1)?,
                account_id: row.get(2)?,
                name: row.get(3)?,
                code: row.get(4)?,
                currency: row.get(5)?,
                channel: row.get(6)?,
                cost,
                market_value,
                gain,
                gain_rate: if cost.abs() > f64::EPSILON {
                    gain / cost
                } else {
                    0.0
                },
                valuation_date: row.get(9)?,
                status: "持有中".to_string(),
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_maturities(state: State<'_, AppState>) -> Result<Vec<MaturityEvent>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let today = Local::now().date_naive();
    let mut statement = conn
        .prepare(
            "SELECT d.id, d.name, a.institution, d.currency, d.principal, d.maturity_date, d.annual_rate
             FROM deposits d JOIN accounts a ON a.id = d.account_id
             WHERE d.status = 'active'
             ORDER BY d.maturity_date",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let maturity_date: String = row.get(5)?;
            let parsed = NaiveDate::parse_from_str(&maturity_date, "%Y-%m-%d").unwrap_or(today);
            Ok(MaturityEvent {
                id: row.get(0)?,
                name: row.get(1)?,
                institution: row.get(2)?,
                currency: row.get(3)?,
                amount: row.get(4)?,
                maturity_date,
                days_remaining: (parsed - today).num_days(),
                annual_rate: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_entry(input: EntryInput, state: State<'_, AppState>) -> Result<EntryResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    apply_entry(&mut conn, &input)
}

#[tauri::command]
fn list_transactions(state: State<'_, AppState>) -> Result<Vec<TransactionRecord>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT t.id,
                    COALESCE(p.name, d.name, '未命名交易') AS title,
                    p.code,
                    t.transaction_type,
                    t.trade_date,
                    t.amount,
                    COALESCE(t.cost_basis, 0),
                    COALESCE(t.realized_gain, 0),
                    t.currency,
                    t.note
             FROM transactions t
             LEFT JOIN products p ON p.id = t.product_id
             LEFT JOIN deposits d ON d.id = t.deposit_id
             ORDER BY t.trade_date DESC, t.id DESC
             LIMIT 500",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(TransactionRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                code: row.get(2)?,
                operation: row.get(3)?,
                trade_date: row.get(4)?,
                amount: row.get(5)?,
                cost_basis: row.get(6)?,
                realized_gain: row.get(7)?,
                currency: row.get(8)?,
                note: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let mut conn = Connection::open(data_dir.join("wealth-manager.sqlite3"))?;
            migrate(&conn)?;
            if let Ok(import_path) = std::env::var("WEALTH_MANAGER_IMPORT_PATH") {
                let imported: i64 =
                    conn.query_row("SELECT COUNT(*) FROM import_runs", [], |row| row.get(0))?;
                if imported == 0 {
                    let path = Path::new(&import_path);
                    if path.exists() {
                        if let Err(error) = parse_workbook(path, &mut conn) {
                            eprintln!("development import failed: {error}");
                        }
                    }
                }
            }
            app.manage(AppState {
                db: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_workbook,
            get_dashboard,
            list_holdings,
            list_maturities,
            record_entry,
            list_transactions
        ])
        .run(tauri::generate_context!())
        .expect("failed to run wealth manager");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_can_be_applied_twice() {
        let conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("first migration");
        migrate(&conn).expect("second migration");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("count tables");
        assert!(count >= 7);
    }

    #[test]
    fn legacy_schema_is_upgraded_without_losing_rows() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            "CREATE TABLE transactions (
               id INTEGER PRIMARY KEY, transaction_type TEXT NOT NULL, trade_date TEXT NOT NULL,
               amount REAL NOT NULL, currency TEXT NOT NULL, source TEXT NOT NULL DEFAULT 'manual'
             );
             CREATE TABLE position_lots (
               id INTEGER PRIMARY KEY, product_id INTEGER NOT NULL, account_id INTEGER NOT NULL,
               purchase_date TEXT NOT NULL, original_amount REAL NOT NULL, status TEXT NOT NULL
             );
             CREATE TABLE deposits (
               id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL, name TEXT NOT NULL,
               currency TEXT NOT NULL, principal REAL NOT NULL, maturity_date TEXT NOT NULL,
               annual_rate REAL NOT NULL
             );
             INSERT INTO position_lots
               (product_id, account_id, purchase_date, original_amount, status)
             VALUES (1, 1, '2026-01-01', 1234, 'active');",
        )
        .expect("create legacy schema");
        migrate(&conn).expect("upgrade legacy schema");
        let remaining: f64 = conn
            .query_row("SELECT remaining_amount FROM position_lots", [], |row| {
                row.get(0)
            })
            .expect("migrated remaining amount");
        assert_eq!(remaining, 1234.0);
        let added_columns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('deposits')
                 WHERE name IN ('status', 'matured_at', 'proceeds')",
                [],
                |row| row.get(0),
            )
            .expect("query upgraded deposit columns");
        assert_eq!(added_columns, 3);
    }

    #[test]
    fn excel_serial_date_uses_excel_epoch() {
        let value = Data::Float(46241.0);
        assert_eq!(excel_date(Some(&value)).unwrap().to_string(), "2026-08-07");
    }

    #[test]
    fn imports_reference_workbook() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        let report = parse_workbook(&workbook, &mut conn).expect("import workbook");
        assert_eq!(report.lots_imported, 51);
        assert_eq!(report.holdings_imported, 21);
        assert_eq!(report.deposits_imported, 4);
        let cny_market: f64 = conn
            .query_row(
                "SELECT SUM(market_value) FROM valuations WHERE currency = 'CNY'",
                [],
                |row| row.get(0),
            )
            .expect("CNY market value");
        assert!((cny_market - 145_172.0).abs() < 0.01);
        let valuation_date: String = conn
            .query_row("SELECT MIN(valuation_date) FROM valuations", [], |row| {
                row.get(0)
            })
            .expect("valuation date");
        assert_eq!(valuation_date, "2026-08-07");
    }

    fn entry(operation: &str, amount: f64) -> EntryInput {
        EntryInput {
            operation: operation.to_string(),
            product_id: None,
            account_id: None,
            deposit_id: None,
            product_name: Some("测试理财".to_string()),
            product_code: Some("TEST001".to_string()),
            institution: Some("测试银行".to_string()),
            currency: Some("CNY".to_string()),
            amount: Some(amount),
            market_value: None,
            trade_date: "2026-08-07".to_string(),
            maturity_date: None,
            annual_rate: None,
            note: None,
        }
    }

    #[test]
    fn multiple_buys_and_sells_use_fifo_and_keep_remaining_cost() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        apply_entry(&mut conn, &entry("BUY", 10_000.0)).expect("first buy");
        apply_entry(&mut conn, &entry("BUY", 5_000.0)).expect("second buy");

        let (product, account): (i64, i64) = conn
            .query_row(
                "SELECT product_id, account_id FROM position_lots LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("holding identifiers");
        let mut valuation = entry("VALUATION", 0.0);
        valuation.product_id = Some(product);
        valuation.account_id = Some(account);
        valuation.market_value = Some(16_500.0);
        apply_entry(&mut conn, &valuation).expect("valuation");

        let mut first_sell = entry("SELL", 5_500.0);
        first_sell.product_id = Some(product);
        first_sell.account_id = Some(account);
        let first = apply_entry(&mut conn, &first_sell).expect("first sell");
        assert!((first.realized_gain.unwrap_or_default() - 500.0).abs() < 0.01);
        let remaining = conn
            .query_row(
                "SELECT SUM(remaining_amount) FROM position_lots WHERE status = 'active'",
                [],
                |row| row.get::<_, f64>(0),
            )
            .expect("remaining cost after first sell");
        assert!((remaining - 10_000.0).abs() < 0.01);

        let mut second_sell = entry("SELL", 5_500.0);
        second_sell.product_id = Some(product);
        second_sell.account_id = Some(account);
        let second = apply_entry(&mut conn, &second_sell).expect("second sell");
        assert!((second.realized_gain.unwrap_or_default() - 500.0).abs() < 0.01);
        let remaining = conn
            .query_row(
                "SELECT SUM(remaining_amount) FROM position_lots WHERE status = 'active'",
                [],
                |row| row.get::<_, f64>(0),
            )
            .expect("remaining cost after second sell");
        assert!((remaining - 5_000.0).abs() < 0.01);

        let mut maturity = entry("PRODUCT_MATURITY", 5_500.0);
        maturity.product_id = Some(product);
        maturity.account_id = Some(account);
        let final_result = apply_entry(&mut conn, &maturity).expect("maturity");
        assert!((final_result.realized_gain.unwrap_or_default() - 500.0).abs() < 0.01);
        let active_lots: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM position_lots WHERE status = 'active'",
                [],
                |row| row.get(0),
            )
            .expect("active lots");
        assert_eq!(active_lots, 0);
    }

    #[test]
    fn reimport_preserves_manual_sale_and_manual_valuation() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("first import");

        let (product, account, market): (i64, i64, f64) = conn
            .query_row(
                "SELECT v.product_id, v.account_id, v.market_value
                 FROM valuations v
                 WHERE v.is_current = 1 AND v.market_value > 1000
                   AND EXISTS (
                     SELECT 1 FROM position_lots l
                     WHERE l.product_id = v.product_id AND l.account_id = v.account_id
                       AND l.status = 'active'
                   )
                 LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("imported holding");
        let mut sale = entry("SELL", market / 10.0);
        sale.product_id = Some(product);
        sale.account_id = Some(account);
        apply_entry(&mut conn, &sale).expect("manual sale");
        let cost_after_sale: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(remaining_amount), 0) FROM position_lots
                 WHERE product_id = ?1 AND account_id = ?2 AND status = 'active'",
                params![product, account],
                |row| row.get(0),
            )
            .expect("cost after sale");

        parse_workbook(&workbook, &mut conn).expect("second import");
        let cost_after_reimport: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(remaining_amount), 0) FROM position_lots
                 WHERE product_id = ?1 AND account_id = ?2 AND status = 'active'",
                params![product, account],
                |row| row.get(0),
            )
            .expect("cost after reimport");
        assert!((cost_after_reimport - cost_after_sale).abs() < 0.01);
        let current_valuations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM valuations
                 WHERE product_id = ?1 AND account_id = ?2 AND is_current = 1",
                params![product, account],
                |row| row.get(0),
            )
            .expect("single current valuation");
        assert_eq!(current_valuations, 1);
    }

    #[test]
    fn deposit_maturity_splits_principal_and_interest() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        let mut open = entry("DEPOSIT_OPEN", 100_000.0);
        open.product_name = Some("三年定期".to_string());
        open.maturity_date = Some("2029-08-07".to_string());
        open.annual_rate = Some(0.02);
        apply_entry(&mut conn, &open).expect("open deposit");
        let deposit: i64 = conn
            .query_row("SELECT id FROM deposits", [], |row| row.get(0))
            .expect("deposit id");

        let mut maturity = entry("DEPOSIT_MATURITY", 106_000.0);
        maturity.deposit_id = Some(deposit);
        let result = apply_entry(&mut conn, &maturity).expect("mature deposit");
        assert!((result.realized_gain.unwrap_or_default() - 6_000.0).abs() < 0.01);
        let status: String = conn
            .query_row(
                "SELECT status FROM deposits WHERE id = ?1",
                [deposit],
                |row| row.get(0),
            )
            .expect("matured status");
        assert_eq!(status, "matured");
    }
}
