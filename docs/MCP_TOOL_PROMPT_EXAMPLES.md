# CodeGraph MCP Tool Prompt Examples

Concrete example outputs demonstrating what each tool's prompts should generate.

---

## 1. code_search - Example Outputs

### Query: "user authentication with JWT tokens"

#### Balanced Mode Output (857 tokens)

```markdown
# RELEVANCE_SUMMARY
Query "user authentication with JWT tokens" matched 25 code locations with strong semantic relevance (avg score: 0.87).
Results span 3 architectural layers: API handlers (40%), business logic (35%), data access (25%).
Coverage is comprehensive with examples from both current production implementation and legacy systems being phased out.

# TOP_MATCHES

1. **src/api/user_service.rs:45-78** (score: 0.94)
   - PRIMARY implementation of user authentication flow
   - Uses JWT tokens with refresh mechanism (RS256 signing)
   - Follows team's standard error handling pattern with Result<AuthToken, AuthError>
   - Production-ready: Handles edge cases, includes metrics, proper logging

2. **src/core/auth/validator.rs:120-156** (score: 0.91)
   - Token validation logic with Redis-based blacklist
   - Caching layer for performance (99% cache hit rate in production)
   - Production-ready with comprehensive error handling
   - IMPORTANT: Depends on Redis for token blacklist - graceful degradation if Redis unavailable

3. **src/core/auth/token_generator.rs:30-65** (score: 0.89)
   - JWT token generation with configurable claims
   - Supports both access tokens (15min TTL) and refresh tokens (7 day TTL)
   - Uses RS256 asymmetric signing (private key for signing, public key for validation)

4. **tests/auth_integration_test.rs:100-250** (score: 0.85)
   - COMPREHENSIVE integration tests for full auth flow
   - Tests login, validation, refresh, logout scenarios
   - Excellent reference for understanding expected behavior
   - Includes edge cases: expired tokens, invalid signatures, blacklisted tokens

5. **src/legacy/user_auth.rs:200-245** (score: 0.78)
   - ⚠️ LEGACY: Older session-based authentication (pre-JWT migration)
   - Still used by admin panel only (scheduled for deprecation in Q2 2025)
   - DO NOT use for new features - included for completeness only

# USAGE_GUIDANCE

**For new authentication features**, use this pattern:

1. **Generate tokens** (use token_generator.rs as template):
```rust
let access_token = TokenGenerator::new()
    .with_user_id(user.id)
    .with_permissions(user.permissions)
    .with_ttl(Duration::minutes(15))
    .generate_access_token()?;
```

2. **Validate on each request** (use validator.rs pattern):
```rust
let claims = TokenValidator::new(redis_client)
    .validate_and_decode(&token)
    .await?;
```

3. **Handle refresh** (use user_service.rs pattern):
```rust
let new_access_token = AuthService::refresh_access_token(&refresh_token).await?;
```

**Key dependencies to include**:
- `jsonwebtoken = "9.0"` (JWT encode/decode)
- `redis = { version = "0.24", features = ["tokio-comp"] }` (token blacklist)
- `argon2 = "0.5"` (password hashing)

**Common gotchas**:
- ❌ Don't forget to check blacklist in addition to signature validation
- ❌ Don't store sensitive data in JWT claims (they're base64-encoded, not encrypted)
- ✅ Do use short TTL for access tokens (15min) and refresh mechanism
- ✅ Do implement token rotation for refresh tokens in production

# NEXT_STEPS

**To understand the full authentication flow**:
1. Search: "authentication middleware" → See how tokens are extracted from requests
2. Search: "permission check" → See how claims are used for authorization
3. Read: tests/auth_integration_test.rs → Comprehensive examples of all flows

**Related concerns to investigate**:
- Search: "rate limiting login" → Anti-bruteforce protection
- Search: "password reset flow" → Account recovery
- Search: "two-factor authentication" → MFA implementation (if present)

**Confidence: 0.93** - High confidence. Coverage is excellent, patterns are consistent, tests validate claims.
```

#### Deep Mode Output (3,247 tokens)

