use calamine::{open_workbook_auto, Data, DataType, Reader};
use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
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
          source TEXT NOT NULL DEFAULT 'manual',
          source_row INTEGER,
          FOREIGN KEY(product_id) REFERENCES products(id),
          FOREIGN KEY(account_id) REFERENCES accounts(id)
        );

        CREATE TABLE IF NOT EXISTS position_lots (
          id INTEGER PRIMARY KEY,
          product_id INTEGER NOT NULL,
          account_id INTEGER NOT NULL,
          purchase_date TEXT NOT NULL,
          original_amount REAL NOT NULL,
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
    )
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

fn clear_excel_data(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM valuations WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM position_lots WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM transactions WHERE source = 'excel'", [])?;
    tx.execute("DELETE FROM deposits WHERE source = 'excel'", [])?;
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
            "INSERT INTO transactions (product_id, account_id, transaction_type, trade_date, amount, currency, note, source, source_row)
             VALUES (?1, ?2, 'BUY', ?3, ?4, ?5, 'Excel 导入', 'excel', ?6)",
            params![product, account, purchase_date.format("%Y-%m-%d").to_string(), amount, currency, (index + 1) as i64],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO position_lots (product_id, account_id, purchase_date, original_amount, end_date, status, source, source_row)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'excel', ?7)",
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
                "INSERT INTO transactions (product_id, account_id, transaction_type, trade_date, amount, currency, note, source, source_row)
                 VALUES (?1, ?2, 'REDEEM', ?3, ?4, ?5, '根据到期日期生成；赎回金额待核对', 'excel', ?6)",
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
        tx.execute(
            "INSERT INTO valuations (product_id, account_id, valuation_date, market_value, currency, is_current, source, source_row)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, 'excel', ?6)",
            params![product, account, as_of, market_value.unwrap_or_default(), currency, (index + 1) as i64],
        )
        .map_err(|error| error.to_string())?;
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
                "SELECT COALESCE(SUM(principal), 0) FROM deposits WHERE currency = ?1",
                params![currency],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let invested_cost: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(original_amount), 0) FROM position_lots l
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
            "SELECT COUNT(*) FROM valuations WHERE is_current = 1",
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
            "SELECT p.id, p.name, p.code, v.currency, a.institution,
                    COALESCE(SUM(l.original_amount), 0) AS cost,
                    v.market_value, v.valuation_date
             FROM valuations v
             JOIN products p ON p.id = v.product_id
             JOIN accounts a ON a.id = v.account_id
             LEFT JOIN position_lots l ON l.product_id = p.id AND l.status = 'active'
             WHERE v.is_current = 1
             GROUP BY p.id, p.name, p.code, v.currency, a.institution, v.market_value, v.valuation_date
             ORDER BY v.currency, v.market_value DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let cost: f64 = row.get(5)?;
            let market_value: f64 = row.get(6)?;
            let gain = market_value - cost;
            Ok(Holding {
                id: row.get(0)?,
                name: row.get(1)?,
                code: row.get(2)?,
                currency: row.get(3)?,
                channel: row.get(4)?,
                cost,
                market_value,
                gain,
                gain_rate: if cost.abs() > f64::EPSILON {
                    gain / cost
                } else {
                    0.0
                },
                valuation_date: row.get(7)?,
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
            list_maturities
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
}
