# Design: per-user data key with password and recovery-key unlock

Status: **design only, nothing built.** This doc goes with the "planned"
per-user encryption in [../SECURITY.md](../SECURITY.md) §4.2.

## 1. Goal and honest scope

Each user's sensitive data is encrypted under a per-user **data key** (DK).
Only that user's password or recovery key can unwrap it. The goals:

- A copy of the database file or a backup alone does not reveal the
  encrypted fields.
- The person who forgot their password can still get their data back with a
  recovery key.
- An admin can reset a password, but **cannot** unlock the data. A reset
  without the recovery key means the encrypted data is gone, and the UI says
  so plainly.

What this does **not** protect against: an operator who modifies the running
server, or reads memory while the user is signed in. While a user has an
active session the server holds their DK, so it can decrypt. This is
"encrypted at rest, decrypted in use", not end-to-end encryption.

### What gets encrypted (phase 1 vs later)

- **Phase 1** (small and well-defined): per-user secrets that are planned
  anyway. These are BYOK AI provider API keys and AI chat history, plus
  later MCP token metadata if it is useful.
- **Phase 2** (open question, §12): health data itself (`readings.value`,
  `workouts.detail_json`, `profiles` fields). This is costly, because
  dashboards query, sort and aggregate in SQL today. See §12 Q1.

## 2. Key hierarchy

```
password ──Argon2id(salt_pw, params)──▶ KEK_pw ─┐
                                                 ├─ each wraps ─▶ DK (32 random bytes)
recovery key ──Argon2id(salt_rk, params)──▶ KEK_rk ┘                │
                                                                    ▼
                                         XChaCha20-Poly1305(DK, nonce, aad = user_id|table|column|row_id)
                                                                    │
                                                              field ciphertexts
```

- **DK** is 32 bytes from the OS CSPRNG. It is created once per user and never
  changes when the password or recovery key changes. Changing DK means
  re-encrypting everything (§8.3).
- **KEK_pw** is `Argon2id(password, salt_pw)` with its own random 16-byte salt.
  It is separate from the login hash: different salt, and the login hash
  stays as it is today. A leaked login hash therefore does not give the KEK
  without the same guessing work.
  - Alternative to consider: derive both from one Argon2id run with HKDF
    split. That halves login CPU, but couples the two. See open question Q4.
- **KEK_rk** is `Argon2id(recovery_key_bytes, salt_rk)`. The recovery key has
  ≥128 bits of entropy, so light parameters would be enough. We still use
  Argon2id for uniformity, and because typing errors shrink the effective
  search space.
- **Wrapping** is `XChaCha20-Poly1305(KEK, nonce=random 24 B, aad="hv-dk-v1|"+user_id+"|"+slot)`.
  Each wrapped DK is stored with its nonce. Binding `user_id` and `slot` in
  the AAD stops someone swapping wraps between users or slots in the
  database.
- **Field encryption** is XChaCha20-Poly1305 under DK with a random nonce per
  value. The AAD binds user, table, column and row id, so a ciphertext cannot
  be copied to another row. The stored format is
  `version(1) || nonce(24) || ciphertext||tag`.
- **Crates:** `argon2` (already used), `chacha20poly1305` and `zeroize` for
  in-memory keys. No custom crypto.

## 3. Storage

New table (migration `00NN_user_keys.sql`):

```sql
CREATE TABLE user_keys (
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    slot         TEXT    NOT NULL,        -- 'password' | 'recovery'
    kdf          TEXT    NOT NULL,        -- 'argon2id-v19'
    kdf_params   TEXT    NOT NULL,        -- JSON {m_kib,t,p}
    salt         BLOB    NOT NULL,        -- 16 bytes
    nonce        BLOB    NOT NULL,        -- 24 bytes
    wrapped_dk   BLOB    NOT NULL,        -- 32 + 16 bytes
    key_version  INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT    NOT NULL,
    PRIMARY KEY (user_id, slot)
);
ALTER TABLE users ADD COLUMN dk_check BLOB;        -- AEAD(DK, "hv-dk-check") to verify unwrap
ALTER TABLE users ADD COLUMN recovery_confirmed_at TEXT;
```

- The recovery key itself is **never stored**, not even hashed. Only its wrap
  is kept. The KDF salt and the AEAD tag are enough to verify a correct entry
  (unwrap fails otherwise).
- **DK in memory.** On login the server unwraps DK and keeps it in a session
  cache in memory (`HashMap<session_hash, Zeroizing<[u8;32]>>`). It is
  dropped on logout, at session expiry, on restart and on password change.
  After a server restart, users must sign in again before encrypted fields
  can be read. Sessions stay valid but act as "locked" (see Q3).