```markdown
# COMPREHENSIVE SEARCH ANALYSIS

## 1. EXECUTIVE_SUMMARY

### Query Interpretation
The query "user authentication with JWT tokens" seeks implementation details for stateless authentication using JSON Web Tokens. Analysis identified 25 semantically relevant code locations implementing:
- Token generation and signing (RS256 asymmetric cryptography)
- Token validation and verification
- Refresh token mechanism for session management
- Token revocation via Redis blacklist

### Result Quality Assessment
- **Coverage**: Excellent (9.2/10) - All aspects of JWT auth lifecycle covered
- **Relevance**: High (8.7/10 avg similarity score) - Strong semantic matches
- **Diversity**: Good - Spans API layer, core business logic, data access, and tests
- **Currency**: Current implementation + legacy code (clearly marked)

### Key Findings
1. ✅ Production-ready JWT implementation with RS256 signing
2. ✅ Dual-token system (short-lived access + long-lived refresh)
3. ✅ Redis-based token blacklist for logout/revocation
4. ⚠️ Legacy session-based auth still in use for admin panel (migration in progress)
5. ✅ Comprehensive test coverage (integration + unit tests)

## 2. DETAILED_ANALYSIS

### Functionality Categorization

**Category 1: Token Lifecycle Management (12 results)**
- Token Generation: `token_generator.rs` (primary), `jwt_utils.rs` (helpers)
- Token Validation: `validator.rs` (primary), `middleware/auth.rs` (HTTP layer)
- Token Refresh: `user_service.rs:180-210`, `refresh_handler.rs`
- Token Revocation: `blacklist.rs`, `logout_handler.rs`

**Category 2: Authentication Flow (8 results)**
- Login: `user_service.rs:45-78`, `login_handler.rs`
- Logout: `logout_handler.rs`, `session_cleanup.rs`
- Password Validation: `user_service.rs:50-55` (Argon2 verification)

**Category 3: Authorization (3 results)**
- Permission Extraction: `claims.rs:parse_permissions()`
- Role-based Access Control: `rbac_middleware.rs`

**Category 4: Testing (2 results)**
- Integration Tests: `auth_integration_test.rs` (comprehensive)
- Unit Tests: `token_generator_test.rs`

### Architectural Patterns

**Pattern 1: Layered Architecture**
```
API Layer (handlers/)
    ↓ uses
Business Logic (core/auth/)
    ↓ uses
Data Access (db/, redis/)
```
Clean separation of concerns with dependency injection.

**Pattern 2: Repository Pattern**
All data access goes through repository abstractions:
- `UserRepository` for user data
- `TokenBlacklistRepository` for revoked tokens
- Enables easy testing with mock implementations

**Pattern 3: Result-based Error Handling**
Consistent use of `Result<T, AuthError>` throughout:
```rust
pub enum AuthError {
    InvalidCredentials,
    TokenExpired,
    TokenRevoked,
    TokenMalformed,
    InternalError(String),
}
```
Enables exhaustive error handling at API boundary.

### Implementation Variations and Trade-offs

**Access Token TTL: 15 minutes vs. 1 hour**
- Production: 15 minutes (more secure, requires refresh mechanism)
- Development: 1 hour (developer convenience, less secure)
- Configured per environment via `config.yml:auth.access_token_ttl`

**Refresh Token Rotation: Enabled in Prod, Disabled in Dev**
- Production: One-time-use refresh tokens (mitigates token theft)
- Development: Reusable refresh tokens (easier testing)
- Trade-off: Security vs. convenience

**Signature Algorithm: RS256 (chosen) vs. HS256**
- RS256 selected for asymmetric key benefits:
  - Public key can be shared for validation (microservices architecture)
  - Private key only needed for token generation (reduced attack surface)
- HS256 would be simpler but requires shared secret everywhere

### Dependency and Relationship Mapping

**Critical Dependencies**:
1. `jsonwebtoken = "9.0"` → Token encode/decode
2. `redis = "0.24"` → Token blacklist storage
3. `argon2 = "0.5"` → Password hashing
4. `chrono = "0.4"` → Timestamp handling

**Internal Module Dependencies**:
```
user_service.rs
  ↓ depends on
  ├─ token_generator.rs (generate tokens)
  ├─ validator.rs (validate tokens)
  ├─ db/user_repository.rs (load user data)
  └─ redis/blacklist_repository.rs (check/add blacklist)

validator.rs
  ↓ depends on
  ├─ redis/blacklist_repository.rs (check if revoked)
  └─ config.rs (load public key for validation)
```

**Coupling Analysis**:
- Afferent Coupling (Ca): 15 modules depend on `validator.rs` → High stability requirement
- Efferent Coupling (Ce): `validator.rs` depends on 3 modules → Moderate coupling
- Instability: I = 3/(3+15) = 0.17 → Very stable module (appropriate for core auth)

## 3. QUALITY_ASSESSMENT

### Code Quality Metrics

**Maintainability**: 85/100 (Good)
- Clear naming conventions
- Comprehensive documentation
- Modular design with single responsibilities

**Complexity**:
- Cyclomatic Complexity: 4.2 avg (Good - below threshold of 10)
- Cognitive Complexity: 6.1 avg (Good - easy to understand)
- Max Function Length: 45 lines in `user_service.rs:login()` (Acceptable)

**Documentation Coverage**: 92%
- All public functions have doc comments
- Complex algorithms explained
- Some edge cases could use more inline comments

### Best Practices Adherence

✅ **Security Best Practices**:
- Argon2 password hashing (OWASP recommended)
- RS256 asymmetric JWT signing (industry standard)
- Short-lived access tokens (OWASP recommendation: < 30min)
- Token blacklist for logout (prevents token reuse)

✅ **Rust Best Practices**:
- Zero-copy where possible (token validation doesn't clone claims)
- Async/await for I/O operations
- Error handling with Result types (no panics in production code)
- Type safety (strong typing for UserId, TokenId, etc.)

✅ **Testing Best Practices**:
- Integration tests cover happy path + error scenarios
- Unit tests for token generation/validation logic
- Test coverage: 87% (Good - above 80% target)

### Potential Technical Debt

⚠️ **Medium Priority**:
1. **Legacy admin auth coexists with JWT** (src/legacy/user_auth.rs)
   - Impact: Code confusion, maintenance burden
   - Mitigation: Migration scheduled for Q2 2025
   - Risk: Low (isolated to admin panel)

⚠️ **Low Priority**:
2. **Token blacklist grows unbounded in Redis**
   - Impact: Memory usage increases over time
   - Mitigation: TTL set to match token expiry (auto-cleanup)
   - Risk: Very low (TTL cleanup working correctly)

3. **No token usage analytics**
   - Impact: Can't detect unusual patterns or attacks
   - Opportunity: Add token usage tracking to Redis
   - Risk: Low (rate limiting mitigates brute force)

### Anti-patterns: NONE DETECTED
No significant anti-patterns found. Implementation follows industry best practices.

## 4. ARCHITECTURAL_CONTEXT

### System Architecture Integration

```
┌─────────────────┐
│  API Gateway    │ ← Extracts token from Authorization header
└────────┬────────┘
         │
         ↓ validates token
