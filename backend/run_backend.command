#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="$SCRIPT_DIR/backend.pid"
LOG_FILE="$SCRIPT_DIR/backend.log"
PORT=8082
STOP_TIMEOUT=10

cd "$SCRIPT_DIR"

# ── Find PIDs listening on our port ──
port_pids() {
    lsof -i "tcp:$PORT" -sTCP:LISTEN -t 2>/dev/null || true
}

# ── Stop ──
stop_backend() {
    local killed=false

    # 1) Kill by PID file (process ID of the binary, not cargo)
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(<"$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            echo "Stopping backend (pid $pid)..."
            kill -TERM "$pid" 2>/dev/null || true
            killed=true
        fi
        rm -f "$PID_FILE"
    fi

    # 2) Kill anything still listening on the port
    local pids
    pids=$(port_pids)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "Stopping process on port $PORT (pid $pid)..."
            kill -TERM "$pid" 2>/dev/null || true
            killed=true
        done
    fi

    # 3) Wait then force-kill
    if $killed; then
        local waited=0
        while true; do
            local remaining
            remaining=$(port_pids)
            [[ -z "$remaining" ]] && break
            if [[ $waited -ge $STOP_TIMEOUT ]]; then
                echo "Timeout, force-killing..."
                for pid in $remaining; do
                    kill -KILL "$pid" 2>/dev/null || true
                done
                sleep 1
                break
            fi
            sleep 0.5
            waited=$((waited + 1))
        done
        echo "Backend stopped."
    else
        echo "Backend is not running."
    fi
}

# ── Start ──
start_backend() {
    # Check .env
    if [[ ! -f "$SCRIPT_DIR/.env" ]]; then
        if [[ -f "$SCRIPT_DIR/.env.example" ]]; then
            cp "$SCRIPT_DIR/.env.example" "$SCRIPT_DIR/.env"
            echo "[OK] Created .env from .env.example (edit if needed)"
        fi
    fi

    # Check PostgreSQL
    if ! pg_isready -q 2>/dev/null; then
        echo "Starting PostgreSQL..."
        brew services start postgresql@16 2>/dev/null || true
        sleep 3
        if ! pg_isready -q 2>/dev/null; then
            echo "ERROR: Failed to start PostgreSQL."
            echo "Run manually: brew services start postgresql@16"
            exit 1
        fi
    fi
    echo "[OK] PostgreSQL"

    # Check Rust
    if ! command -v cargo &>/dev/null; then
        echo "ERROR: Cargo not found. Install from https://rustup.rs/"
        exit 1
    fi
    echo "[OK] Rust toolchain"

    # Build if needed
    local binary="$SCRIPT_DIR/target/debug/lan-video-backend"
    if [[ ! -x "$binary" ]] || [[ "$binary" -ot "$SCRIPT_DIR/src" ]]; then
        echo "Building project..."
        cargo build 2>&1 | tail -3
    fi
    echo "[OK] Build"

    # Start the binary directly (not through cargo, so PID management is clean)
    echo "Starting backend..."
    nohup "$binary" > "$LOG_FILE" 2>&1 &
    local pid=$!
    echo "$pid" > "$PID_FILE"

    # Wait for server to respond
    local waited=0
    while [[ $waited -lt 15 ]]; do
        if curl -s http://localhost:$PORT/server/info > /dev/null 2>&1; then
            echo "[OK] Backend is running on port $PORT"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    echo "WARNING: Timed out waiting for backend. Check $LOG_FILE"
    echo "Last 10 log lines:"
    tail -10 "$LOG_FILE" 2>/dev/null || true
}

# ── Main ──
if [[ -n "$(port_pids)" ]]; then
    echo "Backend is already running. Stopping..."
    stop_backend
else
    start_backend
fi
