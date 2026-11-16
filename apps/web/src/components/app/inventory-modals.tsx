"use client";
import { useEffect, useState } from "react";
import { Archive, Loader2, Plus, X } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { FieldError, Input, Label } from "@/components/ui/input";
import { Modal } from "@/components/ui/modal";
import { Banner } from "@/components/auth/forms";
import { REASON_LABELS, api, problemMessage, type Category, type Movement, type Product, type Variant } from "@/lib/api";
import { formatMoney, minorToInput, parseNairaToMinor } from "@/lib/money";

const selectCls = "h-11 w-full cursor-pointer rounded-lg border border-border-strong bg-surface px-3 text-sm shadow-sm focus-visible:border-accent focus-visible:outline-none";
const SKU_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,39}$/;
const BARCODE_RE = /^[0-9A-Za-z-]{4,32}$/;
const intOrNull = (s: string) => (/^\d{1,9}$/.test(s.trim()) ? Number(s.trim()) : null);

// ---------------------------------------------------------------- add product
type Row = { name: string; sku: string; barcode: string; price: string; cost: string; reorder: string; opening: string };
const emptyRow = (): Row => ({ name: "", sku: "", barcode: "", price: "", cost: "", reorder: "0", opening: "0" });

export function ProductForm({ open, onClose, orgId, categories, onSaved }: { open: boolean; onClose: () => void; orgId: string; categories: Category[]; onSaved: (msg: string) => void }) {
  const [name, setName] = useState(""); const [unit, setUnit] = useState("pcs"); const [category, setCategory] = useState(""); const [description, setDescription] = useState("");
  const [rows, setRows] = useState<Row[]>([emptyRow()]);
  const [errors, setErrors] = useState<string[]>([]); const [busy, setBusy] = useState(false); const [apiError, setApiError] = useState<string | null>(null);
  const set = (i: number, k: keyof Row, v: string) => setRows((r) => r.map((x, j) => (j === i ? { ...x, [k]: v } : x)));

  async function submit(e: React.FormEvent) {
    e.preventDefault(); setApiError(null);
    const errs: string[] = [];
    if (name.trim().length < 1) errs.push("Enter a product name.");
    const variants = rows.map((r, i) => {
      const n = i + 1; const price = parseNairaToMinor(r.price); const cost = r.cost.trim() ? parseNairaToMinor(r.cost) : undefined;
      const reorder = intOrNull(r.reorder); const opening = intOrNull(r.opening);
      if (!SKU_RE.test(r.sku.trim())) errs.push(`Variant ${n}: SKU needs letters/digits (and . _ -), up to 40 characters.`);
      if (price === null) errs.push(`Variant ${n}: enter a valid selling price (e.g. 1,600.50).`);
      if (cost === null) errs.push(`Variant ${n}: cost price is not a valid amount.`);
      if (r.barcode.trim() && !BARCODE_RE.test(r.barcode.trim())) errs.push(`Variant ${n}: barcode must be 4–32 letters, digits or "-".`);
      if (reorder === null || opening === null) errs.push(`Variant ${n}: reorder level and opening stock must be whole numbers.`);
      return { ...(r.name.trim() ? { name: r.name.trim() } : {}), sku: r.sku.trim(), ...(r.barcode.trim() ? { barcode: r.barcode.trim() } : {}), priceMinor: price ?? 0, ...(cost != null ? { costMinor: cost } : {}), reorderLevel: reorder ?? 0, openingStock: opening ?? 0 };
    });
    setErrors(errs); if (errs.length) return;
    setBusy(true);
    try {
      await api(`/v1/orgs/${orgId}/products`, { method: "POST", body: { name: name.trim(), unit: unit.trim() || "pcs", ...(description.trim() ? { description: description.trim() } : {}), ...(category ? { categoryId: category } : {}), variants } });
      setName(""); setRows([emptyRow()]); setDescription(""); setCategory(""); onSaved(`${name.trim()} was added.`); onClose();
    } catch (x) { setApiError(problemMessage(x)); } finally { setBusy(false); }
  }

  return (
    <Modal open={open} onClose={onClose} title="Add product" wide>
      <form noValidate onSubmit={submit} className="space-y-5">
        {apiError && <Banner tone="error">{apiError}</Banner>}
        {errors.length > 0 && <Banner tone="error"><ul className="list-disc pl-4">{errors.map((x) => <li key={x}>{x}</li>)}</ul></Banner>}
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="sm:col-span-2"><Label htmlFor="p-name">Product name</Label><Input id="p-name" value={name} onChange={(e) => setName(e.target.value)} /></div>
          <div><Label htmlFor="p-cat">Category</Label><select id="p-cat" className={selectCls} value={category} onChange={(e) => setCategory(e.target.value)}><option value="">None</option>{categories.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}</select></div>
          <div><Label htmlFor="p-unit">Unit</Label><Input id="p-unit" value={unit} onChange={(e) => setUnit(e.target.value)} placeholder="pcs, bag, carton…" /></div>
          <div className="sm:col-span-2"><Label htmlFor="p-desc">Description (optional)</Label><Input id="p-desc" value={description} onChange={(e) => setDescription(e.target.value)} /></div>
        </div>
        <div className="space-y-3">
          <p className="text-sm font-medium">Variants <span className="font-normal text-subtle">— sizes, colours or pack types. Most products need just one.</span></p>
          {rows.map((r, i) => (
            <fieldset key={i} className="rounded-xl border border-border bg-bg-subtle/50 p-4">
              <legend className="px-1 text-xs font-medium text-muted">Variant {i + 1}</legend>
              <div className="grid gap-3 sm:grid-cols-3">
                <div><Label htmlFor={`v-name-${i}`}>Name</Label><Input id={`v-name-${i}`} placeholder="Default" value={r.name} onChange={(e) => set(i, "name", e.target.value)} /></div>
                <div><Label htmlFor={`v-sku-${i}`}>SKU</Label><Input id={`v-sku-${i}`} value={r.sku} onChange={(e) => set(i, "sku", e.target.value)} /></div>
                <div><Label htmlFor={`v-bar-${i}`}>Barcode</Label><Input id={`v-bar-${i}`} value={r.barcode} onChange={(e) => set(i, "barcode", e.target.value)} /></div>
                <div><Label htmlFor={`v-price-${i}`}>Selling price (₦)</Label><Input id={`v-price-${i}`} inputMode="decimal" value={r.price} onChange={(e) => set(i, "price", e.target.value)} /></div>
                <div><Label htmlFor={`v-cost-${i}`}>Cost price (₦)</Label><Input id={`v-cost-${i}`} inputMode="decimal" value={r.cost} onChange={(e) => set(i, "cost", e.target.value)} /></div>
                <div className="grid grid-cols-2 gap-3">
                  <div><Label htmlFor={`v-re-${i}`}>Reorder at</Label><Input id={`v-re-${i}`} inputMode="numeric" value={r.reorder} onChange={(e) => set(i, "reorder", e.target.value)} /></div>
                  <div><Label htmlFor={`v-op-${i}`}>In stock</Label><Input id={`v-op-${i}`} inputMode="numeric" value={r.opening} onChange={(e) => set(i, "opening", e.target.value)} /></div>
                </div>
              </div>
              {rows.length > 1 && <Button type="button" variant="ghost" size="sm" className="mt-3" onClick={() => setRows((x) => x.filter((_, j) => j !== i))}><X className="size-4" />Remove variant</Button>}
            </fieldset>
          ))}
          {rows.length < 50 && <Button type="button" variant="secondary" size="sm" onClick={() => setRows((r) => [...r, emptyRow()])}><Plus className="size-4" />Add another variant</Button>}
        </div>
        <div className="flex justify-end gap-2 border-t border-border pt-4"><Button type="button" variant="ghost" onClick={onClose}>Cancel</Button><Button type="submit" variant="navy" disabled={busy}>{busy && <Loader2 className="animate-spin" />}Save product</Button></div>
      </form>
    </Modal>
  );
}