┌─────────────────┐
│ Auth Middleware │ ← Uses validator.rs
└────────┬────────┘
         │
         ↓ populates request context
┌─────────────────┐
│ Business Logic  │ ← Access to user claims
└────────┬────────┘
         │
         ↓ may call
┌─────────────────┐
│ Auth Service    │ ← Token generation, refresh
└────────┬────────┘
         │
         ↓ persists to
┌─────────────────┐
│ Redis + DB      │ ← Blacklist + user data
└─────────────────┘
```

### Layer Boundaries and Responsibilities

**API Layer** (src/api/):
- HTTP request/response handling
- Token extraction from headers
- Error serialization to HTTP status codes
- Input validation

**Business Logic Layer** (src/core/auth/):
- Authentication logic (credential verification)
- Token generation and validation
- Business rules (e.g., token rotation policy)
- Domain errors

**Data Access Layer** (src/db/, src/redis/):
- User persistence (PostgreSQL)
- Token blacklist (Redis)
- Repository pattern abstractions

**Boundary Enforcement**: ✅ Good
- No DB queries in API handlers
- No HTTP concerns in business logic
- Clean dependency flow (outer → inner)

### Data Flow Pattern

**Login Flow**:
```
1. Client → POST /api/login {username, password}
2. API Handler → User Service
3. User Service → User Repository (fetch user from DB)
4. User Service → Argon2 verify password
5. User Service → Token Generator (create JWT)
6. User Service → Blacklist Repository (ensure clean state)
7. API Handler ← Access Token + Refresh Token
8. Client ← JSON {access_token, refresh_token, expires_in}
```

**Request Authentication Flow**:
```
1. Client → GET /api/resource [Header: Authorization: Bearer {token}]
2. Auth Middleware → Extract token
3. Auth Middleware → Token Validator
4. Token Validator → Verify signature
5. Token Validator → Check expiry
6. Token Validator → Blacklist Repository (check if revoked)
7. Auth Middleware ← Claims {user_id, permissions, exp}
8. Auth Middleware → Populate request context
9. Business Logic → Access claims from context
```

### Integration Points

**External Integrations**:
- Redis: Token blacklist storage (required for logout)
- PostgreSQL: User credential storage (required for login)
- None: JWT validation is self-contained (public key in config)

**Internal Integrations**:
- All authenticated endpoints depend on auth middleware
- RBAC system depends on claims extracted by auth
- Audit logging reads user_id from auth context

## 5. USAGE_RECOMMENDATIONS

### Scenario 1: Implementing New Authenticated Endpoint

**Use**: Standard auth middleware pattern
```rust
// src/api/my_new_endpoint.rs
use crate::middleware::require_auth;

#[get("/api/my-resource")]
async fn my_handler(claims: Claims) -> Result<Json<Response>, ApiError> {
    // claims.user_id is available
    // claims.permissions is available
    let user_id = claims.user_id;
    // ... your logic
    Ok(Json(response))
}
```

**Integration**: Add middleware in router
```rust
// src/api/router.rs
.route("/api/my-resource", web::get()
    .wrap(require_auth()) // This enforces authentication
    .to(my_handler))
```

### Scenario 2: Implementing Password Reset

**Use**: Generate special-purpose JWT (not for general auth)
```rust
// Generate short-lived reset token (1 hour)
let reset_token = TokenGenerator::new()
    .with_user_id(user.id)
    .with_claim("purpose", "password_reset")
    .with_ttl(Duration::hours(1))
    .generate_special_token()?;

// Validate reset token (different validation path)
let claims = TokenValidator::new_special("password_reset")
    .validate_and_decode(&reset_token)?;
```

**Pitfall to avoid**: Don't use access token for password reset (different security domain)

### Scenario 3: Implementing Admin Endpoint with Elevated Permissions

**Use**: Permission check after auth
```rust
#[get("/api/admin/users")]
async fn admin_list_users(claims: Claims) -> Result<Json<Vec<User>>, ApiError> {
    // First: Already authenticated by middleware
    // Second: Check for admin permission
    if !claims.has_permission("admin:users:read") {
        return Err(ApiError::Forbidden);
    }

    // ... admin logic
}
```

**Alternative**: Custom middleware for role-based access
```rust
.route("/api/admin/users", web::get()
    .wrap(require_permission("admin:users:read"))
    .to(admin_list_users))
