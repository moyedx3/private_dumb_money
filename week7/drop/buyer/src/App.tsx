import QRCode from "qrcode";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CatalogEntry, DropApi } from "./api";
import { HttpDropApi } from "./api";
import { bytesToArrayBuffer } from "./bytes";
import type { MemoForm } from "./memo";
import { onchainMemoBytes } from "./memo";
import { clearPurchase, loadPurchase, savePurchase } from "./persist";
import { DispatchPoller } from "./poller";
import type { UnlockResult } from "./poller";
import { createPurchase, fromRecoveryFile, toRecoveryFile } from "./purchase";
import type { Purchase } from "./purchase";
import { detectKind, mimeFor } from "./render";
import { decryptContent } from "./content";
import { sodiumReady, trySealOpen } from "./seal";
import { buildPaymentUri } from "./zip321";

const POLL_MS = 3000;
const defaultIndexer = import.meta.env.VITE_DROP_INDEXER_URL ?? "http://localhost:8080";

export function App() {
  const [indexerUrl, setIndexerUrl] = useState(defaultIndexer);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [error, setError] = useState("");

  const [memoForm, setMemoForm] = useState<MemoForm>("text");
  const [purchase, setPurchase] = useState<Purchase | null>(null);
  const [paymentUri, setPaymentUri] = useState("");
  const [qr, setQr] = useState("");
  const [polling, setPolling] = useState(false);
  const [unlock, setUnlock] = useState<UnlockResult | null>(null);
  const [remember, setRemember] = useState(true);

  const pollerRef = useRef<DispatchPoller | null>(null);

  // Always the live indexer (mock backend removed).
  const api: DropApi = useMemo(() => new HttpDropApi(indexerUrl), [indexerUrl]);

  const loadCatalog = useCallback(async () => {
    if (!api) return;
    setError("");
    try {
      setCatalog(await api.fetchCatalog());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [api]);

  useEffect(() => {
    void loadCatalog();
  }, [loadCatalog]);

  // Resume an opt-in persisted purchase so a reload / tab close doesn't forfeit it (§8 trap 2).
  useEffect(() => {
    const resumed = loadPurchase();
    if (resumed) {
      setPurchase(resumed);
      setPolling(true);
    }
  }, []);

  // Build the ZIP-321 URI + QR whenever the active purchase or memo form changes.
  useEffect(() => {
    if (!purchase) {
      setPaymentUri("");
      setQr("");
      return;
    }
    let alive = true;
    void (async () => {
      await sodiumReady();
      try {
        const uri = buildPaymentUri({
          depositAddr: purchase.depositAddr,
          priceZec: purchase.priceZec,
          onchainMemo: onchainMemoBytes(memoForm, purchase.dropId, purchase.ePub)
        });
        const dataUrl = await QRCode.toDataURL(uri, { margin: 1, width: 240 });
        if (alive) {
          setPaymentUri(uri);
          setQr(dataUrl);
        }
      } catch (e) {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, [purchase, memoForm]);

  // Poll the bucket for the dispatch blob while a purchase is pending and unopened.
  useEffect(() => {
    if (!polling || !purchase || !api || unlock) return;
    if (!pollerRef.current) pollerRef.current = new DispatchPoller(api);
    let alive = true;
    const tick = async () => {
      try {
        await sodiumReady(); // resumed purchases skip keypair-gen, so ensure WASM is up before trial-open
        const results = await pollerRef.current!.poll([purchase]);
        if (alive && results.length > 0) {
          setUnlock(results[0]);
          setPolling(false);
          clearPurchase();
        }
      } catch (e) {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      }
    };
    void tick();
    const id = setInterval(() => void tick(), POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [polling, purchase, api, unlock]);

  const startPurchase = useCallback(
    async (entry: CatalogEntry) => {
      setError("");
      setUnlock(null);
      pollerRef.current = null;
      try {
        await sodiumReady();
        const p = await createPurchase(entry);
        setPurchase(p);
        setPolling(true);
        if (remember) savePurchase(p);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [remember]
  );

  const reset = useCallback(() => {
    setPurchase(null);
    setUnlock(null);
    setPolling(false);
    pollerRef.current = null;
    clearPurchase();
  }, []);

  const toggleRemember = useCallback(
    (on: boolean) => {
      setRemember(on);
      if (on && purchase) savePurchase(purchase);
      if (!on) clearPurchase();
    },
    [purchase]
  );

  const downloadRecovery = useCallback(() => {
    if (!purchase) return;
    const blob = new Blob([JSON.stringify(toRecoveryFile(purchase), null, 2)], { type: "application/json" });
    triggerDownload(blob, `drop-recovery-${purchase.dropId}-${purchase.id}.json`);
  }, [purchase]);

  return (
    <main className="shell">
      <header className="toolbar">
        <div>
          <p className="eyebrow">Lane B</p>
          <h1>Unlockable Drop — Buyer</h1>
        </div>
        <div className="modes">
          <span className="on">Live indexer</span>
        </div>
      </header>

      <section className="panel">
        <label>
          Indexer URL
          <input value={indexerUrl} onChange={(e) => setIndexerUrl(e.target.value)} />
        </label>
      </section>

      {error ? <p className="error">{error}</p> : null}

      {!purchase ? (
        <section className="panel">
          <div className="panel-head">
            <h2>Catalog</h2>
            <button onClick={() => void loadCatalog()}>Refresh</button>
          </div>
          {catalog.length === 0 ? (
            <p className="note">No drops yet.</p>
          ) : (
            <ul className="drops">
              {catalog.map((d) => (
                <li key={d.drop_id}>
                  <div>
                    <strong>{d.title}</strong>
                    <span className="price">{d.price_zec} ZEC</span>
                  </div>
                  <button className="primary" onClick={() => void startPurchase(d)}>
                    Buy
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      ) : null}

      {purchase && !unlock ? (
        <section className="panel">
          <div className="panel-head">
            <h2>Pay to unlock “{purchase.title}”</h2>
            <button onClick={reset}>Cancel</button>
          </div>

          <div className="pay">
            <div className="qr">{qr ? <img src={qr} alt="ZIP-321 payment QR" /> : <p>building QR…</p>}</div>
            <div className="pay-detail">
              <p>
                Scan with Zashi to pay <strong>{purchase.priceZec} ZEC</strong>. The memo carries your one-time key —
                the indexer answers by publishing a sealed blob only you can open.
              </p>
              <label className="memo-form">
                Memo form
                <select value={memoForm} onChange={(e) => setMemoForm(e.target.value as MemoForm)}>
                  <option value="raw">raw 40B (default)</option>
                  <option value="text">text fallback (A1B64:)</option>
                </select>
              </label>
              <code className="uri">{paymentUri}</code>

              <div className="warn">
                ⚠ Don’t close this tab until it unlocks — the one-time key lives here. Save a recovery file to be safe.
              </div>
              <label className="remember">
                <input type="checkbox" checked={remember} onChange={(e) => toggleRemember(e.target.checked)} /> Keep on this
                device for 24h (survive reload)
              </label>
              <div className="actions">
                <button onClick={downloadRecovery}>Download recovery file</button>
              </div>
              <p className="note">{polling ? "Polling for your dispatch blob…" : "Idle."}</p>
            </div>
          </div>
        </section>
      ) : null}

      {unlock ? <Unlocked result={unlock} onDone={reset} /> : null}

      <ManualUnlock api={api} />

      <footer className="foot">
        Network-layer correlation (your IP polling the bucket + your wallet broadcasting) is a documented,
        out-of-scope limitation — needs Tor/mixnet, not addressed in the demo.
      </footer>
    </main>
  );
}

function Unlocked({ result, onDone }: { result: UnlockResult; onDone: () => void }) {
  const kind = useMemo(() => detectKind(result.content), [result.content]);
  const objectUrl = useMemo(() => {
    if (kind !== "image" && kind !== "video") return "";
    return URL.createObjectURL(new Blob([bytesToArrayBuffer(result.content)], { type: mimeFor(result.content) }));
  }, [kind, result.content]);
  useEffect(() => () => {
    if (objectUrl) URL.revokeObjectURL(objectUrl);
  }, [objectUrl]);

  const text = useMemo(
    () => (kind === "text" ? new TextDecoder().decode(result.content) : ""),
    [kind, result.content]
  );

  const download = useCallback(() => {
    const blob = new Blob([bytesToArrayBuffer(result.content)], { type: mimeFor(result.content) });
    triggerDownload(blob, `drop-${result.purchase.dropId}`);
  }, [result]);

  return (
    <section className="panel unlocked">
      <div className="panel-head">
        <h2>🎉 Unlocked “{result.purchase.title}”</h2>
        <button onClick={onDone}>Back to catalog</button>
      </div>
      {kind === "image" ? <img className="content-img" src={objectUrl} alt={result.purchase.title} /> : null}
      {kind === "video" ? <video className="content-video" src={objectUrl} controls autoPlay loop /> : null}
      {kind === "text" ? <pre className="content-text">{text}</pre> : null}
      {kind === "binary" ? <p className="note">Binary content — use download.</p> : null}
      <button onClick={download}>Download content</button>
    </section>
  );
}

// Standalone tool: browse every published dispatch blob and unlock one by uploading a recovery
// file (which carries e_priv). Decoupled from the live purchase flow — trial-opens ALL blobs, so
// it works regardless of which purchase/e_pub the browser currently holds.
function ManualUnlock({ api }: { api: DropApi }) {
  const [keys, setKeys] = useState<string[]>([]);
  const [result, setResult] = useState<UnlockResult | null>(null);
  const [status, setStatus] = useState("");

  const refresh = useCallback(async () => {
    setStatus("");
    try {
      setKeys(await api.listDispatch());
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    }
  }, [api]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const onFile = useCallback(
    async (file: File) => {
      setResult(null);
      setStatus("trial-opening every blob…");
      await sodiumReady();
      try {
        const rec = fromRecoveryFile(await file.text());
        const dispatchKeys = await api.listDispatch();
        setKeys(dispatchKeys);
        for (const key of dispatchKeys) {
          let blob: Uint8Array;
          try {
            blob = await api.getDispatch(key);
          } catch {
            continue;
          }
          const kDrop = trySealOpen(blob, rec.ePub, rec.ePriv);
          if (!kDrop) continue; // sealed to a different e_pub — not this recovery file's
          const content = await api.getContent(rec.hContent);
          const plaintext = await decryptContent(content, kDrop);
          setResult({ purchase: rec, kDrop, content: plaintext });
          setStatus(`✅ unlocked from blob ${key.slice(0, 12)}…`);
          return;
        }
        setStatus(`no blob matched this e_priv (tried ${dispatchKeys.length}). Wrong recovery file, or payment not dispatched yet.`);
      } catch (e) {
        setStatus(e instanceof Error ? e.message : String(e));
      }
    },
    [api]
  );

  return (
    <section className="panel">
      <div className="panel-head">
        <h2>Manual unlock — dispatch blobs</h2>
        <button onClick={() => void refresh()}>Refresh</button>
      </div>
      <p className="note">Published dispatch blobs on the indexer: {keys.length}</p>
      <ul className="drops">
        {keys.map((k) => (
          <li key={k}>
            <code className="uri">{k}</code>
          </li>
        ))}
      </ul>
      <label>
        Upload recovery file (holds e_priv) — trial-opens every blob, decrypts the match:
        <input
          type="file"
          accept="application/json"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void onFile(f);
            e.currentTarget.value = "";
          }}
        />
      </label>
      {status ? <p className="note">{status}</p> : null}
      {result ? <Unlocked result={result} onDone={() => setResult(null)} /> : null}
    </section>
  );
}

function triggerDownload(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
