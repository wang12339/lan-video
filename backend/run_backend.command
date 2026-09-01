#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="$SCRIPT_DIR/backend.pid"
LOG_FILE="$SCRIPT_DIR/backend.log"
PORT=8082
STOP_TIMEOUT=10
PROFILE_FILE="$SCRIPT_DIR/.build_profile"

cd "$SCRIPT_DIR"

# ── 构建模式(quick=推荐/release=最佳性能/debug=最快编译) ──
BUILD_PROFILE="${BUILD_PROFILE:-}"
if [[ -z "$BUILD_PROFILE" ]]; then
    BUILD_PROFILE="quick"
    [[ -f "$PROFILE_FILE" ]] && BUILD_PROFILE="$(cat "$PROFILE_FILE")"
fi
case "$BUILD_PROFILE" in
    quick|release|debug) ;;
    *) BUILD_PROFILE="quick" ;;
esac

# 根据构建模式解析二进制路径与 cargo 参数
build_target() {
    case "$BUILD_PROFILE" in
        debug)
            echo "$SCRIPT_DIR/target/debug/atmos-video-backend"
            ;;
        quick)
            echo "$SCRIPT_DIR/target/quick/atmos-video-backend"
            ;;
        release)
            echo "$SCRIPT_DIR/target/release/atmos-video-backend"
            ;;
    esac
}

cargo_args_for_profile() {
    case "$BUILD_PROFILE" in
        debug)
            echo "build"
            ;;
        quick)
            echo "build --profile quick"
            ;;
        release)
            echo "build --release"
            ;;
    esac
}

# ── 颜色 ──
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${GREEN}[完成]${NC} $1"; }
warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[错误]${NC} $1"; }

# ── 查找监听端口的进程 ──
port_pids() {
    lsof -i "tcp:$PORT" -sTCP:LISTEN -t 2>/dev/null || true
}

# ── 检查是否运行 ──
is_running() {
    [[ -n "$(port_pids)" ]]
}

# ── 状态 ──
show_status() {
    if is_running; then
        local pids
        pids=$(port_pids)
        echo -e "${GREEN}● 后端运行中${NC} (端口 $PORT, 进程: $pids)"
    else
        echo -e "${RED}● 后端未运行${NC}"
    fi
}