```

### Common Pitfalls and Avoidance

❌ **Pitfall 1: Forgetting to check blacklist**
```rust
// WRONG - only checks signature and expiry
let claims = decode::<Claims>(&token, &validation_key, &Validation::default())?;
```
✅ **Correct - always use TokenValidator which includes blacklist check**
```rust
let claims = TokenValidator::new(redis_client).validate_and_decode(&token).await?;
```

❌ **Pitfall 2: Storing sensitive data in JWT claims**
```rust
// WRONG - password visible in base64-decoded token
let token = TokenGenerator::new()
    .with_claim("password", user.password_hash)
    .generate()?;
```
✅ **Correct - only store identifiers and permissions**
```rust
let token = TokenGenerator::new()
    .with_user_id(user.id)
    .with_permissions(user.permissions)
    .generate()?;
```

❌ **Pitfall 3: Not handling token expiry gracefully**
```rust
// WRONG - generic error doesn't tell client to refresh
return Err(ApiError::Unauthorized);
```
✅ **Correct - specific error enables client to refresh**
```rust
match validator.validate(&token).await {
    Err(AuthError::TokenExpired) => Err(ApiError::TokenExpired),
    Err(e) => Err(ApiError::Unauthorized),
    Ok(claims) => Ok(claims),
}
```

### Testing and Validation Strategies

**Unit Testing**:
```rust
#[tokio::test]
async fn test_token_generation_and_validation() {
    let token = TokenGenerator::test_instance()
        .with_user_id(UserId(123))
        .generate_access_token()
        .unwrap();

    let claims = TokenValidator::test_instance()
        .validate_and_decode(&token)
        .await
        .unwrap();

    assert_eq!(claims.user_id, UserId(123));
}
```

**Integration Testing**:
```rust
#[tokio::test]
async fn test_full_authentication_flow() {
    let app = test_app().await;

    // 1. Login
    let login_response = app.login("user@example.com", "password").await;
    assert!(login_response.is_ok());
    let tokens = login_response.unwrap();

    // 2. Use access token
    let resource = app.get_resource(&tokens.access_token).await;
    assert!(resource.is_ok());

    // 3. Logout
    app.logout(&tokens.access_token).await.unwrap();

    // 4. Verify token is blacklisted
    let resource = app.get_resource(&tokens.access_token).await;
    assert!(matches!(resource.unwrap_err(), ApiError::TokenRevoked));
}
```

## 6. LEARNING_INSIGHTS

### Team Conventions Detected

**Naming Conventions**:
- Functions: `snake_case` (Rust standard)
- Types: `PascalCase` (Rust standard)
- Error types: Suffix with `Error` (e.g., `AuthError`)
- Result types: Explicit (e.g., `Result<AuthToken, AuthError>`)

**Error Handling Convention**:
- Use `Result<T, E>` everywhere (no panics in production code)
- Custom error enums for each module
- `From` trait implemented for error conversions
- Error context preserved through error chain

**Async Convention**:
- All I/O operations are async
- Use `tokio` runtime
- Database operations use connection pools
- No blocking operations in async context

**Testing Convention**:
- Integration tests in `tests/` directory
- Unit tests in same file as implementation (`#[cfg(test)]` mod tests)
- Test fixtures use builder pattern
- Test databases use transactions (rollback after each test)

### Domain-Specific Patterns

**JWT Claims Structure**:
```rust
pub struct Claims {
    pub sub: UserId,              // Subject (user ID)
    pub exp: i64,                 // Expiry timestamp
    pub iat: i64,                 // Issued at
    pub permissions: Vec<String>, // Permission strings
    pub session_id: SessionId,    // For revocation
}
```

**Permission String Format**: `resource:action`
- Examples: `users:read`, `users:write`, `admin:users:delete`
- Hierarchical matching supported (e.g., `admin:*` matches all admin permissions)

