-- PERMTPA ONE — 0010 business-type onboarding + deletable sample catalogue. UNVERIFIED until run by CI / a developer.
--
-- 1. Business types carry their CAPABILITY FLAGS in the database (tracks_expiry, sold_by_weight, has_tables, has_bookings).
--    An organization's capabilities are always read through its business_type, so they can never drift or be forged by
--    the app role (it has SELECT only on business_types).
-- 2. The picker shows 12 types (`selectable`). The four older codes (retail, agriculture, distribution, agency) stay VALID
--    so existing organizations keep working, but are no longer offered.
-- 3. Organizations get an optional phone number.
-- 4. Sample catalogue: reference data in sample_catalogue_items (invisible to the app role: it holds cost prices), loaded and
--    removed ONLY through SECURITY DEFINER functions. Rows are flagged by products.sample_key / stock_movements.is_sample,
--    which the app role can never set (RLS WITH CHECK) or change (no UPDATE grant).
--    NOTE ON APPEND-ONLY: stock_movements stays append-only except for ONE narrow exception: rows flagged is_sample, deleted
--    inside app.delete_sample_catalogue() (which sets a transaction-local flag). The app role has no DELETE privilege at all.

-- ---------------------------------------------------------------- business types
alter table business_types
  add column description     text    not null default '' check (length(description) <= 200),
  add column sort_order      integer not null default 900,
  add column selectable      boolean not null default true,
  add column tracks_expiry   boolean not null default false,
  add column sold_by_weight  boolean not null default false,
  add column has_tables      boolean not null default false,
  add column has_bookings    boolean not null default false;

-- Older codes: still valid, no longer offered.
update business_types set selectable = false, sort_order = 900 where code in ('retail','agriculture','distribution','agency');
update business_types set sold_by_weight = true where code = 'agriculture';

-- Existing codes that are also in the new list.
update business_types set label = 'Restaurant', description = 'Meals, drinks, quick service', sort_order = 30, has_tables = true where code = 'restaurant';
update business_types set label = 'Services', description = 'Bookings, repairs, consultations', sort_order = 90, has_bookings = true where code = 'services';
update business_types set label = 'Other', description = 'Doesn''t fit the categories above', sort_order = 120 where code = 'other';

insert into business_types (code, label, description, sort_order, tracks_expiry, sold_by_weight, has_tables, has_bookings) values
  ('electronics',        'Electronics',           'Phones, gadgets, accessories',                                   10,  false, false, false, false),
  ('fashion',            'Fashion',               'Clothes, shoes, Ankara, tailoring',                              20,  false, false, false, false),
  ('pharmacy',           'Pharmacy',              'OTC, prescriptions, sundries',                                   40,  true,  false, false, false),
  ('salon_spa',          'Salon and spa',         'Hair, nails, beauty services',                                   50,  false, false, false, true),
  ('supermarket',        'Supermarket',           'Groceries and household',                                        60,  false, true,  false, false),
  ('building_materials', 'Building materials',    'Cement, sand, tiles, cables — sold by bag, weight or roll',      70,  false, true,  false, false),
  ('car_wash',           'Car wash',              'Wash packages and add-ons',                                      80,  false, false, false, false),
  ('hospitality',        'Hotel and hospitality', 'Rooms, guests, front desk sales',                                100, false, false, false, true),
  ('general_trade',      'General trade',         'Mixed retail or services',                                       110, false, false, false, false);

-- ---------------------------------------------------------------- organizations: optional phone
alter table organizations add column phone text check (phone is null or phone ~ '^\+?[0-9 ()-]{7,20}$');
grant update (phone) on organizations to permtpa_app;

-- ---------------------------------------------------------------- sample flags
alter table products add column sample_key text check (sample_key is null or sample_key ~ '^[a-z0-9-]{3,60}$');
create unique index products_org_sample_key_uq on products (org_id, sample_key) where sample_key is not null;
-- sample_key is deliberately NOT in the products UPDATE column grant: the app role can never change it.

alter table stock_movements add column is_sample boolean not null default false;

-- The app role can never CREATE flagged rows (only the SECURITY DEFINER loader below can).
drop policy products_insert on products;
create policy products_insert on products for insert to permtpa_app
  with check (org_id = app.current_org_id() and app.can_manage_inventory() and created_by = app.current_user_id() and sample_key is null);