// ---------------------------------------------------------------- adjust stock
const REASONS = ["purchase", "return_in", "adjustment", "damage", "correction"] as const;
export function StockModal({ target, orgId, onClose, onSaved }: { target: { product: Product; variant: Variant } | null; orgId: string; onClose: () => void; onSaved: (msg: string) => void }) {
  return (
    <Modal open={!!target} onClose={onClose} title="Adjust stock">
      {target && <StockForm key={target.variant.id} target={target} orgId={orgId} onClose={onClose} onSaved={onSaved} />}
    </Modal>
  );
}

function StockForm({ target, orgId, onClose, onSaved }: { target: { product: Product; variant: Variant }; orgId: string; onClose: () => void; onSaved: (msg: string) => void }) {
  const [reason, setReason] = useState<(typeof REASONS)[number]>("purchase"); const [direction, setDirection] = useState<"add" | "remove">("add");
  const [quantity, setQuantity] = useState(""); const [note, setNote] = useState(""); const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null);
  const v = target.variant;
  const signed = reason === "purchase" || reason === "return_in" ? 1 : reason === "damage" ? -1 : direction === "add" ? 1 : -1;
  const qty = intOrNull(quantity); const delta = qty && qty > 0 ? signed * qty : null;
  const noteRequired = reason === "adjustment" || reason === "damage" || reason === "correction";
  const after = v && delta !== null ? v.onHand + delta : null;

  async function submit(e: React.FormEvent) {
    e.preventDefault(); setError(null);
    if (delta === null) { setError("Enter a whole-number quantity greater than zero."); return; }
    if (noteRequired && !note.trim()) { setError("Please explain why you are changing the stock."); return; }
    if (after !== null && after < 0) { setError("That would take stock below zero."); return; }
    setBusy(true);
    try { await api(`/v1/orgs/${orgId}/variants/${v.id}/stock`, { method: "POST", body: { delta, reason, ...(note.trim() ? { note: note.trim() } : {}) } }); setQuantity(""); setNote(""); onSaved(`${target.product.name} stock is now ${after}.`); onClose(); }
    catch (x) { setError(problemMessage(x)); } finally { setBusy(false); }
  }
  return (
    <>
      {(
        <form noValidate onSubmit={submit} className="space-y-4">
          <div className="rounded-lg bg-bg-subtle p-3 text-sm"><p className="font-medium">{target.product.name}{v.name !== "Default" && ` · ${v.name}`}</p><p className="text-xs text-subtle">{v.sku} · currently <strong className="text-fg">{v.onHand}</strong> {target.product.unit}</p></div>
          {error && <Banner tone="error">{error}</Banner>}
          <div><Label htmlFor="s-reason">Reason</Label><select id="s-reason" className={selectCls} value={reason} onChange={(e) => setReason(e.target.value as (typeof REASONS)[number])}>{REASONS.map((r) => <option key={r} value={r}>{REASON_LABELS[r]}</option>)}</select></div>
          {(reason === "adjustment" || reason === "correction") && (
            <div role="radiogroup" aria-label="Direction" className="flex gap-2">{(["add", "remove"] as const).map((d) => <button key={d} type="button" role="radio" aria-checked={direction === d} onClick={() => setDirection(d)} className={`flex-1 cursor-pointer rounded-lg border px-3 py-2 text-sm font-medium transition-colors ${direction === d ? "border-accent bg-accent-soft text-accent" : "border-border-strong text-muted hover:bg-bg-subtle"}`}>{d === "add" ? "Add stock" : "Remove stock"}</button>)}</div>
          )}
          <div><Label htmlFor="s-qty">Quantity</Label><Input id="s-qty" inputMode="numeric" value={quantity} onChange={(e) => setQuantity(e.target.value)} />{after !== null && <p className="mt-1.5 text-xs text-muted">{v.onHand} → <strong className={after < 0 ? "text-danger" : "text-fg"}>{after}</strong></p>}</div>
          <div><Label htmlFor="s-note">Note{noteRequired ? "" : " (optional)"}</Label><Input id="s-note" maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} placeholder={noteRequired ? "Why is this changing?" : "e.g. Supplier delivery"} /></div>
          <div className="flex justify-end gap-2 border-t border-border pt-4"><Button type="button" variant="ghost" onClick={onClose}>Cancel</Button><Button type="submit" variant="navy" disabled={busy}>{busy && <Loader2 className="animate-spin" />}Update stock</Button></div>
        </form>
      )}
    </>
  );
}