**User ID Type**: NewType pattern for type safety
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UserId(pub i64);
```
Prevents accidentally passing wrong ID type.

### Technology Stack Patterns

**Web Framework**: Actix-web
- Middleware pattern for cross-cutting concerns
- Dependency injection via app data
- Async request handlers

**Database**: PostgreSQL with SQLx
- Compile-time query verification
- Async operations
- Connection pooling

**Cache**: Redis
- Async operations via `redis-rs` with `tokio-comp` feature
- Used for token blacklist and session data
- TTL-based automatic cleanup

**Serialization**: Serde
- JSON for API responses
- Custom serialization for sensitive data (password hashing)

## 7. FOLLOW_UP_QUERIES

### To Deepen Understanding

1. **"authentication middleware implementation"**
   - How tokens are extracted from HTTP headers
   - How claims are passed to request handlers
   - Error handling in middleware

2. **"permission authorization RBAC"**
   - How permission checks are implemented
   - Role-based access control patterns
   - Permission string matching logic

3. **"token refresh rotation"**
   - Refresh token rotation mechanism
   - One-time-use token implementation
   - Refresh token storage and cleanup

4. **"rate limiting login attempts"**
   - Brute-force protection implementation
   - Rate limit configuration per endpoint
   - Rate limit storage (Redis vs. in-memory)

### Related Areas to Investigate

1. **"session management user context"**
   - How user context is maintained across requests
   - Session state (if any) beyond JWT

2. **"password hashing argon2 configuration"**
   - Argon2 parameter tuning
   - Password strength requirements
   - Password reset flow

3. **"API security CORS CSRF"**
   - CORS configuration
   - CSRF protection (if applicable)
   - Other security middleware

### Gaps in Current Results

**Not covered by results**:
- Multi-factor authentication (MFA) - may not exist
- OAuth2/OIDC integration - may not exist
- Token introspection endpoint - may not be needed
- JWT key rotation - static key vs. rotating keys

**Suggested searches**:
- "two-factor authentication 2FA MFA" → Check if MFA exists
- "OAuth2 OIDC social login" → Check for third-party auth
- "key rotation JWT signing key" → Check key management strategy

## 8. CONFIDENCE AND CITATIONS

**Overall Confidence**: 0.93 (Very High)

**Confidence Breakdown**:
- Code coverage: 0.95 (25 results, all relevant)
- Documentation quality: 0.90 (well-documented code)
- Test coverage: 0.87 (comprehensive tests found)
- Pattern consistency: 0.98 (highly consistent implementation)
- Completeness: 0.92 (minor gaps in MFA/OAuth2 - may not exist)

**Key Citations**:
- Primary implementation: `src/api/user_service.rs:45-78`
- Validation logic: `src/core/auth/validator.rs:120-156`
- Token generation: `src/core/auth/token_generator.rs:30-65`
- Integration tests: `tests/auth_integration_test.rs:100-250`
- Legacy code: `src/legacy/user_auth.rs:200-245` (marked for deprecation)

**Information Sources**:
- Code analysis: 25 files examined
- Test files: 2 comprehensive test suites
- Configuration: `config.yml`, environment variables
- Documentation: Inline doc comments, README references

**Limitations**:
- No runtime behavior observed (static analysis only)
- No production metrics available (token refresh rate, failure rate, etc.)
- Dependency versions current as of code analysis (may have updates)
```

---

## 2. dependency_analysis - Example Outputs

### Target: `process_payment()` in `src/services/payment_service.rs`

#### Balanced Mode Output (782 tokens)

```markdown
# DEPENDENCY_SUMMARY
`process_payment()` has moderate coupling (score: 0.62/1.0) with 8 direct and 23 transitive dependencies.

**Dependency Health**: GOOD
- ✅ No circular dependencies detected
- ✅ Clear layered structure (API → Service → Repository)
- ✅ External dependencies on stable libraries only
- ⚠️ Moderate coupling to Stripe SDK (vendor lock-in risk)

**External Dependencies**:
1. Stripe SDK v12.5.0 (payment processing) - Stable, well-maintained
2. PostgreSQL client (transaction management) - Standard database
3. Redis client (idempotency checks) - Standard cache

# CRITICAL_PATHS

### Path 1: Payment → Stripe SDK → Network I/O
```
process_payment()
  → StripeClient::create_charge()
    → HTTP POST to api.stripe.com
```
**Why it matters**: Most fragile path - network failures will cascade
**Risk**: HIGH - External API dependency
**Mitigation**: ✅ Circuit breaker pattern implemented (fails after 3 retries)
**Current stats**: 99.2% success rate in production

### Path 2: Payment → Database Transaction → Inventory Lock
```
process_payment()
  → db.begin_transaction()
    → OrderRepository::create_order()
      → InventoryRepository::reserve_items()
        → Row-level locks on inventory table
```
**Why it matters**: Critical for data consistency
**Risk**: MEDIUM - Long-running transactions can cause deadlocks
**Current behavior**: Max transaction time 5s (configured timeout)
**Mitigation**: ✅ Transaction timeout configured, retry logic for deadlocks

### Path 3: Payment → Email Service → Notification Queue
```
process_payment()
  → EmailService::send_receipt()
    → MessageQueue::enqueue()
      → RabbitMQ publish