drop policy movements_insert on stock_movements;
create policy movements_insert on stock_movements for insert to permtpa_app
  with check (org_id = app.current_org_id() and app.can_manage_inventory() and actor_user_id = app.current_user_id()
              and reason in ('opening','purchase','return_in','adjustment','damage','correction') and is_sample = false);

-- Append-only, with ONE exception: a flagged sample movement may be deleted while app.delete_sample_catalogue() runs.
-- Nested IFs on purpose: for TRUNCATE (statement trigger) OLD does not exist and must not be touched.
create or replace function app.stock_movements_immutable() returns trigger language plpgsql as $$
begin
  if tg_level = 'ROW' and tg_op = 'DELETE' then
    if old.is_sample and current_setting('app.sample_purge', true) = 'on' then
      return old;
    end if;
  end if;
  raise exception 'stock_movements is append-only' using errcode = 'P0001';
end $$;

-- ---------------------------------------------------------------- sample catalogue reference data
create table sample_catalogue_items (
  sample_key       text primary key check (sample_key ~ '^[a-z0-9-]{3,60}$'),
  business_type    text    not null references business_types(code),
  sku              text    not null unique check (sku ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,39}$'),
  name             text    not null check (length(name) between 1 and 160),
  unit             text    not null check (length(unit) between 1 and 20),
  allow_fractional boolean not null default false,
  price_minor      bigint  not null check (price_minor between 0 and 1000000000000),
  cost_minor       bigint  not null check (cost_minor between 0 and 1000000000000),
  opening_qty      bigint  not null check (opening_qty between 0 and 100000000000),   -- thousandths
  reorder_level    bigint  not null check (reorder_level between 0 and 100000000000), -- thousandths
  check (cost_minor <= price_minor),
  check (allow_fractional or (opening_qty % 1000 = 0 and reorder_level % 1000 = 0))   -- whole-unit rule, same as the stock trigger
);
-- Holds cost prices: never visible to the app role (no grant, and RLS with no policy as a second wall).
revoke all on sample_catalogue_items from permtpa_app;
alter table sample_catalogue_items enable row level security;

