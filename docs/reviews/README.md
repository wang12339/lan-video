# 历史评审归档（Historical Reviews）

本目录是项目历史安全 / 代码评审与渗透测试的产出，**仅作留档参考**，不代表
当前实现：

- **多租户功能已完整移除**（提交 `339d36c refactor: 彻底移除多租户与套餐功能`，
  迁移 `057_remove_multi_tenancy.sql`）。本文档目录中与租户隔离、租户路由、
  `tenant_id` 相关的描述均已过时。
- `pentest-output/` 为一次渗透测试的证据与复现脚本；其中的二进制样本
  （mp4/bin）已从仓库移除。`poc_media_bypass.py` 仅用于历史复现，相关问题
  已修复，不应再视为有效漏洞。
- `*.png` 截图与 `*.json` 报告是当时环境的快照，UI / 接口可能与现状不符。
- 部署、鉴权、限流等实现此后又经过多轮加固（见 git 历史与
  `backend/migrations/058-060`）。

当前状态请以以下来源为准：

- `docs/SECURITY.md`、`docs/DEPLOYMENT.md`、`docs/API.md`
- `backend/src/openapi.rs` 由 utoipa 自动生成的 OpenAPI 文档（`GET /docs`）
- `backend/migrations/` 的迁移历史
