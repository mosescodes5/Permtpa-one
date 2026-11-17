-- PERMTPA ONE — 0009 fractional quantities. UNVERIFIED until run by CI / a developer (written without a database).
-- All stock quantities become integers in THOUSANDTHS of the product's unit (1 pcs = 1000, 2.5 kg = 2500).
-- Affected: variants.reorder_level, stock_levels.on_hand, stock_movements.delta, stock_movements.resulting_on_hand
-- Existing whole-number data is multiplied by 1000.
-- Audit rows: 'stock.adjust' rows written BEFORE this migration have NO 'qty_scale' key and hold whole-unit deltas;
-- rows written after carry "qty_scale": 1000 and hold thousandths. Readers must branch on that key.

-- Opt-in per product, chosen at creation. Not in the UPDATE column grant on purpose: the app role can never change it afterwards.
alter table products add column allow_fractional boolean not null default false;

-- Drop the old bounds first: they would reject the rescaled values.
alter table variants        drop constraint variants_reorder_level_check;
alter table stock_levels    drop constraint stock_levels_on_hand_check;
alter table stock_movements drop constraint stock_movements_delta_check;

alter table variants        alter column reorder_level      type bigint using reorder_level::bigint * 1000;
alter table stock_levels    alter column on_hand            type bigint using on_hand::bigint * 1000;
alter table stock_movements alter column delta              type bigint using delta::bigint * 1000;
alter table stock_movements alter column resulting_on_hand  type bigint using resulting_on_hand::bigint * 1000;

alter table variants        add constraint variants_reorder_level_check check (reorder_level between 0 and 100000000000);
alter table stock_levels    add constraint stock_levels_on_hand_check   check (on_hand >= 0);
alter table stock_movements add constraint stock_movements_delta_check  check (delta <> 0 and delta between -100000000000 and 100000000000);

-- The single write path for balances, now bigint-based and fraction-aware.
create or replace function app.apply_stock_movement() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
declare v_cur bigint; v_frac boolean;
begin
  select p.allow_fractional into v_frac
    from variants v join products p on p.org_id = v.org_id and p.id = v.product_id
   where v.id = new.variant_id and v.org_id = new.org_id;
  if v_frac is null then
    raise exception 'unknown variant' using errcode = 'P0001';
  end if;
  if not v_frac and new.delta % 1000 <> 0 then
    raise exception 'fractional quantity not allowed for this product' using errcode = 'P0001';
  end if;

  insert into stock_levels (variant_id, org_id, on_hand) values (new.variant_id, new.org_id, 0) on conflict (variant_id) do nothing;
  select on_hand into v_cur from stock_levels where variant_id = new.variant_id and org_id = new.org_id for update;
  if v_cur + new.delta < 0 then
    raise exception 'insufficient stock' using errcode = 'P0001';
  end if;
  update stock_levels set on_hand = v_cur + new.delta where variant_id = new.variant_id and org_id = new.org_id;
  new.resulting_on_hand := v_cur + new.delta;
  return new;
end $$;

-- Audit trigger function re-created so new stock.adjust rows carry the quantity scale.
create or replace function app.audit_inventory() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
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
      values (new.org_id, new.actor_user_id, 'stock.adjust', 'variant', new.variant_id::text, jsonb_build_object('delta', new.delta, 'reason', new.reason, 'resulting_on_hand', new.resulting_on_hand, 'qty_scale', 1000));
  end if;
  return null;
end $$;
