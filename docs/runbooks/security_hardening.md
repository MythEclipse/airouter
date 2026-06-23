# Security Hardening Runbook

Operational guide for the Security Hardening feature set.

## Initial Deployment (Greenfield)

1. Deploy with no existing data (fresh Postgres).
2. Server starts, generates random JWT secret + initial admin password.
3. **Critical:** grep logs for `INITIAL ADMIN PASSWORD`:
   ```bash
   journalctl -u airouter -n 100 | grep -A 5 "INITIAL ADMIN PASSWORD"
   # Or for docker:
   docker logs airouter 2>&1 | grep -A 5 "INITIAL ADMIN PASSWORD"
   ```
4. Login with initial password at `/login`.
5. Forced to change password at `/change-password`.
6. New password becomes the permanent admin password.

## Upgrade from Pre-Hardening Version

1. Backup database:
   ```bash
   pg_dump $DATABASE_URL > backup_$(date +%Y%m%d).sql
   ```
2. Deploy new version. Migrations run automatically.
3. Existing password (if any) is preserved. `must_change_password` defaults to FALSE for existing data.
4. Admin can rotate JWT secret via dashboard if desired.

## Forgot Password Recovery

Direct database update:

```sql
-- Requires pgcrypto extension: CREATE EXTENSION IF NOT EXISTS pgcrypto;
UPDATE server_config SET
  password_hash = encode(digest('NEW_PASSWORD_HERE', 'sha256'), 'hex'),
  must_change_password = true
WHERE id = 1;
```

Replace `NEW_PASSWORD_HERE` with desired new password. After update, login with new password and change it via UI.

Alternatively, compute the hash externally and update directly:
  ```bash
  python3 -c "import hashlib; print(hashlib.sha256(b'new_password_here').hexdigest())"
  ```

## JWT Secret Rotation

Via dashboard:
1. Settings -> Security -> Rotate JWT Secret
2. Set grace period (default 24h, max 168h).
3. Click "Rotate".
4. Existing sessions valid until grace period expires.

Via database (emergency):
```sql
UPDATE jwt_secrets SET
  current_secret = encode(gen_random_bytes(32), 'hex'),
  previous_secret = current_secret,
  previous_expires_at = NOW() + INTERVAL '24 hours',
  rotated_at = NOW()
WHERE id = 1;
```
Other instances refresh within 5 minutes.

## Lost JWT Secret (Postgres Corruption)

If the `jwt_secrets` row is lost, all sessions invalidate. Recovery:

```sql
INSERT INTO jwt_secrets (id, current_secret, updated_at)
VALUES (1, encode(gen_random_bytes(32), 'hex'), NOW())
ON CONFLICT (id) DO UPDATE SET
  current_secret = EXCLUDED.current_secret,
  previous_secret = NULL,
  previous_expires_at = NULL,
  rotated_at = NULL,
  updated_at = NOW();
```

All users must re-login.

## Redis Outage Scenarios

| Operation | Behavior |
|-----------|----------|
| API request lookup | Uses stale in-process cache (up to 5 min old). Fail-open. |
| API key create/delete | Returns 503. Admin must retry when Redis recovers. |
| Periodic sync | Skipped, retries next interval. |
| Pub/sub listener | Disconnected, reconnects automatically. |

## Multi-Instance Verification

After deploying multiple instances:

```bash
# Add API key via instance A's dashboard
# Verify via instance B:
curl -H "Authorization: Bearer $KEY" http://instance-b/v1/models
```

Should succeed within 5 seconds (pub/sub propagation).

## Monitoring

Watch for these log messages:

- `JWT secret rotated` -- expected during rotation
- `INITIAL ADMIN PASSWORD` -- first startup only
- `Admin password changed` -- expected after forced change
- `KeyStore: periodic sync failed` -- Redis connectivity issue
- `KeyStore: psubscribe failed` -- pub/sub connection issue
- `Redis unreachable, using stale cache` -- Redis hiccup, fail-open

## Rollback

1. Stop all instances.
2. `git revert` the security hardening commit(s).
3. Deploy old version.
4. Optional cleanup (if full revert needed):
   ```sql
   DROP TABLE IF EXISTS jwt_secrets;
   ALTER TABLE server_config
     DROP COLUMN IF EXISTS password_hash,
     DROP COLUMN IF EXISTS password_changed_at,
     DROP COLUMN IF EXISTS must_change_password;
   ```
