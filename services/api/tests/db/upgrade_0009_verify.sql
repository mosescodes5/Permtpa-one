-- Run as the database superuser AFTER 0009 is applied on top of seeded 0008 data.
\set ON_ERROR_STOP on
do $$
declare r record;
begin
  select on_hand into r from stock_levels;                       if r.on_hand <> 7000 then raise exception 'FAIL on_hand was not x1000: %', r.on_hand; end if;
  select reorder_level into r from variants;                     if r.reorder_level <> 5000 then raise exception 'FAIL reorder_level was not x1000: %', r.reorder_level; end if;
  if (select array_agg(delta order by id) from stock_movements) <> array[10000, -3000]::bigint[] then raise exception 'FAIL movement deltas not x1000'; end if;
  if (select array_agg(resulting_on_hand order by id) from stock_movements) <> array[10000, 7000]::bigint[] then raise exception 'FAIL resulting balances not x1000'; end if;
  if (select on_hand from stock_levels) <> (select sum(delta) from stock_movements) then raise exception 'FAIL balance <> sum(history) after upgrade'; end if;
  if (select allow_fractional from products) then raise exception 'FAIL existing products must stay whole-unit'; end if;
  -- the pre-upgrade audit row keeps its whole-unit delta and has NO scale marker
  select metadata into r from audit_logs where action = 'stock.adjust';
  if (r.metadata->>'delta')::int <> -3 or r.metadata ? 'qty_scale' then raise exception 'FAIL old audit row was altered: %', r.metadata; end if;
  raise notice 'PASS 0009 upgrade preserved data';
end $$;
