-- Seeds whole-unit inventory on a database migrated only to 0008. Run as permtpa_app. UNVERIFIED until run.
\set ON_ERROR_STOP on
\set QUIET on
select app.register_user('up@upgrade.test','Up Owner','$argon2id$stub') as u \gset
select set_config('app.user_id', :'u', false), set_config('app.org_id', '', false);
select app.create_organization('Upgrade Org','upgrade-org','retail') as o \gset
select set_config('app.org_id', :'o', false);
insert into products (org_id, name, created_by) values (:'o', 'Old rice', :'u') returning id as p \gset
insert into variants (org_id, product_id, sku, price_minor, reorder_level) values (:'o', :'p', 'OLD-1', 1000, 5) returning id as v \gset
insert into variant_costs (variant_id, org_id, cost_minor) values (:'v', :'o', 600);
insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (:'o', :'v', 10, 'opening', :'u');
insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values (:'o', :'v', -3, 'damage', 'before upgrade', :'u');
