#!/usr/bin/env bash
set -e

# ── 颜色 ──
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}[INFO]${NC} $1"; }
ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
err()   { echo -e "${RED}[ERR]${NC} $1"; }

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

# ── 1. 检查 PostgreSQL ──
if command -v pg_isready &>/dev/null && pg_isready -q 2>/dev/null; then
    ok "PostgreSQL 已运行"
else
    warn "PostgreSQL 未运行，尝试启动..."
    if command -v brew &>/dev/null && brew services list 2>/dev/null | grep -q postgresql; then
        brew services start postgresql 2>/dev/null || brew services start postgresql@16 2>/dev/null || true
        sleep 2
    fi
    if command -v pg_ctl &>/dev/null; then
        pg_ctl start -D /usr/local/var/postgres 2>/dev/null || pg_ctl start -D ~/Library/Application\ Support/Postgres/var-16 2>/dev/null || true
        sleep 2
    fi
    if ! pg_isready -q 2>/dev/null; then
        err "PostgreSQL 无法启动，请手动启动后重试"
        exit 1
    fi
    ok "PostgreSQL 已启动"
fi

# ── 2. 创建数据库 ──
DB_NAME="lan_video"
if psql -U "$(whoami)" -d "$DB_NAME" -c "SELECT 1" &>/dev/null; then
    ok "数据库 '$DB_NAME' 已存在"
else
    info "创建数据库 '$DB_NAME'..."
    createdb "$DB_NAME" 2>/dev/null && ok "数据库已创建" || warn "数据库创建失败（可能已有）"
fi

# ── 3. 启动 Rust 后端 ──
info "启动 Rust 后端 (http://0.0.0.0:8082)..."
cd "$ROOT/backend"
cargo run &
BACKEND_PID=$!
cd "$ROOT"

# 等待后端就绪
for i in $(seq 1 15); do
    if curl -sf http://localhost:8082/health >/dev/null 2>&1; then
        ok "后端已就绪 (PID $BACKEND_PID)"
        break
    fi
    sleep 1
done

if ! kill -0 $BACKEND_PID 2>/dev/null; then
    err "后端启动失败，检查日志"
    exit 1
fi

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  后端 API:  http://localhost:8082${NC}"
echo -e "${GREEN}  Web 前端:  http://localhost:8082/webapp/${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo ""
info "按 Ctrl+C 停止所有服务"

# ── 5. 等待并清理 ──
trap "echo ''; info '正在停止...'; kill $BACKEND_PID 2>/dev/null;  wait; ok '已停止'" EXIT INT TERM
wait