// ---------------------------------------------------------------- history
export function HistoryModal({ target, orgId, onClose }: { target: { product: Product; variant: Variant } | null; orgId: string; onClose: () => void }) {
  const [items, setItems] = useState<Movement[] | null>(null); const [next, setNext] = useState<number | null>(null); const [error, setError] = useState<string | null>(null);
  const vid = target?.variant.id;
  useEffect(() => {
    let alive = true;
    if (!vid) return;
    api<{ movements: Movement[]; nextBefore: number | null }>(`/v1/orgs/${orgId}/variants/${vid}/movements?limit=20`).then((r) => { if (alive) { setItems(r.movements); setNext(r.nextBefore); } }).catch((e) => { if (alive) setError(problemMessage(e)); });
    return () => { alive = false; setItems(null); setNext(null); setError(null); };
  }, [orgId, vid]);
  async function more() {
    if (!vid || next === null) return;
    try { const r = await api<{ movements: Movement[]; nextBefore: number | null }>(`/v1/orgs/${orgId}/variants/${vid}/movements?limit=20&before=${next}`); setItems((x) => [...(x ?? []), ...r.movements]); setNext(r.nextBefore); } catch (e) { setError(problemMessage(e)); }
  }
  return (
    <Modal open={!!target} onClose={onClose} title={target ? `History · ${target.product.name}` : "History"} wide>
      {error && <Banner tone="error">{error}</Banner>}
      {!items && !error && <div className="skeleton h-24 w-full" />}
      {items && items.length === 0 && <p className="text-sm text-muted">No movements yet.</p>}
      {items && items.length > 0 && (
        <div className="relative overflow-x-auto"><table className="w-full text-sm"><caption className="sr-only">Stock movement history</caption>
          <thead><tr className="text-left text-xs text-subtle"><th className="pb-2 font-medium">When</th><th className="pb-2 font-medium">Reason</th><th className="pb-2 text-right font-medium">Change</th><th className="pb-2 text-right font-medium">Balance</th><th className="pb-2 pl-4 font-medium">Note</th><th className="pb-2 font-medium">By</th></tr></thead>
          <tbody className="divide-y divide-border">{items.map((m) => (
            <tr key={m.id} data-testid="movement-row" className="transition-colors hover:bg-bg-subtle">
              <td className="whitespace-nowrap py-2.5 text-xs text-muted">{new Date(m.createdAt).toLocaleString()}</td><td className="py-2.5">{REASON_LABELS[m.reason] ?? m.reason}</td>
              <td className={`tabular py-2.5 text-right font-medium ${m.delta > 0 ? "text-accent" : "text-danger"}`}>{m.delta > 0 ? `+${m.delta}` : m.delta}</td><td className="tabular py-2.5 text-right">{m.resultingOnHand}</td>
              <td className="py-2.5 pl-4 text-muted">{m.note ?? "—"}</td><td className="py-2.5 text-muted">{m.actor.name ?? "—"}</td></tr>))}</tbody></table></div>)}
      {next !== null && <Button variant="secondary" size="sm" className="mt-4" onClick={more}>Load older movements</Button>}
    </Modal>
  );
}