```
**Why it matters**: Asynchronous but affects user experience
**Risk**: LOW - Non-blocking, can degrade gracefully
**Degradation mode**: Queue overflow delays notifications but doesn't block payment

# IMPACT_ASSESSMENT

## If Signature Changes (HIGH RISK 🔴)
**Direct Impact**:
- 15 direct callers across 3 flows:
  - Checkout flow: 8 callers (user-facing)
  - Admin panel: 4 callers (manual orders)
  - Refund service: 3 callers (automated refunds)

**Transitive Impact**:
- 40+ downstream components affected
- 3 microservices depend on payment events (inventory, shipping, analytics)

**Deployment Coordination Required**: YES
- All services must deploy simultaneously OR
- Use versioned API with backward compatibility

## If Behavior Changes (MEDIUM RISK 🟡)
**Assumptions by Dependents**:
1. **Payment reconciliation service** expects specific error codes:
   - `INSUFFICIENT_FUNDS` → Don't retry
   - `NETWORK_ERROR` → Retry with backoff
   - Changing error codes breaks reconciliation logic

2. **Fraud detection** depends on call sequence:
   - Expects inventory check → fraud check → charge sequence
   - Changing order could bypass fraud detection

3. **Audit log** format parsed by external analytics:
   - Changing log structure breaks reports
   - Analytics team must be notified

## If Removed (CRITICAL 🔴)
- No alternative implementation exists
- **Core business functionality** - revenue impact
- Estimated replacement effort: 2-3 weeks (complex business logic)
- Historical data migration needed (payment records)

# RISK_FACTORS

## Coupling Risks

⚠️ **High coupling to Stripe SDK (coupling score: 0.85)**
- Risk: Vendor lock-in, difficult to switch payment processors
- Impact: High switching cost (~2 months estimated)
- Mitigation: Consider payment abstraction layer

⚠️ **Database transaction spans 3 tables**
- Risk: Deadlocks under high concurrency
- Current: 0.02% deadlock rate in production (acceptable)
- Mitigation: Transaction isolation level tuned to READ_COMMITTED

## Design Risks

⚠️ **No idempotency token validation**
- Risk: Double-charging if client retries
- Severity: HIGH - Financial impact
- Current mitigation: Redis-based deduplication (7-day window)
- Note: Works but not foolproof (Redis failure = potential duplicate)

## Positive Indicators

✓ **Good separation from business logic**
- Payment logic isolated in dedicated service
- Domain logic doesn't directly depend on Stripe

✓ **Dependency injection used**
- Easy to mock for testing
- Test coverage: 92% (excellent)

# SAFE_CHANGES

## ✅ Safe to Change Without Coordination

**Internal validation logic** (lines 45-60):
```rust
// Changing this validation is safe
fn validate_payment_amount(amount: Decimal) -> Result<(), PaymentError> {
    // Can modify thresholds, add checks, etc.
}
```
- No external dependencies on validation details
- Errors are generic enough to not break callers

**Error message formatting**:
- Changing error message text is safe
- Do NOT change error codes (dependents check codes)

**Logging and metrics collection**:
- Can add/remove logs freely
- Can change metric names (analytics team should be notified)

**Retry timing parameters**:
- Can adjust backoff timings (within reason)
- Do NOT change from retry to no-retry (behavior change)

## ⚠️ Requires Coordination

**Error code values**:
- Dependents check specific error codes
- Change process: Update dependents first → then change codes

**Return type structure**:
- Adding fields to `PaymentResult` is OK (backward compatible)
- Removing fields needs migration plan

**Async behavior changes**:
- Current: Blocks until Stripe responds
- Change to async would break caller assumptions

## 🔴 Do Not Change Without Major Version

**Payment amount calculation logic**:
- Affects reconciliation and accounting
- Must maintain exact precision
- Change requires financial audit

**Transaction boundary**:
- Current: Single transaction for order + inventory + payment record
- Changing breaks consistency guarantees
- Requires distributed transaction coordination

**External API call sequence**:
- Fraud detection expects specific order
- Stripe API calls must follow documented pattern
- Changing could bypass security checks

## Recommended Approach for Changes

1. **For safe changes**: Make change, deploy, monitor
2. **For coordinated changes**:
   - Create feature flag
   - Deploy both old and new code paths
   - Migrate callers incrementally
   - Remove old code path after migration complete
3. **For breaking changes**:
   - Version the API (v1 → v2)
   - Deprecation period (3 months minimum)
   - Migration guide and tooling
   - Coordinated deployment across services

**Confidence: 0.88** - High confidence on direct dependencies, moderate on transitive (some inferred from logs)
```

---

## 3. call_chain_analysis - Example Outputs

### Entry Point: `handle_checkout_request()` in `src/api/checkout_controller.rs`

#### Balanced Mode Output (934 tokens)

```markdown
# EXECUTION_SUMMARY

Entry point `handle_checkout_request()` initiates a complex execution flow with 12 function calls across 4 architectural layers.

**Main Flow**:
1. **HTTP Request Handling** (API layer)
2. **Business Logic** (Checkout orchestration)
3. **Payment Processing** (Payment service)
4. **Data Persistence** (Repository layer)

**Key Decision Points**:
- Line 45: Inventory availability check → Abort if unavailable
- Line 67: Payment authorization → Abort if declined
- Line 89: Fraud check → Hold for review if suspicious

**Execution Characteristics**:
- **Synchronous calls**: 10 (blocking operations)
- **Asynchronous calls**: 2 (email notification, analytics event)
- **Database queries**: 5 (within 2 transactions)
- **External API calls**: 2 (Stripe payment, fraud service)
- **Estimated latency**: p50: 280ms, p95: 850ms, p99: 2.1s

# CRITICAL_PATHS

### Path 1: Happy Path (Customer Purchase)
```
handle_checkout_request()
  → validate_cart_items()           [  5ms] - DB query
    → CheckoutService::process()     [250ms] - Main orchestration
      → InventoryService::reserve()  [ 15ms] - DB transaction
      → PaymentService::charge()     [180ms] - Stripe API call
      → OrderService::create()       [ 20ms] - DB transaction
      → EmailService::send_receipt() [ 10ms] - Async, non-blocking
