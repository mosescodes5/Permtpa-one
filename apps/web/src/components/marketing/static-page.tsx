export function StaticPage({ title, intro, draft, children }: { title: string; intro: string; draft?: boolean; children?: React.ReactNode }) {
  return (
    <div className="mx-auto max-w-3xl px-5 py-20 sm:px-8">
      <h1 className="font-display text-5xl">{title}</h1>
      <p className="mt-4 text-lg text-muted">{intro}</p>
      {draft && <p role="note" className="mt-6 rounded-lg border border-border-strong bg-warning-soft px-4 py-3 text-sm text-warning">Draft placeholder — this document must be reviewed by qualified legal counsel before launch.</p>}
      <div className="mt-8 space-y-4 text-muted">{children}</div>
    </div>
  );
}