// ---------------------------------------------------------------- edit variant
export function VariantEditModal({ target, orgId, seeCosts, onClose, onSaved }: { target: { product: Product; variant: Variant } | null; orgId: string; seeCosts: boolean; onClose: () => void; onSaved: (msg: string) => void }) {
  return (
    <Modal open={!!target} onClose={onClose} title="Edit prices">
      {target && <EditForm key={target.variant.id} target={target} orgId={orgId} seeCosts={seeCosts} onClose={onClose} onSaved={onSaved} />}
    </Modal>
  );
}

/** Keyed by variant id, so its state is initialised from the variant each time the dialog opens (no effect-driven copying). */
function EditForm({ target, orgId, seeCosts, onClose, onSaved }: { target: { product: Product; variant: Variant }; orgId: string; seeCosts: boolean; onClose: () => void; onSaved: (msg: string) => void }) {
  const v = target.variant;
  const [price, setPrice] = useState(() => minorToInput(v.priceMinor)); const [cost, setCost] = useState(() => (v.costMinor != null ? minorToInput(v.costMinor) : ""));
  const [reorder, setReorder] = useState(() => String(v.reorderLevel)); const [barcode, setBarcode] = useState(() => v.barcode ?? "");
  const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null);
  async function submit(e: React.FormEvent) {
    e.preventDefault(); setError(null);
    const p = parseNairaToMinor(price); const c = cost.trim() ? parseNairaToMinor(cost) : undefined; const r = intOrNull(reorder);
    if (p === null) return setError("Enter a valid selling price.");
    if (c === null) return setError("Cost price is not a valid amount.");
    if (r === null) return setError("Reorder level must be a whole number.");
    if (barcode.trim() && !BARCODE_RE.test(barcode.trim())) return setError("Barcode must be 4–32 letters, digits or \"-\".");
    setBusy(true);
    try {
      await api(`/v1/orgs/${orgId}/variants/${v.id}`, { method: "PATCH", body: { priceMinor: p, ...(seeCosts && c !== undefined ? { costMinor: c } : {}), reorderLevel: r, ...(barcode.trim() ? { barcode: barcode.trim() } : v.barcode ? { clearBarcode: true } : {}) } });
      onSaved(`${target.product.name} was updated.`); onClose();
    } catch (x) { setError(problemMessage(x)); } finally { setBusy(false); }
  }
  return (
    <form noValidate onSubmit={submit} className="space-y-4">
      <p className="text-sm font-medium">{target.product.name}{v.name !== "Default" && ` · ${v.name}`} <span className="font-normal text-subtle">({v.sku})</span></p>
      {error && <Banner tone="error">{error}</Banner>}
      <div className="grid gap-4 sm:grid-cols-2">
        <div><Label htmlFor="e-price">Selling price (₦)</Label><Input id="e-price" inputMode="decimal" value={price} onChange={(e) => setPrice(e.target.value)} /></div>
        {seeCosts && <div><Label htmlFor="e-cost">Cost price (₦)</Label><Input id="e-cost" inputMode="decimal" value={cost} onChange={(e) => setCost(e.target.value)} /></div>}
        <div><Label htmlFor="e-reorder">Reorder at</Label><Input id="e-reorder" inputMode="numeric" value={reorder} onChange={(e) => setReorder(e.target.value)} /></div>
        <div><Label htmlFor="e-barcode">Barcode</Label><Input id="e-barcode" value={barcode} onChange={(e) => setBarcode(e.target.value)} /></div>
      </div>
      <p className="text-xs text-subtle">Price and cost changes are recorded in the audit log. Stock is changed with “Adjust stock”, never here.</p>
      <div className="flex justify-end gap-2 border-t border-border pt-4"><Button type="button" variant="ghost" onClick={onClose}>Cancel</Button><Button type="submit" variant="navy" disabled={busy}>{busy && <Loader2 className="animate-spin" />}Save changes</Button></div>
    </form>
  );
}