```
**Total (synchronous)**: ~270ms typical
**Why critical**: Main revenue flow, impacts conversion rate

### Path 2: Inventory Insufficient (Early Abort)
```
handle_checkout_request()
  → validate_cart_items()
    → InventoryService::check_availability()  [5ms]
      → Returns Err(OutOfStock)
  → Returns 409 Conflict to client
```
**Total**: ~5ms (fast failure)
**Why critical**: Common error path, must be fast to handle high traffic

### Path 3: Payment Declined (Mid-Flow Abort)
```
handle_checkout_request()
  → ... (same as happy path until payment)
  → PaymentService::charge()
    → Stripe API returns decline
  → InventoryService::release_reservation()  [Rollback]
  → Returns 402 Payment Required to client
```
**Total**: ~200ms (includes rollback)
**Why critical**: Must rollback inventory reservation to prevent lock-up

# PERFORMANCE_RISKS

## 🔴 Critical: Stripe API Call Blocking
**Location**: `payment_service.rs:125`
```rust
// BLOCKING call to external API
let charge = stripe_client.create_charge(&charge_params).await?;
```
**Risk**: Stripe latency directly impacts checkout performance
**Current stats**: p95 latency 180ms, p99 latency 850ms (from Stripe)
**Impact**: High - blocks order completion
**Mitigation**: ✅ Timeout set to 5s (fail fast), ✅ Circuit breaker implemented

## 🟡 Medium: Sequential Database Queries
**Location**: `checkout_service.rs:67-89`
```rust
// Could be parallelized
let cart = cart_repo.find_by_id(cart_id).await?;        // Query 1
let user = user_repo.find_by_id(user_id).await?;        // Query 2
let shipping = shipping_repo.calculate(address).await?; // Query 3
```
**Risk**: Serial I/O operations add latency
**Current**: 3 × 5ms = 15ms (acceptable)
**Optimization opportunity**: Parallelize with `tokio::join!` → reduce to ~5ms
**Priority**: Low (not a bottleneck yet)

## 🟡 Medium: Database Transaction Duration
**Location**: `order_service.rs:45-78`
```rust
let tx = db.begin_transaction().await?;
// ... multiple operations in transaction ...
tx.commit().await?;
```
**Risk**: Long-running transaction holds locks
**Current**: p50: 20ms, p95: 50ms, p99: 120ms
**Threshold**: 5s timeout configured
**Deadlock rate**: 0.02% (acceptable)

## 🟢 Low: N+1 Query - NOT PRESENT
**Good news**: No N+1 query patterns detected
**Location**: Cart item loading uses `JOIN` (efficient)

# ERROR_PROPAGATION

## Error Flow Patterns

### Pattern 1: Early Return on Validation Failure
```rust
validate_cart_items()?; // Propagates ValidationError
// ↓
// Returns 400 Bad Request
```
**Behavior**: Fast failure, no cleanup needed
**User experience**: Immediate feedback

### Pattern 2: Rollback on Payment Failure
```rust
inventory.reserve()?;   // Step 1: Reserve inventory
payment.charge()?;      // Step 2: Charge payment - FAILS
inventory.release()?;   // Step 3: Rollback reservation
```
**Behavior**: Compensating transaction
**User experience**: Safe - inventory not locked

### Pattern 3: Async Error Logging
```rust
// Email sending failure doesn't fail checkout
if let Err(e) = email_service.send_receipt().await {
    log::error!("Receipt email failed: {}", e);
    // Continue - email is non-critical
}
```
**Behavior**: Degrade gracefully
**User experience**: Order succeeds even if email fails

## Error Handling Quality

✅ **Comprehensive error handling**:
- All external calls wrapped in `Result`
- Timeouts configured on I/O operations
- Circuit breakers on external APIs

✅ **Proper cleanup**:
- Inventory reservations released on payment failure
- Database transactions rolled back on error

⚠️ **Partial: Error context**:
- Most errors include context (e.g., order ID, user ID)
- Some low-level errors could use more context

# CONCURRENCY_NOTES

## Sync/Async Boundaries

### Boundary 1: API Handler (Sync) → Service (Async)
```rust
// API handler is async
async fn handle_checkout_request(...) -> Result<HttpResponse> {
    // Calls async service methods
    let result = checkout_service.process(...).await?;
}
```
**Safe**: Clean async boundary

### Boundary 2: Service (Async) → Repository (Async)
```rust
// Service calls async repository
let order = order_repo.create(&order_data).await?;
```
**Safe**: End-to-end async (no blocking)

### Boundary 3: Email Notification (Fire-and-Forget)
```rust
// Spawned on separate task - non-blocking
tokio::spawn(async move {
    email_service.send_receipt(order_id).await
});
```
**Safe**: Doesn't block main flow

## Race Conditions: NONE DETECTED

✅ **Inventory reservation uses database locks** (pessimistic locking)
✅ **No shared mutable state** (functional approach)
✅ **Payment idempotency** via Redis (prevents double-charge on retry)

## Deadlock Potential: LOW

⚠️ **Possible deadlock scenario** (rare):
- Transaction A: Locks inventory → tries to lock payment record
- Transaction B: Locks payment record → tries to lock inventory
- **Frequency**: 0.02% of transactions (from production metrics)
- **Mitigation**: Transaction timeout 5s → automatic rollback

## Parallelization Opportunities

💡 **Opportunity 1**: Parallelize cart validation queries
```rust
// Current (sequential): ~15ms
let cart = cart_repo.find(id).await?;
let user = user_repo.find(id).await?;