insert into sample_catalogue_items (sample_key, business_type, sku, name, unit, allow_fractional, price_minor, cost_minor, opening_qty, reorder_level) values
  ('electronics-01','electronics','SMP-ELECTRONICS-01','Smartphone 64GB','pcs',false,8500000,7200000,8000,2000),
  ('electronics-02','electronics','SMP-ELECTRONICS-02','Phone charger 20W','pcs',false,450000,280000,25000,5000),
  ('electronics-03','electronics','SMP-ELECTRONICS-03','Wireless earbuds','pcs',false,1200000,750000,15000,4000),
  ('electronics-04','electronics','SMP-ELECTRONICS-04','Power bank 10000mAh','pcs',false,1500000,1050000,10000,3000),
  ('electronics-05','electronics','SMP-ELECTRONICS-05','USB-C cable 1m','pcs',false,200000,90000,40000,10000),
  ('fashion-01','fashion','SMP-FASHION-01','Ankara fabric 6 yards','pcs',false,1200000,800000,20000,5000),
  ('fashion-02','fashion','SMP-FASHION-02','Polo shirt (men)','pcs',false,850000,500000,18000,5000),
  ('fashion-03','fashion','SMP-FASHION-03','Ladies sandals','pair',false,900000,550000,12000,3000),
  ('fashion-04','fashion','SMP-FASHION-04','Canvas sneakers','pair',false,1800000,1100000,10000,3000),
  ('fashion-05','fashion','SMP-FASHION-05','Leather belt','pcs',false,500000,250000,15000,4000),
  ('restaurant-01','restaurant','SMP-RESTAURANT-01','Bottled water 75cl','pcs',false,30000,18000,120000,24000),
  ('restaurant-02','restaurant','SMP-RESTAURANT-02','Soft drink 50cl','pcs',false,50000,35000,96000,24000),
  ('restaurant-03','restaurant','SMP-RESTAURANT-03','Meat pie','pcs',false,70000,40000,40000,10000),
  ('restaurant-04','restaurant','SMP-RESTAURANT-04','Bread loaf','pcs',false,120000,90000,30000,10000),
  ('restaurant-05','restaurant','SMP-RESTAURANT-05','Vegetable oil 5 litre','pcs',false,1400000,1150000,6000,2000),
  ('pharmacy-01','pharmacy','SMP-PHARMACY-01','Paracetamol 500mg (pack of 20)','pack',false,60000,40000,60000,15000),
  ('pharmacy-02','pharmacy','SMP-PHARMACY-02','Vitamin C 1000mg (tube)','pack',false,250000,170000,30000,8000),
  ('pharmacy-03','pharmacy','SMP-PHARMACY-03','Bandage roll','pcs',false,80000,50000,40000,10000),
  ('pharmacy-04','pharmacy','SMP-PHARMACY-04','Hand sanitiser 250ml','pcs',false,180000,110000,24000,6000),
  ('pharmacy-05','pharmacy','SMP-PHARMACY-05','Oral rehydration salts sachet','pcs',false,30000,18000,100000,20000),
  ('salon-spa-01','salon_spa','SMP-SALON-SPA-01','Shampoo 500ml','pcs',false,450000,300000,12000,3000),
  ('salon-spa-02','salon_spa','SMP-SALON-SPA-02','Hair relaxer kit','pcs',false,650000,420000,10000,3000),
  ('salon-spa-03','salon_spa','SMP-SALON-SPA-03','Braiding hair pack','pack',false,250000,160000,40000,10000),
  ('salon-spa-04','salon_spa','SMP-SALON-SPA-04','Nail polish','pcs',false,150000,80000,30000,8000),
  ('salon-spa-05','salon_spa','SMP-SALON-SPA-05','Body lotion 400ml','pcs',false,350000,230000,16000,4000),
  ('supermarket-01','supermarket','SMP-SUPERMARKET-01','Rice, loose (per kg)','kg',true,180000,145000,80000,20000),
  ('supermarket-02','supermarket','SMP-SUPERMARKET-02','Sugar, loose (per kg)','kg',true,160000,130000,45500,10000),
  ('supermarket-03','supermarket','SMP-SUPERMARKET-03','Spaghetti 500g','pack',false,120000,90000,50000,12000),
  ('supermarket-04','supermarket','SMP-SUPERMARKET-04','Tomato paste 70g tin','pcs',false,40000,28000,100000,24000),
  ('supermarket-05','supermarket','SMP-SUPERMARKET-05','Detergent 1kg','pcs',false,280000,200000,24000,6000),
  ('building-materials-01','building_materials','SMP-BUILDING-MATERIALS-01','Cement 50kg bag','bag',false,950000,880000,200000,40000),
  ('building-materials-02','building_materials','SMP-BUILDING-MATERIALS-02','Sharp sand (per tonne)','tonne',true,1800000,1400000,12500,3000),
  ('building-materials-03','building_materials','SMP-BUILDING-MATERIALS-03','Iron rod 12mm','length',false,900000,780000,120000,30000),
  ('building-materials-04','building_materials','SMP-BUILDING-MATERIALS-04','Electric cable 2.5mm (roll)','roll',false,3200000,2600000,15000,4000),
  ('building-materials-05','building_materials','SMP-BUILDING-MATERIALS-05','Nails 3 inch (per kg)','kg',true,220000,160000,60250,10000),
  ('building-materials-06','building_materials','SMP-BUILDING-MATERIALS-06','Floor tiles 40x40 (carton)','carton',false,1150000,900000,40000,10000),
  ('car-wash-01','car_wash','SMP-CAR-WASH-01','Car shampoo 5 litre','pcs',false,650000,450000,8000,2000),
  ('car-wash-02','car_wash','SMP-CAR-WASH-02','Tyre shine spray','pcs',false,280000,170000,15000,4000),
  ('car-wash-03','car_wash','SMP-CAR-WASH-03','Microfibre cloth','pcs',false,100000,50000,40000,10000),
  ('car-wash-04','car_wash','SMP-CAR-WASH-04','Air freshener','pcs',false,120000,60000,30000,8000),
  ('car-wash-05','car_wash','SMP-CAR-WASH-05','Wax polish 500g','pcs',false,450000,300000,10000,3000),
  ('services-01','services','SMP-SERVICES-01','LED bulb 12W','pcs',false,80000,50000,50000,12000),
  ('services-02','services','SMP-SERVICES-02','Extension socket 4-way','pcs',false,450000,300000,15000,4000),
  ('services-03','services','SMP-SERVICES-03','Phone screen protector','pcs',false,150000,60000,40000,10000),
  ('services-04','services','SMP-SERVICES-04','Insulating tape','pcs',false,50000,25000,60000,15000),
  ('services-05','services','SMP-SERVICES-05','Fuse pack','pack',false,70000,35000,30000,8000),
  ('hospitality-01','hospitality','SMP-HOSPITALITY-01','Bottled water 75cl','pcs',false,50000,30000,120000,24000),
  ('hospitality-02','hospitality','SMP-HOSPITALITY-02','Soft drink 35cl','pcs',false,70000,45000,96000,24000),
  ('hospitality-03','hospitality','SMP-HOSPITALITY-03','Guest toiletries kit','pcs',false,150000,80000,60000,15000),
  ('hospitality-04','hospitality','SMP-HOSPITALITY-04','Bath towel','pcs',false,600000,400000,30000,8000),
  ('hospitality-05','hospitality','SMP-HOSPITALITY-05','Snack pack','pcs',false,100000,60000,50000,12000),
  ('general-trade-01','general_trade','SMP-GENERAL-TRADE-01','Notebook A5','pcs',false,60000,35000,80000,20000),
  ('general-trade-02','general_trade','SMP-GENERAL-TRADE-02','Ballpoint pens (box of 10)','box',false,120000,80000,30000,8000),
  ('general-trade-03','general_trade','SMP-GENERAL-TRADE-03','Plastic bucket 15 litre','pcs',false,250000,160000,20000,5000),
  ('general-trade-04','general_trade','SMP-GENERAL-TRADE-04','Candles (pack)','pack',false,100000,60000,40000,10000),
  ('general-trade-05','general_trade','SMP-GENERAL-TRADE-05','Matches (box)','box',false,15000,9000,100000,20000),
  ('other-01','other','SMP-OTHER-01','Carrier bags (pack)','pack',false,100000,60000,40000,10000),
  ('other-02','other','SMP-OTHER-02','Receipt paper roll','pcs',false,80000,50000,50000,12000),
  ('other-03','other','SMP-OTHER-03','Cleaning cloth','pcs',false,50000,25000,60000,15000),
  ('other-04','other','SMP-OTHER-04','Padlock','pcs',false,250000,150000,20000,5000),
  ('other-05','other','SMP-OTHER-05','Marker pen','pcs',false,40000,20000,60000,15000)