- Background jobs that need DK (for example an import writing encrypted
  fields) take it from the uploading session at the moment of the request.

## 4. Recovery key format

- 160 random bits are encoded as Crockford base32, which gives 32 characters
  in **8 groups of 4**, e.g. `7KQ3-M9TX-2HDV-...`.
  - Crockford is case-insensitive and treats `I/L→1` and `O→0` as the same,
    with no `U`.
  - One more group carries a check: the first 20 bits of `sha256("hv-rk"||key)`,
    as 4 characters. That catches typos before the slow KDF runs.
- Why groups rather than a word list: they are easy to print and read aloud,
  and need no 2048-word list in the bundle. Option (Q2): a BIP-39-style word
  form (15 words) of the same bytes.
- Parsing: strip spaces and dashes, uppercase, map the confusable letters,
  then check the length and check group. Return a friendly error such as
  "group 5 looks wrong".
- Version prefix: optional `HV1-` so the format can change later.

## 5. UX screens

1. **Create** (on first sign-in after the feature ships, and for new users
   right after they set their own password). Title: "Save your recovery key".
   - The key is shown large, in groups, once.
   - Copy: "If you forget your password, this key is the only way to get your
     encrypted data back. We can't show it again. The server admin can't
     recover it either."
   - Buttons: **Download (.txt)**, **Print**, **Copy**.
   - The download is a small text file: the key, the server URL, the
     username, the date and one line of instructions. Nothing else.
2. **Confirm saved:** the user must type two randomly chosen groups
   (e.g. groups 3 and 7) before continuing. There is a "Show the key again"
   link while this screen is still open. Once they leave, it is gone.
   On success, set `recovery_confirmed_at`.
3. **Not confirmed yet:** a persistent banner reads "You haven't saved a
   recovery key. Without it, a forgotten password means losing your encrypted
   data." The only way to clear it is to regenerate (§8.2), because the old
   key is no longer shown.
4. **Forgot password** on the login screen:
   - Fields: username, recovery key, new password twice.
   - Then the regeneration prompt (§6).
5. **Settings → Security:**
   - "Recovery key: created <date>, confirmed ✓".
   - **Regenerate recovery key** (asks for the current password).
   - Explainer text linking to SECURITY.md.
6. **Admin reset warning** (§7): a red confirm dialog that requires typing
   the username.

The print view is a dedicated print stylesheet with only the key, the
instructions and the date. It has no app chrome and no health data.

## 6. Reset password with the recovery key

`POST /api/v1/recover {username, recovery_key, new_password}`. No session is
needed.

1. Rate limit per IP and per username. Share the lockout counters with login
   (5 failures → 15 min) so the endpoint can't be used as an oracle.
2. Load the `recovery` slot. Derive KEK_rk, unwrap DK and verify with
   `dk_check`. On any failure, return a generic `invalid_recovery` and record a
   failure.
3. Derive KEK_pw' from the new password with a fresh salt and re-wrap DK into
   the `password` slot. Also update `password_hash`, clear
   `must_change_password`, and **delete all of the user's sessions**. All of
   this happens in one transaction.
4. Create a new session with DK cached.
5. **Recommend regenerating the recovery key**: the old one was just typed
   somewhere and may have been exposed. Show this as a strong prompt, not
   forced (Q5).

DK is unchanged, so no data needs to be re-encrypted.

## 7. Admin reset and losing the data key

There is no admin password reset in the code today. This design adds one with
explicit loss semantics.

`POST /api/v1/admin/users/{id}/reset-password {temporary_password, confirm_username}`:

- It sets a temporary password (must-change on next login), deletes the
  user's sessions, and **deletes both `user_keys` rows and every encrypted
  field for that user**. For phase-1 data that means AI keys and history; for
  phase-2 data, all encrypted health fields.
  - Why delete rather than keep: ciphertext without a key is useless, and
    keeping it misleads.
  - Alternative: keep it for N days in case the user finds their recovery
    key. That requires keeping the old wrap, which an admin could pair with a
    later recovery-key entry (Q6).
- On next login the user gets a new DK and a new recovery key (screen 1).
- Plain-text data (everything not yet encrypted) survives the reset as today.
- The admin UI must say exactly what will be lost before confirming:
  > "Sam's encrypted data (AI keys, AI history[, health data]) will be
  > permanently deleted. Only Sam's recovery key could prevent this. If Sam
  > has it, ask Sam to use 'Forgot password' instead."
- An audit log entry (`admin_events`: who, whom, when, what) is written.
  It is visible to the affected user on next login.

**Losing both** password and recovery key with no admin: the encrypted data is
unrecoverable. This is the point of the design and is stated in SECURITY.md.

## 8. Rotation

