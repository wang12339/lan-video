#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="$SCRIPT_DIR/backend.pid"
LOG_FILE="$SCRIPT_DIR/backend.log"
PORT=8082
STOP_TIMEOUT=10
PROFILE_FILE="$SCRIPT_DIR/.build_profile"

# launchd 守护（开机自启 + 崩溃自动拉起）
LAUNCHD_LABEL="com.kuaile.atmos-backend"
PLIST_PATH="$HOME/Library/LaunchAgents/$LAUNCHD_LABEL.plist"

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

# ── 加载 .env ──
load_env() {
    if [[ -f "$SCRIPT_DIR/.env" ]]; then
        set -a
        # shellcheck disable=SC1091
        source "$SCRIPT_DIR/.env"
        set +a
    elif [[ -f "$SCRIPT_DIR/.env.example" ]]; then
        cp "$SCRIPT_DIR/.env.example" "$SCRIPT_DIR/.env"
    fi
}

# ── launchd 辅助 ──
agent_installed() { [[ -f "$PLIST_PATH" ]]; }
agent_loaded()    { launchctl print "gui/$(id -u)/$LAUNCHD_LABEL" >/dev/null 2>&1; }

write_plist() {
    mkdir -p "$(dirname "$PLIST_PATH")"
    cat > "$PLIST_PATH" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$LAUNCHD_LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>$SCRIPT_DIR/run_backend.command</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>StandardOutPath</key>
    <string>$LOG_FILE</string>
    <key>StandardErrorPath</key>
    <string>$LOG_FILE</string>
</dict>
</plist>
PLIST
}

bootstrap_agent() {
    launchctl bootout "gui/$(id -u)/$LAUNCHD_LABEL" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$(id -u)" "$PLIST_PATH" >/dev/null 2>&1 || true
}

# ── 确保 PostgreSQL 就绪（launchd 启动/手动启动共用） ──
ensure_postgres() {
    if pg_isready -q 2>/dev/null; then
        return 0
    fi
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
    fi
    if ! pg_isready -q 2>/dev/null; then
        error "PostgreSQL 启动失败"
        return 1
    fi
}

