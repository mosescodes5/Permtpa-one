"use client";
import { useCallback, useEffect, useState } from "react";
import { formatQty } from "@/lib/quantity";
import { AlertTriangle, Archive, Box, ChevronLeft, ChevronRight, History, Pencil, Plus, Search, Wallet } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Banner } from "@/components/auth/forms";
import { useAuth } from "@/components/auth/auth-provider";
import { CategoriesModal, HistoryModal, ProductForm, StockModal, VariantEditModal } from "@/components/app/inventory-modals";
import { canManageInventory, canViewCosts, api, problemMessage, type Category, type LowStockItem, type Product, type SampleDeleteResult, type SampleLoadResult, type SampleStatus, type Variant } from "@/lib/api";
import { formatMoney } from "@/lib/money";

type Page = { items: Product[]; total: number; limit: number; offset: number };
type Target = { product: Product; variant: Variant } | null;
const selectCls = "h-10 cursor-pointer rounded-lg border border-border-strong bg-surface px-3 text-sm shadow-sm focus-visible:border-accent focus-visible:outline-none";

function useDebounced<T>(value: T, ms: number) {
  const [v, setV] = useState(value);
  useEffect(() => { const t = setTimeout(() => setV(value), ms); return () => clearTimeout(t); }, [value, ms]);
  return v;
}

function StockBadge({ v }: { v: Variant }) {
  if (v.onHand === 0) return <Badge tone="danger">Out of stock</Badge>;
  if (v.reorderLevel > 0 && v.onHand <= v.reorderLevel) return <Badge tone="warning"><AlertTriangle className="size-3" />Low · {formatQty(v.onHand)}</Badge>;
  return <span className="tabular font-medium">{formatQty(v.onHand)}</span>;
}

