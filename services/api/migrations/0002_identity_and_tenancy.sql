-- PERMTPA ONE — 0002 identity + tenancy tables.
create table business_types (
  code  text primary key,
  label text not null
);
insert into business_types (code, label) values
  ('retail','Retail'),('agriculture','Agriculture'),('restaurant','Restaurant'),('distribution','Distribution'),
  ('services','Services'),('agency','Agency'),('other','Other');

create table users (
  id                 uuid primary key default gen_random_uuid(),
  email              citext not null unique check (length(email) <= 254),
  full_name          text not null check (length(full_name) between 2 and 100),
  password_hash      text not null,                 -- Argon2id PHC string. Never selectable by the app role.
  email_verified_at  timestamptz,
  status             text not null default 'active' check (status in ('active','suspended','deleted')),
  failed_login_count integer not null default 0 check (failed_login_count >= 0),
  locked_until       timestamptz,
  mfa_enabled        boolean not null default false,
  created_at         timestamptz not null default now(),
  updated_at         timestamptz not null default now()
);
create trigger users_updated before update on users for each row execute function app.set_updated_at();

create table sessions (
  id           uuid primary key default gen_random_uuid(),
  user_id      uuid not null references users(id) on delete cascade,
  token_hash   bytea not null unique check (octet_length(token_hash) = 32),  -- SHA-256 of the opaque cookie token
  created_at   timestamptz not null default now(),
  last_seen_at timestamptz not null default now(),
  expires_at   timestamptz not null,
  revoked_at   timestamptz,
  user_agent   text check (length(user_agent) <= 512),
  ip           inet
);
create index sessions_user_idx on sessions (user_id) where revoked_at is null;

create table organizations (
  id            uuid primary key default gen_random_uuid(),
  name          text not null check (length(name) between 2 and 120),
  slug          citext not null unique check (slug ~ '^[a-z0-9]([a-z0-9-]{1,48}[a-z0-9])$'),
  business_type text not null references business_types(code),
  currency      char(3) not null default 'NGN' check (currency ~ '^[A-Z]{3}$'),
  created_by    uuid not null references users(id),
  created_at    timestamptz not null default now(),
  updated_at    timestamptz not null default now()
);
create trigger organizations_updated before update on organizations for each row execute function app.set_updated_at();

create table memberships (
  id         uuid primary key default gen_random_uuid(),
  org_id     uuid not null references organizations(id) on delete cascade,
  user_id    uuid not null references users(id) on delete cascade,
  role       org_role not null,
  status     text not null default 'active' check (status in ('active','suspended')),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (org_id, user_id)
);
create index memberships_user_idx on memberships (user_id);
create trigger memberships_updated before update on memberships for each row execute function app.set_updated_at();

create table invitations (
  id          uuid primary key default gen_random_uuid(),
  org_id      uuid not null references organizations(id) on delete cascade,
  email       citext not null check (length(email) <= 254),
  role        org_role not null check (role <> 'owner'),   -- ownership is never granted by invitation
  token_hash  bytea not null unique check (octet_length(token_hash) = 32),
  invited_by  uuid not null references users(id),
  expires_at  timestamptz not null,
  accepted_at timestamptz,
  revoked_at  timestamptz,
  created_at  timestamptz not null default now()
);
create index invitations_org_idx on invitations (org_id);

create table audit_logs (
  id            bigint generated always as identity primary key,
  org_id        uuid references organizations(id),       -- NULL for pre-tenant events (e.g. failed login)
  actor_user_id uuid references users(id),
  action        text not null check (length(action) between 3 and 100),
  target_type   text check (length(target_type) <= 60),
  target_id     text check (length(target_id) <= 100),
  metadata      jsonb not null default '{}'::jsonb,       -- NEVER passwords, tokens, secrets or card data
  ip            inet,
  created_at    timestamptz not null default now()
);
create index audit_logs_org_time_idx on audit_logs (org_id, created_at desc);