# ── 停止 ──
stop_backend() {
    local killed=false

    # 1) 通过 PID 文件停止
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(<"$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            echo "正在停止后端 (进程 $pid)..."
            kill -TERM "$pid" 2>/dev/null || true
            killed=true
        fi
        rm -f "$PID_FILE"
    fi

    # 2) 停止端口上的所有进程
    local pids
    pids=$(port_pids)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "正在停止端口 $PORT 上的进程 (进程 $pid)..."
            kill -TERM "$pid" 2>/dev/null || true
            killed=true
        done
    fi

    # 3) 等待后强制停止
    if $killed; then
        local waited=0
        while true; do
            local remaining
            remaining=$(port_pids)
            [[ -z "$remaining" ]] && break
            if [[ $waited -ge $STOP_TIMEOUT ]]; then
                echo "超时，正在强制停止..."
                for pid in $remaining; do
                    kill -KILL "$pid" 2>/dev/null || true
                done
                sleep 1
                break
            fi
            sleep 0.5
            waited=$((waited + 1))
        done
        info "后端已停止"
    else
        warn "后端未运行"
    fi
}

# ── 启动 ──
start_backend() {
    # 加载 .env 文件
    if [[ -f "$SCRIPT_DIR/.env" ]]; then
        set -a
        source "$SCRIPT_DIR/.env"
        set +a
    elif [[ -f "$SCRIPT_DIR/.env.example" ]]; then
        cp "$SCRIPT_DIR/.env.example" "$SCRIPT_DIR/.env"
    fi

    # 检查 PostgreSQL
    if ! pg_isready -q 2>/dev/null; then
        echo "正在启动 PostgreSQL..."
        brew services start postgresql@16 2>/dev/null || true
        sleep 3
        if ! pg_isready -q 2>/dev/null; then
            local pg_data="/opt/homebrew/var/postgresql@16"
            if [[ -f "$pg_data/postmaster.pid" ]]; then
                rm -f "$pg_data/postmaster.pid"
                /opt/homebrew/opt/postgresql@16/bin/pg_ctl -D "$pg_data" start -l /tmp/postgresql.log 2>&1 || true
                sleep 3
            fi
            if ! pg_isready -q 2>/dev/null; then
                error "PostgreSQL 启动失败"
                exit 1
            fi
        fi
    fi

    # 构建后端
    local binary
    binary="$(build_target)"
    if [[ ! -x "$binary" ]] || find "$SCRIPT_DIR/src" -name "*.rs" -newer "$binary" -print -quit | grep -q .; then
        echo "正在构建后端 [$BUILD_PROFILE]..."
        # shellcheck disable=SC2046
        cargo $(cargo_args_for_profile) 2>&1 | tail -3
    fi

    # 构建前端
    local webapp_dir="$SCRIPT_DIR/../webapp"
    local dist_dir="$webapp_dir/dist"
    if [[ -d "$webapp_dir" ]]; then
        if [[ ! -d "$dist_dir" ]] || find "$webapp_dir/src" "$webapp_dir/index.html" "$webapp_dir/vite.config.ts" -newer "$dist_dir/index.html" -print -quit 2>/dev/null | grep -q .; then
            echo "正在构建前端..."
            (cd "$webapp_dir" && npm run build 2>&1 | tail -5)
        fi
    fi

    # 设置环境变量（已在 .env 中定义，此处为后备默认值）
    export DATABASE_URL="${DATABASE_URL:-postgres://kuaile@localhost:5432/atmos_video}"
    export MEDIA_ROOT="${MEDIA_ROOT:-$SCRIPT_DIR/media}"
    export RUST_LOG="${RUST_LOG:-info}"
    export SERVER_PORT="${SERVER_PORT:-$PORT}"
    export PUBLIC_URL="${PUBLIC_URL:-}"
    export CORS_ORIGIN="${CORS_ORIGIN:-}"
    export COOKIE_SECURE="${COOKIE_SECURE:-true}"
    export APP_ENV="${APP_ENV:-production}"
    export UPLOAD_QUOTA_BYTES="${UPLOAD_QUOTA_BYTES:-0}"
    export ALLOW_FIRST_USER_ADMIN="${ALLOW_FIRST_USER_ADMIN:-false}"

    # 启动
    echo "正在启动后端..."
    nohup "$binary" > "$LOG_FILE" 2>&1 &
    local pid=$!
    echo "$pid" > "$PID_FILE"

    # 等待响应
    local waited=0
    while [[ $waited -lt 15 ]]; do
        if curl -s "http://localhost:$PORT/server/info" > /dev/null 2>&1; then
            info "后端已启动 (端口 $PORT)"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    warn "启动超时，请查看日志: $LOG_FILE"
}

# ── 日志 ──
show_logs() {
    if [[ -f "$LOG_FILE" ]]; then
        tail -${1:-20} "$LOG_FILE"
    else
        warn "未找到日志文件"
    fi
}

# ── 菜单 ──
profile_label() {
    case "$BUILD_PROFILE" in
        debug)   echo "debug  (最快编译)" ;;
        quick)   echo "quick  (推荐: 快编译+接近 release 性能)" ;;
        release) echo "release (最佳性能, 编译最慢)" ;;
    esac
}

