# 稳盈

面向个人银行理财与定期存款的本地优先桌面应用。数据保存在本机 SQLite 数据库中，原始 Excel 文件只读导入，不会被修改。

## 当前阶段

第一阶段已完成：

- Tauri 2 + React + TypeScript 桌面工程
- SQLite 数据库及幂等迁移
- 导入现有 `理财.xlsx` 的三个工作表
- 买入批次、当前持仓、定期存款结构化存储
- 产品币种冲突与缺失字段校验
- 资产总览、持仓列表、到期日历
- macOS 本地构建与真实工作簿导入测试

第二阶段计划：

- 手工新增买入、赎回、分红和市值更新
- 编辑产品、账户和存款
- 数据备份、恢复与 CSV/Excel 导出
- 已实现收益与 XIRR

## 开发命令

```bash
pnpm install
pnpm tauri dev
```

前端生产构建：

```bash
pnpm build
```

Rust 测试：

```bash
cd src-tauri
cargo test
```

桌面二进制构建：

```bash
pnpm tauri build --debug --no-bundle
```

## 数据位置

应用首次启动时在 macOS 应用数据目录创建 `wealth-manager.sqlite3`。重新导入 Excel 时，仅替换上一次由 Excel 生成的记录；后续手工数据不会被导入流程删除。

## 数据表

- `accounts`：购买渠道和银行账户
- `products`：理财产品主数据
- `transactions`：买入、赎回、分红等交易流水
- `position_lots`：分批买入和持有状态
- `valuations`：按日期保存的产品市值
- `deposits`：定期存款合同
- `import_runs`：导入历史与校验警告