### 8.1 Password change (signed in)

Verify the current password, which unwraps DK. Re-wrap DK under a new
KEK_pw with a new salt. Update the login hash. Delete the user's **other**
sessions (today a password change does not, see SECURITY.md §5).

### 8.2 Regenerate recovery key

This requires the current password, even though the DK is cached, to protect
an unattended session. It generates a new key, wraps DK into the `recovery`
slot (replacing the old wrap, so the old key stops working immediately) and
shows screens 1 and 2 again.

### 8.3 Rotate the DK itself (compromise response)

This is rare and admin-triggered or user-triggered. It generates DK', then
re-encrypts all fields in batches, keeping `key_version` on each ciphertext
so a half-finished run can resume. Then it re-wraps both slots. It needs the
user signed in, or the recovery key. Phase 1 is small; phase 2 could take
minutes. **Planned after v1.**

### 8.4 KDF parameter upgrades

`kdf_params` is stored per slot. On successful login, if the parameters are
below the current defaults, re-wrap silently.

## 9. Migrating existing users

Existing users have no DK. Data moves to encrypted form lazily, per user:

1. Migration adds the tables and columns. Nothing else changes, and
   unencrypted data keeps working.
2. At the user's next successful login (the server has the password only
   then) it generates DK and wraps it into the password slot. It encrypts
   phase-1 data in the same request; phase-2 data goes to a background batch
   job holding DK. Then it shows the recovery-key screens.
3. Users who never log in stay unencrypted, and the admin page shows
   "not yet protected". Encryption **cannot** happen without the user's
   password, by design.
4. Each field records its own format (`version` byte, or a NULL ciphertext
   column alongside the legacy plain column during the transition), so mixed
   states are readable.
5. After migration, run `VACUUM` to purge plain-text copies from free pages
   and tell the operator that **old backups still contain plain text**.

## 10. Tests to write

- Unit, crypto:
  - wrap/unwrap round trip for each slot;
  - a wrong password or key fails;
  - swapping a wrap across users or slots fails (AAD);
  - a field ciphertext copied to another row fails;
  - tampered bytes fail.
- Recovery key format:
  - round trip;
  - case, space, dash and confusable-letter tolerance;
  - the check group catches every single-character substitution;
  - a group-level error message.
- Flows (API tests):
  - signup → key shown once, and the confirm step needs the right groups;
  - forgot password with the recovery key → new password works, old sessions
    gone, data readable, DK unchanged;
  - recovery with a wrong key → generic error and lockout after 5;
  - password change → data still readable, other sessions revoked;
  - regenerate → old key rejected, new key works;
  - admin reset → encrypted data gone, plain data intact, audit entry
    present, user gets a new key on login;
  - admin can never call an endpoint that returns another user's decrypted
    data;
  - server restart → session is "locked" until re-login (per the Q3
    decision).
- Migration:
  - a pre-feature database with users → first login creates a DK and
    encrypts phase-1 data;
  - a user who never logs in stays readable;
  - a half-migrated state reads correctly.
- Negative storage test: after setup, grep the SQLite file bytes for the
  recovery key, the password and a known secret value. Expect no match.

## 11. Non-goals

- End-to-end encryption, where the browser holds the keys and the server never
  sees plaintext. The server must aggregate data, and a later AI feature needs
  plaintext server-side.
- Hardware keys or passkeys as an unlock slot (could be added later as
  another `slot`).
- Protecting against a malicious operator changing server code.

## 12. Open questions for Toby

1. **Scope:** phase 1 only (AI keys and history), or also health data
   (phase 2)? Phase 2 means SQL can't filter or aggregate on encrypted
   columns. Either keep the type and time columns plain and encrypt only
   values, or decrypt in Rust per query (slower and a big refactor of
   `stats`/`browse`).
2. **Format:** grouped Crockford base32 (proposed), or a word list? Or offer
   both views of the same bytes?
3. **After a server restart:** should signed-in users be forced to log in
   again (simplest and safest), or should their sessions remain with
   encrypted features "locked" until they re-enter the password?
4. **One or two Argon2id runs at login?** Two separate runs are cleaner;
   one with an HKDF split saves around 0.5 s of CPU per login on small
   hardware.
5. After recovering with the key, should regenerating it be **forced** or
   only strongly suggested?
6. On admin reset: delete the encrypted data immediately (proposed), or
   keep the ciphertext and recovery wrap for a grace period?
7. Should the **first admin** be allowed to skip the recovery key, since
   they control the box? Proposed: no; same rules for everyone.
8. Should background or scheduled jobs (planned) ever be able to decrypt
   without a live session? That needs a server-held key and breaks the
   "operator can't read at rest" property. Proposed: no.