export function Inventory() {
  const { activeOrg } = useAuth();
  const orgId = activeOrg?.orgId; const role = activeOrg?.role ?? "";
  const manage = canManageInventory(role); const seeCosts = canViewCosts(role);
  const [search, setSearch] = useState(""); const dsearch = useDebounced(search, 300);
  const [category, setCategory] = useState(""); const [status, setStatus] = useState<"active" | "archived">("active"); const [offset, setOffset] = useState(0);
  const [page, setPage] = useState<Page | null>(null); const [cats, setCats] = useState<Category[]>([]); const [low, setLow] = useState<LowStockItem[]>([]); const [value, setValue] = useState<{ totalMinor: string; unitsThousandths: number; variantsMissingCost: number } | null>(null);
  const [error, setError] = useState<string | null>(null); const [notice, setNotice] = useState<string | null>(null); const [reload, setReload] = useState(0);
  const [samples, setSamples] = useState<SampleStatus | null>(null); const [confirmSamples, setConfirmSamples] = useState(false); const [samplesBusy, setSamplesBusy] = useState(false);
  const [addOpen, setAddOpen] = useState(false); const [catsOpen, setCatsOpen] = useState(false);
  const [stock, setStock] = useState<Target>(null); const [hist, setHist] = useState<Target>(null); const [edit, setEdit] = useState<Target>(null);
  const bump = useCallback(() => setReload((n) => n + 1), []);

  useEffect(() => {
    let alive = true;
    if (!orgId) return;
    const qs = new URLSearchParams({ status, limit: "20", offset: String(offset) });
    if (dsearch.trim()) qs.set("search", dsearch.trim());
    if (category) qs.set("category", category);
    api<Page>(`/v1/orgs/${orgId}/products?${qs}`).then((p) => { if (alive) { setPage(p); setError(null); } }).catch((e) => { if (alive) setError(problemMessage(e)); });
    return () => { alive = false; };
  }, [orgId, dsearch, category, status, offset, reload]);

  useEffect(() => {
    let alive = true;
    if (!orgId) return;
    api<{ categories: Category[] }>(`/v1/orgs/${orgId}/categories`).then((r) => { if (alive) setCats(r.categories); }).catch(() => {});
    api<{ items: LowStockItem[] }>(`/v1/orgs/${orgId}/inventory/low-stock`).then((r) => { if (alive) setLow(r.items); }).catch(() => {});
    api<SampleStatus>(`/v1/orgs/${orgId}/sample-catalogue`).then((r) => { if (alive) setSamples(r); }).catch(() => {});
    if (seeCosts) api<{ totalMinor: string; unitsThousandths: number; variantsMissingCost: number }>(`/v1/orgs/${orgId}/inventory/valuation`).then((r) => { if (alive) setValue(r); }).catch(() => {});
    return () => { alive = false; };
  }, [orgId, seeCosts, reload]);

  async function loadSamples() {
    setSamplesBusy(true); setError(null); setNotice(null);
    try { const r = await api<SampleLoadResult>(`/v1/orgs/${orgId}/sample-catalogue`, { method: "POST" }); setNotice(r.loaded === 1 ? "1 sample product was added." : `${r.loaded} sample products were added.`); bump(); }
    catch (e) { setError(problemMessage(e)); } finally { setSamplesBusy(false); }
  }
  async function deleteSamples() {
    setSamplesBusy(true); setError(null); setNotice(null);
    try {
      const r = await api<SampleDeleteResult>(`/v1/orgs/${orgId}/sample-catalogue`, { method: "DELETE" });
      const kept = r.kept.length > 0 ? ` ${r.kept.length === 1 ? "1 sample was" : `${r.kept.length} samples were`} kept because of real stock history: ${r.kept.map((k) => k.name).join(", ")}.` : "";
      setNotice(`${r.deleted === 1 ? "1 sample product was" : `${r.deleted} sample products were`} deleted.${kept}`); setConfirmSamples(false); bump();
    } catch (e) { setError(problemMessage(e)); } finally { setSamplesBusy(false); }
  }
  async function setArchived(p: Product, archived: boolean) {
    try { await api(`/v1/orgs/${orgId}/products/${p.id}`, { method: "PATCH", body: { status: archived ? "archived" : "active" } }); setNotice(`${p.name} was ${archived ? "archived" : "restored"}.`); bump(); } catch (e) { setError(problemMessage(e)); }
  }
  const saved = (m: string) => { setNotice(m); setError(null); bump(); };
  const from = page && page.total > 0 ? page.offset + 1 : 0; const to = page ? Math.min(page.offset + page.limit, page.total) : 0;

  return (
    <div className="space-y-6">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
        <div><h1 className="font-display text-4xl">Inventory</h1><p className="mt-1 text-sm text-muted">Products, prices and stock for {activeOrg?.name ?? "your business"}.</p></div>
        {manage && <div className="flex gap-2"><Button variant="secondary" onClick={() => setCatsOpen(true)}>Categories</Button><Button variant="navy" onClick={() => setAddOpen(true)}><Plus className="size-4" />Add product</Button></div>}
      </div>
      {error && <Banner tone="error">{error}</Banner>}
      {notice && <Banner tone="success">{notice}</Banner>}
      {manage && samples && samples.present > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-bg-subtle p-3.5 text-sm">
          {confirmSamples
            ? <p className="text-fg">Delete {samples.deletable} sample product{samples.deletable === 1 ? "" : "s"}? Your own products are never touched. {samples.keptWithHistory > 0 ? `${samples.keptWithHistory} with real stock history will be kept.` : ""}</p>
            : <p className="text-muted">{samples.present} sample product{samples.present === 1 ? " is" : "s are"} in your catalogue. They are marked “Sample”.</p>}
          {confirmSamples
            ? <div className="flex gap-2"><Button variant="secondary" size="sm" disabled={samplesBusy} onClick={() => setConfirmSamples(false)}>Cancel</Button><Button variant="navy" size="sm" disabled={samplesBusy} onClick={() => void deleteSamples()}>Yes, delete samples</Button></div>
            : <Button variant="secondary" size="sm" onClick={() => setConfirmSamples(true)}>Delete sample products</Button>}
        </div>
      )}
      {manage && samples && samples.loadable > 0 && samples.present === 0 && page && page.total === 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-bg-subtle p-3.5 text-sm">
          <p className="text-muted">Add a few example products so you can test a sale immediately. You can delete them later.</p>
          <Button variant="secondary" size="sm" disabled={samplesBusy} onClick={() => void loadSamples()}>Load sample catalogue</Button>
        </div>
      )}

      <section aria-label="Summary" className="grid gap-4 sm:grid-cols-3">
        <Card data-hover-scope className="lift rise"><CardContent><p className="flex items-center gap-2 text-sm text-muted"><Box className="size-4 text-accent" />Products</p><p className="tabular font-display mt-2 text-[2rem] leading-none" data-testid="kpi-products">{page?.total ?? "—"}</p><p className="mt-3 text-xs text-subtle">{status === "active" ? "active" : "archived"}</p></CardContent></Card>
        <Card data-hover-scope className="lift rise" style={{ "--i": 1 } as React.CSSProperties}><CardContent><p className="flex items-center gap-2 text-sm text-muted"><AlertTriangle className="size-4 text-warning" />Needs restocking</p><p className="tabular font-display mt-2 text-[2rem] leading-none" data-testid="kpi-low">{low.length}</p><p className="mt-3 text-xs text-subtle">at or below their reorder level</p></CardContent></Card>
        {seeCosts && <Card data-hover-scope className="lift rise" style={{ "--i": 2 } as React.CSSProperties}><CardContent><p className="flex items-center gap-2 text-sm text-muted"><Wallet className="size-4 text-accent" />Stock value</p><p className="tabular font-display mt-2 text-[2rem] leading-none" data-testid="kpi-value">{value ? formatMoney(BigInt(value.totalMinor), "NGN", { compact: true }) : "—"}</p><p className="mt-3 text-xs text-subtle">{value ? `${formatQty(value.unitsThousandths)} units at cost${value.variantsMissingCost ? ` · ${value.variantsMissingCost} without a cost` : ""}` : ""}</p></CardContent></Card>}
      </section>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="relative flex-1 sm:max-w-sm"><Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-subtle" /><Input aria-label="Search products" placeholder="Search name, SKU or barcode" className="pl-9" value={search} onChange={(e) => { setSearch(e.target.value); setOffset(0); }} /></div>
        <select aria-label="Filter by category" className={selectCls} value={category} onChange={(e) => { setCategory(e.target.value); setOffset(0); }}><option value="">All categories</option>{cats.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}</select>
        <div role="group" aria-label="Status" className="flex w-fit rounded-lg border border-border-strong bg-surface p-0.5 shadow-sm">
          {(["active", "archived"] as const).map((s) => <button key={s} type="button" aria-pressed={status === s} onClick={() => { setStatus(s); setOffset(0); }} className={`cursor-pointer rounded-md px-3 py-1.5 text-sm font-medium capitalize transition-colors ${status === s ? "bg-navy text-[#f4f1ea] dark:bg-[#f4f1ea] dark:text-[#0b1426]" : "text-muted hover:text-fg"}`}>{s}</button>)}
        </div>
      </div>

      <Card>
        {!page && !error && <CardContent><div className="skeleton h-40 w-full" /></CardContent>}
        {page && page.items.length === 0 && (
          <CardContent className="flex flex-col items-center py-16 text-center">
            <span className="flex size-12 items-center justify-center rounded-xl bg-accent-soft text-accent"><Box className="size-6" /></span>
            <h2 className="mt-4 text-lg font-semibold">{dsearch || category ? "Nothing matches your search" : status === "archived" ? "No archived products" : "No products yet"}</h2>
            <p className="mt-1 max-w-sm text-sm text-muted">{manage && !dsearch && !category && status === "active" ? "Add your first product to start tracking stock." : "Try a different search or filter."}</p>
            {manage && !dsearch && !category && status === "active" && <Button variant="navy" className="mt-5" onClick={() => setAddOpen(true)}><Plus className="size-4" />Add product</Button>}
          </CardContent>)}
        {page && page.items.length > 0 && (
          <div className="relative overflow-x-auto">
            <table className="w-full text-sm"><caption className="sr-only">Products and stock levels</caption>
              <thead><tr className="border-b border-border text-left text-xs text-subtle"><th className="px-5 py-3 font-medium">Item</th><th className="px-3 py-3 font-medium">SKU</th><th className="px-3 py-3 text-right font-medium">Price</th>{seeCosts && <th className="px-3 py-3 text-right font-medium">Cost</th>}<th className="px-3 py-3 text-right font-medium">On hand</th><th className="px-5 py-3 text-right font-medium"><span className="sr-only">Actions</span></th></tr></thead>
              <tbody>{page.items.map((p) => p.variants.map((v, i) => (
                <tr key={v.id} data-testid="variant-row" className="border-b border-border transition-colors last:border-0 hover:bg-bg-subtle">
                  <td className="px-5 py-3.5">{i === 0 ? <><p className="font-medium">{p.name}{p.isSample && <Badge className="ml-2">Sample</Badge>}</p><p className="text-xs text-subtle">{p.category ?? "No category"} · {p.unit}</p></> : <span className="text-xs text-subtle">↳</span>}{v.name !== "Default" && <p className="text-xs text-muted">{v.name}</p>}</td>
                  <td className="px-3 py-3.5 font-mono text-xs text-muted">{v.sku}</td>
                  <td className="tabular px-3 py-3.5 text-right">{formatMoney(v.priceMinor)}</td>
                  {seeCosts && <td className="tabular px-3 py-3.5 text-right text-muted">{v.costMinor != null ? formatMoney(v.costMinor) : "—"}</td>}
                  <td className="px-3 py-3.5 text-right"><StockBadge v={v} /></td>
                  <td className="whitespace-nowrap px-5 py-3.5 text-right">
                    <Button variant="ghost" size="icon" aria-label={`History for ${p.name} ${v.sku}`} onClick={() => setHist({ product: p, variant: v })}><History className="size-4" /></Button>
                    {manage && <>
                      <Button variant="ghost" size="icon" aria-label={`Edit ${p.name} ${v.sku}`} onClick={() => setEdit({ product: p, variant: v })}><Pencil className="size-4" /></Button>
                      <Button variant="secondary" size="sm" aria-label={`Adjust stock for ${p.name} ${v.sku}`} onClick={() => setStock({ product: p, variant: v })}>Adjust stock</Button>
                      {i === 0 && <Button variant="ghost" size="icon" aria-label={`${p.status === "active" ? "Archive" : "Restore"} ${p.name}`} onClick={() => void setArchived(p, p.status === "active")}><Archive className="size-4" /></Button>}
                    </>}
                  </td>
                </tr>)))}</tbody>
            </table>
          </div>)}
        {page && page.total > 0 && (
          <div className="flex items-center justify-between border-t border-border px-5 py-3 text-xs text-muted">
            <span data-testid="range">Showing {from}–{to} of {page.total}</span>
            <div className="flex gap-1"><Button variant="ghost" size="icon" aria-label="Previous page" disabled={page.offset === 0} onClick={() => setOffset(Math.max(0, page.offset - page.limit))}><ChevronLeft className="size-4" /></Button><Button variant="ghost" size="icon" aria-label="Next page" disabled={to >= page.total} onClick={() => setOffset(page.offset + page.limit)}><ChevronRight className="size-4" /></Button></div>
          </div>)}
      </Card>
      {!manage && <p className="text-xs text-subtle">Your role can view the catalogue and stock levels. Only owners, administrators, managers and inventory managers can change them.</p>}

      {orgId && manage && <ProductForm open={addOpen} onClose={() => setAddOpen(false)} orgId={orgId} categories={cats} onSaved={saved} />}
      {orgId && manage && <CategoriesModal open={catsOpen} onClose={() => setCatsOpen(false)} orgId={orgId} categories={cats} onChanged={bump} />}
      {orgId && <StockModal target={stock} orgId={orgId} onClose={() => setStock(null)} onSaved={saved} />}
      {orgId && <HistoryModal target={hist} orgId={orgId} onClose={() => setHist(null)} />}
      {orgId && <VariantEditModal target={edit} orgId={orgId} seeCosts={seeCosts} onClose={() => setEdit(null)} onSaved={saved} />}
    </div>
  );
}
