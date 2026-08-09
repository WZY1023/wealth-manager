use calamine::{open_workbook_auto, Data, DataType, Reader};
use chrono::{Datelike, Duration, Local, Months, NaiveDate};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction, MAIN_DB};
use rust_xlsxwriter::{
    Color, ExcelDateTime, Format, FormatAlign, FormatBorder, Formula, Workbook, Worksheet,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{Manager, State};

struct AppState {
    db: Mutex<Connection>,
    backup_dir: PathBuf,
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
    days_since_valuation: i64,
    seven_day_return: Option<f64>,
    thirty_day_return: Option<f64>,
    signal: String,
    signal_label: String,
    signal_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValuationUpdateItem {
    product_id: i64,
    account_id: i64,
    market_value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchValuationInput {
    valuation_date: String,
    items: Vec<ValuationUpdateItem>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchValuationResult {
    updated: usize,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValuationHistoryPoint {
    id: i64,
    date: String,
    market_value: f64,
    change_amount: Option<f64>,
    change_rate: Option<f64>,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HoldingDetail {
    product_id: i64,
    account_id: i64,
    name: String,
    code: String,
    channel: String,
    currency: String,
    cost: f64,
    market_value: f64,
    gain: f64,
    gain_rate: f64,
    valuation_date: String,
    days_since_valuation: i64,
    seven_day_return: Option<f64>,
    thirty_day_return: Option<f64>,
    signal: String,
    signal_label: String,
    signal_reason: String,
    history: Vec<ValuationHistoryPoint>,
}

struct HoldingInsight {
    days_since_valuation: i64,
    seven_day_return: Option<f64>,
    thirty_day_return: Option<f64>,
    signal: String,
    signal_label: String,
    signal_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryInput {
    operation: String,
    product_id: Option<i64>,
    account_id: Option<i64>,
    transfer_account_id: Option<i64>,
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
struct AccountRecord {
    id: i64,
    institution: String,
    name: String,
    currency: String,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountReconciliation {
    account_id: i64,
    institution: String,
    name: String,
    currency: String,
    wealth_value: f64,
    deposit_value: f64,
    tracked_total: f64,
    actual_balance: Option<f64>,
    difference: Option<f64>,
    balance_date: Option<String>,
    note: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BalanceSnapshotInput {
    account_id: i64,
    balance_date: String,
    actual_balance: f64,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoricalRedemptionInput {
    transaction_id: i64,
    trade_date: String,
    proceeds: f64,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransactionEditInput {
    transaction_id: i64,
    trade_date: String,
    amount: f64,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QualityIssue {
    key: String,
    severity: String,
    category: String,
    title: String,
    detail: String,
    target_view: String,
    target_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReconciliationCenter {
    accounts: Vec<AccountReconciliation>,
    issues: Vec<QualityIssue>,
    issue_count: usize,
    high_priority_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductRecord {
    id: i64,
    code: String,
    name: String,
    currency: String,
    issuer: Option<String>,
    risk_level: Option<String>,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DepositRecord {
    id: i64,
    account_id: i64,
    institution: String,
    name: String,
    currency: String,
    principal: f64,
    start_date: Option<String>,
    maturity_date: String,
    annual_rate: f64,
    status: String,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MasterData {
    accounts: Vec<AccountRecord>,
    products: Vec<ProductRecord>,
    deposits: Vec<DepositRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MasterDataUpdate {
    entity_type: String,
    id: i64,
    name: String,
    code: Option<String>,
    institution: Option<String>,
    account_id: Option<i64>,
    currency: String,
    issuer: Option<String>,
    risk_level: Option<String>,
    principal: Option<f64>,
    start_date: Option<String>,
    maturity_date: Option<String>,
    annual_rate: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrendPoint {
    date: String,
    label: String,
    asset_value: f64,
    cumulative_realized_gain: f64,
    net_cash_flow: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrencyAnalytics {
    currency: String,
    xirr: Option<f64>,
    current_value: f64,
    unrealized_gain: f64,
    realized_gain: f64,
    income: f64,
    fees: f64,
    trend: Vec<TrendPoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Analytics {
    as_of_date: String,
    currencies: Vec<CurrencyAnalytics>,
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
    institution: Option<String>,
    operation: String,
    trade_date: String,
    amount: f64,
    cost_basis: f64,
    realized_gain: f64,
    currency: String,
    note: Option<String>,
    source: String,
    reversed_by: Option<i64>,
    reversal_of: Option<i64>,
    can_reverse: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransactionDetail {
    id: i64,
    title: String,
    code: Option<String>,
    institution: Option<String>,
    operation: String,
    trade_date: String,
    amount: f64,
    cost_basis: f64,
    realized_gain: f64,
    currency: String,
    note: Option<String>,
    source: String,
    valuation_before: Option<f64>,
    valuation_after: Option<f64>,
    reversed_by: Option<i64>,
    reversal_of: Option<i64>,
    can_reverse: bool,
    can_edit: bool,
    edit_hint: Option<String>,
    can_confirm_historical_redemption: bool,
    needs_review: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileOperationResult {
    path: String,
    message: String,
}

struct ReversalIdentity {
    operation: String,
    product_id: Option<i64>,
    account_id: Option<i64>,
    transfer_account_id: Option<i64>,
    deposit_id: Option<i64>,
    reversed_by: Option<i64>,
    source: String,
}

struct ReversalTarget {
    operation: String,
    product_id: Option<i64>,
    account_id: Option<i64>,
    transfer_account_id: Option<i64>,
    deposit_id: Option<i64>,
    amount: f64,
    currency: String,
    cost_basis: f64,
    realized_gain: f64,
    valuation_before: Option<f64>,
    valuation_after: Option<f64>,
}

struct HistoricalRedemptionRecord {
    source_row: i64,
    product_code: String,
    institution: String,
    currency: String,
    original_cost: f64,
    original_end_date: String,
    trade_date: String,
    amount: f64,
    cost_basis: f64,
    realized_gain: f64,
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
          import_institution TEXT,
          is_user_edited INTEGER NOT NULL DEFAULT 0,
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
          import_code TEXT,
          is_user_edited INTEGER NOT NULL DEFAULT 0,
          source TEXT NOT NULL DEFAULT 'manual'
        );

        CREATE TABLE IF NOT EXISTS transactions (
          id INTEGER PRIMARY KEY,
          product_id INTEGER,
          account_id INTEGER,
          transfer_account_id INTEGER,
          transaction_type TEXT NOT NULL,
          trade_date TEXT NOT NULL,
          amount REAL NOT NULL,
          currency TEXT NOT NULL,
          note TEXT,
          cost_basis REAL NOT NULL DEFAULT 0,
          realized_gain REAL NOT NULL DEFAULT 0,
          deposit_id INTEGER,
          valuation_before REAL,
          valuation_after REAL,
          reversal_of INTEGER,
          reversed_by INTEGER,
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(account_id) REFERENCES accounts(id),
          FOREIGN KEY(transfer_account_id) REFERENCES accounts(id),
          FOREIGN KEY(deposit_id) REFERENCES deposits(id),
          FOREIGN KEY(reversal_of) REFERENCES transactions(id),
          FOREIGN KEY(reversed_by) REFERENCES transactions(id)
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
          transaction_id INTEGER,
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(account_id) REFERENCES accounts(id),
          FOREIGN KEY(transaction_id) REFERENCES transactions(id)
        );

        CREATE TABLE IF NOT EXISTS valuations (
          id INTEGER PRIMARY KEY,
          product_id INTEGER NOT NULL,
          account_id INTEGER NOT NULL,
          valuation_date TEXT NOT NULL,
          market_value REAL NOT NULL,
          currency TEXT NOT NULL,
          is_current INTEGER NOT NULL DEFAULT 1,
          transaction_id INTEGER,
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(account_id) REFERENCES accounts(id),
          FOREIGN KEY(transaction_id) REFERENCES transactions(id)
        );

        CREATE TABLE IF NOT EXISTS transaction_lot_allocations (
          id INTEGER PRIMARY KEY,
          transaction_id INTEGER NOT NULL,
          lot_id INTEGER NOT NULL,
          allocated_cost REAL NOT NULL,
          FOREIGN KEY(transaction_id) REFERENCES transactions(id) ON DELETE CASCADE,
          FOREIGN KEY(lot_id) REFERENCES position_lots(id) ON DELETE CASCADE,
          UNIQUE(transaction_id, lot_id)
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
          is_user_edited INTEGER NOT NULL DEFAULT 0,
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

        CREATE TABLE IF NOT EXISTS master_data_changes (
          id INTEGER PRIMARY KEY,
          entity_type TEXT NOT NULL,
          entity_id INTEGER NOT NULL,
          changed_at TEXT NOT NULL,
          before_json TEXT NOT NULL,
          after_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS account_balance_snapshots (
          id INTEGER PRIMARY KEY,
          account_id INTEGER NOT NULL,
          balance_date TEXT NOT NULL,
          actual_balance REAL NOT NULL,
          note TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(account_id) REFERENCES accounts(id),
          UNIQUE(account_id, balance_date)
        );

        CREATE TABLE IF NOT EXISTS historical_redemption_confirmations (
          id INTEGER PRIMARY KEY,
          redemption_key TEXT NOT NULL UNIQUE,
          source_row INTEGER,
          product_code TEXT NOT NULL,
          institution TEXT NOT NULL,
          currency TEXT NOT NULL,
          original_cost REAL NOT NULL,
          original_end_date TEXT NOT NULL,
          confirmed_trade_date TEXT NOT NULL,
          proceeds REAL NOT NULL,
          note TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS excel_transaction_corrections (
          id INTEGER PRIMARY KEY,
          correction_key TEXT NOT NULL UNIQUE,
          source_row INTEGER NOT NULL,
          operation TEXT NOT NULL,
          trade_date TEXT NOT NULL,
          amount REAL NOT NULL,
          note TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
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
    ensure_column(conn, "transactions", "product_id", "INTEGER")?;
    ensure_column(conn, "transactions", "account_id", "INTEGER")?;
    ensure_column(conn, "transactions", "source_row", "INTEGER")?;
    ensure_column(conn, "transactions", "transfer_account_id", "INTEGER")?;
    ensure_column(conn, "transactions", "valuation_before", "REAL")?;
    ensure_column(conn, "transactions", "valuation_after", "REAL")?;
    ensure_column(conn, "transactions", "reversal_of", "INTEGER")?;
    ensure_column(conn, "transactions", "reversed_by", "INTEGER")?;
    ensure_column(conn, "position_lots", "remaining_amount", "REAL")?;
    ensure_column(conn, "position_lots", "transaction_id", "INTEGER")?;
    ensure_column(conn, "position_lots", "end_date", "TEXT")?;
    ensure_column(conn, "position_lots", "source_row", "INTEGER")?;
    ensure_column(
        conn,
        "position_lots",
        "source",
        "TEXT NOT NULL DEFAULT 'manual'",
    )?;
    ensure_column(conn, "valuations", "transaction_id", "INTEGER")?;
    ensure_column(conn, "deposits", "status", "TEXT NOT NULL DEFAULT 'active'")?;
    ensure_column(conn, "deposits", "matured_at", "TEXT")?;
    ensure_column(conn, "deposits", "proceeds", "REAL")?;
    ensure_column(conn, "accounts", "import_institution", "TEXT")?;
    ensure_column(
        conn,
        "accounts",
        "is_user_edited",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(conn, "products", "import_code", "TEXT")?;
    ensure_column(
        conn,
        "products",
        "is_user_edited",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "deposits",
        "is_user_edited",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
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
    conn.execute(
        "UPDATE transactions
         SET cost_basis = COALESCE((
               SELECT l.original_amount FROM position_lots l
               WHERE l.source = 'excel' AND l.source_row = transactions.source_row
                 AND l.product_id = transactions.product_id
                 AND l.account_id = transactions.account_id
               LIMIT 1
             ), (
               SELECT SUM(l.original_amount) FROM position_lots l
               WHERE l.source = 'excel' AND l.status = 'closed'
                 AND l.product_id = transactions.product_id
                 AND l.account_id = transactions.account_id
             ), cost_basis),
             realized_gain = amount - COALESCE((
               SELECT l.original_amount FROM position_lots l
               WHERE l.source = 'excel' AND l.source_row = transactions.source_row
                 AND l.product_id = transactions.product_id
                 AND l.account_id = transactions.account_id
               LIMIT 1
             ), (
               SELECT SUM(l.original_amount) FROM position_lots l
               WHERE l.source = 'excel' AND l.status = 'closed'
                 AND l.product_id = transactions.product_id
                 AND l.account_id = transactions.account_id
             ), cost_basis)
         WHERE source = 'excel' AND transaction_type = 'REDEEM'
           AND COALESCE(cost_basis, 0) <= 0.000001",
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

fn historical_redemption_key(
    code: &str,
    institution: &str,
    currency: &str,
    original_cost: f64,
    end_date: &str,
) -> String {
    format!(
        "{}|{}|{}|{:.6}|{}",
        code.trim(),
        institution.trim(),
        currency.trim().to_uppercase(),
        original_cost,
        end_date
    )
}

fn excel_transaction_correction_key(operation: &str, source_row: i64) -> String {
    format!("sheet1|{}|{source_row}", operation.trim().to_uppercase())
}

fn account_id(tx: &Transaction<'_>, institution: &str, currency: &str) -> rusqlite::Result<i64> {
    let existing = tx
        .query_row(
            "SELECT id FROM accounts
             WHERE currency = ?2 AND (
               (institution = ?1 AND name = ?1) OR import_institution = ?1
             ) ORDER BY is_user_edited DESC LIMIT 1",
            params![institution, currency],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO accounts (institution, name, currency, import_institution, source)
         VALUES (?1, ?1, ?2, ?1, 'excel')",
        params![institution, currency],
    )?;
    Ok(tx.last_insert_rowid())
}

fn product_id(
    tx: &Transaction<'_>,
    code: &str,
    name: &str,
    currency: &str,
) -> rusqlite::Result<i64> {
    let existing: Option<(i64, bool)> = tx
        .query_row(
            "SELECT id, is_user_edited != 0 FROM products
             WHERE code = ?1 OR import_code = ?1
             ORDER BY is_user_edited DESC LIMIT 1",
            params![code],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((id, edited)) = existing {
        if !edited {
            tx.execute(
                "UPDATE products SET name = ?1 WHERE id = ?2",
                params![name, id],
            )?;
        }
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO products (code, name, currency, import_code, source)
         VALUES (?1, ?2, ?3, ?1, 'excel')",
        params![code, name, currency],
    )?;
    Ok(tx.last_insert_rowid())
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
    transaction_id: Option<i64>,
) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE valuations SET is_current = 0 WHERE product_id = ?1 AND account_id = ?2 AND is_current = 1",
        params![product, account],
    )?;
    tx.execute(
        "INSERT INTO valuations (product_id, account_id, valuation_date, market_value, currency, is_current, transaction_id, source)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, 'manual')",
        params![product, account, date, value.max(0.0), currency, transaction_id],
    )?;
    Ok(())
}

fn compounded_valuation_return(
    conn: &Connection,
    product: i64,
    account: i64,
    since: NaiveDate,
) -> Result<Option<f64>, String> {
    let mut statement = conn
        .prepare(
            "SELECT valuation_before, valuation_after
             FROM transactions
             WHERE product_id = ?1 AND account_id = ?2
               AND transaction_type = 'VALUATION' AND reversed_by IS NULL
               AND trade_date >= ?3 AND valuation_before > 0.000001
             ORDER BY trade_date, id",
        )
        .map_err(|error| error.to_string())?;
    let changes = statement
        .query_map(
            params![product, account, since.format("%Y-%m-%d").to_string()],
            |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
        )
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if changes.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        changes
            .iter()
            .fold(1.0, |factor, (before, after)| factor * (after / before))
            - 1.0,
    ))
}

fn holding_insight(
    conn: &Connection,
    product: i64,
    account: i64,
    currency: &str,
    market_value: f64,
    gain_rate: f64,
    valuation_date: &str,
) -> Result<HoldingInsight, String> {
    let today = Local::now().date_naive();
    let valued_on = NaiveDate::parse_from_str(valuation_date, "%Y-%m-%d").unwrap_or(today);
    let days_since_valuation = (today - valued_on).num_days().max(0);
    let seven_day_return = compounded_valuation_return(
        conn,
        product,
        account,
        today.checked_sub_signed(Duration::days(7)).unwrap_or(today),
    )?;
    let thirty_day_return = compounded_valuation_return(
        conn,
        product,
        account,
        today
            .checked_sub_signed(Duration::days(30))
            .unwrap_or(today),
    )?;
    let observations: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions
             WHERE product_id = ?1 AND account_id = ?2
               AND transaction_type = 'VALUATION' AND reversed_by IS NULL",
            params![product, account],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let risk_level: Option<String> = conn
        .query_row(
            "SELECT risk_level FROM products WHERE id = ?1",
            [product],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .flatten();
    let currency_assets: f64 = conn
        .query_row(
            "SELECT
               (SELECT COALESCE(SUM(market_value), 0) FROM valuations
                WHERE is_current = 1 AND currency = ?1) +
               (SELECT COALESCE(SUM(principal), 0) FROM deposits
                WHERE status = 'active' AND currency = ?1)",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let exposure = if currency_assets > 0.000_001 {
        market_value / currency_assets
    } else {
        0.0
    };
    let low_risk = risk_level
        .as_deref()
        .map(|value| {
            let normalized = value.trim().to_uppercase();
            matches!(normalized.as_str(), "R1" | "R2")
                || normalized.contains("低风险")
                || normalized.contains("中低风险")
        })
        .unwrap_or(false);
    let thirty = thirty_day_return.unwrap_or_default();
    let seven = seven_day_return.unwrap_or_default();
    let (signal, signal_label, signal_reason) = if days_since_valuation > 10 {
        (
            "UPDATE",
            "待更新",
            format!("市值已 {days_since_valuation} 天未更新，先补充最新数据再判断"),
        )
    } else if observations < 2 {
        (
            "OBSERVE",
            "积累数据",
            "有效估值次数不足 2 次，暂不根据单点数据给出买卖方向".to_string(),
        )
    } else if thirty <= -0.005 || gain_rate <= -0.015 {
        (
            "REVIEW",
            "评估赎回",
            format!(
                "近30日变化 {}、累计收益 {}，建议核对风险、期限和赎回费用",
                format_percent_for_reason(thirty),
                format_percent_for_reason(gain_rate)
            ),
        )
    } else if gain_rate >= 0.03 && seven < 0.0 && thirty < 0.0 {
        (
            "TAKE_PROFIT",
            "考虑止盈",
            format!(
                "累计收益 {}，但近7日转弱至 {}，可评估分批锁定收益",
                format_percent_for_reason(gain_rate),
                format_percent_for_reason(seven)
            ),
        )
    } else if low_risk
        && observations >= 3
        && (0.001..=0.02).contains(&thirty)
        && gain_rate > 0.0
        && exposure < 0.25
    {
        (
            "ADD_WATCH",
            "关注加仓",
            format!(
                "近30日平稳为正且当前占比约 {:.0}%；加仓前仍需核对流动性与资金期限",
                exposure * 100.0
            ),
        )
    } else {
        (
            "HOLD",
            "继续观察",
            format!(
                "近30日变化 {}，尚未触发回撤、止盈或低风险加仓观察条件",
                format_percent_for_reason(thirty)
            ),
        )
    };
    Ok(HoldingInsight {
        days_since_valuation,
        seven_day_return,
        thirty_day_return,
        signal: signal.to_string(),
        signal_label: signal_label.to_string(),
        signal_reason,
    })
}

fn format_percent_for_reason(value: f64) -> String {
    format!("{:+.2}%", value * 100.0)
}

fn apply_batch_valuations(
    conn: &mut Connection,
    input: &BatchValuationInput,
) -> Result<BatchValuationResult, String> {
    let valuation_date = valid_date(&input.valuation_date, "估值日期")?;
    if input.items.is_empty() {
        return Err("请至少提供一笔市值".to_string());
    }
    let mut keys = std::collections::HashSet::new();
    for item in &input.items {
        if !item.market_value.is_finite() || item.market_value < 0.0 {
            return Err("市值必须是大于或等于0的有效数字".to_string());
        }
        if !keys.insert((item.product_id, item.account_id)) {
            return Err("批量市值中存在重复持仓".to_string());
        }
    }
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    for item in &input.items {
        let (currency, name): (String, String) = tx
            .query_row(
                "SELECT p.currency, p.name FROM products p
                 WHERE p.id = ?1 AND EXISTS (
                   SELECT 1 FROM position_lots l
                   WHERE l.product_id = p.id AND l.account_id = ?2 AND l.status = 'active'
                 )",
                params![item.product_id, item.account_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| "批量列表中包含已经结清或不存在的持仓".to_string())?;
        let previous = current_market_value(&tx, item.product_id, item.account_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("{name} 缺少当前市值"))?;
        tx.execute(
            "INSERT INTO transactions
               (product_id, account_id, transaction_type, trade_date, amount, currency, note,
                valuation_before, valuation_after, source)
             VALUES (?1, ?2, 'VALUATION', ?3, ?4, ?5, ?6, ?7, ?4, 'manual')",
            params![
                item.product_id,
                item.account_id,
                valuation_date,
                item.market_value,
                currency,
                input
                    .note
                    .as_deref()
                    .filter(|note| !note.trim().is_empty())
                    .unwrap_or("批量更新市值"),
                previous
            ],
        )
        .map_err(|error| error.to_string())?;
        let transaction_id = tx.last_insert_rowid();
        replace_current_valuation(
            &tx,
            item.product_id,
            item.account_id,
            &valuation_date,
            item.market_value,
            &currency,
            Some(transaction_id),
        )
        .map_err(|error| error.to_string())?;
    }
    tx.commit().map_err(|error| error.to_string())?;
    Ok(BatchValuationResult {
        updated: input.items.len(),
        message: format!("已更新 {} 个持仓的市值", input.items.len()),
    })
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
) -> rusqlite::Result<Vec<(i64, f64)>> {
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
    let mut allocations = Vec::new();
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
        allocations.push((lot_id, consumed));
        cost_to_remove -= consumed;
    }
    Ok(allocations)
}

fn save_allocations(
    tx: &Transaction<'_>,
    transaction_id: i64,
    allocations: &[(i64, f64)],
) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM transaction_lot_allocations WHERE transaction_id = ?1",
        [transaction_id],
    )?;
    for (lot_id, allocated_cost) in allocations {
        tx.execute(
            "INSERT INTO transaction_lot_allocations (transaction_id, lot_id, allocated_cost)
             VALUES (?1, ?2, ?3)",
            params![transaction_id, lot_id, allocated_cost],
        )?;
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
    tx.execute(
        "UPDATE position_lots
         SET remaining_amount = 0, status = 'reversed'
         WHERE transaction_id IN (
           SELECT id FROM transactions WHERE reversed_by IS NOT NULL
         )",
        [],
    )?;
    let exits = {
        let mut statement = tx.prepare(
            "SELECT id, product_id, account_id, trade_date, cost_basis
             FROM transactions
             WHERE source = 'manual'
               AND transaction_type IN ('SELL', 'PRODUCT_MATURITY')
               AND reversed_by IS NULL
               AND product_id IS NOT NULL AND account_id IS NOT NULL
             ORDER BY trade_date, id",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, i64>(0)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for (product, account, date, cost_basis, transaction_id) in exits {
        let allocations = consume_fifo_cost(tx, product, account, &date, cost_basis)?;
        save_allocations(tx, transaction_id, &allocations)?;
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
            let previous = current_market_value(&tx, product, account)
                .map_err(|error| error.to_string())?
                .unwrap_or_default();
            let next_market = previous + amount;
            tx.execute(
                "INSERT INTO transactions
                   (product_id, account_id, transaction_type, trade_date, amount, currency, note,
                    cost_basis, valuation_before, valuation_after, source)
                 VALUES (?1, ?2, 'BUY', ?3, ?4, ?5, ?6, ?4, ?7, ?8, 'manual')",
                params![
                    product,
                    account,
                    trade_date,
                    amount,
                    currency,
                    input.note,
                    previous,
                    next_market
                ],
            )
            .map_err(|error| error.to_string())?;
            let transaction_id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO position_lots
                   (product_id, account_id, purchase_date, original_amount, remaining_amount,
                    status, transaction_id, source)
                 VALUES (?1, ?2, ?3, ?4, ?4, 'active', ?5, 'manual')",
                params![product, account, trade_date, amount, transaction_id],
            )
            .map_err(|error| error.to_string())?;
            replace_current_valuation(
                &tx,
                product,
                account,
                &trade_date,
                next_market,
                &currency,
                Some(transaction_id),
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
            let realized_gain = proceeds - cost_basis;
            let next_market = if full {
                0.0
            } else {
                (market - proceeds).max(0.0)
            };
            tx.execute(
                "INSERT INTO transactions
                   (product_id, account_id, transaction_type, trade_date, amount, currency, note,
                    cost_basis, realized_gain, valuation_before, valuation_after, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'manual')",
                params![
                    product,
                    account,
                    if operation == "PRODUCT_MATURITY" {
                        "PRODUCT_MATURITY"
                    } else {
                        "SELL"
                    },
                    trade_date,
                    proceeds,
                    currency,
                    input.note,
                    cost_basis,
                    realized_gain,
                    market,
                    next_market
                ],
            )
            .map_err(|error| error.to_string())?;
            let transaction_id = tx.last_insert_rowid();
            let allocations = consume_fifo_cost(&tx, product, account, &trade_date, cost_basis)
                .map_err(|error| error.to_string())?;
            save_allocations(&tx, transaction_id, &allocations)
                .map_err(|error| error.to_string())?;
            replace_current_valuation(
                &tx,
                product,
                account,
                &trade_date,
                next_market,
                &currency,
                Some(transaction_id),
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
            let previous = current_market_value(&tx, product, account)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "没有找到该产品的当前市值".to_string())?;
            tx.execute(
                "INSERT INTO transactions
                   (product_id, account_id, transaction_type, trade_date, amount, currency, note,
                    valuation_before, valuation_after, source)
                 VALUES (?1, ?2, 'VALUATION', ?3, ?4, ?5, ?6, ?7, ?4, 'manual')",
                params![product, account, trade_date, value, currency, input.note, previous],
            )
            .map_err(|error| error.to_string())?;
            let transaction_id = tx.last_insert_rowid();
            replace_current_valuation(
                &tx,
                product,
                account,
                &trade_date,
                value,
                &currency,
                Some(transaction_id),
            )
            .map_err(|error| error.to_string())?;
            EntryResult {
                message: "当前市值已更新，旧市值已作为历史快照保留".to_string(),
                realized_gain: None,
            }
        }
        "DIVIDEND" | "FEE" => {
            let amount = positive_amount(
                input.amount,
                if operation == "DIVIDEND" {
                    "分红金额"
                } else {
                    "费用金额"
                },
            )?;
            let product = input
                .product_id
                .ok_or_else(|| "请选择理财产品".to_string())?;
            let account = input
                .account_id
                .ok_or_else(|| "请选择购买渠道".to_string())?;
            let currency = holding_currency(&tx, product, account)?;
            let realized_gain = if operation == "DIVIDEND" {
                amount
            } else {
                -amount
            };
            tx.execute(
                "INSERT INTO transactions
                   (product_id, account_id, transaction_type, trade_date, amount, currency,
                    note, realized_gain, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'manual')",
                params![
                    product,
                    account,
                    operation,
                    trade_date,
                    amount,
                    currency,
                    input.note,
                    realized_gain
                ],
            )
            .map_err(|error| error.to_string())?;
            EntryResult {
                message: if operation == "DIVIDEND" {
                    "分红已计入已实现收益，不改变持仓成本和市值".to_string()
                } else {
                    "费用已计入收益扣减，不改变持仓成本和市值".to_string()
                },
                realized_gain: Some(realized_gain),
            }
        }
        "TRANSFER" => {
            let amount = positive_amount(input.amount, "转账金额")?;
            let source_account = input
                .account_id
                .ok_or_else(|| "请选择转出账户".to_string())?;
            let target_account = input
                .transfer_account_id
                .ok_or_else(|| "请选择转入账户".to_string())?;
            if source_account == target_account {
                return Err("转入账户不能与转出账户相同".to_string());
            }
            let source_currency: String = tx
                .query_row(
                    "SELECT currency FROM accounts WHERE id = ?1",
                    [source_account],
                    |row| row.get(0),
                )
                .map_err(|_| "没有找到转出账户".to_string())?;
            let target_currency: String = tx
                .query_row(
                    "SELECT currency FROM accounts WHERE id = ?1",
                    [target_account],
                    |row| row.get(0),
                )
                .map_err(|_| "没有找到转入账户".to_string())?;
            if source_currency != target_currency {
                return Err("当前只支持同币种账户之间转账".to_string());
            }
            tx.execute(
                "INSERT INTO transactions
                   (account_id, transfer_account_id, transaction_type, trade_date, amount,
                    currency, note, source)
                 VALUES (?1, ?2, 'TRANSFER', ?3, ?4, ?5, ?6, 'manual')",
                params![
                    source_account,
                    target_account,
                    trade_date,
                    amount,
                    source_currency,
                    input.note
                ],
            )
            .map_err(|error| error.to_string())?;
            EntryResult {
                message: "账户转账已记录；内部转移不计入收益和总资产变化".to_string(),
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

fn transaction_can_be_reversed(conn: &Connection, transaction_id: i64) -> Result<bool, String> {
    let target: Option<ReversalIdentity> = conn
        .query_row(
            "SELECT transaction_type, product_id, account_id, transfer_account_id,
                    deposit_id, reversed_by, source
             FROM transactions WHERE id = ?1",
            [transaction_id],
            |row| {
                Ok(ReversalIdentity {
                    operation: row.get(0)?,
                    product_id: row.get(1)?,
                    account_id: row.get(2)?,
                    transfer_account_id: row.get(3)?,
                    deposit_id: row.get(4)?,
                    reversed_by: row.get(5)?,
                    source: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some(ReversalIdentity {
        operation,
        product_id: product,
        account_id: account,
        transfer_account_id: transfer_account,
        deposit_id: deposit,
        reversed_by,
        source,
    }) = target
    else {
        return Ok(false);
    };
    if source != "manual" || operation == "REVERSAL" || reversed_by.is_some() {
        return Ok(false);
    }
    let later: i64 = if let (Some(product), Some(account)) = (product, account) {
        conn.query_row(
            "SELECT COUNT(*) FROM transactions
             WHERE id > ?1 AND source = 'manual' AND reversed_by IS NULL
               AND transaction_type != 'REVERSAL'
               AND product_id = ?2 AND account_id = ?3",
            params![transaction_id, product, account],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?
    } else if let Some(deposit) = deposit {
        conn.query_row(
            "SELECT COUNT(*) FROM transactions
             WHERE id > ?1 AND source = 'manual' AND reversed_by IS NULL
               AND transaction_type != 'REVERSAL' AND deposit_id = ?2",
            params![transaction_id, deposit],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?
    } else if let Some(account) = account {
        conn.query_row(
            "SELECT COUNT(*) FROM transactions
             WHERE id > ?1 AND source = 'manual' AND reversed_by IS NULL
               AND transaction_type != 'REVERSAL'
               AND (account_id = ?2 OR transfer_account_id = ?2)",
            params![transaction_id, account],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?
    } else if transfer_account.is_some() {
        0
    } else {
        1
    };
    if later > 0 {
        return Ok(false);
    }
    if matches!(
        operation.as_str(),
        "BUY" | "SELL" | "PRODUCT_MATURITY" | "VALUATION"
    ) {
        let has_snapshot: bool = conn
            .query_row(
                "SELECT valuation_before IS NOT NULL FROM transactions WHERE id = ?1",
                [transaction_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if !has_snapshot {
            return Ok(false);
        }
    }
    if matches!(operation.as_str(), "SELL" | "PRODUCT_MATURITY") {
        let allocations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transaction_lot_allocations WHERE transaction_id = ?1",
                [transaction_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if allocations == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn reverse_entry(conn: &mut Connection, transaction_id: i64) -> Result<EntryResult, String> {
    if !transaction_can_be_reversed(conn, transaction_id)? {
        return Err("只能撤销该产品或存款最新的一笔可逆手工操作；请先撤销后续操作".to_string());
    }
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let target: ReversalTarget = tx
        .query_row(
            "SELECT transaction_type, product_id, account_id, transfer_account_id, deposit_id, amount,
                    currency, cost_basis, realized_gain, valuation_before, valuation_after
             FROM transactions WHERE id = ?1",
            [transaction_id],
            |row| {
                Ok(ReversalTarget {
                    operation: row.get(0)?,
                    product_id: row.get(1)?,
                    account_id: row.get(2)?,
                    transfer_account_id: row.get(3)?,
                    deposit_id: row.get(4)?,
                    amount: row.get(5)?,
                    currency: row.get(6)?,
                    cost_basis: row.get(7)?,
                    realized_gain: row.get(8)?,
                    valuation_before: row.get(9)?,
                    valuation_after: row.get(10)?,
                })
            },
        )
        .map_err(|error| error.to_string())?;
    let ReversalTarget {
        operation,
        product_id: product,
        account_id: account,
        transfer_account_id: transfer_account,
        deposit_id: deposit,
        amount,
        currency,
        cost_basis,
        realized_gain,
        valuation_before: before,
        valuation_after: after,
    } = target;
    let reversal_date = Local::now().format("%Y-%m-%d").to_string();
    tx.execute(
        "INSERT INTO transactions
           (product_id, account_id, transfer_account_id, deposit_id, transaction_type, trade_date, amount, currency,
            note, cost_basis, realized_gain, valuation_before, valuation_after, reversal_of, source)
         VALUES (?1, ?2, ?3, ?4, 'REVERSAL', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'manual')",
        params![
            product,
            account,
            transfer_account,
            deposit,
            reversal_date,
            amount,
            currency,
            format!("冲销交易 #{transaction_id}（{operation}）"),
            -cost_basis,
            -realized_gain,
            after,
            before,
            transaction_id
        ],
    )
    .map_err(|error| error.to_string())?;
    let reversal_id = tx.last_insert_rowid();

    match operation.as_str() {
        "BUY" => {
            let changed = tx
                .execute(
                    "UPDATE position_lots SET remaining_amount = 0, status = 'reversed'
                     WHERE transaction_id = ?1",
                    [transaction_id],
                )
                .map_err(|error| error.to_string())?;
            if changed == 0 {
                return Err("该买入缺少批次关联，无法安全撤销".to_string());
            }
        }
        "SELL" | "PRODUCT_MATURITY" => {
            let allocations = {
                let mut statement = tx
                    .prepare(
                        "SELECT lot_id, allocated_cost FROM transaction_lot_allocations
                         WHERE transaction_id = ?1",
                    )
                    .map_err(|error| error.to_string())?;
                let rows = statement
                    .query_map([transaction_id], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
                    })
                    .map_err(|error| error.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                rows
            };
            for (lot_id, allocated_cost) in allocations {
                tx.execute(
                    "UPDATE position_lots
                     SET remaining_amount = MIN(original_amount, remaining_amount + ?1), status = 'active'
                     WHERE id = ?2",
                    params![allocated_cost, lot_id],
                )
                .map_err(|error| error.to_string())?;
            }
        }
        "VALUATION" | "DIVIDEND" | "FEE" | "TRANSFER" => {}
        "DEPOSIT_OPEN" => {
            tx.execute(
                "UPDATE deposits SET status = 'cancelled' WHERE id = ?1",
                [deposit.ok_or_else(|| "缺少存款关联".to_string())?],
            )
            .map_err(|error| error.to_string())?;
        }
        "DEPOSIT_MATURITY" => {
            tx.execute(
                "UPDATE deposits SET status = 'active', matured_at = NULL, proceeds = NULL WHERE id = ?1",
                [deposit.ok_or_else(|| "缺少存款关联".to_string())?],
            )
            .map_err(|error| error.to_string())?;
        }
        _ => return Err("该交易类型不支持撤销".to_string()),
    }

    if let (Some(product), Some(account), Some(previous)) = (product, account, before) {
        replace_current_valuation(
            &tx,
            product,
            account,
            &reversal_date,
            previous,
            &currency,
            Some(reversal_id),
        )
        .map_err(|error| error.to_string())?;
    }
    tx.execute(
        "UPDATE transactions SET reversed_by = ?1 WHERE id = ?2",
        params![reversal_id, transaction_id],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(EntryResult {
        message: format!("交易 #{transaction_id} 已安全冲销，相关持仓和收益已恢复"),
        realized_gain: None,
    })
}

fn clear_excel_data(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM valuations WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM position_lots WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM transactions WHERE source = 'excel'", [])?;
    tx.execute(
        "DELETE FROM deposits WHERE source = 'excel' AND is_user_edited = 0",
        [],
    )?;
    tx.execute(
        "UPDATE position_lots
         SET remaining_amount = original_amount, status = 'active'
         WHERE source = 'manual'",
        [],
    )?;
    tx.execute(
        "UPDATE position_lots
         SET remaining_amount = 0, status = 'reversed'
         WHERE transaction_id IN (
           SELECT id FROM transactions WHERE reversed_by IS NOT NULL
         )",
        [],
    )?;
    tx.execute(
        "DELETE FROM products WHERE source = 'excel' AND is_user_edited = 0
         AND id NOT IN (SELECT DISTINCT product_id FROM transactions WHERE product_id IS NOT NULL)
         AND id NOT IN (SELECT DISTINCT product_id FROM position_lots)
         AND id NOT IN (SELECT DISTINCT product_id FROM valuations)",
        [],
    )?;
    tx.execute(
        "DELETE FROM accounts WHERE source = 'excel' AND is_user_edited = 0
         AND id NOT IN (SELECT DISTINCT account_id FROM transactions WHERE account_id IS NOT NULL)
         AND id NOT IN (SELECT DISTINCT account_id FROM position_lots)
         AND id NOT IN (SELECT DISTINCT account_id FROM valuations)
         AND id NOT IN (SELECT DISTINCT account_id FROM deposits)
         AND id NOT IN (SELECT DISTINCT account_id FROM account_balance_snapshots)",
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
        let source_row = (index + 1) as i64;
        let correction_key = excel_transaction_correction_key("BUY", source_row);
        let correction: Option<(String, f64, Option<String>)> = tx
            .query_row(
                "SELECT trade_date, amount, note FROM excel_transaction_corrections
                 WHERE correction_key = ?1",
                [&correction_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let (amount, purchase_date, transaction_note) =
            if let Some((date, amount, note)) = correction {
                let corrected_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                    .map_err(|_| format!("Sheet1 第{source_row}行保存的修正日期无效"))?;
                warnings.push(format!("Sheet1 第{source_row}行已保留软件中的手工修正"));
                (amount, corrected_date, note)
            } else {
                (
                    amount.unwrap_or_default(),
                    purchase_date.unwrap_or(today),
                    Some("Excel 导入".to_string()),
                )
            };
        let active = end_date.map(|date| date >= workbook_as_of).unwrap_or(true);
        let product =
            product_id(&tx, &code, &name, &currency).map_err(|error| error.to_string())?;
        let account =
            account_id(&tx, &institution, &currency).map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO transactions (product_id, account_id, transaction_type, trade_date, amount, currency, note, cost_basis, source, source_row)
             VALUES (?1, ?2, 'BUY', ?3, ?4, ?5, ?6, ?4, 'excel', ?7)",
            params![product, account, purchase_date.format("%Y-%m-%d").to_string(), amount, currency, transaction_note, source_row],
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
                source_row
            ],
        )
        .map_err(|error| error.to_string())?;
        lots_imported += 1;
    }

    replay_manual_exits(&tx).map_err(|error| error.to_string())?;

    let mut reading_historical_redemptions = false;
    let mut historical_redemptions: HashMap<String, (String, String, f64, i64)> = HashMap::new();
    for (index, row) in summaries.rows().enumerate().skip(1) {
        let name = text(row.first());
        if name.is_empty() {
            reading_historical_redemptions = true;
            continue;
        }
        if name == "名称" {
            reading_historical_redemptions = true;
            continue;
        }
        let code = text(row.get(1));
        let currency = text(row.get(2)).to_uppercase();
        let institution = text(row.get(3));
        let market_value = number(row.get(4));
        if code.is_empty() || currency.is_empty() || market_value.is_none() {
            warnings.push(format!(
                "Sheet2 第{}行缺少代码、币种或金额，已跳过",
                index + 1
            ));
            continue;
        }
        if reading_historical_redemptions {
            let key = format!("{}|{}", code.trim(), institution.trim());
            if historical_redemptions
                .insert(
                    key,
                    (
                        name,
                        currency,
                        market_value.unwrap_or_default(),
                        (index + 1) as i64,
                    ),
                )
                .is_some()
            {
                warnings.push(format!(
                    "Sheet2 第{}行的历史赎回产品 {code} 重复，已采用最后一行",
                    index + 1
                ));
            }
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

    let expired_groups = {
        let mut statement = tx
            .prepare(
                "SELECT p.id, a.id, COALESCE(p.import_code, p.code),
                        COALESCE(a.import_institution, a.institution), p.currency,
                        SUM(l.original_amount), MAX(l.end_date), MIN(l.source_row)
                 FROM position_lots l
                 JOIN products p ON p.id = l.product_id
                 JOIN accounts a ON a.id = l.account_id
                 WHERE l.source = 'excel' AND l.status = 'closed'
                 GROUP BY p.id, a.id, COALESCE(p.import_code, p.code),
                          COALESCE(a.import_institution, a.institution), p.currency
                 ORDER BY MIN(l.source_row)",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    let mut used_historical_redemptions = HashSet::new();
    for (product, account, code, institution, currency, original_cost, end_date, first_lot_row) in
        expired_groups
    {
        let redemption_key =
            historical_redemption_key(&code, &institution, &currency, original_cost, &end_date);
        let confirmation: Option<(String, f64, Option<String>, Option<i64>)> = tx
            .query_row(
                "SELECT confirmed_trade_date, proceeds, note, source_row
                 FROM historical_redemption_confirmations WHERE redemption_key = ?1",
                [&redemption_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let sheet_key = format!("{}|{}", code.trim(), institution.trim());
        let (redemption_date, proceeds, redemption_note, transaction_source_row) = if let Some((
            confirmed_date,
            confirmed_proceeds,
            note,
            source_row,
        )) =
            confirmation
        {
            let note = note
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("历史赎回已核对；{}", value.trim()))
                .unwrap_or_else(|| "历史赎回已核对".to_string());
            if historical_redemptions.contains_key(&sheet_key) {
                used_historical_redemptions.insert(sheet_key.clone());
            }
            (
                confirmed_date,
                confirmed_proceeds,
                note,
                source_row.unwrap_or(first_lot_row),
            )
        } else if let Some((_, sheet_currency, sheet_proceeds, source_row)) =
            historical_redemptions.get(&sheet_key)
        {
            used_historical_redemptions.insert(sheet_key.clone());
            if sheet_currency != &currency {
                warnings.push(format!(
                        "Sheet2 历史赎回产品 {code} 的币种为 {sheet_currency}，买入记录为 {currency}；已按产品买入币种 {currency} 处理"
                    ));
            }
            (
                end_date.clone(),
                *sheet_proceeds,
                "根据 Sheet2 历史赎回表导入；赎回日期按最后结束日".to_string(),
                *source_row,
            )
        } else {
            (
                end_date.clone(),
                original_cost,
                "根据到期日期生成；赎回金额待核对".to_string(),
                first_lot_row,
            )
        };
        tx.execute(
            "INSERT INTO transactions
               (product_id, account_id, transaction_type, trade_date, amount, currency, note,
                cost_basis, realized_gain, source, source_row)
             VALUES (?1, ?2, 'REDEEM', ?3, ?4, ?5, ?6, ?7, ?8, 'excel', ?9)",
            params![
                product,
                account,
                redemption_date,
                proceeds,
                currency,
                redemption_note,
                original_cost,
                proceeds - original_cost,
                transaction_source_row
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    for (key, (name, _, _, source_row)) in &historical_redemptions {
        if !used_historical_redemptions.contains(key) {
            warnings.push(format!(
                "Sheet2 第{source_row}行的历史赎回产品 {name} 没有找到对应的已结束买入批次"
            ));
        }
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
        let source_row = (index + 1) as i64;
        let preserved_edit: bool = tx
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM deposits
                   WHERE source = 'excel' AND source_row = ?1 AND is_user_edited = 1
                 )",
                [source_row],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if preserved_edit {
            warnings.push(format!(
                "Sheet3 第{}行已有手工修正，本次导入保留修正后的存款资料",
                index + 1
            ));
            deposits_imported += 1;
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
                source_row
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

fn backup_to_path(conn: &Connection, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "备份路径不正确".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "sqlite3" | "db") {
        return Err("备份文件必须使用 .sqlite3 或 .db 扩展名".to_string());
    }
    let temp_name = format!(
        ".{}.{}.tmp.sqlite3",
        target
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("wealth-manager"),
        Local::now().format("%Y%m%d%H%M%S%3f")
    );
    let temp_path = parent.join(temp_name);
    conn.backup(MAIN_DB, &temp_path, None)
        .map_err(|error| error.to_string())?;
    if target.exists() {
        fs::remove_file(target).map_err(|error| error.to_string())?;
    }
    fs::rename(&temp_path, target).map_err(|error| error.to_string())?;
    Ok(())
}

fn create_auto_backup(
    conn: &Connection,
    backup_dir: &Path,
    reason: &str,
) -> Result<PathBuf, String> {
    let path = backup_dir.join(format!(
        "auto-{}-{reason}.sqlite3",
        Local::now().format("%Y%m%d-%H%M%S-%3f")
    ));
    backup_to_path(conn, &path)?;
    Ok(path)
}

fn ensure_daily_backup(conn: &Connection, backup_dir: &Path) -> Result<(), String> {
    let prefix = format!("auto-{}-", Local::now().format("%Y%m%d"));
    let exists = fs::read_dir(backup_dir)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix));
    if !exists {
        create_auto_backup(conn, backup_dir, "startup")?;
    }
    Ok(())
}

fn validate_backup(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("所选备份文件不存在".to_string());
    }
    let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "无法读取所选备份".to_string())?;
    let integrity: String = source
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if integrity != "ok" {
        return Err(format!("备份完整性检查失败：{integrity}"));
    }
    let required_tables: i64 = source
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name IN ('accounts', 'products', 'transactions',
                                                'position_lots', 'valuations', 'deposits')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if required_tables != 6 {
        return Err("所选文件不是有效的稳盈数据库备份".to_string());
    }
    Ok(())
}

fn export_formats() -> (Format, Format, Format, Format, Format, Format, Format) {
    let title = Format::new()
        .set_bold()
        .set_font_size(18)
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x173B2A))
        .set_align(FormatAlign::VerticalCenter);
    let subtitle = Format::new()
        .set_font_color(Color::RGB(0x66746C))
        .set_italic();
    let header = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x2E7D55))
        .set_border_bottom(FormatBorder::Thin)
        .set_align(FormatAlign::VerticalCenter);
    let money = Format::new()
        .set_num_format("#,##0.00;[Red](#,##0.00);-")
        .set_align(FormatAlign::Right);
    let percent = Format::new()
        .set_num_format("0.00%;[Red](0.00%);-")
        .set_align(FormatAlign::Right);
    let date = Format::new().set_num_format("yyyy-mm-dd");
    let linked_formula = Format::new()
        .set_num_format("#,##0.00;[Red](#,##0.00);-")
        .set_font_color(Color::RGB(0x008000))
        .set_align(FormatAlign::Right);
    (
        title,
        subtitle,
        header,
        money,
        percent,
        date,
        linked_formula,
    )
}

struct ExportHeadingFormats<'a> {
    title: &'a Format,
    subtitle: &'a Format,
    header: &'a Format,
}

fn write_export_heading(
    sheet: &mut Worksheet,
    title_text: &str,
    subtitle_text: &str,
    last_col: u16,
    formats: &ExportHeadingFormats<'_>,
    headers: &[&str],
) -> Result<(), String> {
    sheet
        .merge_range(0, 0, 0, last_col, title_text, formats.title)
        .map_err(|error| error.to_string())?;
    sheet
        .set_row_height(0, 28)
        .map_err(|error| error.to_string())?;
    sheet
        .merge_range(1, 0, 1, last_col, subtitle_text, formats.subtitle)
        .map_err(|error| error.to_string())?;
    for (column, value) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(2, column as u16, *value, formats.header)
            .map_err(|error| error.to_string())?;
    }
    sheet
        .set_freeze_panes(3, 0)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn write_export_date(
    sheet: &mut Worksheet,
    row: u32,
    column: u16,
    value: &str,
    format: &Format,
) -> Result<(), String> {
    match ExcelDateTime::parse_from_str(value) {
        Ok(date) => sheet
            .write_datetime_with_format(row, column, &date, format)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Err(_) => sheet
            .write_string(row, column, value)
            .map(|_| ())
            .map_err(|error| error.to_string()),
    }
}

fn export_excel(conn: &Connection, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "导出路径不正确".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("xlsx") {
        return Err("导出文件必须使用 .xlsx 扩展名".to_string());
    }

    type HoldingRow = (String, String, String, String, f64, f64, f64, f64, String);
    let holdings: Vec<HoldingRow> = {
        let mut statement = conn
            .prepare(
                "SELECT p.name, p.code, v.currency, a.institution,
                        COALESCE(SUM(COALESCE(l.remaining_amount, l.original_amount)), 0),
                        v.market_value,
                        v.market_value - COALESCE(SUM(COALESCE(l.remaining_amount, l.original_amount)), 0),
                        CASE WHEN COALESCE(SUM(COALESCE(l.remaining_amount, l.original_amount)), 0) = 0
                             THEN 0 ELSE (v.market_value / SUM(COALESCE(l.remaining_amount, l.original_amount))) - 1 END,
                        v.valuation_date
                 FROM valuations v
                 JOIN products p ON p.id = v.product_id
                 JOIN accounts a ON a.id = v.account_id
                 LEFT JOIN position_lots l ON l.product_id = v.product_id
                     AND l.account_id = v.account_id AND l.status = 'active'
                 WHERE v.is_current = 1 AND (v.market_value > 0.000001 OR l.id IS NOT NULL)
                 GROUP BY v.id, p.name, p.code, v.currency, a.institution, v.market_value, v.valuation_date
                 ORDER BY v.currency, v.market_value DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    type TransactionRow = (
        i64,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        f64,
        f64,
        f64,
        String,
        String,
        Option<String>,
        Option<i64>,
        Option<i64>,
    );
    let transactions: Vec<TransactionRow> = {
        let mut statement = conn
            .prepare(
                "SELECT t.id, t.trade_date,
                        CASE t.transaction_type
                          WHEN 'BUY' THEN '买入' WHEN 'SELL' THEN '卖出'
                          WHEN 'REDEEM' THEN '历史赎回' WHEN 'PRODUCT_MATURITY' THEN '理财到期'
                          WHEN 'VALUATION' THEN '更新市值' WHEN 'DIVIDEND' THEN '分红'
                          WHEN 'FEE' THEN '费用' WHEN 'TRANSFER' THEN '账户转账'
                          WHEN 'DEPOSIT_OPEN' THEN '定存开户'
                          WHEN 'DEPOSIT_MATURITY' THEN '定存到期' WHEN 'REVERSAL' THEN '冲销'
                          ELSE t.transaction_type END,
                        CASE WHEN t.transaction_type = 'TRANSFER' THEN
                          COALESCE(a.institution, '未知账户') || ' → ' || COALESCE(ta.institution, '未知账户')
                        ELSE COALESCE(p.name, d.name, a.institution, '未命名交易') END,
                        p.code,
                        CASE WHEN t.transaction_type = 'TRANSFER' THEN
                          COALESCE(a.institution, '未知账户') || ' → ' || COALESCE(ta.institution, '未知账户')
                        ELSE a.institution END,
                        t.currency, t.amount, COALESCE(t.cost_basis, 0),
                        COALESCE(t.realized_gain, 0),
                        CASE t.source WHEN 'manual' THEN '手工' ELSE 'Excel' END,
                        CASE WHEN t.reversed_by IS NOT NULL THEN '已冲销'
                             WHEN t.reversal_of IS NOT NULL THEN '冲销记录' ELSE '有效' END,
                        t.note, t.reversal_of, t.reversed_by
                 FROM transactions t
                 LEFT JOIN products p ON p.id = t.product_id
                 LEFT JOIN deposits d ON d.id = t.deposit_id
                 LEFT JOIN accounts a ON a.id = t.account_id
                 LEFT JOIN accounts ta ON ta.id = t.transfer_account_id
                 ORDER BY t.trade_date, t.id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    type DepositRow = (
        String,
        String,
        String,
        f64,
        Option<String>,
        String,
        f64,
        String,
        Option<String>,
        Option<f64>,
    );
    let deposits: Vec<DepositRow> = {
        let mut statement = conn
            .prepare(
                "SELECT d.name, a.institution, d.currency, d.principal, d.start_date,
                        d.maturity_date, d.annual_rate,
                        CASE d.status WHEN 'active' THEN '有效' WHEN 'matured' THEN '已到期'
                             WHEN 'cancelled' THEN '已取消' ELSE d.status END,
                        d.matured_at, d.proceeds
                 FROM deposits d JOIN accounts a ON a.id = d.account_id
                 ORDER BY d.maturity_date",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    type LotRow = (
        i64,
        String,
        String,
        String,
        String,
        f64,
        f64,
        Option<String>,
        String,
        String,
    );
    let lots: Vec<LotRow> = {
        let mut statement = conn
            .prepare(
                "SELECT l.id, p.name, p.code, a.institution, l.purchase_date,
                        l.original_amount, COALESCE(l.remaining_amount, l.original_amount),
                        l.end_date,
                        CASE l.status WHEN 'active' THEN '持有' WHEN 'closed' THEN '结束'
                             WHEN 'reversed' THEN '已冲销' ELSE l.status END,
                        CASE l.source WHEN 'manual' THEN '手工' ELSE 'Excel' END
                 FROM position_lots l
                 JOIN products p ON p.id = l.product_id
                 JOIN accounts a ON a.id = l.account_id
                 ORDER BY l.purchase_date, l.id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    type ValuationRow = (String, String, String, String, String, f64, bool, String);
    let valuations: Vec<ValuationRow> = {
        let mut statement = conn
            .prepare(
                "SELECT p.name, p.code, a.institution, v.currency, v.valuation_date,
                        v.market_value, v.is_current,
                        CASE v.source WHEN 'manual' THEN '手工' ELSE 'Excel' END
                 FROM valuations v
                 JOIN products p ON p.id = v.product_id
                 JOIN accounts a ON a.id = v.account_id
                 ORDER BY v.valuation_date, v.id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get::<_, i64>(6)? != 0,
                    row.get(7)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };

    let (title, subtitle, header, money, percent, date, linked_formula) = export_formats();
    let heading_formats = ExportHeadingFormats {
        title: &title,
        subtitle: &subtitle,
        header: &header,
    };
    let export_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut workbook = Workbook::new();

    {
        let sheet = workbook
            .add_worksheet()
            .set_name("资产概览")
            .map_err(|error| error.to_string())?;
        sheet.set_screen_gridlines(false);
        sheet
            .merge_range(0, 0, 0, 5, "稳盈 · 资产数据导出", &title)
            .map_err(|error| error.to_string())?;
        sheet
            .merge_range(
                1,
                0,
                1,
                5,
                &format!("导出时间：{export_time}｜金额单位：原币"),
                &subtitle,
            )
            .map_err(|error| error.to_string())?;
        for (column, value) in [
            "币种",
            "理财市值",
            "定期存款",
            "总资产",
            "持仓成本",
            "持有收益",
        ]
        .iter()
        .enumerate()
        {
            sheet
                .write_string_with_format(3, column as u16, *value, &header)
                .map_err(|error| error.to_string())?;
        }
        let holding_end = (holdings.len() + 3).max(4);
        let deposit_end = (deposits.len() + 3).max(4);
        for (index, currency) in ["CNY", "USD"].iter().enumerate() {
            let row = 4 + index as u32;
            let excel_row = row + 1;
            let wealth_value: f64 = holdings
                .iter()
                .filter(|item| item.2 == *currency)
                .map(|item| item.5)
                .sum();
            let deposit_value: f64 = deposits
                .iter()
                .filter(|item| item.2 == *currency && item.7 == "有效")
                .map(|item| item.3)
                .sum();
            let holding_cost: f64 = holdings
                .iter()
                .filter(|item| item.2 == *currency)
                .map(|item| item.4)
                .sum();
            sheet
                .write_string(row, 0, *currency)
                .map_err(|error| error.to_string())?;
            sheet
                .write_formula_with_format(
                    row,
                    1,
                    Formula::new(format!("=SUMIF('当前持仓'!$C$4:$C${holding_end},A{excel_row},'当前持仓'!$F$4:$F${holding_end})"))
                        .set_result(wealth_value.to_string()),
                    &linked_formula,
                )
                .map_err(|error| error.to_string())?;
            sheet
                .write_formula_with_format(
                    row,
                    2,
                    Formula::new(format!("=SUMIFS('定期存款'!$D$4:$D${deposit_end},'定期存款'!$C$4:$C${deposit_end},A{excel_row},'定期存款'!$H$4:$H${deposit_end},\"有效\")"))
                        .set_result(deposit_value.to_string()),
                    &linked_formula,
                )
                .map_err(|error| error.to_string())?;
            sheet
                .write_formula_with_format(
                    row,
                    3,
                    Formula::new(format!("=B{excel_row}+C{excel_row}"))
                        .set_result((wealth_value + deposit_value).to_string()),
                    &linked_formula,
                )
                .map_err(|error| error.to_string())?;
            sheet
                .write_formula_with_format(
                    row,
                    4,
                    Formula::new(format!("=SUMIF('当前持仓'!$C$4:$C${holding_end},A{excel_row},'当前持仓'!$E$4:$E${holding_end})"))
                        .set_result(holding_cost.to_string()),
                    &linked_formula,
                )
                .map_err(|error| error.to_string())?;
            sheet
                .write_formula_with_format(
                    row,
                    5,
                    Formula::new(format!("=B{excel_row}-E{excel_row}"))
                        .set_result((wealth_value - holding_cost).to_string()),
                    &linked_formula,
                )
                .map_err(|error| error.to_string())?;
        }
        sheet
            .set_column_width(0, 12)
            .map_err(|error| error.to_string())?;
        for column in 1..=5 {
            sheet
                .set_column_width(column, 18)
                .map_err(|error| error.to_string())?;
        }
        sheet
            .write_string(8, 0, "说明")
            .map_err(|error| error.to_string())?;
        sheet
            .merge_range(
                9,
                0,
                10,
                5,
                "绿色数字为跨工作表公式，可在 Excel 中追溯。当前市值取最新有效快照；已冲销交易仍保留在交易流水中。",
                &subtitle,
            )
            .map_err(|error| error.to_string())?;
    }

    {
        let sheet = workbook
            .add_worksheet()
            .set_name("当前持仓")
            .map_err(|error| error.to_string())?;
        sheet.set_screen_gridlines(false);
        let headers = [
            "产品名称",
            "产品代码",
            "币种",
            "购买渠道",
            "剩余成本",
            "当前市值",
            "持有收益",
            "收益率",
            "估值日期",
        ];
        write_export_heading(
            sheet,
            "当前持仓",
            &format!("截至 {export_time}"),
            8,
            &heading_formats,
            &headers,
        )?;
        for (index, item) in holdings.iter().enumerate() {
            let row = index as u32 + 3;
            sheet
                .write_string(row, 0, &item.0)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 1, &item.1)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 2, &item.2)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 3, &item.3)
                .map_err(|error| error.to_string())?;
            for (column, value) in [(4, item.4), (5, item.5), (6, item.6)] {
                sheet
                    .write_number_with_format(row, column, value, &money)
                    .map_err(|error| error.to_string())?;
            }
            sheet
                .write_number_with_format(row, 7, item.7, &percent)
                .map_err(|error| error.to_string())?;
            write_export_date(sheet, row, 8, &item.8, &date)?;
        }
        for (column, width) in [50.0, 25.0, 9.0, 16.0, 16.0, 16.0, 16.0, 12.0, 13.0]
            .iter()
            .enumerate()
        {
            sheet
                .set_column_width(column as u16, *width)
                .map_err(|error| error.to_string())?;
        }
        sheet
            .autofilter(2, 0, (holdings.len() + 2) as u32, 8)
            .map_err(|error| error.to_string())?;
    }

    {
        let sheet = workbook
            .add_worksheet()
            .set_name("交易流水")
            .map_err(|error| error.to_string())?;
        sheet.set_screen_gridlines(false);
        let headers = [
            "ID",
            "日期",
            "操作",
            "产品/存款",
            "代码",
            "银行",
            "币种",
            "现金金额",
            "核销成本",
            "已实现收益",
            "来源",
            "状态",
            "备注",
            "冲销原交易",
            "被冲销交易",
        ];
        write_export_heading(
            sheet,
            "交易流水",
            "原始交易和冲销记录完整保留",
            14,
            &heading_formats,
            &headers,
        )?;
        for (index, item) in transactions.iter().enumerate() {
            let row = index as u32 + 3;
            sheet
                .write_number(row, 0, item.0 as f64)
                .map_err(|error| error.to_string())?;
            write_export_date(sheet, row, 1, &item.1, &date)?;
            sheet
                .write_string(row, 2, &item.2)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 3, &item.3)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 4, item.4.as_deref().unwrap_or(""))
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 5, item.5.as_deref().unwrap_or(""))
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 6, &item.6)
                .map_err(|error| error.to_string())?;
            for (column, value) in [(7, item.7), (8, item.8), (9, item.9)] {
                sheet
                    .write_number_with_format(row, column, value, &money)
                    .map_err(|error| error.to_string())?;
            }
            sheet
                .write_string(row, 10, &item.10)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 11, &item.11)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 12, item.12.as_deref().unwrap_or(""))
                .map_err(|error| error.to_string())?;
            if let Some(value) = item.13 {
                sheet
                    .write_number(row, 13, value as f64)
                    .map_err(|error| error.to_string())?;
            }
            if let Some(value) = item.14 {
                sheet
                    .write_number(row, 14, value as f64)
                    .map_err(|error| error.to_string())?;
            }
        }
        for (column, width) in [
            8.0, 13.0, 16.0, 50.0, 25.0, 16.0, 9.0, 15.0, 15.0, 16.0, 10.0, 12.0, 40.0, 12.0, 12.0,
        ]
        .iter()
        .enumerate()
        {
            sheet
                .set_column_width(column as u16, *width)
                .map_err(|error| error.to_string())?;
        }
        sheet
            .autofilter(2, 0, (transactions.len() + 2) as u32, 14)
            .map_err(|error| error.to_string())?;
    }

    {
        let sheet = workbook
            .add_worksheet()
            .set_name("定期存款")
            .map_err(|error| error.to_string())?;
        sheet.set_screen_gridlines(false);
        let headers = [
            "存款名称",
            "银行",
            "币种",
            "本金",
            "起息日",
            "到期日",
            "年利率",
            "状态",
            "结清日",
            "到账金额",
        ];
        write_export_heading(
            sheet,
            "定期存款",
            "包含有效、已到期和已取消记录",
            9,
            &heading_formats,
            &headers,
        )?;
        for (index, item) in deposits.iter().enumerate() {
            let row = index as u32 + 3;
            sheet
                .write_string(row, 0, &item.0)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 1, &item.1)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 2, &item.2)
                .map_err(|error| error.to_string())?;
            sheet
                .write_number_with_format(row, 3, item.3, &money)
                .map_err(|error| error.to_string())?;
            if let Some(value) = &item.4 {
                write_export_date(sheet, row, 4, value, &date)?;
            }
            write_export_date(sheet, row, 5, &item.5, &date)?;
            sheet
                .write_number_with_format(row, 6, item.6, &percent)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 7, &item.7)
                .map_err(|error| error.to_string())?;
            if let Some(value) = &item.8 {
                write_export_date(sheet, row, 8, value, &date)?;
            }
            if let Some(value) = item.9 {
                sheet
                    .write_number_with_format(row, 9, value, &money)
                    .map_err(|error| error.to_string())?;
            }
        }
        for (column, width) in [36.0, 16.0, 9.0, 16.0, 13.0, 13.0, 11.0, 12.0, 13.0, 16.0]
            .iter()
            .enumerate()
        {
            sheet
                .set_column_width(column as u16, *width)
                .map_err(|error| error.to_string())?;
        }
        sheet
            .autofilter(2, 0, (deposits.len() + 2) as u32, 9)
            .map_err(|error| error.to_string())?;
    }

    {
        let sheet = workbook
            .add_worksheet()
            .set_name("买入批次")
            .map_err(|error| error.to_string())?;
        sheet.set_screen_gridlines(false);
        let headers = [
            "批次ID",
            "产品名称",
            "产品代码",
            "购买渠道",
            "买入日期",
            "原始成本",
            "剩余成本",
            "结束日期",
            "状态",
            "来源",
        ];
        write_export_heading(
            sheet,
            "买入批次",
            "FIFO 成本核销审计明细",
            9,
            &heading_formats,
            &headers,
        )?;
        for (index, item) in lots.iter().enumerate() {
            let row = index as u32 + 3;
            sheet
                .write_number(row, 0, item.0 as f64)
                .map_err(|error| error.to_string())?;
            for (column, value) in [
                (1, item.1.as_str()),
                (2, item.2.as_str()),
                (3, item.3.as_str()),
            ] {
                sheet
                    .write_string(row, column, value)
                    .map_err(|error| error.to_string())?;
            }
            write_export_date(sheet, row, 4, &item.4, &date)?;
            sheet
                .write_number_with_format(row, 5, item.5, &money)
                .map_err(|error| error.to_string())?;
            sheet
                .write_number_with_format(row, 6, item.6, &money)
                .map_err(|error| error.to_string())?;
            if let Some(value) = &item.7 {
                write_export_date(sheet, row, 7, value, &date)?;
            }
            sheet
                .write_string(row, 8, &item.8)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 9, &item.9)
                .map_err(|error| error.to_string())?;
        }
        for (column, width) in [9.0, 50.0, 25.0, 16.0, 13.0, 16.0, 16.0, 13.0, 12.0, 10.0]
            .iter()
            .enumerate()
        {
            sheet
                .set_column_width(column as u16, *width)
                .map_err(|error| error.to_string())?;
        }
        sheet
            .autofilter(2, 0, (lots.len() + 2) as u32, 9)
            .map_err(|error| error.to_string())?;
    }

    {
        let sheet = workbook
            .add_worksheet()
            .set_name("市值历史")
            .map_err(|error| error.to_string())?;
        sheet.set_screen_gridlines(false);
        let headers = [
            "产品名称",
            "产品代码",
            "购买渠道",
            "币种",
            "估值日期",
            "市值",
            "是否当前",
            "来源",
        ];
        write_export_heading(
            sheet,
            "市值历史",
            "每次手工更新均保留历史快照",
            7,
            &heading_formats,
            &headers,
        )?;
        for (index, item) in valuations.iter().enumerate() {
            let row = index as u32 + 3;
            for (column, value) in [
                (0, item.0.as_str()),
                (1, item.1.as_str()),
                (2, item.2.as_str()),
                (3, item.3.as_str()),
            ] {
                sheet
                    .write_string(row, column, value)
                    .map_err(|error| error.to_string())?;
            }
            write_export_date(sheet, row, 4, &item.4, &date)?;
            sheet
                .write_number_with_format(row, 5, item.5, &money)
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 6, if item.6 { "是" } else { "否" })
                .map_err(|error| error.to_string())?;
            sheet
                .write_string(row, 7, &item.7)
                .map_err(|error| error.to_string())?;
        }
        for (column, width) in [50.0, 25.0, 16.0, 9.0, 13.0, 16.0, 11.0, 10.0]
            .iter()
            .enumerate()
        {
            sheet
                .set_column_width(column as u16, *width)
                .map_err(|error| error.to_string())?;
        }
        sheet
            .autofilter(2, 0, (valuations.len() + 2) as u32, 7)
            .map_err(|error| error.to_string())?;
    }

    workbook.save(target).map_err(|error| error.to_string())?;
    Ok(())
}

fn list_master_data_from_conn(conn: &Connection) -> Result<MasterData, String> {
    let accounts = {
        let mut statement = conn
            .prepare(
                "SELECT id, institution, name, currency, source
                 FROM accounts ORDER BY currency, institution, name",
            )
            .map_err(|error| error.to_string())?;
        let records = statement
            .query_map([], |row| {
                Ok(AccountRecord {
                    id: row.get(0)?,
                    institution: row.get(1)?,
                    name: row.get(2)?,
                    currency: row.get(3)?,
                    source: row.get(4)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        records
    };
    let products = {
        let mut statement = conn
            .prepare(
                "SELECT id, code, name, currency, issuer, risk_level, source
                 FROM products ORDER BY currency, name, code",
            )
            .map_err(|error| error.to_string())?;
        let records = statement
            .query_map([], |row| {
                Ok(ProductRecord {
                    id: row.get(0)?,
                    code: row.get(1)?,
                    name: row.get(2)?,
                    currency: row.get(3)?,
                    issuer: row.get(4)?,
                    risk_level: row.get(5)?,
                    source: row.get(6)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        records
    };
    let deposits = {
        let mut statement = conn
            .prepare(
                "SELECT d.id, d.account_id, a.institution, d.name, d.currency, d.principal,
                        d.start_date, d.maturity_date, d.annual_rate, d.status, d.source
                 FROM deposits d JOIN accounts a ON a.id = d.account_id
                 ORDER BY d.status, d.maturity_date, d.id",
            )
            .map_err(|error| error.to_string())?;
        let records = statement
            .query_map([], |row| {
                Ok(DepositRecord {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    institution: row.get(2)?,
                    name: row.get(3)?,
                    currency: row.get(4)?,
                    principal: row.get(5)?,
                    start_date: row.get(6)?,
                    maturity_date: row.get(7)?,
                    annual_rate: row.get(8)?,
                    status: row.get(9)?,
                    source: row.get(10)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        records
    };
    Ok(MasterData {
        accounts,
        products,
        deposits,
    })
}

fn reconciliation_center_from_conn(conn: &Connection) -> Result<ReconciliationCenter, String> {
    let accounts = {
        let mut statement = conn
            .prepare(
                "SELECT a.id, a.institution, a.name, a.currency,
                        COALESCE((
                          SELECT SUM(v.market_value) FROM valuations v
                          WHERE v.account_id = a.id AND v.is_current = 1
                        ), 0),
                        COALESCE((
                          SELECT SUM(d.principal) FROM deposits d
                          WHERE d.account_id = a.id AND d.status = 'active'
                        ), 0),
                        s.actual_balance, s.balance_date, s.note
                 FROM accounts a
                 LEFT JOIN account_balance_snapshots s ON s.id = (
                   SELECT s2.id FROM account_balance_snapshots s2
                   WHERE s2.account_id = a.id
                   ORDER BY s2.balance_date DESC, s2.id DESC LIMIT 1
                 )
                 ORDER BY a.currency, a.institution, a.name",
            )
            .map_err(|error| error.to_string())?;
        let records = statement
            .query_map([], |row| {
                let wealth_value: f64 = row.get(4)?;
                let deposit_value: f64 = row.get(5)?;
                let tracked_total = wealth_value + deposit_value;
                let actual_balance: Option<f64> = row.get(6)?;
                let difference = actual_balance.map(|value| value - tracked_total);
                let currency: String = row.get(3)?;
                let tolerance = if currency == "USD" { 0.01 } else { 1.0 };
                let status = match difference {
                    None => "missing",
                    Some(value) if value.abs() <= tolerance => "matched",
                    Some(_) => "difference",
                };
                Ok(AccountReconciliation {
                    account_id: row.get(0)?,
                    institution: row.get(1)?,
                    name: row.get(2)?,
                    currency,
                    wealth_value,
                    deposit_value,
                    tracked_total,
                    actual_balance,
                    difference,
                    balance_date: row.get(7)?,
                    note: row.get(8)?,
                    status: status.to_string(),
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        records
    };

    let mut issues = Vec::new();
    for account in &accounts {
        if account.tracked_total <= 0.000_001 && account.actual_balance.is_none() {
            continue;
        }
        match account.status.as_str() {
            "missing" => issues.push(QualityIssue {
                key: format!("reconciliation-missing-{}", account.account_id),
                severity: "medium".to_string(),
                category: "账户对账".to_string(),
                title: format!("{} 尚未核对", account.institution),
                detail: format!(
                    "账本记录 {} 资产 {:.2} {}，请录入银行页面显示的账户总资产",
                    account.name, account.tracked_total, account.currency
                ),
                target_view: "reconciliation".to_string(),
                target_id: Some(account.account_id),
            }),
            "difference" => issues.push(QualityIssue {
                key: format!("reconciliation-difference-{}", account.account_id),
                severity: "high".to_string(),
                category: "账户对账".to_string(),
                title: format!("{} 存在对账差额", account.institution),
                detail: format!(
                    "银行余额与账本相差 {:+.2} {}，请检查现金、漏记产品或入账时差",
                    account.difference.unwrap_or_default(),
                    account.currency
                ),
                target_view: "reconciliation".to_string(),
                target_id: Some(account.account_id),
            }),
            _ => {}
        }
    }

    {
        let mut statement = conn
            .prepare(
                "SELECT t.id, COALESCE(p.name, '历史理财'), t.trade_date, t.amount, t.currency
                 FROM transactions t LEFT JOIN products p ON p.id = t.product_id
                 WHERE t.transaction_type = 'REDEEM' AND t.source = 'excel'
                   AND t.note LIKE '%待核对%' AND t.reversed_by IS NULL
                 ORDER BY t.trade_date DESC, t.id DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for (id, name, date, amount, currency) in rows {
            issues.push(QualityIssue {
                key: format!("redeem-review-{id}"),
                severity: "high".to_string(),
                category: "历史流水".to_string(),
                title: format!("{name} 的历史赎回金额待核对"),
                detail: format!(
                    "{date} 暂按 {:.2} {currency} 生成，会影响历史收益和 XIRR",
                    amount
                ),
                target_view: "transactions".to_string(),
                target_id: Some(id),
            });
        }
    }

    {
        let mut statement = conn
            .prepare(
                "SELECT d.id, d.name, a.institution, d.maturity_date
                 FROM deposits d JOIN accounts a ON a.id = d.account_id
                 WHERE d.status = 'active' AND (d.start_date IS NULL OR trim(d.start_date) = '')
                 ORDER BY d.maturity_date",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for (id, name, institution, maturity) in rows {
            issues.push(QualityIssue {
                key: format!("deposit-start-date-{id}"),
                severity: "medium".to_string(),
                category: "定期存款".to_string(),
                title: format!("{name} 缺少起息日"),
                detail: format!(
                    "{institution} · 到期日 {maturity}；补全后才能纳入准确的期限收益分析"
                ),
                target_view: "masterData".to_string(),
                target_id: Some(id),
            });
        }
    }

    {
        let mut statement = conn
            .prepare(
                "SELECT DISTINCT p.id, p.name, p.code FROM products p
                 WHERE (p.risk_level IS NULL OR trim(p.risk_level) = '')
                   AND EXISTS (
                     SELECT 1 FROM valuations v
                     WHERE v.product_id = p.id AND v.is_current = 1
                       AND v.market_value > 0.000001
                   )
                 ORDER BY p.name",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for (id, name, code) in rows {
            issues.push(QualityIssue {
                key: format!("product-risk-{id}"),
                severity: "low".to_string(),
                category: "产品资料".to_string(),
                title: format!("{name} 缺少风险等级"),
                detail: format!("产品代码 {code}；补全 R1–R5 后，操作参考才能纳入风险约束"),
                target_view: "masterData".to_string(),
                target_id: Some(id),
            });
        }
    }

    {
        let today = Local::now().date_naive();
        let mut statement = conn
            .prepare(
                "SELECT v.product_id, p.name, v.valuation_date
                 FROM valuations v JOIN products p ON p.id = v.product_id
                 WHERE v.is_current = 1 AND v.market_value > 0.000001",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for (id, name, date) in rows {
            let valued_on = NaiveDate::parse_from_str(&date, "%Y-%m-%d").unwrap_or(today);
            let days = (today - valued_on).num_days().max(0);
            if days > 10 {
                issues.push(QualityIssue {
                    key: format!("stale-valuation-{id}"),
                    severity: "medium".to_string(),
                    category: "市值更新".to_string(),
                    title: format!("{name} 的市值已过期"),
                    detail: format!("最近更新于 {date}，距今 {days} 天"),
                    target_view: "holdings".to_string(),
                    target_id: Some(id),
                });
            }
        }
    }

    let last_warnings: Option<String> = conn
        .query_row(
            "SELECT warnings_json FROM import_runs ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(json) = last_warnings {
        if let Ok(warnings) = serde_json::from_str::<Vec<String>>(&json) {
            for (index, warning) in warnings.into_iter().enumerate() {
                issues.push(QualityIssue {
                    key: format!("import-warning-{index}"),
                    severity: "medium".to_string(),
                    category: "Excel 导入".to_string(),
                    title: "最近一次导入存在校验提醒".to_string(),
                    detail: warning,
                    target_view: "reconciliation".to_string(),
                    target_id: None,
                });
            }
        }
    }
    issues.sort_by_key(|issue| match issue.severity.as_str() {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    });
    let high_priority_count = issues
        .iter()
        .filter(|issue| issue.severity == "high")
        .count();
    Ok(ReconciliationCenter {
        accounts,
        issue_count: issues.len(),
        high_priority_count,
        issues,
    })
}

fn apply_balance_snapshot(
    conn: &mut Connection,
    input: &BalanceSnapshotInput,
) -> Result<EntryResult, String> {
    let balance_date = valid_date(&input.balance_date, "对账日期")?;
    if !input.actual_balance.is_finite() || input.actual_balance < 0.0 {
        return Err("银行显示总资产必须是大于或等于0的有效数字".to_string());
    }
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)",
            [input.account_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !exists {
        return Err("没有找到所选账户".to_string());
    }
    conn.execute(
        "INSERT INTO account_balance_snapshots
           (account_id, balance_date, actual_balance, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(account_id, balance_date) DO UPDATE SET
           actual_balance = excluded.actual_balance,
           note = excluded.note,
           created_at = excluded.created_at",
        params![
            input.account_id,
            balance_date,
            input.actual_balance,
            input.note,
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(EntryResult {
        message: "账户余额已核对，差额和数据质量清单已更新".to_string(),
        realized_gain: None,
    })
}

fn apply_historical_redemption_confirmation(
    conn: &mut Connection,
    input: &HistoricalRedemptionInput,
) -> Result<EntryResult, String> {
    let trade_date = valid_date(&input.trade_date, "实际赎回日期")?;
    if !input.proceeds.is_finite() || input.proceeds < 0.0 {
        return Err("实际到账金额必须是大于或等于0的有效数字".to_string());
    }
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let previous = tx
        .query_row(
            "SELECT t.source_row, COALESCE(p.import_code, p.code),
                    COALESCE(a.import_institution, a.institution), t.currency,
                    SUM(l.original_amount), MAX(l.end_date),
                    t.trade_date, t.amount, COALESCE(t.cost_basis, 0),
                    COALESCE(t.realized_gain, 0), t.note
             FROM transactions t
             JOIN products p ON p.id = t.product_id
             JOIN accounts a ON a.id = t.account_id
             JOIN position_lots l ON l.product_id = t.product_id
               AND l.account_id = t.account_id AND l.source = 'excel'
               AND l.status = 'closed'
             WHERE t.id = ?1 AND t.transaction_type = 'REDEEM'
               AND t.source = 'excel' AND t.reversed_by IS NULL
            GROUP BY t.id, p.id, a.id",
            [input.transaction_id],
            |row| {
                Ok(HistoricalRedemptionRecord {
                    source_row: row.get(0)?,
                    product_code: row.get(1)?,
                    institution: row.get(2)?,
                    currency: row.get(3)?,
                    original_cost: row.get(4)?,
                    original_end_date: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    trade_date: row.get(6)?,
                    amount: row.get(7)?,
                    cost_basis: row.get(8)?,
                    realized_gain: row.get(9)?,
                    note: row.get(10)?,
                })
            },
        )
        .map_err(|_| "没有找到可核对的 Excel 历史赎回记录".to_string())?;
    if previous.original_end_date.is_empty() {
        return Err("该历史批次缺少原结束日期，暂时无法核对".to_string());
    }
    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let transaction_note = note
        .as_deref()
        .map(|value| format!("历史赎回已核对；{value}"))
        .unwrap_or_else(|| "历史赎回已核对".to_string());
    let realized_gain = input.proceeds - previous.original_cost;
    let redemption_key = historical_redemption_key(
        &previous.product_code,
        &previous.institution,
        &previous.currency,
        previous.original_cost,
        &previous.original_end_date,
    );
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    tx.execute(
        "INSERT INTO historical_redemption_confirmations
           (redemption_key, source_row, product_code, institution, currency,
            original_cost, original_end_date, confirmed_trade_date, proceeds, note,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
         ON CONFLICT(redemption_key) DO UPDATE SET
           source_row = excluded.source_row,
           confirmed_trade_date = excluded.confirmed_trade_date,
           proceeds = excluded.proceeds,
           note = excluded.note,
           updated_at = excluded.updated_at",
        params![
            redemption_key,
            previous.source_row,
            previous.product_code,
            previous.institution,
            previous.currency,
            previous.original_cost,
            previous.original_end_date,
            trade_date,
            input.proceeds,
            note,
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE transactions SET trade_date = ?1, amount = ?2, cost_basis = ?3,
           realized_gain = ?4, note = ?5 WHERE id = ?6",
        params![
            trade_date,
            input.proceeds,
            previous.original_cost,
            realized_gain,
            transaction_note,
            input.transaction_id
        ],
    )
    .map_err(|error| error.to_string())?;
    record_master_change(
        &tx,
        "historical_redemption",
        input.transaction_id,
        serde_json::json!({
            "tradeDate": previous.trade_date,
            "proceeds": previous.amount,
            "costBasis": previous.cost_basis,
            "realizedGain": previous.realized_gain,
            "note": previous.note,
        }),
        serde_json::json!({
            "tradeDate": trade_date,
            "proceeds": input.proceeds,
            "costBasis": previous.original_cost,
            "realizedGain": realized_gain,
            "note": transaction_note,
        }),
    )?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(EntryResult {
        message: "历史赎回已核对，实际收益和 XIRR 已重新计算".to_string(),
        realized_gain: Some(realized_gain),
    })
}

fn transaction_editability(
    conn: &Connection,
    transaction_id: i64,
) -> Result<(bool, Option<String>), String> {
    let target: Option<(String, String, Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT transaction_type, source, reversed_by, source_row
             FROM transactions WHERE id = ?1",
            [transaction_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((operation, source, reversed_by, source_row)) = target else {
        return Ok((false, Some("没有找到该交易".to_string())));
    };
    if reversed_by.is_some() || operation == "REVERSAL" {
        return Ok((false, Some("已冲销流水不能再次修改".to_string())));
    }
    if source == "manual" {
        return if transaction_can_be_reversed(conn, transaction_id)? {
            Ok((true, None))
        } else {
            Ok((
                false,
                Some("该产品或存款存在更新的手工操作，请先修改最新一笔流水".to_string()),
            ))
        };
    }
    if source == "excel" && operation == "BUY" && source_row.is_some() {
        let allocated: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transaction_lot_allocations a
                 JOIN position_lots l ON l.id = a.lot_id
                 WHERE l.source = 'excel' AND l.source_row = ?1",
                [source_row],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        return if allocated == 0 {
            Ok((true, None))
        } else {
            Ok((
                false,
                Some("该导入批次已被后续手工卖出核销，请先处理后续卖出流水".to_string()),
            ))
        };
    }
    if source == "excel" && operation == "REDEEM" {
        return Ok((
            false,
            Some("历史赎回请使用“修改核对”调整日期和到账金额".to_string()),
        ));
    }
    Ok((
        false,
        Some("该导入流水需要在原 Excel 中修改后重新导入".to_string()),
    ))
}

fn update_transaction_valuation(
    tx: &Transaction<'_>,
    transaction_id: i64,
    trade_date: &str,
    market_value: f64,
) -> Result<(), String> {
    let changed = tx
        .execute(
            "UPDATE valuations SET valuation_date = ?1, market_value = ?2
             WHERE transaction_id = ?3",
            params![trade_date, market_value, transaction_id],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("该流水缺少关联的市值快照，无法安全修改".to_string());
    }
    Ok(())
}

fn apply_excel_buy_edit(
    tx: &Transaction<'_>,
    input: &TransactionEditInput,
    trade_date: &str,
    note: &Option<String>,
) -> Result<EntryResult, String> {
    let target = tx
        .query_row(
            "SELECT t.source_row, t.trade_date, t.amount, t.note,
                    t.product_id, t.account_id, t.currency,
                    COALESCE(p.import_code, p.code),
                    COALESCE(a.import_institution, a.institution),
                    l.id, l.status
             FROM transactions t
             JOIN products p ON p.id = t.product_id
             JOIN accounts a ON a.id = t.account_id
             JOIN position_lots l ON l.source = 'excel' AND l.source_row = t.source_row
               AND l.product_id = t.product_id AND l.account_id = t.account_id
             WHERE t.id = ?1 AND t.source = 'excel' AND t.transaction_type = 'BUY'",
            [input.transaction_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )
        .map_err(|_| "没有找到可修改的 Excel 买入流水".to_string())?;
    let (
        source_row,
        old_date,
        old_amount,
        old_note,
        product,
        account,
        currency,
        code,
        institution,
        lot_id,
        lot_status,
    ) = target;
    let allocated: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM transaction_lot_allocations WHERE lot_id = ?1",
            [lot_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if allocated > 0 {
        return Err("该导入批次已被后续手工卖出核销，请先处理后续卖出流水".to_string());
    }

    let old_closed_group: Option<(f64, String)> = if lot_status == "closed" {
        tx.query_row(
            "SELECT SUM(original_amount), MAX(end_date) FROM position_lots
             WHERE source = 'excel' AND status = 'closed'
               AND product_id = ?1 AND account_id = ?2",
            params![product, account],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
    } else {
        None
    };

    let correction_key = excel_transaction_correction_key("BUY", source_row);
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    tx.execute(
        "INSERT INTO excel_transaction_corrections
           (correction_key, source_row, operation, trade_date, amount, note, created_at, updated_at)
         VALUES (?1, ?2, 'BUY', ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(correction_key) DO UPDATE SET
           trade_date = excluded.trade_date, amount = excluded.amount,
           note = excluded.note, updated_at = excluded.updated_at",
        params![
            correction_key,
            source_row,
            trade_date,
            input.amount,
            note,
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE transactions SET trade_date = ?1, amount = ?2, cost_basis = ?2,
           note = ?3 WHERE id = ?4",
        params![trade_date, input.amount, note, input.transaction_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE position_lots SET purchase_date = ?1, original_amount = ?2,
           remaining_amount = CASE WHEN status = 'active' THEN ?2 ELSE 0 END
         WHERE id = ?3",
        params![trade_date, input.amount, lot_id],
    )
    .map_err(|error| error.to_string())?;

    if let Some((old_group_cost, end_date)) = old_closed_group {
        let new_group_cost: f64 = tx
            .query_row(
                "SELECT SUM(original_amount) FROM position_lots
                 WHERE source = 'excel' AND status = 'closed'
                   AND product_id = ?1 AND account_id = ?2",
                params![product, account],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        tx.execute(
            "UPDATE transactions SET cost_basis = ?1, realized_gain = amount - ?1
             WHERE source = 'excel' AND transaction_type = 'REDEEM'
               AND product_id = ?2 AND account_id = ?3",
            params![new_group_cost, product, account],
        )
        .map_err(|error| error.to_string())?;
        let old_key =
            historical_redemption_key(&code, &institution, &currency, old_group_cost, &end_date);
        let new_key =
            historical_redemption_key(&code, &institution, &currency, new_group_cost, &end_date);
        tx.execute(
            "UPDATE historical_redemption_confirmations
             SET redemption_key = ?1, original_cost = ?2, updated_at = ?3
             WHERE redemption_key = ?4",
            params![new_key, new_group_cost, now, old_key],
        )
        .map_err(|error| error.to_string())?;
    }

    record_master_change(
        tx,
        "transaction",
        input.transaction_id,
        serde_json::json!({"tradeDate": old_date, "amount": old_amount, "note": old_note}),
        serde_json::json!({"tradeDate": trade_date, "amount": input.amount, "note": note}),
    )?;
    Ok(EntryResult {
        message: "Excel 买入流水已修正；以后重新导入时会自动沿用".to_string(),
        realized_gain: None,
    })
}

fn apply_manual_transaction_edit(
    tx: &Transaction<'_>,
    input: &TransactionEditInput,
    trade_date: &str,
    note: &Option<String>,
) -> Result<EntryResult, String> {
    let target = tx
        .query_row(
            "SELECT transaction_type, product_id, account_id, deposit_id,
                    trade_date, amount, cost_basis, realized_gain, note,
                    valuation_before, valuation_after
             FROM transactions WHERE id = ?1 AND source = 'manual'",
            [input.transaction_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, f64>(6)?,
                    row.get::<_, f64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<f64>>(9)?,
                    row.get::<_, Option<f64>>(10)?,
                ))
            },
        )
        .map_err(|_| "没有找到可修改的手工流水".to_string())?;
    let (
        operation,
        product,
        account,
        deposit,
        old_date,
        old_amount,
        old_cost,
        old_gain,
        old_note,
        before,
        old_after,
    ) = target;
    let (next_cost, next_gain, next_after) = match operation.as_str() {
        "BUY" => {
            let previous = before.ok_or_else(|| "该买入缺少操作前市值".to_string())?;
            let next_after = previous + input.amount;
            let changed = tx
                .execute(
                    "UPDATE position_lots SET purchase_date = ?1, original_amount = ?2,
                       remaining_amount = ?2, status = 'active'
                     WHERE transaction_id = ?3",
                    params![trade_date, input.amount, input.transaction_id],
                )
                .map_err(|error| error.to_string())?;
            if changed == 0 {
                return Err("该买入缺少关联批次，无法安全修改".to_string());
            }
            update_transaction_valuation(tx, input.transaction_id, trade_date, next_after)?;
            (input.amount, 0.0, Some(next_after))
        }
        "SELL" | "PRODUCT_MATURITY" => {
            let product = product.ok_or_else(|| "该卖出缺少产品关联".to_string())?;
            let account = account.ok_or_else(|| "该卖出缺少账户关联".to_string())?;
            let market = before.ok_or_else(|| "该卖出缺少操作前市值".to_string())?;
            let allocations = {
                let mut statement = tx
                    .prepare(
                        "SELECT lot_id, allocated_cost FROM transaction_lot_allocations
                         WHERE transaction_id = ?1",
                    )
                    .map_err(|error| error.to_string())?;
                let rows = statement
                    .query_map([input.transaction_id], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
                    })
                    .map_err(|error| error.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                rows
            };
            for (lot_id, allocated_cost) in allocations {
                tx.execute(
                    "UPDATE position_lots
                     SET remaining_amount = MIN(original_amount, remaining_amount + ?1),
                         status = 'active' WHERE id = ?2",
                    params![allocated_cost, lot_id],
                )
                .map_err(|error| error.to_string())?;
            }
            let available =
                active_cost(tx, product, account, trade_date).map_err(|error| error.to_string())?;
            if available <= 0.000_001 {
                return Err("修改后的日期没有可卖出的持仓成本".to_string());
            }
            let full = operation == "PRODUCT_MATURITY" || input.amount >= market * 0.999_999;
            if !full && input.amount > market {
                return Err("部分卖出的到账金额不能超过操作前市值".to_string());
            }
            let next_cost = if full {
                available
            } else {
                available * input.amount / market
            };
            let next_gain = input.amount - next_cost;
            let next_after = if full {
                0.0
            } else {
                (market - input.amount).max(0.0)
            };
            let allocations = consume_fifo_cost(tx, product, account, trade_date, next_cost)
                .map_err(|error| error.to_string())?;
            save_allocations(tx, input.transaction_id, &allocations)
                .map_err(|error| error.to_string())?;
            update_transaction_valuation(tx, input.transaction_id, trade_date, next_after)?;
            (next_cost, next_gain, Some(next_after))
        }
        "VALUATION" => {
            update_transaction_valuation(tx, input.transaction_id, trade_date, input.amount)?;
            (0.0, 0.0, Some(input.amount))
        }
        "DIVIDEND" => (0.0, input.amount, old_after),
        "FEE" => (0.0, -input.amount, old_after),
        "TRANSFER" => (0.0, 0.0, old_after),
        "DEPOSIT_OPEN" => {
            let deposit = deposit.ok_or_else(|| "该流水缺少存款关联".to_string())?;
            tx.execute(
                "UPDATE deposits SET principal = ?1, start_date = ?2 WHERE id = ?3",
                params![input.amount, trade_date, deposit],
            )
            .map_err(|error| error.to_string())?;
            (input.amount, 0.0, old_after)
        }
        "DEPOSIT_MATURITY" => {
            let deposit = deposit.ok_or_else(|| "该流水缺少存款关联".to_string())?;
            let principal: f64 = tx
                .query_row(
                    "SELECT principal FROM deposits WHERE id = ?1",
                    [deposit],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            tx.execute(
                "UPDATE deposits SET matured_at = ?1, proceeds = ?2 WHERE id = ?3",
                params![trade_date, input.amount, deposit],
            )
            .map_err(|error| error.to_string())?;
            (principal, input.amount - principal, old_after)
        }
        _ => return Err("该交易类型不支持修改".to_string()),
    };

    tx.execute(
        "UPDATE transactions SET trade_date = ?1, amount = ?2, note = ?3,
           cost_basis = ?4, realized_gain = ?5, valuation_after = ?6
         WHERE id = ?7",
        params![
            trade_date,
            input.amount,
            note,
            next_cost,
            next_gain,
            next_after,
            input.transaction_id
        ],
    )
    .map_err(|error| error.to_string())?;
    record_master_change(
        tx,
        "transaction",
        input.transaction_id,
        serde_json::json!({
            "tradeDate": old_date, "amount": old_amount, "costBasis": old_cost,
            "realizedGain": old_gain, "note": old_note, "valuationAfter": old_after,
        }),
        serde_json::json!({
            "tradeDate": trade_date, "amount": input.amount, "costBasis": next_cost,
            "realizedGain": next_gain, "note": note, "valuationAfter": next_after,
        }),
    )?;
    Ok(EntryResult {
        message: "交易流水已修改，相关持仓、收益和市值已重新计算".to_string(),
        realized_gain: matches!(
            operation.as_str(),
            "SELL" | "PRODUCT_MATURITY" | "DEPOSIT_MATURITY" | "DIVIDEND" | "FEE"
        )
        .then_some(next_gain),
    })
}

fn apply_transaction_edit(
    conn: &mut Connection,
    input: &TransactionEditInput,
) -> Result<EntryResult, String> {
    let trade_date = valid_date(&input.trade_date, "操作日期")?;
    if !input.amount.is_finite() || input.amount <= 0.0 {
        return Err("金额必须是大于0的有效数字".to_string());
    }
    let (can_edit, hint) = transaction_editability(conn, input.transaction_id)?;
    if !can_edit {
        return Err(hint.unwrap_or_else(|| "该交易当前不能修改".to_string()));
    }
    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let source: String = tx
        .query_row(
            "SELECT source FROM transactions WHERE id = ?1",
            [input.transaction_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let result = if source == "excel" {
        apply_excel_buy_edit(&tx, input, &trade_date, &note)?
    } else {
        apply_manual_transaction_edit(&tx, input, &trade_date, &note)?
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(result)
}

fn record_master_change(
    tx: &Transaction<'_>,
    entity_type: &str,
    entity_id: i64,
    before: serde_json::Value,
    after: serde_json::Value,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO master_data_changes
           (entity_type, entity_id, changed_at, before_json, after_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            entity_type,
            entity_id,
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            before.to_string(),
            after.to_string()
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn apply_master_data_update(
    conn: &mut Connection,
    input: &MasterDataUpdate,
) -> Result<String, String> {
    let entity = input.entity_type.trim().to_lowercase();
    let name = input.name.trim();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let currency = input.currency.trim().to_uppercase();
    if !matches!(currency.as_str(), "CNY" | "USD") {
        return Err("币种必须是 CNY 或 USD".to_string());
    }
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let message = match entity.as_str() {
        "account" => {
            let institution = input.institution.as_deref().unwrap_or_default().trim();
            if institution.is_empty() {
                return Err("银行/机构不能为空".to_string());
            }
            let before: (String, String, String, String) = tx
                .query_row(
                    "SELECT institution, name, currency, source FROM accounts WHERE id = ?1",
                    [input.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(|_| "没有找到该账户".to_string())?;
            let linked: i64 = tx
                .query_row(
                    "SELECT
                       (SELECT COUNT(*) FROM transactions WHERE account_id = ?1 OR transfer_account_id = ?1) +
                       (SELECT COUNT(*) FROM position_lots WHERE account_id = ?1) +
                       (SELECT COUNT(*) FROM valuations WHERE account_id = ?1) +
                       (SELECT COUNT(*) FROM deposits WHERE account_id = ?1) +
                       (SELECT COUNT(*) FROM account_balance_snapshots WHERE account_id = ?1)",
                    [input.id],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if linked > 0 && before.2 != currency {
                return Err("已有账本记录的账户不能修改币种".to_string());
            }
            tx.execute(
                "UPDATE accounts
                 SET institution = ?1, name = ?2, currency = ?3,
                     import_institution = CASE
                       WHEN source = 'excel' THEN COALESCE(import_institution, ?4)
                       ELSE import_institution END,
                     is_user_edited = 1
                 WHERE id = ?5",
                params![institution, name, currency, before.0, input.id],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE") {
                    "已有同名、同币种账户".to_string()
                } else {
                    error.to_string()
                }
            })?;
            record_master_change(
                &tx,
                "account",
                input.id,
                serde_json::json!({"institution": before.0, "name": before.1, "currency": before.2}),
                serde_json::json!({"institution": institution, "name": name, "currency": currency}),
            )?;
            "账户资料已更新".to_string()
        }
        "product" => {
            let code = input.code.as_deref().unwrap_or_default().trim();
            if code.is_empty() {
                return Err("产品代码不能为空".to_string());
            }
            let before: (String, String, String, Option<String>, Option<String>) = tx
                .query_row(
                    "SELECT code, name, currency, issuer, risk_level FROM products WHERE id = ?1",
                    [input.id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(|_| "没有找到该产品".to_string())?;
            let linked: i64 = tx
                .query_row(
                    "SELECT
                       (SELECT COUNT(*) FROM transactions WHERE product_id = ?1) +
                       (SELECT COUNT(*) FROM position_lots WHERE product_id = ?1) +
                       (SELECT COUNT(*) FROM valuations WHERE product_id = ?1)",
                    [input.id],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if linked > 0 && before.2 != currency {
                return Err("已有账本记录的产品不能修改币种".to_string());
            }
            let issuer = input
                .issuer
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            let risk = input
                .risk_level
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            tx.execute(
                "UPDATE products
                 SET code = ?1, name = ?2, currency = ?3, issuer = ?4, risk_level = ?5,
                     import_code = CASE WHEN source = 'excel' THEN COALESCE(import_code, ?6)
                                        ELSE import_code END,
                     is_user_edited = 1
                 WHERE id = ?7",
                params![code, name, currency, issuer, risk, before.0, input.id],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE") {
                    "产品代码已存在".to_string()
                } else {
                    error.to_string()
                }
            })?;
            record_master_change(
                &tx,
                "product",
                input.id,
                serde_json::json!({"code": before.0, "name": before.1, "currency": before.2, "issuer": before.3, "riskLevel": before.4}),
                serde_json::json!({"code": code, "name": name, "currency": currency, "issuer": issuer, "riskLevel": risk}),
            )?;
            "产品资料已更新".to_string()
        }
        "deposit" => {
            let account_id = input
                .account_id
                .ok_or_else(|| "请选择存款账户".to_string())?;
            let principal = positive_amount(input.principal, "存款本金")?;
            let maturity =
                valid_date(input.maturity_date.as_deref().unwrap_or_default(), "到期日")?;
            let start = input
                .start_date
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| valid_date(value, "起息日"))
                .transpose()?;
            let annual_rate = input.annual_rate.unwrap_or_default();
            if !(0.0..=1.0).contains(&annual_rate) {
                return Err("年利率必须在0%到100%之间".to_string());
            }
            let account_currency: String = tx
                .query_row(
                    "SELECT currency FROM accounts WHERE id = ?1",
                    [account_id],
                    |row| row.get(0),
                )
                .map_err(|_| "没有找到存款账户".to_string())?;
            if account_currency != currency {
                return Err("存款币种必须与所选账户一致".to_string());
            }
            let before: (
                i64,
                String,
                String,
                f64,
                Option<String>,
                String,
                f64,
                String,
            ) = tx
                .query_row(
                    "SELECT account_id, name, currency, principal, start_date, maturity_date,
                            annual_rate, status
                     FROM deposits WHERE id = ?1",
                    [input.id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                        ))
                    },
                )
                .map_err(|_| "没有找到该存款".to_string())?;
            if before.7 != "active"
                && (before.0 != account_id
                    || before.2 != currency
                    || (before.3 - principal).abs() > 0.000_001)
            {
                return Err("已结清存款只能修改名称、日期和利率备注资料".to_string());
            }
            tx.execute(
                "UPDATE deposits
                 SET account_id = ?1, name = ?2, currency = ?3, principal = ?4,
                     start_date = ?5, maturity_date = ?6, annual_rate = ?7,
                     is_user_edited = 1
                 WHERE id = ?8",
                params![
                    account_id,
                    name,
                    currency,
                    principal,
                    start,
                    maturity,
                    annual_rate,
                    input.id
                ],
            )
            .map_err(|error| error.to_string())?;
            tx.execute(
                "UPDATE transactions
                 SET account_id = ?1, amount = ?2, cost_basis = ?2, currency = ?3,
                     trade_date = COALESCE(?4, trade_date)
                 WHERE deposit_id = ?5 AND transaction_type = 'DEPOSIT_OPEN'
                   AND reversed_by IS NULL",
                params![account_id, principal, currency, start, input.id],
            )
            .map_err(|error| error.to_string())?;
            record_master_change(
                &tx,
                "deposit",
                input.id,
                serde_json::json!({"accountId": before.0, "name": before.1, "currency": before.2, "principal": before.3, "startDate": before.4, "maturityDate": before.5, "annualRate": before.6}),
                serde_json::json!({"accountId": account_id, "name": name, "currency": currency, "principal": principal, "startDate": start, "maturityDate": maturity, "annualRate": annual_rate}),
            )?;
            "定期存款资料已更新".to_string()
        }
        _ => return Err("不支持的资料类型".to_string()),
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(message)
}

fn xnpv(rate: f64, cash_flows: &[(NaiveDate, f64)]) -> f64 {
    let first = cash_flows[0].0;
    cash_flows
        .iter()
        .map(|(date, amount)| {
            let years = (*date - first).num_days() as f64 / 365.0;
            amount / (1.0 + rate).powf(years)
        })
        .sum()
}

fn calculate_xirr(cash_flows: &[(NaiveDate, f64)]) -> Option<f64> {
    if cash_flows.len() < 2
        || !cash_flows.iter().any(|(_, amount)| *amount < 0.0)
        || !cash_flows.iter().any(|(_, amount)| *amount > 0.0)
    {
        return None;
    }
    let mut low = -0.999_999;
    let mut high = 10.0;
    let mut low_value = xnpv(low, cash_flows);
    let mut high_value = xnpv(high, cash_flows);
    while low_value.signum() == high_value.signum() && high < 1_000_000.0 {
        high *= 10.0;
        high_value = xnpv(high, cash_flows);
    }
    if !low_value.is_finite()
        || !high_value.is_finite()
        || low_value.signum() == high_value.signum()
    {
        return None;
    }
    for _ in 0..160 {
        let mid = (low + high) / 2.0;
        let value = xnpv(mid, cash_flows);
        if value.abs() < 0.000_001 {
            return Some(mid);
        }
        if value.signum() == low_value.signum() {
            low = mid;
            low_value = value;
        } else {
            high = mid;
        }
    }
    Some((low + high) / 2.0)
}

fn transaction_cash_flow(operation: &str, amount: f64) -> Option<f64> {
    match operation {
        "BUY" | "DEPOSIT_OPEN" | "FEE" => Some(-amount),
        "SELL" | "REDEEM" | "PRODUCT_MATURITY" | "DEPOSIT_MATURITY" | "DIVIDEND" => Some(amount),
        _ => None,
    }
}

fn analytics_for_currency(
    conn: &Connection,
    currency: &str,
    today: NaiveDate,
) -> Result<CurrencyAnalytics, String> {
    let current_wealth: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(market_value), 0) FROM valuations
             WHERE is_current = 1 AND currency = ?1",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let current_deposits: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(principal), 0) FROM deposits
             WHERE status = 'active' AND currency = ?1",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let invested_cost: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(l.remaining_amount, l.original_amount)), 0)
             FROM position_lots l JOIN products p ON p.id = l.product_id
             WHERE l.status = 'active' AND p.currency = ?1",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let realized_gain: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(realized_gain), 0) FROM transactions
             WHERE currency = ?1 AND reversed_by IS NULL AND transaction_type != 'REVERSAL'",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let income: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM transactions
             WHERE currency = ?1 AND transaction_type = 'DIVIDEND' AND reversed_by IS NULL",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let fees: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM transactions
             WHERE currency = ?1 AND transaction_type = 'FEE' AND reversed_by IS NULL",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;

    let mut cash_flows = {
        let mut statement = conn
            .prepare(
                "SELECT trade_date, transaction_type, amount FROM transactions
                 WHERE currency = ?1 AND reversed_by IS NULL AND transaction_type != 'REVERSAL'
                 ORDER BY trade_date, id",
            )
            .map_err(|error| error.to_string())?;
        let flows = statement
            .query_map([currency], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .filter_map(|row| row.ok())
            .filter_map(|(date, operation, amount)| {
                Some((
                    NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok()?,
                    transaction_cash_flow(&operation, amount)?,
                ))
            })
            .collect::<Vec<_>>();
        flows
    };
    let terminal_deposits: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(d.principal), 0) FROM deposits d
             WHERE d.status = 'active' AND d.currency = ?1
               AND EXISTS (
                 SELECT 1 FROM transactions t
                 WHERE t.deposit_id = d.id AND t.transaction_type = 'DEPOSIT_OPEN'
                   AND t.reversed_by IS NULL
               )",
            [currency],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let terminal_value = current_wealth + terminal_deposits;
    if terminal_value > 0.000_001 {
        cash_flows.push((today, terminal_value));
    }
    cash_flows.sort_by_key(|(date, _)| *date);

    let first_this_month = today.with_day(1).unwrap_or(today);
    let mut trend = Vec::new();
    for offset in (0..12).rev() {
        let month_start = first_this_month
            .checked_sub_months(Months::new(offset))
            .unwrap_or(first_this_month);
        let month_end = month_start
            .checked_add_months(Months::new(1))
            .and_then(|date| date.checked_sub_signed(Duration::days(1)))
            .unwrap_or(today)
            .min(today);
        let date = month_end.format("%Y-%m-%d").to_string();
        let wealth: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(v.market_value), 0) FROM valuations v
                 WHERE v.currency = ?1 AND v.valuation_date <= ?2
                   AND v.id = (
                     SELECT v2.id FROM valuations v2
                     WHERE v2.product_id = v.product_id AND v2.account_id = v.account_id
                       AND v2.valuation_date <= ?2
                     ORDER BY v2.valuation_date DESC, v2.id DESC LIMIT 1
                   )",
                params![currency, date],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let deposits: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(principal), 0) FROM deposits
                 WHERE currency = ?1 AND status != 'cancelled'
                   AND (start_date IS NULL OR start_date <= ?2)
                   AND (matured_at IS NULL OR matured_at > ?2)",
                params![currency, date],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let cumulative_realized_gain: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(realized_gain), 0) FROM transactions
                 WHERE currency = ?1 AND trade_date <= ?2 AND reversed_by IS NULL
                   AND transaction_type != 'REVERSAL'",
                params![currency, date],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let month_prefix = month_start.format("%Y-%m").to_string();
        let month_flows = {
            let mut statement = conn
                .prepare(
                    "SELECT transaction_type, amount FROM transactions
                     WHERE currency = ?1 AND substr(trade_date, 1, 7) = ?2
                       AND reversed_by IS NULL AND transaction_type != 'REVERSAL'",
                )
                .map_err(|error| error.to_string())?;
            let total = statement
                .query_map(params![currency, month_prefix], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
                })
                .map_err(|error| error.to_string())?
                .filter_map(|row| row.ok())
                .filter_map(|(operation, amount)| transaction_cash_flow(&operation, amount))
                .sum();
            total
        };
        trend.push(TrendPoint {
            date,
            label: month_start.format("%m月").to_string(),
            asset_value: wealth + deposits,
            cumulative_realized_gain,
            net_cash_flow: month_flows,
        });
    }
    Ok(CurrencyAnalytics {
        currency: currency.to_string(),
        xirr: calculate_xirr(&cash_flows),
        current_value: current_wealth + current_deposits,
        unrealized_gain: current_wealth - invested_cost,
        realized_gain,
        income,
        fees,
        trend,
    })
}

#[tauri::command]
fn import_workbook(path: String, state: State<'_, AppState>) -> Result<ImportReport, String> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err("所选文件不存在".to_string());
    }
    let resolved_path = fs::canonicalize(path).map_err(|error| error.to_string())?;
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    create_auto_backup(&conn, &state.backup_dir, "pre-import")?;
    parse_workbook(&resolved_path, &mut conn)
}

#[tauri::command]
fn backup_database(
    path: String,
    state: State<'_, AppState>,
) -> Result<FileOperationResult, String> {
    let target = Path::new(&path);
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    backup_to_path(&conn, target)?;
    Ok(FileOperationResult {
        path,
        message: "数据库备份已完成".to_string(),
    })
}

#[tauri::command]
fn restore_database(
    path: String,
    state: State<'_, AppState>,
) -> Result<FileOperationResult, String> {
    let source = Path::new(&path);
    validate_backup(source)?;
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let safety_backup = create_auto_backup(&conn, &state.backup_dir, "pre-restore")?;
    conn.restore(MAIN_DB, source, None::<fn(rusqlite::backup::Progress)>)
        .map_err(|error| {
            format!(
                "恢复失败，原数据库已备份至 {}：{error}",
                safety_backup.display()
            )
        })?;
    migrate(&conn).map_err(|error| error.to_string())?;
    Ok(FileOperationResult {
        path,
        message: format!("数据库已恢复；恢复前快照保存在 {}", safety_backup.display()),
    })
}

#[tauri::command]
fn export_excel_file(
    path: String,
    state: State<'_, AppState>,
) -> Result<FileOperationResult, String> {
    let target = Path::new(&path);
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    export_excel(&conn, target)?;
    Ok(FileOperationResult {
        path,
        message: "Excel 数据包已导出".to_string(),
    })
}

#[tauri::command]
fn list_master_data(state: State<'_, AppState>) -> Result<MasterData, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    list_master_data_from_conn(&conn)
}

#[tauri::command]
fn update_master_data(
    input: MasterDataUpdate,
    state: State<'_, AppState>,
) -> Result<EntryResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    create_auto_backup(&conn, &state.backup_dir, "pre-master-edit")?;
    let message = apply_master_data_update(&mut conn, &input)?;
    Ok(EntryResult {
        message,
        realized_gain: None,
    })
}

#[tauri::command]
fn get_reconciliation_center(state: State<'_, AppState>) -> Result<ReconciliationCenter, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    reconciliation_center_from_conn(&conn)
}

#[tauri::command]
fn save_balance_snapshot(
    input: BalanceSnapshotInput,
    state: State<'_, AppState>,
) -> Result<EntryResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    create_auto_backup(&conn, &state.backup_dir, "pre-reconciliation")?;
    apply_balance_snapshot(&mut conn, &input)
}

#[tauri::command]
fn confirm_historical_redemption(
    input: HistoricalRedemptionInput,
    state: State<'_, AppState>,
) -> Result<EntryResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    create_auto_backup(&conn, &state.backup_dir, "pre-redemption-confirmation")?;
    apply_historical_redemption_confirmation(&mut conn, &input)
}

#[tauri::command]
fn get_analytics(state: State<'_, AppState>) -> Result<Analytics, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let today = Local::now().date_naive();
    let currencies = ["CNY", "USD"]
        .iter()
        .map(|currency| analytics_for_currency(&conn, currency, today))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Analytics {
        as_of_date: today.format("%Y-%m-%d").to_string(),
        currencies,
    })
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
                days_since_valuation: 0,
                seven_day_return: None,
                thirty_day_return: None,
                signal: String::new(),
                signal_label: String::new(),
                signal_reason: String::new(),
            })
        })
        .map_err(|error| error.to_string())?;
    let mut holdings = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    for holding in &mut holdings {
        let insight = holding_insight(
            &conn,
            holding.product_id,
            holding.account_id,
            &holding.currency,
            holding.market_value,
            holding.gain_rate,
            &holding.valuation_date,
        )?;
        holding.days_since_valuation = insight.days_since_valuation;
        holding.seven_day_return = insight.seven_day_return;
        holding.thirty_day_return = insight.thirty_day_return;
        holding.signal = insight.signal;
        holding.signal_label = insight.signal_label;
        holding.signal_reason = insight.signal_reason;
    }
    Ok(holdings)
}

#[tauri::command]
fn record_batch_valuations(
    input: BatchValuationInput,
    state: State<'_, AppState>,
) -> Result<BatchValuationResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    create_auto_backup(&conn, &state.backup_dir, "pre-batch-valuation")?;
    apply_batch_valuations(&mut conn, &input)
}

#[tauri::command]
fn get_holding_detail(
    product_id: i64,
    account_id: i64,
    state: State<'_, AppState>,
) -> Result<HoldingDetail, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let (name, code, channel, currency, cost, market_value, valuation_date): (
        String,
        String,
        String,
        String,
        f64,
        f64,
        String,
    ) = conn
        .query_row(
            "SELECT p.name, p.code, a.institution, v.currency,
                    COALESCE((
                      SELECT SUM(COALESCE(l.remaining_amount, l.original_amount))
                      FROM position_lots l
                      WHERE l.product_id = p.id AND l.account_id = a.id AND l.status = 'active'
                    ), 0),
                    v.market_value, v.valuation_date
             FROM valuations v
             JOIN products p ON p.id = v.product_id
             JOIN accounts a ON a.id = v.account_id
             WHERE v.product_id = ?1 AND v.account_id = ?2 AND v.is_current = 1",
            params![product_id, account_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .map_err(|_| "没有找到该持仓".to_string())?;
    let gain = market_value - cost;
    let gain_rate = if cost.abs() > f64::EPSILON {
        gain / cost
    } else {
        0.0
    };
    let insight = holding_insight(
        &conn,
        product_id,
        account_id,
        &currency,
        market_value,
        gain_rate,
        &valuation_date,
    )?;
    let history = {
        let mut statement = conn
            .prepare(
                "SELECT v.id, v.valuation_date, v.market_value,
                        CASE WHEN t.transaction_type = 'VALUATION' AND t.reversed_by IS NULL
                             THEN t.valuation_after - t.valuation_before END,
                        CASE WHEN t.transaction_type = 'VALUATION' AND t.reversed_by IS NULL
                                   AND t.valuation_before > 0.000001
                             THEN (t.valuation_after - t.valuation_before) / t.valuation_before END,
                        CASE WHEN t.transaction_type = 'VALUATION' AND t.reversed_by IS NOT NULL
                             THEN '已冲销估值'
                             WHEN t.transaction_type = 'VALUATION' THEN '手工估值'
                             WHEN v.source = 'excel' THEN 'Excel导入'
                             ELSE '交易调整' END
                 FROM valuations v
                 LEFT JOIN transactions t ON t.id = v.transaction_id
                 WHERE v.product_id = ?1 AND v.account_id = ?2
                 ORDER BY v.valuation_date, v.id",
            )
            .map_err(|error| error.to_string())?;
        let points = statement
            .query_map(params![product_id, account_id], |row| {
                Ok(ValuationHistoryPoint {
                    id: row.get(0)?,
                    date: row.get(1)?,
                    market_value: row.get(2)?,
                    change_amount: row.get(3)?,
                    change_rate: row.get(4)?,
                    source: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        points
    };
    Ok(HoldingDetail {
        product_id,
        account_id,
        name,
        code,
        channel,
        currency,
        cost,
        market_value,
        gain,
        gain_rate,
        valuation_date,
        days_since_valuation: insight.days_since_valuation,
        seven_day_return: insight.seven_day_return,
        thirty_day_return: insight.thirty_day_return,
        signal: insight.signal,
        signal_label: insight.signal_label,
        signal_reason: insight.signal_reason,
        history,
    })
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
                    CASE WHEN t.transaction_type = 'TRANSFER' THEN
                      COALESCE(a.institution, '未知账户') || ' → ' || COALESCE(ta.institution, '未知账户')
                    ELSE COALESCE(p.name, d.name, a.institution, '未命名交易') END AS title,
                    p.code,
                    CASE WHEN t.transaction_type = 'TRANSFER' THEN
                      COALESCE(a.institution, '未知账户') || ' → ' || COALESCE(ta.institution, '未知账户')
                    ELSE a.institution END,
                    t.transaction_type,
                    t.trade_date,
                    t.amount,
                    COALESCE(t.cost_basis, 0),
                    COALESCE(t.realized_gain, 0),
                    t.currency,
                    t.note,
                    t.source,
                    t.reversed_by,
                    t.reversal_of
             FROM transactions t
             LEFT JOIN products p ON p.id = t.product_id
             LEFT JOIN deposits d ON d.id = t.deposit_id
             LEFT JOIN accounts a ON a.id = t.account_id
             LEFT JOIN accounts ta ON ta.id = t.transfer_account_id
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
                institution: row.get(3)?,
                operation: row.get(4)?,
                trade_date: row.get(5)?,
                amount: row.get(6)?,
                cost_basis: row.get(7)?,
                realized_gain: row.get(8)?,
                currency: row.get(9)?,
                note: row.get(10)?,
                source: row.get(11)?,
                reversed_by: row.get(12)?,
                reversal_of: row.get(13)?,
                can_reverse: row.get::<_, String>(11)? == "manual"
                    && row.get::<_, Option<i64>>(12)?.is_none()
                    && row.get::<_, String>(4)? != "REVERSAL",
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_transaction_detail(
    transaction_id: i64,
    state: State<'_, AppState>,
) -> Result<TransactionDetail, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    let can_reverse = transaction_can_be_reversed(&conn, transaction_id)?;
    let (can_edit, edit_hint) = transaction_editability(&conn, transaction_id)?;
    conn.query_row(
        "SELECT t.id,
                CASE WHEN t.transaction_type = 'TRANSFER' THEN
                  COALESCE(a.institution, '未知账户') || ' → ' || COALESCE(ta.institution, '未知账户')
                ELSE COALESCE(p.name, d.name, a.institution, '未命名交易') END,
                p.code,
                CASE WHEN t.transaction_type = 'TRANSFER' THEN
                  COALESCE(a.institution, '未知账户') || ' → ' || COALESCE(ta.institution, '未知账户')
                ELSE a.institution END,
                t.transaction_type,
                t.trade_date,
                t.amount,
                COALESCE(t.cost_basis, 0),
                COALESCE(t.realized_gain, 0),
                t.currency,
                t.note,
                t.source,
                t.valuation_before,
                t.valuation_after,
                t.reversed_by,
                t.reversal_of
         FROM transactions t
         LEFT JOIN products p ON p.id = t.product_id
         LEFT JOIN deposits d ON d.id = t.deposit_id
         LEFT JOIN accounts a ON a.id = t.account_id
         LEFT JOIN accounts ta ON ta.id = t.transfer_account_id
         WHERE t.id = ?1",
        [transaction_id],
        |row| {
            let operation: String = row.get(4)?;
            let note: Option<String> = row.get(10)?;
            let source: String = row.get(11)?;
            let reversed_by: Option<i64> = row.get(14)?;
            let can_confirm_historical_redemption = source == "excel"
                && operation == "REDEEM"
                && reversed_by.is_none();
            let needs_review = can_confirm_historical_redemption
                && note
                    .as_deref()
                    .is_some_and(|value| value.contains("待核对"));
            Ok(TransactionDetail {
                id: row.get(0)?,
                title: row.get(1)?,
                code: row.get(2)?,
                institution: row.get(3)?,
                operation,
                trade_date: row.get(5)?,
                amount: row.get(6)?,
                cost_basis: row.get(7)?,
                realized_gain: row.get(8)?,
                currency: row.get(9)?,
                note,
                source,
                valuation_before: row.get(12)?,
                valuation_after: row.get(13)?,
                reversed_by,
                reversal_of: row.get(15)?,
                can_reverse,
                can_edit,
                edit_hint,
                can_confirm_historical_redemption,
                needs_review,
            })
        },
    )
    .map_err(|_| "没有找到该交易".to_string())
}

#[tauri::command]
fn reverse_transaction(
    transaction_id: i64,
    state: State<'_, AppState>,
) -> Result<EntryResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    create_auto_backup(&conn, &state.backup_dir, "pre-reversal")?;
    reverse_entry(&mut conn, transaction_id)
}

#[tauri::command]
fn edit_transaction(
    input: TransactionEditInput,
    state: State<'_, AppState>,
) -> Result<EntryResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "数据库正在使用中".to_string())?;
    create_auto_backup(&conn, &state.backup_dir, "pre-transaction-edit")?;
    apply_transaction_edit(&mut conn, &input)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let backup_dir = data_dir.join("backups");
            fs::create_dir_all(&backup_dir)?;
            let mut conn = Connection::open(data_dir.join("wealth-manager.sqlite3"))?;
            migrate(&conn)?;
            if let Err(error) = ensure_daily_backup(&conn, &backup_dir) {
                eprintln!("daily backup failed: {error}");
            }
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
                backup_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_workbook,
            get_dashboard,
            list_holdings,
            record_batch_valuations,
            get_holding_detail,
            list_maturities,
            record_entry,
            list_transactions,
            get_transaction_detail,
            edit_transaction,
            reverse_transaction,
            backup_database,
            restore_database,
            export_excel_file,
            list_master_data,
            update_master_data,
            get_reconciliation_center,
            save_balance_snapshot,
            confirm_historical_redemption,
            get_analytics
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
        assert_eq!(valuation_date, Local::now().format("%Y-%m-%d").to_string());
    }

    fn entry(operation: &str, amount: f64) -> EntryInput {
        EntryInput {
            operation: operation.to_string(),
            product_id: None,
            account_id: None,
            transfer_account_id: None,
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

    fn temp_artifact(name: &str, extension: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("wealth-manager-{name}-{nonce}.{extension}"))
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
    fn editing_latest_manual_sale_recalculates_fifo_market_value_and_gain() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        apply_entry(&mut conn, &entry("BUY", 10_000.0)).expect("buy");
        let (product, account): (i64, i64) = conn
            .query_row(
                "SELECT product_id, account_id FROM position_lots LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("holding ids");
        let mut valuation = entry("VALUATION", 0.0);
        valuation.product_id = Some(product);
        valuation.account_id = Some(account);
        valuation.market_value = Some(11_000.0);
        apply_entry(&mut conn, &valuation).expect("valuation");
        let mut sale = entry("SELL", 5_500.0);
        sale.product_id = Some(product);
        sale.account_id = Some(account);
        apply_entry(&mut conn, &sale).expect("sale");
        let sale_id: i64 = conn
            .query_row(
                "SELECT id FROM transactions WHERE transaction_type = 'SELL'",
                [],
                |row| row.get(0),
            )
            .expect("sale id");

        let result = apply_transaction_edit(
            &mut conn,
            &TransactionEditInput {
                transaction_id: sale_id,
                trade_date: "2026-08-07".to_string(),
                amount: 4_400.0,
                note: Some("银行流水修正".to_string()),
            },
        )
        .expect("edit sale");
        assert!((result.realized_gain.unwrap_or_default() - 400.0).abs() < 0.01);
        let (amount, cost, gain, after): (f64, f64, f64, f64) = conn
            .query_row(
                "SELECT amount, cost_basis, realized_gain, valuation_after
                 FROM transactions WHERE id = ?1",
                [sale_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("edited sale");
        assert!((amount - 4_400.0).abs() < 0.01);
        assert!((cost - 4_000.0).abs() < 0.01);
        assert!((gain - 400.0).abs() < 0.01);
        assert!((after - 6_600.0).abs() < 0.01);
        let remaining: f64 = conn
            .query_row(
                "SELECT remaining_amount FROM position_lots WHERE transaction_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("remaining lot");
        assert!((remaining - 6_000.0).abs() < 0.01);
        let current_market: f64 = conn
            .query_row(
                "SELECT market_value FROM valuations WHERE is_current = 1",
                [],
                |row| row.get(0),
            )
            .expect("current market");
        assert!((current_market - 6_600.0).abs() < 0.01);
    }

    #[test]
    fn editing_older_manual_transaction_is_blocked_by_later_activity() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        apply_entry(&mut conn, &entry("BUY", 10_000.0)).expect("first buy");
        let first_id: i64 = conn
            .query_row("SELECT MIN(id) FROM transactions", [], |row| row.get(0))
            .expect("first transaction");
        apply_entry(&mut conn, &entry("BUY", 5_000.0)).expect("second buy");
        let error = apply_transaction_edit(
            &mut conn,
            &TransactionEditInput {
                transaction_id: first_id,
                trade_date: "2026-08-07".to_string(),
                amount: 9_000.0,
                note: None,
            },
        )
        .expect_err("older buy must be blocked");
        assert!(error.contains("最新一笔"));
    }

    #[test]
    fn excel_buy_correction_survives_reimport() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("initial import");
        let (transaction_id, source_row, trade_date, amount): (i64, i64, String, f64) = conn
            .query_row(
                "SELECT t.id, t.source_row, t.trade_date, t.amount
                 FROM transactions t JOIN position_lots l
                   ON l.source = 'excel' AND l.source_row = t.source_row
                 WHERE t.source = 'excel' AND t.transaction_type = 'BUY'
                   AND l.status = 'active'
                 ORDER BY t.source_row LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("editable excel buy");
        apply_transaction_edit(
            &mut conn,
            &TransactionEditInput {
                transaction_id,
                trade_date: trade_date.clone(),
                amount: amount + 123.0,
                note: Some("软件内修正".to_string()),
            },
        )
        .expect("edit imported buy");
        let report = parse_workbook(&workbook, &mut conn).expect("reimport");
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("已保留软件中的手工修正")));
        let (saved_amount, saved_note): (f64, Option<String>) = conn
            .query_row(
                "SELECT amount, note FROM transactions
                 WHERE source = 'excel' AND transaction_type = 'BUY' AND source_row = ?1",
                [source_row],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("reimported corrected buy");
        assert!((saved_amount - amount - 123.0).abs() < 0.01);
        assert_eq!(saved_note.as_deref(), Some("软件内修正"));
    }

    #[test]
    fn editing_expired_excel_buy_updates_grouped_redemption_cost() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("initial import");
        let (transaction_id, trade_date, amount): (i64, String, f64) = conn
            .query_row(
                "SELECT t.id, t.trade_date, t.amount
                 FROM transactions t JOIN products p ON p.id = t.product_id
                 WHERE t.source = 'excel' AND t.transaction_type = 'BUY'
                   AND p.code = 'AF247644G' ORDER BY t.source_row LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("expired excel buy");
        apply_transaction_edit(
            &mut conn,
            &TransactionEditInput {
                transaction_id,
                trade_date,
                amount: amount + 100.0,
                note: Some("历史成本修正".to_string()),
            },
        )
        .expect("edit expired buy");
        let cost_after_edit: f64 = conn
            .query_row(
                "SELECT t.cost_basis FROM transactions t
                 JOIN products p ON p.id = t.product_id
                 WHERE t.transaction_type = 'REDEEM' AND p.code = 'AF247644G'",
                [],
                |row| row.get(0),
            )
            .expect("updated redemption cost");
        assert!((cost_after_edit - 10_100.0).abs() < 0.01);
        parse_workbook(&workbook, &mut conn).expect("reimport");
        let cost_after_reimport: f64 = conn
            .query_row(
                "SELECT t.cost_basis FROM transactions t
                 JOIN products p ON p.id = t.product_id
                 WHERE t.transaction_type = 'REDEEM' AND p.code = 'AF247644G'",
                [],
                |row| row.get(0),
            )
            .expect("preserved redemption cost");
        assert!((cost_after_reimport - 10_100.0).abs() < 0.01);
    }

    #[test]
    fn reimport_preserves_manual_sale_and_manual_valuation() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("first import");

        let (product, account, institution, currency): (i64, i64, String, String) = conn
            .query_row(
                "SELECT v.product_id, v.account_id, a.institution, a.currency
                 FROM valuations v JOIN accounts a ON a.id = v.account_id
                 WHERE v.is_current = 1 AND v.market_value > 1000
                 LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("imported holding");
        let mut buy = entry("BUY", 10_000.0);
        buy.product_id = Some(product);
        buy.account_id = Some(account);
        buy.institution = Some(institution);
        buy.currency = Some(currency);
        apply_entry(&mut conn, &buy).expect("manual buy");

        let mut sale = entry("SELL", 1_000.0);
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

    #[test]
    fn reversals_restore_fifo_cost_market_value_and_deposit_state() {
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
            .expect("holding ids");
        let mut valuation = entry("VALUATION", 0.0);
        valuation.product_id = Some(product);
        valuation.account_id = Some(account);
        valuation.market_value = Some(16_500.0);
        apply_entry(&mut conn, &valuation).expect("valuation");
        let mut sale = entry("SELL", 5_500.0);
        sale.product_id = Some(product);
        sale.account_id = Some(account);
        apply_entry(&mut conn, &sale).expect("sale");
        let sale_id: i64 = conn
            .query_row(
                "SELECT id FROM transactions WHERE transaction_type = 'SELL'",
                [],
                |row| row.get(0),
            )
            .expect("sale id");
        assert!(transaction_can_be_reversed(&conn, sale_id).expect("reversible sale"));
        reverse_entry(&mut conn, sale_id).expect("reverse sale");
        let (cost, market): (f64, f64) = conn
            .query_row(
                "SELECT (SELECT SUM(remaining_amount) FROM position_lots WHERE status = 'active'),
                        (SELECT market_value FROM valuations WHERE is_current = 1)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("restored holding");
        assert!((cost - 15_000.0).abs() < 0.01);
        assert!((market - 16_500.0).abs() < 0.01);

        let mut open = entry("DEPOSIT_OPEN", 100_000.0);
        open.product_name = Some("测试定存".to_string());
        open.maturity_date = Some("2029-08-07".to_string());
        open.annual_rate = Some(0.02);
        apply_entry(&mut conn, &open).expect("open deposit");
        let deposit: i64 = conn
            .query_row("SELECT id FROM deposits", [], |row| row.get(0))
            .expect("deposit id");
        let mut maturity = entry("DEPOSIT_MATURITY", 106_000.0);
        maturity.deposit_id = Some(deposit);
        apply_entry(&mut conn, &maturity).expect("mature deposit");
        let maturity_id: i64 = conn
            .query_row(
                "SELECT id FROM transactions WHERE transaction_type = 'DEPOSIT_MATURITY'",
                [],
                |row| row.get(0),
            )
            .expect("maturity id");
        reverse_entry(&mut conn, maturity_id).expect("reverse maturity");
        let status: String = conn
            .query_row(
                "SELECT status FROM deposits WHERE id = ?1",
                [deposit],
                |row| row.get(0),
            )
            .expect("deposit status");
        assert_eq!(status, "active");
    }

    #[test]
    fn database_backup_restores_a_consistent_snapshot() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        apply_entry(&mut conn, &entry("BUY", 10_000.0)).expect("first buy");
        let backup_path = temp_artifact("backup", "sqlite3");
        backup_to_path(&conn, &backup_path).expect("create backup");
        validate_backup(&backup_path).expect("validate backup");
        apply_entry(&mut conn, &entry("BUY", 5_000.0)).expect("second buy");
        conn.restore(
            MAIN_DB,
            &backup_path,
            None::<fn(rusqlite::backup::Progress)>,
        )
        .expect("restore backup");
        let lots: i64 = conn
            .query_row("SELECT COUNT(*) FROM position_lots", [], |row| row.get(0))
            .expect("lot count");
        assert_eq!(lots, 1);
        fs::remove_file(backup_path).expect("remove test backup");
    }

    #[test]
    fn excel_export_contains_auditable_sheets_and_formulas() {
        let workbook_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook_path, &mut conn).expect("import workbook");
        let preserved_export = std::env::var("WEALTH_MANAGER_TEST_EXPORT_PATH").ok();
        let export_path = preserved_export
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| temp_artifact("export", "xlsx"));
        export_excel(&conn, &export_path).expect("export workbook");

        let mut exported = open_workbook_auto(&export_path).expect("open exported workbook");
        let names = exported.sheet_names().to_vec();
        assert_eq!(
            names,
            vec![
                "资产概览",
                "当前持仓",
                "交易流水",
                "定期存款",
                "买入批次",
                "市值历史"
            ]
        );
        let holdings = exported.worksheet_range("当前持仓").expect("holding sheet");
        assert_eq!(text(holdings.get((2, 0))), "产品名称");
        assert!(holdings.height() > 20);
        let formulas = exported
            .worksheet_formula("资产概览")
            .expect("summary formulas");
        assert!(formulas
            .rows()
            .flatten()
            .any(|formula| formula.contains("SUMIF")));
        if preserved_export.is_none() {
            fs::remove_file(export_path).expect("remove test export");
        }
    }

    #[test]
    fn dividend_fee_and_transfer_are_auditable_and_reversible() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        apply_entry(&mut conn, &entry("BUY", 10_000.0)).expect("buy");
        let (product, source_account): (i64, i64) = conn
            .query_row(
                "SELECT product_id, account_id FROM position_lots LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("holding ids");
        let target_account = {
            let tx = conn.transaction().expect("account transaction");
            let id = manual_account_id(&tx, "另一家银行", "CNY").expect("target account");
            tx.commit().expect("commit target account");
            id
        };

        let mut dividend = entry("DIVIDEND", 120.0);
        dividend.product_id = Some(product);
        dividend.account_id = Some(source_account);
        apply_entry(&mut conn, &dividend).expect("dividend");
        let dividend_id: i64 = conn
            .query_row(
                "SELECT id FROM transactions WHERE transaction_type = 'DIVIDEND'",
                [],
                |row| row.get(0),
            )
            .expect("dividend id");

        let mut fee = entry("FEE", 20.0);
        fee.product_id = Some(product);
        fee.account_id = Some(source_account);
        apply_entry(&mut conn, &fee).expect("fee");
        let fee_id: i64 = conn
            .query_row(
                "SELECT id FROM transactions WHERE transaction_type = 'FEE'",
                [],
                |row| row.get(0),
            )
            .expect("fee id");
        assert!(!transaction_can_be_reversed(&conn, dividend_id).expect("older event"));
        assert!(transaction_can_be_reversed(&conn, fee_id).expect("latest event"));
        reverse_entry(&mut conn, fee_id).expect("reverse fee");
        assert!(transaction_can_be_reversed(&conn, dividend_id).expect("dividend now latest"));

        let mut transfer = entry("TRANSFER", 500.0);
        transfer.account_id = Some(source_account);
        transfer.transfer_account_id = Some(target_account);
        apply_entry(&mut conn, &transfer).expect("transfer");
        let transfer_id: i64 = conn
            .query_row(
                "SELECT id FROM transactions WHERE transaction_type = 'TRANSFER'",
                [],
                |row| row.get(0),
            )
            .expect("transfer id");
        assert!(transaction_can_be_reversed(&conn, transfer_id).expect("transfer reversible"));
        reverse_entry(&mut conn, transfer_id).expect("reverse transfer");
        let effective_gain: f64 = conn
            .query_row(
                "SELECT SUM(realized_gain) FROM transactions
                 WHERE reversed_by IS NULL AND transaction_type != 'REVERSAL'",
                [],
                |row| row.get(0),
            )
            .expect("effective gain");
        assert!((effective_gain - 120.0).abs() < 0.01);
    }

    #[test]
    fn edited_imported_master_data_survives_reimport() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("first import");
        let product: ProductRecord = {
            let data = list_master_data_from_conn(&conn).expect("master data");
            data.products.into_iter().next().expect("product")
        };
        apply_master_data_update(
            &mut conn,
            &MasterDataUpdate {
                entity_type: "product".to_string(),
                id: product.id,
                name: "手工修正产品名".to_string(),
                code: Some(product.code.clone()),
                institution: None,
                account_id: None,
                currency: product.currency.clone(),
                issuer: Some("测试发行方".to_string()),
                risk_level: Some("R2".to_string()),
                principal: None,
                start_date: None,
                maturity_date: None,
                annual_rate: None,
            },
        )
        .expect("edit product");
        parse_workbook(&workbook, &mut conn).expect("second import");
        let name: String = conn
            .query_row(
                "SELECT name FROM products WHERE id = ?1",
                [product.id],
                |row| row.get(0),
            )
            .expect("preserved product");
        assert_eq!(name, "手工修正产品名");
        let audit_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM master_data_changes", [], |row| {
                row.get(0)
            })
            .expect("audit rows");
        assert_eq!(audit_rows, 1);
    }

    #[test]
    fn xirr_matches_a_simple_one_year_return() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).expect("start date");
        let end = NaiveDate::from_ymd_opt(2026, 1, 1).expect("end date");
        let result = calculate_xirr(&[(start, -1_000.0), (end, 1_100.0)]).expect("xirr");
        assert!((result - 0.1).abs() < 0.000_001);
    }

    #[test]
    fn batch_valuations_update_all_holdings_and_measure_flow_adjusted_returns() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        apply_entry(&mut conn, &entry("BUY", 10_000.0)).expect("first holding");
        let mut second_buy = entry("BUY", 5_000.0);
        second_buy.product_name = Some("第二个测试理财".to_string());
        second_buy.product_code = Some("TEST002".to_string());
        second_buy.institution = Some("另一家测试银行".to_string());
        apply_entry(&mut conn, &second_buy).expect("second holding");
        let holdings = {
            let mut statement = conn
                .prepare(
                    "SELECT product_id, account_id, original_amount
                     FROM position_lots ORDER BY original_amount DESC",
                )
                .expect("prepare holdings");
            let result = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, f64>(2)?,
                    ))
                })
                .expect("query holdings")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect holdings");
            result
        };
        let today = Local::now().format("%Y-%m-%d").to_string();
        let first = BatchValuationInput {
            valuation_date: today.clone(),
            items: holdings
                .iter()
                .map(|(product, account, cost)| ValuationUpdateItem {
                    product_id: *product,
                    account_id: *account,
                    market_value: cost * 1.01,
                })
                .collect(),
            note: Some("第一次周度估值".to_string()),
        };
        let result = apply_batch_valuations(&mut conn, &first).expect("first batch");
        assert_eq!(result.updated, 2);
        let second = BatchValuationInput {
            valuation_date: today,
            items: holdings
                .iter()
                .map(|(product, account, cost)| ValuationUpdateItem {
                    product_id: *product,
                    account_id: *account,
                    market_value: cost * 1.02,
                })
                .collect(),
            note: None,
        };
        apply_batch_valuations(&mut conn, &second).expect("second batch");
        let valuation_transactions: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions WHERE transaction_type = 'VALUATION'",
                [],
                |row| row.get(0),
            )
            .expect("valuation count");
        assert_eq!(valuation_transactions, 4);
        let current_valuations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM valuations WHERE is_current = 1",
                [],
                |row| row.get(0),
            )
            .expect("current valuation count");
        assert_eq!(current_valuations, 2);
        let return_rate = compounded_valuation_return(
            &conn,
            holdings[0].0,
            holdings[0].1,
            Local::now()
                .date_naive()
                .checked_sub_signed(Duration::days(30))
                .expect("lookback"),
        )
        .expect("return calculation")
        .expect("return exists");
        assert!((return_rate - 0.02).abs() < 0.000_001);
    }

    #[test]
    fn account_reconciliation_records_snapshots_and_detects_differences() {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        apply_entry(&mut conn, &entry("BUY", 10_000.0)).expect("buy");
        let initial = reconciliation_center_from_conn(&conn).expect("initial center");
        let account = initial
            .accounts
            .iter()
            .find(|item| item.tracked_total > 0.0)
            .expect("tracked account");
        assert_eq!(account.status, "missing");
        let account_id = account.account_id;
        let tracked_total = account.tracked_total;
        let input = BalanceSnapshotInput {
            account_id,
            balance_date: "2026-08-09".to_string(),
            actual_balance: tracked_total,
            note: Some("首次核对".to_string()),
        };
        apply_balance_snapshot(&mut conn, &input).expect("matched snapshot");
        let matched = reconciliation_center_from_conn(&conn).expect("matched center");
        assert_eq!(
            matched
                .accounts
                .iter()
                .find(|item| item.account_id == account_id)
                .expect("matched account")
                .status,
            "matched"
        );
        let changed = BalanceSnapshotInput {
            actual_balance: tracked_total + 88.0,
            ..input
        };
        apply_balance_snapshot(&mut conn, &changed).expect("updated snapshot");
        let different = reconciliation_center_from_conn(&conn).expect("different center");
        let account = different
            .accounts
            .iter()
            .find(|item| item.account_id == account_id)
            .expect("different account");
        assert_eq!(account.status, "difference");
        assert!((account.difference.unwrap_or_default() - 88.0).abs() < 0.01);
        let snapshots: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM account_balance_snapshots",
                [],
                |row| row.get(0),
            )
            .expect("snapshot count");
        assert_eq!(snapshots, 1);
    }

    #[test]
    fn data_quality_center_surfaces_imported_records_that_need_completion() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("import workbook");
        let center = reconciliation_center_from_conn(&conn).expect("quality center");
        assert!(center.issue_count > 20);
        assert_eq!(center.high_priority_count, 0);
        assert!(!center
            .issues
            .iter()
            .any(|issue| issue.category == "历史流水"));
        assert!(center
            .issues
            .iter()
            .any(|issue| issue.category == "定期存款"));
        assert!(center
            .issues
            .iter()
            .any(|issue| issue.category == "产品资料"));
        assert!(center
            .issues
            .iter()
            .any(|issue| issue.category == "账户对账"));
    }

    #[test]
    fn imports_sheet2_historical_redemptions_as_grouped_actual_proceeds() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("import workbook");
        let (count, proceeds, cost, gain, pending): (i64, f64, f64, f64, i64) = conn
            .query_row(
                "SELECT COUNT(*), SUM(amount), SUM(cost_basis), SUM(realized_gain),
                        SUM(CASE WHEN note LIKE '%待核对%' THEN 1 ELSE 0 END)
                 FROM transactions WHERE transaction_type = 'REDEEM' AND source = 'excel'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("historical redemption totals");
        assert_eq!(count, 13);
        assert!((proceeds - 93_867.01).abs() < 0.01);
        assert!((cost - 93_000.0).abs() < 0.01);
        assert!((gain - 867.01).abs() < 0.01);
        assert_eq!(pending, 0);
        let (af_amount, af_cost): (f64, f64) = conn
            .query_row(
                "SELECT t.amount, t.cost_basis FROM transactions t
                 JOIN products p ON p.id = t.product_id
                 WHERE t.transaction_type = 'REDEEM' AND p.code = 'AF247644G'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("grouped two-lot redemption");
        assert!((af_amount - 10_078.0).abs() < 0.01);
        assert!((af_cost - 10_000.0).abs() < 0.01);
    }

    #[test]
    fn historical_redemption_confirmation_updates_returns_and_survives_reimport() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("first import");
        let before = reconciliation_center_from_conn(&conn).expect("quality before");
        let historical_before = before
            .issues
            .iter()
            .filter(|issue| issue.category == "历史流水")
            .count();
        assert_eq!(historical_before, 0);
        let (transaction_id, source_row, original_cost, original_date): (i64, i64, f64, String) =
            conn.query_row(
                "SELECT id, source_row, cost_basis, trade_date
                 FROM transactions WHERE transaction_type = 'REDEEM'
                   AND source = 'excel' LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("imported historical redemption");
        let proceeds = original_cost + 123.45;
        let confirmed_date = NaiveDate::parse_from_str(&original_date, "%Y-%m-%d")
            .expect("original date")
            .checked_add_signed(Duration::days(1))
            .expect("next day")
            .format("%Y-%m-%d")
            .to_string();
        let result = apply_historical_redemption_confirmation(
            &mut conn,
            &HistoricalRedemptionInput {
                transaction_id,
                trade_date: confirmed_date.clone(),
                proceeds,
                note: Some("银行流水已核对".to_string()),
            },
        )
        .expect("confirm redemption");
        assert!((result.realized_gain.unwrap_or_default() - 123.45).abs() < 0.01);
        let after = reconciliation_center_from_conn(&conn).expect("quality after");
        assert_eq!(
            after
                .issues
                .iter()
                .filter(|issue| issue.category == "历史流水")
                .count(),
            historical_before
        );

        parse_workbook(&workbook, &mut conn).expect("second import");
        let (date, amount, cost, gain, note): (String, f64, f64, f64, String) = conn
            .query_row(
                "SELECT trade_date, amount, cost_basis, realized_gain, note
                 FROM transactions WHERE transaction_type = 'REDEEM'
                   AND source = 'excel' AND source_row = ?1",
                [source_row],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("reimported confirmed redemption");
        assert_eq!(date, confirmed_date);
        assert!((amount - proceeds).abs() < 0.01);
        assert!((cost - original_cost).abs() < 0.01);
        assert!((gain - 123.45).abs() < 0.01);
        assert!(note.contains("已核对"));
        assert!(!note.contains("待核对"));
        let confirmations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM historical_redemption_confirmations",
                [],
                |row| row.get(0),
            )
            .expect("confirmation count");
        assert_eq!(confirmations, 1);
    }

    #[test]
    fn migration_backfills_historical_redemption_cost_basis() {
        let workbook = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../理财.xlsx");
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrate(&conn).expect("migrate");
        parse_workbook(&workbook, &mut conn).expect("import workbook");
        conn.execute(
            "UPDATE transactions SET cost_basis = 0, realized_gain = 0
             WHERE transaction_type = 'REDEEM' AND source = 'excel'",
            [],
        )
        .expect("simulate legacy redemption data");
        migrate(&conn).expect("rerun migration");
        let missing_cost: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions
                 WHERE transaction_type = 'REDEEM' AND source = 'excel'
                   AND cost_basis <= 0.000001",
                [],
                |row| row.get(0),
            )
            .expect("missing cost count");
        assert_eq!(missing_cost, 0);
    }
}
