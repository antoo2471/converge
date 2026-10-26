import {
  createContext,
  useContext,
  useState,
  useRef,
  useCallback,
  type ReactNode,
} from "react";
import { useDialog } from "../lib/useDialog";

type Request = {
  title: string;
  initial?: string;
  resolve: (value: string | null) => void;
};
const Context = createContext<{
  askText: (title: string, initial?: string) => Promise<string | null>;
  askConfirm: (title: string) => Promise<boolean>;
}>(null!);

export const useAppDialogs = () => useContext(Context);
export function AppDialogs({ children }: { children: ReactNode }) {
  const [request, setRequest] = useState<Request | null>(null);
  const [value, setValue] = useState("");
  const pending = useRef<Request | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const finish = useCallback((result: string | null) => {
    pending.current?.resolve(result);
    pending.current = null;
    setRequest(null);
  }, []);
  const close = useCallback(() => finish(null), [finish]);
  useDialog(!!request, ref, close);
  const ask = (title: string, initial?: string) =>
    new Promise<string | null>((resolve) => {
      if (pending.current) {
        resolve(null);
        return;
      }
      const next = { title, initial, resolve };
      pending.current = next;
      setValue(initial ?? "");
      setRequest(next);
    });
  return (
    <Context.Provider
      value={{
        askText: (title, initial = "") => ask(title, initial),
        askConfirm: async (title) => (await ask(title)) !== null,
      }}
    >
      {children}
      {request && (
        <div
          className="app-dialog-backdrop"
          onMouseDown={(e) => {
            if (e.target === e.currentTarget) close();
          }}
        >
          <div
            ref={ref}
            role="dialog"
            aria-modal="true"
            aria-labelledby="app-dialog-title"
            className="app-dialog"
          >
            <form
              onSubmit={(e) => {
                e.preventDefault();
                if (request.initial === undefined || value.trim())
                  finish(request.initial === undefined ? "yes" : value.trim());
              }}
            >
              <h2 id="app-dialog-title">{request.title}</h2>
              {request.initial !== undefined && (
                <input
                  autoFocus
                  aria-label={request.title}
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  maxLength={120}
                />
              )}
              <div className="app-dialog-actions">
                <button type="button" className="button" onClick={close}>
                  Annuler
                </button>
                <button
                  className="button primary"
                  disabled={request.initial !== undefined && !value.trim()}
                >
                  {request.initial === undefined ? "Confirmer" : "Enregistrer"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </Context.Provider>
  );
}