# ── 状态 ──
show_status() {
    local mode="手动"
    agent_loaded && mode="launchd 守护"
    if is_running; then
        local pids
        pids=$(port_pids)
        local bin
        bin=$(lsof -p "${pids%% *}" 2>/dev/null | awk '$4=="txt" && /atmos-video-backend/ {print $NF; exit}')
        local profile_label="?"
        case "$bin" in
            *target/release/*) profile_label="release" ;;
            *target/quick/*)   profile_label="quick" ;;
            *target/debug/*)   profile_label="debug" ;;
        esac
        echo -e "${GREEN}● 后端运行中${NC} (端口 $PORT, 进程: $pids, 模式: $mode, 二进制: $profile_label)"
    else
        echo -e "${RED}● 后端未运行${NC} (模式: $mode)"
    fi
    if agent_installed; then
        echo -e "  守护已安装: ${GREEN}是${NC} (开机自启 + 崩溃拉起)"
    else
        echo -e "  守护已安装: ${YELLOW}否${NC} (运行 '$0 install' 可启用)"
    fi
}

# ── 停止 ──
stop_backend() {
    local killed=false

    # 1) 若由 launchd 托管，先卸载作业（防止 KeepAlive 立刻拉起）
    if agent_loaded; then
        echo "正在停止 launchd 守护..."
        launchctl bootout "gui/$(id -u)/$LAUNCHD_LABEL" 2>/dev/null || true
        killed=true
    fi

    # 2) 通过 PID 文件停止（手动模式）
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

    # 3) 停止端口上的所有进程
    local pids
    pids=$(port_pids)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "正在停止端口 $PORT 上的进程 (进程 $pid)..."
            kill -TERM "$pid" 2>/dev/null || true
            killed=true
        done
    fi

    # 4) 等待后强制停止
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

# ── 构建后端（按需） ──
build_backend_if_needed() {
    local binary
    binary="$(build_target)"
    if [[ ! -x "$binary" ]] || find "$SCRIPT_DIR/src" -name "*.rs" -newer "$binary" -print -quit | grep -q .; then
        echo "正在构建后端 [$BUILD_PROFILE]..."
        # shellcheck disable=SC2046
        cargo $(cargo_args_for_profile) 2>&1 | tail -3
    fi
}

# ── 构建前端（按需） ──
build_frontend_if_needed() {
    local webapp_dir="$SCRIPT_DIR/../webapp"
    local dist_dir="$webapp_dir/dist"
    if [[ -d "$webapp_dir" ]]; then
        if [[ ! -d "$dist_dir" ]] || find "$webapp_dir/src" "$webapp_dir/index.html" "$webapp_dir/vite.config.ts" -newer "$dist_dir/index.html" -print -quit 2>/dev/null | grep -q .; then
            echo "正在构建前端..."
            (cd "$webapp_dir" && npm run build 2>&1 | tail -5)
        fi
    fi
}

# ── 启动 ──
start_backend() {
    load_env
    ensure_postgres || exit 1
    build_backend_if_needed
    build_frontend_if_needed

    # 统一走 launchd 守护：开机自启 + 崩溃拉起；plist 不存在则自动安装
    if ! agent_installed; then
        write_plist
    fi
    echo "正在通过 launchd 启动后端..."
    bootstrap_agent

    # 等待响应
    local waited=0
    while [[ $waited -lt 15 ]]; do
        if curl -s "http://localhost:$PORT/server/info" > /dev/null 2>&1 || is_running; then
            info "后端已启动 (端口 $PORT, 守护模式)"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    warn "启动超时，请查看日志: $LOG_FILE"
}

# ── 重启（保持当前守护/手动模式） ──
restart_backend() {
    build_backend_if_needed
    build_frontend_if_needed
    if agent_installed; then
        echo "正在重启 launchd 守护..."
        if agent_loaded; then
            launchctl kickstart -k "gui/$(id -u)/$LAUNCHD_LABEL"
        else
            bootstrap_agent
        fi
        sleep 2
        if is_running; then
            info "后端已重启 (端口 $PORT, 守护模式)"
            return 0
        fi
    fi
    # 未安装守护：手动模式
    stop_backend
    sleep 1
    start_backend
}

# ── 日志 ──
show_logs() {
    if [[ -f "$LOG_FILE" ]]; then
        tail -${1:-20} "$LOG_FILE"
    else
        warn "未找到日志文件"
    fi
}

# ── launchd 守护内部入口（由 plist 调用，勿手动执行） ──
daemon_main() {
    cd "$SCRIPT_DIR"
    export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

    load_env

    # 等待 PostgreSQL 就绪（开机时可能还在启动，最多等 90 秒）
    for _ in $(seq 1 90); do
        pg_isready -q 2>/dev/null && break
        sleep 1
    done

    # 选二进制：优先 .build_profile 指定的模式；若它比源码旧，
    # 则在 release/quick 中挑一个不比源码旧的；都过期则用选中者兜底
    # （保证服务可用，待 run_backend.command start 重建）。
    profile="quick"
    [[ -f "$PROFILE_FILE" ]] && profile="$(cat "$PROFILE_FILE")"
    selected="target/$profile/atmos-video-backend"

    is_fresh() { [[ -x "$1" ]] && ! find src -name '*.rs' -newer "$1" -print -quit 2>/dev/null | grep -q .; }

    bin="$selected"
    if ! is_fresh "$selected"; then
        for cand in target/release/atmos-video-backend target/quick/atmos-video-backend; do
            if is_fresh "$cand"; then
                bin="$cand"
                break
            fi
        done
    fi

    exec "./$bin"
}

# ── 安装守护（幂等） ──
install_agent() {
    write_plist
    bootstrap_agent
    info "已安装 launchd 守护 (开机自启 + 崩溃拉起)"
}

# ── 卸载守护 ──
uninstall_agent() {
    if agent_loaded; then
        launchctl bootout "gui/$(id -u)/$LAUNCHD_LABEL" 2>/dev/null || true
    fi
    if agent_installed; then
        rm -f "$PLIST_PATH"
    fi
    info "已卸载 launchd 守护（后端已停止，开机不再自启）"
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
    if agent_installed; then
        echo -e "  ${CYAN}6)${NC} 卸载守护 (关闭开机自启)"
    else
        echo -e "  ${CYAN}6)${NC} 安装守护 (开启开机自启)"
    fi
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
        info "构建模式已切换: $(profile_label) (下次启动生效)"
        read -p "按回车返回..."
        return
    done
}

# ── 交互模式 ──
interactive_mode() {
    while true; do
        show_menu
        read -p "请选择操作 [0-6]: " choice
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
                restart_backend
                ;;
            4)
                show_logs 30
                ;;
            5)
                select_profile
                ;;
            6)
                if agent_installed; then
                    uninstall_agent
                else
                    install_agent
                fi
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
    echo "  start        构建并启动后端（launchd 守护，自动安装 plist）"
    echo "  stop         停止后端（卸载守护作业；下次登录/start 恢复）"
    echo "  restart      重建并重启后端"
    echo "  status       查看运行状态与守护安装情况"
    echo "  logs [N]     查看最近 N 行日志"
    echo "  install      安装 launchd 守护（开机自启 + 崩溃拉起）"
    echo "  uninstall    卸载守护并停止后端"
    echo "  daemon       内部入口（由 launchd 调用，请勿手动执行）"
    echo "  help         显示帮助"
    echo ""
    echo "构建模式 (start/restart 时可选, 会记忆到 .build_profile):"
    echo "  quick  (默认) thin LTO, 编译快 3-5 倍, 性能接近 release"
    echo "  release        full LTO, 最佳性能, 编译最慢"
    echo "  debug          最快编译, 仅开发调试"
    echo ""
    echo "示例:"
    echo "  $0 restart release   # 用 release 模式重建并重启"
    echo "  $0 start quick       # 用 quick 模式启动"
    echo ""
}

# ── 主程序 ──
case "${1:-}" in
    start)
        if [[ -n "${2:-}" ]] && [[ "$2" == "quick" || "$2" == "release" || "$2" == "debug" ]]; then
            BUILD_PROFILE="$2"
            echo "$BUILD_PROFILE" > "$PROFILE_FILE"
        fi
        start_backend
        ;;
    stop)
        stop_backend
        ;;
    restart)
        if [[ -n "${2:-}" ]] && [[ "$2" == "quick" || "$2" == "release" || "$2" == "debug" ]]; then
            BUILD_PROFILE="$2"
            echo "$BUILD_PROFILE" > "$PROFILE_FILE"
        fi
        restart_backend
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs "${2:-20}"
        ;;
    install)
        install_agent
        ;;
    uninstall)
        uninstall_agent
        ;;
    daemon)
        daemon_main
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
