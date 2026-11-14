-- PERMTPA ONE — 0008 inventory: categories, products, variants, costs, append-only stock movements.
-- Stock levels are NEVER written directly: they change only through stock_movements (trigger), so history always explains the balance.

create function app.can_manage_inventory() returns boolean language sql stable as $$
  select coalesce(app.current_role() in ('owner','administrator','manager','inventory_manager'), false) $$;
-- Cost prices are financial data: Sales Staff and Read Only never see them.
create function app.can_view_costs() returns boolean language sql stable as $$
  select coalesce(app.current_role() in ('owner','administrator','manager','accountant','inventory_manager'), false) $$;
grant execute on function app.can_manage_inventory(), app.can_view_costs() to permtpa_app;

create table categories (
  id         uuid primary key default gen_random_uuid(),
  org_id     uuid not null references organizations(id) on delete cascade,
  name       citext not null check (length(name) between 1 and 80),
  created_at timestamptz not null default now(),
  unique (org_id, id), unique (org_id, name)
);

create table products (
  id          uuid primary key default gen_random_uuid(),
  org_id      uuid not null references organizations(id) on delete cascade,
  category_id uuid,
  name        text not null check (length(name) between 1 and 160),
  description text check (length(description) <= 2000),
  unit        text not null default 'pcs' check (length(unit) between 1 and 20),
  status      text not null default 'active' check (status in ('active','archived')),
  created_by  uuid not null references users(id),
  created_at  timestamptz not null default now(),
  updated_at  timestamptz not null default now(),
  unique (org_id, id),
  foreign key (org_id, category_id) references categories (org_id, id)      -- a product can only use its OWN org's categories
);
create index products_org_name_idx on products (org_id, lower(name));
create trigger products_updated before update on products for each row execute function app.set_updated_at();

create table variants (
  id            uuid primary key default gen_random_uuid(),
  org_id        uuid not null references organizations(id) on delete cascade,
  product_id    uuid not null,
  name          text not null default 'Default' check (length(name) between 1 and 80),
  sku           citext not null check (sku ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,39}$'),
  barcode       text check (barcode ~ '^[0-9A-Za-z-]{4,32}$'),
  price_minor   bigint not null check (price_minor between 0 and 1000000000000),
  reorder_level integer not null default 0 check (reorder_level between 0 and 100000000),
  status        text not null default 'active' check (status in ('active','archived')),
  created_at    timestamptz not null default now(),
  updated_at    timestamptz not null default now(),
  unique (org_id, id), unique (org_id, sku),
  foreign key (org_id, product_id) references products (org_id, id)
);
create unique index variants_org_barcode_uq on variants (org_id, barcode) where barcode is not null;
create index variants_product_idx on variants (org_id, product_id);
create trigger variants_updated before update on variants for each row execute function app.set_updated_at();

create table variant_costs (
  variant_id uuid primary key,
  org_id     uuid not null,
  cost_minor bigint not null check (cost_minor between 0 and 1000000000000),
  updated_at timestamptz not null default now(),
  foreign key (org_id, variant_id) references variants (org_id, id) on delete cascade
);

create table stock_levels (
  variant_id uuid primary key,
  org_id     uuid not null,
  on_hand    integer not null default 0 check (on_hand >= 0),
  foreign key (org_id, variant_id) references variants (org_id, id)
);

create table stock_movements (
  id               bigint generated always as identity primary key,
  org_id           uuid not null,
  variant_id       uuid not null,
  delta            integer not null check (delta <> 0 and delta between -100000000 and 100000000),
  reason           text not null check (reason in ('opening','purchase','sale','return_in','adjustment','damage','correction')),
  note             text check (length(note) <= 200),
  resulting_on_hand integer,
  actor_user_id    uuid not null references users(id),
  created_at       timestamptz not null default now(),
  foreign key (org_id, variant_id) references variants (org_id, id)
);
create index stock_movements_variant_idx on stock_movements (org_id, variant_id, id desc);

-- The ONLY way a balance changes. Locks the level row (serialises concurrent writers), refuses negative stock,
-- and records the balance after the movement.
create function app.apply_stock_movement() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
declare v_cur integer;
begin
  insert into stock_levels (variant_id, org_id, on_hand) values (new.variant_id, new.org_id, 0) on conflict (variant_id) do nothing;
  select on_hand into v_cur from stock_levels where variant_id = new.variant_id and org_id = new.org_id for update;
  if v_cur + new.delta < 0 then
    raise exception 'insufficient stock' using errcode = 'P0001';
  end if;
  update stock_levels set on_hand = v_cur + new.delta where variant_id = new.variant_id and org_id = new.org_id;
  new.resulting_on_hand := v_cur + new.delta;
  return new;
end $$;
create trigger stock_movements_apply before insert on stock_movements for each row execute function app.apply_stock_movement();

create function app.stock_movements_immutable() returns trigger language plpgsql as $$
begin raise exception 'stock_movements is append-only' using errcode = 'P0001'; end $$;
create trigger stock_movements_no_change before update or delete on stock_movements for each row execute function app.stock_movements_immutable();
create trigger stock_movements_no_truncate before truncate on stock_movements for each statement execute function app.stock_movements_immutable();

-- ---------------- Row Level Security ----------------
alter table categories enable row level security;  alter table products enable row level security;
alter table variants enable row level security;    alter table variant_costs enable row level security;
alter table stock_levels enable row level security; alter table stock_movements enable row level security;

