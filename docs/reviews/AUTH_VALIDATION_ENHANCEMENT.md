# Auth Input Validation Enhancement Report

## Current Implementation Analysis

### ✅ Strengths (Already Implemented)

1. **Email Validation** (`is_valid_email`):
   - Length validation (1-254 chars)
   - Local part length (1-64 chars)
   - Domain format validation (no leading/trailing dots, no consecutive dots)
   - Whitespace/control character rejection (prevents SMTP injection)

2. **Password Strength** (`is_password_strong_enough`):
   - Minimum 8 characters, maximum 128 characters
   - Dynamic requirements based on length:
     - < 12 chars: requires 3+ character categories (upper, lower, digit, special)
     - ≥ 12 chars: requires 2+ character categories

3. **Username Validation**:
   - Length: 2-64 characters
   - Control character rejection (prevents log injection)
   - Case-insensitive storage (lowercased)

4. **Brute Force Protection**:
   - IP rate limiting: 30 attempts/minute per IP (no block duration)
   - Username rate limiting: 5 attempts/minute, 5-minute block
   - Redis-backed with in-memory fallback
   - Atomic counter with Lua scripts

5. **Timing Attack Prevention**:
   - Dummy argon2 verification for non-existent users
   - Generic error messages (doesn't reveal if user exists)

---

## 🔧 Enhancement Recommendations

### 1. **Email Format Validation - Medium Priority**

**Current Issue**: Basic validation doesn't catch all edge cases

**Recommendations**:

```rust
// Add to is_valid_email()
fn is_valid_email(email: &str) -> bool {
    // ... existing checks ...

    // NEW: RFC 5321 compliant local part validation
    // Reject special characters that could cause issues
    if local.contains("..") {
        return false;
    }

    // NEW: Domain label validation
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        // Labels must start/end with alphanumeric
        if !label.chars().next().map_or(false, |c| c.is_alphanumeric()) ||
           !label.chars().last().map_or(false, |c| c.is_alphanumeric()) {
            return false;
        }
    }

    // NEW: Total domain length (max 253 chars per RFC)
    if domain.len() > 253 {
        return false;
    }

    // NEW: TLD validation (must be at least 2 chars, all alpha)
    if let Some(tld) = domain.split('.').last() {
        if tld.len() < 2 || !tld.chars().all(|c| c.is_alphabetic()) {
            return false;
        }
    }

    true
}
```

**Additional Suggestions**:
- Add disposable email domain blocking (optional, configurable)
- Consider using a dedicated email validation crate like `email_address` for production

---

### 2. **Password Strength Check - High Priority**

**Current Issue**: No check against common/leaked passwords

**Recommendations**:

```rust
// Add common password list (top 10,000 passwords)
const COMMON_PASSWORDS: &[&str] = &[
    "password", "123456", "12345678", "qwerty", "abc123",
    "monkey", "master", "dragon", "letmein", "login",
    "princess", "football", "shadow", "sunshine", "trustno1",
    "iloveyou", "batman", "access", "hello", "charlie",
    // ... expand to 10,000+
];

pub(crate) fn is_password_strong_enough(pw: &str) -> bool {
    // ... existing category checks ...

    // NEW: Check against common passwords (case-insensitive)
    let lower_pw = pw.to_lowercase();
    if COMMON_PASSWORDS.iter().any(|&common| lower_pw.contains(common)) {
        return false;
    }

    // NEW: Reject passwords containing username (if provided)
    // This requires passing username as parameter

    // NEW: Check for keyboard patterns
    let keyboard_patterns = ["qwerty", "asdfgh", "zxcvbn", "123456", "abcdef"];
    let lower_pw = pw.to_lowercase();
    if keyboard_patterns.iter().any(|&pattern| lower_pw.contains(pattern)) {
        return false;
    }

    // NEW: Check for repeated characters (e.g., "aaaaaa", "111111")
    let has_repeated = pw.chars().collect::<Vec<_>>()
        .windows(3)
        .any(|w| w[0] == w[1] && w[1] == w[2]);
    if has_repeated {
        return false;
    }

    // ... existing logic ...
}
```

**Alternative**: Use the `zxcvbn` crate for entropy-based password strength checking

```toml
# Cargo.toml
[dependencies]
zxcvbn = "2.2"
```

```rust
use zxcvbn::zxcvbn;

pub fn check_password_strength(password: &str, user_inputs: &[&str]) -> Result<(), String> {
    let estimate = zxcvbn(password, user_inputs)
        .map_err(|_| "Password strength check failed")?;

    if estimate.score() < 3 {
        let feedback = estimate.feedback()
            .and_then(|f| f.warning())
            .unwrap_or("密码过于简单");
        return Err(feedback.to_string());
    }

    Ok(())
}
```

---

### 3. **Username Special Character Filtering - Medium Priority**

**Current Issue**: Only control characters are rejected

**Recommendations**:

```rust
// In auth_service.rs register()
pub async fn register(&self, req: &AuthRequest, ...) -> ... {
    let username = req.username.trim();

    // ... existing checks ...

    // NEW: Strict character whitelist (alphanumeric + limited specials)
    let is_valid_char = |c: char| {
        c.is_alphanumeric() 
        || c == '_' 
        || c == '-' 
        || c == '.' 
        || c == ' '  // Allow spaces for display names
    };

    if !username.chars().all(is_valid_char) {
        tracing::warn!(username = %sanitize_for_log(&req.username), 
            ip = %sanitize_for_log(client_ip), 
            "register rejected: invalid characters in username");
        return Ok(auth_err("用户名只能包含字母、数字、下划线、连字符和点"));
    }

    // NEW: Cannot start/end with special characters
    if username.starts_with(|c: char| !c.is_alphanumeric()) ||
       username.ends_with(|c: char| !c.is_alphanumeric()) {
        return Ok(auth_err("用户名必须以字母或数字开头和结尾"));
    }

    // NEW: No consecutive special characters
    let has_consecutive_specials = username.chars()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|w| !w[0].is_alphanumeric() && !w[1].is_alphanumeric());
    if has_consecutive_specials {
        return Ok(auth_err("用户名不能包含连续的特殊字符"));
    }

    // NEW: Check against reserved words
    const RESERVED_USERNAMES: &[&str] = &[
        "admin", "root", "system", "support", "help",
        "info", "webmaster", "noreply", "postmaster",
        "security", "abuse", "feedback",
    ];
    let lower_username = username.to_lowercase();
    if RESERVED_USERNAMES.contains(&lower_username.as_str()) {
        return Ok(auth_err("该用户名已被保留"));
    }

    // ... rest of registration logic ...
}
```

---

### 4. **Anti-Brute Force Enhancements - High Priority**

**Current Issue**: IP rate limit has no block duration (BLOCK_SECS = 0)

**Recommendations**:

```rust
// In auth_service.rs
const IP_RATE_LIMIT_MAX_ATTEMPTS: u32 = 30;
const IP_RATE_LIMIT_WINDOW_SECS: u64 = 60;
const IP_RATE_LIMIT_BLOCK_SECS: u64 = 900; // NEW: 15-minute block

// NEW: Progressive rate limiting
async fn check_rate_limits(
    &self,
    username: &str,
    client_ip: &str,
    action: &str,
) -> Result<(), ServiceError> {
    // ... existing IP check ...

    // NEW: Progressive delays based on failed attempts
    let fail_count = self.get_fail_count(client_ip, username).await;
    if fail_count > 0 {
        let delay_ms = std::cmp::min(fail_count * 1000, 10000); // Max 10s delay
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }

    // ... existing username check ...
}

// NEW: Account lockout after too many failed attempts
const ACCOUNT_LOCKOUT_THRESHOLD: u32 = 10;
const ACCOUNT_LOCKOUT_DURATION_SECS: u64 = 1800; // 30 minutes

async fn check_account_lockout(&self, user_id: i64) -> Result<(), ServiceError> {
    let key = format!("lockout:{}", user_id);
    if self.rate_limiter.check_with(&key, ACCOUNT_LOCKOUT_THRESHOLD, 3600, ACCOUNT_LOCKOUT_DURATION_SECS)
        .await
        .is_err()
    {
        return Err(ServiceError::RateLimited);
    }
    Ok(())
}

// NEW: CAPTCHA integration after N failed attempts
const CAPTCHA_THRESHOLD: u32 = 3;

fn requires_captcha(&self, client_ip: &str) -> bool {
    // Check if IP has exceeded threshold
    // Return true if CAPTCHA verification needed
    false // Implement with Redis/memory check
}
```

**Additional Brute Force Measures**:

1. **Implement exponential backoff**:
```rust
fn calculate_delay(attempt_count: u32) -> Duration {
    let base_ms = 1000;
    let delay_ms = base_ms * 2u64.pow(attempt_count.min(10));
    Duration::from_millis(delay_ms)
}
```

2. **IP reputation system**:
```rust
struct IpReputation {
    score: i32,          // Higher = more suspicious
    last_seen: Instant,
    failed_attempts: u32,
}

impl IpReputation {
    fn should_block(&self) -> bool {
        self.score > 100 || self.failed_attempts > 50
    }

    fn update_score(&mut self, success: bool) {
        if success {
            self.score = (self.score - 10).max(0);
        } else {
            self.score += 10;
        }
    }
}
```

3. **Geolocation-based rate limiting** (optional):
```rust
// Rate limit by country/region
async fn check_geo_rate_limit(&self, ip: &str) -> Result<(), ServiceError> {
    // Use MaxMind GeoIP or similar
    // Apply stricter limits for high-risk regions
    Ok(())
}
```

4. **Session fingerprinting**:
```rust
struct SessionFingerprint {
    user_agent: String,
    accept_language: String,
    screen_resolution: Option<String>,
    timezone: Option<String>,
}

// Store and compare fingerprints to detect session hijacking
```

---

### 5. **Additional Security Enhancements**

#### 5.1 Input Sanitization Middleware

```rust
// Add to middleware stack
pub async fn sanitize_input(
    req: Request,
    next: Next,
) -> Response {
    // Check Content-Length
    if let Some(content_length) = req.headers().get("content-length") {
        let length: usize = content_length.to_str().unwrap_or("0").parse().unwrap_or(0);
        if length > 1_048_576 { // 1MB limit
            return (StatusCode::PAYLOAD_TOO_LARGE, "Request too large").into_response();
        }
    }

    // Check for SQL injection patterns in query params
    let query = req.uri().query().unwrap_or("");
    let sql_patterns = ["'", "\"", ";", "--", "/*", "*/", "xp_", "exec", "execute"];
    if sql_patterns.iter().any(|&pattern| query.to_lowercase().contains(pattern)) {
        tracing::warn!("Potential SQL injection attempt detected");
        return (StatusCode::BAD_REQUEST, "Invalid request").into_response();
    }

    next.run(req).await
}
```

#### 5.2 Request Validation

```rust
// Add validation trait
trait ValidateAuthRequest {
    fn validate(&self) -> Result<(), String>;
}

impl ValidateAuthRequest for AuthRequest {
    fn validate(&self) -> Result<(), String> {
        // Username validation
        if self.username.is_empty() || self.username.len() > 64 {
            return Err("用户名长度无效".to_string());
        }

        // Password validation
        if self.password.is_empty() || self.password.len() > 128 {
            return Err("密码长度无效".to_string());
        }

        // Check for null bytes
        if self.username.contains('\0') || self.password.contains('\0') {
            return Err("输入包含非法字符".to_string());
        }

        // Check for Unicode control characters (except standard whitespace)
        let has_illegal_unicode = |s: &str| {
            s.chars().any(|c| {
                c.is_control() && c != '\n' && c != '\r' && c != '\t'
            })
        };

        if has_illegal_unicode(&self.username) || has_illegal_unicode(&self.password) {
            return Err("输入包含非法字符".to_string());
        }

        Ok(())
    }
}
```

#### 5.3 Security Headers Enhancement

```rust
// In security.rs middleware
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;

    let headers = resp.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
    headers.insert("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-inline'".parse().unwrap());
    headers.insert("Strict-Transport-Security", "max-age=31536000; includeSubDomains".parse().unwrap());

    resp
}
```

---

## Implementation Priority

| Priority | Enhancement | Effort | Impact |
|----------|-------------|--------|--------|
| 🔴 High | Password common word list | Low | High |
| 🔴 High | IP rate limit block duration | Low | High |
| 🔴 High | Account lockout mechanism | Medium | High |
| 🟡 Medium | Username character whitelist | Low | Medium |
| 🟡 Medium | Reserved username list | Low | Medium |
| 🟡 Medium | Email RFC compliance | Low | Medium |
| 🟢 Low | Progressive rate limiting | Medium | Medium |
| 🟢 Low | CAPTCHA integration | High | Medium |
| 🟢 Low | Geolocation rate limiting | High | Low |

---

## Testing Recommendations

1. **Unit Tests**:
   - Test all new validation rules
   - Test edge cases (empty strings, max lengths, special chars)
   - Test rate limiting thresholds

2. **Integration Tests**:
   - Test brute force attack scenarios
   - Test account lockout behavior
   - Test rate limit reset after successful login

3. **Security Tests**:
   - SQL injection attempts
   - XSS payloads in usernames
   - Unicode abuse
   - Timing attack verification

4. **Load Tests**:
   - Verify rate limiting under high load
   - Test Redis fallback behavior
   - Measure latency impact of new validations

---

## Configuration Suggestions

Add to `.env` or `AppConfig`:

```toml
[security.password]
min_length = 8
max_length = 128
require_uppercase = true
require_lowercase = true
require_digit = true
require_special = true
common_passwords_file = "common_passwords.txt"

[security.username]
min_length = 2
max_length = 64
allowed_chars = "a-zA-Z0-9_.-"
reserved_words_file = "reserved_usernames.txt"

[security.rate_limit]
ip_max_attempts = 30
ip_window_secs = 60
ip_block_secs = 900
username_max_attempts = 5
username_window_secs = 60
username_block_secs = 300
account_lockout_threshold = 10
account_lockout_duration_secs = 1800

[security.captcha]
enabled = false
threshold = 3
provider = "recaptcha" # or "hcaptcha"
```

---

## References

- [OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)
- [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
- [NIST SP 800-63B: Digital Identity Guidelines](https://pages.nist.gov/800-63-3/sp800-63b.html)
- [RFC 5321: SMTP](https://tools.ietf.org/html/rfc5321)

---

**Report Generated**: $(date)
**Project**: Atmos Video Backend
**File Analyzed**: `backend/src/handlers/auth.rs`