// Optimized (parallel): ~5ms
let (cart, user) = tokio::join!(
    cart_repo.find(cart_id),
    user_repo.find(user_id)
);
```
**Impact**: 10ms latency reduction
**Effort**: Low
**Priority**: Medium

**Confidence: 0.85** - High confidence on main paths, moderate on error paths (some scenarios untested)
```

---

## Context Tier Comparison Examples

### Small Tier (< 50K, 10 results) vs. Large Tier (150K-500K, 50 results)

**Query**: "database connection pooling configuration"

#### Small Tier Output (Balanced Mode) - 412 tokens

```markdown
# RELEVANCE_SUMMARY
Found 10 results related to database connection pooling (avg score: 0.82).
Primary configuration in `db/config.rs`, usage across 5 service files.

# TOP_MATCHES

1. **db/config.rs:15-40** (0.94)
   - Main pool configuration (min_connections: 5, max: 20)
   - Uses r2d2 connection pool library

2. **services/user_service.rs:10** (0.88)
   - Example of getting connection from pool
   - Pattern: `let conn = pool.get()?;`

3. **config.toml:8-12** (0.85)
   - Pool configuration values
   - Tunable via environment variables

# USAGE_GUIDANCE
Configure pool in `db/config.rs`, get connections with `pool.get()`.
Default settings work for most cases (5-20 connections).

# NEXT_STEPS
- Increase `max_connections` if seeing "pool exhausted" errors
- Check `DATABASE_URL` environment variable is set correctly
```

#### Large Tier Output (Balanced Mode) - 1,156 tokens

```markdown
# RELEVANCE_SUMMARY
Found 50 results comprehensively covering database connection pooling (avg score: 0.79).
Results span: configuration (12), usage patterns (28), monitoring (6), testing (4).

**Statistical Analysis**:
- 85% of services use identical pool retrieval pattern
- 3 legacy services use older pooling approach (scheduled for migration)
- Pool configuration varies by environment (dev: 5-10, prod: 20-50)

# TOP_MATCHES

1. **db/config.rs:15-85** (0.94)
   - Primary pool configuration with full builder pattern
   - Configurable: min/max connections, timeout, test_on_checkout
   - Supports multiple databases (primary + read replica pools)
   - Environment-specific tuning

2. **services/user_service.rs:1-30** (0.91)
   - Standard usage pattern with error handling
   - Connection acquisition, usage, automatic return to pool
   - Includes retry logic for pool exhaustion

3. **monitoring/db_metrics.rs:40-75** (0.88)
   - Pool metrics collection (active, idle, waiting connections)
   - Exposed via Prometheus metrics
   - Alerts configured for pool saturation

4. **tests/db_integration_test.rs:20-50** (0.86)
   - Test fixtures using separate pool
   - Transaction rollback pattern for test isolation

5. **config/production.toml:15-30** (0.85)
   - Production pool settings: min 20, max 50, timeout 30s
   - Optimized for 4-core server with 8GB RAM

[... continues with 10 total matches instead of 3]

# USAGE_GUIDANCE

**Standard Pattern** (used in 85% of codebase):
```rust
use crate::db::pool::get_connection;

async fn my_database_operation() -> Result<Data> {
    let mut conn = get_connection().await?;
    // Use connection
    let result = query_something(&mut conn).await?;
    // Connection automatically returned to pool on drop
    Ok(result)
}
```

**Performance Tuning**:
- Development: 5-10 connections (low concurrency)
- Staging: 10-20 connections
- Production: 20-50 connections (tune based on load)
- Formula: connections ≈ (core_count × 2) + disk_count

**Common Issues**:
- Pool exhausted → Increase max_connections or add connection timeout
- High idle connections → Decrease max_connections or add idle_timeout
- Slow queries → Enable test_on_checkout to detect connection issues

**Monitoring**:
Check Prometheus metrics at `/metrics`:
- `db_pool_connections_active` - Currently in use
- `db_pool_connections_idle` - Available in pool
- `db_pool_wait_time_ms` - Time waiting for connection

# NEXT_STEPS

**To tune performance**:
1. Monitor `db_pool_wait_time_ms` - should be < 10ms
2. If wait time high → increase max_connections
3. Check `db_pool_connections_active` - if near max → need more connections

**To debug issues**:
- Search "pool exhausted error handling" → See retry patterns
- Search "database timeout configuration" → Tune query timeouts
- Read monitoring/db_metrics.rs → Understand all available metrics

**Confidence: 0.88** - Excellent coverage of configuration and usage
```

**Analysis**: Large tier gets 3x more content with:
- Statistical analysis across 50 results
- More code examples
- Performance tuning formulas
- Monitoring integration
- Production vs. dev environment differences

---

This demonstrates the structured, production-ready format these prompts should generate across different modes and context tiers.