;

-- ---------------------------------------------------------------- load / delete / status (SECURITY DEFINER; pinned search_path)
-- Loading is idempotent: a sample that is already present (same sample_key) is skipped; a sample whose SKU is already used by
-- a real item is skipped and counted. Only Owner / Administrator / Manager / Inventory Manager (app.can_manage_inventory()).
create function app.load_sample_catalogue(p_org uuid) returns jsonb
language plpgsql security definer set search_path = public, pg_temp as $$
declare
  v_user uuid := app.current_user_id();
  v_type text;
  i record;
  v_pid uuid;
  v_vid uuid;
  v_loaded integer := 0;
  v_present integer := 0;
  v_skipped integer := 0;
begin
  if v_user is null or p_org is distinct from app.current_org_id() or not app.can_manage_inventory() then
    raise exception 'not allowed' using errcode = '42501';
  end if;
  select o.business_type into v_type from organizations o where o.id = p_org;

  for i in select * from sample_catalogue_items s where s.business_type = v_type order by s.sample_key loop
    if exists (select 1 from products p where p.org_id = p_org and p.sample_key = i.sample_key) then
      v_present := v_present + 1;
      continue;
    end if;
    if exists (select 1 from variants v where v.org_id = p_org and v.sku = i.sku::citext) then
      v_skipped := v_skipped + 1;
      continue;
    end if;
    v_pid := null;
    insert into products (org_id, name, unit, allow_fractional, created_by, sample_key)
      values (p_org, i.name, i.unit, i.allow_fractional, v_user, i.sample_key)
      on conflict (org_id, sample_key) where sample_key is not null do nothing
      returning id into v_pid;
    if v_pid is null then
      v_present := v_present + 1;      -- a concurrent load won the race
      continue;
    end if;
    insert into variants (org_id, product_id, name, sku, price_minor, reorder_level)
      values (p_org, v_pid, 'Default', i.sku, i.price_minor, i.reorder_level)
      returning id into v_vid;
    insert into variant_costs (variant_id, org_id, cost_minor) values (v_vid, p_org, i.cost_minor);
    if i.opening_qty > 0 then
      insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id, is_sample)
        values (p_org, v_vid, i.opening_qty, 'opening', v_user, true);
    end if;
    v_loaded := v_loaded + 1;
  end loop;

  if v_loaded > 0 then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (p_org, v_user, 'catalogue.sample_load', 'organization', p_org::text,
              jsonb_build_object('loaded', v_loaded, 'business_type', v_type));
  end if;
  return jsonb_build_object('loaded', v_loaded, 'alreadyPresent', v_present, 'skippedSkuTaken', v_skipped);
