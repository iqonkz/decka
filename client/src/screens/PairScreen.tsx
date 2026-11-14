import { useState, type FormEvent } from "react";
import { normalizeCode, parseHostInput, parsePairUrl } from "@decka/shared";
import { pairErrorText, pairWith } from "../app/controller";
import { isNative } from "../platform/native";
import { scanQr } from "../platform/scan";
import { useAppState } from "../store/store";
import type { ConnFailure } from "../net/connection";

export default function PairScreen() {
  const native = isNative();
  const initialError = useAppState((s) => s.lastError);
  // In the browser the page is served by the PC itself, so the address is known and hidden.
  const [address, setAddress] = useState(native ? "" : location.host);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(initialError);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    const host = parseHostInput(address);
    if (!host) return setError("Enter a valid address");
    const normalized = normalizeCode(code);
    if (!normalized) return setError("Enter the 8-character code");
    setError(null);
    setBusy(true);
    try {
      await pairWith([host.host], host.port, normalized, host.host);
    } catch (err) {
      setError(pairErrorText((err as ConnFailure).code));
      setBusy(false);
    }
  }

  async function scan() {
    setError(null);
    const text = await scanQr();
    if (text === null) return; // cancelled: stay on the screen
    const parsed = parsePairUrl(text);
    if (!parsed) return setError("This is not a Decka QR code");
    setBusy(true);
    try {
      await pairWith(parsed.hosts, parsed.port, parsed.code, parsed.agentName);
    } catch (err) {
      setError(pairErrorText((err as ConnFailure).code));
      setBusy(false);
    }
  }

  return (
    <form className="pair" onSubmit={submit}>
      <h1>Pair with your PC</h1>
      <p className="hint">Open Decka on your PC → Pairing, then scan the QR code or type the code below.</p>
      {native && (
        <button className="primary" type="button" disabled={busy} onClick={() => void scan()}>
          Scan QR code
        </button>
      )}
      {native && (
        <input
          className="field"
          aria-label="PC address"
          placeholder="192.168.1.10"
          inputMode="url"
          autoCapitalize="off"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
        />
      )}
      <input
        className="field code"
        aria-label="Code"
        placeholder="XXXX-XXXX"
        autoCapitalize="characters"
        autoComplete="off"
        maxLength={9}
        value={code}
        onChange={(e) => setCode(e.target.value.toUpperCase())}
      />
      {error && <p className="error">{error}</p>}
      <button className="primary" type="submit" disabled={busy}>
        {busy ? "Pairing…" : "Pair"}
      </button>
    </form>
  );
}
