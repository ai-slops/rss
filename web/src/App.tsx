import { AuthScreen } from "./components/AuthScreen"
import { ProductionWorkspace } from "./components/ProductionWorkspace"
import { useProduction } from "./state/useProduction"

export function App() {
  const production = useProduction()

  if (production.connection === "unauthorized") {
    return <AuthScreen busy={false} error={production.error} onLogin={production.authenticate} />
  }

  if (production.snapshot === null) {
    return (
      <main className="boot-screen" aria-live="polite">
        <div className="signal-loader" />
        <strong>Connecting to production coordinator</strong>
        <span>{production.error ?? "Loading authoritative state..."}</span>
        {production.connection === "stale" ? <button type="button" onClick={() => void production.refresh()}>Retry connection</button> : null}
      </main>
    )
  }

  return <ProductionWorkspace {...production} snapshot={production.snapshot} />
}