end $$;

-- Deletes ONLY flagged sample products that have no REAL stock history (no movement other than the loader's own opening
-- movement). A sample that was ever sold, received, adjusted, etc. is kept and reported. Real products are never touched.
create function app.delete_sample_catalogue(p_org uuid) returns jsonb
language plpgsql security definer set search_path = public, pg_temp as $$
declare
  v_user uuid := app.current_user_id();
  r record;
  v_deleted integer := 0;
  v_kept jsonb := '[]'::jsonb;
begin
  if v_user is null or p_org is distinct from app.current_org_id() or not app.can_manage_inventory() then
    raise exception 'not allowed' using errcode = '42501';
  end if;
  perform set_config('app.sample_purge', 'on', true);
  for r in select p.id, p.name from products p where p.org_id = p_org and p.sample_key is not null order by p.name, p.id loop
    if exists (
      select 1 from variants v join stock_movements m on m.org_id = v.org_id and m.variant_id = v.id
       where v.org_id = p_org and v.product_id = r.id and m.is_sample = false
    ) then
      v_kept := v_kept || jsonb_build_array(jsonb_build_object('id', r.id, 'name', r.name));
      continue;
    end if;
    delete from stock_movements m using variants v
     where m.org_id = p_org and v.org_id = p_org and m.variant_id = v.id and v.product_id = r.id and m.is_sample;
    delete from stock_levels sl using variants v
     where sl.org_id = p_org and v.org_id = p_org and sl.variant_id = v.id and v.product_id = r.id;
    delete from variants v where v.org_id = p_org and v.product_id = r.id;   -- variant_costs cascade
    delete from products p where p.org_id = p_org and p.id = r.id;
    v_deleted := v_deleted + 1;
  end loop;
  perform set_config('app.sample_purge', 'off', true);

  if v_deleted > 0 then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (p_org, v_user, 'catalogue.sample_delete', 'organization', p_org::text,
              jsonb_build_object('deleted', v_deleted, 'kept', jsonb_array_length(v_kept)));
  end if;
  return jsonb_build_object('deleted', v_deleted, 'kept', v_kept);
end $$;

-- Counts only (no costs): any active member may read this, so the UI can decide which buttons to show.
create function app.sample_catalogue_status(p_org uuid) returns jsonb
language plpgsql stable security definer set search_path = public, pg_temp as $$
declare
  v_type text;
  v_loadable integer;
  v_present integer;
  v_deletable integer;
begin
  if app.current_user_id() is null or p_org is distinct from app.current_org_id() then
    raise exception 'not allowed' using errcode = '42501';
  end if;
  select o.business_type into v_type from organizations o where o.id = p_org;
  select count(*) into v_loadable from sample_catalogue_items i
   where i.business_type = v_type
     and not exists (select 1 from products p where p.org_id = p_org and p.sample_key = i.sample_key);
  select count(*) into v_present from products p where p.org_id = p_org and p.sample_key is not null;
  select count(*) into v_deletable from products p
   where p.org_id = p_org and p.sample_key is not null
     and not exists (
       select 1 from variants v join stock_movements m on m.org_id = v.org_id and m.variant_id = v.id
        where v.org_id = p_org and v.product_id = p.id and m.is_sample = false);
  return jsonb_build_object('loadable', v_loadable, 'present', v_present, 'deletable', v_deletable, 'keptWithHistory', v_present - v_deletable);
end $$;

revoke all on function app.load_sample_catalogue(uuid), app.delete_sample_catalogue(uuid), app.sample_catalogue_status(uuid) from public;
grant execute on function app.load_sample_catalogue(uuid), app.delete_sample_catalogue(uuid), app.sample_catalogue_status(uuid) to permtpa_app;