// ---------------------------------------------------------------- categories
export function CategoriesModal({ open, onClose, orgId, categories, onChanged }: { open: boolean; onClose: () => void; orgId: string; categories: Category[]; onChanged: () => void }) {
  const [name, setName] = useState(""); const [error, setError] = useState<string | null>(null); const [busy, setBusy] = useState(false);
  async function add(e: React.FormEvent) { e.preventDefault(); if (!name.trim()) return; setBusy(true); setError(null); try { await api(`/v1/orgs/${orgId}/categories`, { method: "POST", body: { name: name.trim() } }); setName(""); onChanged(); } catch (x) { setError(problemMessage(x)); } finally { setBusy(false); } }
  async function del(c: Category) { setError(null); try { await api(`/v1/orgs/${orgId}/categories/${c.id}`, { method: "DELETE" }); onChanged(); } catch (x) { setError(problemMessage(x)); } }
  return (
    <Modal open={open} onClose={onClose} title="Categories">
      {error && <div className="mb-3"><Banner tone="error">{error}</Banner></div>}
      <form noValidate onSubmit={add} className="flex gap-2"><Input aria-label="New category name" placeholder="New category" value={name} onChange={(e) => setName(e.target.value)} /><Button type="submit" variant="navy" disabled={busy || !name.trim()}>Add</Button></form>
      <ul className="mt-4 divide-y divide-border" aria-label="Categories">
        {categories.length === 0 && <li className="py-3 text-sm text-muted">No categories yet.</li>}
        {categories.map((c) => <li key={c.id} className="flex items-center gap-3 py-2.5"><span className="flex-1 text-sm">{c.name}</span><Badge>{c.productCount} {c.productCount === 1 ? "product" : "products"}</Badge><Button variant="ghost" size="sm" aria-label={`Delete ${c.name}`} onClick={() => void del(c)}><X className="size-4" /></Button></li>)}
      </ul>
      <FieldError>{undefined}</FieldError>
    </Modal>
  );
}

export { Archive, formatMoney };