show_menu() {
    clear
    echo -e "${CYAN}╔════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║       Atmos 后端管理               ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════╝${NC}"
    echo ""
    show_status
    echo ""
    echo -e "  ${GREEN}1)${NC} 启动后端"
    echo -e "  ${RED}2)${NC} 停止后端"
    echo -e "  ${YELLOW}3)${NC} 重启后端"
    echo -e "  ${CYAN}4)${NC} 查看日志"
    echo -e "  ${CYAN}5)${NC} 构建模式: $(profile_label)"
    echo -e "  ${NC}0)${NC} 退出"
    echo ""
}

# ── 构建模式选择 ──
select_profile() {
    while true; do
        clear
        echo -e "${CYAN}构建模式选择${NC}"
        echo ""
        echo "  1) quick    - 推荐: thin LTO, 编译快 3-5 倍, 性能接近 release"
        echo "  2) release  - 最佳性能: full LTO, 编译最慢 (~1分钟+ 增量)"
        echo "  3) debug    - 最快编译, 性能最差, 仅开发调试"
        echo ""
        echo -e "  当前: ${GREEN}$(profile_label)${NC}"
        echo "  0) 返回"
        echo ""
        read -p "请选择 [0-3]: " choice
        case $choice in
            1) BUILD_PROFILE="quick" ;;
            2) BUILD_PROFILE="release" ;;
            3) BUILD_PROFILE="debug" ;;
            0) return ;;
            *) error "无效选择" ; continue ;;
        esac
        echo "$BUILD_PROFILE" > "$PROFILE_FILE"
        info "构建模式已切换: $(profile_label)"
        read -p "按回车返回..."
        return
    done
}

# ── 交互模式 ──
interactive_mode() {
    while true; do
        show_menu
        read -p "请选择操作 [0-5]: " choice
        echo ""
        case $choice in
            1)
                if is_running; then
                    warn "后端已在运行"
                else
                    start_backend
                fi
                ;;
            2)
                stop_backend
                ;;
            3)
                stop_backend
                sleep 1
                start_backend
                ;;
            4)
                show_logs 30
                ;;
            5)
                select_profile
                ;;
            0)
                echo "再见！"
                exit 0
                ;;
            *)
                error "无效选择"
                ;;
        esac
        echo ""
        read -p "按回车继续..."
    done
}

# ── 帮助 ──
usage() {
    echo "用法: $0 [命令] [构建模式]"
    echo ""
    echo "命令:"
    echo "  (无参数)     打开交互菜单"
    echo "  start        启动后端"
    echo "  stop         停止后端"
    echo "  restart      重启后端"
    echo "  status       查看运行状态"
    echo "  logs [N]     查看最近 N 行日志"
    echo "  help         显示帮助"
    echo ""
    echo "构建模式 (仅 start/restart, 可选):"
    echo "  quick  (默认) thin LTO, 编译快 3-5 倍, 性能接近 release"
    echo "  release        full LTO, 最佳性能, 编译最慢"
    echo "  debug          最快编译, 仅开发调试"
    echo ""
    echo "示例: $0 start quick    # 用 quick 模式启动"
    echo ""
}

# ── 主程序 ──
case "${1:-}" in
    start)
        # 支持 `start [quick|release|debug]`
        if [[ -n "${2:-}" ]] && [[ "$2" == "quick" || "$2" == "release" || "$2" == "debug" ]]; then
            BUILD_PROFILE="$2"
            echo "$BUILD_PROFILE" > "$PROFILE_FILE"
        fi
        if is_running; then
            warn "后端已在运行"
        else
            start_backend
        fi
        ;;
    stop)
        stop_backend
        ;;
    restart)
        if [[ -n "${2:-}" ]] && [[ "$2" == "quick" || "$2" == "release" || "$2" == "debug" ]]; then
            BUILD_PROFILE="$2"
            echo "$BUILD_PROFILE" > "$PROFILE_FILE"
        fi
        stop_backend
        sleep 1
        start_backend
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs "${2:-20}"
        ;;
    help|-h|--help)
        usage
        ;;
    "")
        interactive_mode
        ;;
    *)
        error "未知命令: $1"
        usage
        exit 1
        ;;
esac