create policy categories_select on categories for select to permtpa_app using (org_id = app.current_org_id());
create policy categories_write  on categories for all    to permtpa_app using (org_id = app.current_org_id() and app.can_manage_inventory()) with check (org_id = app.current_org_id() and app.can_manage_inventory());
create policy products_select on products for select to permtpa_app using (org_id = app.current_org_id());
create policy products_insert on products for insert to permtpa_app with check (org_id = app.current_org_id() and app.can_manage_inventory() and created_by = app.current_user_id());
create policy products_update on products for update to permtpa_app using (org_id = app.current_org_id() and app.can_manage_inventory()) with check (org_id = app.current_org_id() and app.can_manage_inventory());
create policy variants_select on variants for select to permtpa_app using (org_id = app.current_org_id());
create policy variants_insert on variants for insert to permtpa_app with check (org_id = app.current_org_id() and app.can_manage_inventory());
create policy variants_update on variants for update to permtpa_app using (org_id = app.current_org_id() and app.can_manage_inventory()) with check (org_id = app.current_org_id() and app.can_manage_inventory());
create policy costs_select on variant_costs for select to permtpa_app using (org_id = app.current_org_id() and app.can_view_costs());
create policy costs_insert on variant_costs for insert to permtpa_app with check (org_id = app.current_org_id() and app.can_manage_inventory());
create policy costs_update on variant_costs for update to permtpa_app using (org_id = app.current_org_id() and app.can_manage_inventory()) with check (org_id = app.current_org_id() and app.can_manage_inventory());
create policy levels_select on stock_levels for select to permtpa_app using (org_id = app.current_org_id());
create policy movements_select on stock_movements for select to permtpa_app using (org_id = app.current_org_id());
-- 'sale' is reserved for the sales module (Milestone 4); manual entry covers the other reasons.
create policy movements_insert on stock_movements for insert to permtpa_app
  with check (org_id = app.current_org_id() and app.can_manage_inventory() and actor_user_id = app.current_user_id()
              and reason in ('opening','purchase','return_in','adjustment','damage','correction'));

-- Least-privilege grants: no DELETE on products/variants (archive instead); stock_levels is read-only to the app.
grant select, insert, delete on categories to permtpa_app;      grant update (name) on categories to permtpa_app;
grant select, insert on products to permtpa_app;                grant update (name, description, category_id, unit, status, updated_at) on products to permtpa_app;
grant select, insert on variants to permtpa_app;                grant update (name, sku, barcode, price_minor, reorder_level, status, updated_at) on variants to permtpa_app;
grant select, insert on variant_costs to permtpa_app;           grant update (cost_minor, updated_at) on variant_costs to permtpa_app;
grant select on stock_levels to permtpa_app;
grant select, insert on stock_movements to permtpa_app;         grant usage on sequence stock_movements_id_seq to permtpa_app;

-- ---------------- Audit trail (actor from transaction context, never from the client) ----------------
create function app.audit_inventory() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if tg_table_name = 'products' then
    if tg_op = 'INSERT' then
      insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata) values (new.org_id, app.current_user_id(), 'product.create', 'product', new.id::text, jsonb_build_object('name', new.name));
    elsif old.status is distinct from new.status then
      insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata) values (new.org_id, app.current_user_id(), case when new.status = 'archived' then 'product.archive' else 'product.restore' end, 'product', new.id::text, '{}');
    elsif old.name is distinct from new.name or old.category_id is distinct from new.category_id then
      insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata) values (new.org_id, app.current_user_id(), 'product.update', 'product', new.id::text, jsonb_build_object('from_name', old.name, 'to_name', new.name));
    end if;
  elsif tg_table_name = 'variants' then
    if tg_op = 'INSERT' then
      insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata) values (new.org_id, app.current_user_id(), 'variant.create', 'variant', new.id::text, jsonb_build_object('sku', new.sku));
    elsif old.price_minor is distinct from new.price_minor then
      insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata) values (new.org_id, app.current_user_id(), 'variant.price_change', 'variant', new.id::text, jsonb_build_object('from_minor', old.price_minor, 'to_minor', new.price_minor));
    elsif old.sku is distinct from new.sku or old.status is distinct from new.status then
      insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata) values (new.org_id, app.current_user_id(), 'variant.update', 'variant', new.id::text, jsonb_build_object('from_sku', old.sku, 'to_sku', new.sku, 'from_status', old.status, 'to_status', new.status));
    end if;
  elsif tg_table_name = 'variant_costs' then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (new.org_id, app.current_user_id(), 'variant.cost_change', 'variant', new.variant_id::text, jsonb_build_object('from_minor', case when tg_op = 'UPDATE' then old.cost_minor end, 'to_minor', new.cost_minor));
  elsif tg_table_name = 'stock_movements' and new.reason in ('adjustment','damage','correction') then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (new.org_id, new.actor_user_id, 'stock.adjust', 'variant', new.variant_id::text, jsonb_build_object('delta', new.delta, 'reason', new.reason, 'resulting_on_hand', new.resulting_on_hand));
  end if;
  return null;
end $$;
create trigger products_audit      after insert or update on products       for each row execute function app.audit_inventory();
create trigger variants_audit      after insert or update on variants       for each row execute function app.audit_inventory();
create trigger variant_costs_audit after insert or update on variant_costs  for each row execute function app.audit_inventory();
create trigger stock_movements_audit after insert on stock_movements       for each row execute function app.audit_inventory();
